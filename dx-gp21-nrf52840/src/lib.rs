//! nRF52840 board support for the **DX-GP21-A** GNSS module — Embassy edition.
//!
//! Everything runs on `embassy-nrf`: the UART is an embassy [`Uarte`], the power
//! pin an embassy [`Output`], and the 1PPS pin an embassy [`Input`] whose
//! `wait_for_rising_edge` is woken by the nRF's GPIOTE PORT event. The GPIOTE IRQ
//! is enabled inside `embassy_nrf::init` — no NVIC/GPIOTE setup here and no
//! caller ISR for 1PPS.
//!
//! # Caller provides the state and the interrupt
//!
//! The module holds **no** GNSS state and defines **no** interrupt handlers:
//!
//! - the caller owns the [`GnssState`] in a `&'static Mutex` and passes it into
//!   [`DxGp21GnssModule::new`] — two independent modules can run side by side
//!   with separate states, and the library keeps no globals;
//! - the UARTE interrupt stays owned by the app: the caller's `bind_interrupts!`
//!   struct is passed as `irq` to [`DxGp21GnssModule::new`].
//!
//! # Driving the module
//!
//! - [`DxGp21GnssModule::spawn`] — launch the UART feed loop and (when a PPS pin
//!   was given) a 1PPS task as executor tasks; the caller's `on_pps` callback
//!   runs after every rising edge.
//! - [`DxGp21GnssModule::run`] / [`DxGp21GnssModule::wait_1pps`] — drive the feed
//!   loop or one 1PPS edge by hand instead.
//! - `with_state` / `position` / `has_fix` / … — read the shared state from any
//!   task; the feed loop updates it under a critical section, never across an
//!   `.await`, so other tasks can read it concurrently.
//!
//! # Hardware connections
//!
//! Pin labels are the silkscreen markings on the DX-GP21-A 6-pin connector.
//!
//! | Module pad | Signal | nRF52840 connection | Notes |
//! |---|---|---|---|
//! | **T** | TXD — module transmits | UART RX | NMEA sentences → MCU |
//! | **R** | RXD — module receives | UART TX | `$PCAS` commands → module |
//! | **W** | ON/OFF wake control | GPIO output (optional) | Pull-up on module → floats HIGH = on; drive LOW to shut down |
//! | **P** | 1PPS timing pulse | GPIO input (optional) | Rising edge 1 Hz after GNSS fix |
//! | **V** | VCC power supply | 3.6 – 6 V | Module has onboard LDO; supply must provide ≥ 100 mA |
//! | **G** | GND | GND | |
//!
//! # Example
//!
//! ```ignore
//! use core::cell::RefCell;
//! use critical_section::Mutex;
//! use embassy_nrf::bind_interrupts;
//! use embassy_nrf::gpio::{Input, Level, Output, OutputDrive, Pull};
//! use embassy_nrf::peripherals;
//! use embassy_nrf::uarte;
//! use dx_gp21_embedded::GnssState;
//! use dx_gp21_nrf52840::DxGp21GnssModule;
//!
//! bind_interrupts!(struct Irqs {
//!     UARTE0 => uarte::InterruptHandler<peripherals::UARTE0>;
//! });
//!
//! static GNSS_STATE: Mutex<RefCell<GnssState<64>>> = Mutex::new(RefCell::new(GnssState::new()));
//!
//! // In `main`, after `embassy_nrf::init`:
//! let gnss = DxGp21GnssModule::new(
//!     p.UARTE0, p.P0_08, p.P0_06, Irqs,
//!     Some(Output::new(p.P0_13, Level::High, OutputDrive::Standard)),
//!     Some(Input::new(p.P0_14, Pull::Down)),
//!     &GNSS_STATE,
//! );
//! gnss.spawn(spawner, |snap| {
//!     if let Some((lat, lon)) = snap.lat.zip(snap.lon) {
//!         defmt::info!("1PPS fix {:.6} {:.6}", lat, lon);
//!     }
//! });
//! ```

#![no_std]

use core::cell::RefCell;

use critical_section::Mutex;
use embassy_executor::Spawner;
use embassy_nrf::Peri;
use embassy_nrf::gpio::{Input, Output, Pin as GpioPin};
use embassy_nrf::interrupt::typelevel::Binding;
use embassy_nrf::uarte::{self, Instance, Uarte, UarteRx, UarteTx};

use dx_gp21_core::{
    AntennaStatus, CommandSink, DopValues, FixMode, NmeaDate, NmeaTime, ParsedSentence,
    feed_sentence,
};

// Re-exported so a consumer (like sonde) can depend on just this crate.
pub use dx_gp21_core::GnssStore;
pub use dx_gp21_embedded::GnssState;

/// The satellite-table size the module is built around.
///
/// Embassy `#[task]`s cannot be generic, so the state type is fixed to the
/// crate default of 64 satellites rather than parameterised.
const MAX_SATS: usize = 64;

// ── Errors ────────────────────────────────────────────────────────────────────

/// Errors returned by [`DxGp21GnssModule`] methods.
#[derive(Debug)]
pub enum ModuleError {
    /// UART write error.
    Uart(embassy_nrf::uarte::Error),
}

impl From<embassy_nrf::uarte::Error> for ModuleError {
    fn from(e: embassy_nrf::uarte::Error) -> Self {
        Self::Uart(e)
    }
}

/// Errors from [`DxGp21GnssModule::spawn`].
#[derive(Debug)]
pub enum SpawnError {
    /// The executor failed to spawn a task (arena full).
    Executor(embassy_executor::SpawnError),
}

/// The GNSS facts a 1PPS consumer usually logs, copied out of the shared state
/// and handed to the `on_pps` callback **by value**.
///
/// It is passed by value (not as a `&GnssState`) because embassy `#[task]`
/// arguments must be `'static`, and the callback is a plain `fn` pointer.
#[derive(Clone, Copy, Debug)]
pub struct FixSnapshot {
    /// Whether a 2D/3D fix was present at the pulse.
    pub has_fix: bool,
    /// Latitude in decimal degrees, when a GGA fix is present.
    pub lat: Option<f64>,
    /// Longitude in decimal degrees, when a GGA fix is present.
    pub lon: Option<f64>,
    /// UTC time from the most recent RMC sentence.
    pub utc_time: Option<NmeaTime>,
    /// UTC date from the most recent RMC sentence.
    pub utc_date: Option<NmeaDate>,
}

impl FixSnapshot {
    /// Snapshot the current fix facts out of the shared state.
    ///
    /// Position comes from the latest valid GGA sentence, falling back to the
    /// RMC sentence — some modules output RMC/GSA/GSV but not GGA, in which case
    /// `position()` is empty while RMC still carries the latitude/longitude.
    fn of(state: &GnssState<MAX_SATS>) -> Self {
        let (lat, lon) = match state.position() {
            Some((lat, lon)) => (Some(lat), Some(lon)),
            None => match state.rmc().filter(|r| r.is_valid()) {
                Some(r) => (Some(r.lat), Some(r.lon)),
                None => (None, None),
            },
        };
        Self {
            has_fix: state.has_fix(),
            lat,
            lon,
            utc_time: state.utc_time(),
            utc_date: state.utc_date(),
        }
    }
}

// ── Power ─────────────────────────────────────────────────────────────────────

/// Desired power state for [`DxGp21GnssModule::set_power`].
///
/// The **W** pad has an **onboard pull-up resistor** that holds it HIGH by
/// default, keeping the module in full working mode when undriven (floating).
/// Shutdown is triggered by actively pulling the pad LOW against the pull-up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Power {
    On,
    Off,
}

// ── DxGp21GnssModule ─────────────────────────────────────────────────────────

/// nRF52840 driver for the DX-GP21-A GNSS module, built on embassy-nrf.
///
/// The caller supplies the [`GnssState`] storage and the UARTE interrupt binding
/// (see the crate docs); the driver itself owns no globals, so several modules
/// can coexist with separate states.
pub struct DxGp21GnssModule<'d> {
    state: &'static Mutex<RefCell<GnssState<MAX_SATS>>>,
    rx: UarteRx<'d>,
    tx: UarteTx<'d>,
    power: Option<Output<'d>>,
    pps: Option<Input<'d>>,
}

impl<'d> DxGp21GnssModule<'d> {
    /// Build the module driver at 115200 8N1.
    ///
    /// - `uarte`/`rxd`/`txd` — the UARTE instance and its two data pins.
    /// - `irq` — the app's `bind_interrupts!` struct that binds the UARTE
    ///   interrupt to `uarte::InterruptHandler` (the app keeps interrupt
    ///   ownership).
    /// - `power` — the **W** pad output, if wired (see [`Power`]).
    /// - `pps` — the **P** pad input, if wired (see [`DxGp21GnssModule::wait_1pps`]).
    /// - `state` — the caller-owned GNSS state storage.
    pub fn new<T>(
        uarte: Peri<'d, T>,
        rxd: Peri<'d, impl GpioPin>,
        txd: Peri<'d, impl GpioPin>,
        irq: impl Binding<T::Interrupt, uarte::InterruptHandler<T>> + 'd,
        power: Option<Output<'d>>,
        pps: Option<Input<'d>>,
        state: &'static Mutex<RefCell<GnssState<MAX_SATS>>>,
    ) -> Self
    where
        T: Instance,
    {
        let mut config = uarte::Config::default();
        config.baudrate = uarte::Baudrate::Baud115200;
        let uart = Uarte::new(uarte, rxd, txd, irq, config);
        let (tx, rx) = uart.split();
        Self {
            state,
            rx,
            tx,
            power,
            pps,
        }
    }

    /// Build the module from a **pre-built** UARTE (configured for 115200 8N1).
    ///
    /// Use this when the caller needs the UART first — e.g. to probe whether the
    /// module answers before wiring the 1PPS pin — then hands it over.
    pub fn from_uart(
        uart: Uarte<'d>,
        power: Option<Output<'d>>,
        pps: Option<Input<'d>>,
        state: &'static Mutex<RefCell<GnssState<MAX_SATS>>>,
    ) -> Self {
        let (tx, rx) = uart.split();
        Self {
            state,
            rx,
            tx,
            power,
            pps,
        }
    }

    // ── Power control ─────────────────────────────────────────────────────────

    /// Drive the ON/OFF (W) pad.
    ///
    /// Silent no-op when the pin was not provided. Allow ≥ 100 ms after
    /// [`Power::On`] before reading NMEA.
    pub fn set_power(&mut self, power: Power) {
        if let Some(pin) = &mut self.power {
            match power {
                Power::On => pin.set_high(),
                Power::Off => pin.set_low(),
            };
        }
    }

    /// Returns the current logic level of the 1PPS (P) pad, or `false` if absent.
    pub fn pps_is_high(&mut self) -> bool {
        self.pps.as_mut().map(|p| p.is_high()).unwrap_or(false)
    }

    // ── State access ──────────────────────────────────────────────────────────

    /// Run `f` with a shared reference to the GNSS state inside a critical section.
    pub fn with_state<R>(&self, f: impl FnOnce(&GnssState<MAX_SATS>) -> R) -> R {
        critical_section::with(|cs| f(&self.state.borrow_ref(cs)))
    }

    /// Returns `true` when a 2D or 3D fix is available.
    pub fn has_fix(&self) -> bool {
        self.with_state(|s| s.has_fix())
    }

    /// Returns `(latitude_deg, longitude_deg)` when a valid GGA fix is present.
    pub fn position(&self) -> Option<(f64, f64)> {
        self.with_state(|s| s.position())
    }

    /// Returns altitude above mean sea level in metres.
    pub fn altitude_msl(&self) -> Option<f32> {
        self.with_state(|s| s.altitude_msl())
    }

    /// Returns the most recent UTC time.
    pub fn utc_time(&self) -> Option<NmeaTime> {
        self.with_state(|s| s.utc_time())
    }

    /// Returns the most recent UTC date.
    pub fn utc_date(&self) -> Option<NmeaDate> {
        self.with_state(|s| s.utc_date())
    }

    /// Returns ground speed in km/h.
    pub fn speed_kmh(&self) -> Option<f32> {
        self.with_state(|s| s.speed_kmh())
    }

    /// Returns true course over ground in degrees.
    pub fn course_deg(&self) -> Option<f32> {
        self.with_state(|s| s.course_deg())
    }

    /// Returns `(used_sats, in_view_sats)`.
    pub fn satellite_count(&self) -> (u8, u8) {
        self.with_state(|s| (s.sats_used_count(), s.sats_in_view_count()))
    }

    /// Returns the current fix mode.
    pub fn fix_mode(&self) -> FixMode {
        self.with_state(|s| s.fix_mode())
    }

    /// Returns DOP values.
    pub fn dop(&self) -> DopValues {
        self.with_state(|s| s.dop())
    }

    /// Returns the antenna status.
    pub fn antenna(&self) -> AntennaStatus {
        self.with_state(|s| s.antenna())
    }

    // ── Sentence feeding ──────────────────────────────────────────────────────

    /// Parse one raw NMEA line into the shared state.
    pub fn feed(&mut self, line: &[u8]) -> Option<ParsedSentence> {
        critical_section::with(|cs| feed_sentence(&mut *self.state.borrow_ref_mut(cs), line))
    }

    // ── Async driving ─────────────────────────────────────────────────────────

    /// Feed NMEA forever from the UART RX into the shared state. Never returns.
    ///
    /// Prefer [`Self::spawn`] when you also want the 1PPS task — `run` borrows
    /// the module for its whole lifetime, so it cannot run alongside
    /// [`Self::wait_1pps`] on the same value.
    pub async fn run(&mut self) -> ! {
        feed_loop(&mut self.rx, self.state).await
    }

    /// Wait for the next 1PPS rising edge on the P pin (GPIOTE PORT event).
    ///
    /// Returns whether a fix is present at the moment of the pulse. Returns
    /// `false` immediately if no PPS pin was provided.
    pub async fn wait_1pps(&mut self) -> bool {
        match &mut self.pps {
            Some(pps) => {
                pps.wait_for_rising_edge().await;
                self.has_fix()
            }
            None => false,
        }
    }
}

impl DxGp21GnssModule<'static> {
    /// Launch the UART feed loop and — when a PPS pin was given — a 1PPS task as
    /// executor tasks, calling `on_pps` with a [`FixSnapshot`] after every 1PPS
    /// rising edge.
    ///
    /// `on_pps` is a plain function pointer (embassy `#[task]`s cannot be
    /// generic and their args must be `'static`), so it must not capture — read
    /// the [`FixSnapshot`] it is handed instead; for anything richer, read the
    /// shared state directly (the caller owns it).
    ///
    /// The feed task updates the shared state; the module's TX half and power
    /// pin are consumed (the power pin's level is latched by the module's
    /// onboard pull-up); use [`Self::run`]/[`Self::wait_1pps`] if you need
    /// either after spawning.
    pub fn spawn(self, spawner: Spawner, on_pps: fn(FixSnapshot)) -> Result<(), SpawnError> {
        let state = self.state;
        let feed = feed_task(self.rx, state).map_err(SpawnError::Executor)?;
        spawner.spawn(feed);
        if let Some(pps) = self.pps {
            let pps = pps_task(pps, state, on_pps).map_err(SpawnError::Executor)?;
            spawner.spawn(pps);
        }
        Ok(())
    }
}

// ── CommandSink ───────────────────────────────────────────────────────────────

impl CommandSink for DxGp21GnssModule<'_> {
    type Error = ModuleError;

    fn send_raw(&mut self, bytes: &[u8]) -> Result<(), ModuleError> {
        self.tx.blocking_write(bytes).map_err(ModuleError::Uart)
    }
}

// ── Internal tasks ────────────────────────────────────────────────────────────

/// Accumulate DMA bursts into NMEA lines and feed them into the shared state.
///
/// The state mutex is locked only for the microseconds of `feed_sentence`, never
/// across an `.await`, so other tasks can read the state concurrently.
async fn feed_loop(rx: &mut UarteRx<'_>, state: &'static Mutex<RefCell<GnssState<MAX_SATS>>>) -> ! {
    let mut dma = [0u8; 64];
    let mut line = [0u8; 132];
    let mut n = 0usize;
    loop {
        // Yield until a DMA burst fills `dma`; errors (overrun, etc.) are
        // cleared by the next read, so just keep going.
        if rx.read(&mut dma).await.is_err() {
            continue;
        }
        for &b in dma.iter() {
            match b {
                b'\n' => {
                    if n > 0 {
                        critical_section::with(|cs| {
                            feed_sentence(&mut *state.borrow_ref_mut(cs), &line[..n]);
                        });
                        n = 0;
                    }
                }
                b'\r' => {}
                b if n < line.len() => {
                    line[n] = b;
                    n += 1;
                }
                _ => n = 0,
            }
        }
    }
}

#[embassy_executor::task]
async fn feed_task(
    mut rx: UarteRx<'static>,
    state: &'static Mutex<RefCell<GnssState<MAX_SATS>>>,
) -> ! {
    feed_loop(&mut rx, state).await
}

/// Wait for 1PPS rising edges and invoke the caller's callback with a
/// [`FixSnapshot`] of the shared state.
///
/// Edge-triggered via the GPIOTE PORT event (`Input::wait_for_rising_edge`),
/// which catches any pulse width with zero CPU between pulses. The GPIOTE IRQ is
/// unmasked by `gpiote::init` (from `embassy_nrf::init`) and the vector is
/// wired; the pin is the correct A2/P0.28 with `Pull::Up` on the module's
/// open-drain P output, so the 1 Hz rising edge is a real signal. [`PPS_EDGES`]
/// counts every edge so the caller can confirm the interrupt path fires.
#[embassy_executor::task]
async fn pps_task(
    mut pps: Input<'static>,
    state: &'static Mutex<RefCell<GnssState<MAX_SATS>>>,
    on_pps: fn(FixSnapshot),
) -> ! {
    loop {
        pps.wait_for_rising_edge().await;
        let snap = critical_section::with(|cs| FixSnapshot::of(&state.borrow_ref(cs)));
        on_pps(snap);
    }
}

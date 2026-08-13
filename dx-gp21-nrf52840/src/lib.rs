//! nRF52840 board support for the **DX-GP21-A** GNSS module.
//!
//! Bundles UART, optional power-control pin, and optional 1PPS pin into a
//! single [`DxGp21GnssModule`] struct. The caller owns the GNSS state storage
//! and the 1PPS callback — the library holds no global singletons, so two
//! independent modules can run side by side with separate states.
//!
//! # Two access patterns
//!
//! | Pattern | Method | When to use |
//! |---|---|---|
//! | Sync | [`DxGp21GnssModule::run_sync`] | Blocking loop, no RTOS |
//! | Async | `EmbeddedSession::run` in `dx-gp21-embedded` | Framework-agnostic async loop (feature = `"async"`) |
//! | Interrupt-driven | [`DxGp21GnssModule::install_1pps_interrupt`] | React in GPIOTE ISR on 1PPS |
//!
//! # Build
//!
//! ```text
//! rustup target add thumbv7em-none-eabihf
//! cargo build -p dx-gp21-nrf52840 --target thumbv7em-none-eabihf
//! ```
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
//! # Example — two independent modules
//!
//! ```no_run
//! use core::cell::RefCell;
//! use critical_section::Mutex;
//! use nrf52840_hal::{pac::Peripherals, uarte, gpio, gpiote::Gpiote, pac::interrupt};
//! use dx_gp21_embedded::GnssState;
//! use dx_gp21_nrf52840::{DxGp21GnssModule, Power};
//!
//! // Each module has its own caller-owned state.
//! static STATE_A: Mutex<RefCell<GnssState<64>>> =
//!     Mutex::new(RefCell::new(GnssState::new()));
//! static STATE_B: Mutex<RefCell<GnssState<64>>> =
//!     Mutex::new(RefCell::new(GnssState::new()));
//!
//! fn on_pps_a(state: &GnssState<64>) {
//!     if state.has_fix() { /* BLE notification for module A */ }
//! }
//!
//! fn on_pps_b(state: &GnssState<64>) {
//!     if state.has_fix() { /* log fix from module B */ }
//! }
//!
//! // Caller wires up both callbacks in the GPIOTE ISR.
//! #[interrupt]
//! fn GPIOTE() {
//!     critical_section::with(|cs| on_pps_a(&STATE_A.borrow_ref(cs)));
//!     critical_section::with(|cs| on_pps_b(&STATE_B.borrow_ref(cs)));
//! }
//!
//! // Construction: caller passes their own state reference.
//! let module_a = DxGp21GnssModule::new(tx_a, rx_a, Some(on_off_a), Some(pps_a), &STATE_A);
//! let module_b = DxGp21GnssModule::new(tx_b, rx_b, None,           Some(pps_b), &STATE_B);
//! ```

#![no_std]

use core::cell::RefCell;

use critical_section::Mutex;
use embedded_hal::digital::v2::{InputPin, OutputPin};
use nrf52840_hal::{
    gpio::{Floating, Input, Output, Pin, PushPull},
    gpiote::Gpiote,
    uarte::Uarte,
};

use dx_gp21_core::{
    command::CommandSink,
    state::GnssStore,
    types::*,
    ParsedSentence, feed_sentence,
};
use dx_gp21_embedded::GnssState;

// ── Errors ────────────────────────────────────────────────────────────────────

/// Errors returned by [`DxGp21GnssModule`] methods.
#[derive(Debug)]
pub enum ModuleError {
    /// A 1PPS operation was requested but no PPS pin was provided at construction.
    NoPpsPin,
    /// UART write error.
    Uart(nrf52840_hal::uarte::Error),
}

impl From<nrf52840_hal::uarte::Error> for ModuleError {
    fn from(e: nrf52840_hal::uarte::Error) -> Self { Self::Uart(e) }
}

// ── Power ─────────────────────────────────────────────────────────────────────

/// Desired power state for [`DxGp21GnssModule::set_power`].
///
/// The **W** pad has an **onboard pull-up resistor** that holds it HIGH by
/// default, keeping the module in full working mode when undriven (floating).
/// Shutdown is triggered by actively pulling the pad LOW against the pull-up.
///
/// - [`Power::On`]  → drive HIGH (redundant if floating, but explicit)
/// - [`Power::Off`] → drive LOW  → overrides pull-up, activates shutdown
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Power { On, Off }

// ── DxGp21GnssModule ─────────────────────────────────────────────────────────

/// nRF52840 driver for the DX-GP21-A GNSS module.
///
/// Holds no global state — the caller provides a `&'static` reference to their
/// own [`GnssState`] storage, enabling multiple independent instances.
///
/// `T` is the UARTE peripheral instance (e.g. `UARTE0`).
/// `N` must match the `N` in the caller-provided [`GnssState<N>`] static.
pub struct DxGp21GnssModule<'s, T, const N: usize = 64>
where
    T: nrf52840_hal::uarte::Instance,
{
    state: &'s Mutex<RefCell<GnssState<N>>>,
    uart: Uarte<T>,
    /// ON/OFF control. `None` = power assumed always on.
    on_off: Option<Pin<Output<PushPull>>>,
    /// 1PPS input. `None` = 1PPS operations return `Err(NoPpsPin)`.
    pps: Option<Pin<Input<Floating>>>,
}

impl<'s, T, const N: usize> DxGp21GnssModule<'s, T, N>
where
    T: nrf52840_hal::uarte::Instance,
{
    // ── Construction ──────────────────────────────────────────────────────────

    /// Create the module driver.
    ///
    /// `state` is a caller-owned `&'s Mutex<RefCell<GnssState<N>>>`.
    /// Declaring it as a `static` allows multiple modules to coexist with
    /// fully separate states and 1PPS callbacks.
    ///
    /// ```no_run
    /// static MY_STATE: Mutex<RefCell<GnssState<64>>> =
    ///     Mutex::new(RefCell::new(GnssState::new()));
    ///
    /// let gnss = DxGp21GnssModule::new(uart, Some(on_off), Some(pps), &MY_STATE);
    /// ```
    pub fn new(
        uart: Uarte<T>,
        on_off: Option<Pin<Output<PushPull>>>,
        pps: Option<Pin<Input<Floating>>>,
        state: &'s Mutex<RefCell<GnssState<N>>>,
    ) -> Self {
        critical_section::with(|cs| {
            *state.borrow_ref_mut(cs) = GnssState::new();
        });
        Self { state, uart, on_off, pps }
    }

    // ── Power control ─────────────────────────────────────────────────────────

    /// Drive the ON/OFF (W) pad.
    ///
    /// Silent no-op when the pin was not provided. Allow ≥ 100 ms after
    /// [`Power::On`] before reading NMEA.
    pub fn set_power(&mut self, power: Power) {
        if let Some(pin) = &mut self.on_off {
            match power {
                Power::On  => pin.set_high().ok(),
                Power::Off => pin.set_low().ok(),
            };
        }
    }

    // ── 1PPS ─────────────────────────────────────────────────────────────────

    /// Returns the current logic level of the 1PPS (P) pad, or `false` if absent.
    pub fn pps_is_high(&self) -> bool {
        self.pps.as_ref().and_then(|p| p.is_high().ok()).unwrap_or(false)
    }

    /// Configure GPIOTE channel 0 for a **rising-edge** interrupt on the 1PPS pin.
    ///
    /// The library stores no callback — the caller writes their own GPIOTE ISR
    /// and accesses their state directly. This allows each module instance to
    /// have an independent callback:
    ///
    /// ```no_run
    /// // Configure hardware:
    /// gnss.install_1pps_interrupt(&mut gpiote)?;
    /// unsafe { NVIC::unmask(Interrupt::GPIOTE); }
    ///
    /// // User-owned ISR (accesses their own static state):
    /// #[interrupt]
    /// fn GPIOTE() {
    ///     critical_section::with(|cs| {
    ///         let state = MY_STATE.borrow_ref(cs);
    ///         if state.has_fix() { /* react */ }
    ///     });
    /// }
    /// ```
    ///
    /// Returns [`ModuleError::NoPpsPin`] if no PPS pin was provided.
    pub fn install_1pps_interrupt(
        &self,
        gpiote: &mut Gpiote,
    ) -> Result<(), ModuleError> {
        let pps = self.pps.as_ref().ok_or(ModuleError::NoPpsPin)?;
        gpiote.channel0().input_pin(pps).lo_to_hi().enable_interrupt();
        Ok(())
    }

    /// Configure GPIOTE channel 1 so a rising 1PPS edge wakes the MCU from `WFE`.
    ///
    /// Returns [`ModuleError::NoPpsPin`] if no PPS pin was provided.
    pub fn install_wake_on_1pps(&self, gpiote: &mut Gpiote) -> Result<(), ModuleError> {
        let pps = self.pps.as_ref().ok_or(ModuleError::NoPpsPin)?;
        gpiote.channel1().input_pin(pps).lo_to_hi().enable_interrupt();
        Ok(())
    }

    // ── State access ──────────────────────────────────────────────────────────

    /// Run `f` with a shared reference to the GNSS state inside a critical section.
    pub fn with_state<R>(&self, f: impl FnOnce(&GnssState<N>) -> R) -> R {
        critical_section::with(|cs| f(&self.state.borrow_ref(cs)))
    }

    /// Returns `true` when a 2D or 3D fix is available.
    pub fn has_fix(&self) -> bool { self.with_state(|s| s.has_fix()) }

    /// Returns `(latitude_deg, longitude_deg)` when a valid GGA fix is present.
    pub fn position(&self) -> Option<(f64, f64)> { self.with_state(|s| s.position()) }

    /// Returns altitude above mean sea level in metres.
    pub fn altitude_msl(&self) -> Option<f32> { self.with_state(|s| s.altitude_msl()) }

    /// Returns the most recent UTC time.
    pub fn utc_time(&self) -> Option<NmeaTime> { self.with_state(|s| s.utc_time()) }

    /// Returns the most recent UTC date.
    pub fn utc_date(&self) -> Option<NmeaDate> { self.with_state(|s| s.utc_date()) }

    /// Returns ground speed in km/h.
    pub fn speed_kmh(&self) -> Option<f32> { self.with_state(|s| s.speed_kmh()) }

    /// Returns true course over ground in degrees.
    pub fn course_deg(&self) -> Option<f32> { self.with_state(|s| s.course_deg()) }

    /// Returns `(used_sats, in_view_sats)`.
    pub fn satellite_count(&self) -> (u8, u8) {
        self.with_state(|s| (s.sats_used_count(), s.sats_in_view_count()))
    }

    /// Returns the current fix mode.
    pub fn fix_mode(&self) -> FixMode { self.with_state(|s| s.fix_mode()) }

    /// Returns DOP values.
    pub fn dop(&self) -> DopValues { self.with_state(|s| s.dop()) }

    /// Returns the antenna status.
    pub fn antenna(&self) -> AntennaStatus { self.with_state(|s| s.antenna()) }

    // ── Sentence feeding ──────────────────────────────────────────────────────

    /// Parse one raw NMEA line into the caller-provided state.
    pub fn feed(&mut self, line: &[u8]) -> Option<ParsedSentence> {
        critical_section::with(|cs| {
            feed_sentence(&mut *self.state.borrow_ref_mut(cs), line)
        })
    }

    /// Read one `\n`-terminated NMEA line from UART RX into `buf`.
    pub fn read_line(&mut self, buf: &mut [u8]) -> Result<usize, nrf52840_hal::uarte::Error> {
        let mut n = 0;
        loop {
            let mut b = [0u8; 1];
            self.uart.read(&mut b)?;
            if b[0] == b'\n' { break; }
            if b[0] == b'\r' { continue; }
            if n < buf.len() { buf[n] = b[0]; n += 1; }
        }
        Ok(n)
    }

    // ── Run loops ─────────────────────────────────────────────────────────────

    /// Blocking main loop. Never returns.
    pub fn run_sync(&mut self) -> ! {
        let mut buf = [0u8; 128];
        loop {
            if let Ok(n) = self.read_line(&mut buf) {
                self.feed(&buf[..n]);
            }
        }
    }

    // ── Async usage ───────────────────────────────────────────────────────────
    //
    // The async byte-reading loop lives in `dx-gp21-embedded` as
    // `EmbeddedSession::run` (feature = "async"), keeping it device-agnostic.
    // `dx-gp21-nrf52840` enables that feature unconditionally.
    //
    // For Embassy, construct an EmbeddedSession wrapping this module's state:
    //
    //   let mut session = EmbeddedSession::new(|bytes| uart_tx.blocking_write(bytes).ok());
    //   session.run(uart_rx).await;   // <- from dx_gp21_embedded, feature "async"
}

// ── CommandSink ───────────────────────────────────────────────────────────────

impl<'s, T, const N: usize> CommandSink for DxGp21GnssModule<'s, T, N>
where
    T: nrf52840_hal::uarte::Instance,
    
{
    type Error = ModuleError;

    fn send_raw(&mut self, bytes: &[u8]) -> Result<(), ModuleError> {
        self.uart.write(bytes).map_err(ModuleError::Uart)
    }
}

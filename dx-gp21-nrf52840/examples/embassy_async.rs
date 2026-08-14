//! Async Embassy example for the DX-GP21-A GNSS module on nRF52840.
//!
//! Uses the migrated `dx-gp21-nrf52840` board layer: `DxGp21GnssModule::new`
//! takes the caller's UARTE + interrupt binding + pins + GNSS state, and
//! `spawn` launches the UART feed loop and the 1PPS task.
//!
//! # Wiring (DX-GP21-A silkscreen → nRF52840 GPIO)
//!
//! | Module pad | Function         | nRF52840       |
//! |------------|------------------|----------------|
//! | T          | UART TX → MCU RX | P0.08          |
//! | R          | UART RX ← MCU TX | P0.06          |
//! | P          | 1PPS output       | P0.14          |
//! | W          | Power ON/OFF      | P0.13 (output) |
//! | V          | 3.6–6 V supply   | VCC            |
//! | G          | GND              | GND            |
//!
//! # Build & flash
//!
//! ```text
//! cargo build --example embassy_async --target thumbv7em-none-eabi
//! probe-rs run --chip nRF52840_xxAA target/thumbv7em-none-eabi/debug/embassy_async
//! ```

#![no_std]
#![no_main]
// rust-lld prints "cannot find entry symbol _start" to stderr on every Cortex-M
// link (the entry point is the `Reset` vector, not `_start`). The
// `linker_messages` lint turns that benign message into a warning on 1.97+.
#![allow(linker_messages)]

use core::cell::RefCell;

use cortex_m_rt::entry;
use critical_section::Mutex;
use defmt_rtt as _;
use panic_probe as _;
use static_cell::StaticCell;

use embassy_executor::Executor;
use embassy_nrf::{
    bind_interrupts,
    gpio::{Input, Level, Output, OutputDrive, Pull},
    peripherals, uarte,
};

use dx_gp21_embedded::GnssState;
use dx_gp21_nrf52840::{DxGp21GnssModule, FixSnapshot};

// ── Caller-owned GNSS state ───────────────────────────────────────────────────

static GNSS_STATE: Mutex<RefCell<GnssState<64>>> = Mutex::new(RefCell::new(GnssState::new()));

// ── Peripheral interrupt binding ──────────────────────────────────────────────
// The app owns the UARTE interrupt; the module only borrows it.

bind_interrupts!(struct Irqs {
    UARTE0 => uarte::InterruptHandler<peripherals::UARTE0>;
});

// ── 1PPS callback ─────────────────────────────────────────────────────────────
// A plain `fn` (embassy tasks can't be generic): called once per second with a
// by-value snapshot of the fix.

fn on_pps(snap: FixSnapshot) {
    if let (Some(lat), Some(lon)) = (snap.lat, snap.lon) {
        defmt::info!("1PPS fix {=f64} {=f64}", lat, lon);
    } else {
        defmt::debug!("1PPS no fix");
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

static EXECUTOR: StaticCell<Executor> = StaticCell::new();

#[entry]
fn main() -> ! {
    let p = embassy_nrf::init(Default::default());

    // ── Power pin (W pad) ─────────────────────────────────────────────────────
    // Onboard pull-up: float/HIGH = on. Drive HIGH explicitly for clarity.
    let power = Output::new(p.P0_13, Level::High, OutputDrive::Standard);

    // ── 1PPS input (P pad) ───────────────────────────────────────────────────
    // GPIOTE PORT-event wake — the IRQ is enabled by `embassy_nrf::init`.
    let pps = Input::new(p.P0_14, Pull::Down);

    // ── GNSS module (UART 115200 8N1) ────────────────────────────────────────
    let gnss = DxGp21GnssModule::new(
        p.UARTE0,
        p.P0_08, // MCU RX ← module T pad
        p.P0_06, // MCU TX → module R pad
        Irqs,
        Some(power),
        Some(pps),
        &GNSS_STATE,
    );

    // ── Launch executor ───────────────────────────────────────────────────────
    let executor = EXECUTOR.init(Executor::new());
    executor.run(|spawner| {
        gnss.spawn(spawner, on_pps).unwrap();
    });
}

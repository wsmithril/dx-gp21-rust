//! Async Embassy example for the DX-GP21-A GNSS module on nRF52840.
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
//! cargo build --example embassy_async --target thumbv7em-none-eabihf
//! probe-rs run --chip nRF52840_xxAA target/thumbv7em-none-eabihf/debug/embassy_async
//! ```

#![no_std]
#![no_main]

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
    gpiote::{InputChannel, InputChannelPolarity},
    peripherals,
    uarte::{self, UarteRx, UarteTx},
};

use dx_gp21_core::state::GnssStore;
use dx_gp21_core::{feed_sentence, types::*};
use dx_gp21_embedded::GnssState;

// ── Caller-owned GNSS state ───────────────────────────────────────────────────

static GNSS_STATE: Mutex<RefCell<GnssState<64>>> =
    Mutex::new(RefCell::new(GnssState::new()));

// ── Peripheral interrupt bindings ─────────────────────────────────────────────

bind_interrupts!(struct Irqs {
    UARTE0_UART0 => uarte::InterruptHandler<peripherals::UARTE0>;
});

// ── GNSS UART task ────────────────────────────────────────────────────────────
//
// Reads DMA bursts from UarteRx, accumulates NMEA lines, updates GNSS_STATE.
// Mutex locked only during brief state-update windows (not across .await).

#[embassy_executor::task]
async fn gnss_uart_task(
    mut rx: UarteRx<'static, peripherals::UARTE0>,
    _tx: UarteTx<'static, peripherals::UARTE0>, // kept for sending $PCAS commands
) {
    let mut dma_buf  = [0u8; 128];
    let mut line_buf = [0u8; 128];
    let mut line_len = 0usize;

    loop {
        // Yield until DMA burst arrives. Mutex is NOT held here.
        let got = match rx.read(&mut dma_buf).await {
            Ok(()) => dma_buf.len(),
            Err(_) => continue,
        };

        for &b in &dma_buf[..got] {
            match b {
                b'\n' => {
                    if line_len > 0 {
                        // Lock held for microseconds only — never across .await.
                        critical_section::with(|cs| {
                            feed_sentence(
                                &mut *GNSS_STATE.borrow_ref_mut(cs),
                                &line_buf[..line_len],
                            );
                        });
                        line_len = 0;
                    }
                }
                b'\r' => {}
                b if line_len < line_buf.len() => {
                    line_buf[line_len] = b;
                    line_len += 1;
                }
                _ => { line_len = 0; }
            }
        }
    }
}

// ── 1PPS task ─────────────────────────────────────────────────────────────────
//
// Awaits the rising edge of the 1PPS pin via GPIOTE.
// Zero CPU between pulses; executor runs other tasks while waiting.

#[embassy_executor::task]
async fn pps_task(pps_channel: InputChannel<'static>) {
    loop {
        // Suspend until GPS-epoch-aligned 1PPS rising edge.
        pps_channel.wait().await;

        critical_section::with(|cs| {
            let state = GNSS_STATE.borrow_ref(cs);

            if state.has_fix() {
                let (lat, lon)   = state.position().unwrap_or((0.0, 0.0));
                let alt          = state.altitude_msl().unwrap_or(0.0);
                let dop          = state.dop();
                let (used, view) = (state.sats_used_count(), state.sats_in_view_count());

                defmt::info!(
                    "1PPS {} {} | {=f64} {=f64} alt={=f32} m | sats {}/{} | {}",
                    state.utc_date().unwrap_or_default(),
                    state.utc_time().unwrap_or_default(),
                    lat, lon, alt,
                    used, view,
                    dop,
                );
            } else {
                defmt::debug!(
                    "1PPS {} | {} | antenna: {}",
                    state.utc_time().unwrap_or_default(),
                    state.fix_mode(),
                    state.antenna(),
                );
            }
        });
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

static EXECUTOR: StaticCell<Executor> = StaticCell::new();

#[entry]
fn main() -> ! {
    let p = embassy_nrf::init(Default::default());

    // ── Power pin (W pad) ─────────────────────────────────────────────────────
    // Onboard pull-up: float/HIGH = on. Drive HIGH explicitly for clarity.
    let _power = Output::new(p.P0_13, Level::High, OutputDrive::Standard);

    // ── UART — 115200 8N1 ─────────────────────────────────────────────────────
    let mut uart_cfg = uarte::Config::default();
    uart_cfg.baudrate = uarte::Baudrate::BAUD115200;

    let uart = uarte::Uarte::new(
        p.UARTE0, Irqs,
        p.P0_08,  // MCU RX ← module T pad
        p.P0_06,  // MCU TX → module R pad
        uart_cfg,
    );
    let (tx, rx) = uart.split();

    // ── 1PPS — GPIOTE rising-edge channel ────────────────────────────────────
    // GPIOTE_CH0 is a peripheral in embassy-nrf; no separate Gpiote init needed.
    let pps_pin     = Input::new(p.P0_14, Pull::None);
    let pps_channel = InputChannel::new(
        p.GPIOTE_CH0,
        pps_pin,
        InputChannelPolarity::LoToHi,
    );

    // ── Launch executor ───────────────────────────────────────────────────────
    let executor = EXECUTOR.init(Executor::new());
    executor.run(|spawner| {
        spawner.spawn(gnss_uart_task(rx, tx)).unwrap();
        spawner.spawn(pps_task(pps_channel)).unwrap();
    });
}

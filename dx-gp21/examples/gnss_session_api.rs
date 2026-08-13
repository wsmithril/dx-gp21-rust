//! **GnssSession API (high-level)** — poll managed [`GnssState`] via a session.
//!
//! A background thread reads and parses NMEA sentences automatically. You access
//! the always-current state through a short-lived lock and call convenience
//! methods like [`has_fix`], [`position`], [`utc_time`], and [`sats_used_count`].
//!
//! Both [`SerialSession`] (live port) and [`FileSession`] (log replay) implement
//! the same [`GnssSession`] trait, so the `run_state_loop` function below works
//! with either without any code changes.
//!
//! Contrast this with the Sentence API (low-level API, see `sentence_api`) where you
//! own the read loop and react to individual [`ParsedSentence`] values directly.
//!
//! [`SerialSession`]: dx_gp21::SerialSession
//! [`FileSession`]: dx_gp21::FileSession
//! [`GnssSession`]: dx_gp21::GnssSession
//! [`ParsedSentence`]: dx_gp21::ParsedSentence
//! [`has_fix`]: dx_gp21::GnssState::has_fix
//! [`position`]: dx_gp21::GnssState::position
//! [`utc_time`]: dx_gp21::GnssState::utc_time
//! [`sats_used_count`]: dx_gp21::GnssState::sats_used_count
//!
//! # Run
//!
//! ```text
//! # Live serial port
//! cargo run --example gnss_session_api -- /dev/tty.usbserial-0001
//!
//! # Replay a captured log file (no hardware needed, commands are no-ops)
//! cargo run --example gnss_session_api -- --file path/to/capture.nmea
//! ```

use std::error::Error;
use std::thread;
use std::time::Duration;

use dx_gp21::{FileSession, GnssSession, GnssStore};

fn main() -> Result<(), Box<dyn Error>> {
    let session = FileSession::open("dx-gp21/examples/sample.nmea", 5)
        .map_err(|e| format!("Cannot open sample.nmea: {e}"))?;
    run_state_loop(session)
}

/// GnssSession API loop: lock → read state → display → sleep.
/// Generic over [`GnssSession`] so it works with both [`SerialSession`]
/// and [`FileSession`] without any changes.
///
/// [`SerialSession`]: dx_gp21::SerialSession
/// [`FileSession`]: dx_gp21::FileSession
fn run_state_loop<S: GnssSession>(session: S) -> Result<(), Box<dyn Error>> {
    println!("Waiting for GNSS data… (Ctrl+C to quit)\n");

    let mut last_fix = false;

    loop {
        // ── Acquire the lock, read state, release immediately ─────────────────
        {
            let state = session.state();

            let fix = state.has_fix();

            // Report transition to/from fix
            if fix != last_fix {
                if fix {
                    println!(">>> Fix acquired: {}", state.fix_mode());
                } else {
                    println!(">>> Fix lost");
                }
                last_fix = fix;
            }

            if fix {
                // Convenience methods — no manual field access needed
                let (lat, lon) = state.position().unwrap();
                let date = state.utc_date().map(|d| d.to_string()).unwrap_or_else(|| "--".into());
                let time = state.utc_time().map(|t| t.to_string()).unwrap_or_else(|| "--:--:--".into());
                let alt  = state.altitude_msl().unwrap_or(0.0);
                let mode = state.fix_mode();
                let used = state.sats_used_count();
                let view = state.sats_in_view_count();
                let dop  = state.dop();
                println!(
                    "{date}  {time} UTC  |  {lat:.6}°  {lon:.6}°  \
                     alt={alt:.1}m  |  {mode}  sats={used}/{view}  \
                     hdop={hdop:.1}  pdop={pdop:.1}",
                    hdop = dop.hdop, pdop = dop.pdop,
                );

                if let Some(speed) = state.speed_kmh() {
                    if speed > 0.5 {
                        println!(
                            "  → moving {:.1} km/h  course {:.0}°",
                            speed,
                            state.course_deg().unwrap_or(0.0),
                        );
                    }
                }
            } else {
                print!("  {mode}  antenna={ant}  sats_visible={view}\r",
                    mode = state.fix_mode(),
                    ant  = state.antenna(),
                    view = state.sats_in_view_count(),
                );
            }
        } // ← lock released here

        thread::sleep(Duration::from_secs(1));
    }
}

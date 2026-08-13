//! **Sentence API (low-level)** — iterate NMEA sentences with [`SentenceReader`].
//!
//! `SentenceReader<R>` is an iterator over [`SentenceLine`] values. Each item
//! carries the raw NMEA text and the parse result. You own the loop and decide
//! what happens to every sentence; the library manages no state for you.
//!
//! Contrast this with the [`GnssSession`] API (high-level API, see `gnss_session_api`)
//! where a background thread handles parsing and you poll a managed state object.
//!
//! [`SentenceLine`]: dx_gp21::sentence_reader::SentenceLine
//! [`GnssSession`]: dx_gp21::GnssSession
//!
//! # Run
//!
//! ```text
//! # Serial port
//! cargo run --example sentence_api -- /dev/tty.usbserial-0001
//!
//! # Replay a captured log file (no hardware needed)
//! cargo run --example sentence_api -- path/to/capture.nmea
//! ```

use std::error::Error;
use std::fs::File;
use std::io::{BufReader, Read};
use std::time::Duration;

use dx_gp21::sentence_reader::SentenceReader;
use dx_gp21::{GnssState, GnssStore, ParsedSentence};

fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args().nth(1)
        .unwrap_or_else(|| "dx-gp21/examples/sample.nmea".into());

    // Open either a serial port or a plain file.
    let source: Box<dyn Read> = if looks_like_serial(&path) {
        Box::new(
            serialport::new(&path, 115200)
                .timeout(Duration::from_millis(200))
                .open()
                .map_err(|e| format!("Cannot open serial port {path}: {e}"))?,
        )
    } else {
        Box::new(
            File::open(&path)
                .map_err(|e| format!("Cannot open file {path}: {e}"))?,
        )
    };

    // ── Low-level Sentence API: wrap any BufRead in SentenceReader ────────────────────

    let reader = SentenceReader::new(BufReader::new(source));

    // We own the state and update it selectively.
    let mut state = GnssState::default();
    let mut total = 0usize;
    let mut errors = 0usize;

    for line in reader {
        total += 1;

        if !line.is_valid() {
            errors += 1;
            // Invalid lines are still available — useful for debugging
            eprintln!("[BAD] {}", line.raw);
            continue;
        }

        match line.parsed.unwrap() {
            // Intercept position sentences for immediate reaction
            ParsedSentence::Gga(gga) if gga.is_valid() => {
                println!(
                    "[GGA] {} | lat={:.6} lon={:.6} alt={:.1}m hdop={:.1} sats={}",
                    gga.time, gga.lat, gga.lon, gga.alt_msl, gga.hdop, gga.sats_used
                );
                state.update(ParsedSentence::Gga(gga));
            }

            // Intercept antenna status warnings
            ParsedSentence::Txt(txt) => {
                println!("[TXT] antenna: {}", txt.antenna_status);
                state.update(ParsedSentence::Txt(txt));
            }

            // Forward everything else into managed state
            other => {
                state.update(other);

                // Use state-layer convenience methods at any point
                if state.has_fix() {
                    let (lat, lon) = state.position().unwrap();
                    let _ = (lat, lon); // available when needed
                }
            }
        }
    }

    println!("\n{total} sentences read, {errors} parse errors");
    println!("Final state: {} | sats {}/{} | dop {:.1}/{:.1}/{:.1}",
        state.fix_mode(),
        state.sats_used_count(), state.sats_in_view_count(),
        state.dop().pdop, state.dop().hdop, state.dop().vdop,
    );
    Ok(())
}

fn looks_like_serial(path: &str) -> bool {
    path.starts_with("/dev/tty")
        || path.starts_with("/dev/serial")
        || path.starts_with("COM")      // Windows
        || path.starts_with("\\\\.\\") // Windows extended path
}

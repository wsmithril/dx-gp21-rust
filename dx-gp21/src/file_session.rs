use std::io::{BufReader, Cursor};
use std::sync::{Arc, Mutex, MutexGuard};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread;
use std::time::Duration;

use dx_gp21_core::{GnssStore, command::{BaudRate, ConstellationMask, UpdateRate}};
use crate::host_state::GnssState;
use crate::sentence_reader::{SentenceLine, SentenceReader};
use crate::serial::SessionError;

const CHANNEL_CAP: usize = 500;

/// Replays a captured NMEA log file in a loop for testing without hardware.
pub struct FileSession {
    state: Arc<Mutex<GnssState>>,
    sentence_rx: Receiver<SentenceLine>,
}

impl FileSession {
    pub fn open(path: impl AsRef<std::path::Path>, line_delay_ms: u64) -> Result<Self, SessionError> {
        // Pre-read all content so we can loop cleanly.
        let content = std::fs::read_to_string(path)?;

        let initial = GnssState {
            baud_rate:   BaudRate::default(),
            update_rate: UpdateRate::default(),
            system_mask: ConstellationMask::ALL,
            ..GnssState::default()
        };

        let state = Arc::new(Mutex::new(initial));
        let state_clone = Arc::clone(&state);

        let (tx, sentence_rx) = mpsc::sync_channel::<SentenceLine>(CHANNEL_CAP);

        thread::Builder::new()
            .name("gnss-file-reader".into())
            .spawn(move || playback_loop(content, state_clone, tx, line_delay_ms))?;

        Ok(Self { state, sentence_rx })
    }

    pub(crate) fn state_inner(&self) -> MutexGuard<'_, GnssState> {
        self.state.lock().expect("state lock poisoned")
    }

    pub(crate) fn drain_sentences_inner(&self, out: &mut Vec<SentenceLine>) {
        while let Ok(line) = self.sentence_rx.try_recv() { out.push(line); }
    }
}

fn playback_loop(
    content: String,
    state: Arc<Mutex<GnssState>>,
    tx: SyncSender<SentenceLine>,
    delay_ms: u64,
) {
    let delay = Duration::from_millis(delay_ms);
    loop {
        for line in SentenceReader::new(BufReader::new(Cursor::new(content.as_bytes()))) {
            if let Ok(sentence) = line.parsed {
                let mut s = state.lock().expect("state lock poisoned");
                s.update(sentence);
            }
            let _ = tx.try_send(line);
            thread::sleep(delay);
        }
    }
}

// ── GnssSession impl ──────────────────────────────────────────────────────────

use crate::session::GnssSession;

impl GnssSession for FileSession {
    fn state(&self) -> MutexGuard<'_, GnssState> { self.state_inner() }
    fn drain_sentences(&self, out: &mut Vec<SentenceLine>) { self.drain_sentences_inner(out); }
    fn send_raw(&self, _bytes: &[u8]) -> Result<(), SessionError> { Ok(()) }
    fn is_readonly(&self) -> bool { true }
}

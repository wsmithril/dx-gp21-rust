use std::io::BufReader;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread;
use std::time::Duration;

use dx_gp21_core::{GnssStore, command::{BaudRate, ConstellationMask, UpdateRate}};
use crate::host_state::GnssState;
use crate::sentence_reader::{SentenceLine, SentenceReader};
use crate::serial::SessionError;

const CHANNEL_CAP: usize = 500;

/// Lines skipped per ←/→ step — roughly 5 seconds at 1 Hz (~12 lines/s).
const SEEK_LINES: i64 = 60;
/// Lines skipped per </> step — roughly 1 NMEA measurement cycle (~1 second).
const CYCLE_LINES: i64 = 12;

/// Replays a captured NMEA log file in a loop for testing without hardware.
pub struct FileSession {
    state: Arc<Mutex<GnssState>>,
    sentence_rx: Receiver<SentenceLine>,
    /// Written by the UI; background thread reads it each line.
    delay_ms: Arc<AtomicU64>,
    /// Seek delta in lines: positive = skip forward, negative = rewind.
    seek_delta: Arc<AtomicI64>,
    /// When true the background thread stops advancing but still processes seeks.
    paused: Arc<AtomicBool>,
    /// Total line count (for clamping seek position).
    total_lines: usize,
}

impl FileSession {
    pub fn open(path: impl AsRef<std::path::Path>, line_delay_ms: u64) -> Result<Self, SessionError> {
        let content = std::fs::read_to_string(path)?;
        let total_lines = content.lines().count();

        let initial = GnssState {
            baud_rate:   BaudRate::default(),
            update_rate: UpdateRate::default(),
            system_mask: ConstellationMask::ALL,
            ..GnssState::default()
        };

        let state      = Arc::new(Mutex::new(initial));
        let delay_ms   = Arc::new(AtomicU64::new(line_delay_ms));
        let seek_delta = Arc::new(AtomicI64::new(0));
        let paused     = Arc::new(AtomicBool::new(false));

        let state_c  = Arc::clone(&state);
        let delay_c  = Arc::clone(&delay_ms);
        let seek_c   = Arc::clone(&seek_delta);
        let paused_c = Arc::clone(&paused);
        let (tx, sentence_rx) = mpsc::sync_channel::<SentenceLine>(CHANNEL_CAP);

        thread::Builder::new()
            .name("gnss-file-reader".into())
            .spawn(move || playback_loop(content, state_c, tx, delay_c, seek_c, paused_c))?;

        Ok(Self { state, sentence_rx, delay_ms, seek_delta, paused, total_lines })
    }

    pub(crate) fn state_inner(&self) -> MutexGuard<'_, GnssState> {
        self.state.lock().expect("state lock poisoned")
    }

    pub(crate) fn drain_sentences_inner(&self, out: &mut Vec<SentenceLine>) {
        while let Ok(line) = self.sentence_rx.try_recv() { out.push(line); }
    }

    pub(crate) fn set_delay_ms_inner(&self, ms: u64) {
        self.delay_ms.store(ms, Ordering::Relaxed);
    }

    pub(crate) fn get_delay_ms_inner(&self) -> u64 {
        self.delay_ms.load(Ordering::Relaxed)
    }

    pub(crate) fn seek_inner(&self, steps: i64) {
        self.seek_delta.fetch_add(steps * SEEK_LINES, Ordering::Relaxed);
    }

    pub(crate) fn step_inner(&self, cycles: i64) {
        self.seek_delta.fetch_add(cycles * CYCLE_LINES, Ordering::Relaxed);
    }

    pub(crate) fn set_paused_inner(&self, p: bool) {
        self.paused.store(p, Ordering::Relaxed);
    }

    pub fn total_lines(&self) -> usize { self.total_lines }
}

fn reset_state(state: &Arc<Mutex<GnssState>>) {
    *state.lock().expect("state lock poisoned") = GnssState {
        baud_rate:   BaudRate::default(),
        update_rate: UpdateRate::default(),
        system_mask: ConstellationMask::ALL,
        ..GnssState::default()
    };
}

fn process_line(raw: &str, state: &Arc<Mutex<GnssState>>, tx: &SyncSender<SentenceLine>) {
    if raw.is_empty() { return; }
    for line in SentenceReader::new(BufReader::new(raw.as_bytes())) {
        if let Ok(sentence) = line.parsed {
            state.lock().expect("state lock poisoned").update(sentence);
        }
        let _ = tx.try_send(line);
    }
}

fn playback_loop(
    content: String,
    state: Arc<Mutex<GnssState>>,
    tx: SyncSender<SentenceLine>,
    delay_ms: Arc<AtomicU64>,
    seek_delta: Arc<AtomicI64>,
    paused: Arc<AtomicBool>,
) {
    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();
    if total == 0 { return; }
    let mut pos: usize = 0;

    loop {
        // Apply any pending seek delta — executed even while paused so that
        // </> step-and-pause jumps to the new position and updates state.
        let delta = seek_delta.swap(0, Ordering::Relaxed);
        if delta != 0 {
            let new_pos = (pos as i64 + delta).max(0) as usize % total;
            pos = new_pos;
            reset_state(&state);
            // Process a window of lines at the new position so the TUI
            // immediately reflects the state at the seek destination.
            let preview = CYCLE_LINES.unsigned_abs() as usize;
            for i in 0..preview {
                process_line(lines[(pos + i) % total], &state, &tx);
            }
        }

        // Pause: wait here (checking for seeks every 50 ms) without advancing.
        if paused.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(50));
            continue;
        }

        // Normal playback: emit one line, advance position.
        process_line(lines[pos], &state, &tx);
        pos = (pos + 1) % total;

        let ms = delay_ms.load(Ordering::Relaxed);
        if ms > 0 { thread::sleep(Duration::from_millis(ms)); }
    }
}

// ── GnssSession impl ──────────────────────────────────────────────────────────

use crate::session::{GnssSession, SeekableSession};

impl GnssSession for FileSession {
    fn state(&self) -> MutexGuard<'_, GnssState> { self.state_inner() }
    fn drain_sentences(&self, out: &mut Vec<SentenceLine>) { self.drain_sentences_inner(out); }
    fn send_raw(&self, _bytes: &[u8]) -> Result<(), SessionError> { Ok(()) }
    fn is_readonly(&self) -> bool { true }
    fn seekable(&self) -> bool { true }
    fn as_seekable(&self) -> Option<&dyn SeekableSession> { Some(self) }
}

impl SeekableSession for FileSession {
    fn set_replay_delay_ms(&self, ms: u64) { self.set_delay_ms_inner(ms); }
    fn get_replay_delay_ms(&self) -> u64 { self.get_delay_ms_inner() }
    fn seek(&self, steps: i64) { self.seek_inner(steps); }
    fn step(&self, cycles: i64) { self.step_inner(cycles); }
    fn set_paused(&self, p: bool) { self.set_paused_inner(p); }
}

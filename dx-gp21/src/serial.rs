use std::io::BufReader;
use std::sync::{Arc, Mutex, MutexGuard};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread;
use std::time::Duration;

use serialport::SerialPort;
use dx_gp21_core::GnssStore;
use crate::host_state::GnssState;
use crate::sentence_reader::{SentenceLine, SentenceReader};

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("serialport: {0}")]
    Port(#[from] serialport::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

const CHANNEL_CAP: usize = 500;

pub struct SerialSession {
    state: Arc<Mutex<GnssState>>,
    writer: Arc<Mutex<Box<dyn SerialPort>>>,
    sentence_rx: Receiver<SentenceLine>,
}

impl SerialSession {
    pub fn open(port_name: &str, baud: u32) -> Result<Self, SessionError> {
        use dx_gp21_core::command::{BaudRate, ConstellationMask, UpdateRate};

        let port = serialport::new(port_name, baud)
            .timeout(Duration::from_millis(100))
            .open()?;
        let writer_port = port.try_clone()?;
        let writer = Arc::new(Mutex::new(writer_port));

        let initial = GnssState {
            baud_rate:   BaudRate::nearest(baud),
            update_rate: UpdateRate::default(),
            system_mask: ConstellationMask::ALL,
            ..GnssState::default()
        };

        let state = Arc::new(Mutex::new(initial));
        let state_clone = Arc::clone(&state);

        let (tx, sentence_rx) = mpsc::sync_channel::<SentenceLine>(CHANNEL_CAP);

        thread::Builder::new()
            .name("gnss-reader".into())
            .spawn(move || reader_loop(port, state_clone, tx))?;

        Ok(Self { state, writer, sentence_rx })
    }

    pub(crate) fn state_inner(&self) -> MutexGuard<'_, GnssState> {
        self.state.lock().expect("state lock poisoned")
    }

    pub(crate) fn drain_sentences_inner(&self, out: &mut Vec<SentenceLine>) {
        while let Ok(line) = self.sentence_rx.try_recv() { out.push(line); }
    }

    pub(crate) fn send_raw_inner(&self, bytes: &[u8]) -> Result<(), SessionError> {
        use std::io::Write;
        let mut w = self.writer.lock().expect("writer lock poisoned");
        w.write_all(bytes)?;
        Ok(())
    }
}

fn reader_loop(
    port: Box<dyn SerialPort>,
    state: Arc<Mutex<GnssState>>,
    tx: SyncSender<SentenceLine>,
) {
    for line in SentenceReader::new(BufReader::new(port)) {
        // Update state from the parsed sentence — parse happened once in SentenceReader.
        if let Ok(sentence) = line.parsed {
            let mut s = state.lock().expect("state lock poisoned");
            s.update(sentence);
        }
        // Send the full SentenceLine (raw + parse result) to the monitor.
        let _ = tx.try_send(line);
    }
}

// ── GnssSession impl ──────────────────────────────────────────────────────────

use crate::session::GnssSession;

impl GnssSession for SerialSession {
    fn state(&self) -> MutexGuard<'_, GnssState> { self.state_inner() }
    fn drain_sentences(&self, out: &mut Vec<SentenceLine>) { self.drain_sentences_inner(out); }
    fn send_raw(&self, bytes: &[u8]) -> Result<(), SessionError> { self.send_raw_inner(bytes) }
}

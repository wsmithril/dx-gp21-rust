use std::sync::MutexGuard;
use crate::sentence_reader::SentenceLine;

use dx_gp21_core::command::{
    BaudRate, ConstellationMask, InfoField, RestartMode, UpdateRate,
    cmd_save_config, cmd_restart, cmd_set_baud, cmd_set_update_rate,
    cmd_set_systems, cmd_query_info, cmd_set_nmea_version,
};

use crate::host_state::GnssState;
use crate::serial::SessionError;

/// Core session interface shared by live serial ports and file replay.
///
/// Required: `state`, `drain_sentences`, `send_raw`.
/// All `$PCAS` command helpers are provided as defaults via `send_raw`.
/// Read-only sessions use a no-op `send_raw`; file replay sessions that
/// support time control additionally implement [`SeekableSession`].
pub trait GnssSession {
    fn state(&self) -> MutexGuard<'_, GnssState>;
    fn drain_sentences(&self, out: &mut Vec<SentenceLine>);
    fn send_raw(&self, bytes: &[u8]) -> Result<(), SessionError>;

    /// `true` for read-only sessions (e.g. file playback).
    fn is_readonly(&self) -> bool { false }

    /// `true` if this session also implements [`SeekableSession`].
    fn seekable(&self) -> bool { false }

    /// Returns a reference to this session as a [`SeekableSession`], or `None`.
    fn as_seekable(&self) -> Option<&dyn SeekableSession> { None }

    // ── $PCAS command helpers (build bytes → send_raw) ────────────────────────

    fn save_config(&self) -> Result<(), SessionError> {
        let mut buf = [0u8; 32]; let n = cmd_save_config(&mut buf); self.send_raw(&buf[..n])
    }
    fn restart(&self, mode: RestartMode) -> Result<(), SessionError> {
        let mut buf = [0u8; 32]; let n = cmd_restart(&mut buf, mode); self.send_raw(&buf[..n])
    }
    fn set_baud(&self, rate: BaudRate) -> Result<(), SessionError> {
        let mut buf = [0u8; 32]; let n = cmd_set_baud(&mut buf, rate); self.send_raw(&buf[..n])
    }
    fn set_update_rate(&self, rate: UpdateRate) -> Result<(), SessionError> {
        let mut buf = [0u8; 32]; let n = cmd_set_update_rate(&mut buf, rate); self.send_raw(&buf[..n])
    }
    fn set_systems(&self, mask: ConstellationMask) -> Result<(), SessionError> {
        let mut buf = [0u8; 32]; let n = cmd_set_systems(&mut buf, mask); self.send_raw(&buf[..n])
    }
    fn query_info(&self, field: InfoField) -> Result<(), SessionError> {
        let mut buf = [0u8; 32]; let n = cmd_query_info(&mut buf, field); self.send_raw(&buf[..n])
    }
    fn set_nmea_version(&self, v41_plus: bool) -> Result<(), SessionError> {
        let mut buf = [0u8; 32]; let n = cmd_set_nmea_version(&mut buf, v41_plus); self.send_raw(&buf[..n])
    }
}

/// Time-control extensions for file replay sessions.
///
/// Implemented by [`crate::file_session::FileSession`]; never by serial sessions.
/// Access via [`GnssSession::as_seekable()`].
pub trait SeekableSession: GnssSession {
    /// Adjust the per-line replay delay (microseconds).
    /// Lower values play faster; 0 means no delay.
    fn set_replay_delay_ms(&self, ms: u64);

    /// Returns the current per-line delay in milliseconds.
    fn get_replay_delay_ms(&self) -> u64;

    /// Seek ±N × ~5 s (coarse navigation: ←/→ keys).
    fn seek(&self, steps: i64);

    /// Step ±N × ~1 s and let the caller pause for inspection (</> keys).
    fn step(&self, cycles: i64);

    /// Pause or resume playback. When paused the background thread stops
    /// advancing but still processes seek deltas, so </> step-and-pause
    /// jumps to the new position and updates state before freezing.
    fn set_paused(&self, paused: bool);
}

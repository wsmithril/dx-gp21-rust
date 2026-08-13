use std::sync::MutexGuard;
use crate::sentence_reader::SentenceLine;

use dx_gp21_core::command::{
    BaudRate, ConstellationMask, InfoField, RestartMode, UpdateRate,
    cmd_save_config, cmd_restart, cmd_set_baud, cmd_set_update_rate,
    cmd_set_systems, cmd_query_info, cmd_set_nmea_version,
};

use crate::host_state::GnssState;
use crate::serial::SessionError;

/// Common interface for both [`crate::serial::SerialSession`] (live port) and
/// [`crate::file_session::FileSession`] (read-only replay).
///
/// **Required methods:** `state`, `drain_log`, `send_raw`.
/// All `$PCAS` command helpers (`save_config`, `restart`, `set_baud`, …) are
/// provided as defaults — each builds the correct byte sequence and calls
/// `send_raw`. A read-only session only needs a no-op `send_raw` to make every
/// command silently do nothing.
pub trait GnssSession {
    fn state(&self) -> MutexGuard<'_, GnssState>;
    /// Drain buffered [`SentenceLine`] items (non-blocking). Each item carries
    /// the raw NMEA string and the parse result so callers can log, filter, or
    /// react at the sentence level without re-parsing.
    fn drain_sentences(&self, out: &mut Vec<SentenceLine>);
    fn send_raw(&self, bytes: &[u8]) -> Result<(), SessionError>;

    /// Returns `true` for read-only sessions (e.g. file playback).
    fn is_readonly(&self) -> bool { false }

    // ── Command helpers — all default to: build bytes, then send_raw ──────────

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

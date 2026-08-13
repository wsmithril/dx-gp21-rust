use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use dx_gp21::{GnssSession, SentenceLine};
use dx_gp21_core::command::RestartMode;

const MAX_LOG: usize = 500;
const MAX_HISTORY: usize = 100;

pub struct CommandVariant {
    pub category: &'static str,
    pub category_label: &'static str,
    pub full_command: &'static str,
    pub description: &'static str,
    /// Shown when this is the only matching variant. Multi-line detail.
    pub detail: &'static str,
}

pub const COMMANDS: &[CommandVariant] = &[
    CommandVariant { category: "PCAS00", category_label: "Save config to flash",
        full_command: "$PCAS00*01",
        description: "Save all settings to flash",
        detail: "Saves current settings to flash memory so they survive power cycles.\n\
                 Persists: baud rate, update rate, NMEA output, satellite systems, protocol.\n\
                 Does NOT save ephemeris data (warm/hot-start data stays in VBAT-backed RAM)." },

    CommandVariant { category: "PCAS01", category_label: "Set baud rate",
        full_command: "$PCAS01,0*1C", description: "4800 bps",
        detail: "Changes UART baud rate to 4800 bps.\n\
                 Effect is immediate — reopen your port at the new rate after sending.\n\
                 Save with $PCAS00 to persist across power cycles." },
    CommandVariant { category: "PCAS01", category_label: "Set baud rate",
        full_command: "$PCAS01,1*1D", description: "9600 bps",
        detail: "Changes UART baud rate to 9600 bps.\n\
                 Effect is immediate — reopen your port at the new rate after sending.\n\
                 Save with $PCAS00 to persist across power cycles." },
    CommandVariant { category: "PCAS01", category_label: "Set baud rate",
        full_command: "$PCAS01,2*1E", description: "19200 bps",
        detail: "Changes UART baud rate to 19200 bps.\n\
                 Effect is immediate — reopen your port at the new rate after sending.\n\
                 Save with $PCAS00 to persist across power cycles." },
    CommandVariant { category: "PCAS01", category_label: "Set baud rate",
        full_command: "$PCAS01,3*1F", description: "38400 bps",
        detail: "Changes UART baud rate to 38400 bps.\n\
                 Effect is immediate — reopen your port at the new rate after sending.\n\
                 Save with $PCAS00 to persist across power cycles." },
    CommandVariant { category: "PCAS01", category_label: "Set baud rate",
        full_command: "$PCAS01,4*18", description: "57600 bps",
        detail: "Changes UART baud rate to 57600 bps.\n\
                 Effect is immediate — reopen your port at the new rate after sending.\n\
                 Save with $PCAS00 to persist across power cycles." },
    CommandVariant { category: "PCAS01", category_label: "Set baud rate",
        full_command: "$PCAS01,5*19", description: "115200 bps  (default)",
        detail: "Changes UART baud rate to 115200 bps (factory default).\n\
                 Effect is immediate — reopen your port at the new rate after sending.\n\
                 Save with $PCAS00 to persist across power cycles." },

    CommandVariant { category: "PCAS02", category_label: "Set positioning update rate",
        full_command: "$PCAS02,1000*2E", description: "1 Hz  (default)",
        detail: "Sets fix output rate to 1 Hz (one position update per second).\n\
                 All NMEA sentences (GGA, RMC, VTG, GSA, GSV, GLL, ZDA) update at 1 Hz." },
    CommandVariant { category: "PCAS02", category_label: "Set positioning update rate",
        full_command: "$PCAS02,500*1A", description: "2 Hz",
        detail: "Sets fix output rate to 2 Hz.\n\
                 GGA, RMC, VTG update at 2 Hz. GSA, GSV, GLL, ZDA are capped at 1 Hz." },
    CommandVariant { category: "PCAS02", category_label: "Set positioning update rate",
        full_command: "$PCAS02,200*1D", description: "5 Hz",
        detail: "Sets fix output rate to 5 Hz.\n\
                 GGA, RMC, VTG update at 5 Hz. GSA, GSV, GLL, ZDA are capped at 1 Hz." },
    CommandVariant { category: "PCAS02", category_label: "Set positioning update rate",
        full_command: "$PCAS02,100*1E", description: "10 Hz",
        detail: "Sets fix output rate to 10 Hz (maximum).\n\
                 GGA, RMC, VTG update at 10 Hz. GSA, GSV, GLL, ZDA are capped at 1 Hz." },

    CommandVariant { category: "PCAS03", category_label: "Configure NMEA output",
        full_command: "$PCAS03,1,1,1,1,1,1,1,1,1,0,,,0,1*02",
        description: "Enable all sentences at 1 Hz",
        detail: "Enables all NMEA sentences at 1 Hz.\n\
                 Field order: GGA, GLL, GSA, GSV, RMC, VTG, ZDA, TXT, DHV, Res×4, GST\n\
                 Value: 0=off  1=every fix  2=every 2nd fix  …  10=every 10th fix\n\
                 Empty field = keep current setting." },
    CommandVariant { category: "PCAS03", category_label: "Configure NMEA output",
        full_command: "$PCAS03,1,0,1,1,1,1,0,0,0,0,,,0,0*02",
        description: "Minimal: GGA + GSA + GSV + RMC + VTG only",
        detail: "Enables only the essential sentences for navigation.\n\
                 Disables: GLL, ZDA, TXT, DHV, GST.\n\
                 Field order: GGA, GLL, GSA, GSV, RMC, VTG, ZDA, TXT, DHV, Res×4, GST" },
    CommandVariant { category: "PCAS03", category_label: "Configure NMEA output",
        full_command: "$PCAS03,0,0,0,0,0,0,0,0,0,0,,,0,0*02",
        description: "Disable all NMEA output",
        detail: "Silences all NMEA sentence output.\n\
                 Useful when switching to a custom polling mode.\n\
                 The module still positions internally; no data is sent until re-enabled." },

    CommandVariant { category: "PCAS04", category_label: "Set satellite systems",
        full_command: "$PCAS04,7F*58",
        description: "ALL: GPS + BDS + GLONASS + Galileo + QZSS  (default)",
        detail: "Enables all five satellite constellations (factory default).\n\
                 Bitmask: GPS=0x01  BDS=0x02  GLO=0x04  GAL=0x08  QZS=0x10\n\
                 Example: GPS+BDS = 0x01|0x02 = 3  →  $PCAS04,3*1A" },
    CommandVariant { category: "PCAS04", category_label: "Set satellite systems",
        full_command: "$PCAS04,1*18", description: "GPS only",
        detail: "Enables GPS L1 C/A only.\n\
                 Use for maximum compatibility or power saving.\n\
                 Restart receiver after changing constellation config." },
    CommandVariant { category: "PCAS04", category_label: "Set satellite systems",
        full_command: "$PCAS04,2*1B", description: "BDS only",
        detail: "Enables BeiDou B1I only." },
    CommandVariant { category: "PCAS04", category_label: "Set satellite systems",
        full_command: "$PCAS04,3*1A", description: "GPS + BDS",
        detail: "Enables GPS and BeiDou." },
    CommandVariant { category: "PCAS04", category_label: "Set satellite systems",
        full_command: "$PCAS04,4*1D", description: "GLONASS only",
        detail: "Enables GLONASS L1 only." },
    CommandVariant { category: "PCAS04", category_label: "Set satellite systems",
        full_command: "$PCAS04,5*1C", description: "GPS + GLONASS",
        detail: "Enables GPS and GLONASS." },
    CommandVariant { category: "PCAS04", category_label: "Set satellite systems",
        full_command: "$PCAS04,6*1F", description: "BDS + GLONASS",
        detail: "Enables BeiDou and GLONASS." },
    CommandVariant { category: "PCAS04", category_label: "Set satellite systems",
        full_command: "$PCAS04,7*1E", description: "GPS + BDS + GLONASS",
        detail: "Enables GPS, BeiDou and GLONASS." },
    CommandVariant { category: "PCAS04", category_label: "Set satellite systems",
        full_command: "$PCAS04,8*11", description: "Galileo only",
        detail: "Enables Galileo E1 only." },
    CommandVariant { category: "PCAS04", category_label: "Set satellite systems",
        full_command: "$PCAS04,9*10", description: "GPS + Galileo",
        detail: "Enables GPS and Galileo." },

    CommandVariant { category: "PCAS05", category_label: "Set NMEA protocol version",
        full_command: "$PCAS05,2*1A", description: "NMEA 4.1+  (default)",
        detail: "Outputs NMEA 4.1+ sentences (factory default).\n\
                 Includes systemId field in GSA, signalId field in GSV.\n\
                 Use this for modern software." },
    CommandVariant { category: "PCAS05", category_label: "Set NMEA protocol version",
        full_command: "$PCAS05,5*1D", description: "BDS/GPS dual-mode  (NMEA 2.3+ / 4.0)",
        detail: "Outputs sentences compatible with the China Ministry of Transport\n\
                 BDS/GPS dual-mode standard.\n\
                 Also compatible with NMEA 2.3+ and NMEA 4.0.\n\
                 Use for legacy devices that don't support NMEA 4.1." },

    CommandVariant { category: "PCAS06", category_label: "Query product information",
        full_command: "$PCAS06,0*1B", description: "Firmware version",
        detail: "Queries firmware version. Module responds with firmware info on UART.\n\
                 Response appears in the log below." },
    CommandVariant { category: "PCAS06", category_label: "Query product information",
        full_command: "$PCAS06,1*1A", description: "Hardware model & serial number",
        detail: "Queries hardware model name and serial number.\n\
                 Response appears in the log below." },
    CommandVariant { category: "PCAS06", category_label: "Query product information",
        full_command: "$PCAS06,2*19", description: "Multi-mode receiver operating mode",
        detail: "Queries the current multi-constellation receiver mode.\n\
                 Response appears in the log below." },
    CommandVariant { category: "PCAS06", category_label: "Query product information",
        full_command: "$PCAS06,3*18", description: "Customer code",
        detail: "Queries the customer/OEM code programmed into the module.\n\
                 Response appears in the log below." },
    CommandVariant { category: "PCAS06", category_label: "Query product information",
        full_command: "$PCAS06,5*1E", description: "Upgrade code",
        detail: "Queries firmware upgrade code information.\n\
                 Response appears in the log below." },

    CommandVariant { category: "PCAS10", category_label: "Restart receiver",
        full_command: "$PCAS10,0*1C", description: "Hot start  — fastest reacquisition",
        detail: "Hot start: all backup data remains valid.\n\
                 The receiver uses saved position, time, almanac and ephemeris.\n\
                 Typical TTFF: ≤1 s. Use after a brief power interruption." },
    CommandVariant { category: "PCAS10", category_label: "Restart receiver",
        full_command: "$PCAS10,1*1D", description: "Warm start — clear ephemeris only",
        detail: "Warm start: clears ephemeris but keeps position, time and almanac.\n\
                 Forces the receiver to re-download satellite ephemeris.\n\
                 Typical TTFF: 20–60 s." },
    CommandVariant { category: "PCAS10", category_label: "Restart receiver",
        full_command: "$PCAS10,2*1E", description: "Cold start — clear all backup data",
        detail: "Cold start: clears all backup data except saved configuration.\n\
                 Receiver must re-acquire everything from scratch.\n\
                 Typical TTFF: ≤30 s under open sky." },
    CommandVariant { category: "PCAS10", category_label: "Restart receiver",
        full_command: "$PCAS10,3*1F", description: "Factory reset — restore all defaults",
        detail: "Factory reset: clears ALL memory including saved configuration.\n\
                 Restores baud rate (115200), update rate (1 Hz), all constellations,\n\
                 and NMEA 4.1+ protocol.\n\
                 Use only when you want to start completely fresh." },
];

pub enum CompletionRow {
    Header { category: &'static str, label: &'static str },
    Variant { idx: usize },
}

/// One entry in the NMEA log: the raw text and whether it parsed successfully.
pub struct LogEntry {
    pub raw: String,
    pub valid: bool,
}

pub struct App<S: GnssSession> {
    pub session: S,
    pub port_name: String,
    pub log: VecDeque<LogEntry>,
    pub log_paused: bool,
    pending_sentences: Vec<SentenceLine>,

    pub cmd_input: String,
    /// The text the user actually typed — fixed while Tab cycles completions.
    cmd_search: String,
    pub cmd_history: VecDeque<String>,
    pub cmd_history_pos: Option<usize>,
    pub completion_idx: Option<usize>,

    /// Most recent $PCAS response lines captured from the module.
    pub response_lines: VecDeque<String>,
    /// Set when a $PCAS06 query is sent; cleared after timeout or enough lines captured.
    response_deadline: Option<Instant>,

    pub sat_scroll: usize,
    pub show_help: bool,
    pub confirm_restart: Option<RestartMode>,
    pub status_msg: Option<String>,
}

impl<S: GnssSession> App<S> {
    pub fn new(session: S, port_name: String) -> Self {
        Self {
            session, port_name,
            log: VecDeque::with_capacity(MAX_LOG),
            log_paused: false,
            pending_sentences: Vec::new(),
            cmd_input: String::new(),
            cmd_search: String::new(),
            cmd_history: VecDeque::with_capacity(MAX_HISTORY),
            cmd_history_pos: None,
            completion_idx: None,
            response_lines: VecDeque::with_capacity(8),
            response_deadline: None,
            sat_scroll: 0,
            show_help: false,
            confirm_restart: None,
            status_msg: None,
        }
    }

    pub fn tick(&mut self) {
        if self.response_deadline.map(|d| Instant::now() > d).unwrap_or(false) {
            self.response_deadline = None;
        }

        self.session.drain_sentences(&mut self.pending_sentences);
        for sl in self.pending_sentences.drain(..) {
            // Capture $PCAS responses within the active window (query replies)
            if self.response_deadline.is_some()
                && (sl.raw.starts_with("$PCAS") || sl.raw.starts_with("$GPTXT"))
            {
                if self.response_lines.len() >= 8 { self.response_lines.pop_front(); }
                self.response_lines.push_back(sl.raw.clone());
            }
            if !self.log_paused {
                if self.log.len() >= MAX_LOG { self.log.pop_front(); }
                let valid = sl.is_valid();
                self.log.push_back(LogEntry { raw: sl.raw, valid });
            }
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        if key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL {
            return true;
        }
        match key.code {
            KeyCode::F(1) => { self.show_help = !self.show_help; return false; }
            KeyCode::F(2) | KeyCode::F(5) => { self.log_paused = !self.log_paused; return false; }
            KeyCode::F(3) if !self.session.is_readonly() => { let _ = self.session.save_config(); return false; }
            KeyCode::F(4) if !self.session.is_readonly() => {
                self.confirm_restart = Some(RestartMode::Cold); return false;
            }
            KeyCode::F(6) => { self.log.clear(); return false; }
            KeyCode::Esc => {
                if self.confirm_restart.is_some() { self.confirm_restart = None; }
                else if self.show_help { self.show_help = false; }
                else { self.clear_input(); }
                return false;
            }
            KeyCode::PageUp   => { self.sat_scroll = self.sat_scroll.saturating_sub(5); return false; }
            KeyCode::PageDown => { self.sat_scroll += 5; return false; }
            _ => {}
        }
        if let Some(mode) = self.confirm_restart {
            if matches!(key.code, KeyCode::Char('y') | KeyCode::Char('Y')) {
                let _ = self.session.restart(mode);
                self.confirm_restart = None;
                self.status_msg = Some("Restart command sent".into());
            } else {
                self.confirm_restart = None;
            }
            return false;
        }
        if self.session.is_readonly() { return false; }
        match key.code {
            KeyCode::Enter   => self.submit_command(),
            KeyCode::Tab     => self.cycle_completion(1),
            KeyCode::BackTab => self.cycle_completion(-1),
            KeyCode::Char(c) => {
                self.cmd_input.push(c);
                self.cmd_search.push(c);
                self.completion_idx = None;
            }
            KeyCode::Backspace => {
                self.cmd_input.pop();
                self.cmd_search = self.cmd_input.clone();
                self.completion_idx = None;
            }
            KeyCode::Up   => self.history_navigate(-1),
            KeyCode::Down => self.history_navigate(1),
            _ => {}
        }
        false
    }

    fn clear_input(&mut self) {
        self.cmd_input.clear();
        self.cmd_search.clear();
        self.completion_idx = None;
    }

    /// True when the user has typed something and the popup should be shown.
    pub fn should_show_completions(&self) -> bool {
        !self.cmd_search.is_empty()
    }

    pub fn matching_indices(&self) -> Vec<usize> {
        let q = self.cmd_search.to_ascii_uppercase();
        COMMANDS.iter().enumerate()
            .filter(|(_, c)| {
                if q.is_empty() { return true; }
                c.full_command.to_ascii_uppercase().contains(&q)
                    || c.category.to_ascii_uppercase().contains(&q)
                    || c.description.to_ascii_uppercase().contains(&q)
                    || c.category_label.to_ascii_uppercase().contains(&q)
            })
            .map(|(i, _)| i)
            .collect()
    }

    pub fn completion_rows(&self) -> Vec<CompletionRow> {
        let indices = self.matching_indices();
        let mut rows = Vec::new();
        let mut last_cat = "";
        for idx in indices {
            let cmd = &COMMANDS[idx];
            if cmd.category != last_cat {
                rows.push(CompletionRow::Header { category: cmd.category, label: cmd.category_label });
                last_cat = cmd.category;
            }
            rows.push(CompletionRow::Variant { idx });
        }
        rows
    }

    fn cycle_completion(&mut self, delta: i32) {
        let indices = self.matching_indices();
        if indices.is_empty() { return; }
        let new_pos = match self.completion_idx {
            None => if delta > 0 { 0 } else { indices.len() - 1 },
            Some(cur) => {
                let pos = indices.iter().position(|&i| i == cur).unwrap_or(0);
                (pos as i32 + delta).rem_euclid(indices.len() as i32) as usize
            }
        };
        let selected = indices[new_pos];
        self.completion_idx = Some(selected);
        self.cmd_input = COMMANDS[selected].full_command.to_string();
    }

    fn history_navigate(&mut self, delta: i32) {
        if self.cmd_history.is_empty() { return; }
        let new_pos = match self.cmd_history_pos {
            None if delta < 0 => Some(self.cmd_history.len() - 1),
            Some(p) => {
                let next = p as i32 + delta;
                if next < 0 || next as usize >= self.cmd_history.len() { None }
                else { Some(next as usize) }
            }
            _ => None,
        };
        self.cmd_history_pos = new_pos;
        let text = new_pos.map(|p| self.cmd_history[p].clone()).unwrap_or_default();
        self.cmd_input = text.clone();
        self.cmd_search = text;
        self.completion_idx = None;
    }

    fn submit_command(&mut self) {
        let raw = self.cmd_input.trim().to_string();
        if raw.is_empty() { return; }
        if self.cmd_history.front().map(|s| s != &raw).unwrap_or(true) {
            if self.cmd_history.len() >= MAX_HISTORY { self.cmd_history.pop_back(); }
            self.cmd_history.push_front(raw.clone());
        }
        self.cmd_history_pos = None;
        self.clear_input();
        // Only capture a response for $PCAS06 queries (the only command that replies)
        if raw.to_ascii_uppercase().starts_with("$PCAS06") {
            self.response_lines.clear();
            self.response_deadline = Some(Instant::now() + Duration::from_secs(2));
        }
        let mut bytes = raw.into_bytes();
        if !bytes.ends_with(b"\n") { bytes.extend_from_slice(b"\r\n"); }
        let _ = self.session.send_raw(&bytes);
    }
}

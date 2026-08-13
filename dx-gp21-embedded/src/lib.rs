#![no_std]

use dx_gp21_core::state::GnssStore;
use dx_gp21_core::types::*;
use dx_gp21_core::nmea::{GgaData, RmcData, GsaData, GsvData, VtgData, ZdaData, DhvData, GstData};
use dx_gp21_core::ParsedSentence;
use heapless::Vec as HVec;

pub use dx_gp21_core::command::{self, CommandSink};
pub use dx_gp21_core::state::feed_sentence;

// ── GnssState ─────────────────────────────────────────────────────────

/// Fixed-size GPS state store for no_std / no-heap targets.
/// `MAX_SATS` caps the total satellite list (64 covers all five constellations).
pub struct GnssState<const MAX_SATS: usize = 64> {
    gga: Option<GgaData>,
    rmc: Option<RmcData>,
    vtg: Option<VtgData>,
    zda: Option<ZdaData>,
    gst: Option<GstData>,
    dhv: Option<DhvData>,
    fix_mode: FixMode,
    dop: DopValues,
    antenna: AntennaStatus,
    satellites: HVec<SatInfo, MAX_SATS>,
}

impl<const N: usize> Default for GnssState<N> {
    fn default() -> Self { Self::new() }
}

impl<const N: usize> GnssState<N> {
    /// Const constructor so the state can be placed in a `static`.
    pub const fn new() -> Self {
        Self {
            gga: None, rmc: None, vtg: None, zda: None,
            gst: None, dhv: None,
            fix_mode: FixMode::NoFix,
            dop: DopValues { pdop: 0.0, hdop: 0.0, vdop: 0.0 },
            antenna: AntennaStatus::Unknown,
            satellites: HVec::new(),
        }
    }
}

impl<const N: usize> GnssStore for GnssState<N> {
    fn update_gga(&mut self, d: GgaData) { self.gga = Some(d); }
    fn update_rmc(&mut self, d: RmcData) { self.rmc = Some(d); }
    fn update_vtg(&mut self, d: VtgData) { self.vtg = Some(d); }
    fn update_zda(&mut self, d: ZdaData) { self.zda = Some(d); }
    fn update_gst(&mut self, d: GstData) { self.gst = Some(d); }
    fn update_dhv(&mut self, d: DhvData) { self.dhv = Some(d); }
    fn update_antenna(&mut self, status: AntennaStatus) { self.antenna = status; }

    fn update_gsa(&mut self, d: GsaData) {
        self.fix_mode = d.fix_mode;
        self.dop = DopValues::from(d);
        for sat in self.satellites.iter_mut() {
            if sat.system == d.system && d.svids.contains(&Some(sat.svid)) { sat.used = true; }
        }
    }

    fn update_gsv(&mut self, d: GsvData) {
        if d.total_in_view == 0 { return; }
        if d.msg_num == 1 { self.satellites.retain(|s| s.system != d.system); }
        for sat in d.sats.iter().flatten() { let _ = self.satellites.push(*sat); }
    }

    fn gga(&self) -> Option<&GgaData> { self.gga.as_ref() }
    fn rmc(&self) -> Option<&RmcData> { self.rmc.as_ref() }
    fn vtg(&self) -> Option<&VtgData> { self.vtg.as_ref() }
    fn zda(&self) -> Option<&ZdaData> { self.zda.as_ref() }
    fn gst(&self) -> Option<&GstData> { self.gst.as_ref() }
    fn dhv(&self) -> Option<&DhvData> { self.dhv.as_ref() }
    fn fix_mode(&self) -> FixMode { self.fix_mode }
    fn dop(&self) -> DopValues { self.dop }
    fn antenna(&self) -> AntennaStatus { self.antenna }
    fn satellites(&self) -> &[SatInfo] { &self.satellites }

    fn sats_used_count(&self) -> u8 {
        self.gga.map(|g| g.sats_used)
            .unwrap_or_else(|| self.satellites.iter().filter(|s| s.used).count() as u8)
    }
    fn sats_in_view_count(&self) -> u8 {
        self.gga.map(|g| g.sats_used).unwrap_or(self.satellites.len() as u8)
    }
}

// ── EmbeddedSession ───────────────────────────────────────────────────────────

/// Combines an [`GnssState`] with a byte-sink writer into a single
/// driver object. Implements [`CommandSink`] so all `$PCAS` command helpers
/// are available without any extra boilerplate.
///
/// `W` is any `FnMut(&[u8])` — typically a closure that writes to a UART.
///
/// ```ignore
/// let mut session = EmbeddedSession::new(|bytes| uart.write_all(bytes));
/// session.feed(b"$GNGGA,...*XX\r\n");
/// session.set_update_rate(command::UpdateRate::Hz5).ok();
/// let fix = session.state().fix_mode();
/// ```
pub struct EmbeddedSession<W: FnMut(&[u8]), const MAX_SATS: usize = 64> {
    state: GnssState<MAX_SATS>,
    writer: W,
}

impl<W: FnMut(&[u8]), const N: usize> EmbeddedSession<W, N> {
    pub fn new(writer: W) -> Self {
        Self { state: GnssState::new(), writer }
    }

    /// Parse one raw NMEA line, update state, and return the full parsed sentence.
    /// The sentence is `Copy`, so both the state and caller receive the data.
    pub fn feed(&mut self, line: &[u8]) -> Option<ParsedSentence> {
        feed_sentence(&mut self.state, line)
    }

    /// Read-only access to the current GPS state.
    pub fn state(&self) -> &GnssState<N> { &self.state }

    /// Mutable access to the current GPS state (rarely needed directly).
    pub fn state_mut(&mut self) -> &mut GnssState<N> { &mut self.state }
}

/// `CommandSink` impl: `send_raw` calls the writer closure;
/// all command helpers (`save_config`, `restart`, `set_baud`, …) come for free
/// from the trait's default implementations.
impl<W: FnMut(&[u8]), const N: usize> CommandSink for EmbeddedSession<W, N> {
    type Error = ();

    fn send_raw(&mut self, bytes: &[u8]) -> Result<(), ()> {
        (self.writer)(bytes);
        Ok(())
    }
}

// ── Async run loop (feature = "async") ───────────────────────────────────────

// ── Async run loop (feature = "async") ───────────────────────────────────────
//
// Core declares AsyncLineReader (the "what").
// EmbeddedIoReader (below) implements it using embedded_io_async (the "how").
// EmbeddedSession::run ties them together.

#[cfg(feature = "async")]
use dx_gp21_core::AsyncLineReader; // bring trait into scope for .next_line() calls

#[cfg(feature = "async")]
impl<W: FnMut(&[u8]), const N: usize> EmbeddedSession<W, N> {
    /// Async main loop. Accepts any [`embedded_io_async::Read`] source, wraps it
    /// in the embedded [`AsyncLineReader`] impl, and calls
    /// [`dx_gp21_core::run_with_reader`]. Never returns.
    ///
    /// Yields once per DMA burst (not per byte).
    pub async fn run<R>(&mut self, rx: R) -> !
    where
        R: embedded_io_async::Read,
    {
        let mut reader = EmbeddedIoReader(rx);
        dx_gp21_core::run_with_reader(&mut reader, &mut self.state).await
    }

    /// Read and process the next valid NMEA sentence from `rx`, update state.
    pub async fn next_sentence<R>(&mut self, rx: &mut R) -> dx_gp21_core::ParsedSentence
    where
        R: embedded_io_async::Read,
    {
        let mut buf = [0u8; 128];
        let mut reader = EmbeddedIoReader(rx);
        loop {
            if let Ok(n) = reader.next_line(&mut buf).await {
                if n > 0 {
                    if let Some(s) = dx_gp21_core::feed_sentence(&mut self.state, &buf[..n]) {
                        return s;
                    }
                }
            }
        }
    }
}

// ── embedded_io_async → AsyncLineReader bridge ────────────────────────────────

/// Newtype that adapts any [`embedded_io_async::Read`] into
/// [`dx_gp21_core::AsyncLineReader`].
///
/// This is the embedded platform's impl of the core trait:
/// - core defines the interface (`AsyncLineReader`)
/// - embedded provides the IO (DMA burst accumulation via `embedded_io_async`)
#[cfg(feature = "async")]
struct EmbeddedIoReader<R>(R);

#[cfg(feature = "async")]
impl<R: embedded_io_async::Read> dx_gp21_core::AsyncLineReader for EmbeddedIoReader<R> {
    type Error = R::Error;

    async fn next_line(&mut self, buf: &mut [u8]) -> Result<usize, R::Error> {
        let mut dma = [0u8; 64];
        let mut n = 0usize;
        loop {
            let got = match self.0.read(&mut dma).await {
                Ok(0) | Err(_) => return Ok(0),
                Ok(g) => g,
            };
            for &b in &dma[..got] {
                match b {
                    b'\n' => return Ok(n),
                    b'\r' => {}
                    b if n < buf.len() => { buf[n] = b; n += 1; }
                    _ => { n = 0; }
                }
            }
        }
    }
}

use crate::types::*;
use crate::nmea::{
    ParsedSentence, GgaData, RmcData, GsaData, GsvData,
    VtgData, ZdaData, GstData, DhvData,
};

/// Trait implemented by both GnssStore (heapless) and GnssStore (Vec).
pub trait GnssStore {
    fn update_gga(&mut self, d: GgaData);
    fn update_rmc(&mut self, d: RmcData);
    fn update_gsa(&mut self, d: GsaData);
    fn update_gsv(&mut self, d: GsvData);
    fn update_vtg(&mut self, d: VtgData);
    fn update_zda(&mut self, d: ZdaData);
    fn update_gst(&mut self, d: GstData);
    fn update_dhv(&mut self, d: DhvData);
    fn update_antenna(&mut self, status: AntennaStatus);

    fn gga(&self) -> Option<&GgaData>;
    fn rmc(&self) -> Option<&RmcData>;
    fn vtg(&self) -> Option<&VtgData>;
    fn zda(&self) -> Option<&ZdaData>;
    fn gst(&self) -> Option<&GstData>;
    fn dhv(&self) -> Option<&DhvData>;
    fn fix_mode(&self) -> FixMode;
    fn dop(&self) -> DopValues;
    fn antenna(&self) -> AntennaStatus;
    fn satellites(&self) -> &[SatInfo];
    fn sats_used_count(&self) -> u8;
    fn sats_in_view_count(&self) -> u8;

    // ── Convenience defaults ──────────────────────────────────────────────────

    /// Returns `true` when a 2D or 3D fix is available.
    fn has_fix(&self) -> bool { self.fix_mode() != FixMode::NoFix }

    /// Returns `(latitude_deg, longitude_deg)` from the latest valid GGA sentence.
    fn position(&self) -> Option<(f64, f64)> {
        self.gga().filter(|g| g.is_valid()).map(|g| (g.lat, g.lon))
    }

    /// Returns altitude above mean sea level from the latest valid GGA sentence.
    fn altitude_msl(&self) -> Option<f32> {
        self.gga().filter(|g| g.is_valid()).map(|g| g.alt_msl)
    }

    /// Returns ground speed in km/h from the latest VTG sentence.
    fn speed_kmh(&self) -> Option<f32> { self.vtg().map(|v| v.speed_kmh) }

    /// Returns true course over ground in degrees from the latest VTG sentence.
    fn course_deg(&self) -> Option<f32> { self.vtg().map(|v| v.course_true) }

    /// Returns the most recent UTC time, checking GGA → RMC → ZDA in priority order.
    fn utc_time(&self) -> Option<NmeaTime> {
        self.gga().map(|g| g.time)
            .or_else(|| self.rmc().map(|r| r.time))
            .or_else(|| self.zda().map(|z| z.time))
    }

    /// Returns the most recent UTC date, checking RMC → ZDA in priority order.
    fn utc_date(&self) -> Option<NmeaDate> {
        self.rmc().map(|r| r.date)
            .or_else(|| self.zda().map(|z| z.date))
    }

    /// Dispatch a pre-parsed sentence into this state.
    /// Equivalent to calling the individual `update_*` methods.
    fn update(&mut self, sentence: ParsedSentence) {
        match sentence {
            ParsedSentence::Gga(d) => self.update_gga(d),
            ParsedSentence::Rmc(d) => self.update_rmc(d),
            ParsedSentence::Gsa(d) => self.update_gsa(d),
            ParsedSentence::Gsv(d) => self.update_gsv(d),
            ParsedSentence::Vtg(d) => self.update_vtg(d),
            ParsedSentence::Zda(d) => self.update_zda(d),
            ParsedSentence::Gst(d) => self.update_gst(d),
            ParsedSentence::Dhv(d) => self.update_dhv(d),
            ParsedSentence::Txt(d) => self.update_antenna(d.antenna_status),
        }
    }
}

/// Lightweight discriminant returned when only the sentence type is needed.
/// Use [`ParsedSentence::kind()`] to obtain one, or match directly on [`ParsedSentence`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SentenceType {
    Gga, Rmc, Gsa, Gsv, Vtg, Zda, Gst, Dhv, Txt,
}

#[cfg(feature = "defmt")]
impl defmt::Format for SentenceType {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(f, "{}", match self {
            Self::Gga => "GGA", Self::Rmc => "RMC", Self::Gsa => "GSA",
            Self::Gsv => "GSV", Self::Vtg => "VTG", Self::Zda => "ZDA",
            Self::Gst => "GST", Self::Dhv => "DHV", Self::Txt => "TXT",
        })
    }
}

impl core::fmt::Display for SentenceType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Gga => "GGA", Self::Rmc => "RMC", Self::Gsa => "GSA",
            Self::Gsv => "GSV", Self::Vtg => "VTG", Self::Zda => "ZDA",
            Self::Gst => "GST", Self::Dhv => "DHV", Self::Txt => "TXT",
        })
    }
}

/// Parse one raw NMEA line, update `state`, and return the full parsed sentence.
///
/// Because [`ParsedSentence`] is `Copy`, the data is dispatched to the state
/// **and** returned to the caller — both can use it without any allocation.
///
/// ```ignore
/// use dx_gp21_core::{feed_sentence, ParsedSentence};
/// if let Some(ParsedSentence::Gga(gga)) = feed_sentence(&mut state, line) {
///     // custom handling in addition to what the state already stored
/// }
/// ```
pub fn feed_sentence<S: GnssStore>(state: &mut S, line: &[u8]) -> Option<ParsedSentence> {
    let sentence = crate::nmea::parse_sentence(line)?;
    state.update(sentence); // Copy dispatched to state; original returned to caller.
    Some(sentence)
}

// ── Async capability (feature = "async") ─────────────────────────────────────
//
// Core declares WHAT the async interface looks like.
// Platforms (embedded HAL, tokio, etc.) provide HOW lines are actually read.

/// Async source of NMEA sentence lines.
///
/// Core defines this trait; each platform fills in the implementation:
/// - **Embedded**: wrap an `embedded_io_async::Read` UART (see `dx-gp21-embedded`)
/// - **Std/async**: wrap a `tokio::io::AsyncBufRead` (see `dx-gp21`)
/// - **Custom**: implement directly for any async byte source
///
/// The trait is deliberately narrow — it says nothing about HOW bytes arrive
/// (DMA, interrupt-driven, async file I/O). That is the platform's concern.
#[cfg(feature = "async")]
pub trait AsyncLineReader {
    type Error;

    /// Asynchronously read the next complete NMEA sentence line into `buf`,
    /// without the trailing `\r\n`. Returns the number of bytes written.
    ///
    /// Returns `Ok(0)` on timeout or when no data is available.
    async fn next_line(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error>;
}

/// Drive a [`GnssStore`] from any [`AsyncLineReader`], updating state on every
/// received sentence. Never returns.
///
/// ```no_run
/// use dx_gp21_core::state::{run_with_reader, AsyncLineReader};
///
/// // reader: impl AsyncLineReader (provided by your platform layer)
/// // state: impl GnssStore
/// run_with_reader(&mut reader, &mut state).await;
/// ```
#[cfg(feature = "async")]
pub async fn run_with_reader<R, S>(reader: &mut R, state: &mut S) -> !
where
    R: AsyncLineReader,
    S: GnssStore,
{
    let mut buf = [0u8; 128];
    loop {
        match reader.next_line(&mut buf).await {
            Ok(n) if n > 0 => { feed_sentence(state, &buf[..n]); }
            _ => {}
        }
    }
}

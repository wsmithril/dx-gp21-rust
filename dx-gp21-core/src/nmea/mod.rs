mod gga;
mod gsa;
mod gsv;
mod rmc;
mod vtg;
mod zda;
mod dhv;
mod gst;
mod txt;

// Re-export each NMEA sentence type from its own file.
pub use gga::GgaData;
pub use gsa::GsaData;
pub use gsv::GsvData;
pub use rmc::RmcData;
pub use vtg::VtgData;
pub use zda::ZdaData;
pub use dhv::DhvData;
pub use gst::GstData;
pub use txt::TxtData;

use crate::types::*;
use crate::checksum;

// ── ParseError ────────────────────────────────────────────────────────────────

/// Returned when a raw line cannot be parsed into a [`ParsedSentence`].
///
/// Causes: checksum mismatch, malformed fields, or unknown sentence type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParseError;

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("NMEA parse error (checksum, malformed fields, or unknown type)")
    }
}

// ── ParsedSentence ────────────────────────────────────────────────────────────

/// A fully parsed NMEA sentence with its payload.
///
/// All payload types are `Copy` so this enum is `Copy` too — it can be
/// dispatched to a [`crate::state::GnssState`] **and** returned to the caller
/// for additional handling without any cloning.
///
/// # Entry points
///
/// ```no_run
/// use dx_gp21_core::nmea::{ParsedSentence, ParseError};
/// use core::convert::TryFrom;
///
/// let line: &[u8] = b"$GNGGA,...*6E\r\n";
/// match ParsedSentence::try_from(line) {
///     Ok(ParsedSentence::Gga(gga)) => { /* use gga.lat, gga.lon … */ }
///     Ok(other) => {}
///     Err(ParseError) => {}
/// }
/// ```
#[derive(Clone, Copy, Debug)]
pub enum ParsedSentence {
    Gga(GgaData),
    Rmc(RmcData),
    Gsa(GsaData),
    Gsv(GsvData),
    Vtg(VtgData),
    Zda(ZdaData),
    Gst(GstData),
    Dhv(DhvData),
    Txt(TxtData),
}

impl ParsedSentence {
    /// Return the lightweight discriminant without the payload.
    pub fn kind(self) -> crate::state::SentenceType {
        use crate::state::SentenceType;
        match self {
            Self::Gga(_) => SentenceType::Gga, Self::Rmc(_) => SentenceType::Rmc,
            Self::Gsa(_) => SentenceType::Gsa, Self::Gsv(_) => SentenceType::Gsv,
            Self::Vtg(_) => SentenceType::Vtg, Self::Zda(_) => SentenceType::Zda,
            Self::Gst(_) => SentenceType::Gst, Self::Dhv(_) => SentenceType::Dhv,
            Self::Txt(_) => SentenceType::Txt,
        }
    }

    /// Returns `true` for sentences that carry a position fix (GGA, RMC).
    pub fn has_position(&self) -> bool { matches!(self, Self::Gga(_) | Self::Rmc(_)) }

    pub fn as_gga(&self) -> Option<&GgaData> { if let Self::Gga(d) = self { Some(d) } else { None } }
    pub fn as_rmc(&self) -> Option<&RmcData> { if let Self::Rmc(d) = self { Some(d) } else { None } }
    pub fn as_gsa(&self) -> Option<&GsaData> { if let Self::Gsa(d) = self { Some(d) } else { None } }
    pub fn as_gsv(&self) -> Option<&GsvData> { if let Self::Gsv(d) = self { Some(d) } else { None } }
    pub fn as_vtg(&self) -> Option<&VtgData> { if let Self::Vtg(d) = self { Some(d) } else { None } }
    pub fn as_zda(&self) -> Option<&ZdaData> { if let Self::Zda(d) = self { Some(d) } else { None } }
    pub fn as_gst(&self) -> Option<&GstData> { if let Self::Gst(d) = self { Some(d) } else { None } }
    pub fn as_dhv(&self) -> Option<&DhvData> { if let Self::Dhv(d) = self { Some(d) } else { None } }
}

// TryFrom<&[u8]> — primary entry point (raw UART bytes, log file lines).
impl<'a> core::convert::TryFrom<&'a [u8]> for ParsedSentence {
    type Error = ParseError;
    fn try_from(line: &'a [u8]) -> Result<Self, ParseError> {
        parse_sentence(line).ok_or(ParseError)
    }
}

// TryFrom<&str> — convenience entry point.
impl<'a> core::convert::TryFrom<&'a str> for ParsedSentence {
    type Error = ParseError;
    fn try_from(line: &'a str) -> Result<Self, ParseError> {
        parse_sentence(line.as_bytes()).ok_or(ParseError)
    }
}

// ── Internal parser ───────────────────────────────────────────────────────────

fn split_fields<'a>(body: &'a str, out: &mut [&'a str]) -> usize {
    let mut n = 0;
    for f in body.split(',') {
        if n >= out.len() { break; }
        out[n] = f;
        n += 1;
    }
    n
}

/// Parse a raw NMEA line. Prefer [`ParsedSentence::try_from`] for a `Result`-based API.
pub fn parse_sentence(line: &[u8]) -> Option<ParsedSentence> {
    let body = checksum::verify(line)?;
    let body_str = core::str::from_utf8(body).ok()?;
    let mut fields = [""; 24];
    let nf = split_fields(body_str, &mut fields);
    if nf < 1 { return None; }
    let sentence_id = fields[0];
    if sentence_id.len() < 3 { return None; }
    let (talker, stype) = if sentence_id.starts_with('P') {
        ("", sentence_id)
    } else if sentence_id.len() >= 5 {
        (&sentence_id[..2], &sentence_id[2..])
    } else {
        return None;
    };
    let system = GnssSystem::from_talker(talker);
    let f = &fields[1..nf];
    match stype {
        "GGA" => gga::parse(system, f).map(ParsedSentence::Gga),
        "RMC" => rmc::parse(system, f).map(ParsedSentence::Rmc),
        "GSA" => gsa::parse(system, f).map(ParsedSentence::Gsa),
        "GSV" => gsv::parse(system, f).map(ParsedSentence::Gsv),
        "VTG" => vtg::parse(f).map(ParsedSentence::Vtg),
        "ZDA" => zda::parse(f).map(ParsedSentence::Zda),
        "GST" => gst::parse(f).map(ParsedSentence::Gst),
        "DHV" => dhv::parse(f).map(ParsedSentence::Dhv),
        "TXT" => txt::parse(f).map(ParsedSentence::Txt),
        _ => None,
    }
}

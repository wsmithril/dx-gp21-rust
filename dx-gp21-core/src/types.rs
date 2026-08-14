#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct NmeaTime {
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub millis: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct NmeaDate {
    pub day: u8,
    pub month: u8,
    pub year: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GnssSystem {
    Gps,
    Beidou,
    Glonass,
    Galileo,
    Qzss,
    #[default]
    Multi,
}

impl GnssSystem {
    pub fn from_talker(talker: &str) -> Self {
        match talker {
            "GP" => Self::Gps,
            "GB" | "BD" => Self::Beidou,
            "GL" => Self::Glonass,
            "GA" => Self::Galileo,
            "GQ" => Self::Qzss,
            _ => Self::Multi,
        }
    }

    /// Short uppercase label suitable for narrow display columns.
    pub fn label(self) -> &'static str {
        match self {
            Self::Gps => "GPS",
            Self::Beidou => "BDS",
            Self::Glonass => "GLO",
            Self::Galileo => "GAL",
            Self::Qzss => "QZS",
            Self::Multi => "MUL",
        }
    }
}

impl core::fmt::Display for GnssSystem {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.label())
    }
}

impl core::fmt::Display for NmeaTime {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:02}:{:02}:{:02}", self.hour, self.minute, self.second)
    }
}

impl core::fmt::Display for NmeaDate {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

impl core::fmt::Display for FixQuality {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Invalid => "invalid",
            Self::Sps => "SPS fix",
            Self::Estimated => "estimated (DR)",
        })
    }
}

impl core::fmt::Display for FixMode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::NoFix => "no fix",
            Self::Fix2D => "2D fix",
            Self::Fix3D => "3D fix",
        })
    }
}

impl core::fmt::Display for AntennaStatus {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Unknown => "unknown",
            Self::Ok => "OK",
            Self::Open => "open",
            Self::Short => "short",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FixQuality {
    #[default]
    Invalid,
    Sps,
    Estimated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FixMode {
    #[default]
    NoFix,
    Fix2D,
    Fix3D,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AntennaStatus {
    #[default]
    Unknown,
    Ok,
    Open,
    Short,
}

#[derive(Clone, Copy, Debug)]
pub struct SatInfo {
    pub svid: u16,
    pub system: GnssSystem,
    pub elevation: Option<i8>,
    pub azimuth: Option<u16>,
    pub snr: Option<u8>,
    pub used: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DopValues {
    pub pdop: f32,
    pub hdop: f32,
    pub vdop: f32,
}

// ── defmt::Format implementations (feature = "defmt") ────────────────────────

#[cfg(feature = "defmt")]
impl defmt::Format for NmeaTime {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(f, "{:02}:{:02}:{:02}", self.hour, self.minute, self.second)
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for NmeaDate {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for GnssSystem {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(f, "{}", self.label())
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for FixMode {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(
            f,
            "{}",
            match self {
                Self::NoFix => "no fix",
                Self::Fix2D => "2D fix",
                Self::Fix3D => "3D fix",
            }
        )
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for FixQuality {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(
            f,
            "{}",
            match self {
                Self::Invalid => "invalid",
                Self::Sps => "SPS",
                Self::Estimated => "DR",
            }
        )
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for AntennaStatus {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(
            f,
            "{}",
            match self {
                Self::Unknown => "?",
                Self::Ok => "OK",
                Self::Open => "open",
                Self::Short => "short",
            }
        )
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for DopValues {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(
            f,
            "pdop={=f32} hdop={=f32} vdop={=f32}",
            self.pdop,
            self.hdop,
            self.vdop
        )
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for SatInfo {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(
            f,
            "{} svid={} el={} az={} snr={} used={}",
            self.system,
            self.svid,
            self.elevation.unwrap_or(-1),
            self.azimuth.unwrap_or(0),
            self.snr.unwrap_or(0),
            self.used,
        )
    }
}

#[allow(dead_code)]
pub(crate) fn parse_f32(s: &str) -> Option<f32> {
    if s.is_empty() {
        return None;
    }
    s.parse().ok()
}

#[allow(dead_code)]
pub(crate) fn parse_f64(s: &str) -> Option<f64> {
    if s.is_empty() {
        return None;
    }
    s.parse().ok()
}

#[allow(dead_code)]
pub(crate) fn parse_u8(s: &str) -> Option<u8> {
    if s.is_empty() {
        return None;
    }
    s.parse().ok()
}

#[allow(dead_code)]
pub(crate) fn parse_u16(s: &str) -> Option<u16> {
    if s.is_empty() {
        return None;
    }
    s.parse().ok()
}

/// Parse an NMEA lat/lon field pair `(value, hemisphere)` into decimal degrees.
pub(crate) fn parse_latlon(val: &str, hemi: &str) -> Option<f64> {
    if val.is_empty() {
        return None;
    }
    let dot = val.find('.')?;
    if dot < 2 {
        return None;
    }
    let deg: f64 = val[..dot - 2].parse().ok()?;
    let min: f64 = val[dot - 2..].parse().ok()?;
    let coord = deg + min / 60.0;
    if hemi == "S" || hemi == "W" {
        Some(-coord)
    } else {
        Some(coord)
    }
}

impl core::convert::TryFrom<&str> for NmeaTime {
    type Error = ();
    /// Parse an NMEA time string `"hhmmss.sss"`.
    fn try_from(s: &str) -> Result<Self, ()> {
        if s.len() < 6 {
            return Err(());
        }
        let hour: u8 = s[..2].parse().map_err(|_| ())?;
        let minute: u8 = s[2..4].parse().map_err(|_| ())?;
        let second: u8 = s[4..6].parse().map_err(|_| ())?;
        let millis: u16 = if s.len() > 7 {
            s[7..].parse().unwrap_or(0)
        } else {
            0
        };
        Ok(NmeaTime {
            hour,
            minute,
            second,
            millis,
        })
    }
}

impl core::convert::TryFrom<&str> for NmeaDate {
    type Error = ();
    /// Parse an NMEA date string `"ddmmyy"` (two-digit year → 20xx).
    fn try_from(s: &str) -> Result<Self, ()> {
        if s.len() < 6 {
            return Err(());
        }
        let day: u8 = s[..2].parse().map_err(|_| ())?;
        let month: u8 = s[2..4].parse().map_err(|_| ())?;
        let year_short: u16 = s[4..6].parse().map_err(|_| ())?;
        Ok(NmeaDate {
            day,
            month,
            year: 2000 + year_short,
        })
    }
}

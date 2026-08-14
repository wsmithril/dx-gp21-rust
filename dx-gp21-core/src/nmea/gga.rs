use crate::types::*;

#[derive(Clone, Copy, Debug)]
pub struct GgaData {
    pub time: NmeaTime,
    pub lat: f64,
    pub lon: f64,
    pub fix_quality: FixQuality,
    pub sats_used: u8,
    pub hdop: f32,
    pub alt_msl: f32,
    pub geoid_sep: f32,
}

impl GgaData {
    pub fn is_valid(&self) -> bool {
        self.fix_quality != FixQuality::Invalid
    }
}

// Fields (f): UTCtime, lat, uLat, lon, uLon, FS, numSv, HDOP, msl, uMsl, sep, uSep, diffAge, diffSta
pub(crate) fn parse(_system: GnssSystem, f: &[&str]) -> Option<GgaData> {
    if f.len() < 9 {
        return None;
    }
    let time = NmeaTime::try_from(f[0]).ok()?;
    let lat = parse_latlon(f[1], f[2])?;
    let lon = parse_latlon(f[3], f[4])?;
    let fix_quality = match f[5] {
        "1" => FixQuality::Sps,
        "6" => FixQuality::Estimated,
        _ => FixQuality::Invalid,
    };
    let sats_used: u8 = f[6].parse().unwrap_or(0);
    let hdop: f32 = f[7].parse().unwrap_or(99.9);
    let alt_msl: f32 = f[8].parse().unwrap_or(0.0);
    let geoid_sep: f32 = if f.len() > 10 {
        f[10].parse().unwrap_or(0.0)
    } else {
        0.0
    };
    Some(GgaData {
        time,
        lat,
        lon,
        fix_quality,
        sats_used,
        hdop,
        alt_msl,
        geoid_sep,
    })
}

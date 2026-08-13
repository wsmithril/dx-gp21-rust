use crate::types::*;

#[derive(Clone, Copy, Debug)]
pub struct RmcData {
    pub time: NmeaTime,
    pub date: NmeaDate,
    pub valid: bool,
    pub lat: f64,
    pub lon: f64,
    pub speed_knots: f32,
    pub course_deg: f32,
}

impl RmcData {
    pub fn is_valid(&self) -> bool { self.valid }
}

// Fields: UTCtime, status, lat, uLat, lon, uLon, spd, cog, date, mv, mvE, mode, navStatus
pub(crate) fn parse(_system: GnssSystem, f: &[&str]) -> Option<RmcData> {
    if f.len() < 9 { return None; }
    let time = NmeaTime::try_from(f[0]).ok()?;
    let valid = f[1] == "A";
    let lat = parse_latlon(f[2], f[3]).unwrap_or(0.0);
    let lon = parse_latlon(f[4], f[5]).unwrap_or(0.0);
    let speed_knots: f32 = f[6].parse().unwrap_or(0.0);
    let course_deg: f32 = f[7].parse().unwrap_or(0.0);
    let date = NmeaDate::try_from(f[8]).unwrap_or_default();
    Some(RmcData { time, date, valid, lat, lon, speed_knots, course_deg })
}

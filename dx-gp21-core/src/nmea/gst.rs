

#[derive(Clone, Copy, Debug, Default)]
pub struct GstData {
    pub rms: f32,
    pub std_lat: f32,
    pub std_lon: f32,
    pub std_alt: f32,
}

// Fields: UTCtime, RMS, stdDevMaj, stdfDevMin, orientation, stdLat, stdLon, stdAlt
pub(crate) fn parse(f: &[&str]) -> Option<GstData> {
    if f.len() < 8 { return None; }
    let rms: f32 = f[1].parse().unwrap_or(0.0);
    let std_lat: f32 = f[5].parse().unwrap_or(0.0);
    let std_lon: f32 = f[6].parse().unwrap_or(0.0);
    let std_alt: f32 = f[7].parse().unwrap_or(0.0);
    Some(GstData { rms, std_lat, std_lon, std_alt })
}

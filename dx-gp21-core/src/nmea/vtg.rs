#[derive(Clone, Copy, Debug)]
pub struct VtgData {
    pub course_true: f32,
    pub speed_knots: f32,
    pub speed_kmh: f32,
}

// Fields: cogt, T, cogm, M, sog, N, kph, K, mode
pub(crate) fn parse(f: &[&str]) -> Option<VtgData> {
    if f.len() < 7 {
        return None;
    }
    let course_true: f32 = f[0].parse().unwrap_or(0.0);
    let speed_knots: f32 = f[4].parse().unwrap_or(0.0);
    let speed_kmh: f32 = f[6].parse().unwrap_or(0.0);
    Some(VtgData {
        course_true,
        speed_knots,
        speed_kmh,
    })
}

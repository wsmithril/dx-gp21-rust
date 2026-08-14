#[derive(Clone, Copy, Debug)]
pub struct DhvData {
    pub speed_3d: f32,
    pub speed_x: f32,
    pub speed_y: f32,
    pub speed_z: f32,
    pub ground_speed: f32,
}

// Fields: UTCtime, speed3D, spdX, spdY, spdZ, gdspd, …
pub(crate) fn parse(f: &[&str]) -> Option<DhvData> {
    if f.len() < 6 {
        return None;
    }
    let speed_3d: f32 = f[1].parse().unwrap_or(0.0);
    let speed_x: f32 = f[2].parse().unwrap_or(0.0);
    let speed_y: f32 = f[3].parse().unwrap_or(0.0);
    let speed_z: f32 = f[4].parse().unwrap_or(0.0);
    let ground_speed: f32 = f[5].parse().unwrap_or(0.0);
    Some(DhvData {
        speed_3d,
        speed_x,
        speed_y,
        speed_z,
        ground_speed,
    })
}

use crate::types::*;

#[derive(Clone, Copy, Debug, Default)]
pub struct GsaData {
    pub system: GnssSystem,
    pub fix_mode: FixMode,
    pub pdop: f32,
    pub hdop: f32,
    pub vdop: f32,
    pub svids: [Option<u16>; 12],
}

impl From<GsaData> for DopValues {
    fn from(g: GsaData) -> Self {
        Self { pdop: g.pdop, hdop: g.hdop, vdop: g.vdop }
    }
}

// Fields: smode, FS, sv1..sv12, PDOP, HDOP, VDOP[, systemId]
pub(crate) fn parse(system: GnssSystem, f: &[&str]) -> Option<GsaData> {
    if f.len() < 15 { return None; }
    let fix_mode = match f[1] {
        "2" => FixMode::Fix2D,
        "3" => FixMode::Fix3D,
        _ => FixMode::NoFix,
    };
    let mut svids = [None::<u16>; 12];
    for i in 0..12 {
        if !f[2 + i].is_empty() { svids[i] = f[2 + i].parse().ok(); }
    }
    let pdop: f32 = f[14].parse().unwrap_or(99.9);
    let hdop: f32 = if f.len() > 15 { f[15].parse().unwrap_or(99.9) } else { 99.9 };
    let vdop: f32 = if f.len() > 16 { f[16].parse().unwrap_or(99.9) } else { 99.9 };
    let resolved_system = if f.len() > 17 {
        match f[17] {
            "1" => GnssSystem::Gps, "2" => GnssSystem::Glonass,
            "4" => GnssSystem::Beidou, "8" => GnssSystem::Galileo,
            _ => system,
        }
    } else { system };
    Some(GsaData { system: resolved_system, fix_mode, pdop, hdop, vdop, svids })
}

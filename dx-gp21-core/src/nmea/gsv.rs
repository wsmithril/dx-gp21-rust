use crate::types::*;

#[derive(Clone, Copy, Debug)]
pub struct GsvData {
    pub system: GnssSystem,
    pub total_msgs: u8,
    pub msg_num: u8,
    pub total_in_view: u8,
    pub sats: [Option<SatInfo>; 4],
}

// Fields: numMsg, msgNo, numSv, [svid, ele, az, cn0]×N, [signalId]
pub(crate) fn parse(system: GnssSystem, f: &[&str]) -> Option<GsvData> {
    if f.len() < 3 { return None; }
    let total_msgs: u8 = f[0].parse().ok()?;
    let msg_num: u8 = f[1].parse().ok()?;
    let total_in_view: u8 = f[2].parse().unwrap_or(0);
    let sat_fields = &f[3..];
    let mut sats = [None::<SatInfo>; 4];
    for (i, chunk) in sat_fields.chunks(4).take(4).enumerate() {
        if chunk.len() < 4 { break; }
        let svid: u16 = match chunk[0].parse() { Ok(v) => v, Err(_) => continue };
        let elevation: Option<i8>  = chunk[1].parse().ok();
        let azimuth:   Option<u16> = chunk[2].parse().ok();
        let snr: Option<u8> = if chunk[3].is_empty() { None } else { chunk[3].parse().ok() };
        sats[i] = Some(SatInfo { svid, system, elevation, azimuth, snr, used: false });
    }
    Some(GsvData { system, total_msgs, msg_num, total_in_view, sats })
}

use crate::types::*;

#[derive(Clone, Copy, Debug, Default)]
pub struct TxtData {
    pub antenna_status: AntennaStatus,
}

// Fields: xx (total msgs), yy (msg num), zz (type), info
pub(crate) fn parse(f: &[&str]) -> Option<TxtData> {
    if f.len() < 4 {
        return None;
    }
    let antenna_status = match f[3] {
        s if s.contains("OK") => AntennaStatus::Ok,
        s if s.contains("OPEN") => AntennaStatus::Open,
        s if s.contains("SHORT") => AntennaStatus::Short,
        _ => AntennaStatus::Unknown,
    };
    Some(TxtData { antenna_status })
}

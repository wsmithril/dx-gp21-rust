use crate::types::*;

#[derive(Clone, Copy, Debug)]
pub struct ZdaData {
    pub time: NmeaTime,
    pub date: NmeaDate,
}

// Fields: UTCtime, day, month, year, ltzh, ltzn
pub(crate) fn parse(f: &[&str]) -> Option<ZdaData> {
    if f.len() < 4 {
        return None;
    }
    let time = NmeaTime::try_from(f[0]).ok()?;
    let day: u8 = f[1].parse().ok()?;
    let month: u8 = f[2].parse().ok()?;
    let year: u16 = f[3].parse().ok()?;
    Some(ZdaData {
        time,
        date: NmeaDate { day, month, year },
    })
}

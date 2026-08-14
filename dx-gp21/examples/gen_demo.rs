//! Generates a 60-second synthetic NMEA demo log with a realistic
//! satellite acquisition sequence:
//!
//!  0–9 s   — antenna on, no fix; satellites gradually appearing in GSV
//! 10–19 s  — 2D fix; position locked, altitude unknown
//! 20–59 s  — 3D fix; full position + altitude + speed, DOP improving
//!
//! Location: mid-Pacific Ocean (21°18.742′N, 157°51.532′W) — open water,
//! no sensitive information.
//!
//! ```text
//! cargo run --example gen_demo > dx-gp21/examples/demo.nmea
//! ```

fn cs(body: &str) -> String {
    format!("{:02X}", body.bytes().fold(0u8, |a, b| a ^ b))
}

fn emit(body: &str) { println!("${}*{}", body, cs(body)); }

fn main() {
    // ── Fixed parameters ──────────────────────────────────────────────────────
    const LAT:     &str = "2118.7420,N";
    const LON:     &str = "15751.5320,W";
    const ALT_MSL: f32  = 12.4;
    const GEOID:   f32  = -26.2;

    // Satellite constellations visible (realistic Pacific geometry)
    let gps_sats: &[(u16, i8, u16, u8)] = &[
        (2,  48, 312, 38), (6,  22, 048, 24), (12, 61, 199, 44),
        (17, 15, 140, 20), (24, 73, 082, 46), (29, 33, 261, 35),
        (51, 19, 095, 22),
    ];
    let bds_sats: &[(u16, i8, u16, u8)] = &[
        (1,  42, 175, 36), (3,  28, 230, 30), (6,  55, 110, 42),
        (14, 18, 350, 26), (22, 67, 050, 40),
    ];
    let glo_sats: &[(u16, i8, u16, u8)] = &[
        (65, 35, 155, 31), (72, 51, 025, 38),
    ];

    // Satellites used in each phase
    let active_gps_2d: &[u16] = &[2, 12, 24];
    let active_gps_3d: &[u16] = &[2, 6, 12, 24, 29];
    let active_bds_3d: &[u16] = &[1, 6, 14];
    let active_glo_3d: &[u16] = &[65, 72];

    // ── Emit 60 seconds ───────────────────────────────────────────────────────
    for sec in 0u32..60 {
        let h = 14u32;
        let m = 30u32 + sec / 60;
        let s = sec % 60;
        let time = format!("{:02}{:02}{:02}.00", h, m, s);

        // Phase logic
        let no_fix  = sec < 10;
        let fix_2d  = sec >= 10 && sec < 20;
        let fix_3d  = sec >= 20;

        // DOP and HDOP improve as fix settles
        let (pdop, hdop, vdop) = if no_fix     { (99.9, 99.9, 99.9) }
                                 else if fix_2d { (3.8, 2.9, 2.4) }
                                 else {
                                     // gradual improvement 20→40s, then stable
                                     let t = (sec - 20).min(20) as f32 / 20.0;
                                     let p = 2.8 - t * 1.6;
                                     let h = 1.9 - t * 0.7;
                                     let v = 2.1 - t * 1.0;
                                     (p, h, v)
                                 };
        let nsv = if no_fix { 0u8 }
                  else if fix_2d { active_gps_2d.len() as u8 }
                  else { (active_gps_3d.len() + active_bds_3d.len() + active_glo_3d.len()) as u8 };

        // Antenna message at startup
        if sec == 0 { emit("GPTXT,01,01,01,ANTENNA OK"); }

        // ── RMC ──────────────────────────────────────────────────────────────
        if no_fix {
            emit(&format!("GNRMC,{},V,,,,,,,140826,,,N,V", time));
        } else {
            // tiny drift east (~0.5 m/s); lon_adj encodes mm.mmmm × 10000
            let lon_min = 515320u32 + sec / 2;  // starts at 51.5320′, +0.5″/s
            let lon_str = format!("157{:02}.{:04},W", lon_min / 10000, lon_min % 10000);
            let speed = if fix_3d { 0.27f32 } else { 0.0 };
            emit(&format!("GNRMC,{},A,{},{},{:.2},,140826,,,A,V",
                time, LAT, lon_str, speed));
        }

        // ── VTG ──────────────────────────────────────────────────────────────
        if fix_3d {
            emit("GNVTG,087.4,T,,M,0.27,N,0.50,K,A");
        } else {
            emit("GNVTG,,,,,,,,,N");
        }

        // ── GGA ──────────────────────────────────────────────────────────────
        if no_fix {
            emit(&format!("GNGGA,{},,,,,0,00,99.9,,M,,M,,", time));
        } else {
            let alt   = if fix_2d { 0.0 } else { ALT_MSL };
            let geoid = if fix_2d { 0.0 } else { GEOID };
            emit(&format!("GNGGA,{},{},{},1,{:02},{:.1},{:.1},M,{:.1},M,,",
                time, LAT, LON, nsv, hdop, alt, geoid));
        }

        // ── GSA (one per constellation) ───────────────────────────────────────
        let svid_str = |ids: &[u16]| -> String {
            let mut s = String::new();
            for (i, id) in ids.iter().enumerate() {
                if i > 0 { s.push(','); }
                s.push_str(&id.to_string());
            }
            for _ in ids.len()..12 { s.push(','); }
            s
        };

        let (mode, gps_ids, bds_ids, glo_ids): (u8, &[u16], &[u16], &[u16]) = if no_fix {
            (1, &[], &[], &[])
        } else if fix_2d {
            (2, active_gps_2d, &[], &[])
        } else {
            (3, active_gps_3d, active_bds_3d, active_glo_3d)
        };

        emit(&format!("GNGSA,A,{},{},{:.1},{:.1},{:.1},1",
            mode, svid_str(gps_ids), pdop, hdop, vdop));
        emit(&format!("GNGSA,A,{},{},{:.1},{:.1},{:.1},4",
            mode, svid_str(bds_ids), pdop, hdop, vdop));
        emit(&format!("GNGSA,A,{},{},{:.1},{:.1},{:.1},2",
            mode, svid_str(glo_ids), pdop, hdop, vdop));

        // ── GSV (every 2 s) ───────────────────────────────────────────────────
        if sec % 2 == 0 {
            // Satellites appear gradually during acquisition phase
            let visible_gps = if sec < 3 { 2usize }
                              else if sec < 6 { 4 }
                              else { gps_sats.len() };
            let visible_bds = if sec < 5 { 2usize }
                              else { bds_sats.len() };
            let visible_glo = glo_sats.len();

            let sat4 = |sats: &[(u16, i8, u16, u8)]| -> String {
                sats.iter().map(|(id, el, az, sn)| format!(",{},{},{},{}", id, el, az, sn))
                    .collect()
            };

            // GPS GSV (2 messages)
            let gps_v = &gps_sats[..visible_gps];
            if !gps_v.is_empty() {
                let (g1, g2) = gps_v.split_at(gps_v.len().min(4));
                emit(&format!("GPGSV,2,1,{}{}", visible_gps, sat4(g1)));
                if !g2.is_empty() {
                    emit(&format!("GPGSV,2,2,{}{}", visible_gps, sat4(g2)));
                }
            }
            // BDS GSV
            let bds_v = &bds_sats[..visible_bds];
            if !bds_v.is_empty() {
                emit(&format!("GBGSV,1,1,{}{}", visible_bds, sat4(bds_v)));
            }
            // GLONASS GSV
            emit(&format!("GLGSV,1,1,{}{}", visible_glo, sat4(glo_sats)));
        }

        // ── ZDA ───────────────────────────────────────────────────────────────
        emit(&format!("GNZDA,{},14,08,2026,00,00", time));
    }
}

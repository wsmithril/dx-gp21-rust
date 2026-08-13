use std::convert::TryFrom;

use dx_gp21::{
    FixQuality, GnssState, GnssStore, ParsedSentence, SentenceType, feed_sentence,
};

// Synthetic NMEA sentences — coordinates are round numbers (40 °N 116 °E)
// and do NOT correspond to any real location.
// All checksums were computed from the sentence bodies and are correct.
const SAMPLE: &[&str] = &[
    "$GNRMC,120000.00,A,4000.00000,N,11600.00000,E,0.00,,010101,,,A,V*24",
    "$GNVTG,,,,,0.00,N,0.00,K,A*24",
    "$GNGGA,120000.00,4000.00000,N,11600.00000,E,1,08,1.2,100.0,M,0.0,M,,*49",
    "$GNGSA,A,3,01,02,,,,,,,,,,,2.0,1.2,1.6,1*34",
    "$GNGSA,A,3,,,,,,,,,,,,,2.0,1.2,1.6,2*34",
    "$GNGSA,A,3,,,,,,,,,,,,,2.0,1.2,1.6,3*35",
    "$GNGSA,A,3,05,06,07,08,,,,,,,,,2.0,1.2,1.6,4*3E",
    "$GNGSA,A,3,09,,,,,,,,,,,,2.0,1.2,1.6,5*3A",
    "$GPGSV,1,1,04,01,45,090,35,02,30,180,30,03,20,270,25,04,10,000,20,1*60",
    "$GBGSV,1,1,04,05,50,045,38,06,35,135,33,07,25,225,28,08,15,315,23,1*7A",
    "$GNZDA,120000.00,01,01,2001,00,00*78",
    "$GPTXT,01,01,01,ANTENNA OK*35",
];

// ── helpers ───────────────────────────────────────────────────────────────────

fn parse_inline(sentences: &[&str]) -> (GnssState, usize, usize) {
    let mut state = GnssState::default();
    let (mut parsed, total) = (0usize, sentences.len());
    for &s in sentences {
        if feed_sentence(&mut state, s.as_bytes()).is_some() {
            parsed += 1;
        }
    }
    (state, parsed, total)
}

// ── TryFrom entry-point tests ─────────────────────────────────────────────────

#[test]
fn try_from_bytes_gga() {
    // Synthetic: 40.000 °N 116.000 °E — round numbers, not a real location.
    let line = b"$GNGGA,073028.600,4000.00000,N,11600.00000,E,1,08,1.2,100.0,M,0.0,M,,*72";
    let s = ParsedSentence::try_from(line.as_ref()).expect("GGA should parse");
    match s {
        ParsedSentence::Gga(gga) => {
            assert!(gga.lat > 39.0 && gga.lat < 41.0, "lat {}", gga.lat);
            assert_eq!(gga.sats_used, 8);
            assert!((gga.hdop - 1.2).abs() < 0.01);
        }
        other => panic!("expected Gga, got {:?}", other.kind()),
    }
}

#[test]
fn try_from_str_rmc() {
    // Synthetic: 40.000 °N 116.000 °E, date 2024-07-09.
    let line = "$GNRMC,073028.600,A,4000.00000,N,11600.00000,E,0.00,0.00,090724,,,A,V*08";
    let s = ParsedSentence::try_from(line).expect("RMC should parse");
    assert!(matches!(s, ParsedSentence::Rmc(_)), "expected Rmc, got {:?}", s.kind());
    if let ParsedSentence::Rmc(rmc) = s {
        assert!(rmc.valid);
        assert_eq!(rmc.date.year, 2024);
        assert_eq!(rmc.date.month, 7);
        assert_eq!(rmc.date.day, 9);
    }
}

#[test]
fn try_from_bad_checksum_returns_error() {
    // Correct body but wrong checksum byte — must be rejected.
    let bad = b"$GNGGA,073028.600,4000.00000,N,11600.00000,E,1,08,1.2,100.0,M,0.0,M,,*FF";
    assert!(ParsedSentence::try_from(bad.as_ref()).is_err());
}

#[test]
fn try_from_unknown_sentence_returns_error() {
    // Valid checksum, unknown sentence type (GNFOO).
    let line = b"$GNFOO,1,2,3*53";
    assert!(ParsedSentence::try_from(line.as_ref()).is_err());
}

#[test]
fn kind_matches_variant() {
    let line = b"$GNGGA,073028.600,4000.00000,N,11600.00000,E,1,08,1.2,100.0,M,0.0,M,,*72";
    let s = ParsedSentence::try_from(line.as_ref()).unwrap();
    assert_eq!(s.kind(), SentenceType::Gga);
}

#[test]
fn copy_allows_dual_use() {
    // Because ParsedSentence is Copy, we can update state AND use the data.
    let line = b"$GNGGA,073028.600,4000.00000,N,11600.00000,E,1,08,1.2,100.0,M,0.0,M,,*72";
    let mut state = GnssState::default();

    if let Some(ParsedSentence::Gga(gga)) = feed_sentence(&mut state, line) {
        // Caller gets the data directly from the return value.
        assert!((gga.alt_msl - 100.0).abs() < 0.1);
    } else {
        panic!("expected GGA");
    }
    // State was also updated.
    assert!(state.gga().is_some());
}

// ── Inline-data regression tests ──────────────────────────────────────────────

#[test]
fn sample_has_valid_coordinates() {
    let (state, _, _) = parse_inline(SAMPLE);
    if let Some(gga) = state.gga() {
        if gga.fix_quality != FixQuality::Invalid {
            assert!(state.has_fix(), "expected a position fix");
            assert!(gga.lat > 39.0 && gga.lat < 41.0, "unexpected lat: {}", gga.lat);
            assert!(gga.lon > 115.0 && gga.lon < 118.0, "unexpected lon: {}", gga.lon);
        }
        eprintln!("Last GGA: lat={:.6} lon={:.6} alt={:.1}m", gga.lat, gga.lon, gga.alt_msl);
    }
}

#[test]
fn sample_has_satellites() {
    let (state, _, _) = parse_inline(SAMPLE);
    let sats = state.satellites();
    eprintln!("{} satellites in state, {} used", sats.len(), state.sats_used_count());
    for s in sats {
        eprintln!("  {:3} {:3} el={:?} az={:?} snr={:?}", s.svid, s.system.label(), s.elevation, s.azimuth, s.snr);
    }
    assert!(!sats.is_empty(), "satellite list should not be empty");
}

#[test]
fn sample_time_populated() {
    let (state, _, _) = parse_inline(SAMPLE);
    let has_time = state.gga().is_some() || state.rmc().is_some();
    assert!(has_time, "expected GGA or RMC to be populated");
    if let Some(gga) = state.gga() {
        eprintln!("Last time: {:02}:{:02}:{:02} UTC", gga.time.hour, gga.time.minute, gga.time.second);
    }
}

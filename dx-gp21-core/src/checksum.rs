/// Compute XOR checksum of all bytes (used for the body between '$' and '*').
pub fn compute(body: &[u8]) -> u8 {
    body.iter().fold(0u8, |acc, &b| acc ^ b)
}

/// Verify an NMEA sentence line. Returns the body slice (between '$' and '*') on success.
pub fn verify(line: &[u8]) -> Option<&[u8]> {
    if line.first() != Some(&b'$') {
        return None;
    }
    let star_pos = line.iter().rposition(|&b| b == b'*')?;
    let body = &line[1..star_pos];
    let cs_slice = &line[star_pos + 1..];
    // trim \r\n
    let cs_str = core::str::from_utf8(cs_slice).ok()?.trim_end_matches(['\r', '\n']);
    if cs_str.len() < 2 { return None; }
    let expected = u8::from_str_radix(&cs_str[..2], 16).ok()?;
    if compute(body) == expected { Some(body) } else { None }
}

pub(crate) fn nibble_to_hex(n: u8) -> u8 {
    match n {
        0..=9 => b'0' + n,
        _ => b'A' + n - 10,
    }
}

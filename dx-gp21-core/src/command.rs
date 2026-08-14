use crate::checksum::{compute, nibble_to_hex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[derive(Default)]
pub enum BaudRate {
    B4800,
    B9600,
    B19200,
    B38400,
    B57600,
    #[default]
    B115200,
}


impl BaudRate {
    pub fn param(self) -> u8 {
        match self {
            Self::B4800   => 0,
            Self::B9600   => 1,
            Self::B19200  => 2,
            Self::B38400  => 3,
            Self::B57600  => 4,
            Self::B115200 => 5,
        }
    }

    pub fn bps(self) -> u32 {
        match self {
            Self::B4800   => 4800,
            Self::B9600   => 9600,
            Self::B19200  => 19200,
            Self::B38400  => 38400,
            Self::B57600  => 57600,
            Self::B115200 => 115200,
        }
    }

    pub fn from_bps(bps: u32) -> Option<Self> {
        match bps {
            4800   => Some(Self::B4800),
            9600   => Some(Self::B9600),
            19200  => Some(Self::B19200),
            38400  => Some(Self::B38400),
            57600  => Some(Self::B57600),
            115200 => Some(Self::B115200),
            _      => None,
        }
    }
}

impl core::fmt::Display for BaudRate {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{} bps", self.bps())
    }
}

impl core::convert::TryFrom<u32> for BaudRate {
    type Error = ();
    fn try_from(bps: u32) -> Result<Self, ()> { Self::from_bps(bps).ok_or(()) }
}

impl From<BaudRate> for u32 {
    fn from(r: BaudRate) -> u32 { r.bps() }
}

impl BaudRate {
    /// Round `bps` to the nearest supported baud rate.
    pub fn nearest(bps: u32) -> Self {
        [Self::B4800, Self::B9600, Self::B19200, Self::B38400, Self::B57600, Self::B115200]
            .into_iter()
            .min_by_key(|r| r.bps().abs_diff(bps))
            .unwrap_or(Self::B115200)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[derive(Default)]
pub enum UpdateRate {
    #[default]
    Hz1,
    Hz2,
    Hz5,
    Hz10,
}


impl UpdateRate {
    pub fn period_ms(self) -> u32 {
        match self {
            Self::Hz1  => 1000,
            Self::Hz2  => 500,
            Self::Hz5  => 200,
            Self::Hz10 => 100,
        }
    }

    pub fn hz(self) -> u8 {
        match self {
            Self::Hz1  => 1,
            Self::Hz2  => 2,
            Self::Hz5  => 5,
            Self::Hz10 => 10,
        }
    }
}

impl core::fmt::Display for UpdateRate {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{} Hz", self.hz())
    }
}

impl core::convert::TryFrom<u8> for UpdateRate {
    type Error = ();
    fn try_from(hz: u8) -> Result<Self, ()> {
        match hz { 1 => Ok(Self::Hz1), 2 => Ok(Self::Hz2), 5 => Ok(Self::Hz5), 10 => Ok(Self::Hz10), _ => Err(()) }
    }
}

impl core::convert::TryFrom<u32> for UpdateRate {
    type Error = ();
    fn try_from(ms: u32) -> Result<Self, ()> {
        match ms { 1000 => Ok(Self::Hz1), 500 => Ok(Self::Hz2), 200 => Ok(Self::Hz5), 100 => Ok(Self::Hz10), _ => Err(()) }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestartMode {
    Hot,
    Warm,
    Cold,
    Factory,
}

impl RestartMode {
    pub fn param(self) -> u8 {
        match self {
            Self::Hot => 0,
            Self::Warm => 1,
            Self::Cold => 2,
            Self::Factory => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InfoField {
    Firmware,
    Hardware,
    Mode,
    Customer,
    Upgrade,
}

impl InfoField {
    pub fn param(self) -> u8 {
        match self {
            Self::Firmware => 0,
            Self::Hardware => 1,
            Self::Mode => 2,
            Self::Customer => 3,
            Self::Upgrade => 5,
        }
    }
}

/// Typed bitmask for GNSS constellation selection ($PCAS04).
///
/// Use the associated constants and `|` to compose masks:
/// ```
/// use dx_gp21_core::command::ConstellationMask;
/// let mask = ConstellationMask::GPS | ConstellationMask::BDS;
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConstellationMask(pub u8);

impl Default for ConstellationMask {
    fn default() -> Self { Self::ALL }
}

impl ConstellationMask {
    pub const GPS:     Self = Self(0x01);
    pub const BDS:     Self = Self(0x02);
    pub const GLONASS: Self = Self(0x04);
    pub const GALILEO: Self = Self(0x08);
    pub const QZSS:    Self = Self(0x10);
    pub const ALL:     Self = Self(0x7F);

    pub fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Return the underlying byte value. Prefer `u8::from(mask)`.
    #[inline]
    pub fn as_u8(self) -> u8 { self.0 }
}

impl From<u8> for ConstellationMask {
    fn from(v: u8) -> Self { Self(v) }
}

impl From<ConstellationMask> for u8 {
    fn from(m: ConstellationMask) -> u8 { m.0 }
}

impl core::ops::BitOr for ConstellationMask {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self { Self(self.0 | rhs.0) }
}

impl core::ops::BitOrAssign for ConstellationMask {
    fn bitor_assign(&mut self, rhs: Self) { self.0 |= rhs.0; }
}

impl core::ops::BitAnd for ConstellationMask {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self { Self(self.0 & rhs.0) }
}

impl core::ops::BitAndAssign for ConstellationMask {
    fn bitand_assign(&mut self, rhs: Self) { self.0 &= rhs.0; }
}

impl core::ops::Not for ConstellationMask {
    type Output = Self;
    fn not(self) -> Self { Self(!self.0) }
}

impl core::fmt::Display for ConstellationMask {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if *self == Self::ALL {
            return f.write_str("GPS+BDS+GLO+GAL+QZS");
        }
        let mut first = true;
        for &(bit, label) in &[
            (Self::GPS,     "GPS"),
            (Self::BDS,     "BDS"),
            (Self::GLONASS, "GLO"),
            (Self::GALILEO, "GAL"),
            (Self::QZSS,    "QZS"),
        ] {
            if self.contains(bit) {
                if !first { f.write_str("+")?; }
                f.write_str(label)?;
                first = false;
            }
        }
        Ok(())
    }
}

/// Raw `u8` constants for low-level / FFI use. Prefer [`ConstellationMask`].
pub mod systems {
    pub const GPS:     u8 = 0x01;
    pub const BDS:     u8 = 0x02;
    pub const GLONASS: u8 = 0x04;
    pub const GALILEO: u8 = 0x08;
    pub const QZSS:    u8 = 0x10;
    pub const ALL:     u8 = 0x7F;
}

// ── Internal helpers ──────────────────────────────────────────────────────────

struct Builder<'a> {
    buf: &'a mut [u8],
    pos: usize,
}

impl<'a> Builder<'a> {
    fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn push(&mut self, b: u8) {
        if self.pos < self.buf.len() {
            self.buf[self.pos] = b;
            self.pos += 1;
        }
    }

    fn push_bytes(&mut self, s: &[u8]) {
        for &b in s { self.push(b); }
    }

    fn push_u8_decimal(&mut self, v: u8) {
        if v >= 100 { self.push(b'0' + v / 100); }
        if v >= 10  { self.push(b'0' + (v / 10) % 10); }
        self.push(b'0' + v % 10);
    }

    fn push_u32_decimal(&mut self, mut v: u32) {
        let mut tmp = [0u8; 10];
        let mut n = 0;
        loop {
            tmp[n] = b'0' + (v % 10) as u8;
            n += 1;
            v /= 10;
            if v == 0 { break; }
        }
        for i in (0..n).rev() { self.push(tmp[i]); }
    }

    fn push_hex_u8(&mut self, v: u8) {
        self.push(nibble_to_hex(v >> 4));
        self.push(nibble_to_hex(v & 0xF));
    }

    /// Finalise: wrap content (between pos 0..body_len) with $…*CS\r\n.
    /// Content must be written to buf[0..body_len] before calling.
    fn finalise(self) -> usize {
        let body_len = self.pos;
        let cs = compute(&self.buf[..body_len]);
        let needed = body_len + 6; // $ + body + * + 2hex + \r\n
        if self.buf.len() < needed { return 0; }
        // Shift body right by 1 to make room for '$'
        self.buf.copy_within(0..body_len, 1);
        self.buf[0] = b'$';
        self.buf[body_len + 1] = b'*';
        self.buf[body_len + 2] = nibble_to_hex(cs >> 4);
        self.buf[body_len + 3] = nibble_to_hex(cs & 0xF);
        self.buf[body_len + 4] = b'\r';
        self.buf[body_len + 5] = b'\n';
        needed
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Write a pre-formed command (the full "$…*XX\r\n" body known at compile time).
fn write_fixed(buf: &mut [u8], cmd: &[u8]) -> usize {
    if buf.len() < cmd.len() { return 0; }
    buf[..cmd.len()].copy_from_slice(cmd);
    cmd.len()
}

pub fn cmd_save_config(buf: &mut [u8]) -> usize {
    write_fixed(buf, b"$PCAS00*01\r\n")
}

pub fn cmd_set_baud(buf: &mut [u8], rate: BaudRate) -> usize {
    let mut b = Builder::new(buf);
    b.push_bytes(b"PCAS01,");
    b.push(b'0' + rate.param());
    b.finalise()
}

pub fn cmd_set_update_rate(buf: &mut [u8], rate: UpdateRate) -> usize {
    let mut b = Builder::new(buf);
    b.push_bytes(b"PCAS02,");
    b.push_u32_decimal(rate.period_ms());
    b.finalise()
}

/// Configure NMEA output. Each field is `Some(n)` = output every n fixes, `None` = keep current.
#[allow(clippy::too_many_arguments)]
pub fn cmd_set_nmea_output(
    buf: &mut [u8],
    gga: Option<u8>, gll: Option<u8>, gsa: Option<u8>, gsv: Option<u8>,
    rmc: Option<u8>, vtg: Option<u8>, zda: Option<u8>, txt: Option<u8>,
    dhv: Option<u8>, gst: Option<u8>,
) -> usize {
    let mut b = Builder::new(buf);
    b.push_bytes(b"PCAS03");
    let fields = [gga, gll, gsa, gsv, rmc, vtg, zda, txt, dhv];
    for f in &fields {
        b.push(b',');
        if let Some(v) = f { b.push_u8_decimal(*v); }
    }
    // Res1..Res4 (reserved, leave empty)
    b.push_bytes(b",,,");
    // nGST
    b.push(b',');
    if let Some(v) = gst { b.push_u8_decimal(v); }
    b.finalise()
}

pub fn cmd_set_systems(buf: &mut [u8], mask: ConstellationMask) -> usize {
    let mut b = Builder::new(buf);
    b.push_bytes(b"PCAS04,");
    if mask == ConstellationMask::ALL {
        b.push_bytes(b"7F");
    } else {
        b.push_hex_u8(mask.as_u8());
    }
    b.finalise()
}

pub fn cmd_set_nmea_version(buf: &mut [u8], v41_plus: bool) -> usize {
    write_fixed(buf, if v41_plus { b"$PCAS05,2*1A\r\n" } else { b"$PCAS05,5*1D\r\n" })
}

pub fn cmd_query_info(buf: &mut [u8], field: InfoField) -> usize {
    let mut b = Builder::new(buf);
    b.push_bytes(b"PCAS06,");
    b.push(b'0' + field.param());
    b.finalise()
}

pub fn cmd_restart(buf: &mut [u8], mode: RestartMode) -> usize {
    let mut b = Builder::new(buf);
    b.push_bytes(b"PCAS10,");
    b.push(b'0' + mode.param());
    b.finalise()
}

// ── CommandSink trait ─────────────────────────────────────────────────────────

/// Implemented by anything that can send bytes to a GNSS module.
///
/// Require only `send_raw`; all command helpers are provided as defaults via the
/// `cmd_*` builder functions so there is no boilerplate in implementors.
///
/// Use `&mut self` so it works naturally with embedded HAL write types.
/// (For shared/interior-mutable writers in std, wrap with `RefCell` or `Mutex`.)
pub trait CommandSink {
    type Error;

    /// Write raw bytes to the module (e.g. over UART). Must be a complete,
    /// checksummed `$PCAS…*XX\r\n` command.
    fn send_raw(&mut self, bytes: &[u8]) -> Result<(), Self::Error>;

    /// Returns `false` for read-only stubs (e.g. test harnesses).
    fn is_writable(&self) -> bool { true }

    fn save_config(&mut self) -> Result<(), Self::Error> {
        let mut buf = [0u8; 32]; let n = cmd_save_config(&mut buf); self.send_raw(&buf[..n])
    }
    fn restart(&mut self, mode: RestartMode) -> Result<(), Self::Error> {
        let mut buf = [0u8; 32]; let n = cmd_restart(&mut buf, mode); self.send_raw(&buf[..n])
    }
    fn set_baud(&mut self, rate: BaudRate) -> Result<(), Self::Error> {
        let mut buf = [0u8; 32]; let n = cmd_set_baud(&mut buf, rate); self.send_raw(&buf[..n])
    }
    fn set_update_rate(&mut self, rate: UpdateRate) -> Result<(), Self::Error> {
        let mut buf = [0u8; 32]; let n = cmd_set_update_rate(&mut buf, rate); self.send_raw(&buf[..n])
    }
    fn set_systems(&mut self, mask: ConstellationMask) -> Result<(), Self::Error> {
        let mut buf = [0u8; 32]; let n = cmd_set_systems(&mut buf, mask); self.send_raw(&buf[..n])
    }
    fn query_info(&mut self, field: InfoField) -> Result<(), Self::Error> {
        let mut buf = [0u8; 32]; let n = cmd_query_info(&mut buf, field); self.send_raw(&buf[..n])
    }
    fn set_nmea_version(&mut self, v41_plus: bool) -> Result<(), Self::Error> {
        let mut buf = [0u8; 32]; let n = cmd_set_nmea_version(&mut buf, v41_plus); self.send_raw(&buf[..n])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_config_correct() {
        let mut buf = [0u8; 32];
        let n = cmd_save_config(&mut buf);
        assert_eq!(&buf[..n], b"$PCAS00*01\r\n");
    }

    #[test]
    fn set_baud_115200() {
        let mut buf = [0u8; 32];
        let n = cmd_set_baud(&mut buf, BaudRate::B115200);
        assert_eq!(&buf[..n], b"$PCAS01,5*19\r\n");
    }

    #[test]
    fn set_update_1hz() {
        let mut buf = [0u8; 32];
        let n = cmd_set_update_rate(&mut buf, UpdateRate::Hz1);
        assert_eq!(&buf[..n], b"$PCAS02,1000*2E\r\n");
    }

    #[test]
    fn restart_cold() {
        let mut buf = [0u8; 32];
        let n = cmd_restart(&mut buf, RestartMode::Cold);
        assert_eq!(&buf[..n], b"$PCAS10,2*1E\r\n");
    }

    #[test]
    fn set_systems_all() {
        let mut buf = [0u8; 32];
        let n = cmd_set_systems(&mut buf, ConstellationMask::ALL);
        assert_eq!(&buf[..n], b"$PCAS04,7F*58\r\n");
    }
}

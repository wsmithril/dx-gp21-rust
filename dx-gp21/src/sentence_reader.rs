use std::io::BufRead;
use dx_gp21_core::{ParsedSentence, ParseError};

/// A raw NMEA line paired with its parse result.
///
/// Both fields are always populated: `raw` carries the original bytes for
/// logging; `parsed` holds the typed sentence or the reason it failed.
#[derive(Clone, Debug)]
pub struct SentenceLine {
    /// The original NMEA line, trimmed of `\r\n`.
    pub raw: String,
    /// `Ok(sentence)` on successful parse, `Err(ParseError)` for checksum
    /// failures, malformed fields, or unknown sentence types.
    pub parsed: Result<ParsedSentence, ParseError>,
}

impl SentenceLine {
    pub fn is_valid(&self) -> bool { self.parsed.is_ok() }
}

/// An iterator over NMEA sentences from any [`BufRead`] source.
///
/// Each iteration yields a [`SentenceLine`] containing the raw bytes and the
/// parse result. Empty lines are skipped silently. `None` is returned only on
/// EOF or an I/O error.
///
/// # Example
/// ```ignore
/// use std::io::BufReader;
/// use dx_gp21::sentence_reader::SentenceReader;
/// use dx_gp21::ParsedSentence;
///
/// let reader = SentenceReader::new(BufReader::new(port));
/// for line in reader {
///     if let Ok(ParsedSentence::Gga(gga)) = line.parsed {
///         println!("{:.6}, {:.6}", gga.lat, gga.lon);
///     }
/// }
/// ```
pub struct SentenceReader<R: BufRead> {
    inner: R,
    buf: String,
}

impl<R: BufRead> SentenceReader<R> {
    pub fn new(inner: R) -> Self {
        Self { inner, buf: String::with_capacity(128) }
    }

    /// Consume the reader and return the underlying source.
    pub fn into_inner(self) -> R { self.inner }
}

impl<R: BufRead> Iterator for SentenceReader<R> {
    type Item = SentenceLine;

    fn next(&mut self) -> Option<Self::Item> {
        use std::io::ErrorKind;
        loop {
            self.buf.clear();
            match self.inner.read_line(&mut self.buf) {
                Ok(0) => return None, // true EOF — port closed
                Err(e) if matches!(
                    e.kind(),
                    ErrorKind::TimedOut      // serial port read timeout (most common)
                    | ErrorKind::WouldBlock  // non-blocking would block
                    | ErrorKind::Interrupted // signal interrupted the syscall
                ) => continue,           // transient — retry immediately
                Err(_) => return None,   // fatal I/O error — stop
                Ok(_) => {}
            }
            let raw = self.buf.trim_end_matches(['\r', '\n']).to_string();
            if raw.is_empty() { continue; }
            let parsed = ParsedSentence::try_from(raw.as_str());
            return Some(SentenceLine { raw, parsed });
        }
    }
}

//! Convenience re-exports for glob import: `use dx_gp21_core::prelude::*`
pub use crate::{
    // State trait and feed function
    GnssStore, feed_sentence,
    // Shared primitive types
    GnssSystem, FixMode, FixQuality, AntennaStatus, SatInfo,
    NmeaTime, NmeaDate, DopValues,
    // Sentence data types (each lives in its own nmea/* file)
    GgaData, RmcData, GsaData, GsvData, VtgData, ZdaData, DhvData, GstData, TxtData,
    // Parsing
    ParsedSentence, ParseError, SentenceType, parse_sentence,
    // Commands
    CommandSink, ConstellationMask,
};
pub use crate::command::{BaudRate, UpdateRate, RestartMode, InfoField};

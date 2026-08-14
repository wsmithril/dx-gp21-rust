//! Convenience re-exports for glob import: `use dx_gp21_core::prelude::*`
pub use crate::command::{BaudRate, InfoField, RestartMode, UpdateRate};
pub use crate::{
    AntennaStatus,
    // Commands
    CommandSink,
    ConstellationMask,
    DhvData,
    DopValues,
    FixMode,
    FixQuality,
    // Sentence data types (each lives in its own nmea/* file)
    GgaData,
    // State trait and feed function
    GnssStore,
    // Shared primitive types
    GnssSystem,
    GsaData,
    GstData,
    GsvData,
    NmeaDate,
    NmeaTime,
    ParseError,
    // Parsing
    ParsedSentence,
    RmcData,
    SatInfo,
    SentenceType,
    TxtData,
    VtgData,
    ZdaData,
    feed_sentence,
    parse_sentence,
};

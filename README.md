# dx-gp21

Rust driver for the **DX-GP21-A** multi-constellation GNSS module by Shenzhen DX-Smart Technology.

> Module product page: [DX-GP21 GNSS Module](http://szdx-smart.com/cpzx/GNSSdingweimokuai/duomoduopin/198.html)

The module supports GPS / BeiDou / GLONASS / Galileo / QZSS, outputs NMEA 0183
sentences over UART, and accepts `$PCAS` proprietary commands for configuration.

---

## Workspace layout

```
dx-gp21-core          (no_std, no alloc)
│  NMEA sentence parsers, typed $PCAS command builders, ParsedSentence enum.
│  Core traits: GnssStore (state), CommandSink (commands), AsyncLineReader (async I/O).
│  Features: "async" (AsyncLineReader + run_with_reader), "defmt" (defmt::Format impls).
│
├── dx-gp21-embedded  (no_std, no alloc)
│      EmbeddedGnssState<N> and EmbeddedSession<W, N>.
│      Feature "async": EmbeddedSession::run<R: embedded_io_async::Read>() — device-agnostic
│      async read loop via the AsyncLineReader trait from core.
│
├── dx-gp21           (std)
│      SerialSession / FileSession — background std::thread reads NMEA sentences,
│      updates GnssState, exposes the GnssSession trait for shared state access.
│      SentenceReader<R: BufRead> — low-level sentence iterator (handles serial timeouts).
│
│      └── dx-gp21-monitor  (binary)
│             ratatui TUI with sky plot, satellite SNR table, live $PCAS command REPL,
│             file playback mode. Async event loop via tokio + crossterm EventStream.
│
└── dx-gp21-nrf52840  (no_std, nRF52840)
       DxGp21GnssModule — wraps Uarte<T>, optional power pin (W), optional 1PPS pin (P).
       Caller provides &'s Mutex<RefCell<GnssState<N>>> — no hidden singletons,
       multiple independent modules supported.
       See examples/embassy_async.rs for a complete Embassy application.
```

**Build targets:**

```bash
cargo build                          # host crates (default-members)
cargo test                           # host tests
cd dx-gp21-nrf52840 && cargo build   # ARM (uses .cargo/config.toml)
```

### Dependency graph

```
          dx-gp21-core
         /            \
dx-gp21-embedded    dx-gp21
        |              |
dx-gp21-nrf52840   dx-gp21-monitor
```

---

## Host usage — two APIs

| API | Level | Example |
|---|---|---|
| **Sentence API** — `SentenceReader<R>` | low-level — you own the loop | [`examples/sentence_api.rs`](dx-gp21/examples/sentence_api.rs) |
| **GnssSession API** — `SerialSession` / `FileSession` | high-level — background thread, poll state | [`examples/gnss_session_api.rs`](dx-gp21/examples/gnss_session_api.rs) |

### Sentence API — low-level

```bash
cargo run -p dx-gp21 --example sentence_api -- /dev/tty.usbserial-0001
cargo run -p dx-gp21 --example sentence_api -- dx-gp21/examples/sample.nmea  # no hardware
```

`SentenceReader<R>` is an iterator over `SentenceLine` values — raw NMEA text + parse result.
The library manages no state; you decide what to do with every sentence.

```rust
use dx_gp21::sentence_reader::SentenceReader;
use dx_gp21::{GnssState, ParsedSentence};

let reader = SentenceReader::new(BufReader::new(port));
let mut state = GnssState::default();

for line in reader {
    if !line.is_valid() { eprintln!("bad checksum: {}", line.raw); continue; }
    match line.parsed.unwrap() {
        ParsedSentence::Gga(gga) if gga.is_valid() => {
            println!("fix  lat={:.6} lon={:.6} alt={:.1}m", gga.lat, gga.lon, gga.alt_msl);
        }
        other => { state.update(other); }
    }
}
```

### GnssSession API — high-level

```bash
cargo run -p dx-gp21 --example gnss_session_api -- /dev/tty.usbserial-0001
cargo run -p dx-gp21 --example gnss_session_api -- --file dx-gp21/examples/sample.nmea
```

A background thread handles parsing; you poll state via convenience methods.

```rust
use std::{thread, time::Duration};
use dx_gp21::{GnssSession, SerialSession};

let session = SerialSession::open("/dev/tty.usbserial-0001", 115200)?;
loop {
    let state = session.state();    // MutexGuard<GnssState>
    if state.has_fix() {
        let (lat, lon) = state.position().unwrap();
        println!("{} | {:.6}°, {:.6}° | alt={:.1}m | sats={}/{}",
            state.utc_time().unwrap(), lat, lon,
            state.altitude_msl().unwrap(),
            state.sats_used_count(), state.sats_in_view_count());
    }
    drop(state);
    thread::sleep(Duration::from_secs(1));
}
```

---

## Embedded (`no_std`)

```rust
use dx_gp21_embedded::{EmbeddedSession, command::UpdateRate};

// Writer closure sends $PCAS commands to the module via UART TX.
let mut session = EmbeddedSession::new(|bytes| uart.write_all(bytes));

// Feed raw NMEA lines — call from your UART receive handler or main loop.
if let Some(sentence) = session.feed(line) {
    // sentence is Copy: state is already updated AND caller has the data
}

if session.state().has_fix() {
    let (lat, lon) = session.state().position().unwrap();
}

// All $PCAS command helpers are provided by CommandSink:
session.set_update_rate(UpdateRate::Hz5).ok();
session.save_config().ok();
```

### Async embedded

Enable the `async` feature and call `run` with any `embedded_io_async::Read` source.
The method delegates to `dx_gp21_core::run_with_reader` via the `AsyncLineReader` trait:

```rust
// With Embassy (DMA-backed UarteRx — yields per burst, not per byte):
session.run(uart_rx).await;   // never returns
```

---

## nRF52840 — Embassy async

See the complete working example: [`dx-gp21-nrf52840/examples/embassy_async.rs`](dx-gp21-nrf52840/examples/embassy_async.rs)

```bash
cd dx-gp21-nrf52840
cargo build --example embassy_async   # uses .cargo/config.toml → thumbv7em-none-eabihf
probe-rs run --chip nRF52840_xxAA target/thumbv7em-none-eabihf/debug/embassy_async
```

**Hardware connections (DX-GP21-A 6-pin connector silkscreen):**

| Pad | Signal | nRF52840 | Notes |
|---|---|---|---|
| **T** | UART TX → MCU | UART RX | NMEA sentences |
| **R** | UART RX ← MCU | UART TX | `$PCAS` commands |
| **P** | 1PPS output | GPIO input (optional) | Rising edge 1 Hz after fix |
| **W** | Power ON/OFF | GPIO output (optional) | Onboard pull-up → float/HIGH = on, LOW = shutdown |
| **V** | VCC | 3.6–6 V | Onboard LDO; supply must provide ≥ 100 mA |
| **G** | GND | GND | |

**Design highlights:**
- State storage is caller-owned (`&'s Mutex<RefCell<GnssState<N>>>`): declare one `static` per module for independent state + 1PPS callbacks.
- GNSS UART task yields **once per DMA burst** (not per byte) — executor stays responsive.
- 1PPS task uses `InputChannel::wait().await` — zero CPU between pulses.
- `defmt::Format` is implemented for all types when `dx-gp21-core` feature `"defmt"` is enabled.

---

## TUI monitor

```bash
cargo run -p dx-gp21-monitor -- --port /dev/tty.usbserial-0001
cargo run -p dx-gp21-monitor -- --file dx-gp21/examples/sample.nmea --delay 20
```

Features: sky plot (elevation/azimuth with constellation colors), satellite SNR bar chart,
live `$PCAS` command REPL with Tab autocomplete, NMEA log, fix status, DOP values.
Async event loop: `tokio` single-thread runtime + `crossterm::EventStream` (no busy-polling).

---

## Parsing primitives

`ParsedSentence` is `Copy` and supports `TryFrom<&[u8]>` / `TryFrom<&str>`:

```rust
use core::convert::TryFrom;
use dx_gp21_core::{feed_sentence, ParsedSentence};

// Parse one line directly — returns Ok(sentence) or Err(ParseError)
match ParsedSentence::try_from(b"$GNGGA,...*6E".as_ref())? {
    ParsedSentence::Gga(gga) => println!("{} alt={:.1}m", gga.time, gga.alt_msl),
    ParsedSentence::Rmc(rmc) => println!("{} {}", rmc.date, rmc.time),
    other => { /* other.kind() → SentenceType */ }
}

// Parse + update state in one call; Copy means state AND caller both get the data
if let Some(ParsedSentence::Gga(gga)) = feed_sentence(&mut state, line) {
    // state is updated AND gga is available here
}
```

### Key types

| Type | Crate | Notes |
|---|---|---|
| `GnssStore` | `dx-gp21-core` | Trait: state updates + accessor methods |
| `GnssSession` | `dx-gp21` | Trait: `state()`, `drain_sentences()`, `send_raw()` |
| `AsyncLineReader` | `dx-gp21-core` | Trait: async I/O abstraction for `run_with_reader` |
| `CommandSink` | `dx-gp21-core` | Trait: `send_raw()` + free `$PCAS` command defaults |
| `ParsedSentence` | `dx-gp21-core` | `Copy` enum; `TryFrom<&[u8]>` / `TryFrom<&str>` |
| `ConstellationMask` | `dx-gp21-core` | Bitmask with `\|`, `&`, `!`, `Display` |
| `SentenceReader<R>` | `dx-gp21` | `Iterator<Item = SentenceLine>`; handles serial timeouts |
| `EmbeddedSession<W, N>` | `dx-gp21-embedded` | State + writer; `CommandSink` + optional async `run` |
| `DxGp21GnssModule<T, N>` | `dx-gp21-nrf52840` | nRF52840 UART/GPIO driver; `CommandSink` |

---

## Features

| Crate | Feature | Enables |
|---|---|---|
| `dx-gp21-core` | `async` | `AsyncLineReader` trait + `run_with_reader` |
| `dx-gp21-core` | `defmt` | `defmt::Format` on all public types |
| `dx-gp21-embedded` | `async` | `EmbeddedSession::run` + `next_sentence` |
| `dx-gp21-embedded` | `defmt` | Propagates `dx-gp21-core/defmt` |
| `dx-gp21-nrf52840` | *(always)* | Enables `dx-gp21-embedded/async` + `dx-gp21-core/defmt` |

---

## License

MIT — see [LICENSE](LICENSE).

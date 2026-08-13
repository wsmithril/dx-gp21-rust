mod app;
mod ui;

use std::io;

use clap::Parser;
use crossterm::event::{Event, EventStream};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use futures_util::StreamExt;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::time::{interval, Duration};

use dx_gp21::GnssSession;
use app::App;

#[derive(Parser)]
#[command(name = "dx-gp21-monitor", about = "DX-GP21 GNSS Module Terminal Monitor")]
struct Args {
    /// Serial port path (e.g. /dev/tty.usbserial-0001). Mutually exclusive with --file.
    #[arg(short, long, conflicts_with = "file")]
    port: Option<String>,

    /// Replay a captured NMEA log file for testing (loops continuously).
    #[arg(short, long, conflicts_with = "port")]
    file: Option<String>,

    /// Baud rate (serial port mode only)
    #[arg(short, long, default_value = "115200")]
    baud: u32,

    /// Delay between lines in milliseconds (file mode only)
    #[arg(long, default_value = "20")]
    delay: u64,
}

// Single-threaded runtime: the TUI lives in one thread; background GNSS reading
// uses std::thread::spawn inside SerialSession / FileSession independently.
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    match (&args.port, &args.file) {
        (Some(port), _) => {
            let s = dx_gp21::SerialSession::open(port, args.baud)
                .map_err(|e| format!("Failed to open {port}: {e}"))?;
            run(s, port.clone()).await
        }
        (_, Some(file)) => {
            let s = dx_gp21::FileSession::open(file, args.delay)
                .map_err(|e| format!("Failed to open {file}: {e}"))?;
            run(s, format!("[file] {file}")).await
        }
        _ => Err("Provide --port <TTY> or --file <path>".into()),
    }
}

/// Async TUI event loop. Generic over the session type so no boxing is needed.
///
/// Uses `tokio::select!` to await either a render tick or the next keyboard/
/// resize event. Between ticks the executor is free — no busy-polling.
async fn run<S: GnssSession>(
    session: S,
    port_label: String,
) -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app    = App::new(session, port_label);
    let mut events = EventStream::new();
    let mut tick   = interval(Duration::from_millis(50));

    loop {
        tokio::select! {
            // ── Render tick ───────────────────────────────────────────────────
            // Drains the NMEA sentence channel and redraws at ~20 fps.
            // The async select yields the thread between ticks so the tokio
            // executor stays responsive without busy-polling.
            _ = tick.tick() => {
                app.tick();
                terminal.draw(|frame| ui::render(frame, &mut app))?;
            }

            // ── Keyboard / resize events ──────────────────────────────────────
            // EventStream suspends here until an event arrives — no poll loop.
            maybe = events.next() => {
                match maybe {
                    Some(Ok(Event::Key(key))) => {
                        if app.handle_key(key) { break; }
                    }
                    Some(Ok(Event::Resize(_, _))) => {
                        terminal.draw(|frame| ui::render(frame, &mut app))?;
                    }
                    None | Some(Err(_)) => break,
                    _ => {}
                }
            }
        }
    }

    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

//! KTE Terminal: the standalone reference terminal emulator.
//!
//! ```text
//! kte-term                         open a window running your shell
//! kte-term --snapshot out.png      render text to a PNG (no window)
//! ```
//!
//! Run `kte-term --help` for options.

mod app;
mod fonts;
mod input;
mod pty;
mod render;

use std::io::Read;
use std::process::ExitCode;

use kte_core::unicode::LegacyWidths;
use kte_core::{CellModel, Shaper, Terminal};

use crate::fonts::{Face, describe, find_khmer, find_latin};
use crate::render::{Frame, Overlay, Renderer, Sizing};

const HELP: &str = "\
kte-term: a terminal with first-class Khmer support

USAGE:
    kte-term [OPTIONS]

OPTIONS:
    --font-size <PX>      Latin font size in pixels [default: 15]
    --line-height <X>     Row height as a multiple of the font's line height [default: 1.7]
    --khmer-fill <X>      How much of its cells a Khmer consonant fills, 0.5-1.0 [default: 1.0]
    --model <MODEL>       Khmer cell model [default: legacy]
                            legacy   match the system wcwidth() so today's shells and
                                     editors line up (glibc rules on Linux, Apple's on macOS)
                            wide     KTE Width Spec layout, 2 cells per consonant (best
                                     spacing; needs apps that follow the spec)
                            compact  1 cell per consonant
                            legacy-glibc, legacy-apple  force a legacy variant
    --rows <N>            Rows [default: 24]
    --cols <N>            Columns [default: 80]
    --snapshot <FILE>     Render to a PNG instead of opening a window; reads the
                          text to show from --text, or stdin
    --text <TEXT>         Text for --snapshot (escape sequences allowed as \\x1b)
    --command <CMD>       With --snapshot: run CMD in a real pty and render its output
    --type <TEXT>         With --snapshot: also type TEXT into it (\\n = Enter)
    --wait-ms <MS>        With --command/--type: how long to run [default: 1500]
    -h, --help            Show this help

ENVIRONMENT:
    KTE_FONT              Path to the Latin (monospace) font
    KTE_KHMER_FONT        Path to the Khmer font
";

/// Command-line options.
pub(crate) struct Options {
    pub(crate) sizing: Sizing,
    pub(crate) model: CellModel,
    pub(crate) rows: usize,
    pub(crate) cols: usize,
    pub(crate) snapshot: Option<String>,
    pub(crate) text: Option<String>,
    pub(crate) command: Option<String>,
    pub(crate) typed: Option<String>,
    pub(crate) wait_ms: u64,
}

fn parse_args() -> Result<Options, String> {
    let mut o = Options {
        sizing: Sizing {
            font_px: 15.0,
            line_height: 1.7,
            khmer_fill: 1.0,
        },
        model: CellModel::Legacy(LegacyWidths::native()),
        rows: 24,
        cols: 80,
        snapshot: None,
        text: None,
        command: None,
        typed: None,
        wait_ms: 1500,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or(format!("{name} needs a value"));
        let num = |s: String, name: &str| {
            s.parse::<f32>()
                .map_err(|_| format!("{name}: not a number"))
        };
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{HELP}");
                std::process::exit(0);
            }
            "--font-size" => o.sizing.font_px = num(value(&arg)?, &arg)?,
            "--line-height" => o.sizing.line_height = num(value(&arg)?, &arg)?,
            "--khmer-fill" => o.sizing.khmer_fill = num(value(&arg)?, &arg)?.clamp(0.5, 1.0),
            "--model" => {
                o.model = match value(&arg)?.as_str() {
                    "wide" => CellModel::Wide,
                    "compact" => CellModel::Compact,
                    "legacy" => CellModel::Legacy(LegacyWidths::native()),
                    "legacy-glibc" => CellModel::Legacy(LegacyWidths::Glibc),
                    "legacy-apple" => CellModel::Legacy(LegacyWidths::Apple),
                    other => return Err(format!("unknown model {other}; see --help")),
                }
            }
            "--rows" => o.rows = num(value(&arg)?, &arg)? as usize,
            "--cols" => o.cols = num(value(&arg)?, &arg)? as usize,
            "--snapshot" => o.snapshot = Some(value(&arg)?),
            "--text" => o.text = Some(value(&arg)?),
            "--command" => o.command = Some(value(&arg)?),
            "--type" => o.typed = Some(value(&arg)?),
            "--wait-ms" => o.wait_ms = num(value(&arg)?, &arg)? as u64,
            other => return Err(format!("unknown option {other}; see --help")),
        }
    }
    Ok(o)
}

/// Loads fonts and builds a renderer, reporting which fonts were chosen.
pub(crate) fn build_renderer(o: &Options, scale: f32) -> Result<Renderer, String> {
    let latin_src = find_latin().ok_or("no monospace font found; set KTE_FONT")?;
    let khmer_src = find_khmer().ok_or("no Khmer font found; set KTE_KHMER_FONT")?;
    let latin = Face::load(&latin_src)?;
    let khmer = Shaper::new(fonts::read(&khmer_src)?, khmer_src.index)
        .map_err(|e| format!("{}: {e}", khmer_src.path.display()))?;
    eprintln!(
        "kte-term: fonts: {} + {}",
        describe(&latin_src),
        describe(&khmer_src)
    );
    let mut sizing = o.sizing;
    sizing.font_px *= scale;
    Ok(Renderer::new(latin, khmer, o.model, sizing))
}

/// Runs a command (or the default shell) in a real pty, optionally types
/// into it, and feeds everything it prints to `term` until `wait_ms` passes.
fn run_live(o: &Options, term: &mut Terminal) -> Result<(), String> {
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    let mut pty = pty::Pty::spawn(o.rows, o.cols, o.command.as_deref(), move |b| {
        let _ = tx.send(b.to_vec());
    })?;
    let start = Instant::now();
    let deadline = start + Duration::from_millis(o.wait_ms);
    let mut typed = o
        .typed
        .clone()
        .map(|t| t.replace("\\n", "\r").replace('\n', "\r"));
    loop {
        // Type once the shell has had time to print its prompt.
        if start.elapsed() > Duration::from_millis(o.wait_ms / 3) {
            if let Some(t) = typed.take() {
                for c in t.chars() {
                    pty.write(c.encode_utf8(&mut [0; 4]).as_bytes());
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        }
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        match rx.recv_timeout((deadline - now).min(Duration::from_millis(50))) {
            Ok(bytes) if bytes.is_empty() && typed.is_none() => break,
            Ok(bytes) => {
                term.feed(&bytes);
                pty.write(&term.take_responses());
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    Ok(())
}

fn snapshot(o: &Options, path: &str) -> Result<(), String> {
    let mut renderer = build_renderer(o, 1.0)?;
    let mut term = Terminal::new(o.rows, o.cols, o.model);
    if o.command.is_some() || o.typed.is_some() {
        run_live(o, &mut term)?;
    } else {
        let text = snapshot_text(o)?;
        term.feed(text.replace('\n', "\r\n").as_bytes());
    }
    let (w, h) = renderer.window_size(o.rows, o.cols);
    let mut frame = Frame::new(w, h);
    renderer.render(
        &term,
        &mut frame,
        Overlay {
            focused: true,
            ..Overlay::default()
        },
    );
    save_png(&frame, path)
}

fn snapshot_text(o: &Options) -> Result<String, String> {
    Ok(match &o.text {
        Some(t) => t.replace("\\x1b", "\x1b").replace("\\n", "\n"),
        None => {
            let mut s = String::new();
            std::io::stdin()
                .read_to_string(&mut s)
                .map_err(|e| e.to_string())?;
            s
        }
    })
}

fn save_png(frame: &Frame, path: &str) -> Result<(), String> {
    let mut pixmap = tiny_skia::Pixmap::new(frame.width, frame.height).ok_or("empty frame")?;
    for (dst, src) in pixmap.data_mut().chunks_exact_mut(4).zip(&frame.pixels) {
        dst.copy_from_slice(&[(src >> 16) as u8, (src >> 8) as u8, *src as u8, 255]);
    }
    pixmap.save_png(path).map_err(|e| e.to_string())
}

fn main() -> ExitCode {
    let result = parse_args().and_then(|o| match o.snapshot.clone() {
        Some(path) => snapshot(&o, &path),
        None => app::run(o),
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("kte-term: {e}");
            ExitCode::FAILURE
        }
    }
}

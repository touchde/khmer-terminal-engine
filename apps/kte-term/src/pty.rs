//! Runs the user's shell on a pseudo-terminal.

use std::io::{Read, Write};

use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};

/// The running shell: write keyboard input to it, resize it, and receive its
/// output through the `on_output` callback given to [`Pty::spawn`].
pub struct Pty {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    _child: Box<dyn portable_pty::Child + Send + Sync>,
}

fn size(rows: usize, cols: usize) -> PtySize {
    PtySize {
        rows: rows.min(u16::MAX as usize) as u16,
        cols: cols.min(u16::MAX as usize) as u16,
        pixel_width: 0,
        pixel_height: 0,
    }
}

impl Pty {
    /// Starts `command` (run by the system shell), or the user's default
    /// shell as a login shell when `None`.
    ///
    /// `on_output` runs on a background thread for every chunk of output;
    /// it receives an empty slice once when the shell exits.
    pub fn spawn(
        rows: usize,
        cols: usize,
        command: Option<&str>,
        mut on_output: impl FnMut(&[u8]) + Send + 'static,
    ) -> Result<Pty, String> {
        let pair = native_pty_system()
            .openpty(size(rows, cols))
            .map_err(|e| format!("cannot open a pty: {e}"))?;

        let mut cmd = match command {
            None => CommandBuilder::new_default_prog(),
            Some(c) if cfg!(windows) => {
                let mut cmd = CommandBuilder::new("cmd.exe");
                cmd.args(["/C", c]);
                cmd
            }
            Some(c) => {
                let mut cmd = CommandBuilder::new("/bin/sh");
                cmd.args(["-c", c]);
                cmd
            }
        };
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("TERM_PROGRAM", "kte-term");
        cmd.env("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION"));
        // Apps launched from a GUI often lack a locale; without UTF-8 the
        // shell would mangle Khmer input.
        let utf8 = |v: &str| v.to_ascii_uppercase().contains("UTF-8") || v.contains("utf8");
        let has_utf8 = ["LC_ALL", "LC_CTYPE", "LANG"]
            .iter()
            .any(|k| std::env::var(k).is_ok_and(|v| utf8(&v)));
        if !has_utf8 {
            cmd.env("LANG", "en_US.UTF-8");
        }
        if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
            cmd.cwd(home);
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("cannot start the shell: {e}"))?;
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
        let writer = pair.master.take_writer().map_err(|e| e.to_string())?;

        std::thread::Builder::new()
            .name("pty-reader".into())
            .spawn(move || {
                let mut buf = vec![0u8; 64 * 1024];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => on_output(&buf[..n]),
                    }
                }
                on_output(&[]);
            })
            .map_err(|e| e.to_string())?;

        Ok(Pty {
            master: pair.master,
            writer,
            _child: child,
        })
    }

    pub fn write(&mut self, bytes: &[u8]) {
        if !bytes.is_empty() {
            let _ = self.writer.write_all(bytes);
            let _ = self.writer.flush();
        }
    }

    pub fn resize(&self, rows: usize, cols: usize) {
        let _ = self.master.resize(size(rows, cols));
    }
}

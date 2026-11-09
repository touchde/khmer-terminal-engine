//! The window: event loop, drawing, keyboard, mouse selection, clipboard.

use std::num::NonZeroU32;
use std::rc::Rc;

use kte_core::Terminal;
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, Ime, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, ModifiersState};
use winit::window::{Window, WindowId};

use crate::Options;
use crate::input;
use crate::pty::Pty;
use crate::render::{Frame, Overlay, Renderer};

/// Events sent to the event loop from other threads.
#[derive(Debug)]
pub enum UserEvent {
    /// Output from the shell.
    Output(Vec<u8>),
    /// The shell exited.
    Exited,
}

struct Running {
    window: Rc<Window>,
    surface: softbuffer::Surface<Rc<Window>, Rc<Window>>,
    renderer: Renderer,
    term: Terminal,
    pty: Pty,
    frame: Frame,
    modifiers: ModifiersState,
    focused: bool,
    mouse: PhysicalPosition<f64>,
    /// Selection anchor while the left button is held.
    anchor: Option<(usize, usize)>,
    selection: Option<((usize, usize), (usize, usize))>,
    clipboard: Option<arboard::Clipboard>,
    title: String,
}

struct App {
    options: Options,
    proxy: EventLoopProxy<UserEvent>,
    running: Option<Running>,
    error: Option<String>,
}

pub fn run(options: Options) -> Result<(), String> {
    let event_loop = EventLoop::<UserEvent>::with_user_event()
        .build()
        .map_err(|e| e.to_string())?;
    let mut app = App {
        options,
        proxy: event_loop.create_proxy(),
        running: None,
        error: None,
    };
    event_loop.run_app(&mut app).map_err(|e| e.to_string())?;
    app.error.map_or(Ok(()), Err)
}

impl App {
    fn start(&mut self, event_loop: &ActiveEventLoop) -> Result<Running, String> {
        let attrs = Window::default_attributes()
            .with_title("kte-term")
            .with_visible(false);
        let window = Rc::new(event_loop.create_window(attrs).map_err(|e| e.to_string())?);
        let scale = window.scale_factor() as f32;
        let renderer = crate::build_renderer(&self.options, scale)?;

        let (rows, cols) = (self.options.rows, self.options.cols);
        let (w, h) = renderer.window_size(rows, cols);
        let _ = window.request_inner_size(PhysicalSize::new(w, h));
        window.set_ime_allowed(true);

        let context = softbuffer::Context::new(window.clone()).map_err(|e| e.to_string())?;
        let surface =
            softbuffer::Surface::new(&context, window.clone()).map_err(|e| e.to_string())?;

        let proxy = self.proxy.clone();
        let pty = Pty::spawn(rows, cols, None, move |bytes| {
            let event = if bytes.is_empty() {
                UserEvent::Exited
            } else {
                UserEvent::Output(bytes.to_vec())
            };
            let _ = proxy.send_event(event);
        })?;

        window.set_visible(true);
        Ok(Running {
            window,
            surface,
            renderer,
            term: Terminal::new(rows, cols, self.options.model),
            pty,
            frame: Frame::new(w, h),
            modifiers: ModifiersState::empty(),
            focused: true,
            mouse: PhysicalPosition::new(0.0, 0.0),
            anchor: None,
            selection: None,
            clipboard: arboard::Clipboard::new().ok(),
            title: String::new(),
        })
    }
}

impl Running {
    fn resize(&mut self, size: PhysicalSize<u32>) {
        let (w, h) = (size.width.max(1), size.height.max(1));
        if let (Some(nw), Some(nh)) = (NonZeroU32::new(w), NonZeroU32::new(h)) {
            let _ = self.surface.resize(nw, nh);
        }
        self.frame = Frame::new(w, h);
        let (rows, cols) = self.renderer.grid_size(w, h);
        if (rows, cols) != (self.term.rows(), self.term.cols()) {
            self.term.resize(rows, cols);
            self.pty.resize(rows, cols);
            self.selection = None;
        }
        self.window.request_redraw();
    }

    fn draw(&mut self) {
        // Highlight whole clusters, never part of one.
        let selection = self.selection.map(|(a, b)| {
            let start = (a.0, self.term.cluster_span(a.0, a.1).start);
            let end = (b.0, self.term.cluster_span(b.0, b.1).end - 1);
            (start, end)
        });
        self.renderer.render(
            &self.term,
            &mut self.frame,
            Overlay {
                selection,
                focused: self.focused,
            },
        );
        if let Ok(mut buffer) = self.surface.buffer_mut() {
            if buffer.len() == self.frame.pixels.len() {
                buffer.copy_from_slice(&self.frame.pixels);
                let _ = buffer.present();
            }
        }
    }

    fn output(&mut self, bytes: &[u8]) {
        self.term.feed(bytes);
        let replies = self.term.take_responses();
        self.pty.write(&replies);
        if self.term.title() != self.title {
            self.title = self.term.title().to_owned();
            let title = if self.title.is_empty() {
                "kte-term"
            } else {
                &self.title
            };
            self.window.set_title(title);
        }
        self.window.request_redraw();
    }

    fn copy(&mut self) {
        if let (Some((a, b)), Some(cb)) = (self.selection, self.clipboard.as_mut()) {
            let _ = cb.set_text(self.term.selection_text(a, b));
        }
    }

    fn paste(&mut self) {
        let Some(text) = self.clipboard.as_mut().and_then(|cb| cb.get_text().ok()) else {
            return;
        };
        let text = text.replace("\r\n", "\r").replace('\n', "\r");
        if self.term.bracketed_paste() {
            // Remove any end marker inside the text so it cannot end the paste early.
            let text = text.replace("\x1b[201~", "");
            self.pty
                .write(format!("\x1b[200~{text}\x1b[201~").as_bytes());
        } else {
            self.pty.write(text.as_bytes());
        }
    }

    /// Copy/paste shortcuts: Cmd+C/V on macOS, Ctrl+Shift+C/V elsewhere.
    fn shortcut(&mut self, key: &Key) -> bool {
        let m = self.modifiers;
        let chord = if cfg!(target_os = "macos") {
            m.super_key()
        } else {
            m.control_key() && m.shift_key()
        };
        let Key::Character(c) = key else { return false };
        match (chord, c.to_lowercase().as_str()) {
            (true, "c") => self.copy(),
            (true, "v") => self.paste(),
            _ => return false,
        }
        true
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.running.is_some() {
            return;
        }
        match self.start(event_loop) {
            Ok(running) => self.running = Some(running),
            Err(e) => {
                self.error = Some(e);
                event_loop.exit();
            }
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        let Some(r) = self.running.as_mut() else {
            return;
        };
        match event {
            UserEvent::Output(bytes) => r.output(&bytes),
            UserEvent::Exited => event_loop.exit(),
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(r) = self.running.as_mut() else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => r.resize(size),
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let mut sizing = self.options.sizing;
                sizing.font_px *= scale_factor as f32;
                r.renderer.resize_fonts(sizing);
                let size = r.window.inner_size();
                r.resize(size);
            }
            WindowEvent::RedrawRequested => r.draw(),
            WindowEvent::Focused(focused) => {
                r.focused = focused;
                r.window.request_redraw();
            }
            WindowEvent::ModifiersChanged(m) => r.modifiers = m.state(),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed && r.shortcut(&event.logical_key) {
                    return;
                }
                if let Some(bytes) = input::encode(&event, r.modifiers, r.term.app_cursor_keys()) {
                    r.selection = None;
                    r.pty.write(&bytes);
                }
            }
            WindowEvent::Ime(Ime::Commit(text)) => r.pty.write(text.as_bytes()),
            WindowEvent::CursorMoved { position, .. } => {
                r.mouse = position;
                if let Some(anchor) = r.anchor {
                    let at = r.renderer.cell_at(&r.term, position.x, position.y);
                    r.selection = Some(if at < anchor {
                        (at, anchor)
                    } else {
                        (anchor, at)
                    });
                    r.window.request_redraw();
                }
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => match state {
                ElementState::Pressed => {
                    r.anchor = Some(r.renderer.cell_at(&r.term, r.mouse.x, r.mouse.y));
                    r.selection = None;
                    r.window.request_redraw();
                }
                ElementState::Released => r.anchor = None,
            },
            _ => {}
        }
    }
}

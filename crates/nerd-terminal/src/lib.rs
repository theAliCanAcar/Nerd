//! Local PTY sessions and terminal grid snapshots.

mod input;

#[cfg(test)]
mod tests;

use std::path::Path;
use std::process::ExitStatus;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread;

use alacritty_terminal::Term;
use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, EventLoopSender, Msg};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::{Config, Osc52, TermMode, cell::Flags, color::Colors};
use alacritty_terminal::tty;
use alacritty_terminal::vte::ansi::Color as AnsiColor;
pub use alacritty_terminal::vte::ansi::CursorShape;
use anyhow::{Context, Result};

pub use input::{Key, Modifiers, MouseButton, MouseEvent};

type Terminal = Arc<FairMutex<Term<EventProxy>>>;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Color {
    #[default]
    Default,
    Indexed(u8),
    Rgb([u8; 3]),
}

impl Color {
    fn from_ansi(color: AnsiColor, colors: &Colors) -> Self {
        let (index, default) = match color {
            AnsiColor::Spec(rgb) => return Self::Rgb([rgb.r, rgb.g, rgb.b]),
            AnsiColor::Indexed(index) => (usize::from(index), Self::Indexed(index)),
            AnsiColor::Named(named) => {
                let color = if (named as usize) < 16 {
                    Self::Indexed(named as u8)
                } else {
                    Self::Default
                };
                (named as usize, color)
            }
        };
        colors[index].map_or(default, |rgb| Self::Rgb([rgb.r, rgb.g, rgb.b]))
    }
}

#[derive(Clone, Debug, Default)]
pub struct Cell {
    pub text: String,
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub inverse: bool,
    pub dim: bool,
    pub hidden: bool,
    pub strikeout: bool,
    pub wide: bool,
    pub spacer: bool,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub cols: usize,
    pub rows: usize,
    /// Cells in row-major order, including wide-character placeholders.
    pub cells: Vec<Cell>,
    pub cursor: Option<(usize, usize)>,
    pub cursor_shape: CursorShape,
    pub title: Option<String>,
    pub exited: bool,
    pub exit_code: Option<i32>,
    pub history_size: usize,
    pub scrollback_offset: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SearchMatch {
    pub scrollback_offset: usize,
    pub row: usize,
    pub col: usize,
    /// Width of the match in cells.
    pub len: usize,
}

/// Shared events are kept outside the terminal lock so the PTY worker can
/// answer queries as the emulator parses them.
struct Events {
    wake: Box<dyn Fn() + Send + Sync>,
    channel: Mutex<Option<EventLoopSender>>,
    status: Mutex<Status>,
    size: Mutex<WindowSize>,
    closing: AtomicBool,
}

#[derive(Default)]
struct Status {
    title: Option<String>,
    exit: Option<ExitStatus>,
    exited: bool,
}

#[derive(Clone)]
struct EventProxy(Arc<Events>);

impl EventListener for EventProxy {
    fn send_event(&self, event: Event) {
        match event {
            Event::PtyWrite(text) => self.respond(text),
            Event::TextAreaSizeRequest(format_response) => {
                let size = *self.0.size.lock().unwrap();
                self.respond(format_response(size));
            }
            Event::Title(title) => {
                let title = title
                    .chars()
                    .filter(|&c| {
                        !c.is_control()
                            && !matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
                    })
                    .take(128)
                    .collect();
                self.0.status.lock().unwrap().title = Some(title);
            }
            Event::ResetTitle => self.0.status.lock().unwrap().title = None,
            Event::ChildExit(status) => self.0.status.lock().unwrap().exit = Some(status),
            Event::Wakeup => self.wake(),
            _ => {}
        }
    }
}

impl EventProxy {
    fn respond(&self, text: String) {
        if let Some(channel) = self.0.channel.lock().unwrap().as_ref() {
            let _ = channel.send(Msg::Input(text.into_bytes().into()));
        }
    }

    fn wake(&self) {
        if !self.0.closing.load(Ordering::Relaxed) {
            (self.0.wake)();
        }
    }
}

pub struct Session {
    terminal: Terminal,
    channel: EventLoopSender,
    events: EventProxy,
}

impl Session {
    /// Start the OS default shell in a local PTY. The wake callback should
    /// schedule a repaint and return promptly. It runs on the PTY worker.
    pub fn spawn(
        cols: u16,
        rows: u16,
        cwd: Option<&Path>,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> Result<Self> {
        let options = tty::Options {
            working_directory: cwd.map(Path::to_owned),
            drain_on_exit: true,
            env: [
                ("TERM".into(), "xterm-256color".into()),
                ("COLORTERM".into(), "truecolor".into()),
                ("TERM_PROGRAM".into(), "Nerd".into()),
            ]
            .into(),
            ..Default::default()
        };
        Self::launch(cols, rows, options, wake)
    }

    fn launch(
        cols: u16,
        rows: u16,
        options: tty::Options,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> Result<Self> {
        let size = window_size(cols, rows, 0, 0);
        let events = EventProxy(Arc::new(Events {
            wake: Box::new(wake),
            channel: Mutex::default(),
            status: Mutex::default(),
            size: Mutex::new(size),
            closing: AtomicBool::new(false),
        }));
        let terminal = Arc::new(FairMutex::new(Term::new(
            Config {
                osc52: Osc52::Disabled,
                ..Config::default()
            },
            &Size(size),
            events.clone(),
        )));
        let pty = tty::new(&options, size, 0).context("Could not start the local shell")?;
        let worker = EventLoop::new(terminal.clone(), events.clone(), pty, true, false)
            .context("Could not start the terminal I/O worker")?;
        let channel = worker.channel();
        *events.0.channel.lock().unwrap() = Some(channel.clone());

        let observer = events.clone();
        thread::Builder::new()
            .name("nerd-reaper".into())
            .spawn(move || {
                // Mark the session ended even if the upstream event loop stopped
                // because of an I/O error without an explicit child-exit event.
                let finished = worker.spawn().join();
                observer.0.status.lock().unwrap().exited = true;
                *observer.0.channel.lock().unwrap() = None;
                observer.wake();
                // Some PTY implementations wait for process cleanup in Drop.
                // Keep this off the window thread.
                drop(finished);
            })
            .context("Could not start the terminal process observer")?;

        Ok(Self {
            terminal,
            channel,
            events,
        })
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        let size = *self.events.0.size.lock().unwrap();
        self.resize_with_cell_size(cols, rows, size.cell_width, size.cell_height)
    }

    pub fn resize_with_cell_size(
        &self,
        cols: u16,
        rows: u16,
        width: u16,
        height: u16,
    ) -> Result<()> {
        let size = window_size(cols, rows, width, height);
        {
            let mut term = self.terminal.lock();
            term.resize(Size(size));
            *self.events.0.size.lock().unwrap() = size;
        }
        if self.exited() {
            return Ok(());
        }
        self.channel
            .send(Msg::Resize(size))
            .context("Could not resize the terminal PTY")
    }

    pub fn write(&self, bytes: &[u8]) -> Result<()> {
        if bytes.is_empty() {
            return Ok(());
        }
        anyhow::ensure!(!self.exited(), "The shell has exited");
        self.channel
            .send(Msg::Input(bytes.to_vec().into()))
            .context("Could not send input to the shell")
    }

    pub fn exited(&self) -> bool {
        self.events.0.status.lock().unwrap().exited
    }

    pub fn paste(&self, text: &str) -> Result<()> {
        let modes = *self.terminal.lock().mode();
        self.scroll_to_bottom();
        self.write(&input::paste(text, modes))
    }

    pub fn key(&self, key: Key, modifiers: Modifiers) -> Result<()> {
        let modes = *self.terminal.lock().mode();
        self.scroll_to_bottom();
        self.write(&input::key(key, modifiers, modes))
    }

    pub fn mouse(
        &self,
        event: MouseEvent,
        row: usize,
        col: usize,
        modifiers: Modifiers,
    ) -> Result<bool> {
        let modes = {
            let term = self.terminal.lock();
            if term.grid().display_offset() != 0
                || row >= term.screen_lines()
                || col >= term.columns()
            {
                return Ok(false);
            }
            *term.mode()
        };
        let Some(bytes) = input::mouse(event, row, col, modifiers, modes) else {
            return Ok(false);
        };
        self.write(&bytes)?;
        Ok(true)
    }

    pub fn focus(&self, focused: bool) -> Result<()> {
        let enabled = self.terminal.lock().mode().contains(TermMode::FOCUS_IN_OUT);
        if enabled && !self.exited() {
            self.write(if focused { b"\x1b[I" } else { b"\x1b[O" })?;
        }
        Ok(())
    }

    /// Positive values scroll toward older output.
    pub fn scroll(&self, lines: i32) {
        let mut term = self.terminal.lock();
        let max = term.history_size() as i32;
        term.scroll_display(Scroll::Delta(lines.clamp(-max, max)));
    }

    pub fn scroll_to_bottom(&self) {
        self.terminal.lock().scroll_display(Scroll::Bottom);
    }

    /// Position the viewport at a specific offset from the live screen.
    pub fn seek(&self, offset: usize) {
        let mut term = self.terminal.lock();
        let delta = offset.min(term.history_size()) as i32 - term.grid().display_offset() as i32;
        term.scroll_display(Scroll::Delta(delta));
    }

    pub fn snapshot(&self) -> Snapshot {
        let term = self.terminal.lock();
        let content = term.renderable_content();
        let cols = term.columns();
        let rows = term.screen_lines();
        let cursor = content.cursor;
        let row = cursor.point.line.0 + content.display_offset as i32;
        let (title, exited, exit_code) = {
            let status = self.events.0.status.lock().unwrap();
            (
                status.title.clone(),
                status.exited,
                status.exit.and_then(|exit| exit.code()),
            )
        };
        let cursor_visible = !exited
            && content.display_offset == 0
            && cursor.shape != CursorShape::Hidden
            && (0..rows as i32).contains(&row);
        let cells = content
            .display_iter
            .map(|indexed| {
                let cell = indexed.cell;
                let flags = cell.flags;
                let spacer =
                    flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER);
                let mut text = String::new();
                if !spacer && cell.c != ' ' {
                    text.push(if cell.c == '\t' { ' ' } else { cell.c });
                }
                if let Some(marks) = cell.zerowidth().filter(|_| !spacer) {
                    if text.is_empty() {
                        text.push(' ');
                    }
                    text.extend(marks);
                }
                Cell {
                    text,
                    fg: Color::from_ansi(cell.fg, content.colors),
                    bg: Color::from_ansi(cell.bg, content.colors),
                    bold: flags.contains(Flags::BOLD),
                    italic: flags.contains(Flags::ITALIC),
                    underline: flags.intersects(Flags::ALL_UNDERLINES),
                    inverse: flags.contains(Flags::INVERSE),
                    dim: flags.contains(Flags::DIM),
                    hidden: flags.contains(Flags::HIDDEN),
                    strikeout: flags.contains(Flags::STRIKEOUT),
                    wide: flags.contains(Flags::WIDE_CHAR),
                    spacer,
                }
            })
            .collect();
        Snapshot {
            cols,
            rows,
            cells,
            cursor: cursor_visible.then_some((row as usize, cursor.point.column.0)),
            cursor_shape: cursor.shape,
            title,
            exited,
            exit_code,
            history_size: term.history_size(),
            scrollback_offset: content.display_offset,
        }
    }

    /// Copy an inclusive range in viewport coordinates (row, column).
    pub fn copy(&self, start: (usize, usize), end: (usize, usize)) -> String {
        let term = self.terminal.lock();
        let offset = term.grid().display_offset() as i32;
        let point = |(row, col): (usize, usize)| {
            Point::new(
                Line(row.min(term.screen_lines() - 1) as i32 - offset),
                Column(col.min(term.columns() - 1)),
            )
        };
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        term.bounds_to_string(point(start), point(end))
    }

    /// Find literal, case-sensitive matches within a displayed row, newest
    /// row first. Limits results to avoid allocating for unbounded queries.
    pub fn find(&self, query: &str) -> Vec<SearchMatch> {
        if query.is_empty() {
            return Vec::new();
        }
        let term = self.terminal.lock();
        let grid = term.grid();
        let mut matches = Vec::new();
        let mut text = String::new();
        let mut cells = Vec::new();
        for line in (grid.topmost_line().0..=grid.bottommost_line().0).rev() {
            text.clear();
            cells.clear();
            let row = &grid[Line(line)];
            for col in 0..grid.columns() {
                let cell = &row[Column(col)];
                if cell
                    .flags
                    .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
                {
                    continue;
                }
                let start = text.len();
                let hidden = cell.flags.contains(Flags::HIDDEN);
                text.push(if hidden || cell.c == '\t' {
                    ' '
                } else {
                    cell.c
                });
                if !hidden {
                    text.extend(cell.zerowidth().into_iter().flatten());
                }
                let width = if cell.flags.contains(Flags::WIDE_CHAR) {
                    2
                } else {
                    1
                };
                cells.push((start, col, width));
            }
            for (start, matched) in text.trim_end().match_indices(query) {
                let first = cells.partition_point(|(byte, _, _)| *byte <= start) - 1;
                let last = cells.partition_point(|(byte, _, _)| *byte < start + matched.len()) - 1;
                let col = cells[first].1;
                matches.push(SearchMatch {
                    scrollback_offset: (-line).max(0) as usize,
                    row: line.max(0) as usize,
                    col,
                    len: cells[last].1 + cells[last].2 - col,
                });
                if matches.len() == 1000 {
                    return matches;
                }
            }
        }
        matches
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.events.0.closing.store(true, Ordering::Relaxed);
        let _ = self.channel.send(Msg::Shutdown);
    }
}

fn window_size(cols: u16, rows: u16, cell_width: u16, cell_height: u16) -> WindowSize {
    let num_cols = cols.clamp(2, 512);
    let num_lines = rows.clamp(1, 512);
    WindowSize {
        num_cols,
        num_lines,
        cell_width: cell_width.min(u16::MAX / num_cols),
        cell_height: cell_height.min(u16::MAX / num_lines),
    }
}

struct Size(WindowSize);

impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        self.screen_lines()
    }

    fn screen_lines(&self) -> usize {
        usize::from(self.0.num_lines)
    }

    fn columns(&self) -> usize {
        usize::from(self.0.num_cols)
    }
}

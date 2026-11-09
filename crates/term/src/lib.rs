use std::borrow::Cow;
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, EventLoopSender, Msg};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::TermMode;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::tty::{self, Shell};
use alacritty_terminal::vte::ansi::{Color as AnsiColor, NamedColor};

#[derive(Clone)]
struct Proxy(Sender<Event>);

impl EventListener for Proxy {
    fn send_event(&self, event: Event) {
        let _ = self.0.send(event);
    }
}

#[derive(Clone, Copy)]
struct TermSize {
    cols: usize,
    rows: usize,
}

impl Dimensions for TermSize {
    fn total_lines(&self) -> usize {
        self.rows
    }

    fn screen_lines(&self) -> usize {
        self.rows
    }

    fn columns(&self) -> usize {
        self.cols
    }
}

#[derive(Clone, Debug, Default)]
pub struct SpawnOptions {
    pub shell: Option<(String, Vec<String>)>,
    pub cwd: Option<PathBuf>,
    pub cols: u16,
    pub rows: u16,
    pub env: Vec<(String, String)>,
    pub scrollback: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
    Default,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub inverse: bool,
    pub wide: bool,
    pub wide_spacer: bool,
    pub selected: bool,
}

impl Cell {
    fn blank() -> Self {
        Self {
            ch: ' ',
            fg: Color::Default,
            bg: Color::Default,
            bold: false,
            italic: false,
            underline: false,
            inverse: false,
            wide: false,
            wide_spacer: false,
            selected: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cursor {
    pub row: usize,
    pub col: usize,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub lines: Vec<Vec<Cell>>,
    pub cursor: Cursor,
}

impl Snapshot {
    pub fn text(&self) -> String {
        self.lines
            .iter()
            .map(|line| {
                line.iter()
                    .filter(|c| !c.wide_spacer)
                    .map(|c| c.ch)
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub struct Session {
    term: Arc<FairMutex<Term<Proxy>>>,
    sender: EventLoopSender,
    events: Receiver<Event>,
    pid: u32,
}

impl Session {
    pub fn spawn(options: &SpawnOptions) -> io::Result<Self> {
        let (tx, raw_events) = mpsc::channel();
        let (out_tx, events) = mpsc::channel();
        let size = TermSize {
            cols: options.cols as usize,
            rows: options.rows as usize,
        };
        let term = Arc::new(FairMutex::new(Term::new(
            scrollback_config(options.scrollback),
            &size,
            Proxy(tx.clone()),
        )));

        let pty_options = tty::Options {
            shell: options
                .shell
                .clone()
                .map(|(program, args)| Shell::new(program, args)),
            working_directory: options.cwd.clone(),
            drain_on_exit: true,
            env: HashMap::from_iter(
                [
                    ("TERM".to_string(), "xterm-256color".to_string()),
                    ("COLORTERM".to_string(), "truecolor".to_string()),
                ]
                .into_iter()
                .chain(options.env.iter().cloned()),
            ),
        };
        let pty = tty::new(&pty_options, window_size(options.cols, options.rows), 0)?;
        let pid = pty.child().id();

        let event_loop = EventLoop::new(term.clone(), Proxy(tx), pty, true, false)?;
        let sender = event_loop.channel();
        let _io_thread = event_loop.spawn();

        let responder = sender.clone();
        thread::spawn(move || {
            for event in raw_events {
                match event {
                    // Replies to queries from the program (cursor position, device attributes, ...)
                    // must be written back to the PTY, otherwise the program waits forever.
                    Event::PtyWrite(text) => {
                        let _ = responder.send(Msg::Input(Cow::Owned(text.into_bytes())));
                    }
                    other => {
                        if out_tx.send(other).is_err() {
                            break;
                        }
                    }
                }
            }
        });

        Ok(Self {
            term,
            sender,
            events,
            pid,
        })
    }

    pub fn write(&self, bytes: &[u8]) -> io::Result<()> {
        self.scroll_to_bottom();
        self.sender
            .send(Msg::Input(Cow::Owned(bytes.to_vec())))
            .map_err(|e| io::Error::other(e.to_string()))
    }

    pub fn resize(&self, cols: u16, rows: u16) -> io::Result<()> {
        self.term.lock().resize(TermSize {
            cols: cols as usize,
            rows: rows as usize,
        });
        self.sender
            .send(Msg::Resize(window_size(cols, rows)))
            .map_err(|e| io::Error::other(e.to_string()))
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn set_scrollback(&self, lines: usize) {
        self.term.lock().set_options(scrollback_config(lines));
    }

    pub fn scroll(&self, lines: i32) {
        self.term.lock().scroll_display(Scroll::Delta(lines));
    }

    pub fn scroll_page(&self, up: bool) {
        let scroll = if up { Scroll::PageUp } else { Scroll::PageDown };
        self.term.lock().scroll_display(scroll);
    }

    pub fn scroll_to_bottom(&self) {
        let mut term = self.term.lock();
        if term.grid().display_offset() != 0 {
            term.scroll_display(Scroll::Bottom);
        }
    }

    /// Mouse wheel: scrolls the history, or sends cursor keys when a full-screen
    /// program (such as `mc` or `vim`) is running on the alternate screen.
    pub fn wheel(&self, lines: i32, application_cursor: bool) {
        let alternate = self.term.lock().mode().contains(TermMode::ALT_SCREEN);
        if !alternate {
            self.scroll(lines);
            return;
        }
        let key = match (lines > 0, application_cursor) {
            (true, true) => "\x1bOA",
            (true, false) => "\x1b[A",
            (false, true) => "\x1bOB",
            (false, false) => "\x1b[B",
        };
        let count = lines.unsigned_abs() as usize;
        let _ = self.write(key.repeat(count).as_bytes());
    }

    pub fn start_selection(&self, row: usize, col: usize, right_half: bool) {
        let mut term = self.term.lock();
        let point = viewport_point(&term, row, col);
        term.selection = Some(Selection::new(
            SelectionType::Simple,
            point,
            side(right_half),
        ));
    }

    pub fn update_selection(&self, row: usize, col: usize, right_half: bool) {
        let mut term = self.term.lock();
        let point = viewport_point(&term, row, col);
        if let Some(selection) = term.selection.as_mut() {
            selection.update(point, side(right_half));
        }
    }

    pub fn clear_selection(&self) {
        self.term.lock().selection = None;
    }

    pub fn selection_text(&self) -> Option<String> {
        self.term.lock().selection_to_string()
    }

    pub fn application_cursor(&self) -> bool {
        self.term.lock().mode().contains(TermMode::APP_CURSOR)
    }

    pub fn events(&self) -> &Receiver<Event> {
        &self.events
    }

    pub fn snapshot(&self) -> Snapshot {
        let term = self.term.lock();
        let rows = term.screen_lines();
        let cols = term.columns();
        let content = term.renderable_content();
        let offset = content.display_offset as i32;
        let selection = content.selection;

        let mut lines = vec![vec![Cell::blank(); cols]; rows];
        for indexed in content.display_iter {
            let row = indexed.point.line.0 + offset;
            let col = indexed.point.column.0;
            if let Ok(row) = usize::try_from(row)
                && let Some(slot) = lines.get_mut(row).and_then(|line| line.get_mut(col))
            {
                *slot = convert_cell(indexed.cell);
                slot.selected = selection.is_some_and(|range| range.contains(indexed.point));
            }
        }

        let cursor_row = usize::try_from(content.cursor.point.line.0 + offset).unwrap_or(0);
        let cursor = Cursor {
            row: cursor_row,
            col: content.cursor.point.column.0,
        };
        Snapshot { lines, cursor }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.sender.send(Msg::Shutdown);
    }
}

fn viewport_point(term: &Term<Proxy>, row: usize, col: usize) -> Point {
    let offset = term.grid().display_offset() as i32;
    Point::new(Line(row as i32 - offset), Column(col))
}

fn scrollback_config(scrollback: usize) -> Config {
    Config {
        scrolling_history: scrollback,
        ..Config::default()
    }
}

fn side(right_half: bool) -> Side {
    if right_half { Side::Right } else { Side::Left }
}

fn window_size(cols: u16, rows: u16) -> WindowSize {
    WindowSize {
        num_cols: cols,
        num_lines: rows,
        cell_width: 1,
        cell_height: 1,
    }
}

fn convert_cell(cell: &alacritty_terminal::term::cell::Cell) -> Cell {
    Cell {
        ch: cell.c,
        fg: convert_color(cell.fg),
        bg: convert_color(cell.bg),
        bold: cell.flags.contains(Flags::BOLD),
        italic: cell.flags.contains(Flags::ITALIC),
        underline: cell.flags.contains(Flags::UNDERLINE),
        inverse: cell.flags.contains(Flags::INVERSE),
        wide: cell.flags.contains(Flags::WIDE_CHAR),
        wide_spacer: cell.flags.contains(Flags::WIDE_CHAR_SPACER),
        selected: false,
    }
}

fn convert_color(color: AnsiColor) -> Color {
    match color {
        AnsiColor::Spec(rgb) => Color::Rgb(rgb.r, rgb.g, rgb.b),
        AnsiColor::Indexed(index) => Color::Indexed(index),
        AnsiColor::Named(named) => match named {
            NamedColor::Foreground | NamedColor::Background | NamedColor::Cursor => Color::Default,
            other => {
                let index = other as u16;
                if index < 16 {
                    Color::Indexed(index as u8)
                } else {
                    Color::Default
                }
            }
        },
    }
}

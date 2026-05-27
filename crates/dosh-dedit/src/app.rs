use crate::buffer::TextBuffer;
use crate::completion::{collect_words, suggest};
use crate::diagnostics::compute_error_lines;
use crate::ui;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use crossterm::{ExecutableCommand, terminal};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::collections::BTreeSet;
use std::io;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Edit,
    Find,
}

pub struct EditorApp {
    pub buffer: TextBuffer,
    pub cursor_x: usize,
    pub cursor_y: usize,
    pub scroll_x: usize,
    pub scroll_y: usize,
    pub mode: Mode,
    pub find_query: String,
    pub suggestion: Option<String>,
    pub words: BTreeSet<String>,
    pub error_lines: BTreeSet<usize>,
    should_quit: bool,
}

impl EditorApp {
    fn new(buffer: TextBuffer) -> Self {
        let words = collect_words(&buffer.lines);
        let error_lines = compute_error_lines(&buffer.lines);
        Self {
            buffer,
            cursor_x: 0,
            cursor_y: 0,
            scroll_x: 0,
            scroll_y: 0,
            mode: Mode::Edit,
            find_query: String::new(),
            suggestion: None,
            words,
            error_lines,
            should_quit: false,
        }
    }

    pub fn mode_name(&self) -> &'static str {
        match self.mode {
            Mode::Edit => "EDIT",
            Mode::Find => "FIND",
        }
    }

    fn refresh_index(&mut self) {
        self.words = collect_words(&self.buffer.lines);
        self.error_lines = compute_error_lines(&self.buffer.lines);
        self.suggestion = self
            .current_word()
            .and_then(|prefix| suggest(&prefix, &self.words));
    }

    fn current_word(&self) -> Option<String> {
        let line = self.buffer.line(self.cursor_y);
        if self.cursor_x == 0 || self.cursor_x > line.len() {
            return None;
        }
        let bytes = line.as_bytes();
        let mut start = self.cursor_x.min(bytes.len());
        while start > 0 {
            let ch = bytes[start - 1] as char;
            if ch.is_ascii_alphanumeric() || ch == '_' {
                start -= 1;
            } else {
                break;
            }
        }
        let prefix = &line[start..self.cursor_x.min(line.len())];
        if prefix.is_empty() {
            None
        } else {
            Some(prefix.to_string())
        }
    }

    fn insert_char(&mut self, ch: char) {
        let x = self.cursor_x;
        let line = self.buffer.line_mut(self.cursor_y);
        if x <= line.len() {
            line.insert(x, ch);
            self.cursor_x += 1;
            self.buffer.dirty = true;
        }
        self.refresh_index();
    }

    fn backspace(&mut self) {
        if self.cursor_x > 0 {
            let x = self.cursor_x;
            let line = self.buffer.line_mut(self.cursor_y);
            if x <= line.len() {
                line.remove(x - 1);
                self.cursor_x -= 1;
                self.buffer.dirty = true;
            }
        } else if self.cursor_y > 0 {
            let cur = self.buffer.lines.remove(self.cursor_y);
            self.cursor_y -= 1;
            self.cursor_x = self.buffer.lines[self.cursor_y].len();
            self.buffer.lines[self.cursor_y].push_str(&cur);
            self.buffer.dirty = true;
        }
        self.refresh_index();
    }

    fn newline(&mut self) {
        let x = self.cursor_x;
        let right = {
            let line = self.buffer.line_mut(self.cursor_y);
            line.split_off(x.min(line.len()))
        };
        self.cursor_y += 1;
        self.cursor_x = 0;
        self.buffer.lines.insert(self.cursor_y, right);
        self.buffer.dirty = true;
        self.refresh_index();
    }

    fn apply_suggestion(&mut self) {
        let Some(s) = self.suggestion.clone() else {
            return;
        };
        let Some(prefix) = self.current_word() else {
            return;
        };
        if s == prefix {
            return;
        }
        let suffix = &s[prefix.len()..];
        for ch in suffix.chars() {
            self.insert_char(ch);
        }
    }

    fn find_next(&mut self) {
        if self.find_query.is_empty() {
            return;
        }
        for row in self.cursor_y..self.buffer.lines.len() {
            let start = if row == self.cursor_y {
                self.cursor_x
            } else {
                0
            };
            if let Some(idx) = self.buffer.lines[row][start..].find(&self.find_query) {
                self.cursor_y = row;
                self.cursor_x = start + idx;
                return;
            }
        }
    }

    fn move_cursor(&mut self, code: KeyCode) {
        match code {
            KeyCode::Up => {
                self.cursor_y = self.cursor_y.saturating_sub(1);
            }
            KeyCode::Down => {
                self.cursor_y = (self.cursor_y + 1).min(self.buffer.lines.len().saturating_sub(1));
            }
            KeyCode::Left => {
                self.cursor_x = self.cursor_x.saturating_sub(1);
            }
            KeyCode::Right => {
                self.cursor_x = (self.cursor_x + 1).min(self.buffer.line(self.cursor_y).len());
            }
            KeyCode::Home => self.cursor_x = 0,
            KeyCode::End => self.cursor_x = self.buffer.line(self.cursor_y).len(),
            _ => {}
        }
        let line_len = self.buffer.line(self.cursor_y).len();
        self.cursor_x = self.cursor_x.min(line_len);
    }

    fn ensure_visible(&mut self, width: u16, height: u16) {
        let text_h = height.saturating_sub(3) as usize;
        let text_w = width.saturating_sub(10) as usize;
        if self.cursor_y < self.scroll_y {
            self.scroll_y = self.cursor_y;
        }
        if self.cursor_y >= self.scroll_y + text_h {
            self.scroll_y = self.cursor_y.saturating_sub(text_h.saturating_sub(1));
        }
        if self.cursor_x < self.scroll_x {
            self.scroll_x = self.cursor_x;
        }
        if self.cursor_x >= self.scroll_x + text_w {
            self.scroll_x = self.cursor_x.saturating_sub(text_w.saturating_sub(1));
        }
    }

    fn on_key(&mut self, key: KeyEvent, width: u16, height: u16) -> Result<()> {
        if self.mode == Mode::Find {
            match key.code {
                KeyCode::Esc => self.mode = Mode::Edit,
                KeyCode::Enter => self.find_next(),
                KeyCode::Backspace => {
                    self.find_query.pop();
                }
                KeyCode::Char(c) => self.find_query.push(c),
                _ => {}
            }
            self.ensure_visible(width, height);
            return Ok(());
        }

        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), KeyModifiers::CONTROL) => self.should_quit = true,
            (KeyCode::Char('s'), KeyModifiers::CONTROL) => {
                self.buffer.save()?;
            }
            (KeyCode::Char('f'), KeyModifiers::CONTROL) => {
                self.mode = Mode::Find;
                self.find_query.clear();
            }
            (KeyCode::Tab, _) => self.apply_suggestion(),
            (KeyCode::Enter, _) => self.newline(),
            (KeyCode::Backspace, _) => self.backspace(),
            (KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right, _) => {
                self.move_cursor(key.code)
            }
            (KeyCode::Home | KeyCode::End, _) => self.move_cursor(key.code),
            (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => self.insert_char(c),
            _ => {}
        }
        self.refresh_index();
        self.ensure_visible(width, height);
        Ok(())
    }
}

pub fn run_editor(path: &Path) -> Result<()> {
    let buffer = TextBuffer::open(path)?;
    let mut app = EditorApp::new(buffer);
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    loop {
        terminal.draw(|f| ui::render(f, &app))?;
        if app.should_quit {
            break;
        }
        if event::poll(Duration::from_millis(120))?
            && let Event::Key(key) = event::read()?
            && matches!(key.kind, KeyEventKind::Press)
        {
            let area = terminal.size()?;
            app.on_key(key, area.width, area.height)?;
        }
    }

    disable_raw_mode()?;
    io::stdout().execute(terminal::LeaveAlternateScreen)?;
    Ok(())
}

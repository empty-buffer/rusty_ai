use crate::error::Result;
use ropey::Rope;

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    terminal::{enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use std::fs;
use std::io::{stdout, Write};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Editor {
    buffer: Rope,
    cursor_row: usize,
    cursor_col: usize,
    mode: Mode,
    file_path: Option<String>,
    modified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Select,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            buffer: Rope::new(),
            cursor_row: 0,
            cursor_col: 0,
            mode: Mode::Normal,
            file_path: None,
            modified: false,
        }
    }

    pub fn open_file<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let content = fs::read_to_string(path.as_ref())?;
        self.buffer = Rope::from_str(&content);
        self.file_path = Some(path.as_ref().to_string_lossy().to_string());
        self.cursor_row = 0;
        self.cursor_col = 0;
        self.modified = false;
        Ok(())
    }

    pub fn save_file(&mut self) -> Result<()> {
        if let Some(path) = &self.file_path {
            fs::write(path, self.buffer.to_string())?;
            self.modified = false;
            Ok(())
        } else {
            Err("No file path specified".into())
        }
    }

    pub fn handle_key(&mut self, key: KeyCode) -> Result<bool> {
        match self.mode {
            Mode::Normal => self.handle_normal_mode(key),
            Mode::Insert => self.handle_insert_mode(key),
            Mode::Select => self.handle_select_mode(key),
        }
    }

    fn handle_normal_mode(&mut self, key: KeyCode) -> Result<bool> {
        match key {
            // Navigation
            KeyCode::Up => self.move_cursor_up(),
            KeyCode::Down => self.move_cursor_down(),
            KeyCode::Left => self.move_cursor_left(),
            KeyCode::Right => self.move_cursor_right(),

            KeyCode::Char('k') => self.move_cursor_up(),
            KeyCode::Char('j') => self.move_cursor_down(),
            KeyCode::Char('h') => self.move_cursor_left(),
            KeyCode::Char('l') => self.move_cursor_right(),

            // Mode switching
            KeyCode::Char('i') => {
                self.mode = Mode::Insert;
                Ok(false)
            }
            KeyCode::Char('v') => {
                self.mode = Mode::Select;
                Ok(false)
            }

            // File operations
            KeyCode::Char('s') => {
                self.save_file()?;
                Ok(false)
            }

            // Quit
            KeyCode::Char('q') => Ok(true),

            _ => Ok(false),
        }
    }

    fn handle_insert_mode(&mut self, key: KeyCode) -> Result<bool> {
        match key {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                Ok(false)
            }
            KeyCode::Char(c) => {
                self.insert_char(c)?;
                Ok(false)
            }
            KeyCode::Enter => {
                self.insert_newline()?;
                Ok(false)
            }
            KeyCode::Backspace => {
                self.delete_char_before_cursor()?;
                Ok(false)
            }
            KeyCode::Delete => {
                self.delete_char_at_cursor()?;
                Ok(false)
            }
            KeyCode::Up => self.move_cursor_up(),
            KeyCode::Down => self.move_cursor_down(),
            KeyCode::Left => self.move_cursor_left(),
            KeyCode::Right => self.move_cursor_right(),
            _ => Ok(false),
        }
    }

    fn handle_select_mode(&mut self, key: KeyCode) -> Result<bool> {
        match key {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                Ok(false)
            }
            KeyCode::Up => self.move_cursor_up(),
            KeyCode::Down => self.move_cursor_down(),
            KeyCode::Left => self.move_cursor_left(),
            KeyCode::Right => self.move_cursor_right(),
            _ => Ok(false),
        }
    }

    fn move_cursor_up(&mut self) -> Result<bool> {
        if self.cursor_row > 0 {
            self.cursor_row -= 1;

            // Make sure cursor doesn't go beyond end of line
            let line = self.buffer.line(self.cursor_row);
            let line_len = line.len_chars().saturating_sub(1); // Account for newline
            if self.cursor_col > line_len {
                self.cursor_col = line_len;
            }
        }
        Ok(false)
    }

    fn move_cursor_down(&mut self) -> Result<bool> {
        if self.cursor_row < self.buffer.len_lines().saturating_sub(1) {
            self.cursor_row += 1;

            // Make sure cursor doesn't go beyond end of line
            let line = self.buffer.line(self.cursor_row);
            let line_len = line.len_chars().saturating_sub(1); // Account for newline
            if self.cursor_col > line_len {
                self.cursor_col = line_len;
            }
        }
        Ok(false)
    }

    fn move_cursor_left(&mut self) -> Result<bool> {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            // Move to end of previous line
            self.cursor_row -= 1;
            let line = self.buffer.line(self.cursor_row);
            self.cursor_col = line.len_chars().saturating_sub(1); // Account for newline
        }
        Ok(false)
    }

    fn move_cursor_right(&mut self) -> Result<bool> {
        let current_line = self.buffer.line(self.cursor_row);
        let line_len = current_line.len_chars().saturating_sub(1); // Account for newline

        if self.cursor_col < line_len {
            self.cursor_col += 1;
        } else if self.cursor_row < self.buffer.len_lines().saturating_sub(1) {
            // Move to beginning of next line
            self.cursor_row += 1;
            self.cursor_col = 0;
        }
        Ok(false)
    }

    fn insert_char(&mut self, c: char) -> Result<()> {
        let char_idx = self.get_char_idx();
        self.buffer.insert_char(char_idx, c);
        self.cursor_col += 1;
        self.modified = true;
        Ok(())
    }

    fn insert_newline(&mut self) -> Result<()> {
        let char_idx = self.get_char_idx();
        self.buffer.insert_char(char_idx, '\n');
        self.cursor_row += 1;
        self.cursor_col = 0;
        self.modified = true;
        Ok(())
    }

    fn delete_char_before_cursor(&mut self) -> Result<()> {
        let char_idx = self.get_char_idx();
        if char_idx > 0 {
            self.buffer.remove(char_idx - 1..char_idx);

            // Update cursor position
            if self.cursor_col > 0 {
                self.cursor_col -= 1;
            } else if self.cursor_row > 0 {
                self.cursor_row -= 1;
                let line = self.buffer.line(self.cursor_row);
                self.cursor_col = line.len_chars().saturating_sub(1);
            }

            self.modified = true;
        }
        Ok(())
    }

    fn delete_char_at_cursor(&mut self) -> Result<()> {
        let char_idx = self.get_char_idx();
        if char_idx < self.buffer.len_chars() {
            self.buffer.remove(char_idx..char_idx + 1);
            self.modified = true;
        }
        Ok(())
    }

    // Helper methods
    fn get_char_idx(&self) -> usize {
        let mut char_idx = 0;

        // Add up all characters in preceding lines
        for i in 0..self.cursor_row {
            char_idx += self.buffer.line(i).len_chars();
        }

        // Add column position
        char_idx += self.cursor_col;

        char_idx
    }

    // Rendering related methods
    pub fn get_content(&self) -> String {
        self.buffer.to_string()
    }

    pub fn get_cursor_position(&self) -> (usize, usize) {
        (self.cursor_row, self.cursor_col)
    }

    pub fn get_mode(&self) -> &Mode {
        &self.mode
    }

    pub fn is_modified(&self) -> bool {
        self.modified
    }

    pub fn get_file_name(&self) -> Option<&str> {
        self.file_path.as_deref()
    }
}

use crate::error::Result;
use once_cell::sync::Lazy;
use ropey::Rope;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};

use crate::chat::ChatContext;

use crate::syntax::{Style, SyntaxHighlighter};
use std::fs;
use std::io::{stdout, Write};
use std::ops::Range;
use std::path::Path;
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use tokio::runtime::Runtime;

static RUNTIME: Lazy<Runtime> =
    Lazy::new(|| Runtime::new().expect("Failed to create Tokio runtime"));

// #[derive(Debug, Clone)]
pub struct Editor {
    buffer: Rope,
    cursor_row: usize,
    cursor_col: usize,
    mode: Mode,
    file_path: Option<String>,
    modified: bool,
    chat_context: ChatContext,
    syntax_highlighter: Option<SyntaxHighlighter>,
    syntax_highlights: Vec<(Range<usize>, Style)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Select,
}

impl Editor {
    pub fn new() -> Self {
        let api = ChatContext::new().unwrap();
        let syntax_highlighter = SyntaxHighlighter::new().ok();

        Self {
            buffer: Rope::new(),
            cursor_row: 0,
            cursor_col: 0,
            mode: Mode::Normal,
            file_path: None,
            modified: false,
            syntax_highlighter,
            syntax_highlights: Vec::new(),
            chat_context: api,
        }
    }

    pub fn update_syntax_highlighting(&mut self) {
        if let Some(highlighter) = &self.syntax_highlighter {
            let language = self
                .file_path
                .as_ref()
                .and_then(|path| highlighter.detect_language(path));

            let highlights = highlighter.highlight_buffer(&self.buffer, language);
            self.syntax_highlights =
                highlighter.convert_highlights_to_char_ranges(&self.buffer, highlights);
        }
    }

    pub fn open_file<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let content = fs::read_to_string(path.as_ref())?;
        self.buffer = Rope::from_str(&content);
        self.file_path = Some(path.as_ref().to_string_lossy().to_string());
        self.cursor_row = 0;
        self.cursor_col = 0;
        self.modified = false;

        // Update syntax highlighting for the newly loaded file
        self.update_syntax_highlighting();

        Ok(())
    }

    pub fn get_style_at(&self, char_idx: usize) -> Style {
        for (range, style) in &self.syntax_highlights {
            if range.contains(&char_idx) {
                return *style;
            }
        }
        Style::Normal
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

    pub fn handle_key(&mut self, key: KeyCode, modifiers: KeyModifiers) -> Result<bool> {
        // Handle special key combinations first
        if modifiers.contains(KeyModifiers::ALT) {
            match key {
                KeyCode::Char('a') => {
                    self.send_to_antropic()?;
                    return Ok(false);
                }
                KeyCode::Char('l') => {
                    self.send_to_openai()?;
                    return Ok(false);
                }
                _ => {}
            }
        }

        // Handle regular keys based on mode
        match self.mode {
            Mode::Normal => self.handle_normal_mode(key),
            Mode::Insert => self.handle_insert_mode(key),
            Mode::Select => self.handle_select_mode(key),
        }
    }

    fn send_to_antropic(&mut self) -> Result<()> {
        self.send_to_api("antropic")
    }

    fn send_to_openai(&mut self) -> Result<()> {
        println!("Processing!");
        self.send_to_api("openai")
    }
    fn send_to_api(&mut self, api_name: &str) -> Result<()> {
        use std::fs::OpenOptions;
        use std::io::Write;

        let content = self.buffer.to_string();

        if content.is_empty() {
            println!("Cannot send empty buffer. Please write the question");
            // self.status_message = Some(format!("Cannot send empty buffer to {}", api_name));
            return Ok(());
        }

        // FIXME REDO LOGS
        let mut log = OpenOptions::new()
            .create(true)
            .append(true)
            .open("rusty_ai_error.log")
            .unwrap_or_else(|_| {
                eprintln!("Could not open log file");
                std::process::exit(1);
            });

        writeln!(log, "Sending to {}", api_name).unwrap_or_else(|e| {
            eprintln!("Could not write to log: {}", e);
        });

        // Create a channel to receive the result from the async operation
        let (sender, receiver) = mpsc::channel();

        let chat_context = self.chat_context.clone();

        let content_clone = content.clone();
        let api_name_clone = api_name.to_string();

        thread::spawn(move || {
            // Execute the async operation in the runtime
            let result = RUNTIME.block_on(async { chat_context.send_to_api(&content_clone).await });

            // Send the result back through the channel
            sender.send((api_name_clone, result)).unwrap_or_else(|e| {
                eprintln!("Failed to send result: {:?}", e);
            });
        });

        match receiver.recv() {
            Ok((api_name, result)) => {
                match result {
                    Ok(response) => {
                        // Add the response to the end of the buffer
                        let char_idx = self.buffer.len_chars();
                        let formatted_response = format!("\n\nAassistant\n {}", response);

                        self.buffer.insert(char_idx, &formatted_response);
                        self.update_syntax_highlighting();

                        // Update cursor position to the end
                        let new_lines = self.buffer.len_lines() - 1;
                        self.cursor_row = new_lines;
                        let last_line = self.buffer.line(new_lines);
                        self.cursor_col = last_line.len_chars().saturating_sub(1);

                        // self.status_message =
                        //     Some(format!("Response from {} added to buffer", api_name));
                        self.modified = true;
                    }
                    Err(e) => {
                        // self.status_message = Some(format!("Error from {}: {:?}", api_name, e));
                        writeln!(log, "API Error: {:?}", e).unwrap_or_default();
                        eprintln!("API Error: {:?}", e);
                    }
                }
            }
            Err(e) => {
                // self.status_message = Some(format!("Failed to receive response: {:?}", e));
                writeln!(log, "Channel Error: {:?}", e).unwrap_or_default();
            }
        }

        Ok(())
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

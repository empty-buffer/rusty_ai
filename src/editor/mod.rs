use crate::error::Result;
use once_cell::sync::Lazy;
use ropey::Rope;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};

use crate::chat::{ChatContext, Model};

use crate::syntax::{Style, SyntaxHighlighter};
use clipboard::{ClipboardContext, ClipboardProvider};

use crate::async_handler::{AsyncCommandHandler, EditorState};
use std::sync::{Arc, Mutex};

use std::fs;
use std::io::{stdout, Write};
use std::ops::Range;
use std::path::Path;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
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
    selection_start: Option<(usize, usize)>,
    selection_active: bool,

    // New fields for async support
    shared_state: Arc<Mutex<EditorState>>,
    async_handler: AsyncCommandHandler,

    // Track if we need to check for responses
    needs_response_check: bool,

    waiting_for_g_command: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestState {
    Idle,
    Proccessing,
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Select,
}

impl Editor {
    pub fn new() -> Self {
        let chat_context = ChatContext::new().unwrap();
        let syntax_highlighter = SyntaxHighlighter::new().ok();

        // Create shared state
        let shared_state = Arc::new(Mutex::new(EditorState::new()));

        // Create async handler
        let async_handler =
            AsyncCommandHandler::new(Arc::clone(&shared_state), chat_context.clone());

        Self {
            buffer: Rope::new(),
            cursor_row: 0,
            cursor_col: 0,
            mode: Mode::Normal,
            file_path: None,
            modified: false,
            syntax_highlighter,
            syntax_highlights: Vec::new(),
            chat_context,
            selection_start: None,
            selection_active: false,

            // New fields for async support
            shared_state,
            async_handler,

            // Track if we need to check for responses
            needs_response_check: false,

            waiting_for_g_command: false, // Initialize to false
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

    fn copy_selection_to_clipboard(&mut self) -> Result<()> {
        if let Some(text) = self.get_selected_text() {
            // Create a clipboard context
            let mut ctx: ClipboardContext = ClipboardProvider::new()
                .map_err(|e| format!("Failed to create clipboard context: {}", e))?;

            // Set the clipboard content
            ctx.set_contents(text.to_owned())
                .map_err(|e| format!("Failed to set clipboard contents: {}", e))?;

            // If in Select mode, exit to Normal mode
            if self.mode == Mode::Select {
                self.mode = Mode::Normal;
                self.selection_active = false;
                self.selection_start = None;
            }

            Ok(())
        } else {
            Err("No text selected".into())
        }
    }

    fn get_selected_text(&self) -> Option<String> {
        if !self.selection_active || self.selection_start.is_none() {
            return None;
        }

        let selection_range = self.get_selection_range()?;
        let start_idx = selection_range.start;
        let end_idx = selection_range.end;

        // Extract the text from the buffer
        Some(self.buffer.slice(start_idx..end_idx).to_string())
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

    // Get the current request state
    pub fn get_request_state(&self) -> RequestState {
        match self.shared_state.lock() {
            Ok(state) => state.request_state.clone(),
            Err(_) => RequestState::Error("Failed to lock state".to_string()),
        }
    }

    // New method to check and process any API responses
    pub fn check_api_responses(&mut self) {
        // Only check if we need to
        if !self.needs_response_check {
            return;
        }

        // Create a variable to store the response we'll process
        let response_to_process = {
            // Scope the lock to this block only
            if let Ok(mut state) = self.shared_state.lock() {
                // Take the response if available
                state.api_response.take()
            } else {
                None
            }
        }; // Lock is released here when the block ends

        // Try to lock the shared state
        if let Some(response) = response_to_process {
            // If there was an error, we've already set the request state
            if response.error.is_none() && !response.content.is_empty() {
                // Add the response to the end of the buffer
                let char_idx = self.buffer.len_chars();
                self.buffer.insert(char_idx, &response.content);

                // Now we can safely call this method since the lock is dropped
                self.update_syntax_highlighting();

                // Update cursor position to the end
                let new_lines = self.buffer.len_lines() - 1;
                self.cursor_row = new_lines;
                let last_line = self.buffer.line(new_lines);
                self.cursor_col = last_line.len_chars().saturating_sub(1);

                self.modified = true;
            }

            // We've processed the response, no need to check again
            self.needs_response_check = false;
        }
    }

    pub fn is_waiting_for_command(&self) -> bool {
        self.waiting_for_g_command
    }

    // fn select_to_end_of_line(&mut self) -> Result<bool> {
    //     // Set the starting point for the selection
    //     self.selection_start = Some((self.cursor_row, self.cursor_col));
    //     self.selection_active = true;

    //     // Move the cursor to the end of the line
    //     let line = self.buffer.line(self.cursor_row);
    //     let line_len = line.len_chars().saturating_sub(1); // Account for newline
    //     self.cursor_col = line_len;

    //     // Switch to select mode
    //     self.mode = Mode::Select;

    //     Ok(false)
    // }
    //
    fn move_to_end_of_line(&mut self) -> Result<bool> {
        // Move the cursor to the end of the line
        let line = self.buffer.line(self.cursor_row);
        let line_len = line.len_chars().saturating_sub(1); // Account for newline
        self.cursor_col = line_len;

        Ok(false)
    }

    fn move_to_start_of_line(&mut self) -> Result<bool> {
        // Move the cursor to the beginning of the line
        self.cursor_col = 0;

        Ok(false)
    }

    fn move_to_start_of_buffer(&mut self) -> Result<bool> {
        // Move cursor to the first position in the buffer
        self.cursor_row = 0;
        self.cursor_col = 0;

        Ok(false)
    }

    fn move_to_end_of_buffer(&mut self) -> Result<bool> {
        // Get the last line index
        let last_line_idx = self.buffer.len_lines().saturating_sub(1);

        // Move cursor to the last line
        self.cursor_row = last_line_idx;

        // Move cursor to the end of the last line
        let line = self.buffer.line(last_line_idx);
        let line_len = line.len_chars().saturating_sub(1);
        self.cursor_col = line_len;

        Ok(false)
    }

    // Add a new method to select the current line or expand selection
    fn select_current_line(&mut self) -> Result<bool> {
        // Check if we're already in select mode with an active selection
        if self.mode == Mode::Select && self.selection_active && self.selection_start.is_some() {
            // Get the current selection range to determine if it already covers full lines
            let selection_range = self.get_selection_range().unwrap_or(0..0);
            let start_row = self.buffer.char_to_line(selection_range.start);
            let end_row = self.buffer.char_to_line(selection_range.end);

            // If the selection already covers complete lines, extend to include one more line
            if end_row < self.buffer.len_lines() - 1 {
                // Move cursor to beginning of the next line
                self.cursor_row = end_row + 1;

                // If this is the last line, move to the end of it
                if self.cursor_row >= self.buffer.len_lines() - 1 {
                    let line = self.buffer.line(self.cursor_row);
                    self.cursor_col = line.len_chars().saturating_sub(1);
                } else {
                    // Otherwise, move to the end of this line
                    let line = self.buffer.line(self.cursor_row);
                    self.cursor_col = line.len_chars().saturating_sub(1);
                }
            }
        } else {
            // Start a new line selection
            // Move cursor to the beginning of the current line
            self.cursor_col = 0;

            // Set selection start
            self.selection_start = Some((self.cursor_row, 0));

            // Move cursor to the end of the line
            let line = self.buffer.line(self.cursor_row);
            let line_end = line.len_chars().saturating_sub(1);
            self.cursor_col = line_end;

            // Activate selection and enter select mode
            self.selection_active = true;
            self.mode = Mode::Select;
        }

        Ok(false)
    }

    pub fn get_selection_range(&self) -> Option<std::ops::Range<usize>> {
        if !self.selection_active || self.selection_start.is_none() {
            return None;
        }

        let (start_row, start_col) = self.selection_start.unwrap();
        let (end_row, end_col) = (self.cursor_row, self.cursor_col);

        let start_idx = self.char_idx_from_position(start_row, start_col);
        let end_idx = self.char_idx_from_position(end_row, end_col);

        // Make sure start_idx <= end_idx
        if start_idx <= end_idx {
            Some(start_idx..end_idx)
        } else {
            Some(end_idx..start_idx)
        }
    }

    pub fn is_position_selected(
        &self,
        row: usize,
        col: usize,
        selection_range: &Option<std::ops::Range<usize>>,
    ) -> bool {
        if let Some(range) = selection_range {
            let pos_idx = self.char_idx_from_position(row, col);
            range.contains(&pos_idx)
        } else {
            false
        }
    }

    pub fn get_style_for_position(&self, row: usize, col: usize) -> Style {
        // Check if the position is selected first
        if self.is_position_selected(row, col, &self.get_selection_range()) {
            return Style::Selection;
        }

        // Otherwise, get the syntax highlighting style
        let char_idx = self.char_idx_from_position(row, col);
        self.get_style_at(char_idx)
    }

    fn char_idx_from_position(&self, row: usize, col: usize) -> usize {
        if row >= self.buffer.len_lines() {
            return self.buffer.len_chars();
        }

        // Get the char index of the start of the line
        let line_start_idx = self.buffer.line_to_char(row);

        // Add the column, clamping to line length
        let line_len = self.buffer.line(row).len_chars();
        let clamped_col = col.min(line_len);

        line_start_idx + clamped_col
    }

    pub fn handle_key(&mut self, key: KeyCode, modifiers: KeyModifiers) -> Result<bool> {
        // Handle special key combinations first
        if modifiers.contains(KeyModifiers::ALT) {
            match key {
                KeyCode::Char('a') => {
                    self.send_to_antropic()?;
                    return Ok(false);
                }
                KeyCode::Char('o') => {
                    self.send_to_openai()?;
                    return Ok(false);
                }
                KeyCode::Char('l') => {
                    self.send_to_ollama()?;
                    return Ok(false);
                }
                KeyCode::Char('w') => {
                    // Save changes to file if there's a file path
                    // if self.file_path.is_some() {
                    //     self.save_file()?;
                    // }

                    // Clear the buffer
                    self.buffer = Rope::new();
                    self.cursor_row = 0;
                    self.cursor_col = 0;
                    self.modified = false;

                    if self.file_path.is_some() {
                        self.save_file()?;
                    }

                    // Update syntax highlighting for the empty buffer
                    self.update_syntax_highlighting();

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
        self.send_to_api(Model::ANTROPIC)
    }

    fn send_to_ollama(&mut self) -> Result<()> {
        // self.async_handler.request_ollama();
        // self.needs_response_check = true;
        self.send_to_api(Model::OLLAMA);
        Ok(())
    }

    fn send_to_openai(&mut self) -> Result<()> {
        // self.request_state = RequestState::Proccessing;
        self.send_to_api(Model::OPENAI)
    }

    fn send_to_api(&mut self, ai_model: Model) -> Result<()> {
        let content = self.buffer.to_string();

        // Delegate to the async handler
        self.async_handler.send_to_api(content, ai_model);

        // Set flag to check for responses
        self.needs_response_check = true;

        Ok(())
    }

    fn handle_normal_mode(&mut self, key: KeyCode) -> Result<bool> {
        if self.waiting_for_g_command {
            self.waiting_for_g_command = false; // Reset the flag

            // Handle the key after 'g'
            match key {
                KeyCode::Char('l') => return self.move_to_end_of_line(),
                KeyCode::Char('h') => return self.move_to_start_of_line(),
                KeyCode::Char('g') => return self.move_to_start_of_buffer(),
                KeyCode::Char('e') => return self.move_to_end_of_buffer(),
                // Add more 'g' commands here as needed
                _ => return Ok(false), // Ignore other keys
            }
        }

        match key {
            KeyCode::Char('x') => return self.select_current_line(),

            KeyCode::Char('g') => {
                self.waiting_for_g_command = true;
                return Ok(false);
            }

            // Mode switching
            KeyCode::Char('v') => {
                self.mode = Mode::Select;
                self.selection_start = Some((self.cursor_row, self.cursor_col));
                self.selection_active = true;
                Ok(false)
            }

            KeyCode::Char('y') => {
                // In normal mode, try to copy selection if it exists
                // This is useful if selection was made but user went back to normal mode
                if self.selection_active && self.selection_start.is_some() {
                    match self.copy_selection_to_clipboard() {
                        Ok(_) => {}
                        Err(e) => eprintln!("Clipboard error: {}", e),
                    }
                }
                Ok(false)
            }

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

            KeyCode::Char('d') => {
                self.delete_char_at_cursor()?;
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
        if self.waiting_for_g_command {
            self.waiting_for_g_command = false; // Reset the flag

            // Handle the key after 'g'
            match key {
                KeyCode::Char('l') => return self.move_to_end_of_line(),
                KeyCode::Char('h') => return self.move_to_start_of_line(),
                KeyCode::Char('g') => return self.move_to_start_of_buffer(),
                KeyCode::Char('e') => return self.move_to_end_of_buffer(),
                // Add more 'g' commands here as needed
                _ => return Ok(false), // Ignore other keys
            }
        }

        match key {
            KeyCode::Char('x') => return self.select_current_line(),

            KeyCode::Char('g') => {
                self.waiting_for_g_command = true;
                return Ok(false);
            }

            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.selection_active = false;
                self.selection_start = None;
                Ok(false)
            }

            KeyCode::Char('y') => {
                // Copy selection to clipboard and exit select mode
                match self.copy_selection_to_clipboard() {
                    Ok(_) => {}
                    Err(e) => eprintln!("Clipboard error: {}", e),
                }
                Ok(false)
            }
            KeyCode::Char('d') => {
                // Delete selection and exit select mode
                match self.delete_selection() {
                    Ok(_) => {}
                    Err(e) => eprintln!("Delete error: {}", e),
                }
                Ok(false)
            }

            KeyCode::Up => self.move_cursor_up(),
            KeyCode::Down => self.move_cursor_down(),
            KeyCode::Left => self.move_cursor_left(),
            KeyCode::Right => self.move_cursor_right(),
            KeyCode::Char('k') => self.move_cursor_up(),
            KeyCode::Char('j') => self.move_cursor_down(),
            KeyCode::Char('h') => self.move_cursor_left(),
            KeyCode::Char('l') => self.move_cursor_right(),
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

    fn delete_selection(&mut self) -> Result<()> {
        if let Some(selection_range) = self.get_selection_range() {
            let start_idx = selection_range.start;
            let end_idx = selection_range.end;

            // Remove the selected text from the buffer
            self.buffer.remove(start_idx..end_idx);

            // Update cursor position to the start of the selection
            // Convert the character index back to row and column
            let pos = self.position_from_char_idx(start_idx);
            self.cursor_row = pos.0;
            self.cursor_col = pos.1;

            // Exit select mode
            self.mode = Mode::Normal;
            self.selection_active = false;
            self.selection_start = None;

            // Mark the buffer as modified
            self.modified = true;

            // Update syntax highlighting
            self.update_syntax_highlighting();

            Ok(())
        } else {
            Err("No text selected".into())
        }
    }

    // Helper to convert character index back to (row, col) position
    fn position_from_char_idx(&self, char_idx: usize) -> (usize, usize) {
        if char_idx >= self.buffer.len_chars() {
            // If at the end of buffer, return the last position
            let last_line_idx = self.buffer.len_lines().saturating_sub(1);
            let last_line_len = if last_line_idx < self.buffer.len_lines() {
                self.buffer
                    .line(last_line_idx)
                    .len_chars()
                    .saturating_sub(1)
            } else {
                0
            };
            return (last_line_idx, last_line_len);
        }

        // Get the line that contains this character
        let line_idx = self.buffer.char_to_line(char_idx);

        // Get the start of this line in character indices
        let line_start_char = self.buffer.line_to_char(line_idx);

        // Calculate the column
        let col = char_idx - line_start_char;

        (line_idx, col)
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

    // pub fn get_request_state(&self) -> &RequestState {
    //     &self.request_state
    // }

    pub fn is_modified(&self) -> bool {
        self.modified
    }

    pub fn get_file_name(&self) -> Option<&str> {
        self.file_path.as_deref()
    }
}

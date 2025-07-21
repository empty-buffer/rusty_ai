use crate::error::{Error, Result};

pub mod filepicker;
pub mod menu;

use menu::MenuType;

use once_cell::sync::Lazy;
use ropey::Rope;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};

use crate::chat::{history::History, ChatContext, Model};

use crate::syntax::{Style, SyntaxHighlighter};
use clipboard::{ClipboardContext, ClipboardProvider};

use crate::async_handler::{AsyncCommandHandler, EditorState};
use std::num::IntErrorKind;
use std::sync::{Arc, Mutex};

use std::fs;
use std::io::{stdout, Write};
use std::ops::Range;
use std::path::Path;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tokio::runtime::Runtime;

use crate::syntax::cache::SyntaxCache;

// #[derive(Debug)]
pub struct Editor {
    buffer: Rope,
    cursor_row: usize,
    cursor_col: usize,
    mode: Mode,

    history: History,
    modified: bool,
    chat_context: ChatContext,

    syntax_cache: SyntaxCache,
    syntax_highlighter: Option<SyntaxHighlighter>,
    syntax_highlights: Vec<(Range<usize>, Style)>,

    selection_start: Option<(usize, usize)>,
    selection_active: bool,

    // New fields for async support
    shared_state: Arc<Mutex<EditorState>>,
    async_handler: AsyncCommandHandler,

    // Track if we need to check for responses
    needs_response_check: bool,

    show_help_menu: bool,
    pub menu_status: menu::CommandsMenu,
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
    pub fn new() -> Result<Self> {
        let current_file = History::new()?;

        let chat_context = ChatContext::new().unwrap();
        let syntax_highlighter = SyntaxHighlighter::new().ok();

        // Create shared state
        let shared_state = Arc::new(Mutex::new(EditorState::new()));

        // Create async handler
        let async_handler =
            AsyncCommandHandler::new(Arc::clone(&shared_state), chat_context.clone());

        let mut buffer = Rope::new();
        buffer.insert(0, "\n");
        Ok(Self {
            buffer,
            cursor_row: 0,
            cursor_col: 0,
            mode: Mode::Normal,
            history: current_file,
            modified: false,

            syntax_cache: SyntaxCache::new(),
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

            show_help_menu: false,
            menu_status: menu::CommandsMenu::default(),
        })
    }

    pub fn is_help_popup_active(&self) -> bool {
        self.show_help_menu
    }

    pub fn toggle_help_popup(&mut self) {
        self.show_help_menu = !self.show_help_menu;
    }

    pub fn get_help_content(&self) -> (Option<String>, Option<Vec<String>>) {
        self.menu_status.show_menu()
    }

    pub fn get_syntax_cache_dirty_lines(&self, real_line_number: usize) -> bool {
        self.syntax_cache.dirty_lines.contains(&real_line_number)
    }

    pub fn syntax_cache_is_line_cached(&self, real_line_number: usize) -> bool {
        self.syntax_cache.is_line_cached(real_line_number)
    }

    pub fn set_syntax_cache_line_styles(
        &mut self,
        real_line_number: usize,
        line_styles: Vec<Style>,
    ) {
        self.syntax_cache
            .cache_line_styles(real_line_number, line_styles);
    }

    pub fn get_syntax_cache_cached_style(
        &self,
        actual_row: usize,
        char_col: usize,
    ) -> Option<Style> {
        self.syntax_cache.get_cached_style(actual_row, char_col)
    }

    //this method to invalidate syntax highlighting after edits
    pub fn invalidate_syntax_at_line(&mut self, line: usize) {
        // Mark this line and subsequent lines as dirty
        let total_lines = self.buffer.len_lines();
        self.syntax_cache.mark_range_dirty(line, total_lines);
    }

    pub fn update_syntax_highlighting(&mut self) {
        // Check if we need a full update
        let current_len = self.buffer.len_chars();
        let need_full_update = current_len != self.syntax_cache.last_content_length;

        if need_full_update {
            // Clear cache for a full update
            self.syntax_cache.mark_all_dirty();
            self.syntax_cache.last_content_length = current_len;
        }

        if let Some(highlighter) = &self.syntax_highlighter {
            let language = highlighter.detect_language(&self.history.file_path);
            // .as_ref()
            // .and_then(|path| highlighter.detect_language(path));

            // Only perform full highlighting when necessary
            let highlights = highlighter.highlight_buffer(&self.buffer, language);
            self.syntax_highlights =
                highlighter.convert_highlights_to_char_ranges(&self.buffer, highlights);
        }
    }

    pub fn open_file(&mut self) -> Result<()> {
        // let file = match self.hisxfile_path.as_ref() {
        //     Some(f) => f,
        //     None => return Err(Error::Custom("editor: can't find file".to_string())),
        // };

        let content = fs::read_to_string(&self.history.file_path)?;
        self.buffer = Rope::from_str(&content);
        // self.file_path = Some(file.to_string());
        self.cursor_row = 0;
        self.cursor_col = 0;
        self.modified = false;

        // Update syntax highlighting for the newly loaded file
        self.update_syntax_highlighting();

        Ok(())
    }

    pub fn highlight_line(&mut self, line_number: usize) -> Vec<Style> {
        // Check if the line is already cached and not dirty
        if self.syntax_cache.is_line_cached(line_number) {
            return self
                .syntax_cache
                .line_styles
                .get(&line_number)
                .unwrap()
                .clone();
        }

        let line = self.buffer.line(line_number);
        let line_start_char = self.buffer.line_to_char(line_number);
        let line_end_char = line_start_char + line.len_chars();

        let mut line_styles = Vec::with_capacity(line.len_chars());

        // For each character in the line, determine its style
        for i in 0..line.len_chars() {
            let char_idx = line_start_char + i;

            // Default style
            let mut style = Style::Normal;

            // Check against syntax highlights
            if self.is_position_selected(line_number, i, &self.get_selection_range()) {
                style = Style::Selection;
            } else {
                // Check against syntax highlights
                for (range, highlight_style) in &self.syntax_highlights {
                    if range.contains(&char_idx) {
                        style = *highlight_style;
                        break;
                    }
                }
            }

            line_styles.push(style);
        }

        // Cache the result
        self.syntax_cache
            .cache_line_styles(line_number, line_styles.clone());

        line_styles
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

    pub fn get_style_at(&mut self, char_idx: usize) -> Style {
        // Convert char_idx to line and column
        let position = self.position_from_char_idx(char_idx);
        let (line, col) = position;

        // Check for selection first (direct check)
        if self.is_position_selected(line, col, &self.get_selection_range()) {
            return Style::Selection;
        }

        // Try to get from cache first
        if let Some(style) = self.syntax_cache.get_cached_style(line, col) {
            return style;
        }

        // If not in cache, highlight the whole line and get the style
        let line_styles = self.highlight_line(line);
        if col < line_styles.len() {
            return line_styles[col];
        }

        // Fallback
        Style::Normal
    }

    pub fn save_file(&mut self) -> Result<()> {
        self.history.save_file(self.buffer.to_string())?;
        self.modified = false;

        Ok(())
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
        self.menu_status.is_active_menu()
            // && !self.menu_status.is_active(MenuType::FilePicker)
            // && !self.menu_status.is_active(MenuType::FileSaveAs)
            && !self.menu_status.is_file_picker_active()
    }

    fn move_to_end_of_line(&mut self) -> Result<bool> {
        // Move the cursor to the end of the line
        let line = self.buffer.line(self.cursor_row);
        let line_len = line.len_chars().saturating_sub(1); // Account for newline
        self.cursor_col = line_len;

        self.clamp_cursor();

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
        let total_lines = self.buffer.len_lines();

        if total_lines == 0 {
            // Buffer is empty: place cursor at 0,0
            self.cursor_row = 0;
            self.cursor_col = 0;
            return Ok(false);
        }

        let last_line_idx = total_lines - 1;

        self.cursor_row = last_line_idx;

        let line = self.buffer.line(last_line_idx);

        // line.len_chars() is always at least 1 (newline at end)
        // saturate to 0 if len_chars() == 0 (shouldn't happen)
        let line_len = line.len_chars().saturating_sub(1);

        self.cursor_col = line_len;

        self.clamp_cursor();

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

    pub fn get_style_for_position(&mut self, row: usize, col: usize) -> Style {
        // Check if the position is selected first
        if self.is_position_selected(row, col, &self.get_selection_range()) {
            return Style::Selection;
        }

        // Otherwise, get the syntax highlighting style
        let char_idx = self.char_idx_from_position(row, col);
        self.get_style_at(char_idx)
    }

    pub fn char_idx_from_position(&self, row: usize, col: usize) -> usize {
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
        // Handle regular keys based on mode
        match self.mode {
            Mode::Normal => self.handle_normal_mode(key, modifiers),
            Mode::Insert => self.handle_insert_mode(key, modifiers),
            Mode::Select => self.handle_select_mode(key, modifiers),
        }
    }

    fn send_to_anthropic(&mut self) -> Result<()> {
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

    fn handle_normal_mode(&mut self, key: KeyCode, modifiers: KeyModifiers) -> Result<bool> {
        if (modifiers.contains(KeyModifiers::ALT) && key == KeyCode::Char('v'))
            || (modifiers.is_empty() && key == KeyCode::Char('p'))
        {
            match self.paste_from_clipboard() {
                Ok(_) => return Ok(false),
                Err(e) => eprintln!("Paste error: {}", e),
            }
        }

        if self.menu_status.file_picker_state(filepicker::Action::Save) {
            match key {
                KeyCode::Char(c) => {
                    // Insert character at cursor position
                    self.menu_status.file_picker.insert_char(c);

                    return Ok(false);
                }
                KeyCode::Backspace => {
                    self.menu_status.file_picker.delete_previous_char();

                    return Ok(false);
                }
                KeyCode::Delete => {
                    self.menu_status.file_picker.delete_current_char();

                    return Ok(false);
                }
                KeyCode::Left => {
                    self.menu_status.file_picker.move_cursor_pos_left();

                    return Ok(false);
                }
                KeyCode::Right => {
                    self.menu_status.file_picker.move_cursor_pos_right();

                    return Ok(false);
                }
                KeyCode::Enter => {
                    let filename = self.menu_status.file_picker.get_input();
                    if !filename.is_empty() {
                        // Save to file
                        let content = self.buffer.to_string();
                        self.history.save_to_file(filename.to_string(), content)?;

                        let content = self.history.current_file_content()?;

                        self.buffer = Rope::from_str(&content);
                        self.cursor_row = 0;
                        self.cursor_col = 0;
                        self.modified = false;

                        // Update syntax highlighting
                        self.update_syntax_highlighting();

                        self.modified = false;
                    }

                    self.menu_status.reset();
                    return Ok(false);
                }
                KeyCode::Esc => {
                    self.menu_status.reset();
                    return Ok(false);
                }
                _ => return Ok(false),
            }
        }

        if self.menu_status.file_picker_state(filepicker::Action::Load) {
            match key {
                KeyCode::Up => {
                    self.menu_status.file_picker.move_file_picker_up();
                    return Ok(false);
                }
                KeyCode::Down => {
                    self.menu_status.file_picker.move_file_picker_down();
                    return Ok(false);
                }
                KeyCode::Enter => {
                    if let Some(selected_file) = self.menu_status.file_picker.get_selected_file() {
                        // load the selected file into editor's buffer
                        let content = self.history.load_file(selected_file.to_string())?;

                        self.buffer = Rope::from_str(&content);
                        self.cursor_row = 0;
                        self.cursor_col = 0;
                        self.modified = false;

                        // Update file path in history or state if relevant
                        self.history.file_path = selected_file.to_string();

                        // Update syntax highlighting
                        self.update_syntax_highlighting();
                        // self.history.load_file(selected_file.to_string())?;
                        self.menu_status.reset(); // close popup
                    }
                    return Ok(false);
                }
                KeyCode::Esc => {
                    self.menu_status.reset(); // close popup
                    return Ok(false);
                }
                _ => {
                    // Ignore other keys when file picker active
                    return Ok(false);
                }
            }
        }

        // Handle the key 'Go To (g)' menu
        if self.menu_status.is_active(MenuType::GoTo) {
            self.menu_status.reset(); // Reset the flag

            match key {
                KeyCode::Char('l') => return self.move_to_end_of_line(),
                KeyCode::Char('h') => return self.move_to_start_of_line(),
                KeyCode::Char('g') => return self.move_to_start_of_buffer(),
                KeyCode::Char('e') => return self.move_to_end_of_buffer(),
                _ => return Ok(false),
            }
        }

        if self.menu_status.is_active(MenuType::AI) {
            self.menu_status.reset(); // Reset the flag

            match key {
                KeyCode::Char('a') => {
                    self.send_to_anthropic()?;
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
                _ => return Ok(false),
            }
        }

        // Handle the key 'File (:)' menu
        if self.menu_status.is_active(MenuType::File) {
            self.menu_status.reset();
            match key {
                KeyCode::Char('w') => {
                    self.buffer = Rope::new();
                    self.cursor_row = 0;
                    self.cursor_col = 0;
                    self.modified = false;

                    // if self.file_path.is_some() {
                    self.save_file()?;
                    // }

                    // Update syntax highlighting for the empty buffer
                    self.update_syntax_highlighting();

                    return Ok(false);
                }

                KeyCode::Char('s') => {
                    self.save_file()?;
                    return Ok(false);
                }

                KeyCode::Char('S') => {
                    self.menu_status.file_picker.init_file_save_as();
                    return Ok(false);
                }

                KeyCode::Char('l') => {
                    // self.menu_status.set_active_menu(MenuType::FilePicker);
                    self.menu_status.file_picker.init_file_picker()?;
                    return Ok(false);
                }

                KeyCode::Char('q') => return Ok(true),

                _ => return Ok(false),
            }
        }
        match key {
            KeyCode::Char('x') => return self.select_current_line(),

            KeyCode::Char('g') => {
                self.menu_status.set_active_menu(MenuType::GoTo);
                return Ok(false);
            }

            KeyCode::Char(' ') => {
                self.menu_status.set_active_menu(MenuType::File);
                return Ok(false);
            }

            KeyCode::Char('"') => {
                self.menu_status.set_active_menu(MenuType::AI);
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

                // println!("{}", self.buffer.len_lines());

                if self.buffer.len_lines() == 1 && self.buffer.len_chars() == 0 {
                    self.buffer.insert(0, "\n");
                    self.cursor_row = 0;
                    self.cursor_col = 0;
                }

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
            // KeyCode::Char('q') => Ok(true),
            _ => {
                self.menu_status.reset();
                Ok(false)
            }
        }
    }

    fn handle_insert_mode(&mut self, key: KeyCode, modifiers: KeyModifiers) -> Result<bool> {
        if modifiers.contains(KeyModifiers::META) && key == KeyCode::Char('v') {
            match self.paste_from_clipboard() {
                Ok(_) => return Ok(false),
                Err(e) => eprintln!("Paste error: {}", e),
            }
        }

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
            _ => {
                self.menu_status.reset();
                Ok(false)
            }
        }
    }

    fn handle_select_mode(&mut self, key: KeyCode, modifiers: KeyModifiers) -> Result<bool> {
        if self.menu_status.is_active(MenuType::GoTo) {
            self.menu_status.reset();

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

            // Set GoTo Menu Is Active
            KeyCode::Char('g') => {
                self.menu_status.set_active_menu(MenuType::GoTo);
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
            _ => {
                self.menu_status.reset();
                Ok(false)
            }
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
        // Get the actual number of lines in the buffer
        let total_lines = self.buffer.len_lines();

        let last_line_index = if total_lines > 0 { total_lines - 1 } else { 0 };

        // Only move down if we're not already at the last line
        if self.cursor_row < last_line_index {
            self.cursor_row += 1;

            // Make sure cursor doesn't go beyond end of line
            let line = self.buffer.line(self.cursor_row);
            let line_len = if line.len_chars() > 0 {
                line.len_chars() - 1 // Account for newline
            } else {
                0 // Handle empty lines
            };

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

        let mut line_len: usize;

        if self.mode == Mode::Insert {
            line_len = current_line.len_chars();
        } else {
            line_len = current_line.len_chars().saturating_sub(1); // Account for newline
        }

        if self.cursor_col < line_len {
            self.cursor_col += 1;
        } else if self.cursor_row < self.buffer.len_lines().saturating_sub(1) {
            // Move to beginning of next line
            let total_lines = self.buffer.len_lines();

            let last_line_index = if total_lines > 0 { total_lines - 1 } else { 0 };

            if self.cursor_row < last_line_index {
                self.cursor_row += 1;
                self.cursor_col = 0;

                // Make sure cursor doesn't go beyond end of line
                let line = self.buffer.line(self.cursor_row);
                let line_len = if line.len_chars() > 0 {
                    line.len_chars() - 1 // Account for newline
                } else {
                    0 // Handle empty lines
                };

                if self.cursor_col > line_len {
                    self.cursor_col = line_len;
                }
            }

            // self.cursor_row += 1;
            // self.cursor_col = 0;
        }
        Ok(false)
    }

    fn insert_char(&mut self, c: char) -> Result<()> {
        let char_idx = self.get_char_idx();
        self.buffer.insert_char(char_idx, c);
        self.cursor_col += 1;
        self.modified = true;

        self.invalidate_syntax_at_line(self.cursor_row);

        Ok(())
    }

    fn insert_newline(&mut self) -> Result<()> {
        let char_idx = self.get_char_idx();

        self.buffer.insert_char(char_idx, '\n');
        self.cursor_row += 1;
        self.cursor_col = 0;

        self.modified = true;

        self.invalidate_syntax_at_line(self.cursor_row - 1);

        Ok(())
    }

    fn delete_char_before_cursor(&mut self) -> Result<()> {
        let char_idx = self.get_char_idx();
        if char_idx > 0 {
            // Get the current line before deletion
            let current_line = self.cursor_row;

            self.buffer.remove(char_idx - 1..char_idx);

            // Update cursor position
            if self.cursor_col > 0 {
                self.cursor_col -= 1;
            } else if self.cursor_row > 0 {
                self.cursor_row -= 1;
                let line = self.buffer.line(self.cursor_row);
                self.cursor_col = line.len_chars();
            }

            self.modified = true;

            // Invalidate syntax highlighting for affected lines
            self.invalidate_syntax_at_line(current_line.saturating_sub(1));
        }
        Ok(())
    }

    fn delete_char_at_cursor(&mut self) -> Result<()> {
        let char_idx = self.get_char_idx();
        if char_idx < self.buffer.len_chars() {
            let current_line = self.cursor_row;

            // Delete the character
            self.buffer.remove(char_idx..char_idx + 1);
            self.modified = true;

            // Check if we need to update cursor position
            if self.cursor_row < self.buffer.len_lines() {
                let new_line_len = self.buffer.line(self.cursor_row).len_chars();

                // If we're at the end of an empty line (except the newline character)
                // and it's not the only line, move up to the previous line
                if new_line_len <= 1 && self.cursor_col == 0 && self.cursor_row > 0 {
                    self.cursor_row -= 1;
                    // Move to the end of the previous line
                    let prev_line_len = self.buffer.line(self.cursor_row).len_chars();
                    self.cursor_col = prev_line_len.saturating_sub(1);
                }
                // Otherwise adjust cursor if it's beyond the new line length
                else if self.cursor_col >= new_line_len {
                    self.cursor_col = new_line_len.saturating_sub(1);
                }
            }

            // Invalidate syntax highlighting
            self.invalidate_syntax_at_line(current_line);
        }
        Ok(())
    }

    fn get_char_idx(&self) -> usize {
        // Get the character index at the beginning of the cursor row
        let line_start_char = self.buffer.line_to_char(self.cursor_row);

        // Add column position
        let char_idx = line_start_char + self.cursor_col;

        char_idx
    }

    // Helper methods
    // fn get_char_idx(&self) -> usize {
    //     let mut char_idx = 0;

    //     // Add up all characters in preceding lines
    //     for i in 0..self.cursor_row {
    //         char_idx += self.buffer.line(i).len_chars();
    //     }

    //     // Add column position
    //     char_idx += self.cursor_col;

    //     char_idx
    // }

    fn delete_selection(&mut self) -> Result<()> {
        if let Some(selection_range) = self.get_selection_range() {
            let start_idx = selection_range.start;
            let end_idx = selection_range.end;

            // Get line numbers affected by the deletion
            let start_line = self.buffer.char_to_line(start_idx);
            let end_line = self.buffer.char_to_line(end_idx);

            // Remove the selected text from the buffer
            self.buffer.remove(start_idx..end_idx);

            // Update cursor position to the start of the selection
            let pos = self.position_from_char_idx(start_idx);
            self.cursor_row = pos.0;
            self.cursor_col = pos.1;

            // Exit select mode
            self.mode = Mode::Normal;
            self.selection_active = false;
            self.selection_start = None;

            // Mark the buffer as modified
            self.modified = true;

            // Invalidate syntax highlighting from start_line onwards
            self.invalidate_syntax_at_line(start_line);

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
        Some(self.history.file_path.as_str())
    }

    fn paste_from_clipboard(&mut self) -> Result<()> {
        // Create a clipboard context
        let mut ctx: ClipboardContext = ClipboardProvider::new()
            .map_err(|e| format!("Failed to create clipboard context: {}", e))?;

        // Get the clipboard content
        let content = ctx
            .get_contents()
            .map_err(|e| format!("Failed to get clipboard contents: {}", e))?;

        if content.is_empty() {
            return Ok(());
        }

        // Get current character index
        let char_idx = self.get_char_idx();

        // Get current position before insertion
        let current_row = self.cursor_row;

        // Insert the content
        self.buffer.insert(char_idx, &content);

        // Update cursor position by counting newlines in pasted content
        let new_position = self.position_from_char_idx(char_idx + content.len());
        self.cursor_row = new_position.0;
        self.cursor_col = new_position.1;

        // Mark as modified
        self.modified = true;

        // Force a full refresh of syntax highlighting
        self.syntax_cache.mark_all_dirty();
        self.update_syntax_highlighting();

        self.refresh_display();

        Ok(())
    }

    pub fn refresh_display(&mut self) {
        // Force a complete refresh of the editor state
        self.syntax_cache.mark_all_dirty();
        self.update_syntax_highlighting();
    }

    fn clamp_cursor(&mut self) {
        let total_lines = self.buffer.len_lines();
        if total_lines == 0 {
            self.cursor_row = 0;
            self.cursor_col = 0;
            return;
        }

        if self.cursor_row >= total_lines {
            self.cursor_row = total_lines - 1;
        }

        let line_len = self
            .buffer
            .line(self.cursor_row)
            .len_chars()
            .saturating_sub(1);

        if self.cursor_col > line_len {
            self.cursor_col = line_len;
        }
    }
}

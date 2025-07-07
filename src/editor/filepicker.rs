use crate::error::Result;
use crate::files::list_files;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Idle,
    Load,
    Save,
}

#[derive(Debug, Clone)]
pub(super) struct FilePicker {
    action: Action,
    active: bool,

    files: Vec<String>,
    files_selected_index: usize,

    input: String,
    cursor_pos: usize,
}

impl FilePicker {
    pub(super) fn new() -> Self {
        FilePicker {
            action: Action::Idle,
            active: false,

            // Subject for separation
            files_selected_index: 0,
            files: Vec::new(),

            // Subject for separation
            cursor_pos: 0,
            input: String::new(),
        }
    }

    pub(super) fn state(&self) -> (bool, &Action) {
        (self.active, &self.action)
    }

    pub(super) fn init_file_picker(&mut self) -> Result<()> {
        match list_files() {
            Ok(files) => {
                self.files = files;
                self.files_selected_index = 0;
                self.active = true;
                self.action = Action::Load;

                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Activate FileSaveAs popup
    pub fn init_file_save_as(&mut self) {
        // self.menu_type = MenuType::FileSaveAs;
        self.active = true;
        self.action = Action::Save;

        self.input.clear();
        self.cursor_pos = 0;
    }

    /// Moves the selection cursor up (if possible)
    pub(super) fn move_file_picker_up(&mut self) {
        if self.files_selected_index > 0 {
            self.files_selected_index -= 1;
        }
    }

    /// Moves the selection cursor down (if possible)
    pub(super) fn move_file_picker_down(&mut self) {
        if self.files_selected_index + 1 < self.files.len() {
            self.files_selected_index += 1;
        }
    }

    /// Get currently selected file (if any)
    pub(super) fn get_selected_file(&self) -> Option<&String> {
        self.files.get(self.files_selected_index)
    }

    pub(super) fn get_files(&self) -> &Vec<String> {
        &self.files
    }

    pub(super) fn get_selected_file_index(&self) -> usize {
        self.files_selected_index
    }

    pub(super) fn set_active(&mut self) {
        self.active = true;
    }

    pub(super) fn reset(&mut self) {
        self.active = false;

        self.files.clear();
        self.files_selected_index = 0;

        self.input.clear();
        self.cursor_pos = 0;

        self.action = Action::Idle;
    }

    pub(super) fn get_input(&self) -> String {
        let input = self.input.trim();
        input.to_owned()
    }
}

/// That part dedicated to file save pop up menu
impl FilePicker {
    /// Return cursor position of file picker
    pub(super) fn cursor_pos(&self) -> usize {
        self.cursor_pos
    }

    pub(super) fn is_input_empty(&self) -> bool {
        self.input.is_empty()
    }

    pub(super) fn insert_char(&mut self, c: char) {
        self.input.insert(self.cursor_pos, c);
        self.move_cursor_pos_right();
    }

    pub(super) fn delete_previous_char(&mut self) {
        if self.cursor_pos > 0 && !self.input.is_empty() {
            self.input.remove(self.cursor_pos - 1);
            self.move_cursor_pos_left();
        }
    }

    pub(super) fn delete_current_char(&mut self) {
        if self.cursor_pos < self.input.len() {
            self.input.remove(self.cursor_pos);
        }
    }

    pub(super) fn remove_char(&mut self, pos: usize) {
        self.input.remove(pos);
    }

    pub(super) fn move_cursor_pos_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
        }
    }

    pub(super) fn move_cursor_pos_right(&mut self) {
        if self.cursor_pos < self.input.len() {
            self.cursor_pos += 1;
        }
    }
}

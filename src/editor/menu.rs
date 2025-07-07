use crate::error::Result;
use crate::files::list_files;

use super::filepicker::{self, FilePicker};

const HELP_GOTO_COMMANDS: &'static [&'static str] = &[
    "g - Goto first line",
    "e - Goto end last line",
    "l - Goto end of line",
    "h - Goto start of line",
];

const HELP_AI_COMMANDS: &'static [&'static str] = &[
    "l - Send request to Ollama",
    "o - Send request to OpenAI",
    "a - Send request to Anthropic",
    "e - Exit",
];

const HELP_FILE_COMMANDS: &'static [&'static str] = &[
    "w - Wipe buffer",
    "l - Load file",
    "s - Save",
    "S - Save as",
    "q - Exit editor",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuType {
    InActive,
    GoTo,
    Main,
    File,
    AI,
}

#[derive(Debug, Clone)]
pub struct CommandsMenu {
    menu_type: MenuType,
    active: bool,

    pub(super) file_picker: filepicker::FilePicker,
}

impl From<MenuType> for String {
    fn from(value: MenuType) -> Self {
        match value {
            MenuType::InActive => "In Active".to_string(),
            MenuType::GoTo => "Go to".to_string(),
            MenuType::Main => "Main".to_string(),
            MenuType::File => "File".to_string(),
            MenuType::AI => "AI".to_string(),
        }
    }
}

impl core::fmt::Display for MenuType {
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::result::Result<(), core::fmt::Error> {
        write!(fmt, "{self:?}")
    }
}

impl CommandsMenu {
    fn new() -> Self {
        Self {
            menu_type: MenuType::InActive,
            active: false,

            file_picker: FilePicker::new(),
        }
    }

    fn vec_string_from_slice(&self, slice: &[&str]) -> Vec<String> {
        slice.iter().map(|s| s.to_string()).collect()
    }

    pub(super) fn is_file_picker_active(&self) -> bool {
        let (state, action) = self.file_picker.state();
        state
    }

    pub(crate) fn file_picker_state(&self, action: filepicker::Action) -> bool {
        let (state, current_action) = self.file_picker.state();

        state && &action == current_action
    }

    /// Returns the slice of commands for the current menu,
    /// or None if no menu is active.
    pub fn show_menu(&self) -> (Option<String>, Option<Vec<String>>) {
        match self.menu_type {
            MenuType::InActive => (None, None),
            MenuType::Main => {
                let s = self.vec_string_from_slice(&[]);

                (Some("Main".to_string()), Some(s))
            }
            MenuType::File => {
                let s = self.vec_string_from_slice(HELP_FILE_COMMANDS);

                (Some(self.menu_type.into()), Some(s))
            }
            MenuType::GoTo => {
                let s = self.vec_string_from_slice(HELP_GOTO_COMMANDS);

                (Some(self.menu_type.into()), Some(s))
            }
            MenuType::AI => {
                let s = self.vec_string_from_slice(HELP_AI_COMMANDS);

                (Some(self.menu_type.into()), Some(s))
            }

            _ => (None, None),
        }
    }

    /// Returns whether the menu is active at all.
    pub fn is_active_menu(&self) -> bool {
        self.active
    }

    /// Sets active menu type and marks the menu as active.
    pub fn set_active_menu(&mut self, m: MenuType) -> Result<()> {
        self.menu_type = m;
        self.active = true;
        Ok(())
    }

    /// Checks if the given menu type is currently active.
    pub fn is_active(&self, m: MenuType) -> bool {
        self.active && m == self.menu_type
    }

    pub fn get_file_picker_input(&self) -> String {
        self.file_picker.get_input()
    }

    pub fn get_file_picker_cursor_position(&self) -> usize {
        self.file_picker.cursor_pos()
    }

    pub fn get_file_picker_files(&self) -> &Vec<String> {
        self.file_picker.get_files()
    }

    pub fn file_picker_selected_index(&self) -> usize {
        self.file_picker.get_selected_file_index()
    }

    /// Reset menu state and clear inputs
    pub fn reset(&mut self) {
        self.menu_type = MenuType::InActive;
        self.active = false;

        self.file_picker.reset();
    }
}

impl Default for CommandsMenu {
    fn default() -> Self {
        Self::new()
    }
}

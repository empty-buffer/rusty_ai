use crate::error::Result;

const HELP_GOTO_COMMANDS: &'static [&'static str] = &[
    "g - Goto first line",
    "e - Goto end last line",
    "l - Goto end of line",
    "h - Goto start of line",
];

const HELP_AI_COMMANDS: &'static [&'static str] = &[
    "l - Send request to Ollama",
    "o - Send request to OpenAI",
    "a - Send request to Antropic",
    "e - exit",
];

const HELP_FILE_COMMANDS: &'static [&'static str] = &["w - Wipe buffer", "s - Save buffer"];

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
}

// pub fn is_active_menu(&self) -> bool {
//     self.menu_type != MenuType::InActive
// }

impl CommandsMenu {
    fn new() -> Self {
        Self {
            menu_type: MenuType::InActive,
            active: false,
        }
    }

    /// Returns the slice of commands for the current menu,
    /// or None if no menu is active.
    pub fn show_menu(&self) -> Option<&'static [&'static str]> {
        match self.menu_type {
            MenuType::InActive => None,
            MenuType::Main => Some(&[]), // Or add commands if you have them
            MenuType::File => Some(HELP_FILE_COMMANDS),
            MenuType::GoTo => Some(HELP_GOTO_COMMANDS),
            MenuType::AI => Some(HELP_AI_COMMANDS),
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

    /// Resets the menu to inactive state.
    pub fn reset(&mut self) {
        self.menu_type = MenuType::InActive;
        self.active = false;
    }
}

impl Default for CommandsMenu {
    fn default() -> Self {
        Self::new()
    }
}

use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuType {
    InActive,
    Main,
    File,
    AI,
}

#[derive(Debug, Clone, Copy)]
pub struct CommandsMenu {
    menu_type: MenuType,
    active: bool,
}

impl CommandsMenu {
    fn new() -> Self {
        Self {
            menu_type: MenuType::InActive,
            active: false,
        }
    }

    pub fn show_menu(self) -> Option<String> {
        match self.menu_type {
            MenuType::InActive => None,
            MenuType::Main => todo!(),
            MenuType::File => todo!(),
            MenuType::AI => Some("l - Send request to Ollama".to_string()),
        }
    }

    pub fn set_active_menu(&mut self, m: MenuType) -> Result<()> {
        self.menu_type = m;
        self.active = true;
        Ok(())
    }

    pub fn is_active(self, m: MenuType) -> bool {
        if self.active && m == self.menu_type {
            true
        } else {
            false
        }
    }

    // MUT Problem!
    pub fn reset(&mut self) {
        self.menu_type = MenuType::InActive;
        self.active = false
    }
}

impl Default for CommandsMenu {
    fn default() -> Self {
        Self::new()
    }
}

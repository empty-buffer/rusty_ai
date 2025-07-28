use crossterm::style::Color;

pub(super) struct Theme {
    pub(super) default_font: Color,
    pub(super) background: Color,
    pub(super) secondary_bacgroud: Color,
}

impl Theme {
    pub(super) fn new() -> Theme {
        Theme {
            default_font: Color::Rgb {
                r: 207,
                g: 207,
                b: 207,
            },
            background: Color::Black,
            secondary_bacgroud: Color::DarkBlue,
        }
    }

    pub(super) fn default_cell(&self) -> (char, Color, Option<Color>) {
        (' ', self.default_font, Some(self.background))
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::new()
    }
}

use crossterm::style::Color;

pub(super) struct Theme {
    pub(super) default_font: Color,
    pub(super) background: Color,
    pub(super) secondary_background: Color,
    pub(super) function: Color,
    pub(super) keyword: Color,
    pub(super) string: Color,
    pub(super) types: Color,
    pub(super) variable: Color,
    pub(super) constructor: Color,
    pub(super) property: Color,
    pub(super) builtin: Color,
}

// comment
impl Theme {
    pub(super) fn new() -> Theme {
        Theme {
            default_font: Color::Rgb {
                r: 207,
                g: 207,
                b: 207,
            },
            background: Color::Black,
            secondary_background: Color::DarkBlue,
            function: Color::Rgb {
                r: 234,
                g: 200,
                b: 141,
            },
            keyword: Color::Rgb {
                r: 124,
                g: 204,
                b: 177,
            },
            types: Color::Rgb {
                r: 135,
                g: 195,
                b: 255,
            },
            // Coral
            builtin: Color::Rgb {
                r: 204,
                g: 124,
                b: 137,
            },

            // 135, 195, 255
            variable: Color::Rgb {
                r: 204,
                g: 124,
                b: 137,
            },
            string: Color::Rgb {
                r: 227,
                g: 148,
                b: 220,
            },
            constructor: Color::Rgb {
                r: 135,
                g: 195,
                b: 255,
            },
            property: Color::Rgb {
                r: 176,
                g: 156,
                b: 255,
            },
        }
    }

    /// default_cell default color to fill gaps in editor
    pub(super) fn default_cell(&self) -> (char, Color, Option<Color>) {
        (' ', self.default_font, Some(self.background))
    }

    pub(super) fn default_style(&self) -> (Color, Option<Color>) {
        (self.default_font, Some(self.background))
    }

    pub(super) fn function_style(&self) -> (Color, Option<Color>) {
        (self.function, Some(self.background))
    }

    pub(super) fn keyword_style(&self) -> (Color, Option<Color>) {
        (self.keyword, Some(self.background))
    }

    pub(super) fn type_style(&self) -> (Color, Option<Color>) {
        (self.types, Some(self.background))
    }

    pub(super) fn type_builtin_style(&self) -> (Color, Option<Color>) {
        (self.builtin, Some(self.background))
    }

    pub(super) fn variable_style(&self) -> (Color, Option<Color>) {
        (self.variable, Some(self.background))
    }

    pub(super) fn string_style(&self) -> (Color, Option<Color>) {
        (self.string, Some(self.background))
    }

    pub(super) fn constructor_style(&self) -> (Color, Option<Color>) {
        (self.constructor, Some(self.background))
    }

    pub(super) fn property_style(&self) -> (Color, Option<Color>) {
        (self.property, Some(self.background))
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::new()
    }
}

// fn test() -> Option<i32> {
//     let bool = false;

//     !todo!()
// }

// static GREETING: &str = "Hello, wo1rld!";
// const GREETING1: &str = "Hello, world!";

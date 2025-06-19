use crate::editor::{Editor, Mode, RequestState};
use crate::error::Result;

use crossterm::{
    cursor::MoveTo,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{size, Clear, ClearType},
    QueueableCommand,
};
use std::cmp::{max, min};
use std::io::{self, stdout, Stdout, Write};

use crate::syntax::Style;

pub struct WrappedLineInfo {
    pub logical_line: usize,
    pub start_col: usize,
    pub screen_row: usize,
}

pub struct RenderState {
    wrapped_lines_info: Vec<WrappedLineInfo>,

    scroll_offset: usize, // First line displayed (for scrolling)
    term_width: u16,
    term_height: u16,
    line_number_width: usize,

    force_full_redraw: bool,

    // Double buffering support
    previous_content: String, // Stores the previously rendered content
    previous_cursor: (usize, usize), // Previous cursor position
    previous_mode: Mode,      // Previous editor mode
    previous_request_stae: RequestState,
    previous_modified: bool, // Previous modification state

    // Double buffering
    current_buffer: Vec<Vec<(char, Color, Option<Color>)>>, // char, fg, bg
    previous_buffer: Vec<Vec<(char, Color, Option<Color>)>>,
}

impl RenderState {
    pub fn new() -> Result<Self> {
        let (term_width, term_height) = size()?;

        // Create buffers with default values (space character with default colors)
        let default_cell = (' ', Color::Reset, None);
        let current_buffer = vec![vec![default_cell; term_width as usize]; term_height as usize];
        let previous_buffer = vec![vec![default_cell; term_width as usize]; term_height as usize];

        Ok(Self {
            wrapped_lines_info: Vec::new(),
            scroll_offset: 0,
            term_width,
            term_height,
            force_full_redraw: false,
            line_number_width: 4,
            previous_content: String::new(),
            previous_cursor: (0, 0),
            previous_mode: Mode::Normal,
            previous_request_stae: RequestState::Idle,
            previous_modified: false,
            current_buffer,
            previous_buffer,
        })
    }

    pub fn update_dimensions(&mut self) -> Result<()> {
        let (width, height) = size()?;

        if width != self.term_width || height != self.term_height {
            self.term_width = width;
            self.term_height = height;

            // Resize buffers
            let default_cell = (' ', Color::Reset, None);
            self.current_buffer = vec![vec![default_cell; width as usize]; height as usize];
            self.previous_buffer = vec![vec![default_cell; width as usize]; height as usize];

            // Force full redraw
            self.previous_content = String::new();
            self.force_full_redraw = true;

            stdout().queue(Clear(ClearType::All))?.flush()?;
        }
        Ok(())
    }
}

impl RenderState {
    // Additional methods

    // Set a character with style in the current buffer
    fn set_cell(&mut self, x: usize, y: usize, ch: char, fg: Color, bg: Option<Color>) {
        if y < self.term_height as usize && x < self.term_width as usize {
            self.current_buffer[y][x] = (ch, fg, bg);
        }
    }

    // Compare buffers and determine if a cell has changed
    fn cell_changed(&self, x: usize, y: usize) -> bool {
        if y >= self.term_height as usize || x >= self.term_width as usize {
            return false;
        }

        self.current_buffer[y][x] != self.previous_buffer[y][x]
    }

    // Swap buffers after drawing is complete
    fn swap_buffers(&mut self) {
        std::mem::swap(&mut self.current_buffer, &mut self.previous_buffer);
    }

    // Clear the current buffer (fill with spaces)
    fn clear_buffer(&mut self) {
        let default_cell = (' ', Color::Reset, None);
        for row in &mut self.current_buffer {
            for cell in row {
                *cell = default_cell;
            }
        }
    }
}

pub fn draw_screen(editor: &mut Editor, render_state: &mut RenderState) -> Result<()> {
    // Update terminal dimensions in case of resize
    render_state.update_dimensions()?;

    // Update scroll position to ensure cursor is visible
    adjust_scroll(editor, render_state);

    // Get current editor state
    let content = editor.get_content();
    let (cursor_row, cursor_col) = editor.get_cursor_position();
    let mode = editor.get_mode().clone();
    let request_state = editor.get_request_state().clone();
    let modified = editor.is_modified();

    // Clear the current buffer
    render_state.clear_buffer();

    // Adjust line number width based on number of lines
    let line_count = content.lines().count();
    render_state.line_number_width = line_count.to_string().len().max(3);

    // Draw content into buffer
    draw_content_to_buffer(editor, render_state)?;

    // Draw status and message lines to buffer
    draw_status_line_to_buffer(editor, render_state)?;
    // draw_message_line_to_buffer(editor, render_state)?;
    draw_request_state_line_to_buffer(editor, render_state)?;

    // Render the changes to the terminal
    render_buffer_changes(render_state)?;

    // Position the cursor
    let (cursor_row, cursor_col) = editor.get_cursor_position();

    let cursor_visual_line = render_state
        .wrapped_lines_info
        .iter()
        .filter(|wli| wli.logical_line == cursor_row && wli.start_col <= cursor_col)
        .max_by_key(|wli| wli.start_col)
        .map(|wli| wli.screen_row);

    let visual_row = if let Some(screen_row) = cursor_visual_line {
        if screen_row >= render_state.scroll_offset {
            screen_row - render_state.scroll_offset
        } else {
            0 // cursor above viewport; clamped to top
        }
    } else {
        0 // fallback
    };

    let visual_col = cursor_col
        - render_state
            .wrapped_lines_info
            .iter()
            .filter(|wli| wli.logical_line == cursor_row && wli.start_col <= cursor_col)
            .max_by_key(|wli| wli.start_col)
            .map(|wli| wli.start_col)
            .unwrap_or(0)
        + render_state.line_number_width
        + 1;

    let mut stdout = stdout();
    stdout.queue(MoveTo(visual_col as u16, visual_row as u16))?;
    stdout.flush()?;

    // Swap buffers for next frame
    render_state.swap_buffers();

    // Update previous state
    render_state.previous_content = content;
    render_state.previous_cursor = (cursor_row, cursor_col);
    render_state.previous_mode = mode;
    render_state.previous_request_stae = request_state;
    render_state.previous_modified = modified;

    Ok(())
}

fn draw_content_to_buffer(editor: &mut Editor, render_state: &mut RenderState) -> Result<()> {
    let content = editor.get_content();
    let viewport_height = render_state.term_height as usize - 2;
    let line_number_width = render_state.line_number_width;
    let max_line_width = render_state.term_width as usize - line_number_width - 1;

    let selection_range = editor.get_selection_range();

    // First, clear previous wrapped lines info
    render_state.wrapped_lines_info.clear();

    // For **all** logical lines, build wrapped lines info
    let lines: Vec<&str> = content.lines().collect();

    let mut all_wrapped_lines = Vec::new();

    for (logical_line, line) in lines.iter().enumerate() {
        let line_chars: Vec<char> = line.chars().collect();
        let mut visual_col_in_line = 0;

        while visual_col_in_line < line_chars.len()
            || (line_chars.is_empty() && visual_col_in_line == 0)
        {
            all_wrapped_lines.push((logical_line, visual_col_in_line));

            let mut displayed_width = 0;
            let mut chars_drawn = 0;
            while visual_col_in_line + chars_drawn < line_chars.len() {
                let ch = line_chars[visual_col_in_line + chars_drawn];
                let width = if ch == '\t' {
                    4 - (displayed_width % 4)
                } else {
                    1
                };
                if displayed_width + width > max_line_width {
                    break;
                }

                displayed_width += width;
                chars_drawn += 1;
            }

            if chars_drawn == 0 && visual_col_in_line == 0 && line_chars.is_empty() {
                chars_drawn = 1; // draw empty line chunk
            }

            visual_col_in_line += chars_drawn;
        }
    }

    // Store the wrapped lines info with screen_row filled (relative to whole buffer)
    render_state.wrapped_lines_info = all_wrapped_lines
        .into_iter()
        .enumerate()
        .map(|(screen_row, (logical_line, start_col))| WrappedLineInfo {
            logical_line,
            start_col,
            screen_row,
        })
        .collect();

    // Now draw only the wrapped lines visible under scroll_offset

    render_state.clear_buffer();

    let viewport_start = render_state.scroll_offset;
    let viewport_end =
        (viewport_start + viewport_height).min(render_state.wrapped_lines_info.len());

    for screen_row in viewport_start..viewport_end {
        let wli = &render_state.wrapped_lines_info[screen_row];
        let logical_line = wli.logical_line;
        let start_col = wli.start_col;

        // Draw line number only if first wrapped chunk in that logical line
        let line_num_str = if start_col == 0 {
            format!("{:>width$} ", logical_line + 1, width = line_number_width)
        } else {
            " ".repeat(line_number_width + 1)
        };
        for (x, ch) in line_num_str.chars().enumerate() {
            render_state.set_cell(
                x,
                (screen_row - viewport_start) as usize,
                ch,
                Color::DarkGrey,
                None,
            );
        }

        // Draw wrapped line chunk content
        let line_chars: Vec<char> = lines[logical_line].chars().collect();

        let mut displayed_width = 0;
        let mut col = line_number_width + 1;

        let mut chars_drawn = 0;
        while start_col + chars_drawn < line_chars.len() {
            let ch = line_chars[start_col + chars_drawn];
            let width = if ch == '\t' {
                4 - (displayed_width % 4)
            } else {
                1
            };
            if displayed_width + width > max_line_width {
                break;
            }

            // Determine style (selection, syntax, etc.)
            let style = {
                let char_idx = editor.char_idx_from_position(logical_line, start_col + chars_drawn);
                if editor.is_position_selected(
                    logical_line,
                    start_col + chars_drawn,
                    &selection_range,
                ) {
                    Style::Selection
                } else if let Some(cached_style) =
                    editor.get_syntax_cache_cached_style(logical_line, start_col + chars_drawn)
                {
                    cached_style
                } else {
                    editor.get_style_at(char_idx)
                }
            };
            let (fg_color, bg_color) = match style {
                Style::Normal => (Color::White, None),
                Style::Keyword => (Color::Magenta, None),
                Style::Function => (Color::Blue, None),
                Style::Type => (Color::Cyan, None),
                Style::String => (Color::Green, None),
                Style::Number => (Color::Yellow, None),
                Style::Comment => (Color::DarkGrey, None),
                Style::Variable => (Color::White, None),
                Style::Constant => (Color::Yellow, None),
                Style::Operator => (Color::White, None),
                Style::Selection => (Color::Black, Some(Color::Grey)),
                Style::Error => (Color::Red, Some(Color::White)),
            };

            for _ in 0..width {
                render_state.set_cell(
                    col,
                    (screen_row - viewport_start) as usize,
                    ' ',
                    fg_color,
                    bg_color,
                );
                col += 1;
                displayed_width += 1;
            }

            if ch != '\t' {
                // Overwrite last space with actual char
                let x = col - width;
                for i in 0..width {
                    render_state.set_cell(
                        x + i,
                        (screen_row - viewport_start) as usize,
                        ch,
                        fg_color,
                        bg_color,
                    );
                }
            }

            chars_drawn += 1;
        }

        // Fill end of line with spaces
        while col < render_state.term_width as usize {
            render_state.set_cell(
                col,
                (screen_row - viewport_start) as usize,
                ' ',
                Color::Reset,
                None,
            );
            col += 1;
        }
    }

    // Clear leftover lines if any
    for row in (viewport_end - viewport_start)..viewport_height {
        for x in 0..render_state.term_width as usize {
            render_state.set_cell(x, row as usize, ' ', Color::Reset, None);
        }
    }

    Ok(())
}

fn draw_status_line_to_buffer(editor: &Editor, render_state: &mut RenderState) -> Result<()> {
    let row = render_state.term_height as usize - 2;

    // Filename or [No Name]
    let filename = editor.get_file_name().unwrap_or("[No Name]");
    let modified_indicator = if editor.is_modified() { " [+] " } else { " " };

    // Mode indicator
    let mode = if editor.is_waiting_for_command() {
        "WAITING FOR COMMAND"
    } else {
        match editor.get_mode() {
            Mode::Normal => "NORMAL",
            Mode::Insert => "INSERT",
            Mode::Select => "SELECT",
        }
    };

    // Get cursor position
    let (cursor_row, cursor_col) = editor.get_cursor_position();

    // Format the status line
    let left_status = format!("{}{} - {} ", filename, modified_indicator, mode);
    let right_status = format!("  {}:{}  ", cursor_row + 1, cursor_col + 1);

    let term_width = render_state.term_width as usize;

    // Calculate padding
    let padding_width = term_width
        .saturating_sub(left_status.len())
        .saturating_sub(right_status.len());

    let status_line = format!(
        "{}{}{}",
        left_status,
        " ".repeat(padding_width),
        right_status
    );

    // Fill the entire status line
    for (x, ch) in status_line.chars().enumerate() {
        if x >= render_state.term_width as usize {
            break;
        }
        render_state.set_cell(x, row, ch, Color::Black, Some(Color::White));
    }

    // Fill any remaining space
    for x in status_line.len()..render_state.term_width as usize {
        render_state.set_cell(x, row, ' ', Color::Black, Some(Color::White));
    }

    Ok(())
}

fn draw_message_line_to_buffer(editor: &Editor, render_state: &mut RenderState) -> Result<()> {
    let row = render_state.term_height as usize - 2;

    // Help message based on mode
    let help_msg = match editor.get_mode() {
        Mode::Normal => "^Q: Quit | i: Insert | v: Select | s: Save | y: Copy selection",
        Mode::Insert => "ESC: Normal mode | Arrow keys: Navigate",
        Mode::Select => {
            "ESC: Normal mode | Arrow keys: Extend selection | y: Copy and exit selection | d: Delete"
        }
    };

    // Fill message line
    for (x, ch) in help_msg.chars().enumerate() {
        if x >= render_state.term_width as usize {
            break;
        }
        render_state.set_cell(x, row, ch, Color::DarkGrey, None);
    }

    // Clear any remaining part of the line
    for x in help_msg.len()..render_state.term_width as usize {
        render_state.set_cell(x, row, ' ', Color::Reset, None);
    }

    Ok(())
}

fn draw_request_state_line_to_buffer(
    editor: &Editor,
    render_state: &mut RenderState,
) -> Result<()> {
    let row = render_state.term_height as usize - 1;

    // Help message based on mode
    let help_msg = match editor.get_request_state() {
        RequestState::Idle => format!("Request Status: {}", "Idle"),
        //TODO PROVIDER
        RequestState::Proccessing => format!("Request Status: {}", "In Progress"),
        RequestState::Error(e) => {
            let msg = format!("Request Status: Error: {}", e);
            msg
        }
    };

    // Fill message line
    for (x, ch) in help_msg.chars().enumerate() {
        if x >= render_state.term_width as usize {
            break;
        }
        render_state.set_cell(x, row, ch, Color::White, None);
    }

    // Clear any remaining part of the line
    for x in help_msg.len()..render_state.term_width as usize {
        render_state.set_cell(x, row, ' ', Color::Reset, None);
    }

    Ok(())
}

fn render_buffer_changes(render_state: &mut RenderState) -> Result<()> {
    let mut stdout = stdout();

    // Track the current style to minimize style change commands
    let mut current_fg = Color::Reset;
    let mut current_bg: Option<Color> = None;

    // Compare buffers and output only the differences
    for y in 0..render_state.term_height as usize {
        let mut current_x = 0;

        while current_x < render_state.term_width as usize {
            // If this cell hasn't changed, skip it
            // if !render_state.force_full_redraw && !render_state.cell_changed(current_x, y) {
            if !render_state.cell_changed(current_x, y) {
                current_x += 1;
                continue;
            }

            // Find how many consecutive cells have changed
            let start_x = current_x;
            let mut end_x = start_x;

            // Get the style for this cell
            let (_, cell_fg, cell_bg) = render_state.current_buffer[y][start_x];

            // Find consecutive cells with the same style
            while end_x < render_state.term_width as usize
                && render_state.cell_changed(end_x, y)
                && render_state.current_buffer[y][end_x].1 == cell_fg
                && render_state.current_buffer[y][end_x].2 == cell_bg
            {
                end_x += 1;
            }

            // Move cursor to start of changed region
            stdout.queue(MoveTo(start_x as u16, y as u16))?;

            // Update style if needed
            if current_fg != cell_fg {
                stdout.queue(SetForegroundColor(cell_fg))?;
                current_fg = cell_fg;
            }

            if current_bg != cell_bg {
                if let Some(bg) = cell_bg {
                    stdout.queue(SetBackgroundColor(bg))?;
                } else {
                    stdout.queue(ResetColor)?;
                    // Need to restore foreground color after reset
                    stdout.queue(SetForegroundColor(current_fg))?;
                }
                current_bg = cell_bg;
            }

            // Output the changed text
            let mut text = String::with_capacity(end_x - start_x);
            for x in start_x..end_x {
                text.push(render_state.current_buffer[y][x].0);
            }
            stdout.queue(Print(text))?;

            // Update current position
            current_x = end_x;
        }
    }

    render_state.force_full_redraw = false;

    // Reset styles
    stdout.queue(ResetColor)?;
    stdout.flush()?;

    Ok(())
}

fn adjust_scroll(editor: &Editor, render_state: &mut RenderState) {
    let (cursor_row, cursor_col) = editor.get_cursor_position();
    let viewport_height = render_state.term_height as usize - 2; // Space for status/message lines

    // Find which visual line contains the cursor position
    // Find the visual line containing the cursor:
    let mut cursor_visual_line: Option<&WrappedLineInfo> = None;

    for wli in &render_state.wrapped_lines_info {
        if wli.logical_line == cursor_row && wli.start_col <= cursor_col && {
            // Next wrapped line start_col (for same logical line) must be > cursor_col or no next wrap
            true
        } {
            // To pick correct chunk, remember the max start_col ≤ cursor_col for logical_line
            // We'll iterate all below, then take max start_col ≤ cursor_col

            if let Some(current) = cursor_visual_line {
                if wli.start_col > current.start_col {
                    cursor_visual_line = Some(wli);
                }
            } else {
                cursor_visual_line = Some(wli);
            }
        }
    }

    let cursor_visual_line = match cursor_visual_line {
        Some(wli) => wli.screen_row,
        None => 0, // fallback if not found
    };

    // Now scroll_offset is visual line index, adjust it to keep cursor visible

    if cursor_visual_line < render_state.scroll_offset {
        render_state.scroll_offset = cursor_visual_line;
    } else if cursor_visual_line >= render_state.scroll_offset + viewport_height {
        render_state.scroll_offset = cursor_visual_line - viewport_height + 1;
    }

    // Clamp scroll_offset not to exceed max visual lines
    let max_scroll = render_state
        .wrapped_lines_info
        .len()
        .saturating_sub(viewport_height);
    if render_state.scroll_offset > max_scroll {
        render_state.scroll_offset = max_scroll;
    }
}

fn draw_content(
    editor: &mut Editor,
    render_state: &RenderState,
    stdout: &mut Stdout,
) -> Result<()> {
    let content = editor.get_content();
    let viewport_height = render_state.term_height as usize - 2; // Space for status/message lines
    let line_number_width = render_state.line_number_width;

    // Get all visible lines
    let visible_lines: Vec<&str> = content
        .lines()
        .skip(render_state.scroll_offset)
        .take(viewport_height)
        .collect();

    // Clear only the lines that we need to redraw
    for row in 0..min(viewport_height, visible_lines.len()) {
        stdout.queue(MoveTo(0, row as u16))?;
        stdout.queue(Clear(ClearType::CurrentLine))?;
    }

    // Draw each visible line
    for (i, line) in visible_lines.iter().enumerate() {
        let row = i;
        let real_line_number = i + render_state.scroll_offset + 1;

        // Draw line number
        stdout.queue(MoveTo(0, row as u16))?;
        stdout.queue(SetForegroundColor(Color::DarkGrey))?;
        stdout.queue(Print(format!(
            "{:>width$} ",
            real_line_number,
            width = line_number_width
        )))?;
        stdout.queue(ResetColor)?;

        // Calculate character index for the start of this line
        let mut line_start_char_idx = 0;
        for line_idx in 0..render_state.scroll_offset + i {
            if let Some(line) = content.lines().nth(line_idx) {
                line_start_char_idx += line.len() + 1; // +1 for newline
            }
        }

        // Draw the actual line content with syntax highlighting
        stdout.queue(MoveTo(line_number_width as u16 + 1, row as u16))?;

        let max_line_width = render_state.term_width as usize - line_number_width - 1;
        let mut displayed_width = 0;

        // Process each character in the line with its style
        for (char_idx, ch) in line.chars().enumerate() {
            let actual_char_idx = line_start_char_idx + char_idx;
            let actual_row = row + render_state.scroll_offset;

            // let style = editor.get_style_at(actual_char_idx);
            let style = editor.get_style_for_position(actual_row, actual_char_idx);

            // Set color based on style
            let (fg_color, bg_color) = match style {
                Style::Normal => (Color::White, None),
                Style::Keyword => (Color::Magenta, None),
                Style::Function => (Color::Blue, None),
                Style::Type => (Color::Cyan, None),
                Style::String => (Color::Green, None),
                Style::Number => (Color::Yellow, None),
                Style::Comment => (Color::DarkGrey, None),
                Style::Variable => (Color::White, None),
                Style::Constant => (Color::Yellow, None),
                Style::Operator => (Color::White, None),
                Style::Selection => (Color::Black, Some(Color::Grey)),
                Style::Error => (Color::Red, Some(Color::White)),
            };

            stdout.queue(SetForegroundColor(fg_color))?;
            if let Some(bg) = bg_color {
                stdout.queue(SetBackgroundColor(bg))?;
            }

            // Handle tab and width calculations
            let width = if ch == '\t' {
                4 - (displayed_width % 4) // Tab stops every 4 spaces
            } else {
                1
            };

            if displayed_width + width > max_line_width {
                break;
            }

            // Print the character
            if ch == '\t' {
                stdout.queue(Print(" ".repeat(width)))?;
            } else {
                stdout.queue(Print(ch))?;
            }

            displayed_width += width;

            // Reset color after each character
            stdout.queue(ResetColor)?;
        }
    }

    // Clear any remaining lines in the viewport that don't have content
    for row in visible_lines.len()..viewport_height {
        stdout.queue(MoveTo(0, row as u16))?;
        stdout.queue(Clear(ClearType::CurrentLine))?;
    }

    Ok(())
}

// fn draw_status_line(
//     editor: &Editor,
//     render_state: &RenderState,
//     stdout: &mut Stdout,
// ) -> Result<()> {
//     // Position at the bottom of the screen - 2
//     stdout.queue(MoveTo(0, render_state.term_height - 2))?;
//     stdout.queue(Clear(ClearType::CurrentLine))?;

//     // Set background to white, text to black
//     stdout.queue(SetBackgroundColor(Color::White))?;
//     stdout.queue(SetForegroundColor(Color::Black))?;

//     // Filename or [No Name]
//     let filename = editor.get_file_name().unwrap_or("[No Name]");
//     let modified_indicator = if editor.is_modified() { " [+] " } else { " " };

//     // Mode indicator
//     let mode = match editor.get_mode() {
//         Mode::Normal => "NORMAL",
//         Mode::Insert => "INSERT",
//         Mode::Select => "SELECT",
//     };

//     // Get cursor position
//     let (row, col) = editor.get_cursor_position();

//     // Format the status line
//     let left_status = format!("{}{} - {} ", filename, modified_indicator, mode);
//     let right_status = format!(" Ln {}, Col {} ", row + 1, col + 1);

//     let current_term_width = render_state.term_width as usize;
//     // Calculate padding to right-align the position info
//     let padding_width = current_term_width
//         .saturating_sub(left_status.len())
//         .saturating_sub(right_status.len());
//     let padding = " ".repeat(padding_width);

//     stdout.queue(Print(left_status))?;
//     stdout.queue(Print(padding))?;
//     stdout.queue(Print(right_status))?;
//     stdout.queue(ResetColor)?;

//     Ok(())
// }

// fn draw_message_line(
//     editor: &Editor,
//     render_state: &RenderState,
//     stdout: &mut Stdout,
// ) -> Result<()> {
//     // Position at the bottom of the screen - 1
//     stdout.queue(MoveTo(0, render_state.term_height - 1))?;
//     stdout.queue(Clear(ClearType::CurrentLine))?;

//     // You can add messages here (like "File saved" or error messages)
//     // For now, you can show key bindings as a help message
//     let help_msg = match editor.get_mode() {
//         Mode::Normal => "^Q: Quit | i: Insert | v: Select | s: Save",
//         Mode::Insert => "ESC: Normal mode | Arrow keys: Navigate",
//         Mode::Select => "ESC: Normal mode | Arrow keys: Extend selection",
//     };

//     stdout.queue(SetForegroundColor(Color::DarkGrey))?;
//     stdout.queue(Print(help_msg))?;
//     stdout.queue(ResetColor)?;

//     Ok(())
// }

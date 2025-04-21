use crate::editor::{Editor, Mode};
use crate::error::Result;
use crossterm::{
    cursor::MoveTo,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{size, Clear, ClearType},
    QueueableCommand,
};
use std::cmp::{max, min};
use std::io::{self, stdout, Stdout, Write};

pub struct RenderState {
    scroll_offset: usize, // First line displayed (for scrolling)
    term_width: u16,
    term_height: u16,
    line_number_width: usize,

    // Double buffering support
    previous_content: String, // Stores the previously rendered content
    previous_cursor: (usize, usize), // Previous cursor position
    previous_mode: Mode,      // Previous editor mode
    previous_modified: bool,  // Previous modification state
}

impl RenderState {
    pub fn new() -> Result<Self> {
        let (term_width, term_height) = size()?;
        Ok(Self {
            scroll_offset: 0,
            term_width,
            term_height,
            line_number_width: 4,
            previous_content: String::new(),
            previous_cursor: (0, 0),
            previous_mode: Mode::Normal,
            previous_modified: false,
        })
    }

    pub fn update_dimensions(&mut self) -> Result<()> {
        let (width, height) = size()?;
        // If dimensions have changed, force a full redraw
        if width != self.term_width || height != self.term_height {
            self.term_width = width;
            self.term_height = height;
            self.previous_content = String::new(); // Force redraw
        }
        Ok(())
    }
}

pub fn draw_screen(editor: &Editor, render_state: &mut RenderState) -> Result<()> {
    // Update terminal dimensions in case of resize
    render_state.update_dimensions()?;

    // Update scroll position to ensure cursor is visible
    adjust_scroll(editor, render_state);

    // Get current editor state
    let content = editor.get_content();
    let (cursor_row, cursor_col) = editor.get_cursor_position();
    let mode = editor.get_mode().clone();
    let modified = editor.is_modified();

    // Check if content has changed
    let content_changed = content != render_state.previous_content;
    let cursor_changed = (cursor_row, cursor_col) != render_state.previous_cursor;

    let mode_changed = mode != render_state.previous_mode;
    let modified_changed = modified != render_state.previous_modified;

    // Get the terminal
    let mut stdout = stdout();

    // If nothing changed, we can skip redrawing
    if !content_changed && !cursor_changed && !mode_changed && !modified_changed {
        // Still need to position cursor correctly
        let visible_row = cursor_row.saturating_sub(render_state.scroll_offset);
        let visible_col = cursor_col + render_state.line_number_width + 1;
        stdout.queue(MoveTo(visible_col as u16, visible_row as u16))?;
        stdout.flush()?;
        return Ok(());
    }

    // Adjust line number width based on number of lines
    let line_count = content.lines().count();
    render_state.line_number_width = line_count.to_string().len().max(3);

    // Only clear and redraw content if it's changed
    if content_changed || cursor_changed {
        // Instead of clearing the whole screen, clear each line as we redraw it
        draw_content(editor, render_state, &mut stdout)?;
    }

    // Only redraw status line if relevant info changed
    if content_changed || mode_changed || modified_changed {
        draw_status_line(editor, render_state, &mut stdout)?;
    }

    // Only redraw message line if mode changed
    if mode_changed {
        draw_message_line(editor, render_state, &mut stdout)?;
    }

    // Position the cursor
    let visible_row = cursor_row.saturating_sub(render_state.scroll_offset);
    let visible_col = cursor_col + render_state.line_number_width + 1;
    stdout.queue(MoveTo(visible_col as u16, visible_row as u16))?;

    // Flush all queued commands
    stdout.flush()?;

    // Update previous state for next comparison
    render_state.previous_content = content;
    render_state.previous_cursor = (cursor_row, cursor_col);
    render_state.previous_mode = mode;
    render_state.previous_modified = modified;

    Ok(())
}

fn adjust_scroll(editor: &Editor, render_state: &mut RenderState) {
    let (cursor_row, _) = editor.get_cursor_position();
    let viewport_height = render_state.term_height as usize - 2; // Space for status/message lines

    // If cursor is above visible area, scroll up
    if cursor_row < render_state.scroll_offset {
        render_state.scroll_offset = cursor_row;
    }

    // If cursor is below visible area, scroll down
    if cursor_row >= render_state.scroll_offset + viewport_height {
        render_state.scroll_offset = cursor_row - viewport_height + 1;
    }
}

fn draw_content(editor: &Editor, render_state: &RenderState, stdout: &mut Stdout) -> Result<()> {
    let content = editor.get_content();
    let viewport_height = render_state.term_height as usize - 2; // Space for status/message lines
    let line_number_width = render_state.line_number_width;

    // Get all visible lines
    let visible_lines: Vec<&str> = content
        .lines()
        .skip(render_state.scroll_offset)
        .take(viewport_height)
        .collect();

    // Clear only the lines we're about to draw
    for row in 0..viewport_height {
        if row < visible_lines.len() {
            stdout.queue(MoveTo(0, row as u16))?;
            stdout.queue(Clear(ClearType::CurrentLine))?;
        }
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

        // Draw the actual line content
        stdout.queue(MoveTo(line_number_width as u16 + 1, row as u16))?;

        // Handle tab characters and truncate lines that exceed terminal width
        let max_line_width = render_state.term_width as usize - line_number_width - 1;
        let mut displayed_width = 0;
        let mut displayed_text = String::new();

        for ch in line.chars() {
            let width = if ch == '\t' {
                4 - (displayed_width % 4) // Tab stops every 4 spaces
            } else {
                1
            };

            if displayed_width + width > max_line_width {
                break;
            }

            if ch == '\t' {
                displayed_text.push_str(&" ".repeat(width));
            } else {
                displayed_text.push(ch);
            }

            displayed_width += width;
        }

        stdout.queue(Print(displayed_text))?;
    }

    // Clear any remaining lines in the viewport that don't have content
    for row in visible_lines.len()..viewport_height {
        stdout.queue(MoveTo(0, row as u16))?;
        stdout.queue(Clear(ClearType::CurrentLine))?;
    }

    Ok(())
}

fn draw_status_line(
    editor: &Editor,
    render_state: &RenderState,
    stdout: &mut Stdout,
) -> Result<()> {
    // Position at the bottom of the screen - 2
    stdout.queue(MoveTo(0, render_state.term_height - 2))?;
    stdout.queue(Clear(ClearType::CurrentLine))?;

    // Set background to white, text to black
    stdout.queue(SetBackgroundColor(Color::White))?;
    stdout.queue(SetForegroundColor(Color::Black))?;

    // Filename or [No Name]
    let filename = editor.get_file_name().unwrap_or("[No Name]");
    let modified_indicator = if editor.is_modified() { " [+] " } else { " " };

    // Mode indicator
    let mode = match editor.get_mode() {
        Mode::Normal => "NORMAL",
        Mode::Insert => "INSERT",
        Mode::Select => "SELECT",
    };

    // Get cursor position
    let (row, col) = editor.get_cursor_position();

    // Format the status line
    let left_status = format!("{}{} - {} ", filename, modified_indicator, mode);
    let right_status = format!(" Ln {}, Col {} ", row + 1, col + 1);

    let current_term_width = render_state.term_width as usize;
    // Calculate padding to right-align the position info
    let padding_width = current_term_width
        .saturating_sub(left_status.len())
        .saturating_sub(right_status.len());
    let padding = " ".repeat(padding_width);

    stdout.queue(Print(left_status))?;
    stdout.queue(Print(padding))?;
    stdout.queue(Print(right_status))?;
    stdout.queue(ResetColor)?;

    Ok(())
}

fn draw_message_line(
    editor: &Editor,
    render_state: &RenderState,
    stdout: &mut Stdout,
) -> Result<()> {
    // Position at the bottom of the screen - 1
    stdout.queue(MoveTo(0, render_state.term_height - 1))?;
    stdout.queue(Clear(ClearType::CurrentLine))?;

    // You can add messages here (like "File saved" or error messages)
    // For now, you can show key bindings as a help message
    let help_msg = match editor.get_mode() {
        Mode::Normal => "^Q: Quit | i: Insert | v: Select | s: Save",
        Mode::Insert => "ESC: Normal mode | Arrow keys: Navigate",
        Mode::Select => "ESC: Normal mode | Arrow keys: Extend selection",
    };

    stdout.queue(SetForegroundColor(Color::DarkGrey))?;
    stdout.queue(Print(help_msg))?;
    stdout.queue(ResetColor)?;

    Ok(())
}

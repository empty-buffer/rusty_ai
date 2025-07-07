use crossterm::style::Color;

use crate::error::Result;

use super::RenderState;

pub(super) fn draw_file_picker_popup_to_buffer(
    render_state: &mut RenderState,
    files: &[String],
    selected_index: usize,
) -> Result<()> {
    let max_file_len = files.iter().map(|f| f.len()).max().unwrap_or(0);
    let title = "Pick a file";

    let popup_width = max_file_len + 4; // padding + borders
    let popup_height = files.len() + 2; // files + top & bottom borders

    let term_width = render_state.term_width as usize;
    let term_height = render_state.term_height as usize;

    // Center the popup
    let start_x = if term_width > popup_width {
        (term_width - popup_width) / 2
    } else {
        0
    };
    let start_y = if term_height > popup_height {
        (term_height - popup_height) / 2
    } else {
        0
    };

    let fg = Color::White;
    let bg = Some(Color::DarkGrey);
    let selection_fg = Color::Black;
    let selection_bg = Some(Color::White);

    // Draw border
    render_state.set_cell(start_x, start_y, '┌', fg, bg);

    let title_len = title.len();
    let available_space = popup_width - 2;

    let title_start_pos = start_x + 1 + (available_space - title_len) / 2;

    // Fill the line with '─' first
    for x in (start_x + 1)..(start_x + popup_width - 1) {
        render_state.set_cell(x, start_y, '─', fg, bg);
    }

    // Overwrite with the title characters
    for (i, ch) in title.chars().enumerate() {
        render_state.set_cell(title_start_pos + i, start_y, ch, fg, bg);
    }

    render_state.set_cell(start_x + popup_width - 1, start_y, '┐', fg, bg);

    for (i, file_name) in files.iter().enumerate() {
        let y = start_y + 1 + i;
        render_state.set_cell(start_x, y, '│', fg, bg);

        if i == selected_index {
            // Draw the selected item with inverted colors (white bg + black fg)
            for (j, ch) in file_name.chars().enumerate() {
                if start_x + 1 + j < render_state.term_width as usize {
                    render_state.set_cell(start_x + 1 + j, y, ch, selection_fg, selection_bg);
                }
            }
            // Fill to popup width with spaces in selected bg color
            for x in (start_x + 1 + file_name.len())..(start_x + popup_width - 1) {
                render_state.set_cell(x, y, ' ', selection_fg, selection_bg);
            }
        } else {
            // Draw normally
            for (j, ch) in file_name.chars().enumerate() {
                if start_x + 1 + j < render_state.term_width as usize {
                    render_state.set_cell(start_x + 1 + j, y, ch, fg, bg);
                }
            }
            // Fill remaining space
            for x in (start_x + 1 + file_name.len())..(start_x + popup_width - 1) {
                render_state.set_cell(x, y, ' ', fg, bg);
            }
        }

        render_state.set_cell(start_x + popup_width - 1, y, '│', fg, bg);
    }

    // Bottom border
    let bottom_y = start_y + popup_height - 1;
    render_state.set_cell(start_x, bottom_y, '└', fg, bg);
    for x in (start_x + 1)..(start_x + popup_width - 1) {
        render_state.set_cell(x, bottom_y, '─', fg, bg);
    }
    render_state.set_cell(start_x + popup_width - 1, bottom_y, '┘', fg, bg);

    Ok(())
}

pub(super) fn draw_help_popup_to_buffer(
    render_state: &mut RenderState,
    title: String,
    commands: Vec<String>,
) -> Result<()> {
    let max_line_length = commands.iter().map(|line| line.len()).max().unwrap_or(0);

    // Calculate popup box dimensions: width & height
    let popup_width = max_line_length.max(title.len()) + 4; // padding + borders
    let popup_height = commands.len() + 2; // commands + top & bottom border

    // Starting position - bottom right corner with some padding
    let term_width = render_state.term_width as usize;
    let term_height = render_state.term_height as usize;

    let start_x = if term_width > popup_width + 1 {
        term_width - popup_width - 1
    } else {
        0
    };
    let start_y = if term_height > popup_height + 1 {
        term_height - popup_height - 1
    } else {
        0
    };

    let fg = Color::White;
    let bg = Some(Color::DarkGrey);

    // Draw border: top line with title
    render_state.set_cell(start_x, start_y, '┌', fg, bg);

    let title_len = title.len();
    let available_space = popup_width - 2; // excluding corners

    // Option 1: Center the title horizontally in the top border
    let title_start_pos = start_x + 1 + (available_space - title_len) / 2;

    // Fill the line with '─' first
    for x in (start_x + 1)..(start_x + popup_width - 1) {
        render_state.set_cell(x, start_y, '─', fg, bg);
    }

    // Overwrite with the title characters
    for (i, ch) in title.chars().enumerate() {
        render_state.set_cell(title_start_pos + i, start_y, ch, fg, bg);
    }

    render_state.set_cell(start_x + popup_width - 1, start_y, '┐', fg, bg);

    // Draw middle lines (with sides)
    for (i, cmd) in commands.iter().enumerate() {
        let y = start_y + 1 + i;
        render_state.set_cell(start_x, y, '│', fg, bg);

        for (j, ch) in cmd.chars().enumerate() {
            render_state.set_cell(start_x + 1 + j, y, ch, fg, bg);
        }

        // fill rest with spaces if the line is shorter than popup_width
        for x in (start_x + 1 + cmd.len())..(start_x + popup_width - 1) {
            render_state.set_cell(x, y, ' ', fg, bg);
        }

        render_state.set_cell(start_x + popup_width - 1, y, '│', fg, bg);
    }

    // Draw bottom line
    let bottom_y = start_y + popup_height - 1;
    render_state.set_cell(start_x, bottom_y, '└', fg, bg);
    for x in (start_x + 1)..(start_x + popup_width - 1) {
        render_state.set_cell(x, bottom_y, '─', fg, bg);
    }
    render_state.set_cell(start_x + popup_width - 1, bottom_y, '┘', fg, bg);

    Ok(())
}

pub(super) fn draw_file_save_as_popup_to_buffer(
    render_state: &mut RenderState,
    input: &str,
    cursor_pos: usize,
) -> Result<()> {
    // Determine popup size (fixed width or dynamic based on input length)
    let popup_width = 40;
    let popup_height = 5; // Enough for title, input line, borders

    let term_width = render_state.term_width as usize;
    let term_height = render_state.term_height as usize;

    // Center popup
    let start_x = if term_width > popup_width {
        (term_width - popup_width) / 2
    } else {
        0
    };

    let start_y = if term_height > popup_height {
        (term_height - popup_height) / 2
    } else {
        0
    };

    let fg = Color::White;
    let bg = Some(Color::DarkGrey);

    // Draw border
    render_state.set_cell(start_x, start_y, '┌', fg, bg);
    for x in (start_x + 1)..(start_x + popup_width - 1) {
        render_state.set_cell(x, start_y, '─', fg, bg);
    }
    render_state.set_cell(start_x + popup_width - 1, start_y, '┐', fg, bg);

    for y in (start_y + 1)..(start_y + popup_height - 1) {
        render_state.set_cell(start_x, y, '│', fg, bg);
        render_state.set_cell(start_x + popup_width - 1, y, '│', fg, bg);
    }

    render_state.set_cell(start_x, start_y + popup_height - 1, '└', fg, bg);
    for x in (start_x + 1)..(start_x + popup_width - 1) {
        render_state.set_cell(x, start_y + popup_height - 1, '─', fg, bg);
    }
    render_state.set_cell(
        start_x + popup_width - 1,
        start_y + popup_height - 1,
        '┘',
        fg,
        bg,
    );

    // Title line - "Save As:"
    let title = "Save As:";
    for (i, ch) in title.chars().enumerate() {
        render_state.set_cell(start_x + 2 + i, start_y + 1, ch, fg, bg);
    }

    // Input line
    let input_start_x = start_x + 2;
    let input_y = start_y + 2;

    // Display input text (truncate if too long)
    let input_display = if input.len() > popup_width - 4 {
        let start_idx = if cursor_pos >= popup_width - 4 {
            cursor_pos - (popup_width - 4) + 1
        } else {
            0
        };
        &input[start_idx..]
    } else {
        input
    };

    for (i, ch) in input_display.chars().enumerate() {
        if input_start_x + i >= render_state.term_width as usize - 1 {
            break;
        }
        render_state.set_cell(input_start_x + i, input_y, ch, Color::White, bg);
    }

    // Clear rest of input line
    for x in (input_start_x + input_display.len())..(start_x + popup_width - 2) {
        render_state.set_cell(x, input_y, ' ', Color::White, bg);
    }

    // Draw cursor position (inverted color)
    let cursor_visual_x = input_start_x + cursor_pos.min(popup_width - 4);
    let cursor_char = if cursor_pos < input.len() {
        input.chars().nth(cursor_pos).unwrap_or(' ')
    } else {
        ' '
    };
    render_state.set_cell(
        cursor_visual_x,
        input_y,
        cursor_char,
        Color::Black,
        Some(Color::White),
    );

    // Optional message / hint line
    let hint = "Enter: Save | Esc: Cancel";
    for (i, ch) in hint.chars().enumerate() {
        if start_x + 2 + i >= render_state.term_width as usize {
            break;
        }
        render_state.set_cell(start_x + 2 + i, start_y + 3, ch, fg, bg);
    }

    Ok(())
}

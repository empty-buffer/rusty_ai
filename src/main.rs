mod async_handler;
mod chat;
mod editor;
mod error;
mod render;
mod syntax;

mod files;
// use std::io::{self, Write};
// mod commands;
use error::Result;

use crossterm::{
    event::{
        self, Event, KeyCode, KeyEvent, KeyModifiers, KeyboardEnhancementFlags,
        PushKeyboardEnhancementFlags,
    },
    execute, queue,
    terminal::{
        disable_raw_mode, enable_raw_mode, DisableLineWrap, EnableLineWrap, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};
use std::io::{self, stdout};
use std::time::{Duration, Instant};

fn main() -> Result<()> {
    // Process command-line arguments
    // let args: Vec<String> = env::args().collect();

    let mut stdout = io::stdout();
    // Setup terminal
    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen, EnableLineWrap)?;

    let supports_keyboard_enhancement = matches!(
        crossterm::terminal::supports_keyboard_enhancement(),
        Ok(true)
    );

    if supports_keyboard_enhancement {
        queue!(
            stdout,
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES // | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                                                                    // | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
                                                                    // | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
            )
        )?;
    }

    // Create an editor instance
    // let editor = Arc::new(Mutex::new(Editor::new()));
    let mut editor = editor::Editor::new();

    if let Err(e) = editor.open_file(".rusty/history.md") {
        // Handle file opening error (you might want to show this to the user)
        eprintln!("Error opening file: {}", e);
    }

    // Run editor
    let mut render_state = render::RenderState::new()?;

    // Run editor
    let result = run_editor(&mut editor, &mut render_state);

    // Restore terminal
    disable_raw_mode()?;
    execute!(stdout, LeaveAlternateScreen, DisableLineWrap)?;

    // Return any error that occurred
    result
}

fn run_editor(editor: &mut editor::Editor, render_state: &mut render::RenderState) -> Result<()> {
    let frame_duration = Duration::from_millis(16); // ~60 FPS
    let mut last_render = Instant::now();

    loop {
        // Check for any API responses that need to be processed
        editor.check_api_responses();

        // Render the screen at controlled intervals
        let now = Instant::now();
        if now.duration_since(last_render) >= frame_duration {
            render::draw_screen(editor, render_state)?;
            last_render = now;
        }

        // Handle user input with a timeout to maintain smooth rendering
        if crossterm::event::poll(Duration::from_millis(1))? {
            if let Event::Key(KeyEvent {
                code, modifiers, ..
            }) = event::read()?
            {
                // Check for Ctrl+Q to quit
                if code == KeyCode::Char('q') && modifiers.contains(KeyModifiers::CONTROL) {
                    break;
                }

                // Pass both the key and modifiers to the editor
                let should_quit = editor.handle_key(code, modifiers)?;
                if should_quit {
                    break;
                }
            }
        }
    }
    Ok(())
}

// use inquire::Select;

// #[tokio::main]
// async fn main() -> Result<()> {
//     let mut chat_context = chat::ChatContext::new()?;

//     let options = vec![
//         commands::Command::ListFiles,
//         commands::Command::LoadFile,
//         commands::Command::ChangeDirectory,
//         commands::Command::AskQuestion,
//         commands::Command::ShowHistory,
//         commands::Command::Exit,
//     ];

//     print!("\x1B[2J\x1B[1;1H");
//     io::stdout().flush().unwrap();

//     loop {
//         let ans = Select::new("What would you like to do?", options.clone())
//             .with_help_message("Use ↑↓ arrows to navigate, enter to select")
//             .prompt()
//             .map_err(|e| {
//                 println!("Error while selection an option {}", e);
//                 e
//             })?;

//         commands::execute_command(ans, &mut chat_context).await?;
//     }
// }

/*
B////
U\\\\
F////
F\\\\
EMPTY
R////


!EMPTY
 BUFFER!


BUFFER
\\\\M\
////P/
\\\\T\
////E/

    E
    M
    P
BUFFER
*/

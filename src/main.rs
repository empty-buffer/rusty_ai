mod chat;
mod error;
mod files;
use std::io::{self, Write};
mod commands;
use error::Result;

use inquire::Select;

#[tokio::main]
async fn main() -> Result<()> {
    let mut chat_context = chat::ChatContext::new()?;

    let options = vec![
        commands::Command::ListFiles,
        commands::Command::LoadFile,
        commands::Command::ChangeDirectory,
        commands::Command::AskQuestion,
        commands::Command::ShowHistory,
        commands::Command::Exit,
    ];

    print!("\x1B[2J\x1B[1;1H");
    io::stdout().flush().unwrap();

    loop {
        let ans = Select::new("What would you like to do?", options.clone())
            .with_help_message("Use ↑↓ arrows to navigate, enter to select")
            .prompt()
            .map_err(|e| {
                println!("Error while selection an option {}", e);
                e
            })?;

        commands::execute_command(ans, &mut chat_context).await?;
    }
}

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

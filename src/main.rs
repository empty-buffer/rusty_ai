mod chat;
mod error;
mod files;
use std::io::{self, Write};
mod commands;
use error::Result;

use inquire::Select;

#[tokio::main]
async fn main() -> Result<()> {
    let mut chat_context = chat::ChatContext::new();

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

    // loop {
    //     println!("\nAvailable commands:");
    //     println!("1. List files");
    //     println!("2. Load file");
    //     println!("3. Change directory");
    //     println!("4. Ask question");
    //     println!("5. Show conversation history");
    //     println!("0. Exit");

    //     let input = commands::read_input("\nEnter command (1-5): ")?;

    //     let cmd = commands::parse_command(&input);
    //     commands::execute_command(cmd, &mut chat_context).await?;
    // }

    // Ok(())
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

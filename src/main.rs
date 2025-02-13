mod chat;
mod error;
mod files;

mod commands;
use error::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let mut chat_context = chat::ChatContext::new();

    loop {
        println!("\nAvailable commands:");
        println!("1. List files");
        println!("2. Load file");
        println!("3. Change directory");
        println!("4. Ask question");
        println!("5. Show conversation history");
        println!("0. Exit");

        let input = commands::read_input("\nEnter command (1-5): ")?;

        let cmd = commands::parse_command(&input);
        commands::execute_command(cmd, &mut chat_context).await?;
    }

    // Ok(())
}

/*
B////
U\\\\
F////
F\\\\
EMPTY
R////

BUFFER
\\\\M\
////P/
\\\\T\
////E/ 

*/

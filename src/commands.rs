use crate::chat::ChatContext;
use crate::Result;
use std::io::{self, Write};

pub(super) enum Command {
    ListFiles,
    LoadFile,
    ChangeDirectory,
    AskQuestion,
    ShowHistory,
    Exit,
    Invalid,
}

pub(super) fn parse_command(input: &str) -> Command {
    match input.trim() {
        "1" => Command::ListFiles,
        "2" => Command::LoadFile,
        "3" => Command::ChangeDirectory,
        "4" => Command::AskQuestion,
        "5" => Command::ShowHistory,
        "0" => Command::Exit,
        _ => Command::Invalid,
    }
}

pub(super) async fn execute_command(
    command: Command,
    chat_context: &mut ChatContext,
) -> Result<()> {
    match command {
        Command::ListFiles => handle_list_files(chat_context).await?,
        Command::LoadFile => handle_load_file(chat_context).await?,
        Command::ChangeDirectory => handle_change_directory(chat_context).await?,
        Command::AskQuestion => handle_ask_question(chat_context).await?,
        Command::ShowHistory => handle_show_history(chat_context).await?,
        Command::Exit => return Err(crate::error::Error::Exit),
        Command::Invalid => println!("Invalid command!"),
    }
    Ok(())
}

pub(super) async fn handle_list_files(chat_context: &mut ChatContext) -> Result<()> {
    println!("\nAvailable files:");
    let (files, dirs) = chat_context.files()?;

    println!("\ndirs");
    for dir in &dirs {
        println!("- {}", dir);
    }

    println!("\nfiles");
    for file in &files {
        println!("- {}", file);
    }

    Ok(())
}

pub(super) async fn handle_load_file(chat_context: &mut ChatContext) -> Result<()> {
    print!("Enter filename to load: ");
    io::stdout().flush()?;
    let mut filename = String::new();
    io::stdin().read_line(&mut filename)?;
    match chat_context.load_context_from_file(filename.trim()) {
        Ok(_) => println!("File loaded successfully!"),
        Err(e) => println!("Error loading file: {}", e),
    }

    Ok(())
}

pub(super) async fn handle_change_directory(chat_context: &mut ChatContext) -> Result<()> {
    print!("Enter directory name (or .. for parent): ");
    io::stdout().flush()?;
    let mut dirname = String::new();
    io::stdin().read_line(&mut dirname)?;
    match chat_context.set_new_dir(dirname.trim()) {
        Ok(_) => println!("Changed directory successfully!"),
        Err(e) => println!("Error changing directory: {}", e),
    }
    Ok(())
}

pub(super) async fn handle_ask_question(chat_context: &mut ChatContext) -> Result<()> {
    print!("Enter your question: ");
    io::stdout().flush()?;
    let mut question = String::new();
    io::stdin().read_line(&mut question)?;
    let question = question.trim().to_string();

    let context = chat_context.add_conv_context(&question);
    let response = chat_context.send_to_api(&context).await?;
    chat_context.save_response(question, response.clone());
    println!("\nResponse: {}", response);

    Ok(())
}

pub(super) async fn handle_show_history(chat_context: &mut ChatContext) -> Result<()> {
    println!("\nConversation history:");
    for (i, (question, answer)) in chat_context.conversation_history.iter().enumerate() {
        println!("\nQ{}: {}", i + 1, question);
        println!("A{}: {}", i + 1, answer);
    }
    Ok(())
}

pub(super) fn read_input(prompt: &str) -> Result<String> {
    print!("{}", prompt);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

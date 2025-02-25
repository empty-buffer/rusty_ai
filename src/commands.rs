use crate::chat::ChatContext;
use crate::Result;
use std::fmt::{self, write};
use std::io::{self, Write};

use inquire::Select;

#[derive(Clone)]
pub(super) enum Command {
    ListFiles,
    LoadFile,
    ChangeDirectory,
    AskQuestion,
    ShowHistory,
    Exit,
    Back,
    ParentDir,
}

// pub(super) fn parse_command(input: &str) -> Command {
//     match input.trim() {
//         "1" => Command::ListFiles,
//         "2" => Command::LoadFile,
//         "3" => Command::ChangeDirectory,
//         "4" => Command::AskQuestion,
//         "5" => Command::ShowHistory,
//         "0" => Command::Exit,
//         _ => Command::Invalid,
//     }
// }

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Command::ListFiles => write!(f, "List Files"),
            Command::LoadFile => write!(f, "Load File"),
            Command::ChangeDirectory => write!(f, "Change Directory"),
            Command::AskQuestion => write!(f, "Ask Question"),
            Command::ShowHistory => write!(f, "Show History"),
            Command::Exit => write!(f, "Exit"),
            Command::Back => write!(f, "Back"),
            Command::ParentDir => write!(f, "../"),
        }
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
        _ => println!("Invalid command!"),
    }
    Ok(())
}

pub(super) async fn handle_list_files(chat_context: &mut ChatContext) -> Result<()> {
    println!("Available files:");
    let (files, dirs) = chat_context.files()?;

    println!("\nDirs");
    for dir in &dirs {
        println!("- {}", dir);
    }

    println!("\nFiles");
    for file in &files {
        println!("- {}", file);
    }

    println!("\n");

    Ok(())
}

pub(super) async fn handle_load_file(chat_context: &mut ChatContext) -> Result<()> {
    let mut opts = Vec::new();

    let (files, _) = chat_context.files()?;

    for file in files {
        opts.push(file);
    }

    opts.push(Command::Back.to_string());

    let ans = Select::new("What files we should load?", opts.clone())
        .prompt()
        .map_err(|e| {
            println!("Error while selection an option {}", e);
            e
        })?;

    match ans.as_str() {
        "Back" => Ok(()),
        _ => match chat_context.load_context_from_file(&ans) {
            Ok(_) => {
                println!("File loaded successfully!");
                Ok(())
            }
            Err(e) => {
                println!("Error loading file: {}", e);
                Ok(())
            }
        },
    }
}

pub(super) async fn handle_change_directory(chat_context: &mut ChatContext) -> Result<()> {
    let mut opts = Vec::new();

    let (_, dirs) = chat_context.files()?;
    opts.push(Command::ParentDir.to_string());
    for dirname in dirs {
        opts.push(dirname);
    }

    opts.push(Command::Back.to_string());

    let ans = Select::new("Please select directory", opts.clone())
        .prompt()
        .map_err(|e| {
            println!("Error while selection an option {}", e);
            e
        })?;

    match ans.as_str() {
        "Back" => Ok(()),
        _ => match chat_context.set_new_dir(&ans) {
            Ok(_) => {
                println!("Changed directory successfully!");
                Ok(())
            }
            Err(e) => {
                println!("Error changing directory: {}", e);
                Ok(())
            }
        },
    }

    // match chat_context.set_new_dir(ans.as_str()) {
    //     Ok(_) => println!("Changed directory successfully!"),
    //     Err(e) => println!("Error changing directory: {}", e),
    // }
    // Ok(())
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

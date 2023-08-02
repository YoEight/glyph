use clap::{Parser, Subcommand};
use glyph::{in_memory_inputs, Input, Options};

#[derive(Parser, Debug)]
#[command(name = "clap")]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Connect to something
    Connect { node: String },
}

fn main() -> std::io::Result<()> {
    let options = Options::default()
        .author("Yo Eight")
        .version("1.2.3")
        .date("July, 28th 2023")
        .command_prompt("run");

    let mut inputs = in_memory_inputs(options)?;

    while let Some(input) = inputs.next_input_with_parser::<Cli>()? {
        match input {
            Input::Exit => break,
            Input::String(c) => {
                println!(">>> {:?}", c)
            }
            Input::Command(c) => {
                println!(">>> {:?}", c)
            }
        }
    }

    Ok(())
}

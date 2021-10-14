use glyph::{in_memory_inputs, Input, Options};

fn main() -> std::io::Result<()> {
    let options = Options::default();
    let mut inputs = in_memory_inputs(options)?;

    while let Some(input) = inputs.next_input()? {
        if let Input::Exit = input {
            break;
        }

        println!(">>> {:?}", input)
    }

    Ok(())
}

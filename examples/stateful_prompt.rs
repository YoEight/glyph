use glyph::{in_memory_inputs, Input, Options, PromptOptions};

fn main() -> std::io::Result<()> {
    let options = Options::default()
        .header(include_str!("./header.txt"))
        .author("Yo Eight")
        .version("1.2.3")
        .date("June, 16th 2023")
        .disable_free_expression();

    let mut inputs = in_memory_inputs(options)?;
    let mut round = 0;
    let mut prompt = "ping";
    while let Some(input) =
        inputs.next_input_with_options(&PromptOptions::default().prompt(prompt))?
    {
        round += 1;
        prompt = if round % 2 == 0 { "ping" } else { "pong" };

        if let Input::Exit = input {
            break;
        }

        println!(">>> {:?}", input)
    }

    Ok(())
}

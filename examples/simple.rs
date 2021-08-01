use glyph::{Input, Inputs};
fn main() {
    for input in Inputs::new() {
        if let Input::Exit = input {
            break;
        }

        println!(">>> {:?}", input)
    }
}

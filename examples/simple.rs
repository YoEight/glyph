use glyph::{Input, Inputs, Options};
fn main() {
    let options = Options::default();
    for input in Inputs::new(options) {
        if let Input::Exit = input {
            break;
        }

        println!(">>> {:?}", input)
    }
}

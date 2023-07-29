use clap::Parser;

#[derive(Parser, Debug)]
pub struct Params {
    values: Vec<String>,
}

impl Params {
    pub fn new(values: Vec<String>) -> Self {
        Self { values }
    }

    pub fn values(self) -> Vec<String> {
        self.values
    }
}

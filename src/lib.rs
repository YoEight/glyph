pub(crate) mod history;
mod input;
mod persistence;

pub use input::{
    file_backed_inputs, in_memory_inputs, params::Params, Input, Inputs, Options, PromptOptions,
};
pub use persistence::{FileBackend, Noop};

pub type FileBackedInputs = Inputs<FileBackend>;
pub type MemoryBackedInputs = Inputs<Noop>;

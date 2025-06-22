use std::fmt::{Display, Formatter};

#[derive(Debug, Clone)]
pub struct ChunkError {
    message: String
}
impl ChunkError {
    pub fn new(message: &str) -> Self {
        Self {
            message: String::from(message)
        }
    }
}
impl Display for ChunkError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Chunk Error: {}", self.message)
    }
}
impl std::error::Error for ChunkError {}


#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("Registry {1}: Value {0} is already registered!")]
    Duplicate(String, String),
    #[error("Registry {0}: Cannot write to frozen registry!")]
    Frozen(String),
}

#[derive(Debug, thiserror::Error)]
pub enum BlockStateError {
    #[error("Invalid block id: {0}")]
    InvalidId(String)
}
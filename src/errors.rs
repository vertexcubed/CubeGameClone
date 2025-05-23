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
    #[error("Value {0} already registered as {1}")]
    Duplicate(String, String),
    #[error("{0} registry is frozen.")]
    Frozen(String),
}
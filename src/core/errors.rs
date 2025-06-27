use bevy::prelude::IVec3;
use std::fmt::Display;

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("Registry {1}: Value {0} is already registered!")]
    Duplicate(String, String),
    #[error("Registry {0}: Cannot write to frozen registry!")]
    Frozen(String),
}

#[derive(Debug, thiserror::Error)]
pub enum BlockStateError {
    #[error("Invalid block id: {0}.")]
    InvalidId(String)
}

#[derive(Debug, thiserror::Error)]
pub enum BlockModelError {
    #[error("Invalid face: {0}.")]
    InvalidFace(String),
    #[error("Could not find texture with key {0}.")]
    KeyNotFound(String),
    #[error("Texture {0} not found in block array texture.")]
    TextureNotFound(String),
    #[error("Cannot depend on model {0}, as this creates a circular dependency.")]
    CircularDependency(String)
}

#[derive(Debug, thiserror::Error)]
pub enum ChunkError {
    #[error("Chunk {0} is not yet initialized, and can't be read/written from.")]
    Uninitialized(IVec3),
    #[error("Point {0} is out of chunk bounds.")]
    OutOfBounds(IVec3),
    #[error("Chunk {0} is already initialized.")]
    AlreadyInitialized(IVec3),
    #[error("Chunk {0} already exists.")]
    DuplicateChunk(IVec3)
}

#[derive(Debug, thiserror::Error)]
pub enum WorldError {
    #[error("Chunk error: {0}")]
    Chunk(#[from] ChunkError),
    #[error("Chunk {0} is not loaded.")]
    UnloadedChunk(IVec3)
}
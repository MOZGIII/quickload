//! A notion of chunks and related utilities for quickload.

pub mod linear;

/// A chunk of data sized by `T`.
#[derive(Debug, PartialEq, Eq)]
pub struct Chunk<T> {
    /// Start of the chunk.
    pub start: T,
    /// End of the chunk.
    pub end: T,
}

impl<T> Chunk<T> {
    /// Consume the chunk and return the start and the end positions.
    pub fn into_inner(self) -> (T, T) {
        (self.start, self.end)
    }
}

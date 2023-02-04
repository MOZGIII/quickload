//! A notion of chunks and related utilities for quickload.

use std::{fmt::Debug, num::NonZeroU64};

mod utils;

/// The type for a size in the bytes space.
pub type Size = u64;
/// The type for a size in the bytes space, for when the zero size is not acceptable.
pub type NonZeroSize = NonZeroU64;
/// The type for an offset in the bytes space.
pub type Offset = u64;

/// The type for the total size of the bytes space.
pub type TotalSize = Size;
/// The type for a size of a signle chunk.
pub type ChunkSize = NonZeroSize;
/// The type for a size of the chunk index.
pub type ChunkIndex = u64;

/// The chunker can split an abstract linear bytes space into equally-sized
/// (except for the last one) chunks.
#[derive(Debug)]
pub struct Chunker {
    /// The total size of the bytes space.
    pub total_size: TotalSize,
    /// The size of a chunk.
    pub chunk_size: ChunkSize,
}

/// An error when calculating the chunk offset.
#[derive(Debug)]
pub enum ChunkOffsetError {
    /// The data is empty, so no valid offsets exist.
    EmptyData,
    /// The offset has overflowed its type bound.
    OffsetOverflow,
    /// The offset is greater than the last valid offset of the data.
    OutOfDataBounds(Offset),
}

impl Chunker {
    /// Capture and return the chunk with a given index.
    pub fn capture_chunk(
        &self,
        chunk_index: ChunkIndex,
    ) -> Result<CapturedChunk, ChunkOffsetError> {
        let (first_byte_offset, last_byte_offset) = self.chunk_offsets(chunk_index)?;
        Ok(CapturedChunk {
            index: chunk_index,
            first_byte_offset,
            last_byte_offset,
        })
    }

    /// The pair of the first and last byte offsets of the chunk with a given
    /// index.
    pub fn chunk_offsets(
        &self,
        chunk_index: ChunkIndex,
    ) -> Result<(Offset, Offset), ChunkOffsetError> {
        let last_valid_offset =
            utils::last_valid_offset(self.total_size).ok_or(ChunkOffsetError::EmptyData)?;

        let first_byte_offset = chunk_index * self.chunk_size.get();
        if !utils::is_within_bound(first_byte_offset, last_valid_offset) {
            return Err(ChunkOffsetError::OutOfDataBounds(first_byte_offset));
        }

        let last_byte_offset =
            utils::first_to_last_byte_of_chunk(first_byte_offset, self.chunk_size);
        let last_byte_offset =
            utils::bounded(last_byte_offset, last_valid_offset).unwrap_or(last_valid_offset);

        Ok((first_byte_offset, last_byte_offset))
    }
}

/// The captured chunk information.
///
/// Useful for when you need to compute the offsets just once and cache them.
#[derive(Debug, Clone, Copy)]
pub struct CapturedChunk {
    /// The index of the chunk.
    pub index: ChunkIndex,
    /// The first byte offset of this chunk.
    pub first_byte_offset: Offset,
    /// The last byte offset of this chunk.
    pub last_byte_offset: Offset,
}

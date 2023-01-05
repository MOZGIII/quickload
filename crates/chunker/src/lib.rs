//! A notion of chunks and related utilities for quickload.

use std::fmt::Debug;

mod traits;

use self::traits::{BoundedBy, FirstToLastByteOfChunk, LastValidOffset, OffsetFromChunkSize};

/// A chunker config.
pub trait Config {
    /// The index of the chunk.
    type ChunkIndex: Debug + Copy + OffsetFromChunkSize<Self::ChunkSize, Self::Offset>;
    /// The total size of the data.
    /// Can be zero.
    type TotalSize: Debug + Copy + LastValidOffset<Self::Offset>;
    /// The size of a single chunk.
    /// Must be non-zero.
    type ChunkSize: Debug + Copy + nonzero_ext::NonZero;
    /// The offset into the data.
    /// The sensible values range is `[0, total_size)`,
    /// meaning the `total_size` is an illegal value.
    type Offset: Debug + Copy + BoundedBy<Self::Offset> + FirstToLastByteOfChunk<Self::ChunkSize>;
}

/// The chunker can split an abstract linear bytes space into equally-sized
/// (except for the last one) chunks.
#[derive(Debug)]
pub struct Chunker<C: Config> {
    /// The total size of the bytes space.
    pub total_size: C::TotalSize,
    /// The size of a chunk.
    pub chunk_size: C::ChunkSize,
}

/// An error when calculating the chunk offset.
#[derive(Debug)]
pub enum ChunkOffsetError<C: Config> {
    /// The data is empty, so no valid offsets exist.
    EmptyData,
    /// The offset has overflowed its type bound.
    OffsetOverflow,
    /// The offset is greater than the last valid offset of the data.
    OutOfDataBounds(C::Offset),
}

impl<C: Config> Chunker<C> {
    /// Capture and return the chunk with a given index.
    pub fn capture_chunk(
        &self,
        chunk_index: C::ChunkIndex,
    ) -> Result<CapturedChunk<C>, ChunkOffsetError<C>> {
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
        chunk_index: C::ChunkIndex,
    ) -> Result<(C::Offset, C::Offset), ChunkOffsetError<C>> {
        let last_valid_offset: C::Offset = self
            .total_size
            .last_valid_offset()
            .ok_or(ChunkOffsetError::EmptyData)?;

        let first_byte_offset: C::Offset = chunk_index.offset_from_chunk_size(self.chunk_size);
        if !first_byte_offset.is_within_bound(&last_valid_offset) {
            return Err(ChunkOffsetError::OutOfDataBounds(first_byte_offset));
        }

        let last_byte_offset: C::Offset =
            first_byte_offset.first_to_last_byte_of_chunk(self.chunk_size);
        let last_byte_offset = last_byte_offset
            .bounded(&last_valid_offset)
            .unwrap_or(last_valid_offset);

        Ok((first_byte_offset, last_byte_offset))
    }
}

/// The captured chunk information.
///
/// Useful for when you need to compute the offsets just once and cache them.
#[derive(Debug, Clone, Copy)]
pub struct CapturedChunk<C: Config> {
    /// The index of the chunk.
    pub index: C::ChunkIndex,
    /// The first byte offset of this chunk.
    pub first_byte_offset: C::Offset,
    /// The last byte offset of this chunk.
    pub last_byte_offset: C::Offset,
}

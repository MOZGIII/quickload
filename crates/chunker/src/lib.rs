//! A notion of chunks and related utilities for quickload.

use std::fmt::Debug;

mod utils;

use derivative::Derivative;
use utils::{from_usize, to_usize, UsizeInterop};

/// A chunker config.
pub trait Config {
    /// The type used as a metric in the chunks space.
    /// Used for chunk indexes and lengthes.
    type ChunkSpaceUnit: Debug + Copy + UsizeInterop;
    /// The type used as a metric in the byte space.
    /// Used for sizes and offsets.
    type ByteSpaceUnit: Debug + Copy + UsizeInterop;
}

/// The chunker can split an abstract linear bytes space into equally-sized
/// (except for the last one) chunks.
#[derive(Debug)]
pub struct Chunker<C: Config> {
    /// The total size of the bytes space.
    pub total_size: C::ByteSpaceUnit,
    /// The size of a chunk.
    pub chunk_size: C::ByteSpaceUnit,
}

/// An error when calculating the chunk offset.
#[derive(Debug)]
pub enum ChunkOffsetError<C: Config> {
    /// The data is empty, so no valid offsets exist.
    EmptyData,
    /// The offset has overflowed its type bound.
    OffsetOverflow,
    /// The offset is greater than the last valid offset of the data.
    OutOfDataBounds(C::ByteSpaceUnit),
}

impl<C: Config> Chunker<C> {
    /// Capture and return the chunk with a given index.
    pub fn capture_chunk(
        &self,
        chunk_index: C::ChunkSpaceUnit,
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
        chunk_index: C::ChunkSpaceUnit,
    ) -> Result<(C::ByteSpaceUnit, C::ByteSpaceUnit), ChunkOffsetError<C>> {
        let last_valid_offset = utils::last_valid_offset(to_usize(self.total_size))
            .ok_or(ChunkOffsetError::EmptyData)?;

        let first_byte_offset = to_usize(chunk_index) * to_usize(self.chunk_size);
        if !utils::is_within_bound(first_byte_offset, last_valid_offset) {
            return Err(ChunkOffsetError::OutOfDataBounds(from_usize(
                first_byte_offset,
            )));
        }

        let last_byte_offset =
            utils::first_to_last_byte_of_chunk(first_byte_offset, to_usize(self.chunk_size));
        let last_byte_offset =
            utils::bounded(last_byte_offset, last_valid_offset).unwrap_or(last_valid_offset);

        Ok((from_usize(first_byte_offset), from_usize(last_byte_offset)))
    }
}

/// The captured chunk information.
///
/// Useful for when you need to compute the offsets just once and cache them.
#[derive(Derivative)]
#[derivative(Debug, Clone, Copy)]
pub struct CapturedChunk<C: Config> {
    /// The index of the chunk.
    pub index: C::ChunkSpaceUnit,
    /// The first byte offset of this chunk.
    pub first_byte_offset: C::ByteSpaceUnit,
    /// The last byte offset of this chunk.
    pub last_byte_offset: C::ByteSpaceUnit,
}

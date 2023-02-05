//! Utility fns.

use crate::{ChunkSize, Offset, TotalSize};

/// Check if the value is within the bound (less or equal).
pub fn is_within_bound<T: std::cmp::PartialOrd>(what: T, bound: T) -> bool {
    what <= bound
}

/// Return the value if it is bounded by the bound.
pub fn bounded<T: std::cmp::PartialOrd + Copy>(what: T, bound: T) -> Option<T> {
    if is_within_bound(what, bound) {
        return Some(what);
    }
    None
}

/// Get the last valid offset in the space of the given size.
pub fn last_valid_offset(total_size: TotalSize) -> Option<Offset> {
    total_size.checked_sub(1)
}

/// Get last byte of the chunk from the first byte of the chunk and the chunk size.
pub fn first_to_last_byte_of_chunk(first_byte_offset: Offset, chunk_size: ChunkSize) -> Offset {
    chunk_size.get() - 1 + first_byte_offset
}

//! Utility traits.

use nonzero_ext::NonZero;

/// Check that the value is bound by a certain bound.
/// `Bound` can be a different type than `Self`.
pub trait BoundedBy<Bound>: Sized + std::cmp::PartialOrd<Bound> {
    /// Is the `self` value within the `bound`?
    fn is_within_bound(&self, bound: &Bound) -> bool;

    /// Return the `self` if within the `bound`, or `None` if outside.
    fn bounded(self, bound: &Bound) -> Option<Self>;
}

impl<T: std::cmp::PartialOrd<Bound>, Bound> BoundedBy<Bound> for T {
    fn is_within_bound(&self, bound: &Bound) -> bool {
        self <= bound
    }

    fn bounded(self, bound: &Bound) -> Option<Self> {
        if self.is_within_bound(bound) {
            return Some(self);
        }
        None
    }
}

/// Compute the last valid offset of the data, meaning the offset to
/// the very last byte of the data.
/// Assumed to be implemented for a type representing the data total size.
pub trait LastValidOffset<Offset: Sized>
where
    Self: num_traits::One + num_traits::CheckedSub + Into<Offset>,
{
    /// The offset to the last byte in the data of the `self` size.
    fn last_valid_offset(&self) -> Option<Offset>;
}

impl<TotalSize, Offset: Sized> LastValidOffset<Offset> for TotalSize
where
    Self: num_traits::One + num_traits::CheckedSub + Into<Offset>,
{
    fn last_valid_offset(&self) -> Option<Offset> {
        let one: TotalSize = num_traits::one();
        let total_size_minus_one = self.checked_sub(&one)?;
        Some(total_size_minus_one.into())
    }
}

/// Compute an offset in the data from the chunk index value (`self`) and a chunk size.
/// Assumed to be implemented on the type representing a chunk index.
pub trait OffsetFromChunkSize<ChunkSize: NonZero, Offset>:
    std::ops::Mul<ChunkSize::Primitive, Output = Offset>
{
    /// Compute the offset of this chunk index with a given chunk size.
    fn offset_from_chunk_size(self, chunk_size: ChunkSize) -> Offset;
}

impl<Offset, ChunkIndex, ChunkSize: NonZero> OffsetFromChunkSize<ChunkSize, Offset> for ChunkIndex
where
    Self: std::ops::Mul<ChunkSize::Primitive, Output = Offset>,
{
    fn offset_from_chunk_size(self, chunk_size: ChunkSize) -> Offset {
        self * chunk_size.get()
    }
}

/// Given the offet to the first byte of the chunk and the chunk size,
/// compute the offset to the last byte in the chunk.
/// Assumed to be implemented on the type represeting an offset in the data.
pub trait FirstToLastByteOfChunk<ChunkSize: NonZero>:
    std::ops::Add<ChunkSize::Primitive, Output = Self>
    + num_traits::One
    + std::ops::Sub<Self, Output = Self>
{
    /// Given an offset to the first byte of the chunk (`self`) and a chunk size,
    /// compute an offset to the last byte of the chunk.
    fn first_to_last_byte_of_chunk(self, chunk_size: ChunkSize) -> Self;
}

impl<Offset, ChunkSize: NonZero> FirstToLastByteOfChunk<ChunkSize> for Offset
where
    Self: std::ops::Add<ChunkSize::Primitive, Output = Self>
        + num_traits::One
        + std::ops::Sub<Self, Output = Self>,
{
    fn first_to_last_byte_of_chunk(self, chunk_size: ChunkSize) -> Self {
        let one = num_traits::one();
        self + chunk_size.get() - one
    }
}

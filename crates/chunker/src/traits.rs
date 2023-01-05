//! Utility traits.

use nonzero_ext::NonZero;

pub trait BoundedBy<Bound>: Sized + std::cmp::PartialOrd<Bound> {
    fn is_within_bound(&self, bound: &Bound) -> bool;
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

pub trait LastValidOffset<Offset: Sized>
where
    Self: num_traits::One + num_traits::CheckedSub + Into<Offset>,
{
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

/// For implementing on the chunk index.
pub trait OffsetFromChunkSize<ChunkSize: NonZero, Offset>:
    std::ops::Mul<ChunkSize::Primitive, Output = Offset>
{
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

/// For implementing on the offset.
pub trait FirstToLastByteOfChunk<ChunkSize: NonZero>:
    std::ops::Add<ChunkSize::Primitive, Output = Self>
    + num_traits::One
    + std::ops::Sub<Self, Output = Self>
{
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

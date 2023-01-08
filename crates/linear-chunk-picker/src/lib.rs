//! Linear chunk picker.

use quickload_chunker::{CapturedChunk, Chunker, Config};

mod traits;

use self::traits::CheckedIncrement;

/// The linear chunk picker strategy.
///
/// Selects the chunks one by one from the beginning of the range to the end.
#[derive(Debug)]
pub struct Linear<'a, C: Config> {
    /// The chunker to use for generating chunks.
    chunker: &'a Chunker<C>,
    /// The index of the next chunk.
    index: C::ChunkIndex,
}

impl<'a, C: Config> Linear<'a, C>
where
    C::ChunkIndex: num_traits::Zero,
{
    /// Create a new [`Linear`].
    pub fn new(chunker: &'a Chunker<C>) -> Self {
        Self {
            chunker,
            index: num_traits::zero(),
        }
    }
}

impl<'a, C: Config> Iterator for Linear<'a, C>
where
    C::ChunkIndex: Copy + crate::traits::CheckedIncrement,
{
    type Item = CapturedChunk<C>;

    fn next(&mut self) -> Option<Self::Item> {
        let chunk = self.chunker.capture_chunk(self.index).ok()?;
        self.index = self.index.checked_inc()?;
        Some(chunk)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::erasing_op, clippy::identity_op)]

    use super::Linear;
    use crate::{Chunker, Config};
    use pretty_assertions::assert_eq;
    use std::num::NonZeroU64;

    const CHUNK_SIZE: u64 = 512 * 1024;

    #[derive(Debug)]
    struct TestConfig;

    impl Config for TestConfig {
        type TotalSize = u64;
        type ChunkSize = NonZeroU64;
        type Offset = u64;
        type ChunkIndex = u64;
    }

    fn test(chunker: Chunker<TestConfig>, expected: &[(u64, u64)]) {
        let picker = Linear::new(&chunker);
        let actual: Vec<(u64, u64)> = picker
            .map(|chunk| (chunk.first_byte_offset, chunk.last_byte_offset))
            .collect();

        assert_eq!(actual, expected, "results don't match, expected is right");
    }

    fn make_chunker(total_size: u64) -> Chunker<TestConfig> {
        Chunker {
            total_size,
            chunk_size: CHUNK_SIZE.try_into().unwrap(),
        }
    }

    #[test]
    fn empty() {
        test(make_chunker(0), &[]);
    }

    #[test]
    fn one_byte() {
        test(make_chunker(1), &[(0, 0)]);
    }

    #[test]
    fn two_bytes() {
        test(make_chunker(2), &[(0, 1)]);
    }

    #[test]
    fn three_bytes() {
        test(make_chunker(3), &[(0, 2)]);
    }

    #[test]
    fn four_bytes() {
        test(make_chunker(4), &[(0, 3)]);
    }

    #[test]
    fn full() {
        test(make_chunker(CHUNK_SIZE), &[(0, CHUNK_SIZE - 1)]);
    }

    #[test]
    fn double() {
        test(
            make_chunker(CHUNK_SIZE * 2),
            &[(0, CHUNK_SIZE - 1), (CHUNK_SIZE, CHUNK_SIZE * 2 - 1)],
        );
    }

    #[test]
    fn triple() {
        test(
            make_chunker(CHUNK_SIZE * 3),
            &[
                (CHUNK_SIZE * 0, CHUNK_SIZE * 1 - 1),
                (CHUNK_SIZE * 1, CHUNK_SIZE * 2 - 1),
                (CHUNK_SIZE * 2, CHUNK_SIZE * 3 - 1),
            ],
        );
    }
}

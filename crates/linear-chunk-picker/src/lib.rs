//! Linear chunk picker.

use quickload_chunker::{CapturedChunk, ChunkIndex, Chunker};

/// The linear chunk picker strategy.
///
/// Selects the chunks one by one from the beginning of the range to the end.
#[derive(Debug)]
pub struct Linear<'a> {
    /// The chunker to use for generating chunks.
    chunker: &'a Chunker,
    /// The index of the next chunk.
    index: ChunkIndex,
}

impl<'a> Linear<'a> {
    /// Create a new [`Linear`].
    pub fn new(chunker: &'a Chunker) -> Self {
        Self { chunker, index: 0 }
    }
}

impl<'a> Iterator for Linear<'a> {
    type Item = CapturedChunk;

    fn next(&mut self) -> Option<Self::Item> {
        let chunk = self.chunker.capture_chunk(self.index).ok()?;
        self.index += 1;
        Some(chunk)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::erasing_op, clippy::identity_op)]

    use super::Linear;
    use crate::Chunker;
    use pretty_assertions::assert_eq;

    const CHUNK_SIZE: u64 = 512 * 1024;

    fn test(chunker: Chunker, expected: &[(u64, u64)]) {
        let picker = Linear::new(&chunker);
        let actual: Vec<(u64, u64)> = picker
            .map(|chunk| (chunk.first_byte_offset, chunk.last_byte_offset))
            .collect();

        assert_eq!(actual, expected, "results don't match, expected is right");
    }

    fn make_chunker(total_size: u64) -> Chunker {
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

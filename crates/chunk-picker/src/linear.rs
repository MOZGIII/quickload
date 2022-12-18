//! Linear picker.

use crate::Chunk;
use std::{cmp::min, num::NonZeroU64};

/// The linear chunk picker strategy.
///
/// Selects the chunks one by one from the beginning of the range to the end.
pub struct Linear {
    total_size: u64,
    chunk_size: NonZeroU64,
    position: u64,
}

impl Linear {
    /// Create a new [`Linear`].
    pub fn new(total_size: u64, chunk_size: NonZeroU64) -> Self {
        Self {
            total_size,
            chunk_size,
            position: 0,
        }
    }
}

impl Iterator for Linear {
    type Item = Chunk<u64>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.position >= self.total_size {
            return None;
        }

        let next_position = min(self.position + u64::from(self.chunk_size), self.total_size);

        let start = self.position;
        let end = next_position - 1;

        self.position = next_position;

        Some(Chunk { start, end })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::erasing_op, clippy::identity_op)]

    use super::Linear;
    use crate::Chunk;
    use pretty_assertions::assert_eq;
    use std::num::NonZeroU64;

    const CHUNK_SIZE: u64 = 512 * 1024;

    fn test(picker: impl Iterator<Item = Chunk<u64>>, expected: &[(u64, u64)]) {
        assert_eq!(
            picker.collect::<Vec<_>>(),
            expected
                .iter()
                .map(|(start, end)| Chunk {
                    start: *start,
                    end: *end
                })
                .collect::<Vec<_>>(),
            "results don't match"
        );
    }

    fn make_linear(size: u64) -> Linear {
        Linear::new(size, NonZeroU64::new(CHUNK_SIZE).unwrap())
    }

    #[test]
    fn empty() {
        test(make_linear(0), &[]);
    }

    #[test]
    fn one_byte() {
        test(make_linear(1), &[(0, 0)]);
    }

    #[test]
    fn two_bytes() {
        test(make_linear(2), &[(0, 1)]);
    }

    #[test]
    fn three_bytes() {
        test(make_linear(3), &[(0, 2)]);
    }

    #[test]
    fn four_bytes() {
        test(make_linear(4), &[(0, 3)]);
    }

    #[test]
    fn full() {
        test(make_linear(CHUNK_SIZE), &[(0, CHUNK_SIZE - 1)]);
    }

    #[test]
    fn double() {
        test(
            make_linear(CHUNK_SIZE * 2),
            &[(0, CHUNK_SIZE - 1), (CHUNK_SIZE, CHUNK_SIZE * 2 - 1)],
        );
    }

    #[test]
    fn triple() {
        test(
            make_linear(CHUNK_SIZE * 3),
            &[
                (CHUNK_SIZE * 0, CHUNK_SIZE * 1 - 1),
                (CHUNK_SIZE * 1, CHUNK_SIZE * 2 - 1),
                (CHUNK_SIZE * 2, CHUNK_SIZE * 3 - 1),
            ],
        );
    }
}

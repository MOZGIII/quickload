//! The progress state.

use std::collections::BTreeMap;

use quickload_chunker::{Offset, Size};

/// The progress state.
#[derive(Debug, Default)]
pub struct ProgressState {
    /// A tree with the offset start as the key.
    tree: BTreeMap<Offset, Offset>,
}

impl ProgressState {
    /// Merge the incoming update in.
    pub fn merge_update(&mut self, update: Range) {
        if update.start == update.end {
            return;
        }

        let mut old_tree = BTreeMap::new();
        std::mem::swap(&mut self.tree, &mut old_tree);

        let mut update = update;
        for current in old_tree {
            let current = Range::from(current);

            match current.merge(&update) {
                Some(merged) => update = merged,
                None => {
                    debug_assert!(self.tree.insert(current.start, current.end).is_none());
                }
            }
        }
        debug_assert!(self.tree.insert(update.start, update.end).is_none());
    }

    /// Get all the progress ranges.
    pub fn ranges(&self) -> impl Iterator<Item = Range> + '_ {
        self.tree.iter().map(|(start, end)| Range {
            start: *start,
            end: *end,
        })
    }

    /// The collective size of all progress ranges.
    pub fn total(&self) -> Size {
        self.tree.iter().map(|(start, end)| end - start).sum()
    }
}

/// The range.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Range {
    /// Start of the progress.
    /// Points to the first byte that the operation started progress at.
    pub start: Offset,
    /// End of the progress.
    /// Points to the _position after the last byte_ the operation has completed the progress at.
    /// Effectively, this is `start_offset + data_size`, not `start_offset + data_size - 1`!
    pub end: Offset,
}

impl Range {
    /// True if the value if within the range.
    pub fn includes(&self, offset: Offset) -> bool {
        self.start <= offset && offset <= self.end
    }

    /// Merge the two ranges together.
    pub fn merge(&self, other: &Range) -> Option<Range> {
        let self_includes_other_start = self.includes(other.start);
        let self_includes_other_end = self.includes(other.end);
        match (self_includes_other_start, self_includes_other_end) {
            (false, false) => {
                // no overlap, just return the two values, in order
                None
            }
            (true, true) => {
                // the `other` range is entirely contained within `self` range
                Some(self.clone())
            }
            (true, false) => {
                // the `other` range starts within `self` range, but exceeds it's end
                let merged = Range {
                    start: self.start,
                    end: other.end,
                };
                Some(merged)
            }
            (false, true) => {
                // the `other` range starts before `self` range, but ends within `self` range
                let merged = Range {
                    start: other.start,
                    end: self.end,
                };
                Some(merged)
            }
        }
    }
}

impl From<(Offset, Offset)> for Range {
    fn from(value: (Offset, Offset)) -> Self {
        let (start, end) = value;
        Self { start, end }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn routine<Sample: Into<Range>>(
        samples: impl IntoIterator<Item = Sample>,
        expected: impl IntoIterator<Item = Sample>,
    ) {
        let mut state = ProgressState::default();

        for sample in samples {
            state.merge_update(sample.into());
        }

        let actual = state.ranges();
        let expected = expected.into_iter().map(Into::into);

        let actual: Vec<_> = actual.collect();
        let expected: Vec<_> = expected.collect();

        assert_eq!(actual, expected);
    }

    #[test]
    fn simple() {
        let samples = [(0, 1)];
        let expected = [(0, 1)];
        routine(samples, expected);
    }

    #[test]
    fn separate() {
        let samples = [(0, 1), (3, 4)];
        let expected = [(0, 1), (3, 4)];
        routine(samples, expected);
    }

    #[test]
    fn back_to_back() {
        // init -> ____
        // #___ -> #___
        // __#_ -> #_#_
        let samples = [(0, 1), (2, 3)];
        let expected = [(0, 1), (2, 3)];
        routine(samples, expected);
    }

    #[test]
    fn back_to_back_reverse() {
        let samples = [(2, 3), (0, 1)];
        let expected = [(0, 1), (2, 3)];
        routine(samples, expected);
    }

    #[test]
    fn overlap() {
        let samples = [(0, 1), (1, 2)];
        let expected = [(0, 2)];
        routine(samples, expected);
    }

    #[test]
    fn same() {
        let samples = [(0, 1), (0, 1)];
        let expected = [(0, 1)];
        routine(samples, expected);
    }

    #[test]
    fn same_twice() {
        let samples = [(0, 1), (0, 1), (0, 1)];
        let expected = [(0, 1)];
        routine(samples, expected);
    }

    #[test]
    fn multi_overlap() {
        let samples = [(0, 1), (3, 4), (0, 4)];
        let expected = [(0, 4)];
        routine(samples, expected);
    }

    #[test]
    fn zero() {
        let samples = [(0, 0), (1, 1)];
        let expected = [];
        routine(samples, expected);
    }
}

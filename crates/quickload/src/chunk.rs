#[derive(Debug, PartialEq, Eq)]
pub struct Chunk<T> {
    pub start: T,
    pub end: T,
}

impl<T> Chunk<T> {
    pub fn into_inner(self) -> (T, T) {
        (self.start, self.end)
    }
}

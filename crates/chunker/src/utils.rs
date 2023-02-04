//! Utility fns.

/// Two-way conversion to the usize
pub trait UsizeInterop: TryInto<usize> + TryFrom<usize> {}

impl<T: TryInto<usize> + TryFrom<usize>> UsizeInterop for T {}

/// Convert the value to usize or panic.
pub fn to_usize<T>(val: T) -> usize
where
    T: TryInto<usize>,
{
    match val.try_into() {
        Ok(v) => v,
        Err(_) => panic!("unable to convert value to usize"),
    }
}

/// Convert the value from usize or panic.
pub fn from_usize<T>(val: usize) -> T
where
    usize: TryInto<T>,
{
    match val.try_into() {
        Ok(v) => v,
        Err(_) => panic!("unable to convert value from usize"),
    }
}

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
pub fn last_valid_offset(size: usize) -> Option<usize> {
    size.checked_sub(1)
}

/// Get last byte of the chunk from the first byte of the chunk and the chunk size.
pub fn first_to_last_byte_of_chunk(first_byte_offset: usize, chunk_size: usize) -> usize {
    first_byte_offset + chunk_size - 1
}

//! Utility traits.

/// Adds the ability to an increment that checks for overflow.
pub trait CheckedIncrement: Sized + num_traits::One + num_traits::CheckedAdd {
    /// Increment value and return it, or return `None` on overflow.
    fn checked_inc(&self) -> Option<Self>;
}

impl<T> CheckedIncrement for T
where
    T: num_traits::One + num_traits::CheckedAdd,
{
    fn checked_inc(&self) -> Option<Self> {
        let one: Self = num_traits::one();
        one.checked_add(self)
    }
}

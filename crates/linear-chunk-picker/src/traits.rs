//! Utility traits.

pub trait CheckedIncrement: Sized + num_traits::One + num_traits::CheckedAdd {
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

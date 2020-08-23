use std::fs::File;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

pub fn allocate(file: &mut File, offset: u64, len: u64) -> Result<(), anyhow::Error> {
    #[cfg(unix)]
    {
        unix::allocate(file, offset, len)
    }
    #[cfg(windows)]
    {
        windows::allocate(file, offset, len)
    }
}

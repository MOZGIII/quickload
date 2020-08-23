use std::fs::{File, OpenOptions};

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

pub fn prepare_privileges() -> Result<(), anyhow::Error> {
    #[cfg(unix)]
    {
        unix::prepare_privileges()
    }
    #[cfg(windows)]
    {
        windows::prepare_privileges()
    }
}

pub fn open_options() -> OpenOptions {
    #[cfg(unix)]
    {
        unix::open_options()
    }
    #[cfg(windows)]
    {
        windows::open_options()
    }
}

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

use std::fs::File;

#[cfg(all(unix, not(target_os = "macos")))]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(all(unix, target_os = "macos"))]
mod fallback;

#[cfg(all(unix, not(target_os = "macos")))]
use self::unix as implementation;
#[cfg(windows)]
use self::windows as implementation;

#[cfg(all(unix, target_os = "macos"))]
use self::fallback as implementation;

pub fn prepare_privileges() -> Result<(), anyhow::Error> {
    implementation::prepare_privileges()
}

pub fn allocate(file: &mut File, offset: u64, len: u64) -> Result<(), anyhow::Error> {
    implementation::allocate(file, offset, len)
}

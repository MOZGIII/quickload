use nix::fcntl::{fallocate, FallocateFlags};
use std::os::unix::io::AsRawFd;
use std::{convert::TryInto, fs::File};

#[inline]
pub fn allocate(file: &mut File, offset: u64, len: u64) -> Result<(), anyhow::Error> {
    fallocate(
        file.as_raw_fd(),
        // We need an actual allocation (not a hole), and at the same time we
        // down want to make the stale disk content readable for security
        // reasons.
        FallocateFlags::FALLOC_FL_ZERO_RANGE,
        offset.try_into()?,
        len.try_into()?,
    )?;
    Ok(())
}

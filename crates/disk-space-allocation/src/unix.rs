//! Unix-specific implementation.
//! Might not work for all UNIX-es, in that case write another specialized implementation or just
//! use fallback.

use nix::fcntl::{fallocate, FallocateFlags};
use std::{convert::TryInto, fs::File, os::unix::io::AsRawFd};

/// A no-op.
#[inline]
pub(super) fn prepare_privileges() -> Result<(), std::io::Error> {
    // NOOP
    Ok(())
}

/// Allocate the disk space by using [`fallocate`].
#[inline]
pub(super) fn allocate(file: &mut File, len: u64) -> Result<(), std::io::Error> {
    let len = len
        .try_into()
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;
    fallocate(
        file.as_raw_fd(),
        // We need an actual allocation (not a hole), and at the same time we
        // don't want to make the stale disk content readable for security
        // reasons.
        FallocateFlags::FALLOC_FL_ZERO_RANGE,
        0,
        len,
    )?;
    Ok(())
}

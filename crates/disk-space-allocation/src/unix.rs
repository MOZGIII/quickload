//! Unix-specific implementation.
//! Might not work for all UNIX-es, in that case write another specialized implementation or just
//! use fallback.

use nix::fcntl::{fallocate, FallocateFlags};
use std::{convert::TryInto, fs::File, os::unix::io::AsRawFd};

/// A no-op.
#[inline]
pub(super) fn prepare_privileges() -> Result<(), anyhow::Error> {
    // NOOP
    Ok(())
}

/// Allocate the disk space by using [`fallocate`].
#[inline]
pub(super) fn allocate(file: &mut File, len: u64) -> Result<(), anyhow::Error> {
    fallocate(
        file.as_raw_fd(),
        // We need an actual allocation (not a hole), and at the same time we
        // don't want to make the stale disk content readable for security
        // reasons.
        FallocateFlags::FALLOC_FL_ZERO_RANGE,
        0,
        len.try_into()?,
    )?;
    Ok(())
}

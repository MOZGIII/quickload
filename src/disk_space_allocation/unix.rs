use nix::fcntl::{fallocate, FallocateFlags};
use std::{
    convert::TryInto,
    fs::{File, OpenOptions},
    os::unix::io::AsRawFd,
};

#[inline]
pub(super) fn prepare_privileges() -> Result<(), anyhow::Error> {
    // NOOP
}

#[inline]
pub(super) fn open_options() -> OpenOptions {
    let mut opts = OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    opts
}

#[inline]
pub(super) fn allocate(file: &mut File, offset: u64, len: u64) -> Result<(), anyhow::Error> {
    fallocate(
        file.as_raw_fd(),
        // We need an actual allocation (not a hole), and at the same time we
        // don't want to make the stale disk content readable for security
        // reasons.
        FallocateFlags::FALLOC_FL_ZERO_RANGE,
        offset.try_into()?,
        len.try_into()?,
    )?;
    Ok(())
}

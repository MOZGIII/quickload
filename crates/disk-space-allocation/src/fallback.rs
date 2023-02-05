//! The fallback implementation.

use std::{
    fs::File,
    io::{Seek, SeekFrom, Write},
};

/// A no-op.
#[inline]
pub(super) fn prepare_privileges() -> Result<(), std::io::Error> {
    // NOOP
    Ok(())
}

/// Allocate the disk space by writing to the very end of the file.
#[inline]
pub(super) fn allocate(file: &mut File, len: u64) -> Result<(), std::io::Error> {
    // Compute the position one byte behind the desired end of the file.
    let Some(pre_end_pos) = len.checked_sub(1) else {
        return Ok(())
    };

    // Store current file position.
    let current_pos = file.stream_position()?;

    // Go back one byte behind the desired end of file and write a zero.
    file.seek(SeekFrom::Start(pre_end_pos))?;
    let _ = file.write(&[0])?;

    // Restore the file position we captured at the start.
    file.seek(SeekFrom::Start(current_pos))?;

    Ok(())
}

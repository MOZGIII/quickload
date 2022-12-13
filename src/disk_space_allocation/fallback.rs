use std::{
    fs::File,
    io::{Seek, SeekFrom, Write},
};

#[inline]
pub(super) fn prepare_privileges() -> Result<(), anyhow::Error> {
    // noop
    Ok(())
}

#[inline]
pub(super) fn allocate(file: &mut File, len: u64) -> Result<(), anyhow::Error> {
    // Compute the position one byte behind the desired end of the file.
    let Some(pre_end_pos) = len.checked_sub(1) else {
        return Ok(())
    };

    // Store current file position.
    let current_pos = file.seek(SeekFrom::Current(0))?;

    // Go back one byte behind the desired end of file and write a zero.
    file.seek(SeekFrom::Start(pre_end_pos))?;
    file.write(&[0])?;

    // Restore the file position we captured at the start.
    file.seek(SeekFrom::Start(current_pos))?;

    Ok(())
}

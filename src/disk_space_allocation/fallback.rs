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
pub(super) fn allocate(file: &mut File, offset: u64, len: u64) -> Result<(), anyhow::Error> {
    // Store current file position.
    let current_pos = file.seek(SeekFrom::Current(0))?;

    // Aim at the desired end of the file.
    let end_pos = offset + len;
    file.seek(SeekFrom::Start(end_pos))?;

    if end_pos > 0 {
        file.seek(SeekFrom::Start(end_pos - 1))?;
        file.write(&[0])?;
    }

    // Restore the file position we captured at the start.
    file.seek(SeekFrom::Start(current_pos))?;

    Ok(())
}

//! Windows-specific implementation.

#![allow(unsafe_code)]

use std::{
    ffi::CString,
    fs::File,
    io::{Seek, SeekFrom, Write},
    os::windows::io::AsRawHandle,
};
use winapi::{
    shared::{
        minwindef::{BOOL, FALSE, PDWORD},
        ntdef::NULL,
    },
    um::{
        fileapi::SetEndOfFile,
        handleapi::CloseHandle,
        processthreadsapi::{GetCurrentProcess, OpenProcessToken},
        securitybaseapi::AdjustTokenPrivileges,
        winbase::LookupPrivilegeValueA,
        winnt::{
            HANDLE, LPCSTR, PHANDLE, PLUID, PTOKEN_PRIVILEGES, SE_MANAGE_VOLUME_NAME,
            SE_PRIVILEGE_ENABLED, TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
        },
    },
};

/// A wrapper around a HANDLE that automatically closes.
struct Handle(HANDLE);

impl Handle {
    /// Create an null handle.
    pub fn null() -> Self {
        Handle(NULL)
    }

    /// Return the underlying `HANDLE`.
    pub fn as_ptr(&self) -> HANDLE {
        self.0
    }

    /// Return a mutable pointer to the underlying `HANDLE`, allowing it to be
    /// used as an output parameter.
    pub fn as_out_ptr(&mut self) -> PHANDLE {
        &mut self.0 as PHANDLE
    }
}

impl From<HANDLE> for Handle {
    fn from(h: HANDLE) -> Self {
        Handle(h)
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            let rv = unsafe { CloseHandle(self.0) };
            assert!(rv != 0);

            self.0 = NULL;
        }
    }
}

/// Request the required priviliges.
#[inline]
pub(super) fn prepare_privileges() -> Result<(), anyhow::Error> {
    let mut current_process_token = Handle::null();

    cvt(unsafe {
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            current_process_token.as_out_ptr(),
        )
    })?;

    let name = CString::new(SE_MANAGE_VOLUME_NAME).unwrap();
    let mut privs = TOKEN_PRIVILEGES::default();
    cvt(unsafe {
        LookupPrivilegeValueA(
            NULL as LPCSTR,
            name.as_ptr(),
            &mut privs.Privileges[0].Luid as PLUID,
        )
    })?;
    privs.PrivilegeCount = 1;
    privs.Privileges[0].Attributes = SE_PRIVILEGE_ENABLED;

    cvt(unsafe {
        AdjustTokenPrivileges(
            current_process_token.as_ptr(),
            FALSE,
            &mut privs as PTOKEN_PRIVILEGES,
            0, // PreviousState is NULL
            NULL as PTOKEN_PRIVILEGES,
            NULL as PDWORD,
        )
    })?;

    Ok(())
}

/// Allocate disk space with Windows specifics.
#[inline]
pub(super) fn allocate(file: &mut File, len: u64) -> Result<(), anyhow::Error> {
    // Aim at the desired end of the file while recording the previous
    // seek position.
    let current_pos = file.seek(SeekFrom::Start(len))?;

    // Set new file end. This call will fill the file with zeroes.
    // We need an actual allocation (not a hole), and at the same time we
    // don't want to make the stale disk content readable for security
    // reasons.
    cvt(unsafe { SetEndOfFile(file.as_raw_handle()) })?;

    // Compute the position one byte behind the desired end of the file.
    if let Some(pre_end_pos) = len.checked_sub(1) {
        file.seek(SeekFrom::Start(pre_end_pos))?;
        let _ = file.write(&[0])?;
    };

    // Restore the file position we captured at the start.
    file.seek(SeekFrom::Start(current_pos))?;

    Ok(())
}

/// Interpret the result of a system API call.
fn cvt(i: BOOL) -> std::io::Result<BOOL> {
    if i == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(i)
    }
}

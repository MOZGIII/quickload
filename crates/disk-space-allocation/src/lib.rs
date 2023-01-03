//! The disk space allocation facilities.

use std::fs::File;

cfg_if::cfg_if! {
    if #[cfg(all(unix, not(any(target_os = "macos", target_os = "ios"))))] {
        mod unix;
        use self::unix as implementation;
    } else if #[cfg(windows)] {
        mod windows;
        use self::windows as implementation;
    } else {
        mod fallback;
        use self::fallback as implementation;
    }
}

/// Aquire the system privilges necessary for conducting disk space allocation
/// operations.
///
/// Heavily platform-dependent, might be a no-op.
pub fn prepare_privileges() -> Result<(), anyhow::Error> {
    implementation::prepare_privileges()
}

/// Allocate the disk space for a given file descriptor.
///
/// This funtion will attempt to esnure that the given `file` has
/// a real reserved space (of `len`) on the underlying filesystem, that is
/// not a "hole" but reserves some actual space on the device.
///
/// The goal is to ensure that the file is ready to be written into in the
/// random access fashion, yet those write to have a higher chance to not
/// result in a set of fragmented stripes on disk, but rather in
/// a single linear blob.
pub fn allocate(file: &mut File, len: u64) -> Result<(), anyhow::Error> {
    implementation::allocate(file, len)
}

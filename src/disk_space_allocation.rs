use std::fs::File;

cfg_if::cfg_if! {
    if #[cfg(all(unix, not(target_os = "macos")))] {
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

pub fn prepare_privileges() -> Result<(), anyhow::Error> {
    implementation::prepare_privileges()
}

pub fn allocate(file: &mut File, len: u64) -> Result<(), anyhow::Error> {
    implementation::allocate(file, len)
}

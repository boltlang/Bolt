use crate::{
    system::{SystemPathBuf, path::{SystemPath, SystemVirtualPath}}, vfs::Metadata
};

pub type Result<T> = std::io::Result<T>;

/// All interactions in the Salsa database with the broader world should be provided by this trait.
pub trait System : Send + Sync {
    fn read_to_string(&self, path: &SystemPath) -> Result<String>;
    fn read_virtual_path_to_string(&self, path: &SystemVirtualPath) -> Result<String>;
    fn path_metadata(&self, path: &SystemPath) -> Result<Metadata>;
    fn virtual_path_metadata(&self, path: &SystemVirtualPath) -> Result<Metadata>;
    fn current_directory(&self) -> Result<SystemPathBuf>;
    fn canonicalize_path(&self, path: &SystemPath) -> Result<SystemPathBuf>;
}

pub trait WritableSystem : System {
    fn write_file_bytes(&self, path: &SystemPath, contents: &[u8]) -> Result<()>;
    fn write_file_bytes_virtual(&self, path: &SystemVirtualPath, contents: &[u8]) -> Result<()>;
}

use filetime::FileTime;

#[cfg(not(target_arch = "wasm32"))]
pub fn file_time_now() -> FileTime {
    FileTime::now()
}

#[cfg(target_arch = "wasm32")]
pub fn file_time_now() -> FileTime {
    // Copied from FileTime::from_system_time()
    let time = web_time::SystemTime::now();

    time.duration_since(web_time::UNIX_EPOCH)
        .map(|d| FileTime::from_unix_time(d.as_secs() as i64, d.subsec_nanos()))
        .unwrap_or_else(|e| {
            let until_epoch = e.duration();
            let (sec_offset, nanos) = if until_epoch.subsec_nanos() == 0 {
                (0, 0)
            } else {
                (-1, 1_000_000_000 - until_epoch.subsec_nanos())
            };

            FileTime::from_unix_time(-(until_epoch.as_secs() as i64) + sec_offset, nanos)
        })
}


pub type SystemPath = crate::vfs::Path;

pub type SystemPathBuf = crate::vfs::PathBuf;

pub type SystemVirtualPath = str;

pub type SystemVirtualPathBuf = String;

// #[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
// #[repr(transparent)]
// pub struct SystemVirtualPath(str);

// impl SystemVirtualPath {

//     pub fn new(str: &(impl AsRef<str> + ?Sized)) -> &Self {
//         let str = str.as_ref();
//         // SAFETY: We used #[repr(transparent)] on this struct
//         unsafe { &*(str as *const str as *const &SystemVirtualPath) }
//     }

// }

// #[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
// pub struct SystemVirtualPathBuf(String);

// impl SystemVirtualPathBuf {

//     pub fn as_virtual_system_path(&self) -> &SystemVirtualPath {
//         &SystemVirtualPath::new(self.0.as_str())
//     }

// }

// impl Deref for SystemVirtualPathBuf {
//     type Target = SystemVirtualPath;
//     fn deref(&self) -> &Self::Target {
//         self.as_virtual_system_path()
//     }
// }
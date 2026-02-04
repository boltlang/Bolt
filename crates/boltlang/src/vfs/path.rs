use std::{borrow::Borrow, ops::Deref, path::Component};

use crate::system::SystemPathBuf;

#[repr(transparent)]
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Path(std::path::Path);

impl Path {

    pub fn from_str(value: &str) -> &Path {
        Path::new(std::path::Path::new(value))
    }

    pub fn display(&self) -> std::path::Display<'_> {
        self.0.display()
    }

    pub fn new(path: &(impl AsRef<std::path::Path> + ?Sized)) -> &Path {
        let path = path.as_ref();
        unsafe { &*(path as *const std::path::Path as *const Path) }
    }

    pub fn as_std_path(&self) -> &std::path::Path {
        &self.0
    }

    pub fn is_absolute(&self) -> bool {
        self.0.is_absolute()
    }

    pub fn absolute(path: impl AsRef<Path>, cwd: impl AsRef<Path>) -> PathBuf {
        fn absolute(path: &Path, cwd: &Path) -> PathBuf {
            let path = &path.0;

            let mut components = path.components().peekable();
            let mut ret = if let Some(
                c @ (Component::Prefix(..) | Component::RootDir),
            ) = components.peek().cloned()
            {
                components.next();
                std::path::PathBuf::from(c.as_os_str())
            } else {
                cwd.0.to_path_buf()
            };

            for component in components {
                match component {
                    Component::Prefix(..) => unreachable!(),
                    Component::RootDir => {
                        ret.push(component);
                    }
                    Component::CurDir => {}
                    Component::ParentDir => {
                        ret.pop();
                    }
                    Component::Normal(c) => {
                        ret.push(c);
                    }
                }
            }

            PathBuf::from_std_path_buf(ret)
        }

        absolute(path.as_ref(), cwd.as_ref())
    }

    pub fn to_system_path_buf(&self) -> PathBuf {
        PathBuf(self.0.to_path_buf())
    }

}

impl <'a> From<&'a std::path::Path> for &'a Path {
    fn from(value: &'a std::path::Path) -> Self {
        Path::new(value)
    }
}

impl AsRef<std::path::Path> for Path {
    fn as_ref(&self) -> &std::path::Path {
        &self.0
    }
}

impl AsRef<Path> for Path {
    #[inline]
    fn as_ref(&self) -> &Path {
        self
    }
}

impl AsRef<Path> for PathBuf {
    #[inline]
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}

impl From<&str> for PathBuf {
    fn from(value: &str) -> Self {
        PathBuf(std::path::PathBuf::from(value))
    }
}

impl AsRef<Path> for str {
    #[inline]
    fn as_ref(&self) -> &Path {
        Path::new(self)
    }
}

impl AsRef<Path> for String {
    #[inline]
    fn as_ref(&self) -> &Path {
        Path::new(self)
    }
}

#[repr(transparent)]
#[derive(Debug, Eq, PartialEq, Clone, Hash, PartialOrd, Ord)]
pub struct PathBuf(std::path::PathBuf);

impl PathBuf {

    pub fn from_std_path_buf(pb: std::path::PathBuf) -> Self {
        Self(pb)
    }

    pub fn is_absolute(&self) -> bool {
        self.0.is_absolute()
    }

    pub fn as_path(&self) -> &Path {
        Path::new(&self.0)
    }

    pub fn as_str(&self) -> Option<&str> {
        self.0.as_os_str().to_str()
    }

}

impl From<std::path::PathBuf> for SystemPathBuf {
    fn from(value: std::path::PathBuf) -> Self {
        PathBuf(value)
    }
}

impl From<String> for PathBuf {
    fn from(value: String) -> Self {
        PathBuf(std::path::PathBuf::from(value))
    }
}

impl Deref for PathBuf {
    type Target = Path;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_path()
    }
}

impl Borrow<Path> for PathBuf {
    fn borrow(&self) -> &Path {
        self.as_path()
    }
}

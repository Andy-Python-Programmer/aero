use core::borrow::Borrow;
use core::fmt::Display;
use core::ops::Deref;

use alloc::borrow::ToOwned;
use alloc::vec::Vec;

/// A slice of a path (akin to [str]).
#[derive(Debug)]
pub struct Path(str);

impl Path {
    pub fn new(path: &str) -> &Self {
        unsafe { &*(path as *const str as *const Path) }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns [`true`] if the path is absolute.
    pub fn is_absolute(&self) -> bool {
        self.0.starts_with('/')
    }

    /// Returns an iterator over the components of the path.
    pub fn components(&self) -> impl Iterator<Item = &str> {
        self.0.split('/').filter(|e| !e.is_empty() && *e != ".")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Creates an owned [`PathBuf`] with `path` adjoined to `self`.
    ///
    /// If `path` is absolute, it replaces the current path.
    ///
    /// See [`PathBuf::push`] for more details on what it means to adjoin a path.
    pub fn join<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        let mut result = self.to_owned();
        result.push(path);
        result
    }

    /// Helper function that returns the parent path and the base name
    /// of the path.
    pub fn parent_and_basename(&self) -> (&Self, &str) {
        if let Some(slash_index) = self.0.rfind('/') {
            let parent_dir = if slash_index == 0 {
                Path::new("/")
            } else {
                Path::new(&self.0[..slash_index])
            };

            let basename = &self.0[(slash_index + 1)..];
            (parent_dir, basename)
        } else {
            // A relative path without any slashes.
            (Path::new(""), &self.0)
        }
    }

    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    /// Returns the byte length of the path.
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl Borrow<Path> for PathBuf {
    #[inline]
    fn borrow(&self) -> &Path {
        self.deref()
    }
}

impl ToOwned for Path {
    type Owned = PathBuf;

    #[inline]
    fn to_owned(&self) -> Self::Owned {
        PathBuf(self.0.to_owned())
    }
}

impl AsRef<Path> for Path {
    #[inline]
    fn as_ref(&self) -> &Path {
        self
    }
}

impl AsRef<Path> for &str {
    #[inline]
    fn as_ref(&self) -> &Path {
        Path::new(self)
    }
}

/// An owned, mutable path (akin to [`String`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathBuf(String);

impl PathBuf {
    /// Allocates an empty `PathBuf`.
    #[inline]
    pub fn new() -> Self {
        Self(String::new())
    }

    #[inline]
    fn as_mut_vec(&mut self) -> &mut Vec<u8> {
        // TODO: safety?
        unsafe { self.0.as_mut_vec() }
    }

    /// Extends `self` with `path`.
    ///
    /// If `path` is absolute, it replaces the current path.
    pub fn push<P: AsRef<Path>>(&mut self, path: P) {
        let path = path.as_ref();

        // absolute `path` replaces `self`
        if path.is_absolute() {
            self.as_mut_vec().truncate(0);
        }

        let need_sep = self.0.chars().last().map(|c| c != '/').unwrap_or(false);

        // TODO: verbatim pahts need . and .. removed

        if need_sep {
            self.0.push('/');
        }

        self.0.push_str(path.as_str());
    }
}

impl From<String> for PathBuf {
    #[inline]
    fn from(path: String) -> Self {
        Self(path)
    }
}

impl From<&str> for PathBuf {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl Deref for PathBuf {
    type Target = Path;

    #[inline]
    fn deref(&self) -> &Self::Target {
        Path::new(&self.0)
    }
}

impl AsRef<Path> for PathBuf {
    #[inline]
    fn as_ref(&self) -> &Path {
        self
    }
}

impl Into<String> for PathBuf {
    #[inline]
    fn into(self) -> String {
        self.0
    }
}

impl Display for PathBuf {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.fmt(f)
    }
}

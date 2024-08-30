use crate::LoaderError;
use std::fs::read;
use std::io::ErrorKind;
use std::path::PathBuf;

/// Trait for the various sources that assets can be loaded from
pub trait AssetSource {
    /// Check if a path exists in the source
    fn has(&self, path: &str) -> Result<bool, LoaderError>;

    /// Load an asset from the source by path if it exists
    fn load(&self, path: &str) -> Result<Option<Vec<u8>>, LoaderError>;
}

impl AssetSource for PathBuf {
    fn has(&self, path: &str) -> Result<bool, LoaderError> {
        Ok(self.join(path).exists())
    }

    fn load(&self, path: &str) -> Result<Option<Vec<u8>>, LoaderError> {
        match read(self.join(path)) {
            Ok(data) => Ok(Some(data)),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(feature = "vpk")]
mod vdf {
    use super::AssetSource;
    use crate::LoaderError;
    use vpk::VPK;

    impl AssetSource for VPK {
        fn has(&self, path: &str) -> Result<bool, LoaderError> {
            Ok(self.tree.contains_key(path))
        }

        fn load(&self, path: &str) -> Result<Option<Vec<u8>>, LoaderError> {
            if let Some(entry) = self.tree.get(path) {
                Ok(Some(entry.get()?.into()))
            } else {
                Ok(None)
            }
        }
    }
}

#[cfg(feature = "bsp")]
mod vbsp {
    use super::AssetSource;
    use crate::LoaderError;
    use vbsp::Packfile;

    impl AssetSource for Packfile {
        fn has(&self, path: &str) -> Result<bool, LoaderError> {
            Ok(self.has(path)?)
        }

        fn load(&self, path: &str) -> Result<Option<Vec<u8>>, LoaderError> {
            Ok(self.get(path)?)
        }
    }
}

#[cfg(feature = "zip")]
mod zip {
    use super::AssetSource;
    use crate::LoaderError;
    use std::io::{Read, Seek};
    use std::sync::Mutex;
    use zip::result::ZipError;
    use zip::ZipArchive;

    impl<Reader: Read + Seek> AssetSource for Mutex<ZipArchive<Reader>> {
        fn has(&self, path: &str) -> Result<bool, LoaderError> {
            match self.lock().unwrap().by_name(path) {
                Ok(_) => Ok(true),
                Err(ZipError::FileNotFound) => Ok(false),
                Err(e) => Err(e.into()),
            }
        }

        fn load(&self, path: &str) -> Result<Option<Vec<u8>>, LoaderError> {
            let mut zip = self.lock().unwrap();
            let mut entry = match zip.by_name(path) {
                Ok(entry) => entry,
                Err(ZipError::FileNotFound) => {
                    return Ok(None);
                }
                Err(e) => {
                    return Err(e.into());
                }
            };
            let mut buff = vec![0; entry.size() as usize];
            entry.read_exact(&mut buff)?;
            Ok(Some(buff))
        }
    }
}

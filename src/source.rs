use crate::LoaderError;
use std::fs::read;
use std::io::ErrorKind;
use std::path::PathBuf;

pub trait AssetSource {
    fn has(&self, path: &str) -> Result<bool, LoaderError>;

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

#[cfg(feature = "vbsp")]
mod vbsp {
    use super::AssetSource;
    use crate::LoaderError;
    use vbsp::{BspError, Packfile};

    impl AssetSource for Packfile {
        fn has(&self, path: &str) -> Result<bool, LoaderError> {
            match self.has(path) {
                Ok(found) => Ok(found),
                Err(BspError::Zip(err)) => Err(err.into()),
                Err(e) => Err(LoaderError::Other(e.to_string())), // the error *should* always be a zip error
            }
        }

        fn load(&self, path: &str) -> Result<Option<Vec<u8>>, LoaderError> {
            match self.get(path) {
                Ok(data) => Ok(data),
                Err(BspError::Zip(err)) => Err(err.into()),
                Err(e) => Err(LoaderError::Other(e.to_string())), // the error *should* always be a zip error
            }
        }
    }
}

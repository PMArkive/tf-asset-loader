//! Utility for loading assets from tf2 data files.
//!
//! Supports loading assets like models and textures from the tf2 data directory. The tf2 data directory should be
//! automatically detected when installed to steam, or you can use the `TF_DIR` environment variable to overwrite the data
//! directory.
//!
//! Supports loading both plain file data and data embedded in `vpk` files.
//! ```rust,no_run
//! # use tf_asset_loader::{Loader, LoaderError};
//! #
//! fn main() -> Result<(), LoaderError> {
//!     let loader = Loader::new()?;
//!     if let Some(model) = loader.load("models/props_gameplay/resupply_locker.mdl")? {
//!         println!("resupply_locker.mdl is {} bytes large", model.len());
//!     }
//!     Ok(())
//! }
//! ```

pub mod source;

use path_dedot::ParseDot;
pub use source::AssetSource;
use std::borrow::Cow;
use std::env::var_os;
use std::fmt::{Debug, Display, Formatter};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use steamlocate::SteamDir;
use thiserror::Error;
use tracing::warn;
#[cfg(feature = "bsp")]
use vbsp::BspError;

#[derive(Debug, Error)]
pub enum LoaderError {
    #[error("Failed to find tf2 install location")]
    Tf2NotFound,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[cfg(feature = "zip")]
    #[error(transparent)]
    Zip(#[from] zip::result::ZipError),
    #[error("{0}")]
    Other(String),
}

#[cfg(feature = "bsp")]
impl From<BspError> for LoaderError {
    fn from(value: BspError) -> Self {
        match value {
            BspError::Zip(err) => LoaderError::Zip(err),
            BspError::IO(err) => LoaderError::Io(err),
            err => LoaderError::Other(err.to_string()),
        }
    }
}

/// The tf2 asset loader instance
#[derive(Clone)]
pub struct Loader {
    sources: Vec<Arc<dyn AssetSource + Send + Sync>>,
}

impl Debug for Loader {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Loader")
            .field("sources", &self.sources.len())
            .finish_non_exhaustive()
    }
}

impl Loader {
    /// Create the loader, either auto-detecting the tf2 directory or from the `TF_DIR` environment variable.
    pub fn new() -> Result<Self, LoaderError> {
        let tf2_dir = tf2_path()?;
        Self::with_tf2_dir(tf2_dir)
    }

    /// Create the loader with the specified tf2 directory.
    pub fn with_tf2_dir<P: AsRef<Path>>(tf2_dir: P) -> Result<Self, LoaderError> {
        let tf2_dir = tf2_dir.as_ref();

        let tf_dir = tf2_dir.join("tf");
        let hl_dir = tf2_dir.join("hl2");
        let download = tf_dir.join("download");

        #[cfg(feature = "vpk")]
        let vpks = tf_dir
            .read_dir()?
            .chain(hl_dir.read_dir()?)
            .filter_map(|item| item.ok())
            .filter_map(|item| Some(item.path().to_str()?.to_string()))
            .filter(|path| path.ends_with("dir.vpk"))
            .map(|path| vpk::from_path(&path))
            .filter_map(|res| {
                if let Err(e) = &res {
                    warn!(error = ?e, "error while loading vpk");
                }
                res.ok()
            })
            .map(|vpk| Arc::new(vpk) as Arc<dyn AssetSource + Send + Sync>);

        #[allow(unused_mut)]
        let mut sources = vec![
            Arc::new(tf_dir) as Arc<dyn AssetSource + Send + Sync>,
            Arc::new(hl_dir),
        ];

        if download.exists() {
            sources.push(Arc::new(download));
        }

        #[cfg(feature = "vpk")]
        sources.extend(vpks);

        Ok(Loader { sources })
    }

    /// Add a new source to the loader.
    ///
    /// This is intended to be used to add data from bsp files
    pub fn add_source<S: AssetSource + Send + Sync + 'static>(&mut self, source: S) {
        self.sources.push(Arc::new(source))
    }

    /// Check if a file by path exists.
    #[tracing::instrument(skip(self))]
    pub fn exists(&self, name: &str) -> Result<bool, LoaderError> {
        let name = clean_path(name);
        for source in self.sources.iter() {
            if source.has(&name)? {
                return Ok(true);
            }
        }

        let lower_name = name.to_ascii_lowercase();
        if name != lower_name {
            for source in self.sources.iter() {
                if source.has(&lower_name)? {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Load a file by path.
    ///
    /// Returns the file data as `Vec<u8>` or `None` if the path doesn't exist.
    #[tracing::instrument(skip(self))]
    pub fn load(&self, name: &str) -> Result<Option<Vec<u8>>, LoaderError> {
        let name = clean_path(name);
        for source in self.sources.iter() {
            if let Some(data) = source.load(&name)? {
                return Ok(Some(data));
            }
        }

        let lower_name = name.to_ascii_lowercase();
        if name != lower_name {
            for source in self.sources.iter() {
                if let Some(data) = source.load(&lower_name)? {
                    return Ok(Some(data));
                }
            }
        }

        Ok(None)
    }

    /// Look for a file by name in one or more paths
    pub fn find_in_paths<S: Display>(&self, name: &str, paths: &[S]) -> Option<String> {
        for path in paths {
            let full_path = format!("{}{}", path, name);
            let full_path = clean_path(&full_path);
            if self.exists(&full_path).unwrap_or_default() {
                return Some(full_path.to_string());
            }
        }

        let lower_name = name.to_ascii_lowercase();
        if name != lower_name {
            for path in paths {
                let full_path = format!("{}{}", path, lower_name);
                let full_path = clean_path(&full_path);
                if self.exists(&full_path).unwrap_or_default() {
                    return Some(full_path.to_string());
                }
            }
        }

        None
    }
}

fn clean_path(path: &str) -> Cow<str> {
    if path.contains("/../") {
        let path_buf = PathBuf::from(format!("/{path}"));
        let Ok(absolute_path) = path_buf.parse_dot_from("/") else {
            return path.into();
        };
        let path = absolute_path.to_str().unwrap().trim_start_matches('/');
        String::from(path).into()
    } else {
        path.into()
    }
}

#[test]
fn test_clean_path() {
    assert_eq!("foo/bar", clean_path("foo/bar"));
    assert_eq!("foo/bar", clean_path("foo/asd/../bar"));
    assert_eq!("../bar", clean_path("../bar"));
}

fn tf2_path() -> Result<PathBuf, LoaderError> {
    if let Some(path) = var_os("TF_DIR") {
        let path: PathBuf = path.into();
        if path.is_dir() {
            Ok(path)
        } else {
            Err(LoaderError::Tf2NotFound)
        }
    } else {
        let (app, library) = SteamDir::locate()
            .map_err(|_| LoaderError::Tf2NotFound)?
            .find_app(440)
            .map_err(|_| LoaderError::Tf2NotFound)?
            .ok_or(LoaderError::Tf2NotFound)?;
        Ok(library.resolve_app_dir(&app))
    }
}

pub mod source;

pub use source::AssetSource;
use std::env::var_os;
use std::fmt::{Debug, Formatter};
use std::path::PathBuf;
use std::sync::Arc;
use steamlocate::SteamDir;
use thiserror::Error;
use tracing::warn;

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

#[derive(Clone)]
pub struct Loader {
    sources: Vec<Arc<dyn AssetSource>>,
}

impl Debug for Loader {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Loader")
            .field("sources", &self.sources.len())
            .finish_non_exhaustive()
    }
}

impl Loader {
    /// Create the loader
    pub fn new() -> Result<Self, LoaderError> {
        let tf2_dir = tf2_path()?;

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
            .map(|vpk| Arc::new(vpk) as Arc<dyn AssetSource>);

        #[allow(unused_mut)]
        let mut sources = vec![
            Arc::new(tf_dir) as Arc<dyn AssetSource>,
            Arc::new(download),
            Arc::new(hl_dir),
        ];

        #[cfg(feature = "vpk")]
        sources.extend(vpks);

        Ok(Loader { sources })
    }

    pub fn add_source<S: AssetSource + 'static>(&mut self, source: S) {
        self.sources.push(Arc::new(source))
    }

    #[tracing::instrument(skip(self))]
    pub fn exists(&self, name: &str) -> Result<bool, LoaderError> {
        for source in self.sources.iter() {
            if source.has(name)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    #[tracing::instrument(skip(self))]
    pub fn load(&self, name: &str) -> Result<Option<Vec<u8>>, LoaderError> {
        for source in self.sources.iter() {
            if let Some(data) = source.load(name)? {
                return Ok(Some(data));
            }
        }
        Ok(None)
    }

    pub fn find_in_paths(&self, name: &str, paths: &[String]) -> Option<String> {
        for path in paths {
            let full_path = format!("{}{}", path, name);
            if self.exists(&full_path).unwrap_or_default() {
                return Some(full_path);
            }
        }
        None
    }
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
        Ok(SteamDir::locate()
            .ok_or(LoaderError::Tf2NotFound)?
            .app(&440)
            .ok_or(LoaderError::Tf2NotFound)?
            .path
            .clone())
    }
}

use std::{fmt::Display, fs, io, path::Path, time::Duration};

use serde::{Deserialize, Serialize};

use crate::{profile_manager::OnFailure, util::NaiveLeakyBucketConfig};

#[derive(Debug)]
pub enum AppStateError {
    ParseError(serde_yaml::Error),
    IOError(io::Error),
}

impl Display for AppStateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let out = match self {
            AppStateError::ParseError(err) => err.to_string(),
            AppStateError::IOError(err) => err.to_string(),
        };
        write!(f, "{}", out)
    }
}

impl From<serde_yaml::Error> for AppStateError {
    fn from(err: serde_yaml::Error) -> Self {
        Self::ParseError(err)
    }
}
impl From<io::Error> for AppStateError {
    fn from(err: io::Error) -> Self {
        Self::IOError(err)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppState {
    /// `""` indicates none.
    pub most_recent_profile: String,
    pub on_fail: OnFailure,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            most_recent_profile: String::new(),
            on_fail: OnFailure::Restart {
                limit: NaiveLeakyBucketConfig::new(5, Duration::from_secs(30)),
            },
        }
    }
}

impl AppState {
    pub fn from_file<P>(path: P) -> Result<Self, AppStateError>
    where
        P: AsRef<Path>,
    {
        let content = fs::read_to_string(path)?;
        let state = serde_yaml::from_str(&content)?;
        Ok(state)
    }
    pub fn write_to_file<P>(&self, path: P) -> Result<(), AppStateError>
    where
        P: AsRef<Path>,
    {
        let content = serde_yaml::to_string(self)?;
        fs::write(path, content)?;
        Ok(())
    }
}

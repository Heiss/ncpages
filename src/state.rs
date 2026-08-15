//! Persisted state.
//!
//! Without this, every `compose up` causes a full rebuild — and the reconcile
//! path stays untested until the day it is actually needed.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct State {
    /// ETag of the watched root. One PROPFIND answers "did anything change".
    #[serde(default)]
    pub root_etag: Option<String>,
    /// Per-file ETags of the working copy, keyed by path relative to the root.
    #[serde(default)]
    pub file_etags: BTreeMap<String, String>,
    /// Content hash of the assembled content directory at the last build.
    #[serde(default)]
    pub content_hash: Option<String>,
    /// Release id of the last successful publish.
    #[serde(default)]
    pub last_release: Option<String>,
    #[serde(default)]
    pub last_result: Option<String>,
    #[serde(default)]
    pub last_build_finished: Option<String>,
    /// Fingerprint of the status report this service last wrote itself, so it
    /// does not treat its own write as a change.
    #[serde(default)]
    pub self_write_fingerprint: Option<String>,
}

impl State {
    fn file(dir: &Path) -> PathBuf {
        dir.join("state.json")
    }

    pub fn load(dir: &Path) -> Result<Self> {
        let path = Self::file(dir);
        match std::fs::read_to_string(&path) {
            Ok(text) => Ok(serde_json::from_str(&text)
                .with_context(|| format!("parsing {}", path.display()))?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
        }
    }

    /// Written through a temporary file and renamed, so a crash mid-write cannot
    /// leave state that fails to parse on the next start.
    pub fn save(&self, dir: &Path) -> Result<()> {
        std::fs::create_dir_all(dir)?;
        let path = Self::file(dir);
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(&tmp, &path).with_context(|| format!("replacing {}", path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_state_is_an_empty_state_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let state = State::load(dir.path()).unwrap();
        assert!(state.root_etag.is_none());
    }

    #[test]
    fn state_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = State {
            root_etag: Some("\"abc\"".into()),
            ..Default::default()
        };
        state.file_etags.insert("a.md".into(), "\"1\"".into());
        state.save(dir.path()).unwrap();

        let loaded = State::load(dir.path()).unwrap();
        assert_eq!(loaded.root_etag.as_deref(), Some("\"abc\""));
        assert_eq!(
            loaded.file_etags.get("a.md").map(String::as_str),
            Some("\"1\"")
        );
    }
}

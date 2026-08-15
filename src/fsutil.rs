//! Filesystem helpers shared by assemble, gate and publish.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

/// Nextcloud names conflict copies like `note (conflicted copy 2026-08-14 120000).md`.
/// They are excluded from builds *and* reported: their existence means a version
/// of someone's work is about to be lost.
pub fn is_conflict_copy(name: &str) -> bool {
    name.contains("(conflicted copy")
}

pub fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    for entry in WalkDir::new(from).follow_links(false) {
        let entry = entry.with_context(|| format!("walking {}", from.display()))?;
        let rel = entry.path().strip_prefix(from).unwrap();
        let target = to.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(entry.path(), &target)
                .with_context(|| format!("copying {}", entry.path().display()))?;
        }
    }
    Ok(())
}

pub fn remove_dir_if_exists(path: &Path) -> Result<()> {
    match std::fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e).with_context(|| format!("removing {}", path.display())),
    }
}

pub fn count_files_with_extension(root: &Path, ext: &str) -> usize {
    WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|x| x == ext))
        .count()
}

/// Basenames that occur more than once. Wikilink resolution matches by
/// basename, so duplicates make some links silently point at the wrong page.
pub fn duplicate_basenames(root: &Path, ext: &str) -> Vec<String> {
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().extension().is_none_or(|x| x != ext) {
            continue;
        }
        if let Some(name) = entry.path().file_name().and_then(|n| n.to_str()) {
            *seen.entry(name.to_string()).or_default() += 1;
        }
    }
    seen.into_iter()
        .filter(|(_, n)| *n > 1)
        .map(|(name, _)| name)
        .collect()
}

/// Content hash of a directory tree: sorted relative paths plus file contents.
/// Deliberately not mtime-based — timestamps are unreliable across sync
/// boundaries, so they are never used for change detection.
pub fn tree_hash(root: &Path) -> Result<String> {
    let mut files: Vec<PathBuf> = WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().to_path_buf())
        .collect();
    files.sort();

    let mut hasher = Sha256::new();
    for file in files {
        let rel = file.strip_prefix(root).unwrap_or(&file);
        hasher.update(rel.to_string_lossy().as_bytes());
        hasher.update([0u8]);
        hasher.update(std::fs::read(&file).with_context(|| format!("hashing {}", file.display()))?);
        hasher.update([0u8]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conflict_copies_are_recognised() {
        assert!(is_conflict_copy(
            "note (conflicted copy 2026-08-14 120000).md"
        ));
        assert!(!is_conflict_copy("note.md"));
    }

    #[test]
    fn tree_hash_ignores_timestamps_but_not_content() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.md"), "one").unwrap();
        let first = tree_hash(dir.path()).unwrap();

        // Rewriting identical content changes mtime, not the hash.
        std::fs::write(dir.path().join("a.md"), "one").unwrap();
        assert_eq!(first, tree_hash(dir.path()).unwrap());

        std::fs::write(dir.path().join("a.md"), "two").unwrap();
        assert_ne!(first, tree_hash(dir.path()).unwrap());
    }

    #[test]
    fn duplicate_basenames_are_found_across_directories() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("sub")).unwrap();
        std::fs::write(dir.path().join("note.md"), "").unwrap();
        std::fs::write(dir.path().join("sub/note.md"), "").unwrap();
        std::fs::write(dir.path().join("other.md"), "").unwrap();
        assert_eq!(
            duplicate_basenames(dir.path(), "md"),
            vec!["note.md".to_string()]
        );
    }
}

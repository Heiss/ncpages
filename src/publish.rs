//! Atomic publish: `rename(2)` on a symlink, within one filesystem.
//!
//! There is no instant at which a request can see half a site. No network
//! protocol offers this, which is why the publish target lives next to the
//! builder rather than behind rsync or WebDAV.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::fsutil;

/// Release ids sort lexicographically in chronological order, which is what
/// retention relies on.
pub fn new_release_id(now: std::time::SystemTime) -> String {
    let secs = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let (y, mo, d, h, mi, s) = civil_from_unix(secs as i64);
    format!("{y:04}{mo:02}{d:02}T{h:02}{mi:02}{s:02}Z")
}

/// Days-to-civil conversion (Howard Hinnant's algorithm), so no date crate is
/// needed for the one place a timestamp is formatted.
fn civil_from_unix(secs: i64) -> (i64, u32, u32, u32, u32, u32) {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (
        y,
        m,
        d,
        (rem / 3600) as u32,
        ((rem % 3600) / 60) as u32,
        (rem % 60) as u32,
    )
}

pub fn releases_dir(root: &Path) -> PathBuf {
    root.join("releases")
}

pub fn current_link(root: &Path) -> PathBuf {
    root.join("current")
}

/// The release `current` points at, if any.
pub fn current_release(root: &Path) -> Option<PathBuf> {
    std::fs::read_link(current_link(root))
        .ok()
        .map(|target| {
            if target.is_absolute() {
                target
            } else {
                root.join(target)
            }
        })
        .filter(|p| p.exists())
}

/// Point `current` at `release` atomically.
///
/// `symlink(2)` cannot replace an existing link, so the new link is created
/// under a temporary name and `rename(2)`d over the old one. That rename is the
/// atomic step the whole pipeline is built around.
pub fn swap(root: &Path, release: &Path) -> Result<()> {
    if !release.is_dir() {
        bail!("release {} does not exist", release.display());
    }
    let link = current_link(root);
    let tmp = root.join(".current.tmp");

    let target = release
        .strip_prefix(root)
        .map(Path::to_path_buf)
        .unwrap_or_else(|_| release.to_path_buf());

    let _ = std::fs::remove_file(&tmp);
    std::os::unix::fs::symlink(&target, &tmp)
        .with_context(|| format!("creating symlink {}", tmp.display()))?;
    std::fs::rename(&tmp, &link)
        .with_context(|| format!("swapping {} to {}", link.display(), target.display()))?;
    Ok(())
}

/// Keep the newest `keep` releases. Retention has to be enforced actively: a
/// full root filesystem takes the Nextcloud on the same host down with it.
pub fn retain(root: &Path, keep: usize) -> Result<Vec<String>> {
    let dir = releases_dir(root);
    let keep = keep.max(1);
    let current = current_release(root);

    let mut ids: Vec<String> = std::fs::read_dir(&dir)
        .with_context(|| format!("reading {}", dir.display()))?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    ids.sort();

    let mut removed = Vec::new();
    while ids.len() > keep {
        let id = ids.remove(0);
        let path = dir.join(&id);
        // Never remove what is currently being served, however old it is.
        if current.as_deref() == Some(path.as_path()) {
            continue;
        }
        fsutil::remove_dir_if_exists(&path)?;
        removed.push(id);
    }
    Ok(removed)
}

/// Without a `current` link the web server answers 404 to everything during the
/// first sync, which looks like a broken deployment rather than a starting one.
pub fn ensure_bootstrap(root: &Path) -> Result<bool> {
    if current_release(root).is_some() {
        return Ok(false);
    }
    let release = releases_dir(root).join("00000000T000000Z-bootstrap");
    std::fs::create_dir_all(&release)?;
    std::fs::write(
        release.join("index.html"),
        "<!doctype html><meta charset=\"utf-8\"><title>ncpages</title>\
         <h1>Nothing published yet</h1>\
         <p>ncpages is running and has not completed a build yet.</p>\n",
    )?;
    swap(root, &release)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(root: &Path, id: &str, pages: usize) -> PathBuf {
        let dir = releases_dir(root).join(id);
        std::fs::create_dir_all(&dir).unwrap();
        for i in 0..pages {
            std::fs::write(dir.join(format!("p{i}.html")), "x").unwrap();
        }
        dir
    }

    #[test]
    fn swapping_replaces_an_existing_link() {
        let root = tempfile::tempdir().unwrap();
        let a = release(root.path(), "a", 1);
        let b = release(root.path(), "b", 1);

        swap(root.path(), &a).unwrap();
        assert_eq!(current_release(root.path()).unwrap(), a);

        swap(root.path(), &b).unwrap();
        assert_eq!(current_release(root.path()).unwrap(), b);
    }

    #[test]
    fn the_link_is_relative_so_the_volume_can_move() {
        let root = tempfile::tempdir().unwrap();
        let a = release(root.path(), "a", 1);
        swap(root.path(), &a).unwrap();
        let target = std::fs::read_link(current_link(root.path())).unwrap();
        assert!(target.is_relative(), "{}", target.display());
    }

    #[test]
    fn retention_keeps_the_newest_and_never_the_served_one() {
        let root = tempfile::tempdir().unwrap();
        for id in ["a", "b", "c", "d", "e"] {
            release(root.path(), id, 1);
        }
        swap(root.path(), &releases_dir(root.path()).join("a")).unwrap();

        let removed = retain(root.path(), 2).unwrap();
        assert_eq!(removed, vec!["b".to_string(), "c".to_string()]);
        assert!(
            releases_dir(root.path()).join("a").exists(),
            "served release was removed"
        );
        assert!(releases_dir(root.path()).join("e").exists());
    }

    #[test]
    fn bootstrap_creates_a_holding_page_once() {
        let root = tempfile::tempdir().unwrap();
        assert!(ensure_bootstrap(root.path()).unwrap());
        assert!(current_release(root.path())
            .unwrap()
            .join("index.html")
            .exists());
        assert!(!ensure_bootstrap(root.path()).unwrap());
    }

    #[test]
    fn release_ids_sort_chronologically() {
        let earlier =
            new_release_id(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_760_000_000));
        let later =
            new_release_id(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_770_000_000));
        assert!(earlier < later, "{earlier} !< {later}");
        assert_eq!(earlier.len(), 16);
    }
}

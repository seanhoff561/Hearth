//! Caches kept on disk (E4.1 §4.4): each a folder holding at most so much, its oldest files let
//! go first; and the folders of other planets beside it let go, the longest unused first, when
//! all of them together hold too much. Files are written whole or not at all.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

/// Files being written, told apart (threads may write the same file at once).
static WRITING: AtomicU64 = AtomicU64::new(0);

/// A folder of cached files holding at most `cap` bytes.
#[derive(Debug)]
pub struct Folder {
    dir: PathBuf,
    cap: u64,
    /// What it holds (bytes): counted when first wanted, then kept up as files are written.
    used: OnceLock<AtomicU64>,
}

impl Folder {
    pub fn new(dir: impl Into<PathBuf>, cap: u64) -> Self {
        Self {
            dir: dir.into(),
            cap,
            used: OnceLock::new(),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn counter(&self) -> &AtomicU64 {
        self.used
            .get_or_init(|| AtomicU64::new(files(&self.dir).iter().map(|f| f.1).sum()))
    }

    /// What the folder holds (bytes).
    pub fn used(&self) -> u64 {
        self.counter().load(Ordering::Relaxed)
    }

    /// Writes a file of the folder (whole or not at all), letting the oldest go when the folder
    /// holds more than its most.
    pub fn write(&self, path: &Path, bytes: &[u8]) -> std::io::Result<()> {
        write_whole(path, bytes)?;
        let len = bytes.len() as u64;
        if self.counter().fetch_add(len, Ordering::Relaxed) + len > self.cap {
            self.prune();
        }
        Ok(())
    }

    /// Lets the oldest files go until the folder holds four fifths of its most.
    fn prune(&self) {
        let mut all = files(&self.dir);
        all.sort_by_key(|f| f.2);
        let mut used: u64 = all.iter().map(|f| f.1).sum();
        for (path, len, _) in all {
            if used <= self.cap / 5 * 4 {
                break;
            }
            if std::fs::remove_file(&path).is_ok() {
                used -= len;
            }
        }
        self.counter().store(used, Ordering::Relaxed);
    }
}

/// Writes a file whole or not at all: aside, then moved into place, so a reader never finds
/// half of one.
pub fn write_whole(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let aside = path.with_extension(format!(
        "{}-{}.part",
        std::process::id(),
        WRITING.fetch_add(1, Ordering::Relaxed)
    ));
    let written = std::fs::write(&aside, bytes).and_then(|()| std::fs::rename(&aside, path));
    if written.is_err() {
        let _ = std::fs::remove_file(&aside);
    }
    written
}

/// Lets the folders in `root` but `keep` go, the longest unused first, while all of them
/// together (`keep` too) hold more than `cap` bytes.
pub fn prune_others(root: &Path, keep: &Path, cap: u64) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    let mut others: Vec<(PathBuf, u64, SystemTime)> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir() && p != keep)
        .map(|p| {
            let held = files(&p);
            let newest = held
                .iter()
                .map(|f| f.2)
                .max()
                .unwrap_or(SystemTime::UNIX_EPOCH);
            let len = held.iter().map(|f| f.1).sum();
            (p, len, newest)
        })
        .collect();
    let mut total =
        files(keep).iter().map(|f| f.1).sum::<u64>() + others.iter().map(|f| f.1).sum::<u64>();
    others.sort_by_key(|f| f.2);
    for (dir, len, _) in others {
        if total <= cap {
            break;
        }
        if std::fs::remove_dir_all(&dir).is_ok() {
            total -= len;
        }
    }
}

/// The files in a folder and in the folders in it (two deep): path, size, when last written.
fn files(dir: &Path) -> Vec<(PathBuf, u64, SystemTime)> {
    fn collect(dir: &Path, depth: usize, out: &mut Vec<(PathBuf, u64, SystemTime)>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let Ok(m) = e.metadata() else {
                continue;
            };
            if m.is_dir() {
                if depth > 0 {
                    collect(&e.path(), depth - 1, out);
                }
            } else if let Ok(when) = m.modified() {
                out.push((e.path(), m.len(), when));
            }
        }
    }
    let mut out = Vec::new();
    collect(dir, 2, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_keeps_to_its_size_and_other_planets_go_when_all_hold_too_much() {
        let root = std::env::temp_dir().join(format!("hearth-disk-cache-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = root.join("this");
        let folder = Folder::new(&dir, 10_000);
        let bytes = vec![7u8; 1000];
        for k in 0..30 {
            folder
                .write(
                    &dir.join(format!("{}", k % 3)).join(format!("{k}.bin")),
                    &bytes,
                )
                .expect("written");
        }
        assert!(folder.used() <= 10_000, "{} bytes", folder.used());
        assert!(files(&dir).len() < 30, "the oldest let go");
        assert!(dir.join("2").join("29.bin").exists(), "the newest kept");
        // Counted afresh, the same.
        assert_eq!(Folder::new(&dir, 10_000).used(), folder.used());
        // Another planet's folder goes when all together hold more than their most.
        let other = root.join("other");
        Folder::new(&other, 10_000)
            .write(&other.join("a.bin"), &bytes)
            .expect("written");
        prune_others(&root, &dir, 1 << 30);
        assert!(other.exists(), "within the most: kept");
        prune_others(&root, &dir, 1);
        assert!(!other.exists(), "beyond it: let go");
        assert!(dir.exists(), "this one kept");
        let _ = std::fs::remove_dir_all(&root);
    }
}

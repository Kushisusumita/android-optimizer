//! Locating the project folders at runtime, without any hard-coded absolute path.

use std::path::{Path, PathBuf};

pub struct Paths {
    pub root: PathBuf,
    pub lists: PathBuf,
    pub reports: PathBuf,
    pub keep: PathBuf,
}

fn has_lists(dir: &Path) -> bool {
    dir.join("lists").join("protected.txt").is_file()
}

impl Paths {
    /// Find the project root: the first folder (searching up from the CWD, then from the executable)
    /// that contains a `lists/` folder. `override_lists` forces a specific lists directory.
    pub fn locate(override_lists: Option<&str>) -> Option<Paths> {
        if let Some(l) = override_lists {
            let lists = PathBuf::from(l);
            let root = lists.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
            return Some(Paths::from_root(root, lists));
        }
        let mut starts: Vec<PathBuf> = Vec::new();
        if let Ok(cwd) = std::env::current_dir() {
            starts.push(cwd);
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(d) = exe.parent() {
                starts.push(d.to_path_buf());
            }
        }
        for start in starts {
            for dir in start.ancestors() {
                if has_lists(dir) {
                    return Some(Paths::from_root(dir.to_path_buf(), dir.join("lists")));
                }
            }
        }
        None
    }

    fn from_root(root: PathBuf, lists: PathBuf) -> Paths {
        let reports = root.join("reports");
        let keep = root.join("keep");
        Paths { root, lists, reports, keep }
    }

    pub fn list_file(&self, stem: &str) -> PathBuf {
        self.lists.join(format!("{stem}.txt"))
    }

    pub fn keep_file(&self, device_id: &str) -> PathBuf {
        let safe: String =
            device_id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect();
        self.keep.join(format!("{safe}.txt"))
    }
}

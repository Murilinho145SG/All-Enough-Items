//! The AEI bundle as Horizon sees it: manifests, pins and the files it ships.

use crate::Result;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// The bundle folder, relative to the workspace root.
pub const BUNDLE: &str = "mixins/aei";
/// Registry limits for a native bundle (`horizon_mods::index::native`).
pub const MAX_FILES: usize = 512;
pub const MAX_BYTES: u64 = 64 << 20;

pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives inside the workspace")
        .to_path_buf()
}

pub fn read(relative: &str) -> Result<String> {
    let path = root().join(relative);
    fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn json(relative: &str) -> Result<serde_json::Value> {
    serde_json::from_str(&read(relative)?).map_err(|e| format!("{relative}: {e}"))
}

pub fn toml(relative: &str) -> Result<toml::Table> {
    read(relative)?
        .parse()
        .map_err(|e| format!("{relative}: {e}"))
}

/// Whether Horizon keeps `rest`, a path under `mixins/<id>/`, in a bundle: Rust and TOML
/// sources, data folders, and a crate folder's text files. Mirrors
/// `horizon_mods::index::native::allowed`.
pub fn allowed(rest: &str) -> bool {
    let parts: Vec<&str> = rest.split('/').collect();
    let name = parts[parts.len() - 1];
    let in_data =
        parts.len() >= 2 && (parts[0] == "data" || (parts.len() >= 3 && parts[1] == "data"));
    let text = parts.len() <= 2
        && (["README", "LICENSE", "LICENCE", "COPYING", "NOTICE"]
            .iter()
            .any(|prefix| name.starts_with(prefix))
            || name == ".gitignore");
    rest.ends_with(".rs") || rest.ends_with(".toml") || in_data || text
}

/// Every file of the bundle, by its path under `mixins/aei/`. Build output (`target/`) is
/// skipped; links and files Horizon would drop are refused, so nothing is lost silently.
pub fn files() -> Result<BTreeMap<String, PathBuf>> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, PathBuf>) -> Result {
        let entries = fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            let kind = entry.file_type().map_err(|e| e.to_string())?;
            let rest = path
                .strip_prefix(base)
                .expect("walked below the base")
                .to_string_lossy()
                .replace('\\', "/");
            if kind.is_symlink() {
                return Err(format!("{rest}: Horizon refuses links in a bundle"));
            } else if kind.is_dir() {
                if entry.file_name() != "target" {
                    walk(base, &path, out)?;
                }
            } else if allowed(&rest) {
                out.insert(rest, path);
            } else {
                return Err(format!(
                    "{rest}: Horizon would drop this file from the bundle"
                ));
            }
        }
        Ok(())
    }
    let base = root().join(BUNDLE);
    let mut files = BTreeMap::new();
    walk(&base, &base, &mut files)?;
    let bytes: u64 = files
        .values()
        .map(|path| path.metadata().map_or(0, |m| m.len()))
        .sum();
    if files.len() >= MAX_FILES || bytes > MAX_BYTES {
        return Err(format!(
            "the bundle exceeds the registry limit of {MAX_FILES} files or 64 MiB"
        ));
    }
    Ok(files)
}

/// A string field of a TOML table, as `table.key`.
pub fn field<'a>(table: &'a toml::Table, section: &str, key: &str) -> Option<&'a str> {
    table.get(section)?.get(key)?.as_str()
}

//! `cargo xtask install`: package the working tree and build it into a Horizon instance.
//!
//! The `.hnmod` replaces any other AEI bundle in the instance's `horizon-native-mods/`, then
//! `horizon-launcher native-build` rebuilds the instance's game with it. Building trusts the
//! bundle's native code, which is this repository's own source.

use crate::Result;
use crate::{bundle, package};
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn run(home: &Path, instance: &str, launcher: &str) -> Result {
    let dist = bundle::root().join("dist");
    package::run(&dist)?;
    let native = bundle::json("horizon-native.json")?;
    let (Some(id), Some(version)) = (native["id"].as_str(), native["version"].as_str()) else {
        return Err("horizon-native.json needs an id and a version".into());
    };
    let name = format!("{id}-{version}.hnmod");

    let mods = home.join("horizon-native-mods");
    fs::create_dir_all(&mods).map_err(|e| format!("{}: {e}", mods.display()))?;
    for entry in fs::read_dir(&mods).map_err(|e| format!("{}: {e}", mods.display()))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        let file = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if file.starts_with(&format!("{id}-")) && file.ends_with(".hnmod") && file != name {
            fs::remove_file(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        }
    }
    let target = mods.join(&name);
    fs::copy(dist.join(&name), &target).map_err(|e| format!("{}: {e}", target.display()))?;
    println!("installed {}", target.display());

    let status = Command::new(launcher)
        .args([
            "native-build",
            "--instance",
            instance,
            "--trust-native-code",
        ])
        .status()
        .map_err(|e| format!("{launcher}: {e}"))?;
    if !status.success() {
        return Err(format!("native-build failed ({status})"));
    }
    Ok(())
}

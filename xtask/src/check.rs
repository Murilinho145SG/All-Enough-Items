//! `cargo xtask check`: everything that must hold before a commit or a release.

use crate::Result;
use crate::bundle::{self, BUNDLE, field};
use std::path::Path;
use std::process::Command;

pub fn run() -> Result {
    pins()?;
    hygiene()?;
    let root = bundle::root();
    cargo(&root, &["fmt", "--all", "--check"])?;
    // Modules and hook helpers are injected into Horizon, not built by this workspace,
    // so the Cargo formatter never sees them.
    let injected: Vec<String> = bundle::files()?
        .into_keys()
        .filter(|rest| rest.starts_with("modules/") || rest.starts_with("hooks/"))
        .map(|rest| format!("{BUNDLE}/{rest}"))
        .collect();
    let mut rustfmt = vec!["--edition", "2024", "--check"];
    rustfmt.extend(injected.iter().map(String::as_str));
    command(&root, "rustfmt", &rustfmt)?;
    cargo(
        &root,
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--offline",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    cargo(&root, &["test", "--workspace", "--offline"])?;
    println!("Pins, bundle contents, formatting, Clippy and tests passed.");
    Ok(())
}

/// The mod id, version and compatibility pins agree across every manifest, and every module
/// and helper the bundle names exists.
fn pins() -> Result {
    let compatibility = bundle::json("compatibility.json")?;
    let native = bundle::json("horizon-native.json")?;
    let mixin = bundle::toml(&format!("{BUNDLE}/horizon-mixin.toml"))?;
    let engine = bundle::toml(&format!("{BUNDLE}/engine/Cargo.toml"))?;

    let expect = |what: &str, found: Option<&str>, wanted: Option<&str>| match (found, wanted) {
        (Some(found), Some(wanted)) if found == wanted => Ok(()),
        _ => Err(format!("{what}: {found:?} differs from {wanted:?}")),
    };
    let native_str = |key: &str| native[key].as_str();
    expect(
        "horizon-native.json game",
        native_str("game"),
        compatibility["game"].as_str(),
    )?;
    expect(
        "horizon-native.json horizon",
        native_str("horizon"),
        compatibility["horizon_revision"].as_str(),
    )?;
    expect(
        "horizon-native.json mixins",
        native_str("mixins"),
        compatibility["horizon_mixins_revision"].as_str(),
    )?;
    expect(
        "horizon-mixin.toml [compatibility] horizon_revision",
        field(&mixin, "compatibility", "horizon_revision"),
        native_str("horizon"),
    )?;
    expect(
        "horizon-mixin.toml [mod] id",
        field(&mixin, "mod", "id"),
        native_str("id"),
    )?;
    expect(
        "horizon-mixin.toml [mod] version",
        field(&mixin, "mod", "version"),
        native_str("version"),
    )?;
    expect(
        "engine/Cargo.toml version",
        field(&engine, "package", "version"),
        native_str("version"),
    )?;
    if native["hooks"] != compatibility["hooks"] {
        return Err("horizon-native.json hooks differs from compatibility.json".into());
    }

    let base = bundle::root().join(BUNDLE);
    for (list, key) in [("module", "source"), ("injection", "helper")] {
        let entries = mixin
            .get(list)
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten();
        for entry in entries {
            let path = entry
                .get(key)
                .and_then(|v| v.as_str())
                .ok_or_else(|| format!("horizon-mixin.toml: a [[{list}]] has no {key}"))?;
            if !base.join(path).is_file() {
                return Err(format!("horizon-mixin.toml names a missing {key}: {path}"));
            }
        }
    }
    Ok(())
}

/// Only sources Horizon accepts are in the bundle, and no packaged mod, archive or JEI
/// material is anywhere in the source tree.
fn hygiene() -> Result {
    bundle::files()?;
    let root = bundle::root();
    if root.join(BUNDLE).join("engine/data/jei").exists() {
        return Err("JEI assets are prohibited (AGENTS.md)".into());
    }
    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)
            .map_err(|e| e.to_string())?
            .flatten()
        {
            let path = entry.path();
            let name = entry.file_name();
            if path.is_dir() {
                if !matches!(
                    name.to_str(),
                    Some("target" | "dist" | ".git" | "reference")
                ) {
                    stack.push(path);
                }
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str())
                && matches!(
                    ext.to_ascii_lowercase().as_str(),
                    "hnmod" | "hmod" | "jar" | "zip" | "wasm"
                )
            {
                return Err(format!(
                    "{}: keep packaged mods and archives outside the source tree",
                    path.strip_prefix(&root).unwrap_or(&path).display()
                ));
            }
        }
    }
    Ok(())
}

fn cargo(dir: &Path, args: &[&str]) -> Result {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    command(dir, &cargo, args)
}

fn command(dir: &Path, program: &str, args: &[&str]) -> Result {
    let status = Command::new(program)
        .args(args)
        .current_dir(dir)
        .status()
        .map_err(|e| format!("{program}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "{program} {} failed ({status})",
            args.first().unwrap_or(&"")
        ))
    }
}

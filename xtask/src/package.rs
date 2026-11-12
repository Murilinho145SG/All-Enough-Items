//! `cargo xtask package`: a reproducible `.hnmod` for a GitHub release.
//!
//! The archive holds `horizon-native.json` and every bundle file under `mixins/aei/`, plus
//! the repository LICENSE. Entries are sorted with fixed timestamps and permissions, so the
//! same sources always give the same bytes.

use crate::Result;
use crate::bundle;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Cursor, Write};
use std::path::Path;
use zip::write::SimpleFileOptions;

pub fn run(out: &Path) -> Result {
    let native = bundle::json("horizon-native.json")?;
    let (Some(id), Some(version)) = (native["id"].as_str(), native["version"].as_str()) else {
        return Err("horizon-native.json needs an id and a version".into());
    };

    let mut entries = vec![(
        "horizon-native.json".to_owned(),
        bundle::read("horizon-native.json")?.into_bytes(),
    )];
    let files = bundle::files()?;
    let license = (!files.contains_key("LICENSE")).then(|| bundle::root().join("LICENSE"));
    let mut sources: Vec<_> = files.into_iter().collect();
    sources.extend(license.map(|path| ("LICENSE".to_owned(), path)));
    sources.sort();
    for (rest, path) in sources {
        let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        entries.push((format!("mixins/{id}/{rest}"), bytes));
    }

    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(0o644);
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in &entries {
        zip.start_file(name.as_str(), options)
            .map_err(|e| e.to_string())?;
        zip.write_all(bytes).map_err(|e| e.to_string())?;
    }
    let archive = zip.finish().map_err(|e| e.to_string())?.into_inner();

    fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let name = format!("{id}-{version}.hnmod");
    let target = out.join(&name);
    let digest = format!("{:x}", Sha256::digest(&archive));
    fs::write(&target, &archive).map_err(|e| format!("{}: {e}", target.display()))?;
    let sum = out.join(format!("{name}.sha256"));
    fs::write(&sum, format!("{digest}  {name}\n"))
        .map_err(|e| format!("{}: {e}", sum.display()))?;
    println!(
        "{} ({} files)\nsha256 {digest}",
        target.display(),
        entries.len()
    );
    Ok(())
}

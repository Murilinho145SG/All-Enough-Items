//! `cargo xtask ui-test SOCKET --out DIR`: exercise an AEI test build already running in a
//! disposable survival world through its control socket. Screenshots and the result are
//! written to `DIR`, never into the source tree.

use crate::Result;
use serde_json::{Value, json};
use std::path::Path;

const CHECKS: [&str; 9] = [
    "recipe pages",
    "scroll",
    "categories",
    "category recipes",
    "modal clicks",
    "history",
    "clear search",
    "typing",
    "return to catalogue",
];

#[cfg(not(unix))]
pub fn run(_socket: &Path, _out: &Path) -> Result {
    Err("ui-test needs a Unix control socket".into())
}

#[cfg(unix)]
pub fn run(socket: &Path, out: &Path) -> Result {
    let out = std::path::absolute(out).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mut game = Game::connect(socket)?;

    game.send("options-set tutorialStep none")?;
    if game.send("state")?["screen"] != "inventory" {
        game.send("open-inventory")?;
    }
    for _ in 0..20 {
        if game.ingredients()?.is_empty() {
            break;
        }
        game.send("key esc tap")?;
    }
    let field = game.field()?;
    game.send(&format!("click {field}"))?;
    game.select_all()?;
    for _ in 0..50 {
        game.send(r"type \b")?;
    }

    game.click("Stone", true)?;
    let first = game.ingredients()?;
    ensure(
        first.iter().any(|label| label.starts_with("Recipe 1 ")),
        "the first recipe page shows recipe 1",
    )?;
    game.click("Next recipe page", false)?;
    let second = game.ingredients()?;
    ensure(first != second, "the next page shows other recipes")?;
    game.send("scroll-at 210 120 -1")?;
    ensure(game.ingredients()? != second, "scrolling turns the page")?;
    game.click("Recipe category 1", false)?;
    ensure(
        !game.ingredients()?.is_empty(),
        "a second category shows recipes",
    )?;
    game.click("Recipe category 0", false)?;
    game.click("Show all recipes in category", false)?;
    ensure(
        !game.ingredients()?.is_empty(),
        "a category lists its recipes",
    )?;

    // Modal blank space must consume clicks; history works without a hovered item.
    game.send("click-at 1 1")?;
    game.send("key left tap")?;
    ensure(
        !game.ingredients()?.is_empty(),
        "Back keeps the previous lookup open",
    )?;
    game.send("key left tap")?;
    ensure(game.ingredients()?.is_empty(), "Back reaches the catalogue")?;
    game.send("key right tap")?;
    ensure(
        !game.ingredients()?.is_empty(),
        "Forward reopens the first lookup",
    )?;

    let field = game.field()?;
    game.send(&format!("click {field}"))?;
    game.send("type nonexistent-aei-fixture")?;
    ensure(!game.has_items()?, "an unmatched search hides every item")?;
    game.select_all()?;
    game.send(&format!("click {field} right"))?;
    game.send("type oak")?;
    ensure(
        game.has_items()?,
        "right-click clears the search and typing filters again",
    )?;
    let field = game.field()?;
    game.send(&format!("click {field} right"))?;
    game.send("click-at 1 1")?;
    ensure(
        game.send("state")?["screen"] == "inventory",
        "the inventory stays open",
    )?;
    game.send(&format!("screenshot {}", out.join("recipes.png").display()))?;
    game.send("key esc tap")?;
    ensure(
        game.ingredients()?.is_empty(),
        "Escape returns to the catalogue",
    )?;
    game.send(&format!(
        "screenshot {}",
        out.join("catalogue.png").display()
    ))?;

    let result = json!({ "status": "passed", "checks": CHECKS });
    let text = serde_json::to_string_pretty(&result).map_err(|e| e.to_string())? + "\n";
    std::fs::write(out.join("result.json"), text).map_err(|e| e.to_string())?;
    println!(
        "Viewer interaction checks passed; screenshots: {}",
        out.display()
    );
    Ok(())
}

fn ensure(condition: bool, what: &str) -> Result {
    if condition {
        Ok(())
    } else {
        Err(format!("viewer check failed: {what}"))
    }
}

#[cfg(unix)]
struct Game {
    reader: std::io::BufReader<std::os::unix::net::UnixStream>,
    writer: std::os::unix::net::UnixStream,
}

#[cfg(unix)]
impl Game {
    fn connect(socket: &Path) -> Result<Self> {
        let stream = std::os::unix::net::UnixStream::connect(socket)
            .map_err(|e| format!("{}: {e}", socket.display()))?;
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(45)))
            .map_err(|e| e.to_string())?;
        let writer = stream.try_clone().map_err(|e| e.to_string())?;
        Ok(Self {
            reader: std::io::BufReader::new(stream),
            writer,
        })
    }

    /// Sends one control line; a reply carrying `error` fails the test.
    fn send(&mut self, line: &str) -> Result<Value> {
        use std::io::{BufRead, Write};
        writeln!(self.writer, "{line}").map_err(|e| format!("{line}: {e}"))?;
        let mut reply = String::new();
        self.reader
            .read_line(&mut reply)
            .map_err(|e| format!("{line}: {e}"))?;
        let reply: Value = serde_json::from_str(&reply).map_err(|e| format!("{line}: {e}"))?;
        if reply.get("error").is_some() {
            return Err(format!("{line}: {reply}"));
        }
        Ok(reply)
    }

    fn widgets(&mut self) -> Result<Vec<Value>> {
        match self.send("ui")?["widgets"].take() {
            Value::Array(widgets) => Ok(widgets),
            other => Err(format!("ui: no widget list in {other}")),
        }
    }

    fn labels(&mut self) -> Result<Vec<String>> {
        Ok(self
            .widgets()?
            .iter()
            .filter_map(|w| w["label"].as_str().map(str::to_owned))
            .collect())
    }

    fn ingredients(&mut self) -> Result<Vec<String>> {
        Ok(self
            .labels()?
            .into_iter()
            .filter(|label| label.starts_with("Recipe ") && label.contains(" ingredient "))
            .collect())
    }

    fn has_items(&mut self) -> Result<bool> {
        Ok(self
            .labels()?
            .iter()
            .any(|label| label.starts_with("Item ")))
    }

    /// The id of the first text field on screen.
    fn field(&mut self) -> Result<String> {
        self.widgets()?
            .iter()
            .find(|w| w["kind"] == "field")
            .and_then(|w| w["id"].as_str().map(str::to_owned))
            .ok_or_else(|| "no text field on screen".into())
    }

    fn click(&mut self, label: &str, right: bool) -> Result {
        let id = self
            .widgets()?
            .iter()
            .find(|w| w["label"] == label)
            .and_then(|w| w["id"].as_str().map(str::to_owned))
            .ok_or_else(|| format!("no widget labelled {label:?}"))?;
        let suffix = if right { " right" } else { "" };
        self.send(&format!("click {id}{suffix}")).map(drop)
    }

    fn select_all(&mut self) -> Result {
        self.send("key ctrl down")?;
        self.send("key a tap")?;
        self.send("key ctrl up").map(drop)
    }
}

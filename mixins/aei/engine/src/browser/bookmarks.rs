//! JEI's bookmarks: stacks and recipes the player keeps beside the screen, saved as text.
//!
//! One bookmark per line, tab-separated: `item` or `recipe`, the item id, its components
//! (possibly empty) and, for a recipe, the recipe id.

use super::Browser;
use crate::{Bookmark, Lookup, Stack, Station, Update};

/// Bookmarks kept at most, as a guard against a corrupt file.
const LIMIT: usize = 4096;

pub(crate) fn parse(text: &str) -> Vec<Bookmark> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.split('\t');
            let kind = fields.next()?;
            let id = fields.next().filter(|id| id.contains(':'))?;
            let components = fields.next().unwrap_or_default();
            let recipe = match kind {
                "item" => None,
                "recipe" => Some(fields.next().filter(|r| !r.is_empty())?.to_owned()),
                _ => return None,
            };
            Some(Bookmark {
                stack: Stack {
                    id: id.to_owned(),
                    count: 1,
                    components: components.to_owned(),
                },
                recipe,
            })
        })
        .take(LIMIT)
        .collect()
}

pub(crate) fn format(bookmarks: &[Bookmark]) -> String {
    bookmarks
        .iter()
        .map(|b| match &b.recipe {
            None => format!("item\t{}\t{}\n", b.stack.id, b.stack.components),
            Some(recipe) => format!("recipe\t{}\t{}\t{recipe}\n", b.stack.id, b.stack.components),
        })
        .collect()
}

impl Browser {
    /// Replaces the bookmarks with the saved `text`.
    pub fn load_bookmarks(&mut self, text: &str) -> Update {
        self.bookmarks = parse(text);
        Update::Bookmarks(self.bookmarks.clone())
    }

    /// The bookmarks as the text [`Browser::load_bookmarks`] reads.
    pub fn bookmarks_text(&self) -> String {
        format(&self.bookmarks)
    }

    /// The shown bookmarks as a collection to share as text: a `# name` line, then
    /// one bookmark per line, as [`Browser::import_collection`] reads it.
    pub fn collection_text(&self, name: &str) -> String {
        format!("# {}\n{}", name.trim(), format(&self.bookmarks))
    }

    /// Replaces the shown bookmarks with a shared collection: lines as
    /// [`Browser::collection_text`] writes them, or plain item ids one per line. The
    /// collection's name (its `# name` line), if any, comes back with the update; text
    /// with no bookmark in it changes nothing.
    pub fn import_collection(&mut self, text: &str) -> Option<(Option<String>, Update)> {
        let name = text
            .lines()
            .find_map(|line| line.trim().strip_prefix("# "))
            .map(|name| name.trim().to_owned())
            .filter(|name| !name.is_empty());
        let lines: String = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.starts_with('#') && !line.is_empty())
            .map(|line| {
                if line.contains('\t') {
                    format!("{line}\n")
                } else {
                    format!("item\t{line}\t\n")
                }
            })
            .collect();
        let bookmarks = parse(&lines);
        if bookmarks.is_empty() {
            return None;
        }
        self.bookmarks = bookmarks;
        Some((name, Update::Bookmarks(self.bookmarks.clone())))
    }

    fn changed(&self) -> Vec<Update> {
        vec![Update::Bookmarks(self.bookmarks.clone())]
    }

    pub(crate) fn toggle_bookmark(&mut self, mut stack: Stack) -> Vec<Update> {
        if stack.id.is_empty() || stack.id == "minecraft:air" {
            return Vec::new();
        }
        stack.count = 1;
        let item = Bookmark {
            stack,
            recipe: None,
        };
        match self.bookmarks.iter().position(|b| *b == item) {
            Some(i) => {
                self.bookmarks.remove(i);
            }
            None if self.bookmarks.len() < LIMIT => self.bookmarks.push(item),
            None => return Vec::new(),
        }
        self.changed()
    }

    pub(crate) fn bookmark_recipe(&mut self, card: usize) -> Vec<Update> {
        let Some(&recipe) = self.shown_recipe_ids.get(card) else {
            return Vec::new();
        };
        let recipe = &self.recipes[recipe];
        let Some(output) = recipe.outputs.first() else {
            return Vec::new();
        };
        if recipe.id.is_empty() {
            return Vec::new();
        }
        let bookmark = Bookmark {
            stack: Stack {
                count: 1,
                ..self.as_stack(output)
            },
            recipe: Some(recipe.id.clone()),
        };
        match self.bookmarks.iter().position(|b| *b == bookmark) {
            Some(i) => {
                self.bookmarks.remove(i);
            }
            None if self.bookmarks.len() < LIMIT => self.bookmarks.push(bookmark),
            None => return Vec::new(),
        }
        self.changed()
    }

    /// Whether card `card` of the shown page is bookmarked.
    pub fn shown_recipe_bookmarked(&self, card: usize) -> bool {
        self.shown_recipe_id(card).is_some_and(|id| {
            self.bookmarks
                .iter()
                .any(|b| b.recipe.as_deref() == Some(id))
        })
    }

    pub(crate) fn move_bookmark(&mut self, from: usize, to: usize) -> Vec<Update> {
        if from >= self.bookmarks.len() || from == to {
            return Vec::new();
        }
        let bookmark = self.bookmarks.remove(from);
        self.bookmarks
            .insert(to.min(self.bookmarks.len()), bookmark);
        self.changed()
    }

    /// Opens a recipe bookmark: its output's recipes, on the recipe's category with the
    /// recipe first. A recipe that no longer exists opens the output's recipes.
    pub(crate) fn open_recipe(&mut self, output: &Stack, id: &str, known: &[i32]) -> Vec<Update> {
        let Some(focus) = self.recipes.iter().position(|r| r.id == id) else {
            return self
                .open_lookup(output.clone(), false, known)
                .into_iter()
                .collect();
        };
        let mut lookup = Lookup {
            focus: Some(focus),
            category: None,
            item: output.clone(),
            uses: false,
            station: 0,
            tag: None,
        };
        let ids = self.page_recipe_ids(&lookup);
        if ids.is_empty() {
            return Vec::new();
        }
        let wanted = Station::of(self.recipes[focus].kind);
        lookup.station = self
            .stations()
            .filter(|s| ids.iter().any(|&i| Station::of(self.recipes[i].kind) == *s))
            .position(|s| s == wanted)
            .unwrap_or(0);
        self.push_lookup(lookup, known)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn collections_round_trip_as_text() {
        let (mut browser, _) = crate::Browser::new(Vec::new(), Vec::new());
        browser.load_bookmarks(
            "item\tminecraft:stone\t\nrecipe\tminecraft:stick\t\tminecraft:stick\n",
        );
        let text = browser.collection_text(" Redstone ");
        assert!(text.starts_with("# Redstone\n"));
        let saved = browser.bookmarks_text();
        browser.load_bookmarks("");
        let (name, _) = browser.import_collection(&text).unwrap();
        assert_eq!(name.as_deref(), Some("Redstone"));
        assert_eq!(browser.bookmarks_text(), saved);
        // Plain ids, one per line, import too; text with none changes nothing.
        let (name, _) = browser
            .import_collection("minecraft:dirt\n\nminecraft:sand")
            .unwrap();
        assert_eq!(name, None);
        assert_eq!(
            browser.bookmarks_text(),
            "item\tminecraft:dirt\t\nitem\tminecraft:sand\t\n"
        );
        assert!(browser.import_collection("hello there").is_none());
        assert_eq!(browser.bookmarks_text().lines().count(), 2);
    }

    use super::*;

    #[test]
    fn bookmarks_survive_a_round_trip_and_skip_bad_lines() {
        let text = "item\tminecraft:stick\t\n\
                    recipe\tminecraft:oak_planks\t\tminecraft:oak_planks\n\
                    item\tbroken\n\
                    recipe\tminecraft:stick\t\t\n\
                    nonsense\n\
                    item\tminecraft:potion\t[potion_contents={potion:\"minecraft:healing\"}]\n";
        let bookmarks = parse(text);
        assert_eq!(bookmarks.len(), 3);
        assert_eq!(bookmarks[1].recipe.as_deref(), Some("minecraft:oak_planks"));
        assert_eq!(parse(&format(&bookmarks)), bookmarks);
    }
}

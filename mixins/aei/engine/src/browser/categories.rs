//! The recipe categories' tab order and the hidden ones, as JEI's
//! `recipe-category-sort-order` configuration sets them.
//!
//! The text holds one station id per line (`minecraft:furnace`) in tab order; a leading
//! `!` hides that category, and `#` starts a comment. Categories the text leaves out
//! follow in their default order.

use super::Browser;
use crate::{RecipeKind, Station};

impl Browser {
    /// Applies the saved category `text`.
    pub fn load_categories(&mut self, text: &str) {
        let mut order = Vec::new();
        let mut hidden = Vec::new();
        for line in text.lines().map(str::trim) {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (hide, id) = match line.strip_prefix('!') {
                Some(id) => (true, id.trim()),
                None => (false, line),
            };
            let Some(station) = Station::ALL.into_iter().find(|s| s.icon() == id) else {
                continue;
            };
            if !order.contains(&station) && !hidden.contains(&station) {
                if hide {
                    hidden.push(station);
                } else {
                    order.push(station);
                }
            }
        }
        let rest: Vec<Station> = Station::ALL
            .into_iter()
            .filter(|s| !order.contains(s) && !hidden.contains(s))
            .collect();
        order.extend(rest);
        self.categories = order;
        self.hidden_categories = hidden;
    }

    /// The category text [`Browser::load_categories`] reads, with every category listed.
    pub fn categories_text(&self) -> String {
        let mut text = String::from(
            "# AEI recipe categories in tab order. Move lines to reorder them;\n\
             # put ! before one to hide it.\n",
        );
        for station in &self.categories {
            text.push_str(station.icon());
            text.push('\n');
        }
        for station in &self.hidden_categories {
            text.push('!');
            text.push_str(station.icon());
            text.push('\n');
        }
        text
    }

    /// The shown categories in tab order.
    pub(super) fn stations(&self) -> impl Iterator<Item = Station> + '_ {
        self.categories.iter().copied()
    }

    /// How many of a category's recipe-book recipes the player has unlocked, and how
    /// many it has; `station` is the category's station id. None without book recipes.
    pub fn progress(&self, station: &str, known: &[i32]) -> Option<(usize, usize)> {
        let station = Station::ALL.into_iter().find(|s| s.icon() == station)?;
        let book: Vec<i32> = self
            .recipes
            .iter()
            .filter(|r| Station::of(r.kind) == station && r.book_id >= 0)
            .map(|r| r.book_id)
            .collect();
        if book.is_empty() {
            return None;
        }
        let unlocked = book.iter().filter(|id| known.contains(id)).count();
        Some((unlocked, book.len()))
    }

    pub(super) fn shows_category(&self, kind: RecipeKind) -> bool {
        self.categories.contains(&Station::of(kind))
    }
}

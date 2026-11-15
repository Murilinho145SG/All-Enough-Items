//! The item list's order and JEI's edit mode, which hides entries from it.
//!
//! Hidden entries are saved as text, one per line: the item id, a tab, then the stack's
//! components, or `*` for every stack of the item.

use super::Browser;
use crate::{SortOrder, Update};

const WILDCARD: &str = "*";

impl Browser {
    /// Replaces the hidden entries with the saved `text`.
    pub fn load_hidden(&mut self, text: &str) -> Update {
        self.hidden = text
            .lines()
            .filter_map(|line| {
                let (id, components) = line.split_once('\t').unwrap_or((line, ""));
                id.contains(':')
                    .then(|| (id.to_owned(), components.to_owned()))
            })
            .collect();
        self.item_list()
    }

    /// The hidden entries as the text [`Browser::load_hidden`] reads, sorted.
    pub fn hidden_text(&self) -> String {
        let mut lines: Vec<String> = self
            .hidden
            .iter()
            .map(|(id, components)| format!("{id}\t{components}\n"))
            .collect();
        lines.sort();
        lines.concat()
    }

    /// Whether catalogue entry `entry` is hidden.
    pub(super) fn is_hidden(&self, entry: usize) -> bool {
        let stack = &self.catalogue[entry].stack;
        !self.hidden.is_empty()
            && (self
                .hidden
                .contains(&(stack.id.clone(), stack.components.clone()))
                || self
                    .hidden
                    .contains(&(stack.id.clone(), WILDCARD.to_owned())))
    }

    /// Whether the shown list entry `index` is hidden (only edit mode shows those).
    pub fn shown_hidden(&self, index: usize) -> bool {
        self.edit_mode
            && self
                .filtered_items
                .get(index)
                .is_some_and(|&entry| self.is_hidden(entry))
    }

    pub(super) fn toggle_hidden(&mut self, index: usize, wildcard: bool) -> Vec<Update> {
        let Some(&entry) = self.filtered_items.get(index) else {
            return Vec::new();
        };
        let stack = &self.catalogue[entry].stack;
        let key = (
            stack.id.clone(),
            if wildcard {
                WILDCARD.to_owned()
            } else {
                stack.components.clone()
            },
        );
        if !self.hidden.remove(&key) {
            // Showing a stack hidden by its item's wildcard shows the whole item again.
            let wild = (stack.id.clone(), WILDCARD.to_owned());
            if !wildcard && self.hidden.remove(&wild) {
                return vec![Update::Hidden, self.item_list()];
            }
            self.hidden.insert(key);
        }
        vec![Update::Hidden, self.item_list()]
    }

    /// Orders the catalogue as `order` asks; the creative order breaks ties.
    pub(super) fn sort(&mut self, order: SortOrder) {
        let mut ranks: Vec<usize> = (0..self.catalogue.len()).collect();
        match order {
            SortOrder::Creative => {}
            SortOrder::Mod => {
                let namespace = |i: usize| {
                    let id = &self.catalogue[i].stack.id;
                    let ns = id.split_once(':').map_or("minecraft", |(ns, _)| ns);
                    // JEI lists Minecraft before the mods.
                    (ns != "minecraft", ns.to_owned())
                };
                ranks.sort_by_cached_key(|&i| namespace(i));
            }
            SortOrder::Name => {
                ranks.sort_by_cached_key(|&i| self.catalogue[i].name.to_lowercase());
            }
        }
        self.order = ranks;
    }
}

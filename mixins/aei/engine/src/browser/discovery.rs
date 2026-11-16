//! What the player has found: items that were ever in the inventory, for the
//! spoiler-free mode (only found items and unlocked recipes show) and the "new" marks
//! on items found this session but not looked at yet.

use super::Browser;
use crate::Update;

impl Browser {
    /// Replaces the found items with the saved `text`: one id per line, a `*` before
    /// the ones still new (not looked at yet).
    pub fn load_seen(&mut self, text: &str) {
        self.seen.clear();
        self.fresh.clear();
        for line in text.lines() {
            let (fresh, id) = match line.strip_prefix('*') {
                Some(id) => (true, id),
                None => (false, line),
            };
            if id.contains(':') {
                self.seen.insert(id.to_owned());
                if fresh {
                    self.fresh.insert(id.to_owned());
                }
            }
        }
    }

    pub fn seen_text(&self) -> String {
        let mut ids: Vec<&String> = self.seen.union(&self.fresh).collect();
        ids.sort();
        ids.dedup();
        ids.into_iter()
            .map(|id| {
                let mark = if self.fresh.contains(id) { "*" } else { "" };
                format!("{mark}{id}\n")
            })
            .collect()
    }

    /// From now on, recipes that unlock mark what they make as new (the game sends the
    /// player's recipes in several parts on joining; those aren't news).
    pub fn watch_unlocks(&mut self) {
        self.watch_unlocks = true;
    }

    /// How many items are new (found or newly unlocked, not looked at yet).
    pub fn fresh_count(&self) -> usize {
        self.fresh.len()
    }

    /// Every item counts as looked at: no new marks are left.
    pub fn clear_fresh(&mut self) -> Update {
        self.fresh.clear();
        for entry in &mut self.indexed {
            entry.fresh = false;
        }
        self.item_list()
    }

    /// Notes the inventory's items as found; true when a new one was.
    pub(super) fn discover(&mut self) -> bool {
        let mut found = false;
        for held in &self.inventory {
            if self.seen.insert(held.id.clone()) {
                self.fresh.insert(held.id.clone());
                found = true;
            }
        }
        found
    }

    /// Whether list entry `index` holds an item found this session and not looked at.
    pub fn shown_fresh(&self, index: usize) -> bool {
        self.filtered_items
            .get(index)
            .is_some_and(|&entry| self.fresh.contains(&self.catalogue[entry].stack.id))
    }

    /// Whether `item` may show in spoiler-free mode: found, or made by an unlocked
    /// recipe.
    pub(super) fn discovered(&self, item: &str) -> bool {
        self.seen.contains(item)
            || self.by_id.get(item).is_some_and(|&i| {
                self.recipe_index.produced_by(i).iter().any(|&id| {
                    let book = self.recipes[id].book_id;
                    book >= 0 && self.known.contains(&book)
                })
            })
    }

    /// Spoiler-free mode hides recipes the player hasn't unlocked.
    pub(super) fn recipe_unlocked(&self, id: usize) -> bool {
        let book = self.recipes[id].book_id;
        !self.spoiler_free || book < 0 || self.known.contains(&book)
    }

    pub(super) fn set_spoiler_free(&mut self, on: bool) -> Vec<Update> {
        if on == self.spoiler_free {
            return Vec::new();
        }
        self.spoiler_free = on;
        vec![self.item_list()]
    }

    pub(super) fn set_known(&mut self, known: Vec<i32>) -> Vec<Update> {
        if known == self.known {
            return Vec::new();
        }
        // What a recipe unlocked now makes is new (not while the game is still sending
        // the recipes the player had before: see `watch_unlocks`).
        if self.watch_unlocks {
            let added: Vec<i32> = known
                .iter()
                .copied()
                .filter(|id| !self.known.contains(id))
                .collect();
            for recipe in self.recipes.iter().filter(|r| added.contains(&r.book_id)) {
                for output in &recipe.outputs {
                    if let Some(item) = self.items.get(output.item) {
                        self.fresh.insert(item.id.clone());
                    }
                }
            }
        }
        self.known = known;
        self.refresh_locked();
        if self.search.reads_known() {
            return vec![self.item_list()];
        }
        if self.spoiler_free {
            vec![self.item_list()]
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Browser, Event, Ingredient, ItemInfo, Recipe, RecipeKind, Stack, Update};

    fn stack(id: &str) -> Stack {
        Stack {
            id: format!("minecraft:{id}"),
            count: 1,
            components: String::new(),
        }
    }

    fn shown(updates: &[Update]) -> Vec<String> {
        match updates.first() {
            Some(Update::ItemList(list)) => list.iter().map(|s| s.id.clone()).collect(),
            other => panic!("expected item list, got {other:?}"),
        }
    }

    #[test]
    fn spoiler_free_mode_shows_found_items_and_unlocked_recipes_only() {
        let items = ["log", "planks", "stick"]
            .map(|id| ItemInfo {
                id: format!("minecraft:{id}"),
                name: id.into(),
                tags: vec![],
            })
            .to_vec();
        let one = |item| {
            vec![Ingredient {
                item,
                count: 1,
                components: String::new(),
            }]
        };
        let recipe = |input, output, book_id| Recipe {
            id: String::new(),
            kind: RecipeKind::Shapeless,
            width: 0,
            height: 0,
            inputs: vec![one(input)],
            outputs: one(output),
            ticks: 0,
            experience: 0.0,
            book_id,
        };
        let (mut browser, _) = Browser::new(items, vec![recipe(0, 1, 1), recipe(1, 2, 2)]);
        let list = browser.handle(Event::SetSpoilerFree(true), false, &[]);
        assert!(shown(&list).is_empty());
        browser.handle(
            Event::SetInventory {
                stacks: vec![stack("log")],
                grid: 2,
            },
            false,
            &[],
        );
        assert!(browser.seen_text().contains("minecraft:log"));
        // Planks' recipe is unlocked: planks show too; sticks' isn't.
        let list = browser.handle(Event::SetKnown(vec![1]), false, &[]);
        assert_eq!(shown(&list), ["minecraft:log", "minecraft:planks"]);
        assert!(browser.shown_fresh(0) && !browser.shown_fresh(1));
        // Looking the log up clears its mark.
        browser.handle(
            Event::Lookup {
                stack: stack("log"),
                uses: true,
            },
            false,
            &[1],
        );
        assert!(!browser.shown_fresh(0));
        // The stick recipe (locked) doesn't show among the planks' uses.
        let uses = browser.handle(
            Event::Lookup {
                stack: stack("planks"),
                uses: true,
            },
            false,
            &[1],
        );
        assert!(uses.is_empty());
    }

    #[test]
    fn newly_unlocked_recipes_mark_their_items_new_and_marks_are_saved() {
        let items = ["log", "planks", "stick"]
            .map(|id| ItemInfo {
                id: format!("minecraft:{id}"),
                name: id.into(),
                tags: vec![],
            })
            .to_vec();
        let one = |item| {
            vec![Ingredient {
                item,
                count: 1,
                components: String::new(),
            }]
        };
        let recipe = |input, output, book_id| Recipe {
            id: String::new(),
            kind: RecipeKind::Shapeless,
            width: 0,
            height: 0,
            inputs: vec![one(input)],
            outputs: one(output),
            ticks: 0,
            experience: 0.0,
            book_id,
        };
        let (mut browser, _) = Browser::new(items, vec![recipe(0, 1, 1), recipe(1, 2, 2)]);
        // What comes while joining is what the player already had: nothing new.
        browser.handle(Event::SetKnown(vec![1]), false, &[]);
        assert_eq!(browser.fresh_count(), 0);
        browser.watch_unlocks();
        browser.handle(Event::SetKnown(vec![1, 2]), false, &[]);
        assert_eq!(browser.fresh_count(), 1);
        let text = browser.seen_text();
        assert_eq!(text, "*minecraft:stick\n");
        let (mut other, _) = Browser::new(Vec::new(), Vec::new());
        other.load_seen(&text);
        assert_eq!(other.fresh_count(), 1);
        assert_eq!(other.seen_text(), text);
    }
}

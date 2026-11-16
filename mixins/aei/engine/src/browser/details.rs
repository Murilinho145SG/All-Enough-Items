//! An item's details panel: what AEI knows about one stack, for the I key.

use super::Browser;
use crate::Stack;

/// One stack's details, as lines of `label: value`, and its tags.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Details {
    pub name: String,
    pub lines: Vec<(String, String)>,
    /// Its item tags, without the `#`, sorted.
    pub tags: Vec<String>,
}

impl Browser {
    /// The player's note on `item`, if any.
    pub fn note(&self, item: &str) -> Option<&str> {
        self.notes.get(item).map(String::as_str)
    }

    /// Sets (or, when blank, removes) the note on `item`.
    pub fn set_note(&mut self, item: &str, note: &str) {
        let note = note.trim();
        if note.is_empty() {
            self.notes.remove(item);
        } else {
            self.notes.insert(item.to_owned(), note.to_owned());
        }
    }

    /// The notes as text, one `id<TAB>note` per line, sorted.
    pub fn notes_text(&self) -> String {
        let mut lines: Vec<String> = self
            .notes
            .iter()
            .map(|(id, note)| format!("{id}\t{note}\n"))
            .collect();
        lines.sort();
        lines.concat()
    }

    pub fn load_notes(&mut self, text: &str) {
        self.notes = text
            .lines()
            .filter_map(|line| line.split_once('\t'))
            .filter(|(id, note)| id.contains(':') && !note.trim().is_empty())
            .map(|(id, note)| (id.to_owned(), note.trim().to_owned()))
            .collect();
    }

    /// How many recipes make and use `item` (by id).
    pub fn counts(&self, item: &str) -> Option<(usize, usize)> {
        let &i = self.by_id.get(item)?;
        Some((
            self.recipe_index.produced_by(i).len(),
            self.recipe_index.used_by(i),
        ))
    }

    /// The details of `stack`, or None for an item the registry doesn't have.
    pub fn details(&self, stack: &Stack) -> Option<Details> {
        let &item = self.by_id.get(&stack.id)?;
        let info = &self.items[item];
        let entry = self
            .catalogue
            .iter()
            .position(|e| e.stack.id == stack.id && e.stack.components == stack.components)
            .or_else(|| self.catalogue.iter().position(|e| e.stack.id == stack.id));
        let entry = entry.map(|i| &self.catalogue[i]);
        let namespace = stack.id.split_once(':').map_or("minecraft", |(ns, _)| ns);
        let mut lines = vec![
            ("Id".to_owned(), stack.id.clone()),
            ("Mod".to_owned(), crate::mod_name(namespace)),
        ];
        if let Some(entry) = entry {
            for alias in &entry.aliases {
                lines.push(("English".to_owned(), alias.clone()));
            }
            let p = &entry.props;
            lines.push(("Stack".to_owned(), p.max_stack.to_string()));
            if p.durability > 0 {
                lines.push(("Durability".to_owned(), p.durability.to_string()));
            }
            if let Some(nutrition) = p.nutrition {
                lines.push(("Food".to_owned(), nutrition.to_string()));
            }
            if !p.rarity.is_empty() && p.rarity != "common" {
                lines.push(("Rarity".to_owned(), p.rarity.clone()));
            }
            if !entry.tabs.is_empty() {
                lines.push(("Creative tabs".to_owned(), entry.tabs.join(", ")));
            }
        }
        let made = self.recipe_index.produced_by(item).len();
        let used = self.recipe_index.used_by(item);
        lines.push((
            "Recipes".to_owned(),
            format!("{made} make it, {used} use it"),
        ));
        if !stack.components.is_empty() {
            lines.push(("Components".to_owned(), stack.components.clone()));
        }
        if let Some(note) = self.notes.get(&stack.id) {
            lines.push(("Note".to_owned(), note.clone()));
        }
        let mut tags = info.tags.clone();
        tags.sort();
        Some(Details {
            name: entry.map_or_else(|| info.name.clone(), |e| e.name.clone()),
            lines,
            tags,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{Browser, ItemInfo, Stack};

    #[test]
    fn details_list_the_id_mod_recipes_and_sorted_tags() {
        let items = vec![ItemInfo {
            id: "test_mod:thing".into(),
            name: "Thing".into(),
            tags: vec!["z:tag".into(), "a:tag".into()],
        }];
        let (browser, _) = Browser::new(items, Vec::new());
        let stack = Stack {
            id: "test_mod:thing".into(),
            count: 1,
            components: String::new(),
        };
        let details = browser.details(&stack).unwrap();
        assert_eq!(details.name, "Thing");
        assert_eq!(details.tags, ["a:tag", "z:tag"]);
        assert!(details.lines.contains(&("Mod".into(), "Test Mod".into())));
        assert!(
            details
                .lines
                .contains(&("Recipes".into(), "0 make it, 0 use it".into()))
        );
        let missing = Stack {
            id: "test_mod:none".into(),
            ..stack
        };
        assert!(browser.details(&missing).is_none());
    }
}

impl Browser {
    /// Card `card` of the shown page as Markdown, for the clipboard: its result, its
    /// category and id, then its ingredients counted (a tag where a slot takes any).
    pub fn recipe_text(&self, card: usize) -> Option<String> {
        let &id = self.shown_recipe_ids.get(card)?;
        let recipe = &self.recipes[id];
        let name = |item: usize| self.items.get(item).map_or("?", |i| i.name.as_str());
        let output = recipe.outputs.first()?;
        let category = crate::Station::of(recipe.kind)
            .icon()
            .trim_start_matches("minecraft:")
            .replace('_', " ");
        let mut text = format!(
            "**{} \u{d7} {}** ({category}",
            name(output.item),
            output.count
        );
        if !recipe.id.is_empty() {
            text.push_str(&format!(", `{}`", recipe.id));
        }
        text.push_str(")\n");
        // Each ingredient once, with how many the recipe takes.
        let mut lines: Vec<(String, u32)> = Vec::new();
        for (slot, alternatives) in recipe.inputs.iter().enumerate() {
            let Some(first) = alternatives.first() else {
                continue;
            };
            let label = match self.shown_slot_tag(card, slot) {
                Some(tag) => format!("any `#{tag}`"),
                None => name(first.item).to_owned(),
            };
            match lines.iter_mut().find(|(l, _)| *l == label) {
                Some((_, n)) => *n += first.count.max(1),
                None => lines.push((label, first.count.max(1))),
            }
        }
        for (label, count) in lines {
            text.push_str(&format!("- {count} \u{d7} {label}\n"));
        }
        Some(text)
    }
}

#[cfg(test)]
mod recipe_text_tests {
    use crate::{Browser, Event, Ingredient, ItemInfo, Recipe, RecipeKind, Stack};

    #[test]
    fn a_recipe_copies_as_markdown_with_counted_ingredients() {
        let item = |id: &str, name: &str| ItemInfo {
            id: id.into(),
            name: name.into(),
            tags: vec![],
        };
        let items = vec![
            item("minecraft:planks", "Planks"),
            item("minecraft:stick", "Stick"),
        ];
        let one = |item| Ingredient {
            item,
            count: 1,
            components: String::new(),
        };
        let recipe = Recipe {
            id: "minecraft:stick".into(),
            kind: RecipeKind::Shaped,
            width: 1,
            height: 2,
            inputs: vec![vec![one(0)], vec![one(0)]],
            outputs: vec![Ingredient { count: 4, ..one(1) }],
            ticks: 0,
            experience: 0.0,
            book_id: -1,
        };
        let (mut browser, _) = Browser::new(items, vec![recipe]);
        let stick = Stack {
            id: "minecraft:stick".into(),
            count: 1,
            components: String::new(),
        };
        browser.handle(
            Event::Lookup {
                stack: stick,
                uses: false,
            },
            false,
            &[],
        );
        assert_eq!(
            browser.recipe_text(0).unwrap(),
            "**Stick \u{d7} 4** (crafting table, `minecraft:stick`)\n- 2 \u{d7} Planks\n"
        );
    }
}

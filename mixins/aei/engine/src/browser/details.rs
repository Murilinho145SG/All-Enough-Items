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

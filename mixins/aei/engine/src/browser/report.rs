//! The diagnostic report for modpack makers: items no recipe makes or uses, recipes
//! whose ingredients make different results, and recipe counts by category.

use super::Browser;
use crate::{RecipeKind, Station};
use std::collections::HashMap;

impl Browser {
    /// The report as Markdown.
    pub fn report(&self) -> String {
        let mut text = String::from("# AEI diagnostic report\n\n");
        text.push_str(&format!(
            "{} items, {} recipes.\n\n## Recipes by category\n\n",
            self.items.len(),
            self.recipes.len()
        ));
        for station in Station::ALL {
            let count = self
                .recipes
                .iter()
                .filter(|r| Station::of(r.kind) == station)
                .count();
            if count > 0 {
                text.push_str(&format!("- {}: {count}\n", station.icon()));
            }
        }

        let lonely: Vec<&str> = self
            .items
            .iter()
            .enumerate()
            .filter(|(i, item)| {
                item.id != "minecraft:air"
                    && self.recipe_index.produced_by(*i).is_empty()
                    && self.recipe_index.used_by(*i) == 0
            })
            .map(|(_, item)| item.id.as_str())
            .collect();
        text.push_str(&format!(
            "\n## Items no recipe makes or uses ({})\n\n",
            lonely.len()
        ));
        for id in &lonely {
            text.push_str(&format!("- `{id}`\n"));
        }

        // Crafting recipes with the same ingredients (first alternatives, ignoring
        // their order for shapeless ones) but different results.
        let mut seen: HashMap<String, (usize, &str)> = HashMap::new();
        let mut conflicts = Vec::new();
        for recipe in &self.recipes {
            let shaped = match recipe.kind {
                RecipeKind::Shaped => true,
                RecipeKind::Shapeless => false,
                _ => continue,
            };
            let mut slots: Vec<usize> = recipe
                .inputs
                .iter()
                .map(|slot| slot.first().map_or(usize::MAX, |i| i.item))
                .collect();
            if !shaped {
                slots.sort_unstable();
            }
            let key = format!("{shaped}:{}x{}:{slots:?}", recipe.width, recipe.height);
            let Some(output) = recipe.outputs.first() else {
                continue;
            };
            match seen.get(&key) {
                Some(&(other, id)) if other != output.item => {
                    conflicts.push((id.to_owned(), recipe.id.clone()));
                }
                Some(_) => {}
                None => {
                    seen.insert(key, (output.item, recipe.id.as_str()));
                }
            }
        }
        text.push_str(&format!(
            "\n## Recipes with the same ingredients and different results ({})\n\n",
            conflicts.len()
        ));
        for (a, b) in conflicts {
            text.push_str(&format!("- `{a}` and `{b}`\n"));
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use crate::{Browser, Ingredient, ItemInfo, Recipe, RecipeKind};

    #[test]
    fn the_report_lists_lonely_items_and_conflicting_recipes() {
        let items = ["a", "b", "c", "lonely"]
            .map(|id| ItemInfo {
                id: format!("test:{id}"),
                name: id.into(),
                tags: vec![],
            })
            .to_vec();
        let one = |item| Ingredient {
            item,
            count: 1,
            components: String::new(),
        };
        let recipe = |id: &str, inputs: Vec<usize>, output| Recipe {
            id: id.into(),
            kind: RecipeKind::Shapeless,
            width: 0,
            height: 0,
            inputs: inputs.into_iter().map(|i| vec![one(i)]).collect(),
            outputs: vec![one(output)],
            ticks: 0,
            experience: 0.0,
            book_id: -1,
        };
        let recipes = vec![
            recipe("test:b_from_a", vec![0, 1], 2),
            recipe("test:c_from_a", vec![1, 0], 1),
        ];
        let (browser, _) = Browser::new(items, recipes);
        let report = browser.report();
        assert!(report.contains("- `test:lonely`"));
        assert!(!report.contains("- `test:a`"));
        assert!(report.contains("- `test:b_from_a` and `test:c_from_a`"));
    }
}

//! The diagnostic report for modpack makers: items no recipe makes or uses, recipes
//! whose ingredients make different results, and recipe counts by category.

use super::Browser;
use crate::{RecipeKind, Station};
use std::collections::HashMap;

impl Browser {
    /// Reads the item tag files (`tag` -> its `values`, a `#` before nested tags) to
    /// find the tags no item the game has fills, for the report.
    pub fn load_tag_files(&mut self, files: &HashMap<String, Vec<String>>) {
        fn filled(
            tag: &str,
            files: &HashMap<String, Vec<String>>,
            known: &HashMap<String, usize>,
            visiting: &mut Vec<String>,
        ) -> bool {
            if visiting.iter().any(|t| t == tag) {
                return false;
            }
            visiting.push(tag.to_owned());
            let any = files.get(tag).is_some_and(|values| {
                values.iter().any(|value| match value.strip_prefix('#') {
                    Some(nested) => filled(nested, files, known, visiting),
                    None => known.contains_key(value),
                })
            });
            visiting.pop();
            any
        }
        let mut empty: Vec<String> = files
            .keys()
            .filter(|tag| !filled(tag, files, &self.by_id, &mut Vec::new()))
            .cloned()
            .collect();
        empty.sort();
        self.empty_tags = empty;
    }

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

        let unreachable = self.unreachable();
        text.push_str(&format!(
            "\n## Items recipes make but none can reach from raw materials (items no recipe makes, or blocks found in the world) ({})\n\n",
            unreachable.len()
        ));
        for id in &unreachable {
            text.push_str(&format!("- `{id}`\n"));
        }

        text.push_str(&format!(
            "\n## Item tags no item fills ({})\n\n",
            self.empty_tags.len()
        ));
        for tag in &self.empty_tags {
            text.push_str(&format!("- `#{tag}`\n"));
        }
        text
    }

    /// Items some recipe makes that no chain of recipes reaches from the raw materials
    /// (items no recipe makes): every recipe for them needs one of them, in a loop.
    /// Recipes with an empty input slot count as filled there, so empty tags don't
    /// hide whole chains (they have their own section).
    fn unreachable(&self) -> Vec<&str> {
        // A recipe giving back one of its inputs (a block dropping itself with silk
        // touch) doesn't make that item.
        let gives_back = |recipe: &crate::Recipe, item: usize| {
            recipe.inputs.iter().flatten().any(|i| i.item == item)
        };
        let mut reached: Vec<bool> = (0..self.items.len())
            .map(|i| {
                self.recipe_index
                    .produced_by(i)
                    .iter()
                    .all(|&r| gives_back(&self.recipes[r], i))
            })
            .collect();
        // A block that drops something is found in the world: a raw material too.
        for recipe in self.recipes.iter().filter(|r| r.kind == RecipeKind::Drop) {
            for input in recipe.inputs.iter().flatten() {
                if let Some(r) = reached.get_mut(input.item) {
                    *r = true;
                }
            }
        }
        loop {
            let mut changed = false;
            for recipe in &self.recipes {
                if recipe
                    .outputs
                    .iter()
                    .all(|o| reached.get(o.item) != Some(&false))
                {
                    continue;
                }
                let makeable = recipe.inputs.iter().all(|slot| {
                    slot.is_empty() || slot.iter().any(|i| reached.get(i.item) == Some(&true))
                });
                if makeable {
                    for output in recipe
                        .outputs
                        .iter()
                        .filter(|o| !gives_back(recipe, o.item))
                    {
                        if let Some(r) = reached.get_mut(output.item)
                            && !*r
                        {
                            *r = true;
                            changed = true;
                        }
                    }
                }
            }
            if !changed {
                break;
            }
        }
        self.items
            .iter()
            .zip(reached)
            .filter(|(_, reached)| !reached)
            .map(|(item, _)| item.id.as_str())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::{Browser, Ingredient, ItemInfo, Recipe, RecipeKind};

    #[test]
    fn the_report_lists_lonely_items_and_conflicting_recipes() {
        let items = ["a", "b", "c", "lonely", "loop1", "loop2", "raw2", "made"]
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
            recipe("test:loop1", vec![5], 4),
            recipe("test:loop2", vec![4], 5),
            Recipe {
                kind: RecipeKind::Drop,
                ..recipe("test:c_drops_b", vec![2], 1)
            },
            recipe("test:made", vec![6], 7),
            Recipe {
                outputs: vec![one(6), one(7)],
                ..recipe("test:raw2_drops_itself", vec![6], 6)
            },
        ];
        let (mut browser, _) = Browser::new(items, recipes);
        let files = [
            ("test:filled", vec!["test:a", "test:gone"]),
            ("test:empty", vec!["test:gone"]),
            ("test:nested", vec!["#test:filled"]),
            ("test:nested_empty", vec!["#test:empty", "#test:missing"]),
        ]
        .into_iter()
        .map(|(tag, values)| {
            (
                tag.to_owned(),
                values.into_iter().map(str::to_owned).collect(),
            )
        })
        .collect();
        browser.load_tag_files(&files);
        let report = browser.report();
        assert!(report.contains("- `test:lonely`"));
        assert!(!report.contains("- `test:a`"));
        assert!(report.contains("- `test:b_from_a` and `test:c_from_a`"));
        let unreachable = report
            .split("## Items recipes make but none")
            .nth(1)
            .and_then(|rest| rest.split("\n## ").next())
            .unwrap();
        assert!(unreachable.contains("- `test:loop1`"));
        assert!(unreachable.contains("- `test:loop2`"));
        assert!(!unreachable.contains("- `test:made`"));
        assert!(!unreachable.contains("- `test:a`"));
        // c is mined (it drops b), so b and c count as reachable.
        assert!(!unreachable.contains("- `test:b`"));
        assert!(!unreachable.contains("- `test:c`"));
        let empty = report.split("## Item tags no item fills").nth(1).unwrap();
        assert!(empty.contains("- `#test:empty`"));
        assert!(empty.contains("- `#test:nested_empty`"));
        assert!(!empty.contains("- `#test:filled`"));
        assert!(!empty.contains("- `#test:nested`\n"));
    }
}

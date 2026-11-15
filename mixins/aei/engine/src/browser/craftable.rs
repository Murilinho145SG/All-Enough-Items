//! `is:craftable`: the items one crafting recipe makes from what the player holds, in
//! the open grid (2x2 in the inventory, 3x3 at a crafting table).

use super::Browser;
use crate::{RecipeKind, Stack, Update};
use std::collections::HashMap;

impl Browser {
    pub(super) fn set_inventory(&mut self, stacks: Vec<Stack>, grid: u32) -> Vec<Update> {
        if stacks == self.inventory && grid == self.grid {
            return Vec::new();
        }
        self.inventory = stacks;
        self.grid = grid;
        let mut stock: HashMap<usize, u32> = HashMap::new();
        for held in &self.inventory {
            if let Some(&i) = self.by_id.get(&held.id) {
                *stock.entry(i).or_default() += held.count;
            }
        }
        let craftable: Vec<bool> = (0..self.items.len())
            .map(|item| self.craftable(item, &stock))
            .collect();
        for entry in &mut self.indexed {
            entry.craftable = entry.item.is_some_and(|i| craftable[i]);
        }
        let found = self.discover();
        for (entry, listed) in self.indexed.iter_mut().zip(&self.catalogue) {
            entry.fresh = self.fresh.contains(&listed.stack.id);
        }
        if self.search.reads_inventory() || (found && self.spoiler_free) {
            vec![self.item_list()]
        } else {
            Vec::new()
        }
    }

    /// Assisted crafting: the recipe-book id of the plan's first step the inventory can
    /// craft now in the open grid, when the player's book holds it.
    pub fn next_book_step(&self, plan: &super::Plan, known: &[i32]) -> Option<i32> {
        let mut stock: HashMap<usize, u32> = HashMap::new();
        for held in &self.inventory {
            if let Some(&i) = self.by_id.get(&held.id) {
                *stock.entry(i).or_default() += held.count;
            }
        }
        plan.steps.iter().find_map(|step| {
            let book = self.recipes.get(step.recipe)?.book_id;
            (book >= 0 && known.contains(&book) && self.recipe_craftable(step.recipe, &stock))
                .then_some(book)
        })
    }

    /// Whether one crafting recipe for `item` fits the grid and the stock covers it.
    fn craftable(&self, item: usize, stock: &HashMap<usize, u32>) -> bool {
        self.recipe_index
            .produced_by(item)
            .iter()
            .any(|&id| self.recipe_craftable(id, stock))
    }

    /// Whether crafting recipe `id` fits the open grid and the stock covers it.
    fn recipe_craftable(&self, id: usize, stock: &HashMap<usize, u32>) -> bool {
        let grid = self.grid.max(1);
        {
            let recipe = &self.recipes[id];
            let fits = match recipe.kind {
                RecipeKind::Shaped => recipe.width <= grid && recipe.height <= grid,
                RecipeKind::Shapeless => {
                    recipe.inputs.iter().filter(|s| !s.is_empty()).count() as u32 <= grid * grid
                }
                _ => false,
            };
            if !fits {
                return false;
            }
            // Spend the stock slot by slot, taking any alternative still held.
            let mut left = stock.clone();
            recipe.inputs.iter().filter(|s| !s.is_empty()).all(|slot| {
                slot.iter().any(|choice| {
                    let have = left.entry(choice.item).or_default();
                    let need = choice.count.max(1);
                    if *have >= need {
                        *have -= need;
                        true
                    } else {
                        false
                    }
                })
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Browser, Event, Ingredient, ItemInfo, Recipe, RecipeKind, Stack, Update};

    fn stack(id: &str, count: u32) -> Stack {
        Stack {
            id: format!("minecraft:{id}"),
            count,
            components: String::new(),
        }
    }

    #[test]
    fn craftable_follows_the_inventory_and_the_grid() {
        let items = ["planks", "stick", "table"]
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
        let shaped = |w, h, n, out| Recipe {
            id: String::new(),
            kind: RecipeKind::Shaped,
            width: w,
            height: h,
            inputs: vec![one(0); n],
            outputs: one(out),
            ticks: 0,
            experience: 0.0,
            book_id: -1,
        };
        // Sticks: 1x2 planks; a table: 3x3 of planks (to test the grid).
        let mut table = shaped(3, 3, 9, 2);
        table.book_id = 5;
        let (mut browser, _) = Browser::new(items, vec![shaped(1, 2, 2, 1), table]);
        browser.handle(Event::Search("is:craftable".into()), false, &[]);
        let shown = |updates: Vec<Update>| match updates.first() {
            Some(Update::ItemList(list)) => list.iter().map(|s| s.id.clone()).collect::<Vec<_>>(),
            _ => panic!("expected item list"),
        };
        let inventory = |n, grid| Event::SetInventory {
            stacks: vec![stack("planks", n)],
            grid,
        };
        assert_eq!(
            shown(browser.handle(inventory(9, 2), false, &[])),
            ["minecraft:stick"]
        );
        assert_eq!(
            shown(browser.handle(inventory(9, 3), false, &[])),
            ["minecraft:stick", "minecraft:table"]
        );
        assert!(shown(browser.handle(inventory(1, 3), false, &[])).is_empty());
        // The same inventory again changes nothing.
        assert!(browser.handle(inventory(1, 3), false, &[]).is_empty());
        // Assisted crafting: the first step the inventory can make, if the book has it.
        browser.handle(inventory(9, 3), false, &[]);
        let plan = browser.plan(&stack("table", 1), 1, &[stack("planks", 9)]);
        assert_eq!(browser.next_book_step(&plan, &[]), None);
        assert_eq!(browser.next_book_step(&plan, &[5]), Some(5));
        browser.handle(inventory(9, 2), false, &[]);
        assert_eq!(browser.next_book_step(&plan, &[5]), None);
    }
}

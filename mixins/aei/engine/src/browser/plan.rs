//! The material calculator: how many raw materials a number of an item takes, through
//! which crafting steps, after what the inventory already holds.
//!
//! Each item is made with its first usable recipe (crafting first, then cooking, then
//! stonecutting); a recipe that would need an item already being made further up is
//! skipped, which breaks cycles such as ingot ↔ block. Items without a usable recipe are
//! raw materials. Of an ingredient's alternatives, one the inventory holds is preferred.

use super::Browser;
use crate::{RecipeKind, Stack};
use std::collections::HashMap;

/// One raw material: how many the plan takes in all, and how many of them the
/// inventory covers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanMaterial {
    pub stack: Stack,
    pub need: u32,
    pub have: u32,
}

/// One crafting step: `crafts` runs of a recipe making `stack.count` items in all, at
/// the station `station` (an item id).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanStep {
    pub stack: Stack,
    pub crafts: u32,
    pub station: String,
    /// The recipe the plan makes it with (an index into the browser's recipes).
    pub recipe: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    pub target: Stack,
    pub count: u32,
    /// Raw materials, in the order the plan first needs them.
    pub materials: Vec<PlanMaterial>,
    /// Steps in the order to do them: ingredients before what they make.
    pub steps: Vec<PlanStep>,
    /// Items left over because recipes make more than needed.
    pub leftovers: Vec<Stack>,
}

/// How deep the calculator follows ingredients.
const DEPTH: usize = 24;

fn rank(kind: RecipeKind) -> Option<u8> {
    match kind {
        RecipeKind::Shaped | RecipeKind::Shapeless => Some(0),
        RecipeKind::Smelting
        | RecipeKind::Blasting
        | RecipeKind::Smoking
        | RecipeKind::Campfire => Some(1),
        RecipeKind::Stonecutting => Some(2),
        _ => None,
    }
}

struct Planner<'a> {
    browser: &'a Browser,
    /// What the inventory still has, by registry index (spent as the plan uses it).
    stock: HashMap<usize, u32>,
    /// Leftovers from earlier crafts, by registry index.
    spare: HashMap<usize, u32>,
    materials: Vec<(usize, u32, u32)>,
    steps: Vec<(usize, u32, u32, RecipeKind, usize)>,
    /// Items being made on the current path, to break cycles.
    path: Vec<usize>,
}

impl Planner<'_> {
    /// The recipe to make `item` with, and how many it makes per craft.
    fn recipe(&self, item: usize) -> Option<(usize, u32)> {
        let browser = self.browser;
        let ids = browser.recipe_index.produced_by(item);
        ids.iter()
            .copied()
            .filter_map(|id| {
                let recipe = &browser.recipes[id];
                let rank = rank(recipe.kind)?;
                if !browser.shows_category(recipe.kind) {
                    return None;
                }
                let output = recipe.outputs.iter().find(|o| o.item == item)?;
                // A recipe needing something made further up would loop.
                let loops = recipe.inputs.iter().any(|slot| {
                    !slot.is_empty()
                        && slot
                            .iter()
                            .all(|i| i.item == item || self.path.contains(&i.item))
                });
                // Unpacking a storage form (a block into ingots, an ingot into
                // nuggets) is no source: what it needs is itself made from the item.
                let reversible = recipe.inputs.iter().flatten().any(|i| {
                    browser
                        .recipe_index
                        .produced_by(i.item)
                        .iter()
                        .any(|&back| {
                            browser.recipes[back]
                                .inputs
                                .iter()
                                .flatten()
                                .any(|b| b.item == item)
                        })
                });
                // Packing (nuggets into an ingot, ingots into a block) is the way to
                // make a storage form, but any other recipe is a better source.
                let rank = match (reversible, output.count > 1) {
                    (true, true) => return None,
                    (true, false) => rank + 5,
                    (false, _) => rank,
                };
                // Of equal recipes, the one whose ingredients serve the most recipes
                // (raw iron over iron ore) is the commoner material.
                let common: usize = recipe
                    .inputs
                    .iter()
                    .filter_map(|slot| slot.first())
                    .map(|i| browser.recipe_index.used_by(i.item))
                    .sum();
                (!loops).then_some((rank, std::cmp::Reverse(common), id, output.count.max(1)))
            })
            .min_by_key(|(rank, common, id, _)| (*rank, *common, *id))
            .map(|(_, _, id, count)| (id, count))
    }

    fn take(map: &mut HashMap<usize, u32>, item: usize, want: u32) -> u32 {
        let have = map.entry(item).or_default();
        let took = (*have).min(want);
        *have -= took;
        took
    }

    fn need(&mut self, item: usize, count: u32) {
        let mut count = count;
        count -= Self::take(&mut self.spare, item, count);
        let from_stock = Self::take(&mut self.stock, item, count);
        count -= from_stock;
        let recipe = (self.path.len() < DEPTH)
            .then(|| self.recipe(item))
            .flatten();
        let Some((id, per_craft)) = recipe else {
            self.add_material(item, count + from_stock, from_stock);
            return;
        };
        if count == 0 {
            return;
        }
        let crafts = count.div_ceil(per_craft);
        let made = crafts * per_craft;
        *self.spare.entry(item).or_default() += made - count;
        self.path.push(item);
        let recipe = &self.browser.recipes[id];
        for slot in &recipe.inputs {
            if slot.is_empty() {
                continue;
            }
            // An alternative the inventory holds, else the first.
            let choice = slot
                .iter()
                .find(|i| self.stock.get(&i.item).copied().unwrap_or(0) > 0)
                .unwrap_or(&slot[0]);
            let (input, amount) = (choice.item, choice.count.max(1));
            self.need(input, amount * crafts);
        }
        self.path.pop();
        self.steps.push((item, made, crafts, recipe.kind, id));
    }

    fn add_material(&mut self, item: usize, need: u32, have: u32) {
        if need == 0 {
            return;
        }
        match self.materials.iter_mut().find(|(i, ..)| *i == item) {
            Some((_, n, h)) => {
                *n += need;
                *h += have;
            }
            None => self.materials.push((item, need, have)),
        }
    }
}

impl Browser {
    /// The plan for `count` of `target`, given the stacks the player holds.
    pub fn plan(&self, target: &Stack, count: u32, inventory: &[Stack]) -> Plan {
        let mut plan = self.plan_all(&[(target.clone(), count)], inventory);
        plan.target = Stack {
            count: 1,
            ..target.clone()
        };
        plan.count = count.max(1);
        plan
    }

    /// One plan for several targets at once (a shopping list): materials, steps and
    /// leftovers summed, each target made rather than taken from the inventory.
    pub fn plan_all(&self, targets: &[(Stack, u32)], inventory: &[Stack]) -> Plan {
        let stack = |item: usize, count: u32| Stack {
            id: self.items[item].id.clone(),
            count,
            components: String::new(),
        };
        let mut stock = HashMap::new();
        for held in inventory {
            if let Some(&i) = self.by_id.get(&held.id) {
                *stock.entry(i).or_default() += held.count;
            }
        }
        let mut planner = Planner {
            browser: self,
            stock,
            spare: HashMap::new(),
            materials: Vec::new(),
            steps: Vec::new(),
            path: Vec::new(),
        };
        for (target, count) in targets {
            let Some(&item) = self.by_id.get(&target.id) else {
                continue;
            };
            // The target itself is made, not taken from the inventory.
            let held = planner.stock.remove(&item);
            planner.need(item, (*count).max(1));
            if let Some(held) = held {
                planner.stock.insert(item, held);
            }
        }
        // Steps that make the target or anything else: merge repeats of one item.
        let mut steps: Vec<PlanStep> = Vec::new();
        for (i, made, crafts, kind, recipe) in planner.steps {
            let station = crate::Station::of(kind).icon().to_owned();
            match steps.iter_mut().find(|s| s.stack.id == self.items[i].id) {
                Some(step) => {
                    step.stack.count += made;
                    step.crafts += crafts;
                }
                None => steps.push(PlanStep {
                    stack: stack(i, made),
                    crafts,
                    station,
                    recipe,
                }),
            }
        }
        let mut leftovers: Vec<Stack> = planner
            .spare
            .into_iter()
            .filter(|(_, n)| *n > 0)
            .map(|(i, n)| stack(i, n))
            .collect();
        leftovers.sort_by(|a, b| a.id.cmp(&b.id));
        Plan {
            target: Stack::default(),
            count: targets.len() as u32,
            materials: planner
                .materials
                .into_iter()
                .map(|(i, need, have)| PlanMaterial {
                    stack: stack(i, need),
                    need,
                    have,
                })
                .collect(),
            steps,
            leftovers,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Browser, Ingredient, ItemInfo, Recipe, RecipeKind, Stack};

    fn item(id: &str) -> ItemInfo {
        ItemInfo {
            id: format!("minecraft:{id}"),
            name: id.into(),
            tags: vec![],
        }
    }

    fn recipe(kind: RecipeKind, inputs: &[(usize, u32)], output: (usize, u32)) -> Recipe {
        let one = |(item, count): (usize, u32)| Ingredient {
            item,
            count,
            components: String::new(),
        };
        Recipe {
            id: String::new(),
            kind,
            width: 0,
            height: 0,
            inputs: inputs.iter().map(|&i| vec![one(i)]).collect(),
            outputs: vec![one(output)],
            ticks: 0,
            experience: 0.0,
            book_id: -1,
        }
    }

    /// log(0) → planks(1) x4; planks x2 → stick(2) x4; 3 planks + 2 sticks → pickaxe(3);
    /// raw iron(4) → ingot(5) (smelting); 9 ingots ↔ block(6).
    fn browser() -> Browser {
        let items = [
            "log",
            "planks",
            "stick",
            "pickaxe",
            "raw_iron",
            "ingot",
            "block",
            "raw_block",
        ]
        .map(item)
        .to_vec();
        use RecipeKind::*;
        let recipes = vec![
            recipe(Shapeless, &[(0, 1)], (1, 4)),
            recipe(Shaped, &[(1, 1), (1, 1)], (2, 4)),
            recipe(Shaped, &[(1, 1), (1, 1), (1, 1), (2, 1), (2, 1)], (3, 1)),
            recipe(Shapeless, &[(6, 1)], (5, 9)),
            recipe(Shaped, &[(5, 1); 9], (6, 1)),
            recipe(Smelting, &[(4, 1)], (5, 1)),
            recipe(Shapeless, &[(7, 1)], (4, 9)),
            recipe(Shaped, &[(4, 1); 9], (7, 1)),
            recipe(Shaped, &[(8, 1); 9], (5, 1)),
            recipe(Shapeless, &[(5, 1)], (8, 9)),
            recipe(Smelting, &[(3, 1)], (8, 1)),
        ];
        Browser::new(items, recipes).0
    }

    fn stack(id: &str, count: u32) -> Stack {
        Stack {
            id: format!("minecraft:{id}"),
            count,
            components: String::new(),
        }
    }

    #[test]
    fn a_pickaxe_takes_logs_through_planks_and_sticks() {
        let plan = browser().plan(&stack("pickaxe", 1), 1, &[]);
        // 3 planks + 2 sticks (one craft of sticks = 2 planks) = 5 planks = 2 logs.
        assert_eq!(plan.materials.len(), 1);
        assert_eq!(plan.materials[0].stack.id, "minecraft:log");
        assert_eq!(plan.materials[0].need, 2);
        let ids: Vec<_> = plan.steps.iter().map(|s| s.stack.id.as_str()).collect();
        assert_eq!(
            ids,
            ["minecraft:planks", "minecraft:stick", "minecraft:pickaxe"]
        );
        assert_eq!(plan.steps[0].crafts, 2);
        // 8 planks made, 5 used; 4 sticks made, 2 used.
        assert_eq!(plan.leftovers, [stack("planks", 3), stack("stick", 2)]);
    }

    #[test]
    fn cycles_fall_back_to_the_next_recipe_and_counts_scale() {
        let plan = browser().plan(&stack("block", 1), 2, &[]);
        assert_eq!(
            plan.materials,
            [crate::PlanMaterial {
                stack: stack("raw_iron", 18),
                need: 18,
                have: 0,
            }]
        );
        assert_eq!(plan.steps[0].station, "minecraft:furnace");
    }

    #[test]
    fn storage_blocks_are_never_unpacked_into_raw_materials() {
        // Raw iron's only recipe would unpack a block: raw iron is the raw material.
        let plan = browser().plan(&stack("raw_iron", 1), 4, &[]);
        assert_eq!(plan.materials[0].stack, stack("raw_iron", 4));
        assert!(plan.steps.is_empty());
    }

    #[test]
    fn ingots_come_from_smelting_rather_than_unpacking_blocks() {
        let plan = browser().plan(&stack("ingot", 1), 3, &[]);
        assert_eq!(plan.materials[0].stack, stack("raw_iron", 3));
        assert_eq!(plan.steps.len(), 1);
    }

    #[test]
    fn a_shopping_list_sums_its_targets() {
        let list = [(stack("pickaxe", 1), 2), (stack("stick", 1), 8)];
        let plan = browser().plan_all(&list, &[]);
        // Two pickaxes: 6 planks + 4 sticks; with 8 more sticks, 12 sticks = 6 planks.
        // 12 planks = 3 logs.
        assert_eq!(plan.materials[0].stack, stack("log", 3));
        assert_eq!(plan.count, 2);
        assert!(plan.steps.iter().any(|s| s.stack.id == "minecraft:pickaxe"));
    }

    #[test]
    fn the_inventory_covers_what_it_holds_first() {
        let plan = browser().plan(
            &stack("pickaxe", 1),
            1,
            &[stack("stick", 2), stack("log", 5)],
        );
        // Sticks are held: only 3 planks, 1 log, which the inventory has.
        assert_eq!(plan.materials[0].need, 1);
        assert_eq!(plan.materials[0].have, 1);
        assert!(plan.steps.iter().all(|s| s.stack.id != "minecraft:stick"));
    }
}

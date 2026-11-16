use super::*;
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct RecipeIndex {
    produces: HashMap<usize, Vec<usize>>,
    uses: HashMap<usize, Vec<usize>>,
}

impl RecipeIndex {
    /// The recipes that make registry item `item`.
    pub(super) fn produced_by(&self, item: usize) -> &[usize] {
        self.produces.get(&item).map_or(&[], Vec::as_slice)
    }

    /// How many recipes use registry item `item`.
    pub(super) fn used_by(&self, item: usize) -> usize {
        self.uses.get(&item).map_or(0, Vec::len)
    }

    /// Whether any recipe makes registry item `item`.
    pub(super) fn makes(&self, item: usize) -> bool {
        self.produces.contains_key(&item)
    }

    pub(super) fn new(recipes: &[Recipe]) -> Self {
        let mut index = Self::default();
        for (id, recipe) in recipes.iter().enumerate() {
            for (map, ingredients) in [
                (
                    &mut index.produces,
                    recipe.outputs.iter().collect::<Vec<_>>(),
                ),
                (&mut index.uses, recipe.inputs.iter().flatten().collect()),
            ] {
                for ingredient in ingredients {
                    let ids = map.entry(ingredient.item).or_default();
                    if ids.last() != Some(&id) {
                        ids.push(id);
                    }
                }
            }
        }
        index
    }
}

/// A cooking recipe's note: the experience line, a [`crate::NOTE_SEPARATOR`], then the cook
/// time line (`0.1 XP`, `10s`). Either line is empty when its value is zero.
pub(crate) fn cooking_note(ticks: u32, experience: f32) -> String {
    let experience = match experience {
        xp if xp <= 0.0 || !xp.is_finite() => String::new(),
        xp if xp.fract() == 0.0 => format!("{xp:.1} XP"),
        xp => format!("{xp} XP"),
    };
    let time = match ticks / 20 {
        0 => String::new(),
        seconds => format!("{seconds}s"),
    };
    format!("{experience}{}{time}", crate::NOTE_SEPARATOR)
}

/// A fuel's note, as JEI words it: how many items it smelts, then its burn time.
pub(crate) fn fuel_note(ticks: u32) -> String {
    let smelts = ticks as f32 / 200.0;
    let smelts = if smelts.fract() == 0.0 {
        format!("{smelts:.0}")
    } else {
        format!("{smelts:.1}")
    };
    let noun = if smelts == "1" { "item" } else { "items" };
    format!(
        "Smelts {smelts} {noun}{}Burn time: {}s",
        crate::NOTE_SEPARATOR,
        ticks / 20
    )
}

/// An anvil combination's note, as the anvil words its level cost.
pub(crate) fn anvil_note(cost: u32) -> String {
    if cost >= 40 {
        "Too Expensive!".into()
    } else {
        format!("Enchantment Cost: {cost}")
    }
}

/// A compostable item's note: its chance to add a layer.
pub(crate) fn compost_note(chance: f32) -> String {
    format!(
        "{}Chance: {:.0}%",
        crate::NOTE_SEPARATOR,
        (chance.clamp(0.0, 1.0) * 100.0).round()
    )
}

impl Browser {
    /// Card `card` of the shown page, to pin beside the screen: its card, the station
    /// whose category draws it, and its recipe id.
    pub fn shown_card(&self, card: usize) -> Option<(RecipeCard, &'static str, String)> {
        let &id = self.shown_recipe_ids.get(card)?;
        let recipe = &self.recipes[id];
        Some((
            self.make_card(recipe, &[]),
            Station::of(recipe.kind).icon(),
            recipe.id.clone(),
        ))
    }

    /// The preview of `stack`'s first recipe (crafting first): its card and the
    /// station whose category draws it.
    pub fn preview(&self, stack: &Stack) -> Option<(RecipeCard, &'static str)> {
        let &item = self.by_id.get(&stack.id)?;
        let id = self
            .recipe_index
            .produced_by(item)
            .iter()
            .copied()
            .filter(|&id| self.shows_category(self.recipes[id].kind))
            .min_by_key(|&id| {
                let crafting = matches!(
                    self.recipes[id].kind,
                    RecipeKind::Shaped | RecipeKind::Shapeless
                );
                (!crafting, id)
            })?;
        let recipe = &self.recipes[id];
        Some((self.make_card(recipe, &[]), Station::of(recipe.kind).icon()))
    }

    /// The id of the recipe on card `card` of the shown page.
    pub fn shown_recipe_id(&self, card: usize) -> Option<&str> {
        let recipe = &self.recipes[*self.shown_recipe_ids.get(card)?];
        (!recipe.id.is_empty()).then_some(recipe.id.as_str())
    }

    /// The tag an input slot of card `card` stands for: the tag whose items are exactly
    /// the slot's alternatives (JEI's "Accepts any"). None for a single item, or for
    /// alternatives no single tag lists; the shortest name wins a tie.
    pub fn shown_slot_tag(&self, card: usize, slot: usize) -> Option<&str> {
        let recipe = &self.recipes[*self.shown_recipe_ids.get(card)?];
        let mut items: Vec<usize> = recipe.inputs.get(slot)?.iter().map(|i| i.item).collect();
        items.sort_unstable();
        items.dedup();
        if items.len() < 2 {
            return None;
        }
        let first = self.items.get(items[0])?;
        first
            .tags
            .iter()
            .filter(|tag| self.tag_members.get(*tag) == Some(&items))
            .min_by_key(|tag| (tag.len(), tag.as_str()))
            .map(String::as_str)
    }

    fn item_id(&self, ingredient: &Ingredient) -> &str {
        self.items
            .get(ingredient.item)
            .map_or("minecraft:air", |item| item.id.as_str())
    }

    pub(super) fn as_stack(&self, ingredient: &Ingredient) -> Stack {
        Stack {
            id: self.item_id(ingredient).to_owned(),
            count: ingredient.count,
            components: ingredient.components.clone(),
        }
    }

    fn item_matches_stack(&self, ingredient: &Ingredient, target: &Stack) -> bool {
        self.item_id(ingredient) == target.id
            && (target.components.is_empty() || ingredient.components == target.components)
    }

    pub(super) fn page_recipe_ids(&self, lookup: &Lookup) -> Vec<usize> {
        // "All recipes" (the category title's click): every recipe, one tab per station.
        if lookup.category.is_some() {
            return (0..self.recipes.len())
                .filter(|&id| self.shows_category(self.recipes[id].kind))
                .collect();
        }
        let Some(&item) = self.by_id.get(&lookup.item.id) else {
            return Vec::new();
        };
        let index = if lookup.uses {
            &self.recipe_index.uses
        } else {
            &self.recipe_index.produces
        };
        index
            .get(&item)
            .into_iter()
            .flatten()
            .copied()
            .filter(|&id| self.shows_category(self.recipes[id].kind))
            .filter(|&id| self.recipe_unlocked(id))
            .filter(|&id| {
                let recipe = &self.recipes[id];
                if lookup.uses {
                    recipe
                        .inputs
                        .iter()
                        .flatten()
                        .any(|ingredient| self.item_matches_stack(ingredient, &lookup.item))
                } else {
                    recipe
                        .outputs
                        .iter()
                        .any(|ingredient| self.item_matches_stack(ingredient, &lookup.item))
                }
            })
            .collect::<Vec<_>>()
            .into_iter()
            // An opened recipe bookmark's recipe comes first.
            .fold(Vec::new(), |mut ids, id| {
                if Some(id) == lookup.focus {
                    ids.insert(0, id);
                } else {
                    ids.push(id);
                }
                ids
            })
    }

    pub(super) fn make_card(&self, recipe: &Recipe, known: &[i32]) -> RecipeCard {
        let mut inputs: Vec<Vec<Stack>> = recipe
            .inputs
            .iter()
            .map(|slot| {
                slot.iter()
                    .take(STACK_LIMIT)
                    .map(|stack| self.as_stack(stack))
                    .collect()
            })
            .collect();
        let (layout, width, height, note) = match recipe.kind {
            RecipeKind::Shaped => (
                Layout::Grid,
                recipe.width.max(1),
                recipe.height.max(1),
                String::new(),
            ),
            RecipeKind::Shapeless => {
                let count = inputs.len() as u32;
                let width = match count {
                    0..=1 => 1,
                    2..=4 => 2,
                    _ => 3,
                };
                let height = count.div_ceil(width).max(1);
                inputs.resize((width * height) as usize, Vec::new());
                (Layout::Grid, width, height, crate::SHAPELESS_NOTE.into())
            }
            RecipeKind::Smelting
            | RecipeKind::Blasting
            | RecipeKind::Smoking
            | RecipeKind::Campfire => (
                Layout::Cooking,
                1,
                1,
                cooking_note(recipe.ticks, recipe.experience),
            ),
            RecipeKind::Stonecutting => (Layout::Single, 1, 1, String::new()),
            RecipeKind::Fuel => (Layout::Single, 1, 1, fuel_note(recipe.ticks)),
            RecipeKind::Compost => (Layout::Single, 1, 1, compost_note(recipe.experience)),
            RecipeKind::Smithing => (Layout::Smithing, 3, 1, String::new()),
            RecipeKind::Anvil => (Layout::Smithing, 2, 1, anvil_note(recipe.ticks)),
            RecipeKind::Trade => (Layout::Smithing, 2, 1, String::new()),
            RecipeKind::Drop => (Layout::Single, 1, 1, String::new()),
            RecipeKind::Brewing => (Layout::Brewing, 2, 1, String::new()),
        };
        let place_enabled = (recipe.book_id >= 0).then(|| known.contains(&recipe.book_id));
        RecipeCard {
            layout,
            width,
            height,
            inputs,
            outputs: recipe
                .outputs
                .iter()
                .take(STACK_LIMIT)
                .map(|stack| self.as_stack(stack))
                .collect(),
            note,
            place_enabled,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn cooking_notes_read_like_jei() {
        assert_eq!(super::cooking_note(200, 0.1), "0.1 XP\n10s");
        assert_eq!(super::cooking_note(100, 1.0), "1.0 XP\n5s");
        assert_eq!(super::cooking_note(600, 0.0), "\n30s");
    }
}

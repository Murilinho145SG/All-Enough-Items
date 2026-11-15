use super::*;
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct RecipeIndex {
    produces: HashMap<usize, Vec<usize>>,
    uses: HashMap<usize, Vec<usize>>,
}

impl RecipeIndex {
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

impl Browser {
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
            RecipeKind::Smithing => (Layout::Smithing, 3, 1, String::new()),
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

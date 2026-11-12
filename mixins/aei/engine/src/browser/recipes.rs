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

impl Browser {
    fn item_id(&self, ingredient: &Ingredient) -> &str {
        self.items
            .get(ingredient.item)
            .map_or("minecraft:air", |item| item.id.as_str())
    }

    fn as_stack(&self, ingredient: &Ingredient) -> Stack {
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
        if let Some(station) = lookup.category {
            return self
                .recipes
                .iter()
                .enumerate()
                .filter_map(|(i, recipe)| (Station::of(recipe.kind) == station).then_some(i))
                .collect();
        }
        let Some(item) = self.items.iter().position(|item| item.id == lookup.item.id) else {
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
            .collect()
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
            | RecipeKind::Campfire => {
                let seconds = recipe.ticks as f32 / 20.0;
                let note = format!("{seconds} s, {:.2} XP", recipe.experience);
                (Layout::Cooking, 1, 1, note)
            }
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

pub const ID: &str = "aei";
pub const NAME: &str = "All Enough Items";
pub const VERSION: &str = "0.1.1";
pub const DESCRIPTION: &str = "Browse items, recipes and uses beside your inventory.";
pub const KEY_RECIPE: [&str; 3] = ["recipe", "Show recipes", "key.keyboard.r"];
pub const KEY_USES: [&str; 3] = ["uses", "Show uses", "key.keyboard.u"];

const ITEM_LIMIT: usize = 4096;
const RECIPE_LIMIT: usize = 256;
const STACK_LIMIT: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecipeKind {
    Shaped,
    Shapeless,
    Smelting,
    Blasting,
    Smoking,
    Campfire,
    Stonecutting,
    Smithing,
    Brewing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Station {
    Crafting,
    Smelting,
    Blasting,
    Smoking,
    Campfire,
    Stonecutting,
    Smithing,
    Brewing,
}

impl Station {
    const ALL: [Self; 8] = [
        Self::Crafting,
        Self::Smelting,
        Self::Blasting,
        Self::Smoking,
        Self::Campfire,
        Self::Stonecutting,
        Self::Smithing,
        Self::Brewing,
    ];

    fn of(kind: RecipeKind) -> Self {
        match kind {
            RecipeKind::Shaped | RecipeKind::Shapeless => Self::Crafting,
            RecipeKind::Smelting => Self::Smelting,
            RecipeKind::Blasting => Self::Blasting,
            RecipeKind::Smoking => Self::Smoking,
            RecipeKind::Campfire => Self::Campfire,
            RecipeKind::Stonecutting => Self::Stonecutting,
            RecipeKind::Smithing => Self::Smithing,
            RecipeKind::Brewing => Self::Brewing,
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Crafting => "minecraft:crafting_table",
            Self::Smelting => "minecraft:furnace",
            Self::Blasting => "minecraft:blast_furnace",
            Self::Smoking => "minecraft:smoker",
            Self::Campfire => "minecraft:campfire",
            Self::Stonecutting => "minecraft:stonecutter",
            Self::Smithing => "minecraft:smithing_table",
            Self::Brewing => "minecraft:brewing_stand",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ItemInfo {
    pub id: String,
    pub name: String,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stack {
    pub id: String,
    pub count: u32,
    /// Item components in the game's SNBT-like item syntax, or an empty string.
    pub components: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ingredient {
    pub item: usize,
    pub count: u32,
    pub components: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Recipe {
    pub kind: RecipeKind,
    pub width: u32,
    pub height: u32,
    pub inputs: Vec<Vec<Ingredient>>,
    pub outputs: Vec<Ingredient>,
    pub ticks: u32,
    pub experience: f32,
    /// Recipe-book id, or -1 for entries without a book placement action.
    pub book_id: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Grid,
    Cooking,
    Single,
    Smithing,
    Brewing,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecipeCard {
    pub layout: Layout,
    pub width: u32,
    pub height: u32,
    pub inputs: Vec<Vec<Stack>>,
    pub outputs: Vec<Stack>,
    pub note: String,
    pub place_enabled: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecipePage {
    pub title: String,
    pub stations: Vec<Stack>,
    pub station: usize,
    pub cards: Vec<RecipeCard>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Search(String),
    ClickItem { index: usize, right: bool },
    Lookup { stack: Stack, uses: bool },
    ClickRecipeIngredient { stack: Stack, right: bool },
    SelectStation(usize),
    Place(usize),
    Back,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Update {
    ItemList(Vec<Stack>),
    RecipeList(RecipePage),
    Give { item: String, count: u32 },
    PlaceKnownRecipe(i32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Lookup {
    item: Stack,
    uses: bool,
    station: usize,
}

/// State for one open world. All visible lists are rebuilt from this state,
/// keeping item indexes and recipe action indexes aligned with what was drawn.
#[derive(Default)]
pub struct Browser {
    items: Vec<ItemInfo>,
    recipes: Vec<Recipe>,
    filtered_items: Vec<usize>,
    search: String,
    history: Vec<Lookup>,
    shown_recipe_ids: Vec<usize>,
}

impl Browser {
    pub fn new(items: Vec<ItemInfo>, recipes: Vec<Recipe>) -> (Self, Update) {
        let mut browser = Self {
            items: items.into_iter().take(ITEM_LIMIT).collect(),
            recipes: recipes.into_iter().take(ITEM_LIMIT * 16).collect(),
            ..Self::default()
        };
        let update = browser.item_list();
        (browser, update)
    }

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

    fn searchable(&self, item: &ItemInfo) -> bool {
        if item.id == "minecraft:air" {
            return false;
        }
        let id = item.id.to_ascii_lowercase();
        let name = item.name.to_ascii_lowercase();
        self.search.split_whitespace().all(|term| {
            let term = term.to_ascii_lowercase();
            if let Some(namespace) = term.strip_prefix('@') {
                id.split_once(':')
                    .is_some_and(|(actual, _)| actual.starts_with(namespace))
            } else if let Some(tag) = term.strip_prefix('#') {
                item.tags
                    .iter()
                    .any(|actual| actual.to_ascii_lowercase().contains(tag))
            } else {
                name.contains(&term) || id.contains(&term)
            }
        })
    }

    fn item_list(&mut self) -> Update {
        self.filtered_items = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| self.searchable(item).then_some(index))
            .take(ITEM_LIMIT)
            .collect();
        Update::ItemList(
            self.filtered_items
                .iter()
                .map(|&index| Stack {
                    id: self.items[index].id.clone(),
                    count: 1,
                    components: String::new(),
                })
                .collect(),
        )
    }

    fn page_recipe_ids(&self, lookup: &Lookup) -> Vec<usize> {
        self.recipes
            .iter()
            .enumerate()
            .filter_map(|(index, recipe)| {
                let candidates = if lookup.uses {
                    recipe.inputs.iter().flatten().collect::<Vec<_>>()
                } else {
                    recipe.outputs.iter().collect::<Vec<_>>()
                };
                candidates
                    .iter()
                    .any(|ingredient| self.item_matches_stack(ingredient, &lookup.item))
                    .then_some(index)
            })
            .collect()
    }

    fn make_card(&self, recipe: &Recipe, known: &[i32]) -> RecipeCard {
        let mut inputs: Vec<Vec<Stack>> = recipe
            .inputs
            .iter()
            .map(|slot| {
                slot.iter()
                    .take(STACK_LIMIT)
                    .map(|s| self.as_stack(s))
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
                let n = inputs.len() as u32;
                let width = match n {
                    0..=1 => 1,
                    2..=4 => 2,
                    _ => 3,
                };
                let height = n.div_ceil(width).max(1);
                inputs.resize((width * height) as usize, Vec::new());
                (Layout::Grid, width, height, "Shapeless".into())
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
                .map(|s| self.as_stack(s))
                .collect(),
            note,
            place_enabled,
        }
    }

    fn render_page(&mut self, known: &[i32]) -> Update {
        let Some(mut lookup) = self.history.last().cloned() else {
            self.shown_recipe_ids.clear();
            return Update::RecipeList(RecipePage {
                title: String::new(),
                stations: Vec::new(),
                station: 0,
                cards: Vec::new(),
            });
        };
        let all = self.page_recipe_ids(&lookup);
        let available: Vec<Station> = Station::ALL
            .into_iter()
            .filter(|station| {
                all.iter()
                    .any(|&i| Station::of(self.recipes[i].kind) == *station)
            })
            .collect();
        if lookup.station >= available.len() {
            lookup.station = 0;
        }
        let current = available.get(lookup.station).copied();
        let selected: Vec<usize> = all
            .into_iter()
            .filter(|&i| Some(Station::of(self.recipes[i].kind)) == current)
            .take(RECIPE_LIMIT)
            .collect();
        let title_name = self
            .items
            .iter()
            .find(|item| item.id == lookup.item.id)
            .map_or(lookup.item.id.as_str(), |item| item.name.as_str());
        let title = format!(
            "{}: {}",
            if lookup.uses { "Uses" } else { "Recipes" },
            title_name
        );
        let tabs = available
            .iter()
            .map(|station| Stack {
                id: station.icon().to_owned(),
                count: 1,
                components: String::new(),
            })
            .collect();
        let cards = selected
            .iter()
            .map(|&index| self.make_card(&self.recipes[index], known))
            .collect();
        self.shown_recipe_ids = selected;
        if let Some(last) = self.history.last_mut() {
            last.station = lookup.station;
        }
        Update::RecipeList(RecipePage {
            title,
            stations: tabs,
            station: lookup.station,
            cards,
        })
    }

    fn open_lookup(&mut self, target: Stack, uses: bool, known: &[i32]) -> Option<Update> {
        let candidate = Lookup {
            item: target,
            uses,
            station: 0,
        };
        if self.page_recipe_ids(&candidate).is_empty() {
            return None;
        }
        self.history.push(candidate);
        Some(self.render_page(known))
    }

    pub fn handle(&mut self, event: Event, give: bool, known: &[i32]) -> Vec<Update> {
        match event {
            Event::Search(query) => {
                self.search = query;
                vec![self.item_list()]
            }
            Event::ClickItem { index, right } => {
                let Some(&item_index) = self.filtered_items.get(index) else {
                    return Vec::new();
                };
                let id = self.items[item_index].id.clone();
                if give {
                    return vec![Update::Give {
                        item: id,
                        count: if right { 99 } else { 1 },
                    }];
                }
                self.open_lookup(
                    Stack {
                        id,
                        count: 1,
                        components: String::new(),
                    },
                    right,
                    known,
                )
                .into_iter()
                .collect()
            }
            Event::Lookup { stack, uses } | Event::ClickRecipeIngredient { stack, right: uses } => {
                self.open_lookup(stack, uses, known).into_iter().collect()
            }
            Event::SelectStation(station) => {
                if let Some(last) = self.history.last_mut() {
                    last.station = station;
                }
                vec![self.render_page(known)]
            }
            Event::Place(index) => {
                match self
                    .shown_recipe_ids
                    .get(index)
                    .map(|&i| self.recipes[i].book_id)
                {
                    Some(id) if id >= 0 && known.contains(&id) => {
                        vec![Update::PlaceKnownRecipe(id)]
                    }
                    _ => Vec::new(),
                }
            }
            Event::Back => {
                self.history.pop();
                vec![self.render_page(known)]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (Vec<ItemInfo>, Vec<Recipe>) {
        let items = vec![
            ItemInfo {
                id: "minecraft:air".into(),
                name: "Air".into(),
                tags: vec![],
            },
            ItemInfo {
                id: "minecraft:oak_log".into(),
                name: "Oak Log".into(),
                tags: vec!["minecraft:logs".into()],
            },
            ItemInfo {
                id: "minecraft:oak_planks".into(),
                name: "Oak Planks".into(),
                tags: vec!["minecraft:planks".into()],
            },
            ItemInfo {
                id: "minecraft:stick".into(),
                name: "Stick".into(),
                tags: vec![],
            },
        ];
        let ingredient = |item, count| Ingredient {
            item,
            count,
            components: String::new(),
        };
        let recipes = vec![
            Recipe {
                kind: RecipeKind::Shapeless,
                width: 0,
                height: 0,
                inputs: vec![vec![ingredient(1, 1)]],
                outputs: vec![ingredient(2, 4)],
                ticks: 0,
                experience: 0.0,
                book_id: 7,
            },
            Recipe {
                kind: RecipeKind::Shaped,
                width: 1,
                height: 2,
                inputs: vec![vec![ingredient(2, 1)], vec![ingredient(2, 1)]],
                outputs: vec![ingredient(3, 4)],
                ticks: 0,
                experience: 0.0,
                book_id: 8,
            },
        ];
        (items, recipes)
    }

    #[test]
    fn search_filters_names_ids_namespaces_tags_and_excludes_air() {
        let (items, recipes) = fixture();
        let (mut browser, initial) = Browser::new(items, recipes);
        let Update::ItemList(all) = initial else {
            panic!("expected item list")
        };
        assert_eq!(all.len(), 3);
        for (query, expected) in [("oak", 2), ("@mine stick", 1), ("#logs", 1), ("@other", 0)] {
            let result = browser.handle(Event::Search(query.into()), false, &[]);
            assert!(matches!(&result[0], Update::ItemList(items) if items.len() == expected));
        }
    }

    #[test]
    fn lookup_tabs_and_back_keep_a_navigation_stack() {
        let (items, recipes) = fixture();
        let (mut browser, _) = Browser::new(items, recipes);
        let output = browser.handle(
            Event::Lookup {
                stack: Stack {
                    id: "minecraft:oak_log".into(),
                    count: 1,
                    components: String::new(),
                },
                uses: true,
            },
            false,
            &[8],
        );
        let Update::RecipeList(page) = &output[0] else {
            panic!("expected recipe page")
        };
        assert_eq!(page.title, "Uses: Oak Log");
        assert_eq!(page.stations.len(), 1);
        assert_eq!(page.cards[0].place_enabled, Some(false));
        let output = browser.handle(Event::Place(0), false, &[7]);
        assert_eq!(output, [Update::PlaceKnownRecipe(7)]);
        let back = browser.handle(Event::Back, false, &[]);
        assert!(matches!(&back[0], Update::RecipeList(page) if page.cards.is_empty()));
    }

    #[test]
    fn unresolved_lookups_leave_the_current_view_unchanged() {
        let (items, recipes) = fixture();
        let (mut browser, _) = Browser::new(items, recipes);
        let output = browser.handle(
            Event::Lookup {
                stack: Stack {
                    id: "minecraft:diamond".into(),
                    count: 1,
                    components: String::new(),
                },
                uses: false,
            },
            false,
            &[],
        );
        assert!(output.is_empty());
        assert!(browser.history.is_empty());
    }
}

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
pub(crate) enum Station {
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
    pub(crate) const ALL: [Self; 8] = [
        Self::Crafting,
        Self::Smelting,
        Self::Blasting,
        Self::Smoking,
        Self::Campfire,
        Self::Stonecutting,
        Self::Smithing,
        Self::Brewing,
    ];

    pub(crate) fn of(kind: RecipeKind) -> Self {
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

    pub(crate) fn icon(self) -> &'static str {
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
    ShowCategory,
    Place(usize),
    Back,
    Forward,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Update {
    ItemList(Vec<Stack>),
    RecipeList(RecipePage),
    Give { item: String, count: u32 },
    PlaceKnownRecipe(i32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Lookup {
    pub category: Option<Station>,
    pub item: Stack,
    pub uses: bool,
    pub station: usize,
}

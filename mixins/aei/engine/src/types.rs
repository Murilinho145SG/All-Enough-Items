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

/// One stack of the item list: an item, or one of its variants (an enchanted book, a
/// potion) with its components, as the creative tabs list them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Entry {
    pub stack: Stack,
    pub name: String,
    /// The tooltip's lines after the name, for `#` searches.
    pub tooltip: Vec<String>,
    /// The titles of the creative tabs that list it, for `%` searches.
    pub tabs: Vec<String>,
}

/// How a prefixed search field takes part in searches (JEI's search modes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchMode {
    /// Plain words search it too, and its prefix searches it alone.
    Enabled,
    /// Only its prefix searches it.
    RequirePrefix,
    /// Its prefix is an ordinary character.
    Disabled,
}

impl SearchMode {
    pub const ALL: [Self; 3] = [Self::Enabled, Self::RequirePrefix, Self::Disabled];

    pub fn name(self) -> &'static str {
        match self {
            Self::Enabled => "enabled",
            Self::RequirePrefix => "prefix",
            Self::Disabled => "disabled",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.name() == text)
    }

    /// The mode a settings button moves to on a click.
    pub fn next(self) -> Self {
        match self {
            Self::Enabled => Self::RequirePrefix,
            Self::RequirePrefix => Self::Disabled,
            Self::Disabled => Self::Enabled,
        }
    }
}

/// The search mode of each prefixed field: `@` mod, `#` tooltip, `$` tag, `%` creative
/// tab and `&` identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchModes {
    pub mod_name: SearchMode,
    pub tooltip: SearchMode,
    pub tag: SearchMode,
    pub tab: SearchMode,
    pub id: SearchMode,
}

impl Default for SearchModes {
    /// JEI's defaults, with identifiers searched by plain words as AEI always has.
    fn default() -> Self {
        Self {
            mod_name: SearchMode::RequirePrefix,
            tooltip: SearchMode::Enabled,
            tag: SearchMode::RequirePrefix,
            tab: SearchMode::RequirePrefix,
            id: SearchMode::Enabled,
        }
    }
}

impl SearchModes {
    /// Each field's settings key, prefix and label.
    pub const FIELDS: [(&'static str, char, &'static str); 5] = [
        ("search_mod", '@', "Mod name"),
        ("search_tooltip", '#', "Tooltip"),
        ("search_tag", '$', "Tag"),
        ("search_tab", '%', "Creative tab"),
        ("search_id", '&', "Identifier"),
    ];

    pub fn get(&self, field: usize) -> SearchMode {
        [self.mod_name, self.tooltip, self.tag, self.tab, self.id][field.min(4)]
    }

    pub fn set(&mut self, field: usize, mode: SearchMode) {
        match field {
            0 => self.mod_name = mode,
            1 => self.tooltip = mode,
            2 => self.tag = mode,
            3 => self.tab = mode,
            _ => self.id = mode,
        }
    }
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
    SetSearchModes(SearchModes),
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

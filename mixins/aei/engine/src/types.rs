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
    /// A furnace fuel: its input burns for `ticks`.
    Fuel,
    /// A compostable item: `experience` holds its chance (0 to 1) to add a layer.
    Compost,
    /// An anvil combination (repair or enchanted book): `ticks` holds its level cost.
    Anvil,
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
    Fuel,
    Compost,
    Anvil,
}

impl Station {
    pub(crate) const ALL: [Self; 11] = [
        Self::Crafting,
        Self::Smelting,
        Self::Blasting,
        Self::Smoking,
        Self::Campfire,
        Self::Stonecutting,
        Self::Smithing,
        Self::Anvil,
        Self::Brewing,
        Self::Fuel,
        Self::Compost,
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
            RecipeKind::Fuel => Self::Fuel,
            RecipeKind::Compost => Self::Compost,
            RecipeKind::Anvil => Self::Anvil,
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
            Self::Fuel => "minecraft:coal",
            Self::Compost => "minecraft:composter",
            Self::Anvil => "minecraft:anvil",
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
    /// Other names plain words find it by (its English name in another language).
    pub aliases: Vec<String>,
    /// What the `is:`, `stack:`, `dur`, `nutrition` and `rarity:` filters read.
    pub props: EntryProps,
}

/// An entry's item properties for the search filters.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EntryProps {
    pub max_stack: u32,
    /// Its durability; 0 when it doesn't take damage.
    pub durability: u32,
    /// Its food points, when it can be eaten.
    pub nutrition: Option<u32>,
    /// `common`, `uncommon`, `rare` or `epic`.
    pub rarity: String,
    pub enchantable: bool,
    /// It places a block.
    pub block: bool,
}

/// The item list's order (JEI's ingredient sort stages, as one choice).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortOrder {
    /// The creative tabs' order (JEI's default for one mod).
    #[default]
    Creative,
    /// By mod, Minecraft first, then the creative order within each mod.
    Mod,
    /// Alphabetically by name.
    Name,
}

impl SortOrder {
    pub const ALL: [Self; 3] = [Self::Creative, Self::Mod, Self::Name];

    pub fn name(self) -> &'static str {
        match self {
            Self::Creative => "creative",
            Self::Mod => "mod",
            Self::Name => "name",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|order| order.name() == text)
    }

    pub fn next(self) -> Self {
        match self {
            Self::Creative => Self::Mod,
            Self::Mod => Self::Name,
            Self::Name => Self::Creative,
        }
    }
}

/// A bookmarked stack, or a bookmarked recipe shown by its output.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bookmark {
    pub stack: Stack,
    /// A recipe bookmark's recipe id.
    pub recipe: Option<String>,
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
    /// The recipe's id (`minecraft:oak_planks`), or "".
    pub id: String,
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
    /// Bookmarks or un-bookmarks a stack (JEI's A key).
    ToggleBookmark(Stack),
    /// Bookmarks or un-bookmarks the recipe of card `n` on the shown page.
    BookmarkRecipe(usize),
    ClickBookmark {
        index: usize,
        right: bool,
    },
    /// Moves bookmark `from` to position `to`.
    MoveBookmark {
        from: usize,
        to: usize,
    },
    /// Shows the shown category's recipes from `offset`, at most `len` of them (the
    /// native panel holds 256 at a time, so longer lists are shown a window at a time).
    ShowWindow {
        offset: usize,
        len: usize,
    },
    Search(String),
    /// What the player holds and the open crafting grid's width (2 or 3), for
    /// `is:craftable`.
    SetInventory {
        stacks: Vec<Stack>,
        grid: u32,
    },
    SetSearchModes(SearchModes),
    SetSort(SortOrder),
    /// Edit mode shows hidden entries (marked) and makes clicks hide or show them.
    SetEditMode(bool),
    /// Edit mode's click: hide or show list entry `index`, or every stack of its item
    /// when `wildcard` (JEI's Ctrl+click).
    ToggleHidden {
        index: usize,
        wildcard: bool,
    },
    ClickItem {
        index: usize,
        right: bool,
    },
    Lookup {
        stack: Stack,
        uses: bool,
    },
    ClickRecipeIngredient {
        stack: Stack,
        right: bool,
    },
    SelectStation(usize),
    ShowCategory,
    Place(usize),
    Back,
    Forward,
    /// Shows lookup `n` of the trail again (a breadcrumb click).
    JumpHistory(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Update {
    /// The bookmark list changed: show it and save [`crate::Browser::bookmarks_text`].
    Bookmarks(Vec<Bookmark>),
    ItemList(Vec<Stack>),
    /// The hidden entries changed: save [`crate::Browser::hidden_text`].
    Hidden,
    RecipeList(RecipePage),
    /// Cheat mode: give the player `count` of `stack` (at most one full stack).
    Give {
        stack: Stack,
        count: u32,
    },
    PlaceKnownRecipe(i32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Lookup {
    /// A recipe shown first (an opened recipe bookmark's).
    pub focus: Option<usize>,
    pub category: Option<Station>,
    pub item: Stack,
    pub uses: bool,
    pub station: usize,
}

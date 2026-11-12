pub const ID: &str = "aei";
pub const NAME: &str = "All Enough Items";
pub const VERSION: &str = aei_engine::VERSION;
pub const DESCRIPTION: &str = "Browse items, recipes and uses beside your inventory.";
pub const KEY_RECIPE: [&str; 3] = ["recipe", "Show recipes", "key.keyboard.r"];
pub const KEY_USES: [&str; 3] = ["uses", "Show uses", "key.keyboard.u"];
pub const KEY_BACK: [&str; 3] = ["back", "Previous recipe lookup", "key.keyboard.left"];
pub const KEY_FORWARD: [&str; 3] = ["forward", "Next recipe lookup", "key.keyboard.right"];

pub use aei_engine::{
    Browser, Event, Ingredient, ItemInfo, Layout, Recipe, RecipeCard, RecipeKind, Stack, Update,
};
pub use aei_engine::{SHAPELESS_NOTE, viewer};

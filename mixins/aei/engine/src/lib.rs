//! Horizon-independent AEI browser model.
//!
//! `browser` owns per-screen state and event handling; `types` contains the
//! model shared with the Horizon integration layer.

mod browser;
mod types;
pub mod viewer;

pub use browser::Browser;
pub use types::{
    Event, Ingredient, ItemInfo, Layout, Recipe, RecipeCard, RecipeKind, RecipePage, Stack, Update,
};
pub(crate) use types::{Lookup, Station};

pub(crate) const ITEM_LIMIT: usize = 8192;
pub(crate) const RECIPE_LIMIT: usize = 256;
pub(crate) const STACK_LIMIT: usize = 64;

/// Marker carried in the native recipe-view note field for shapeless crafting.
pub const SHAPELESS_NOTE: &str = "Shapeless";

/// Separates a cooking recipe's experience line from its cook-time line in the note field.
pub const NOTE_SEPARATOR: char = '\n';

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

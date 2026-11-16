//! Horizon-independent AEI browser model.
//!
//! `browser` owns per-screen state and event handling; `types` contains the
//! model shared with the Horizon integration layer.

mod browser;
mod types;
pub mod viewer;

pub use browser::{Browser, Details, Plan, PlanMaterial, PlanStep, RECENT, Trade};
pub use types::{
    Bookmark, Entry, EntryProps, Event, Ingredient, ItemInfo, Layout, Recipe, RecipeCard,
    RecipeKind, RecipePage, SearchMode, SearchModes, SortOrder, Stack, Update,
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

/// A mod's display name from its namespace, as JEI's tooltips name it: vanilla's is
/// "Minecraft"; others are the namespace's words capitalised (`create_addon` →
/// "Create Addon").
pub fn mod_name(namespace: &str) -> String {
    namespace
        .split(['_', '-'])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(chars).collect()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    #[test]
    fn mod_names_come_from_namespaces() {
        assert_eq!(super::mod_name("minecraft"), "Minecraft");
        assert_eq!(super::mod_name("create_addon"), "Create Addon");
        assert_eq!(super::mod_name("a--b"), "A B");
    }
}

// Whether the open screen can take a viewer recipe and whether the player holds its
// ingredients, decided as the vanilla recipe book decides it (`RecipeDisplayEntry.canCraft`
// over the inventory and the grid). Injected into `game/recipe_book.rs` to reach the
// book's model.
use super::{BookType, Game};

impl Game<'_> {
    /// For each recipe-book id: None when the open screen can't place that recipe (no
    /// crafting grid open, the recipe is too large for it, or the book doesn't have it);
    /// otherwise whether the inventory holds its ingredients.
    pub(in crate::game) fn aei_recipe_status(&mut self, book_ids: &[i32]) -> Vec<Option<bool>> {
        let none = vec![None; book_ids.len()];
        if !self.mod_recipe_screen() || book_ids.iter().all(|&id| id < 0) {
            return none;
        }
        self.ensure_book();
        let available = self.book_available(BookType::Crafting);
        let (width, height) = self.book_grid();
        let Some(book) = self.recipe_ui.book.as_ref() else {
            return none;
        };
        book_ids
            .iter()
            .map(|&id| {
                let entry = &book.entries[book.entry_of_display(id).filter(|_| id >= 0)?];
                entry
                    .fits(width, height)
                    .then(|| book.can_craft(entry, &available))
            })
            .collect()
    }
}

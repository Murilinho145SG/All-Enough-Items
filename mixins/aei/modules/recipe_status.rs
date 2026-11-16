// Whether the open screen can take a viewer recipe and whether the player holds its
// ingredients, decided as the vanilla recipe book decides it (`RecipeDisplayEntry.canCraft`
// over the inventory and the grid), and placing it through that book. Injected into
// `game/recipe_book.rs` to reach the book's model. The crafting grid, furnace, blast
// furnace and smoker each have their book, as in vanilla.
use super::{BookType, Game};
use horizon_client::recipe_book::Tab;

impl Game<'_> {
    /// The book of the open screen, when it takes recipes.
    fn aei_book(&self) -> Option<BookType> {
        self.active_book().filter(|_| !self.spectator())
    }

    /// The index in the book of the entry with `display_id`, when it belongs to `book`.
    fn aei_entry(&self, book: BookType, display_id: i32) -> Option<usize> {
        let tabs = book.tabs();
        let entries = &self.recipe_ui.book.as_ref()?.entries;
        let index = entries.iter().position(|e| e.display_id == display_id)?;
        tabs.contains(&Tab::Category(entries[index].category))
            .then_some(index)
    }

    /// For each recipe-book id: None when the open screen can't place that recipe (no
    /// book screen open, another book's recipe, too large for the grid, or not in the
    /// book); otherwise whether the inventory holds its ingredients.
    pub(in crate::game) fn aei_recipe_status(&mut self, book_ids: &[i32]) -> Vec<Option<bool>> {
        let none = vec![None; book_ids.len()];
        let Some(book_type) = self.aei_book() else {
            return none;
        };
        if book_ids.iter().all(|&id| id < 0) {
            return none;
        }
        self.ensure_book();
        let available = self.book_available(book_type);
        let (width, height) = self.book_grid();
        let indexes: Vec<Option<usize>> = book_ids
            .iter()
            .map(|&id| (id >= 0).then(|| self.aei_entry(book_type, id)).flatten())
            .collect();
        let Some(book) = self.recipe_ui.book.as_ref() else {
            return none;
        };
        indexes
            .into_iter()
            .map(|index| {
                let entry = &book.entries[index?];
                let fits = book_type != BookType::Crafting || entry.fits(width, height);
                fits.then(|| book.can_craft(entry, &available))
            })
            .collect()
    }

    /// The book's click on the recipe with `display_id`, in the open screen's own book;
    /// refused as `handlePlaceRecipe` refuses it.
    pub(in crate::game) fn aei_place_recipe(
        &mut self,
        display_id: i32,
        all: bool,
    ) -> Result<(), String> {
        let Some(book_type) = self.aei_book() else {
            return Err("no screen that takes recipes is open".into());
        };
        if book_type == BookType::Crafting {
            return self.recipe_place(display_id, all);
        }
        if !self.recipe_ui.client.displays().contains(&display_id) {
            return Err(format!(
                "recipe {display_id} is not in the player's recipe book"
            ));
        }
        self.ensure_book();
        let Some(index) = self.aei_entry(book_type, display_id) else {
            return Err("the open screen can't take that recipe".into());
        };
        self.recipe_ui.ghost.clear();
        // A furnace, blast furnace or smoker on a server's menu (a single-player world's
        // too): the ingredients go in with ordinary container clicks, which the server
        // takes as the player's own. Horizon's book placement leaves that menu as it was.
        if self.remote_menu_id() != 0 {
            return self.aei_fill_furnace(display_id, all);
        }
        self.place_recipe(book_type, index, all);
        Ok(())
    }

    /// Puts the ingredient of furnace-family recipe `display_id` in the open furnace's
    /// input with container clicks: one item (picked up, one placed with a right click,
    /// the rest put back), or with `all` every stack of it Shift-clicked in, as the
    /// player would.
    fn aei_fill_furnace(&mut self, display_id: i32, all: bool) -> Result<(), String> {
        use horizon_client::menu::Input;
        // The furnace's input slot, as the game numbers the open container's slots.
        const INPUT: usize = 300;
        let Some(accepts) = crate::game::aei_glue::book_inputs(display_id)
            .and_then(|slots| slots.into_iter().find(|s| !s.is_empty()))
        else {
            return Err("the recipe has no input".into());
        };
        if self.carried.is_some() {
            return Err("an item is held on the cursor".into());
        }
        let takes = |id: &str| accepts.iter().any(|a| a == id);
        if let Some(input) = self.slot_view(INPUT)
            && !takes(&input.item)
        {
            return Err("the furnace already holds another item".into());
        }
        let held: Vec<usize> = (0..36)
            .filter(|&slot| self.slot_view(slot).is_some_and(|s| takes(&s.item)))
            .collect();
        let Some(&first) = held.first() else {
            return Err("the inventory has none of the ingredient".into());
        };
        let ops: Vec<(Option<usize>, i32, Input)> = if all {
            held.iter()
                .map(|&slot| (Some(slot), 0, Input::QuickMove))
                .collect()
        } else {
            vec![
                (Some(first), 0, Input::Pickup),
                (Some(INPUT), 1, Input::Pickup),
                (Some(first), 0, Input::Pickup),
            ]
        };
        if self.remote_menu_ops(&ops) {
            Ok(())
        } else {
            Err("the furnace's slots aren't open".into())
        }
    }
}

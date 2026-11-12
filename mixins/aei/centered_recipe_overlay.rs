use std::{cell::RefCell, ops::ControlFlow, time::Instant};

use super::aei_recipe_paint as paint;
use super::{BrowserEvent, BrowserStack, Game, Layout, Owner, SLOT, inside};
use crate::game::aei::{
    Layout as RecipeLayout,
    viewer::{self, Viewer},
};
use crate::game::native_hooks::{NativeRecipeLayout, NativeRecipeView};
use horizon_client::hud::{HudBatch, rgba};

struct Navigation {
    identity: (String, u32),
    page: usize,
    since: Instant,
}

thread_local! {
    static ITEM_SCROLL: RefCell<i32> = const { RefCell::new(0) };
    static NAVIGATION: RefCell<Option<Navigation>> = const { RefCell::new(None) };
}

fn active(game: &mut Game<'_>) -> Option<Owner> {
    let owner = game.browser_sync()?;
    (owner.native_id() == crate::game::aei::ID && game.panel.recipes.is_some()).then_some(owner)
}

fn recipe_layout(recipe: &NativeRecipeView) -> RecipeLayout {
    match recipe.layout {
        NativeRecipeLayout::Grid => RecipeLayout::Grid,
        NativeRecipeLayout::Cooking => RecipeLayout::Cooking,
        NativeRecipeLayout::Single => RecipeLayout::Single,
        NativeRecipeLayout::Smithing => RecipeLayout::Smithing,
        NativeRecipeLayout::Brewing => RecipeLayout::Brewing,
    }
}

fn layouts(game: &Game<'_>, batch: &HudBatch) -> Option<(Viewer, Option<Layout>)> {
    let page = game.panel_shown()?.recipes.as_ref()?;
    let index = NAVIGATION.with(|cell| {
        let mut state = cell.borrow_mut();
        if state
            .as_ref()
            .is_none_or(|state| state.identity != (page.title.clone(), page.tab))
        {
            *state = Some(Navigation {
                identity: (page.title.clone(), page.tab),
                page: 0,
                since: Instant::now(),
            });
        }
        state.as_ref().map_or(0, |state| state.page)
    });
    let recipes: Vec<_> = page
        .recipes
        .iter()
        .map(|r| (recipe_layout(r), r.width, r.height, r.slots.len()))
        .collect();
    let screen = [batch.width, batch.height];
    let view = Viewer::new(screen, page.tabs.len(), &recipes, index)?;
    let items = view.sidebar(screen).map(|items| Layout {
        area: items.area,
        grid: (items.area[0], 26.0),
        columns: items.columns,
        rows: items.rows,
        arrows_y: 2.0,
        search: items.search,
        settings: Some(items.settings),
    });
    Some((view, items))
}

fn cycle<'a>(stacks: &'a [BrowserStack], game: &Game<'_>) -> Option<&'a BrowserStack> {
    let seconds = NAVIGATION.with(|cell| {
        cell.borrow()
            .as_ref()
            .map_or(0, |state| state.since.elapsed().as_secs() as usize)
    });
    // Resolve before cycling so unavailable alternatives cannot create blank frames.
    let valid: Vec<_> = stacks
        .iter()
        .filter(|stack| super::browser_stack(world_items!(game), stack).is_some())
        .collect();
    valid.get(seconds % valid.len().max(1)).copied()
}

fn recipe_stack(game: &Game<'_>, recipe: usize, slot: usize) -> Option<BrowserStack> {
    let recipe = game.panel_shown()?.recipes.as_ref()?.recipes.get(recipe)?;
    let alternatives = if slot == recipe.slots.len() {
        &recipe.results
    } else {
        recipe.slots.get(slot)?
    };
    cycle(alternatives, game).cloned()
}

fn hovered_item(game: &Game<'_>, layout: &Layout) -> Option<(usize, BrowserStack)> {
    let first = game.panel.page * layout.per_page();
    for offset in 0..layout.per_page() {
        let (x, y) = layout.slot(offset);
        if inside(game.cursor, [x, y, SLOT, SLOT]) {
            let index = first + offset;
            game.panel.stacks.get(index)?.as_ref()?;
            return Some((index, game.panel_shown()?.items.get(index)?.clone()));
        }
    }
    None
}

fn hovered_recipe(game: &Game<'_>, view: &Viewer) -> Option<(usize, usize, BrowserStack)> {
    for (i, card) in &view.cards {
        for (slot, rect) in &card.slots {
            if viewer::contains(*rect, game.cursor) {
                return Some((*i, *slot, recipe_stack(game, *i, *slot)?));
            }
        }
    }
    None
}

fn draw_stack(
    game: &Game<'_>,
    batch: &mut HudBatch,
    sent: &BrowserStack,
    rect: viewer::Rect,
    name: &str,
) {
    let Some(stack) = super::browser_stack(world_items!(game), sent) else {
        return;
    };
    let [x, y, w, h] = rect;
    game.draw_item(batch, &stack, x + (w - 16.0) / 2.0, y + (h - 16.0) / 2.0);
    if viewer::contains(rect, game.cursor) {
        paint::fill(
            game,
            batch,
            [x + 1.0, y + 1.0, w - 2.0, h - 2.0],
            rgba(255, 255, 255, 100),
        );
    }
    horizon_client::widgets::record(name, rect, true);
}

fn category(game: &Game<'_>, station: &str) -> String {
    let (key, fallback) = match station {
        "minecraft:crafting_table" => ("container.crafting", "Crafting"),
        "minecraft:furnace" => ("block.minecraft.furnace", "Smelting"),
        "minecraft:blast_furnace" => ("block.minecraft.blast_furnace", "Blasting"),
        "minecraft:smoker" => ("block.minecraft.smoker", "Smoking"),
        "minecraft:campfire" => ("block.minecraft.campfire", "Campfire Cooking"),
        "minecraft:stonecutter" => ("block.minecraft.stonecutter", "Stonecutting"),
        "minecraft:smithing_table" => ("block.minecraft.smithing_table", "Smithing"),
        _ => ("block.minecraft.brewing_stand", "Brewing"),
    };
    game.translate(key, fallback)
}

fn draw_recipes(game: &Game<'_>, batch: &mut HudBatch, view: &Viewer) {
    let Some(page) = game.panel_shown().and_then(|shown| shown.recipes.as_ref()) else {
        return;
    };
    for (i, rect) in view.tabs.iter().enumerate() {
        paint::window(game, batch, *rect);
        if i != page.tab as usize {
            paint::fill(
                game,
                batch,
                [rect[0] + 3.0, rect[1] + 3.0, rect[2] - 6.0, rect[3] - 4.0],
                rgba(162, 162, 162, 255),
            );
        }
        draw_stack(
            game,
            batch,
            &page.tabs[i],
            [rect[0] + 4.0, rect[1] + 3.0, 18.0, 18.0],
            &format!("Recipe category {i}"),
        );
    }
    paint::window(game, batch, view.area);
    if let Some(station) = page.tabs.get(page.tab as usize) {
        paint::window(game, batch, view.catalyst);
        draw_stack(game, batch, station, view.catalyst, "Recipe catalyst");
        paint::fill(game, batch, view.category, rgba(160, 160, 160, 255));
        paint::label(
            game,
            batch,
            &category(game, &station.item),
            view.category,
            u32::MAX,
        );
        horizon_client::widgets::record("Show all recipes in category", view.category, true);
    }
    paint::fill(game, batch, view.page_label, rgba(160, 160, 160, 255));
    paint::label(
        game,
        batch,
        &format!("{}/{}", view.page + 1, view.pages.len()),
        view.page_label,
        u32::MAX,
    );
    for (i, rect) in view.category_arrows.iter().enumerate() {
        paint::button(
            game,
            batch,
            *rect,
            if i == 0 { "<" } else { ">" },
            page.tabs.len() > 1,
            if i == 0 {
                "Previous category"
            } else {
                "Next category"
            },
        );
    }
    for (i, rect) in view.page_arrows.iter().enumerate() {
        paint::button(
            game,
            batch,
            *rect,
            if i == 0 { "<" } else { ">" },
            view.pages.len() > 1,
            if i == 0 {
                "Previous recipe page"
            } else {
                "Next recipe page"
            },
        );
    }
    for (index, card) in &view.cards {
        let recipe = &page.recipes[*index];
        paint::bevel(game, batch, card.area, true);
        if recipe.layout == NativeRecipeLayout::Grid {
            for i in 0..9 {
                paint::slot(
                    game,
                    batch,
                    [
                        card.area[0] + 4.0 + (i % 3) as f32 * SLOT,
                        card.area[1] + 4.0 + (i / 3) as f32 * SLOT,
                        SLOT,
                        SLOT,
                    ],
                );
            }
        }
        for (_, rect) in &card.slots {
            paint::slot(game, batch, *rect);
        }
        paint::arrow(game, batch, card.arrow);
        if let Some(rect) = card.flame {
            batch.sprite(
                &game.gui.atlas,
                "gui/sprites/container/furnace/lit_progress",
                rect[0],
                rect[1],
            );
        }
        if recipe.note == crate::game::aei::SHAPELESS_NOTE {
            paint::label(
                game,
                batch,
                "*",
                [card.area[0] + 112.0, card.area[1] + 2.0, 12.0, 12.0],
                paint::INK,
            );
        } else if !recipe.note.is_empty() {
            batch.text(
                &game.gui.atlas,
                &recipe.note,
                card.note.0,
                card.note.1,
                paint::INK,
                false,
            );
        }
        if let Some(enabled) = recipe.action {
            paint::button(
                game,
                batch,
                card.action,
                "+",
                enabled,
                &format!("Move recipe {index}"),
            );
        }
        for (slot, rect) in &card.slots {
            if let Some(stack) = recipe_stack(game, *index, *slot) {
                draw_stack(
                    game,
                    batch,
                    &stack,
                    *rect,
                    &format!("Recipe {index} ingredient {slot}"),
                );
            }
        }
    }
}

fn draw_items(game: &mut Game<'_>, batch: &mut HudBatch, layout: &Layout) {
    let pages = game.browser_pages(layout);
    let delta = ITEM_SCROLL.with(|cell| cell.replace(0));
    game.panel.page = (game.panel.page as i64 + i64::from(delta)).rem_euclid(pages as i64) as usize;
    game.panel.page = game.panel.page.min(pages - 1);
    let first = game.panel.page * layout.per_page();
    for offset in 0..layout.per_page() {
        let index = first + offset;
        let Some(stack) = game.panel_shown().and_then(|shown| shown.items.get(index)) else {
            break;
        };
        let (x, y) = layout.slot(offset);
        draw_stack(
            game,
            batch,
            stack,
            [x, y, SLOT, SLOT],
            &format!("Item {index}"),
        );
    }
    paint::label(
        game,
        batch,
        &format!("{}/{}", game.panel.page + 1, pages),
        [
            layout.grid.0 + 20.0,
            layout.arrows_y + 3.0,
            layout.width() - 40.0,
            16.0,
        ],
        u32::MAX,
    );
    if let Some(rect) = layout.settings {
        paint::button(game, batch, rect, "...", true, "AEI settings");
    }
    let rects = layout.browser_arrow_rects(true);
    for (i, rect) in rects.into_iter().enumerate() {
        paint::button(
            game,
            batch,
            rect,
            if i == 0 { "<" } else { ">" },
            pages > 1,
            if i == 0 {
                "Previous item page"
            } else {
                "Next item page"
            },
        );
    }
    let hint = game.translate("gui.recipebook.search_hint", "Search...");
    let search = game.browser_search(layout);
    search.suggestion = (search.value.is_empty() && !search.focused).then_some(hint);
    let mut search = search.clone();
    search.draw(batch, &game.gui.atlas, None);
}

pub(super) fn draw(game: &mut Game<'_>, batch: &mut HudBatch) -> ControlFlow<()> {
    if active(game).is_none() {
        return ControlFlow::Continue(());
    }
    let Some((view, items)) = layouts(game, batch) else {
        return ControlFlow::Continue(());
    };
    paint::fill(
        game,
        batch,
        [0.0, 0.0, batch.width, batch.height],
        rgba(0, 0, 0, 96),
    );
    draw_recipes(game, batch, &view);
    if let Some(items) = &items {
        draw_items(game, batch, items);
    } else if let Some(search) = game.panel.search.as_mut() {
        search.focused = false;
    }
    let hovered = hovered_recipe(game, &view)
        .map(|(_, _, stack)| stack)
        .or_else(|| {
            items
                .as_ref()
                .and_then(|items| hovered_item(game, items))
                .map(|(_, stack)| stack)
        })
        .or_else(|| {
            let page = game.panel_shown()?.recipes.as_ref()?;
            view.tabs
                .iter()
                .position(|rect| viewer::contains(*rect, game.cursor))
                .and_then(|i| page.tabs.get(i).cloned())
                .or_else(|| {
                    viewer::contains(view.catalyst, game.cursor)
                        .then(|| page.tabs.get(page.tab as usize).cloned())
                        .flatten()
                })
        });
    if let Some(stack) = hovered.and_then(|sent| super::browser_stack(world_items!(game), &sent)) {
        game.draw_stack_tooltip(batch, &stack, game.cursor.0, game.cursor.1);
    } else if game.carried.is_some() {
        game.draw_carried(batch);
    }
    ControlFlow::Break(())
}

fn turn_page(view: &Viewer, forward: bool) {
    NAVIGATION.with(|cell| {
        if let Some(state) = cell.borrow_mut().as_mut() {
            state.page = if forward {
                (view.page + 1) % view.pages.len()
            } else {
                (view.page + view.pages.len() - 1) % view.pages.len()
            };
        }
    });
}

pub(super) fn press(game: &mut Game<'_>, right: bool) -> ControlFlow<bool> {
    let Some(owner) = active(game) else {
        return ControlFlow::Continue(());
    };
    let Some(batch) = game.browser_batch() else {
        return ControlFlow::Continue(());
    };
    let Some((view, items)) = layouts(game, &batch) else {
        return ControlFlow::Continue(());
    };
    if let Some(search) = game.panel.search.as_mut() {
        search.focused = false;
    }
    if let Some(items) = &items {
        if items.settings.is_some_and(|rect| inside(game.cursor, rect)) {
            if !right {
                game.list_native_mods();
                if let Some(index) =
                    game.settings.mods.installed.iter().position(|entry| {
                        entry.id == crate::game::aei::ID && entry.native.is_some()
                    })
                {
                    game.mod_browser_closed();
                    game.panel.recipes_settings_return = Some(game.screen);
                    game.open_options_at(false, horizon_client::options_ui::Page::NativeMod(index));
                }
            }
            return ControlFlow::Break(true);
        }
        if inside(game.cursor, items.search) {
            let shift = game.has_shift_down();
            let cursor = game.cursor;
            let mut search = game.browser_search(items).clone();
            if right {
                search.set_value("");
            } else {
                search.click(&game.gui.atlas, cursor.0, shift, false);
            }
            search.focused = true;
            search.reset_blink();
            game.panel.search = Some(search);
            if right {
                game.browser_searched();
            }
            return ControlFlow::Break(true);
        }
        let pages = game.browser_pages(items);
        if let Some(i) = items
            .browser_arrow_rects(true)
            .iter()
            .position(|rect| inside(game.cursor, *rect))
        {
            game.panel.page = if i == 1 {
                (game.panel.page + 1) % pages
            } else {
                (game.panel.page + pages - 1) % pages
            };
            return ControlFlow::Break(true);
        }
        if let Some((index, _)) = hovered_item(game, items) {
            game.panel_event(
                &owner,
                BrowserEvent::Clicked {
                    index: index as u32,
                    right,
                },
            );
            return ControlFlow::Break(true);
        }
    }
    if let Some(index) = view
        .tabs
        .iter()
        .position(|rect| viewer::contains(*rect, game.cursor))
    {
        game.panel_event(&owner, BrowserEvent::Tab(index as u32));
    } else if let Some(i) = view
        .category_arrows
        .iter()
        .position(|rect| viewer::contains(*rect, game.cursor))
    {
        let page = game
            .panel_shown()
            .and_then(|shown| shown.recipes.as_ref())
            .unwrap();
        let count = page.tabs.len();
        if count > 1 {
            let tab = page.tab as usize;
            let tab = if i == 1 {
                (tab + 1) % count
            } else {
                (tab + count - 1) % count
            };
            game.panel_event(&owner, BrowserEvent::Tab(tab as u32));
        }
    } else if viewer::contains(view.category, game.cursor) {
        crate::game::aei_glue::show_category(game);
    } else if let Some(i) = view
        .page_arrows
        .iter()
        .position(|rect| viewer::contains(*rect, game.cursor))
    {
        turn_page(&view, i == 1);
    } else if let Some((recipe, slot, stack)) = hovered_recipe(game, &view) {
        game.panel_event(
            &owner,
            BrowserEvent::SlotClicked {
                recipe: recipe as u32,
                slot: slot as u32,
                stack,
                right,
            },
        );
    } else {
        for (index, card) in &view.cards {
            if viewer::contains(card.action, game.cursor) {
                let enabled = game
                    .panel_shown()
                    .and_then(|shown| shown.recipes.as_ref())
                    .and_then(|p| p.recipes.get(*index))
                    .and_then(|r| r.action)
                    == Some(true);
                if enabled {
                    game.panel_event(&owner, BrowserEvent::Action(*index as u32));
                }
                break;
            }
        }
    }
    ControlFlow::Break(true)
}

pub(super) fn pointed(game: &mut Game<'_>) -> ControlFlow<Option<BrowserStack>> {
    if active(game).is_none() {
        return ControlFlow::Continue(());
    }
    let Some(batch) = game.browser_batch() else {
        return ControlFlow::Continue(());
    };
    let Some((view, items)) = layouts(game, &batch) else {
        return ControlFlow::Continue(());
    };
    let stack = hovered_recipe(game, &view)
        .map(|(_, _, stack)| stack)
        .or_else(|| {
            items
                .as_ref()
                .and_then(|items| hovered_item(game, items))
                .map(|(_, stack)| stack)
        });
    ControlFlow::Break(stack)
}

fn scroll(game: &Game<'_>, amount: f64) -> bool {
    if !matches!(
        game.screen,
        crate::game::Screen::Inventory | crate::game::Screen::Creative
    ) || game.title.is_some()
        || game.panel_shown().is_none_or(|shown| {
            shown.owner.native_id() != crate::game::aei::ID || shown.recipes.is_none()
        })
    {
        return false;
    }
    let Some(batch) = game.browser_batch() else {
        return false;
    };
    let Some((view, items)) = layouts(game, &batch) else {
        return false;
    };
    if amount == 0.0 {
        return true;
    }
    if let Some(items) = items.filter(|items| inside(game.cursor, items.area)) {
        let _ = items;
        ITEM_SCROLL.with(|cell| *cell.borrow_mut() += if amount < 0.0 { 1 } else { -1 });
    } else {
        turn_page(&view, amount < 0.0);
    }
    true
}

pub(super) fn back(game: &mut Game<'_>) -> ControlFlow<bool> {
    let Some(owner) = active(game) else {
        return ControlFlow::Continue(());
    };
    game.mod_browser_closed();
    game.panel_event(&owner, BrowserEvent::Back);
    ControlFlow::Break(true)
}

impl Game<'_> {
    pub(in crate::game) fn aei_recipe_scroll(&self, amount: f64) -> bool {
        scroll(self, amount)
    }
}

impl Game<'_> {
    pub(in crate::game) fn aei_reset_viewer(&self) {
        NAVIGATION.with(|cell| *cell.borrow_mut() = None);
        ITEM_SCROLL.with(|cell| *cell.borrow_mut() = 0);
    }
}

impl Game<'_> {
    pub(in crate::game) fn aei_recipe_modal(&mut self, batch: &HudBatch) -> bool {
        active(self).is_some() && layouts(self, batch).is_some()
    }
}

impl Game<'_> {
    pub(in crate::game) fn aei_recipe_is_modal(&mut self) -> bool {
        self.browser_batch()
            .is_some_and(|batch| self.aei_recipe_modal(&batch))
    }

    pub(in crate::game) fn aei_focus_search(&mut self) -> bool {
        let Some(owner) = self.browser_sync() else {
            return false;
        };
        if owner.native_id() != crate::game::aei::ID {
            return false;
        }
        let Some(batch) = self.browser_batch() else {
            return false;
        };
        let items = if self.panel.recipes.is_some() {
            layouts(self, &batch).and_then(|(_, items)| items)
        } else {
            self.browser_layout(&batch)
        };
        let Some(items) = items else {
            return false;
        };
        let search = self.browser_search(&items);
        search.focused = true;
        search.reset_blink();
        true
    }
}

impl Game<'_> {
    pub(in crate::game) fn aei_reset_catalogue(&mut self) {
        self.aei_reset_viewer();
        self.panel.search = None;
        self.panel.sent.clear();
        self.panel.page = 0;
    }
}

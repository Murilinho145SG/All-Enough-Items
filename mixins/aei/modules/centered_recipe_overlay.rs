// AEI's screens over a container: the item list on the right (always), and the recipe
// window with its category tabs and station tab when a lookup is open. Geometry comes from
// the engine's `viewer`; drawing from `aei_recipe_paint`.
use std::{cell::RefCell, ops::ControlFlow, time::Instant};

use super::aei_recipe_paint as paint;
use super::{BrowserEvent, BrowserStack, Game, Layout, Owner, inside};
use crate::game::aei::{
    NOTE_SEPARATOR, SHAPELESS_NOTE,
    viewer::{self, BookmarkList, Category, Sidebar, Viewer},
};
use crate::game::native_hooks::{NativeRecipeLayout, NativeRecipeView};
use horizon_client::hud::{HudBatch, rgba};

const FLAME: &str = "gui/sprites/container/furnace/lit_progress";
const COOK_ARROW: &str = "gui/sprites/container/furnace/burn_progress";
const BREW_ARROW: &str = "gui/sprites/container/brewing_stand/brew_progress";
const BUBBLES: &str = "gui/sprites/container/brewing_stand/bubbles";
const BLAZE_FUEL: &str = "gui/sprites/container/brewing_stand/fuel_length";
/// Ticks a flame takes to burn down before it restarts.
const FLAME_TICKS: f32 = 300.0;

struct Navigation {
    identity: (String, u32),
    page: usize,
}

thread_local! {
    static ITEM_SCROLL: RefCell<i32> = const { RefCell::new(0) };
    static BOOKMARK_PAGE: RefCell<usize> = const { RefCell::new(0) };
    static NAVIGATION: RefCell<Option<Navigation>> = const { RefCell::new(None) };
    static STARTED: Instant = Instant::now();
    static CYCLE: RefCell<Option<(Instant, f64)>> = const { RefCell::new(None) };
}

/// What AEI shows this frame: the recipe window (when a lookup is open) and the item list
/// with Horizon's matching panel layout.
struct Screen {
    view: Option<Viewer>,
    items: Option<(Sidebar, Layout)>,
    bookmarks: Option<BookmarkList>,
}

/// AEI's panel is up on a container screen.
fn owned(game: &mut Game<'_>) -> Option<Owner> {
    let owner = game.browser_sync()?;
    (owner.native_id() == crate::game::aei::ID).then_some(owner)
}

/// AEI's panel is up with a recipe lookup open.
fn active(game: &mut Game<'_>) -> Option<Owner> {
    owned(game).filter(|_| game.panel.recipes.is_some())
}

fn recipe_layout(recipe: &NativeRecipeView) -> crate::game::aei::Layout {
    use crate::game::aei::Layout as RecipeLayout;
    match recipe.layout {
        NativeRecipeLayout::Grid => RecipeLayout::Grid,
        NativeRecipeLayout::Cooking => RecipeLayout::Cooking,
        NativeRecipeLayout::Single => RecipeLayout::Single,
        NativeRecipeLayout::Smithing => RecipeLayout::Smithing,
        NativeRecipeLayout::Brewing => RecipeLayout::Brewing,
    }
}

fn seconds() -> f32 {
    STARTED.with(|start| start.elapsed().as_secs_f32())
}

/// The recipe window for the open lookup.
fn recipe_view(game: &Game<'_>, screen: [f32; 2]) -> Option<Viewer> {
    let page = game.panel_shown()?.recipes.as_ref()?;
    let index = NAVIGATION.with(|cell| {
        let mut state = cell.borrow_mut();
        let identity = (page.title.clone(), page.tab);
        if state
            .as_ref()
            .is_none_or(|state| state.identity != identity)
        {
            *state = Some(Navigation { identity, page: 0 });
        }
        state.as_ref().map_or(0, |state| state.page)
    });
    let station = page
        .tabs
        .get(page.tab as usize)
        .map_or("", |stack| stack.item.as_str());
    let category = page.recipes.first().map_or(Category::Crafting, |recipe| {
        Category::of(recipe_layout(recipe), station)
    });
    let recipes: Vec<_> = page
        .recipes
        .iter()
        .map(|r| (r.width, r.height, r.slots.len()))
        .collect();
    Viewer::new(
        screen,
        page.tabs.len(),
        page.tab as usize,
        category,
        &recipes,
        index,
    )
}

/// The right edge of the screen's GUI: the recipe window, or the open container.
fn gui_right(game: &Game<'_>, batch: &HudBatch, view: Option<&Viewer>) -> f32 {
    if let Some(view) = view {
        view.area[0] + view.area[2]
    } else if game.screen == crate::game::Screen::Creative {
        Game::creative_origin(batch).0 + 195.0
    } else {
        let image = game.container_image(batch);
        (image.left + image.width) as f32
    }
}

/// The left edge of the screen's GUI: the recipe window's station tab, or the container.
fn gui_left(game: &Game<'_>, batch: &HudBatch, view: Option<&Viewer>) -> f32 {
    if let Some(view) = view {
        view.catalyst_tab[0]
    } else if game.screen == crate::game::Screen::Creative {
        Game::creative_origin(batch).0
    } else {
        game.container_image(batch).left as f32
    }
}

fn screen(game: &Game<'_>, batch: &HudBatch, recipes: bool) -> Option<Screen> {
    let size = [batch.width, batch.height];
    let view = if recipes {
        Some(recipe_view(game, size)?)
    } else {
        None
    };
    let right = gui_right(game, batch, view.as_ref());
    let left = gui_left(game, batch, view.as_ref());
    let options = crate::game::aei_glue::list_options();
    let items = Sidebar::with(size, [left, right], game.panel.stacks.len(), options).map(|items| {
        let layout = Layout {
            area: items.area,
            grid: items.grid,
            columns: items.columns,
            rows: items.rows,
            arrows_y: items.nav[1],
            search: items.search,
            settings: Some(items.settings),
        };
        (items, layout)
    });
    let bookmarks = BookmarkList::new(size, left, crate::game::aei_glue::bookmark_count());
    // Without a recipe window, AEI only takes over when its list fits.
    (view.is_some() || items.is_some()).then_some(Screen {
        view,
        items,
        bookmarks,
    })
}

/// Seconds the ingredients have cycled for: the clock stands still while Shift is held,
/// as JEI pauses its cycling.
fn cycle_seconds(game: &Game<'_>) -> usize {
    CYCLE.with(|cell| {
        let mut clock = cell.borrow_mut();
        let now = Instant::now();
        let (last, total) = clock.get_or_insert((now, 0.0));
        if !game.has_shift_down() {
            *total += now.duration_since(*last).as_secs_f64();
        }
        *last = now;
        *total as usize
    })
}

fn cycle<'a>(stacks: &'a [BrowserStack], game: &Game<'_>) -> Option<&'a BrowserStack> {
    let seconds = cycle_seconds(game);
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
        if inside(game.cursor, [x, y, viewer::SLOT, viewer::SLOT]) {
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

/// The shown category's catalysts (JEI's column left of the window) and their wells.
fn catalysts(game: &Game<'_>, view: &Viewer) -> Vec<(BrowserStack, viewer::Rect)> {
    let Some(page) = game.panel_shown().and_then(|shown| shown.recipes.as_ref()) else {
        return Vec::new();
    };
    let Some(station) = page.tabs.get(page.tab as usize) else {
        return Vec::new();
    };
    let mut ids: Vec<&str> = viewer::catalysts(&station.item).to_vec();
    if ids.is_empty() {
        ids.push(&station.item);
    }
    // Only blocks the game has: a catalyst from a later version is left out.
    ids.retain(|id| world_items!(game).get(id).is_some());
    let (_, wells) = view.catalyst_column(ids.len());
    ids.into_iter()
        .zip(wells)
        .map(|(id, rect)| {
            let stack = BrowserStack {
                item: id.to_owned(),
                count: 1,
                components: String::new(),
            };
            (stack, rect)
        })
        .collect()
}

/// The catalyst under the pointer.
fn hovered_catalyst(game: &Game<'_>, view: &Viewer) -> Option<BrowserStack> {
    catalysts(game, view)
        .into_iter()
        .find(|(_, rect)| viewer::contains(*rect, game.cursor))
        .map(|(stack, _)| stack)
}

/// The station or category icon under the pointer.
fn hovered_station(game: &Game<'_>, view: &Viewer) -> Option<BrowserStack> {
    let page = game.panel_shown()?.recipes.as_ref()?;
    match view
        .tabs
        .iter()
        .find(|(_, rect)| viewer::contains(*rect, game.cursor))
    {
        Some((tab, _)) => page.tabs.get(*tab).cloned(),
        None => hovered_catalyst(game, view),
    }
}

/// Draws `sent` centred in `rect`, with the slot highlight when `highlight` and hovered.
fn draw_stack(
    game: &Game<'_>,
    batch: &mut HudBatch,
    sent: &BrowserStack,
    rect: viewer::Rect,
    name: &str,
    highlight: bool,
) {
    let Some(stack) = super::browser_stack(world_items!(game), sent) else {
        return;
    };
    let [x, y, w, h] = rect;
    let item = (
        (x + (w - 16.0) / 2.0).floor(),
        (y + (h - 16.0) / 2.0).floor(),
    );
    game.draw_item(batch, &stack, item.0, item.1);
    if highlight && viewer::contains(rect, game.cursor) {
        paint::highlight(game, batch, item);
    }
    // "Item 3: minecraft:stone x1": the control socket's id stops at the
    // first ':' (`item-3`), so automation keeps stable ids and reads the stack.
    let label = format!("{name}: {} x{}", sent.item, sent.count);
    horizon_client::widgets::record(&label, rect, true);
}

/// The category's title as JEI names it.
fn category_title(game: &Game<'_>, station: &str) -> String {
    match station {
        "minecraft:crafting_table" => game.translate("container.crafting", "Crafting"),
        "minecraft:furnace" => "Smelting".into(),
        "minecraft:blast_furnace" => "Blasting".into(),
        "minecraft:smoker" => "Smoking".into(),
        "minecraft:campfire" | "minecraft:soul_campfire" => "Campfire Cooking".into(),
        "minecraft:stonecutter" => "Stonecutting".into(),
        "minecraft:smithing_table" => "Smithing".into(),
        _ => "Brewing".into(),
    }
}

fn draw_cooking(game: &Game<'_>, batch: &mut HudBatch, card: &viewer::Card, note: &str) {
    let atlas = &game.gui.atlas;
    let (experience, time) = note.split_once(NOTE_SEPARATOR).unwrap_or(("", note));
    let cook = time
        .trim_end_matches('s')
        .parse::<f32>()
        .unwrap_or(10.0)
        .max(0.05);
    let now = seconds();
    paint::arrow(game, batch, card.arrow);
    let [ax, ay, _, _] = card.arrow;
    batch.sprite_part(atlas, COOK_ARROW, ax, ay, (now % cook) / cook);
    if let Some([fx, fy, fw, fh]) = card.flame {
        let left = 1.0 - (now * 20.0 % FLAME_TICKS) / FLAME_TICKS;
        let start = batch.vertices.len();
        batch.sprite(atlas, FLAME, fx, fy);
        batch.clip_since(
            start,
            fx,
            fy + (fh * (1.0 - left)).round(),
            fx + fw,
            fy + fh,
        );
    }
    if card.top_note[2] > 0.0 && !experience.is_empty() {
        paint::right_aligned(game, batch, experience, card.top_note);
    }
    if !time.is_empty() {
        let [x, y, w, h] = card.bottom_note;
        let font = &game.gui.atlas.font;
        let tx = (x + (w - font.width(time)) / 2.0).round();
        batch.text(atlas, time, tx, y + h - 8.0, paint::grey_text(), false);
    }
}

fn draw_brewing(game: &Game<'_>, batch: &mut HudBatch, card: &viewer::Card) {
    let atlas = &game.gui.atlas;
    let (ox, oy) = (card.area[0] + 4.0, card.area[1] + 4.0);
    let now = seconds();
    batch.sprite(atlas, BLAZE_FUEL, ox + 5.0, oy + 30.0);
    // The bubbles rise in seven steps; the arrow fills down over 20 seconds.
    let step = (now * 20.0 / 14.0) as usize % 7;
    let bubbles = [0.0f32, 5.0, 9.0, 13.0, 18.0, 23.0, 29.0][step].min(28.0);
    let (bx, by) = (ox + 9.0, oy + 1.0);
    let start = batch.vertices.len();
    batch.sprite(atlas, BUBBLES, bx, by);
    batch.clip_since(start, bx, by + 28.0 - bubbles, bx + 12.0, by + 29.0);
    let [ax, ay, aw, ah] = card.arrow;
    let filled = ((now % 20.0) / 20.0 * ah).round();
    let start = batch.vertices.len();
    batch.sprite(atlas, BREW_ARROW, ax, ay);
    batch.clip_since(start, ax, ay, ax + aw, ay + filled);
}

/// What a recipe's "+" button can do on the open screen.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Transfer {
    /// The player holds the ingredients: moves them into the grid.
    Ready,
    /// Some ingredients are missing: the grid shows the recipe as a ghost.
    Missing,
    /// The player's recipe book doesn't hold the recipe, so it can't be placed.
    Locked,
}

impl Transfer {
    fn tooltip(self) -> &'static str {
        match self {
            Self::Ready => "Move Items",
            Self::Missing => "Missing Items",
            Self::Locked => "Recipe not unlocked",
        }
    }
}

/// The "+" button of each card on the page, parallel to `view.cards`. None hides it:
/// the open screen has no crafting grid that fits the recipe (a crafting table for 3x3
/// recipes, the inventory's 2x2 grid for small ones), or it isn't a crafting recipe.
fn transfers(game: &mut Game<'_>, view: &Viewer) -> Vec<Option<Transfer>> {
    let ids = crate::game::aei_glue::shown_book_ids();
    let Some(page) = game.panel_shown().and_then(|shown| shown.recipes.as_ref()) else {
        return vec![None; view.cards.len()];
    };
    let cards: Vec<(i32, bool)> = view
        .cards
        .iter()
        .map(|(index, _)| {
            let recipe = &page.recipes[*index];
            let crafting = recipe.layout == NativeRecipeLayout::Grid;
            let id = ids.get(*index).copied().filter(|_| crafting).unwrap_or(-1);
            (id, recipe.action == Some(true))
        })
        .collect();
    let ids: Vec<i32> = cards.iter().map(|(id, _)| *id).collect();
    game.aei_recipe_status(&ids)
        .into_iter()
        .zip(cards)
        .map(|(status, (_, known))| {
            status.map(|craftable| match (known, craftable) {
                (false, _) => Transfer::Locked,
                (true, true) => Transfer::Ready,
                (true, false) => Transfer::Missing,
            })
        })
        .collect()
}

/// The "+" button under the pointer and what it does.
fn hovered_transfer(
    game: &Game<'_>,
    view: &Viewer,
    transfers: &[Option<Transfer>],
) -> Option<(usize, Transfer)> {
    view.cards
        .iter()
        .zip(transfers)
        .find_map(|((index, card), transfer)| {
            viewer::contains(card.action, game.cursor)
                .then_some(*transfer)
                .flatten()
                .map(|transfer| (*index, transfer))
        })
}

fn draw_recipes(
    game: &Game<'_>,
    batch: &mut HudBatch,
    view: &Viewer,
    transfers: &[Option<Transfer>],
) {
    let Some(page) = game.panel_shown().and_then(|shown| shown.recipes.as_ref()) else {
        return;
    };
    let station = page.tabs.get(page.tab as usize);
    paint::window(game, batch, view.area);

    paint::strip(game, batch, view.category);
    if let Some(station) = station {
        let title = category_title(game, &station.item);
        let [x, y, w, h] = view.category;
        paint::title(game, batch, &title, [x + 14.0, y, w - 28.0, h]);
    }
    horizon_client::widgets::record("Show all recipes in category", view.category, true);
    paint::strip(game, batch, view.page_label);
    paint::title(
        game,
        batch,
        &format!("{}/{}", view.page + 1, view.pages),
        view.page_label,
    );
    let names = [
        ["Previous category", "Next category"],
        ["Previous recipe page", "Next recipe page"],
    ];
    let enabled = [page.tabs.len() > 1, view.pages > 1];
    for (row, arrows) in [view.category_arrows, view.page_arrows].iter().enumerate() {
        for (i, rect) in arrows.iter().enumerate() {
            paint::arrow_button(game, batch, *rect, i == 1, enabled[row], names[row][i]);
        }
    }

    for ((index, card), transfer) in view.cards.iter().zip(transfers) {
        let recipe = &page.recipes[*index];
        paint::plate(game, batch, card.area);
        for rect in &card.empty {
            paint::slot(game, batch, *rect);
        }
        for (_, rect) in &card.slots {
            paint::slot(game, batch, *rect);
        }
        match recipe.layout {
            NativeRecipeLayout::Cooking => draw_cooking(game, batch, card, &recipe.note),
            NativeRecipeLayout::Brewing => draw_brewing(game, batch, card),
            _ => paint::arrow(game, batch, card.arrow),
        }
        if recipe.note == SHAPELESS_NOTE {
            paint::shapeless(game, batch, card.shapeless);
        }
        if crate::game::aei_glue::recipe_bookmarkable(*index) {
            let marked = crate::game::aei_glue::recipe_bookmarked(*index);
            let name = format!("Bookmark recipe {index}");
            paint::bookmark_button(game, batch, card.bookmark, marked, &name);
        }
        if let Some(transfer) = transfer {
            let name = format!("Move recipe {index}");
            let enabled = *transfer != Transfer::Locked;
            let missing = *transfer == Transfer::Missing;
            paint::transfer_button(game, batch, card.action, enabled, missing, &name);
        }
        for (slot, rect) in &card.slots {
            if let Some(stack) = recipe_stack(game, *index, *slot) {
                let name = format!("Recipe {index} ingredient {slot}");
                draw_stack(game, batch, &stack, *rect, &name, true);
            }
        }
    }

    if station.is_some() {
        let catalysts = catalysts(game, view);
        let (tab, wells) = view.catalyst_column(catalysts.len());
        paint::station_tab(game, batch, tab, view.catalyst);
        for rect in wells.iter().skip(1) {
            paint::slot(game, batch, *rect);
        }
        for (i, (stack, rect)) in catalysts.iter().enumerate() {
            draw_stack(
                game,
                batch,
                stack,
                *rect,
                &format!("Recipe catalyst {i}"),
                true,
            );
        }
    }
    for (i, rect) in &view.tabs {
        paint::tab(game, batch, *rect, *i == page.tab as usize);
        if let Some(icon) = page.tabs.get(*i) {
            let [x, y, _, _] = *rect;
            let name = format!("Recipe category {i}");
            draw_stack(
                game,
                batch,
                icon,
                [x + 4.0, y + 4.0, 16.0, 16.0],
                &name,
                false,
            );
        }
        horizon_client::widgets::record(&format!("Recipe category {i}"), *rect, true);
    }
}

fn draw_items(game: &mut Game<'_>, batch: &mut HudBatch, items: &Sidebar, layout: &Layout) {
    let pages = game.browser_pages(layout);
    let delta = ITEM_SCROLL.with(|cell| cell.replace(0));
    game.panel.page = (game.panel.page as i64 + i64::from(delta)).rem_euclid(pages as i64) as usize;
    game.panel.page = game.panel.page.min(pages - 1);
    let first = game.panel.page * layout.per_page();
    if crate::game::aei_glue::cheat_mode() {
        // JEI's cheat mode tint behind the list.
        let (x, y) = layout.slot(0);
        let w = layout.columns as f32 * viewer::SLOT;
        let h = layout.rows as f32 * viewer::SLOT;
        paint::fill(game, batch, [x, y, w, h], rgba(255, 0, 0, 0x30));
    }
    for offset in 0..layout.per_page() {
        let index = first + offset;
        let Some(stack) = game.panel_shown().and_then(|shown| shown.items.get(index)) else {
            break;
        };
        let (x, y) = layout.slot(offset);
        let rect = [x, y, viewer::SLOT, viewer::SLOT];
        draw_stack(game, batch, stack, rect, &format!("Item {index}"), true);
        if crate::game::aei_glue::shown_hidden(index) {
            // Edit mode: a hidden entry, shaded red as JEI marks it.
            paint::fill(game, batch, rect, rgba(255, 0, 0, 0x60));
        }
    }

    let [back, forward] = items.arrows();
    let [x, y, w, h] = items.nav;
    paint::strip(game, batch, [x + back[2], y, w - back[2] - forward[2], h]);
    let label = format!("{}/{}", game.panel.page + 1, pages);
    if game.gui.atlas.font.width(&label) <= w - back[2] - forward[2] {
        paint::title(game, batch, &label, items.nav);
    }
    paint::arrow_button(game, batch, back, false, pages > 1, "Previous item page");
    paint::arrow_button(game, batch, forward, true, pages > 1, "Next item page");

    let unmatched = game.panel.stacks.is_empty();
    let search = game.browser_search(layout);
    search.suggestion = None;
    search.text_color = if unmatched && !search.value.is_empty() {
        rgba(255, 0, 0, 255)
    } else {
        paint::white()
    };
    let mut search = search.clone();
    search.draw_with_skin(batch, &game.gui.atlas, None, Some(paint::SEARCH_SKIN));
    game.panel.search = Some(search);
    paint::settings_button(game, batch, items.settings, "AEI settings");
}

fn bookmark_page(list: &BookmarkList) -> (usize, usize) {
    let pages = crate::game::aei_glue::bookmark_count()
        .div_ceil(list.per_page())
        .max(1);
    let page = BOOKMARK_PAGE.with(|cell| {
        let mut page = cell.borrow_mut();
        *page = (*page).min(pages - 1);
        *page
    });
    (page, pages)
}

/// The bookmark under the pointer: its index and stack.
fn hovered_bookmark(game: &Game<'_>, list: &BookmarkList) -> Option<(usize, BrowserStack)> {
    let (page, _) = bookmark_page(list);
    let first = page * list.per_page();
    let bookmarks = crate::game::aei_glue::bookmarks();
    (0..list.per_page())
        .find(|&offset| viewer::contains(list.slot(offset), game.cursor))
        .and_then(|offset| {
            let index = first + offset;
            Some((index, bookmarks.get(index)?.0.clone()))
        })
}

fn draw_bookmarks(game: &mut Game<'_>, batch: &mut HudBatch, list: &BookmarkList) {
    let (page, pages) = bookmark_page(list);
    let first = page * list.per_page();
    let bookmarks = crate::game::aei_glue::bookmarks();
    for (offset, (stack, recipe)) in bookmarks
        .iter()
        .skip(first)
        .take(list.per_page())
        .enumerate()
    {
        let rect = list.slot(offset);
        draw_stack(
            game,
            batch,
            stack,
            rect,
            &format!("Bookmark {}", first + offset),
            true,
        );
        if *recipe {
            paint::recipe_mark(game, batch, rect);
        }
    }
    let [back, forward] = list.arrows();
    let [x, y, w, h] = list.nav;
    paint::strip(game, batch, [x + back[2], y, w - back[2] - forward[2], h]);
    let label = format!("{}/{}", page + 1, pages);
    if game.gui.atlas.font.width(&label) <= w - back[2] - forward[2] {
        paint::title(game, batch, &label, list.nav);
    }
    paint::arrow_button(
        game,
        batch,
        back,
        false,
        pages > 1,
        "Previous bookmark page",
    );
    paint::arrow_button(game, batch, forward, true, pages > 1, "Next bookmark page");
}

fn turn_bookmark_page(list: &BookmarkList, forward: bool) {
    let (page, pages) = bookmark_page(list);
    BOOKMARK_PAGE.with(|cell| {
        *cell.borrow_mut() = if forward {
            (page + 1) % pages
        } else {
            (page + pages - 1) % pages
        };
    });
}

pub(super) fn draw(game: &mut Game<'_>, batch: &mut HudBatch) -> ControlFlow<()> {
    if owned(game).is_none() {
        return ControlFlow::Continue(());
    }
    let recipes = game.panel.recipes.is_some();
    let Some(screen) = screen(game, batch, recipes) else {
        return ControlFlow::Continue(());
    };
    let transfers = screen
        .view
        .as_ref()
        .map(|view| transfers(game, view))
        .unwrap_or_default();
    if let Some(view) = &screen.view {
        // The vanilla in-game screen background behind the recipe window.
        batch.fill_gradient(
            &game.gui.atlas,
            (0.0, 0.0, batch.width, batch.height),
            rgba(16, 16, 16, 0xC0),
            rgba(16, 16, 16, 0xD0),
        );
        draw_recipes(game, batch, view, &transfers);
    }
    if let Some((items, layout)) = &screen.items {
        draw_items(game, batch, items, layout);
    } else if let Some(search) = game.panel.search.as_mut() {
        search.focused = false;
    }
    if let Some(list) = &screen.bookmarks {
        draw_bookmarks(game, batch, list);
    }

    let cursor = game.cursor;
    // A category tab names its category; every other icon shows its item.
    let tab_title = screen.view.as_ref().and_then(|view| {
        let page = game.panel_shown()?.recipes.as_ref()?;
        let (i, _) = view
            .tabs
            .iter()
            .find(|(_, rect)| viewer::contains(*rect, cursor))?;
        Some(category_title(game, &page.tabs.get(*i)?.item))
    });
    let mut slot_lines = Vec::new();
    let hovered = screen
        .view
        .as_ref()
        .filter(|_| tab_title.is_none())
        .and_then(|view| {
            hovered_recipe(game, view)
                .map(|(card, slot, stack)| {
                    let output = game
                        .panel_shown()
                        .and_then(|shown| shown.recipes.as_ref()?.recipes.get(card))
                        .is_some_and(|recipe| slot == recipe.slots.len());
                    let advanced = game.settings.file.bool("advancedItemTooltips");
                    slot_lines = crate::game::aei_glue::slot_lines(card, slot, output, advanced);
                    stack
                })
                .or_else(|| hovered_station(game, view))
        })
        .or_else(|| {
            let (_, layout) = screen.items.as_ref()?;
            hovered_item(game, layout).map(|(_, stack)| stack)
        })
        .or_else(|| {
            let list = screen.bookmarks.as_ref()?;
            hovered_bookmark(game, list).map(|(_, stack)| stack)
        });
    let bookmark_tip = screen.view.as_ref().and_then(|view| {
        let (card, _) = view.cards.iter().find(|(i, card)| {
            viewer::contains(card.bookmark, cursor)
                && crate::game::aei_glue::recipe_bookmarkable(*i)
        })?;
        Some(if crate::game::aei_glue::recipe_bookmarked(*card) {
            "Remove bookmark"
        } else {
            "Bookmark this recipe"
        })
    });
    if game.carried.is_some() {
        game.draw_carried(batch);
    } else if let Some(title) = tab_title {
        game.draw_tooltip_lines(batch, &[(title, u32::MAX)], cursor.0, cursor.1);
    } else if let Some(tip) = bookmark_tip {
        game.draw_tooltip_lines(batch, &[(tip.to_owned(), u32::MAX)], cursor.0, cursor.1);
    } else if let Some((_, transfer)) = screen
        .view
        .as_ref()
        .and_then(|view| hovered_transfer(game, view, &transfers))
    {
        let line = (transfer.tooltip().to_owned(), u32::MAX);
        game.draw_tooltip_lines(batch, &[line], cursor.0, cursor.1);
    } else if let Some(stack) =
        hovered.and_then(|sent| super::browser_stack(world_items!(game), &sent))
    {
        let extra = slot_lines
            .into_iter()
            .map(|line| (horizon_client::text::Text::legacy(&line), u32::MAX))
            .collect();
        game.draw_stack_tooltip_with(batch, &stack, false, cursor.0, cursor.1, extra);
    } else if let Some((items, _)) = &screen.items
        && viewer::contains(items.settings, cursor)
    {
        let label = game.translate("options.title", "Options");
        let cheat = if crate::game::aei_glue::cheat_mode() {
            "\u{a7}cCheat mode is on"
        } else {
            "\u{a7}7Ctrl+click: cheat mode"
        };
        let lines = [
            (horizon_client::text::Text::legacy(&label), u32::MAX),
            (horizon_client::text::Text::legacy(cheat), u32::MAX),
        ];
        game.draw_tooltip_lines(batch, &lines, cursor.0, cursor.1);
    } else if let Some(text) = screen
        .view
        .as_ref()
        .and_then(|view| shapeless_hovered(game, view))
    {
        game.draw_tooltip_lines(batch, &[(text, u32::MAX)], cursor.0, cursor.1);
    } else if screen.view.is_none() {
        game.draw_slot_tooltip(batch);
    }
    ControlFlow::Break(())
}

fn shapeless_hovered(game: &Game<'_>, view: &Viewer) -> Option<String> {
    let page = game.panel_shown()?.recipes.as_ref()?;
    view.cards
        .iter()
        .any(|(i, card)| {
            page.recipes[*i].note == SHAPELESS_NOTE && viewer::contains(card.shapeless, game.cursor)
        })
        .then(|| "Shapeless Recipe".to_owned())
}

fn turn_page(view: &Viewer, forward: bool) {
    NAVIGATION.with(|cell| {
        if let Some(state) = cell.borrow_mut().as_mut() {
            state.page = if forward {
                (view.page + 1) % view.pages
            } else {
                (view.page + view.pages - 1) % view.pages
            };
        }
    });
}

/// A press on the item list; true when it hit something there.
fn press_items(
    game: &mut Game<'_>,
    owner: &Owner,
    items: &Sidebar,
    layout: &Layout,
    right: bool,
) -> bool {
    if viewer::contains(items.settings, game.cursor) {
        if !right && game.has_control_down() {
            crate::game::aei_glue::toggle_cheat(game);
            game.audio
                .play("minecraft:ui.button.click", None, 0.25, 1.0);
        } else if !right {
            game.list_native_mods();
            if let Some(index) = game
                .settings
                .mods
                .installed
                .iter()
                .position(|entry| entry.id == crate::game::aei::ID && entry.native.is_some())
            {
                game.mod_browser_closed();
                game.panel.recipes_settings_return = Some(game.screen);
                game.open_options_at(false, horizon_client::options_ui::Page::NativeMod(index));
            }
        }
        return true;
    }
    if inside(game.cursor, items.search) {
        let shift = game.has_shift_down();
        let cursor = game.cursor;
        let mut search = game.browser_search(layout).clone();
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
        return true;
    }
    let pages = game.browser_pages(layout);
    if let Some(i) = items
        .arrows()
        .iter()
        .position(|rect| viewer::contains(*rect, game.cursor))
    {
        if pages > 1 {
            game.panel.page = if i == 1 {
                (game.panel.page + 1) % pages
            } else {
                (game.panel.page + pages - 1) % pages
            };
            game.audio
                .play("minecraft:ui.button.click", None, 0.25, 1.0);
        }
        return true;
    }
    if let Some((index, _)) = hovered_item(game, layout) {
        let event = BrowserEvent::Clicked {
            index: index as u32,
            right,
        };
        game.panel_event(owner, event);
        return true;
    }
    false
}

fn press_recipes(game: &mut Game<'_>, owner: &Owner, view: &Viewer, right: bool) {
    let Some(page) = game.panel_shown().and_then(|shown| shown.recipes.as_ref()) else {
        return;
    };
    let (tabs, tab) = (page.tabs.len(), page.tab as usize);
    if let Some((index, _)) = view
        .tabs
        .iter()
        .find(|(_, rect)| viewer::contains(*rect, game.cursor))
    {
        game.panel_event(owner, BrowserEvent::Tab(*index as u32));
        game.audio
            .play("minecraft:ui.button.click", None, 0.25, 1.0);
    } else if let Some(i) = view
        .category_arrows
        .iter()
        .position(|rect| viewer::contains(*rect, game.cursor))
    {
        if tabs > 1 {
            let tab = if i == 1 {
                (tab + 1) % tabs
            } else {
                (tab + tabs - 1) % tabs
            };
            game.panel_event(owner, BrowserEvent::Tab(tab as u32));
        }
    } else if let Some(stack) = hovered_catalyst(game, view) {
        // JEI: a catalyst's recipes (left) or uses (right).
        crate::game::aei_glue::lookup(game, &stack, right);
    } else if viewer::contains(view.category, game.cursor) {
        crate::game::aei_glue::show_category(game);
    } else if let Some(i) = view
        .page_arrows
        .iter()
        .position(|rect| viewer::contains(*rect, game.cursor))
    {
        if view.pages > 1 {
            turn_page(view, i == 1);
        }
    } else if let Some((index, _)) = view.cards.iter().find(|(i, card)| {
        viewer::contains(card.bookmark, game.cursor)
            && crate::game::aei_glue::recipe_bookmarkable(*i)
    }) {
        let event = crate::game::aei::Event::BookmarkRecipe(*index);
        crate::game::aei_glue::bookmark_event(game, event);
        game.audio
            .play("minecraft:ui.button.click", None, 0.25, 1.0);
    } else if let Some((recipe, slot, stack)) = hovered_recipe(game, view) {
        let event = BrowserEvent::SlotClicked {
            recipe: recipe as u32,
            slot: slot as u32,
            stack,
            right,
        };
        game.panel_event(owner, event);
    } else {
        let transfers = transfers(game, view);
        if let Some((index, transfer)) = hovered_transfer(game, view, &transfers)
            && transfer != Transfer::Locked
        {
            game.panel_event(owner, BrowserEvent::Action(index as u32));
        }
    }
}

pub(super) fn press(game: &mut Game<'_>, right: bool) -> ControlFlow<bool> {
    let Some(owner) = owned(game) else {
        return ControlFlow::Continue(());
    };
    let Some(batch) = game.browser_batch() else {
        return ControlFlow::Continue(());
    };
    let recipes = game.panel.recipes.is_some();
    let Some(screen) = screen(game, &batch, recipes) else {
        return ControlFlow::Continue(());
    };
    if let Some(search) = game.panel.search.as_mut() {
        search.focused = false;
    }
    if let Some((items, layout)) = &screen.items
        && (inside(game.cursor, items.area)
            || inside(game.cursor, items.search)
            || inside(game.cursor, items.settings))
    {
        // JEI deletes a stack dropped on the list; on the creative screen the cursor's
        // stack is the client's own, so it can simply go.
        if game.carried.is_some() && game.screen == crate::game::Screen::Creative {
            game.carried = None;
            return ControlFlow::Break(true);
        }
        press_items(game, &owner, items, layout, right);
        return ControlFlow::Break(true);
    }
    if let Some(list) = &screen.bookmarks
        && inside(game.cursor, list.area)
    {
        if let Some(i) = list
            .arrows()
            .iter()
            .position(|rect| viewer::contains(*rect, game.cursor))
        {
            turn_bookmark_page(list, i == 1);
            game.audio
                .play("minecraft:ui.button.click", None, 0.25, 1.0);
            return ControlFlow::Break(true);
        }
        if let Some((index, _)) = hovered_bookmark(game, list) {
            let event = crate::game::aei::Event::ClickBookmark { index, right };
            crate::game::aei_glue::bookmark_event(game, event);
            return ControlFlow::Break(true);
        }
    }
    match &screen.view {
        // The recipe window is modal: every other click is its own.
        Some(view) => {
            press_recipes(game, &owner, view, right);
            ControlFlow::Break(true)
        }
        // Over the catalogue, clicks off the list reach the container.
        None => ControlFlow::Break(false),
    }
}

pub(super) fn pointed(game: &mut Game<'_>) -> ControlFlow<Option<BrowserStack>> {
    if owned(game).is_none() {
        return ControlFlow::Continue(());
    }
    let Some(batch) = game.browser_batch() else {
        return ControlFlow::Continue(());
    };
    let recipes = game.panel.recipes.is_some();
    let Some(screen) = screen(game, &batch, recipes) else {
        return ControlFlow::Continue(());
    };
    if let Some((items, layout)) = &screen.items
        && inside(game.cursor, items.area)
    {
        return ControlFlow::Break(hovered_item(game, layout).map(|(_, stack)| stack));
    }
    if let Some(list) = &screen.bookmarks
        && let Some((_, stack)) = hovered_bookmark(game, list)
    {
        return ControlFlow::Break(Some(stack));
    }
    match &screen.view {
        Some(view) => ControlFlow::Break(
            hovered_recipe(game, view)
                .map(|(_, _, stack)| stack)
                .or_else(|| hovered_station(game, view)),
        ),
        // Over the container itself, Horizon finds the slot's stack.
        None => ControlFlow::Continue(()),
    }
}

fn scroll(game: &Game<'_>, amount: f64) -> bool {
    if !matches!(
        game.screen,
        crate::game::Screen::Inventory | crate::game::Screen::Creative
    ) || game.title.is_some()
        || game
            .panel_shown()
            .is_none_or(|shown| shown.owner.native_id() != crate::game::aei::ID)
    {
        return false;
    }
    let Some(batch) = game.browser_batch() else {
        return false;
    };
    let Some(screen) = screen(game, &batch, game.panel.recipes.is_some()) else {
        return false;
    };
    if let Some(list) = &screen.bookmarks
        && inside(game.cursor, list.area)
        && (0..list.per_page()).any(|o| viewer::contains(list.slot(o), game.cursor))
    {
        if amount != 0.0 {
            turn_bookmark_page(list, amount < 0.0);
        }
        return true;
    }
    let over_items = screen
        .items
        .as_ref()
        .is_some_and(|(items, _)| inside(game.cursor, items.area));
    if over_items {
        if amount != 0.0 {
            ITEM_SCROLL.with(|cell| *cell.borrow_mut() += if amount < 0.0 { 1 } else { -1 });
        }
        return true;
    }
    let Some(view) = &screen.view else {
        return false;
    };
    if amount != 0.0 && view.pages > 1 {
        turn_page(view, amount < 0.0);
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

    pub(in crate::game) fn aei_reset_viewer(&self) {
        NAVIGATION.with(|cell| *cell.borrow_mut() = None);
        ITEM_SCROLL.with(|cell| *cell.borrow_mut() = 0);
    }

    /// Whether the recipe window covers the container this frame.
    pub(in crate::game) fn aei_recipe_modal(&mut self, batch: &HudBatch) -> bool {
        active(self).is_some() && screen(self, batch, true).is_some_and(|s| s.view.is_some())
    }

    pub(in crate::game) fn aei_recipe_is_modal(&mut self) -> bool {
        self.browser_batch()
            .is_some_and(|batch| self.aei_recipe_modal(&batch))
    }

    pub(in crate::game) fn aei_focus_search(&mut self) -> bool {
        if owned(self).is_none() {
            return false;
        }
        let Some(batch) = self.browser_batch() else {
            return false;
        };
        let recipes = self.panel.recipes.is_some();
        let Some((_, layout)) = screen(self, &batch, recipes).and_then(|s| s.items) else {
            return false;
        };
        let search = self.browser_search(&layout);
        search.focused = true;
        search.reset_blink();
        true
    }

    /// The pointer is over AEI's item or bookmark list, whose clicks AEI takes: their
    /// release must not reach the container (it would drop a carried stack).
    pub(in crate::game) fn aei_over_panels(&mut self) -> bool {
        if owned(self).is_none() {
            return false;
        }
        let Some(batch) = self.browser_batch() else {
            return false;
        };
        let recipes = self.panel.recipes.is_some();
        let Some(screen) = screen(self, &batch, recipes) else {
            return false;
        };
        let cursor = self.cursor;
        screen.items.as_ref().is_some_and(|(items, _)| {
            inside(cursor, items.area)
                || inside(cursor, items.search)
                || inside(cursor, items.settings)
        }) || screen.bookmarks.as_ref().is_some_and(|list| {
            list.arrows()
                .iter()
                .chain(
                    &(0..list.per_page())
                        .map(|o| list.slot(o))
                        .collect::<Vec<_>>(),
                )
                .any(|rect| viewer::contains(*rect, cursor))
        })
    }

    /// Cheat mode on the creative screen: `stack` on the cursor, as picking it from a
    /// creative tab does. False when something is already carried or the screen is not
    /// the creative one.
    pub(in crate::game) fn aei_pick_up(&mut self, stack: &BrowserStack) -> bool {
        if self.screen != crate::game::Screen::Creative || self.carried.is_some() {
            return false;
        }
        let Some(mut item) = super::browser_stack(world_items!(self), stack) else {
            return false;
        };
        item.count = stack.count;
        self.carried = Some(item);
        true
    }

    /// Shows a new item list for the current query as a search reply: an empty result
    /// keeps the panel and its field, so the player can still edit the query.
    pub(in crate::game) fn aei_show_search_result(&mut self, items: Vec<BrowserStack>) {
        self.panel.searching = true;
        self.native_panel_items(crate::game::aei::ID, items);
        self.panel.searching = false;
    }

    /// No panel is shown although AEI has a session (Horizon drops a panel that shows
    /// nothing outside a search).
    pub(in crate::game) fn aei_panel_missing(&self) -> bool {
        self.panel_shown().is_none()
    }

    pub(in crate::game) fn aei_reset_catalogue(&mut self) {
        self.aei_reset_viewer();
        self.panel.search = None;
        self.panel.sent.clear();
        self.panel.page = 0;
    }
}

// AEI's screens over a container: the item list on the right (always), and the recipe
// window with its category tabs and station tab when a lookup is open. Geometry comes from
// the engine's `viewer`; drawing from `aei_recipe_paint`.
use std::{cell::RefCell, ops::ControlFlow, time::Instant};

use super::aei_recipe_paint as paint;
use super::{BrowserEvent, BrowserStack, Game, Layout, Owner, inside};
use crate::game::aei::{
    NOTE_SEPARATOR, SHAPELESS_NOTE,
    viewer::{self, BookmarkList, Category, PlanPanel, Sidebar, Viewer},
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
    /// The card the narrator was told about: its words, since when it is under the
    /// pointer, and whether it was said.
    static SPOKEN: RefCell<Option<(String, std::time::Instant, bool)>> = const { RefCell::new(None) };
    /// A recipe image being taken (Ctrl+Shift+C over a card).
    static EXPORT: RefCell<Option<Export>> = const { RefCell::new(None) };
    /// Recipes pinned beside the screen (P over a card): card, station, recipe id.
    static PINNED: RefCell<Vec<(crate::game::aei::RecipeCard, &'static str, String)>> =
        const { RefCell::new(Vec::new()) };
    /// Keyboard focus in the item list (Tab): the focused entry's index.
    static KB_FOCUS: RefCell<Option<usize>> = const { RefCell::new(None) };
    /// The material calculator while it is open: its targets (a shopping list) with
    /// their counts, and the one the count buttons change.
    static PLAN: RefCell<Option<(Vec<(BrowserStack, u32)>, usize)>> = const { RefCell::new(None) };
    /// The item whose details panel is open (the I key).
    static DETAILS: RefCell<Option<BrowserStack>> = const { RefCell::new(None) };
    /// A note being typed in the search field: the item, and the search to restore.
    static NOTE_EDIT: RefCell<Option<(String, String)>> = const { RefCell::new(None) };
    /// The bookmark a left press went down on: its release clicks it, or moves it to
    /// the bookmark it is released over (JEI's drag to reorder).
    static BOOKMARK_DRAG: RefCell<Option<usize>> = const { RefCell::new(None) };
    /// A window of a long category to show (offset, length), asked for where the game
    /// can't be changed (page turns, the wheel) and shown on the next frame.
    static PENDING_WINDOW: RefCell<Option<(usize, usize)>> = const { RefCell::new(None) };
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
    /// The recently viewed row (an option), stack and cell each.
    recent: Vec<(BrowserStack, viewer::Rect)>,
    /// Saved searches (chips) under the bookmarks, and the chip that pins the search.
    chips: Vec<(Chip, viewer::Rect)>,
}

/// A recipe image being taken: the screenshot's file, the image's, the card (GUI
/// pixels), the GUI's width then and when it was asked for.
#[derive(Clone, Debug)]
struct Export {
    capture: std::path::PathBuf,
    out: std::path::PathBuf,
    area: viewer::Rect,
    gui_width: f32,
    since: std::time::Instant,
}

/// A saved-search chip.
#[derive(Clone, Debug, PartialEq)]
enum Chip {
    /// Saved search `i`, its text.
    Saved(usize, String),
    /// Pins the typed search.
    Pin(String),
    /// The items new to the player (`is:new`), how many.
    New(usize),
}

impl Chip {
    fn label(&self) -> String {
        match self {
            Chip::Saved(_, query) => query.clone(),
            Chip::Pin(_) => "+ Pin search".to_owned(),
            Chip::New(n) => format!("New ({n})"),
        }
    }
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
    let recent_stacks = if crate::game::aei_glue::recent_enabled() {
        crate::game::aei_glue::recent()
    } else {
        Vec::new()
    };
    let cells = viewer::recent_row(size, left, recent_stacks.len());
    let reserved = if cells.is_empty() {
        0.0
    } else {
        viewer::SLOT + 4.0
    };
    let recent: Vec<_> = recent_stacks.into_iter().zip(cells).collect();
    // Saved searches, and a chip that pins the typed one.
    let saved = crate::game::aei_glue::chips();
    let fresh = crate::game::aei_glue::fresh_count();
    // The "new" items first, like a tab of their own.
    let mut chips: Vec<Chip> = (fresh > 0)
        .then_some(Chip::New(fresh))
        .into_iter()
        .collect();
    chips.extend(
        saved
            .iter()
            .enumerate()
            .map(|(i, q)| Chip::Saved(i, q.clone())),
    );
    let typed = game
        .panel
        .search
        .as_ref()
        .map(|s| s.value.trim().to_owned())
        .unwrap_or_default();
    if !typed.is_empty()
        && !game.aei_editing_note()
        && !saved.contains(&typed)
        && saved.len() < crate::game::aei_glue::CHIPS
    {
        chips.push(Chip::Pin(typed));
    }
    let font = &game.gui.atlas.font;
    let widths: Vec<f32> = chips
        .iter()
        .map(|chip| (font.width(&chip.label()) + 6.0).min(left))
        .collect();
    let (rects, used) = viewer::chip_rows(left, size[1] - 6.0 - reserved, &widths);
    let reserved = reserved + used;
    let chips: Vec<_> = chips
        .into_iter()
        .zip(rects)
        .filter_map(|(chip, rect)| Some((chip, rect?)))
        .collect();
    // Another collection than the first shows even while empty, to switch back.
    let mut count = crate::game::aei_glue::bookmark_count();
    if crate::game::aei_glue::bookmark_list() != 0 {
        count = count.max(1);
    }
    let bookmarks = BookmarkList::above(size, left, count, reserved);
    // Without a recipe window, AEI only takes over when its list fits.
    (view.is_some() || items.is_some()).then_some(Screen {
        view,
        items,
        bookmarks,
        recent,
        chips,
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
        .filter(|stack| {
            crate::game::aei_glue::fluid(&stack.item).is_some()
                || super::browser_stack(world_items!(game), stack).is_some()
        })
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

/// The item list's `n`th cell on a page: 18 px, or 16 in the compact list.
fn list_cell(layout: &Layout, n: usize) -> viewer::Rect {
    let cell = crate::game::aei_glue::list_options().cell;
    let columns = layout.columns.max(1);
    [
        layout.grid.0 + (n % columns) as f32 * cell,
        layout.grid.1 + (n / columns) as f32 * cell,
        cell,
        cell,
    ]
}

fn hovered_item(game: &Game<'_>, layout: &Layout) -> Option<(usize, BrowserStack)> {
    let first = game.panel.page * layout.per_page();
    for offset in 0..layout.per_page() {
        if inside(game.cursor, list_cell(layout, offset)) {
            let index = first + offset;
            let sent = game.panel_shown()?.items.get(index)?.clone();
            // The game resolves items only; a fluid entry is AEI's own.
            if crate::game::aei_glue::fluid(&sent.item).is_none() {
                game.panel.stacks.get(index)?.as_ref()?;
            }
            return Some((index, sent));
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

/// The breadcrumbs drawn: (trail index, stack, uses, rect), the last [`viewer::TRAIL`]
/// lookups; none with fewer than two.
fn breadcrumbs(
    view: &Viewer,
) -> (
    Vec<(usize, BrowserStack, bool, viewer::Rect)>,
    Option<usize>,
) {
    let (trail, current) = crate::game::aei_glue::trail();
    if trail.len() < 2 {
        return (Vec::new(), current);
    }
    let first = trail.len().saturating_sub(viewer::TRAIL);
    let cells = view.trail(trail.len() - first);
    let crumbs = trail
        .into_iter()
        .enumerate()
        .skip(first)
        .zip(cells)
        .map(|((i, (stack, uses)), rect)| (i, stack, uses, rect))
        .collect();
    (crumbs, current)
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
    let [x, y, w, h] = rect;
    let item = (
        (x + (w - 16.0) / 2.0).floor(),
        (y + (h - 16.0) / 2.0).floor(),
    );
    let fluid = crate::game::aei_glue::fluid(&sent.item);
    if let Some(i) = fluid {
        // A fluid: its still texture, from AEI's own HUD image.
        game.native_sprite(
            batch,
            horizon_client::hud::MOD_IMAGE_SPRITE,
            crate::game::aei_glue::fluid_sprite(i),
            [item.0, item.1, 16.0, 16.0],
        );
    } else {
        let Some(stack) = super::browser_stack(world_items!(game), sent) else {
            return;
        };
        game.draw_item(batch, &stack, item.0, item.1);
    }
    if highlight && viewer::contains(rect, game.cursor) {
        paint::highlight(game, batch, item);
    }
    // "Item 3: minecraft:stone x1": the control socket's id stops at the
    // first ':' (`item-3`), so automation keeps stable ids and reads the stack.
    let label = format!("{name}: {} x{}", sent.item, sent.count);
    horizon_client::widgets::record(&label, rect, true);
    // The narrator reads the item's name (and count), as it reads buttons.
    horizon_client::widgets::narratable(|| {
        let mut title = match fluid {
            Some(i) => crate::game::aei_glue::fluid_name(game, i),
            None => game.pack.names.display_item(&sent.item).to_string(),
        };
        if sent.count > 1 {
            title.push_str(&format!(", {}", sent.count));
        }
        horizon_client::widgets::Narratable::new(
            horizon_client::widgets::What::Named {
                title,
                active: true,
            },
            rect,
        )
    });
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
        "minecraft:coal" => "Fuel".into(),
        "minecraft:composter" => "Compostable".into(),
        "minecraft:anvil" => "Anvil".into(),
        "minecraft:emerald" => "Trading".into(),
        "minecraft:iron_pickaxe" => "Block Drops".into(),
        "minecraft:bucket" => "Fluids".into(),
        "minecraft:iron_sword" => "Mob Drops".into(),
        "minecraft:chest" => "Loot".into(),
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

/// Says card `card` with the narrator after the pointer rests on it for half a second,
/// once until it leaves (`say_system_now` stays quiet while the narrator is off).
fn speak_card(card: usize) {
    let Some(speech) = crate::game::aei_glue::recipe_speech(card) else {
        return;
    };
    SPOKEN.with(|cell| {
        let mut state = cell.borrow_mut();
        let now = std::time::Instant::now();
        match state.as_mut() {
            Some((said, since, done)) if *said == speech => {
                if !*done && now.duration_since(*since).as_millis() >= 500 {
                    horizon_narrator::say_system_now(&speech);
                    *done = true;
                }
            }
            _ => *state = Some((speech, now, false)),
        }
    });
}

/// A recipe preview: one card left of the pointer (right of it when there is no
/// room), drawn as the recipe window draws its cards, without animations.
fn draw_preview(
    game: &mut Game<'_>,
    batch: &mut HudBatch,
    card: &crate::game::aei::RecipeCard,
    station: &str,
) {
    let (w, h) = card_size(card, station);
    let (cx, cy) = game.cursor;
    let x = if cx - w - 8.0 >= 0.0 {
        cx - w - 8.0
    } else {
        cx + 12.0
    };
    let y = (cy - h / 2.0).clamp(0.0, (batch.height - h).max(0.0));
    draw_card_at(game, batch, card, station, x, y, "Preview");
}

/// A recipe card's plate size.
fn card_size(card: &crate::game::aei::RecipeCard, station: &str) -> (f32, f32) {
    let (cw, ch) = Category::of(card.layout, station).size();
    (cw + 8.0, ch + 8.0)
}

/// The pinned recipes' plates, stacked up from the bottom left of the screen.
fn pinned_rects(batch: &HudBatch) -> Vec<viewer::Rect> {
    PINNED.with(|cell| {
        let mut y = batch.height - 6.0;
        cell.borrow()
            .iter()
            .map(|(card, station, _)| {
                let (w, h) = card_size(card, station);
                y -= h + 2.0;
                [6.0, y, w, h]
            })
            .collect()
    })
}

/// Pins the card under the pointer, or unpins the pinned card under it (P).
fn toggle_pin(game: &mut Game<'_>, batch: &HudBatch) -> bool {
    let cursor = game.cursor;
    if let Some(i) = pinned_rects(batch)
        .iter()
        .position(|rect| viewer::contains(*rect, cursor))
    {
        PINNED.with(|cell| cell.borrow_mut().remove(i));
        save_pinned(game);
        return true;
    }
    let Some(card) = game.aei_hovered_card() else {
        return false;
    };
    let Some(pin) = crate::game::aei_glue::shown_card(card) else {
        return false;
    };
    PINNED.with(|cell| {
        let mut pinned = cell.borrow_mut();
        if !pinned.iter().any(|(_, _, id)| *id == pin.2) {
            pinned.push(pin);
            // Two at most: the oldest goes.
            if pinned.len() > 2 {
                pinned.remove(0);
            }
        }
    });
    save_pinned(game);
    true
}

/// Saves the pinned recipes, so they come back in the next session.
fn save_pinned(game: &mut Game<'_>) {
    let ids: Vec<String> =
        PINNED.with(|cell| cell.borrow().iter().map(|(_, _, id)| id.clone()).collect());
    crate::game::aei_glue::save_pinned(game, &ids);
}

fn draw_pinned(game: &mut Game<'_>, batch: &mut HudBatch) {
    let pinned = PINNED.with(|cell| cell.borrow().clone());
    for ((card, station, _), rect) in pinned.iter().zip(pinned_rects(batch)) {
        draw_card_at(game, batch, card, station, rect[0], rect[1], "Pinned");
    }
}

/// Draws a recipe card at `x`, `y` as the recipe window draws its cards, without
/// animations; its stacks are recorded under `name`.
fn draw_card_at(
    game: &mut Game<'_>,
    batch: &mut HudBatch,
    card: &crate::game::aei::RecipeCard,
    station: &str,
    x: f32,
    y: f32,
    name: &str,
) {
    let category = Category::of(card.layout, station);
    let geometry = viewer::Card::new(category, card.width, card.height, card.inputs.len(), x, y);
    paint::plate(game, batch, geometry.area);
    for rect in &geometry.empty {
        paint::slot(game, batch, *rect);
    }
    if geometry.arrow[2] > 0.0 {
        paint::arrow(game, batch, geometry.arrow);
    }
    let native = |s: &crate::game::aei::Stack| BrowserStack {
        item: s.id.clone(),
        count: s.count,
        components: s.components.clone(),
    };
    for (slot, rect) in &geometry.slots {
        paint::slot(game, batch, *rect);
        let stack = if *slot == card.inputs.len() {
            card.outputs.first()
        } else {
            card.inputs
                .get(*slot)
                .and_then(|alternatives| alternatives.first())
        };
        if let Some(stack) = stack {
            draw_stack(game, batch, &native(stack), *rect, name, false);
        }
    }
}

/// A fuel or compostable card: the burning flame (fuel) and its two grey note lines.
fn draw_note_card(game: &Game<'_>, batch: &mut HudBatch, card: &viewer::Card, note: &str) {
    let atlas = &game.gui.atlas;
    if let Some([fx, fy, fw, fh]) = card.flame {
        let left = 1.0 - (seconds() * 20.0 % FLAME_TICKS) / FLAME_TICKS;
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
    let (top, bottom) = note.split_once(NOTE_SEPARATOR).unwrap_or(("", note));
    for (text, [x, y, w, _]) in [(top, card.top_note), (bottom, card.bottom_note)] {
        if !text.is_empty() && w > 0.0 {
            batch.text(atlas, text, x, y, paint::grey_text(), false);
        }
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
            // The recipe book places crafting and furnace-family recipes.
            let book = matches!(
                recipe.layout,
                NativeRecipeLayout::Grid | NativeRecipeLayout::Cooking
            );
            let id = ids.get(*index).copied().filter(|_| book).unwrap_or(-1);
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
        &{
            let (page, pages) = global_page(view);
            format!("{}/{}", page + 1, pages)
        },
        view.page_label,
    );
    let names = [
        ["Previous category", "Next category"],
        ["Previous recipe page", "Next recipe page"],
    ];
    let enabled = [page.tabs.len() > 1, global_page(view).1 > 1];
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
            // Fuel and compostables: no arrow, their note says what they do.
            _ if card.arrow[2] == 0.0 => draw_note_card(game, batch, card, &recipe.note),
            _ => {
                paint::arrow(game, batch, card.arrow);
                // The anvil's level cost under its inputs (green, red when too expensive).
                if recipe.layout != NativeRecipeLayout::Grid
                    && !recipe.note.is_empty()
                    && card.bottom_note[2] > 0.0
                {
                    let color = if recipe.note.starts_with("Too") {
                        rgba(255, 96, 96, 255)
                    } else {
                        rgba(128, 255, 32, 255)
                    };
                    let [x, y, w, _] = card.bottom_note;
                    let font = &game.gui.atlas.font;
                    let tx = (x + (w - font.width(&recipe.note)) / 2.0).round();
                    batch.text(&game.gui.atlas, &recipe.note, tx, y, color, true);
                }
            }
        }
        if recipe.note == SHAPELESS_NOTE {
            paint::shapeless(game, batch, card.shapeless);
        }
        // The narrator reads the whole card (what it takes and what it makes) when
        // the pointer is over it, not only its slots.
        horizon_client::widgets::narratable(|| {
            let title = crate::game::aei_glue::recipe_speech(*index).unwrap_or_default();
            horizon_client::widgets::Narratable::new(
                horizon_client::widgets::What::Named {
                    title,
                    active: true,
                },
                card.area,
            )
        });
        // The narrator reads the card the pointer rests on, once.
        if viewer::contains(card.area, game.cursor) {
            speak_card(*index);
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
    // The lookup trail above the tabs; the shown lookup is framed in gold.
    let (crumbs, current) = breadcrumbs(view);
    for (i, stack, _, rect) in &crumbs {
        paint::slot(game, batch, *rect);
        draw_stack(game, batch, stack, *rect, &format!("Breadcrumb {i}"), true);
        if Some(*i) == current {
            let [x, y, w, h] = *rect;
            let gold = rgba(255, 200, 40, 255);
            paint::fill(game, batch, [x, y, w, 1.0], gold);
            paint::fill(game, batch, [x, y + h - 1.0, w, 1.0], gold);
            paint::fill(game, batch, [x, y, 1.0, h], gold);
            paint::fill(game, batch, [x + w - 1.0, y, 1.0, h], gold);
        }
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
        let [x, y, cell, _] = list_cell(layout, 0);
        let w = layout.columns as f32 * cell;
        let h = layout.rows as f32 * cell;
        paint::fill(game, batch, [x, y, w, h], rgba(255, 0, 0, 0x30));
    }
    for offset in 0..layout.per_page() {
        let index = first + offset;
        let Some(stack) = game.panel_shown().and_then(|shown| shown.items.get(index)) else {
            break;
        };
        let rect = list_cell(layout, offset);
        draw_stack(game, batch, stack, rect, &format!("Item {index}"), true);
        if KB_FOCUS.with(|cell| *cell.borrow()) == Some(index) {
            let [x, y, w, h] = rect;
            let gold = rgba(255, 200, 40, 255);
            paint::fill(game, batch, [x, y, w, 1.0], gold);
            paint::fill(game, batch, [x, y + h - 1.0, w, 1.0], gold);
            paint::fill(game, batch, [x, y, 1.0, h], gold);
            paint::fill(game, batch, [x + w - 1.0, y, 1.0, h], gold);
        }
        if let Some((_, open)) = crate::game::aei_glue::shown_group(index) {
            // A group of variants: "+" folded, "-" unfolded, in the top left corner.
            let [x, y, _, _] = rect;
            let mark = if open { "-" } else { "+" };
            batch.text(
                &game.gui.atlas,
                mark,
                x + 1.0,
                y,
                rgba(255, 255, 160, 255),
                true,
            );
        }
        if crate::game::aei_glue::shown_fresh(index) {
            // Newly found: a small gold dot in the corner.
            let [x, y, w, _] = rect;
            paint::fill(
                game,
                batch,
                [x + w - 5.0, y + 2.0, 3.0, 3.0],
                rgba(255, 200, 40, 255),
            );
        }
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

    if let Some((item, _)) = NOTE_EDIT.with(|cell| cell.borrow().clone()) {
        let [x, y, _, _] = items.search;
        let hint = match item.strip_prefix("aei:list/") {
            Some(list) => format!(
                "Name of bookmark list {}: Enter saves, Esc cancels",
                list.parse::<usize>().unwrap_or(0) + 1
            ),
            None => format!(
                "Note on {}: Enter saves, Esc cancels",
                game.pack.names.display_item(&item)
            ),
        };
        batch.text(
            &game.gui.atlas,
            &hint,
            x,
            y - 10.0,
            rgba(255, 255, 160, 255),
            true,
        );
    }
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
    // A dragged bookmark follows the pointer once it leaves its cell.
    if let Some(from) = BOOKMARK_DRAG.with(|cell| *cell.borrow())
        && let Some((stack, _)) = bookmarks.get(from)
        && hovered_bookmark(game, list).is_none_or(|(i, _)| i != from)
        && let Some(item) = super::browser_stack(world_items!(game), stack)
    {
        let (cx, cy) = game.cursor;
        game.draw_item(batch, &item, cx - 8.0, cy - 8.0);
    }
    let [back, forward] = list.arrows();
    let [x, y, w, h] = list.nav;
    paint::strip(game, batch, [x + back[2], y, w - back[2] - forward[2], h]);
    // Which collection (bookmark list) shows; clicking the label switches it.
    let list_index = crate::game::aei_glue::bookmark_list();
    let mut label = format!(
        "{} \u{b7} {}/{}",
        crate::game::aei_glue::list_name(game, list_index),
        page + 1,
        pages
    );
    if game.gui.atlas.font.width(&label) > w - back[2] - forward[2] {
        label = format!("L{} \u{b7} {}/{}", list_index + 1, page + 1, pages);
    }
    horizon_client::widgets::record(
        "Switch bookmark list",
        [x + back[2], y, w - back[2] - forward[2], h],
        true,
    );
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

/// The open details panel: the item, its details, the window and each tag's chip.
struct DetailsView {
    stack: BrowserStack,
    details: crate::game::aei::Details,
    area: viewer::Rect,
    tags: Vec<(String, viewer::Rect)>,
    /// The "Edit note" button.
    note: viewer::Rect,
}

impl DetailsView {
    fn new(game: &Game<'_>, size: [f32; 2]) -> Option<Self> {
        let stack = DETAILS.with(|cell| cell.borrow().clone())?;
        let details = crate::game::aei_glue::details(&stack)?;
        let area = PlanPanel::new(size, [0; 4])?.area;
        let [x, y, w, h] = area;
        // Tag chips flow under the lines, wrapping at the window's edge.
        let font = &game.gui.atlas.font;
        let left = x + 6.0;
        let mut cx = left;
        let mut cy = y + 34.0 + details.lines.len() as f32 * 11.0 + 14.0;
        let mut tags = Vec::new();
        for tag in &details.tags {
            let width = font.width(&format!("${tag}")) + 6.0;
            if cx + width > x + w - 6.0 && cx > left {
                cx = left;
                cy += 13.0;
            }
            if cy + 12.0 > y + h - 6.0 {
                break;
            }
            tags.push((tag.clone(), [cx, cy, width, 12.0]));
            cx += width + 3.0;
        }
        let note = [
            x + w - 6.0 - 60.0,
            y + h - 6.0 - viewer::BUTTON,
            60.0,
            viewer::BUTTON,
        ];
        Some(Self {
            stack,
            details,
            area,
            tags,
            note,
        })
    }
}

fn draw_details(game: &mut Game<'_>, batch: &mut HudBatch, view: &DetailsView) {
    let [x, y, w, _] = view.area;
    paint::window(game, batch, view.area);
    let icon = [x + 6.0, y + 6.0, viewer::SLOT, viewer::SLOT];
    paint::slot(game, batch, icon);
    draw_stack(game, batch, &view.stack, icon, "Details item", false);
    let atlas = &game.gui.atlas;
    // Dark text on the light panel, as vanilla container titles are.
    let label_colour = paint::label_colour();
    let text_colour = paint::text_colour();
    batch.text(
        atlas,
        &view.details.name,
        x + 28.0,
        y + 11.0,
        text_colour,
        false,
    );
    let font = &atlas.font;
    let mut ly = y + 30.0;
    for (label, value) in &view.details.lines {
        let label = format!("{label}: ");
        batch.text(atlas, &label, x + 6.0, ly, label_colour, false);
        // Long values (components) are cut at the window's edge.
        let room = w - 12.0 - font.width(&label);
        let mut shown = value.clone();
        while !shown.is_empty() && font.width(&shown) > room {
            shown.pop();
        }
        if shown.len() < value.len() && shown.len() > 1 {
            shown.pop();
            shown.push('\u{2026}');
        }
        batch.text(
            atlas,
            &shown,
            x + 6.0 + font.width(&label),
            ly,
            text_colour,
            false,
        );
        ly += 11.0;
    }
    if !view.tags.is_empty() {
        let text = "Tags (click: their items; right-click: recipes taking any):";
        batch.text(atlas, text, x + 6.0, ly + 3.0, label_colour, false);
    }
    paint::action_button(game, batch, view.note, "Edit note", true, "Edit note");
    for (tag, rect) in &view.tags {
        let hovered = viewer::contains(*rect, game.cursor);
        let back = if hovered {
            rgba(80, 80, 140, 255)
        } else {
            rgba(60, 60, 60, 255)
        };
        paint::fill(game, batch, *rect, back);
        let [tx, ty, _, _] = *rect;
        batch.text(
            &game.gui.atlas,
            &format!("${tag}"),
            tx + 3.0,
            ty + 2.0,
            text_colour,
            false,
        );
        horizon_client::widgets::record(&format!("Tag {tag}"), *rect, true);
    }
}

/// The open calculator's targets and focused target.
fn plan_state() -> Option<(Vec<(BrowserStack, u32)>, usize)> {
    PLAN.with(|cell| cell.borrow().clone())
}

/// What the calculator shows: the plan of all its targets and its panel.
struct PlanView {
    targets: Vec<(BrowserStack, u32)>,
    focus: usize,
    plan: crate::game::aei::Plan,
    panel: PlanPanel,
    /// Assisted crafting (an option): the button and the step it would place.
    next: Option<(viewer::Rect, Option<i32>)>,
}

impl PlanView {
    fn new(game: &Game<'_>, size: [f32; 2]) -> Option<Self> {
        let (targets, focus) = plan_state()?;
        let plan = crate::game::aei_glue::plan(game, &targets)?;
        let counts = [
            targets.len(),
            plan.materials.len(),
            plan.steps.len(),
            plan.leftovers.len(),
        ];
        let panel = PlanPanel::new(size, counts)?;
        let next = crate::game::aei_glue::assist_enabled().then(|| {
            let [x, y, w, h] = panel.area;
            let rect = [
                x + w - 6.0 - 60.0,
                y + h - 6.0 - viewer::BUTTON,
                60.0,
                viewer::BUTTON,
            ];
            (rect, crate::game::aei_glue::next_step(&plan))
        });
        Some(Self {
            targets,
            focus,
            plan,
            panel,
            next,
        })
    }

    /// Every drawn cell: (section, index, stack with its count, rect). Section 0 holds
    /// the targets.
    fn cells(&self) -> Vec<(usize, usize, BrowserStack, viewer::Rect)> {
        let native = |id: &str, count: u32| BrowserStack {
            item: id.to_owned(),
            count,
            components: String::new(),
        };
        let stacks: [Vec<BrowserStack>; 4] = [
            self.targets
                .iter()
                .map(|(stack, count)| BrowserStack {
                    count: *count,
                    ..stack.clone()
                })
                .collect(),
            self.plan
                .materials
                .iter()
                .map(|m| native(&m.stack.id, m.need))
                .collect(),
            self.plan
                .steps
                .iter()
                .map(|s| native(&s.stack.id, s.stack.count))
                .collect(),
            self.plan
                .leftovers
                .iter()
                .map(|l| native(&l.id, l.count))
                .collect(),
        ];
        let mut cells = Vec::new();
        for (section, ((_, rects), stacks)) in self.panel.sections.iter().zip(stacks).enumerate() {
            for (index, (rect, stack)) in rects.iter().zip(stacks).enumerate() {
                cells.push((section, index, stack, *rect));
            }
        }
        cells
    }

    /// The tooltip lines a cell adds below its item's.
    fn lines(&self, section: usize, index: usize) -> Vec<String> {
        match section {
            0 => vec![
                "\u{a7}7Click: change its count".to_owned(),
                "\u{a7}7Right-click: remove it".to_owned(),
            ],
            1 => self.plan.materials.get(index).map_or_else(Vec::new, |m| {
                let colour = if m.have >= m.need { "a" } else { "c" };
                vec![format!(
                    "\u{a7}{colour}Need {} \u{b7} have {}",
                    m.need, m.have
                )]
            }),
            2 => self.plan.steps.get(index).map_or_else(Vec::new, |s| {
                vec![
                    format!(
                        "\u{a7}7{} \u{d7} {} \u{2192} {}",
                        s.crafts,
                        s.station.trim_start_matches("minecraft:").replace('_', " "),
                        s.stack.count
                    ),
                    "\u{a7}8Shift+click: another recipe".to_owned(),
                ]
            }),
            _ => vec!["\u{a7}7Left over".to_owned()],
        }
    }
}

fn draw_plan(game: &mut Game<'_>, batch: &mut HudBatch, view: &PlanView) {
    let panel = &view.panel;
    paint::window(game, batch, panel.area);
    let title = match view.targets.as_slice() {
        [(only, _)] => {
            let name = game.pack.names.display_item(&only.item).to_string();
            format!("Materials: {name}")
        }
        targets => format!("Shopping list: {} items", targets.len()),
    };
    paint::strip(game, batch, panel.title);
    paint::title(game, batch, &title, panel.title);
    for (rect, (label, step)) in panel.buttons.iter().zip([
        ("-64", "minus 64"),
        ("-1", "minus 1"),
        ("+1", "plus 1"),
        ("+64", "plus 64"),
    ]) {
        paint::text_button(game, batch, *rect, label, &format!("Plan count {step}"));
    }
    let count = view.targets.get(view.focus).map_or(1, |(_, n)| *n);
    paint::strip(game, batch, panel.count);
    paint::title(game, batch, &format!("\u{d7} {count}"), panel.count);
    let titles = [
        "Targets (T over an item adds one)",
        "Raw materials",
        "Crafting steps",
        "Left over",
    ];
    for ((label, _), title) in panel.sections.iter().zip(titles) {
        let [x, y, _, _] = *label;
        batch.text(&game.gui.atlas, title, x, y, paint::grey_text(), false);
    }
    if let Some((rect, step)) = view.next {
        paint::action_button(
            game,
            batch,
            rect,
            "Next step",
            step.is_some(),
            "Plan next step",
        );
    }
    let names = ["target", "material", "step", "leftover"];
    for (section, index, stack, rect) in view.cells() {
        paint::slot(game, batch, rect);
        let label = format!("Plan {} {index}", names[section]);
        // The item alone: its count, which can pass 99, is written below.
        let single = BrowserStack {
            count: 1,
            ..stack.clone()
        };
        draw_stack(game, batch, &single, rect, &label, true);
        horizon_client::widgets::record(
            &format!("{label}: {} x{}", stack.item, stack.count),
            rect,
            true,
        );
        let text = stack.count.to_string();
        let font = &game.gui.atlas.font;
        let [x, y, w, h] = rect;
        let tx = x + w - 1.0 - font.width(&text);
        batch.text(
            &game.gui.atlas,
            &text,
            tx,
            y + h - 9.0,
            paint::white(),
            true,
        );
        let short = section == 1
            && view
                .plan
                .materials
                .get(index)
                .is_some_and(|m| m.have < m.need);
        if short {
            paint::fill(game, batch, rect, rgba(255, 0, 0, 0x50));
        }
        if section == 0 && index == view.focus && view.targets.len() > 1 {
            let gold = rgba(255, 200, 40, 255);
            paint::fill(game, batch, [x, y, w, 1.0], gold);
            paint::fill(game, batch, [x, y + h - 1.0, w, 1.0], gold);
            paint::fill(game, batch, [x, y, 1.0, h], gold);
            paint::fill(game, batch, [x + w - 1.0, y, 1.0, h], gold);
        }
    }
}

/// A press on the open calculator: count buttons; a target to focus (right: remove);
/// any other cell's recipes (left) or uses (right), which closes it.
fn press_plan(game: &mut Game<'_>, view: &PlanView, right: bool) {
    let steps = [-64i64, -1, 1, 64];
    if let Some((rect, Some(book))) = view.next
        && viewer::contains(rect, game.cursor)
    {
        crate::game::aei_glue::place_step(game, book);
        game.audio
            .play("minecraft:ui.button.click", None, 0.25, 1.0);
    } else if let Some(i) = view
        .panel
        .buttons
        .iter()
        .position(|rect| viewer::contains(*rect, game.cursor))
    {
        change_plan_count(steps[i]);
        game.audio
            .play("minecraft:ui.button.click", None, 0.25, 1.0);
    } else if let Some((section, index, stack, _)) = view
        .cells()
        .into_iter()
        .find(|(.., rect)| viewer::contains(*rect, game.cursor))
    {
        if section == 0 {
            PLAN.with(|cell| {
                let mut state = cell.borrow_mut();
                if let Some((targets, focus)) = state.as_mut() {
                    if right {
                        targets.remove(index.min(targets.len() - 1));
                        *focus = (*focus).min(targets.len().saturating_sub(1));
                    } else {
                        *focus = index;
                    }
                }
                if state
                    .as_ref()
                    .is_some_and(|(targets, _)| targets.is_empty())
                {
                    *state = None;
                }
            });
        } else if section == 2 && !right && game.has_shift_down() {
            // Beyond the defaults: make this step with another recipe.
            crate::game::aei_glue::cycle_plan_recipe(game, &stack.item);
            game.audio
                .play("minecraft:ui.button.click", None, 0.25, 1.0);
        } else {
            PLAN.with(|cell| *cell.borrow_mut() = None);
            crate::game::aei_glue::lookup(game, &stack, right);
        }
    }
}

fn change_plan_count(step: i64) {
    PLAN.with(|cell| {
        if let Some((targets, focus)) = cell.borrow_mut().as_mut()
            && let Some((_, count)) = targets.get_mut(*focus)
        {
            *count = (i64::from(*count) + step).clamp(1, 9999) as u32;
        }
    });
}

pub(super) fn draw(game: &mut Game<'_>, batch: &mut HudBatch) -> ControlFlow<()> {
    if owned(game).is_none() {
        return ControlFlow::Continue(());
    }
    if let Some((offset, len)) = PENDING_WINDOW.with(|cell| cell.borrow_mut().take()) {
        crate::game::aei_glue::show_window(game, offset, len);
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
    let details = DetailsView::new(game, [batch.width, batch.height]);
    let plan = PlanView::new(game, [batch.width, batch.height]).filter(|_| details.is_none());
    if let Some(details) = &details {
        batch.fill_gradient(
            &game.gui.atlas,
            (0.0, 0.0, batch.width, batch.height),
            rgba(16, 16, 16, 0xC0),
            rgba(16, 16, 16, 0xD0),
        );
        draw_details(game, batch, details);
    } else if let Some(plan) = &plan {
        batch.fill_gradient(
            &game.gui.atlas,
            (0.0, 0.0, batch.width, batch.height),
            rgba(16, 16, 16, 0xC0),
            rgba(16, 16, 16, 0xD0),
        );
        draw_plan(game, batch, plan);
    } else if let Some(view) = &screen.view {
        // A long category's first window ends on a page boundary, so pages read on
        // across windows.
        let (offset, total) = crate::game::aei_glue::recipe_window();
        let shown = game
            .panel_shown()
            .and_then(|shown| shown.recipes.as_ref())
            .map_or(0, |page| page.recipes.len());
        let len = window_len(view.per_page);
        if total > shown && shown != len && offset + shown < total {
            PENDING_WINDOW.with(|cell| *cell.borrow_mut() = Some((offset, len)));
        }
        // The vanilla in-game screen background behind the recipe window.
        batch.fill_gradient(
            &game.gui.atlas,
            (0.0, 0.0, batch.width, batch.height),
            rgba(16, 16, 16, 0xC0),
            rgba(16, 16, 16, 0xD0),
        );
        draw_recipes(game, batch, view, &transfers);
    }
    // Beyond JEI (an option): a search dims the container's other items.
    if screen.view.is_none() && game.screen == crate::game::Screen::Inventory {
        let (ox, oy) = game.inventory_origin(batch);
        for slot in game.slots() {
            let Some(stack) = game.slot_view(slot) else {
                continue;
            };
            if crate::game::aei_glue::highlight(&stack.item) == Some(false) {
                let (sx, sy) = game.slot_position(slot);
                paint::fill(
                    game,
                    batch,
                    [ox + sx, oy + sy, 16.0, 16.0],
                    rgba(0, 0, 0, 0xA0),
                );
            }
        }
    }
    // On the creative screen: the player's slots (the inventory tab's, else the hotbar
    // row under the item tabs); the tabs' own items are a catalogue, left as they are.
    if screen.view.is_none() && game.screen == crate::game::Screen::Creative {
        use horizon_client::creative::Creative;
        let (ox, oy) = Game::creative_origin(batch);
        let slots: Vec<(usize, (f32, f32))> = if game.creative_inventory_open() {
            Creative::inventory_slots()
                .filter_map(|slot| Some((slot, Creative::inventory_slot_position(slot)?)))
                .collect()
        } else {
            (0..9)
                .filter_map(|slot| Some((slot, Creative::tab_slot_position(slot)?)))
                .collect()
        };
        for (slot, (sx, sy)) in slots {
            let Some(stack) = game.slot_view(slot) else {
                continue;
            };
            if crate::game::aei_glue::highlight(&stack.item) == Some(false) {
                paint::fill(
                    game,
                    batch,
                    [ox + sx, oy + sy, 16.0, 16.0],
                    rgba(0, 0, 0, 0xA0),
                );
            }
        }
    }
    if let Some((items, layout)) = &screen.items {
        draw_items(game, batch, items, layout);
    } else if let Some(search) = game.panel.search.as_mut() {
        search.focused = false;
    }
    if let Some(list) = &screen.bookmarks {
        draw_bookmarks(game, batch, list);
    }
    draw_pinned(game, batch);
    for (i, (stack, rect)) in screen.recent.iter().enumerate() {
        paint::slot(game, batch, *rect);
        draw_stack(game, batch, stack, *rect, &format!("Recent {i}"), true);
    }
    for (chip, rect) in &screen.chips {
        let hovered = viewer::contains(*rect, game.cursor);
        let back = match (chip, hovered) {
            (Chip::Pin(_), false) => rgba(40, 70, 40, 220),
            (Chip::Pin(_), true) => rgba(60, 110, 60, 240),
            (Chip::New(_), false) => rgba(110, 85, 20, 220),
            (Chip::New(_), true) => rgba(150, 120, 30, 240),
            (_, false) => rgba(50, 50, 70, 220),
            (_, true) => rgba(80, 80, 140, 240),
        };
        paint::fill(game, batch, *rect, back);
        let [x, y, w, _] = *rect;
        let mut label = chip.label();
        while label.chars().count() > 1 && game.gui.atlas.font.width(&label) > w - 6.0 {
            label.pop();
        }
        batch.text(&game.gui.atlas, &label, x + 3.0, y + 2.0, u32::MAX, false);
        let name = match chip {
            Chip::Saved(i, query) => format!("Search chip {i}: {query}"),
            Chip::Pin(_) => "Pin search".to_owned(),
            Chip::New(_) => "New items".to_owned(),
        };
        horizon_client::widgets::record(&name, *rect, true);
    }

    let cursor = game.cursor;
    // A category tab names its category; every other icon shows its item.
    let tab_title = screen
        .view
        .as_ref()
        .filter(|_| plan.is_none() && details.is_none())
        .and_then(|view| {
            let page = game.panel_shown()?.recipes.as_ref()?;
            let (i, _) = view
                .tabs
                .iter()
                .find(|(_, rect)| viewer::contains(*rect, cursor))?;
            let station = &page.tabs.get(*i)?.item;
            let title = category_title(game, station);
            // Beyond JEI: how much of the category's recipe book the player unlocked.
            Some(match crate::game::aei_glue::progress(station) {
                Some((unlocked, total)) => {
                    let mut text = format!("{title}\n\u{a7}7Unlocked: {unlocked}/{total}");
                    // What is still locked: the first few, then how many more
                    // (`is:locked` lists them all).
                    let missing = crate::game::aei_glue::missing(station);
                    for name in missing.iter().take(8) {
                        text.push_str(&format!("\n\u{a7}8- {name}"));
                    }
                    if missing.len() > 8 {
                        text.push_str(&format!(
                            "\n\u{a7}8... and {} more (search is:locked)",
                            missing.len() - 8
                        ));
                    }
                    text
                }
                None => title,
            })
        });
    let mut slot_lines = Vec::new();
    let plan_hovered = plan.as_ref().and_then(|plan| {
        let (section, index, stack, _) = plan
            .cells()
            .into_iter()
            .find(|(.., rect)| viewer::contains(*rect, cursor))?;
        Some((plan.lines(section, index), stack))
    });
    let plan_hovered = plan_hovered.map(|(lines, stack)| {
        slot_lines = lines;
        stack
    });
    let hovered = plan_hovered
        .or_else(|| {
            screen
                .view
                .as_ref()
                .filter(|_| tab_title.is_none() && plan.is_none() && details.is_none())
                .and_then(|view| {
                    let (crumbs, _) = breadcrumbs(view);
                    if let Some((_, stack, uses, _)) = crumbs
                        .into_iter()
                        .find(|(.., rect)| viewer::contains(*rect, cursor))
                    {
                        let what = if uses { "Uses" } else { "Recipes" };
                        slot_lines = vec![format!("\u{a7}7{what}")];
                        return Some(stack);
                    }
                    hovered_recipe(game, view)
                        .map(|(card, slot, stack)| {
                            let output = game
                                .panel_shown()
                                .and_then(|shown| shown.recipes.as_ref()?.recipes.get(card))
                                .is_some_and(|recipe| slot == recipe.slots.len());
                            let advanced = game.settings.file.bool("advancedItemTooltips");
                            slot_lines =
                                crate::game::aei_glue::slot_lines(card, slot, output, advanced);
                            stack
                        })
                        .or_else(|| hovered_station(game, view))
                })
        })
        .or_else(|| {
            let (_, layout) = screen.items.as_ref()?;
            let (index, stack) = hovered_item(game, layout)?;
            if let Some((size, open)) = crate::game::aei_glue::shown_group(index) {
                let action = if open { "fold" } else { "unfold" };
                slot_lines = vec![format!(
                    "\u{a7}7{size} variants \u{b7} Shift+click to {action}"
                )];
            }
            Some(stack)
        })
        .or_else(|| {
            let list = screen.bookmarks.as_ref()?;
            hovered_bookmark(game, list).map(|(_, stack)| stack)
        })
        .or_else(|| {
            screen
                .recent
                .iter()
                .find(|(_, rect)| viewer::contains(*rect, cursor))
                .map(|(stack, _)| stack.clone())
        });
    let bookmark_tip = screen
        .view
        .as_ref()
        .filter(|_| plan.is_none() && details.is_none())
        .and_then(|view| {
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
        // JEI names the category and the mod that adds it.
        let mut lines: Vec<_> = title
            .lines()
            .map(|line| (horizon_client::text::Text::legacy(line), u32::MAX))
            .collect();
        let mod_line = (
            horizon_client::text::Text::legacy("\u{a7}9\u{a7}oMinecraft"),
            u32::MAX,
        );
        lines.insert(1.min(lines.len()), mod_line);
        game.draw_tooltip_lines(batch, &lines, cursor.0, cursor.1);
    } else if let Some(tip) = bookmark_tip {
        game.draw_tooltip_lines(batch, &[(tip.to_owned(), u32::MAX)], cursor.0, cursor.1);
    } else if let Some((_, transfer)) = screen
        .view
        .as_ref()
        .filter(|_| plan.is_none() && details.is_none())
        .and_then(|view| hovered_transfer(game, view, &transfers))
    {
        let line = (transfer.tooltip().to_owned(), u32::MAX);
        game.draw_tooltip_lines(batch, &[line], cursor.0, cursor.1);
    } else if let Some(i) = hovered
        .as_ref()
        .and_then(|sent| crate::game::aei_glue::fluid(&sent.item))
    {
        // A fluid's tooltip: its name, then what it is.
        let mut lines = vec![
            (
                horizon_client::text::Text::legacy(&crate::game::aei_glue::fluid_name(game, i)),
                u32::MAX,
            ),
            (horizon_client::text::Text::legacy("\u{a7}7Fluid"), u32::MAX),
        ];
        lines.extend(
            slot_lines
                .into_iter()
                .map(|line| (horizon_client::text::Text::legacy(&line), u32::MAX)),
        );
        game.draw_tooltip_lines(batch, &lines, cursor.0, cursor.1);
    } else if let Some((sent, stack)) = hovered.and_then(|sent| {
        let stack = super::browser_stack(world_items!(game), &sent)?;
        Some((sent, stack))
    }) {
        let extra = slot_lines
            .into_iter()
            .map(|line| (horizon_client::text::Text::legacy(&line), u32::MAX))
            .collect();
        game.draw_stack_tooltip_with(batch, &stack, false, cursor.0, cursor.1, extra);
        // Beyond JEI (an option): Shift shows the item's first recipe by the pointer.
        if game.has_shift_down()
            && let Some((card, station)) = crate::game::aei_glue::preview(&sent)
        {
            draw_preview(game, batch, &card, station);
        }
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

/// The panel holds at most this many recipes of a category at a time.
const WINDOW_LIMIT: usize = 256;

/// The longest window that ends on a page boundary for `per_page` recipes a page.
fn window_len(per_page: usize) -> usize {
    let per_page = per_page.max(1);
    (WINDOW_LIMIT / per_page).max(1) * per_page
}

/// The page shown out of all the category's pages: (page, pages), across windows.
fn global_page(view: &Viewer) -> (usize, usize) {
    let (offset, total) = crate::game::aei_glue::recipe_window();
    let per_page = view.per_page.max(1);
    let pages = total.div_ceil(per_page).max(view.pages);
    (offset / per_page + view.page, pages)
}

fn set_local_page(page: usize) {
    NAVIGATION.with(|cell| {
        if let Some(state) = cell.borrow_mut().as_mut() {
            state.page = page;
        }
    });
}

/// The next or previous page, moving to another window of a long category at the
/// window's ends and wrapping around at the category's ends.
fn turn_page(view: &Viewer, forward: bool) {
    let (offset, total) = crate::game::aei_glue::recipe_window();
    let len = window_len(view.per_page);
    let shown = view.pages * view.per_page;
    let windowed = total > shown || offset > 0;
    let mut window = None;
    let page = if forward {
        if view.page + 1 < view.pages {
            view.page + 1
        } else if windowed {
            let next = if offset + shown < total {
                offset + len
            } else {
                0
            };
            window = Some(next);
            0
        } else {
            0
        }
    } else if view.page > 0 {
        view.page - 1
    } else if windowed {
        let previous = if offset > 0 {
            offset.saturating_sub(len)
        } else {
            (total.saturating_sub(1) / len) * len
        };
        window = Some(previous);
        (total - previous).min(len).div_ceil(view.per_page.max(1)) - 1
    } else {
        view.pages - 1
    };
    if let Some(offset) = window {
        PENDING_WINDOW.with(|cell| *cell.borrow_mut() = Some((offset, len)));
    }
    set_local_page(page);
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
    let (crumbs, _) = breadcrumbs(view);
    if let Some((i, ..)) = crumbs
        .iter()
        .find(|(.., rect)| viewer::contains(*rect, game.cursor))
    {
        crate::game::aei_glue::jump(game, *i);
        game.audio
            .play("minecraft:ui.button.click", None, 0.25, 1.0);
    } else if let Some((index, _)) = view
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
        if global_page(view).1 > 1 {
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
    // The details panel is modal: a tag lists its items; elsewhere it closes.
    if let Some(details) = DetailsView::new(game, [batch.width, batch.height]) {
        if let Some((tag, _)) = details
            .tags
            .iter()
            .find(|(_, rect)| viewer::contains(*rect, game.cursor))
        {
            DETAILS.with(|cell| *cell.borrow_mut() = None);
            if right {
                let tag = tag.clone();
                crate::game::aei_glue::tag_uses(game, &tag);
            } else {
                game.aei_set_search(&format!("${tag}"));
            }
            return ControlFlow::Break(true);
        }
        if viewer::contains(details.note, game.cursor) {
            DETAILS.with(|cell| *cell.borrow_mut() = None);
            let item = details.stack.item.clone();
            let note = crate::game::aei_glue::note(&item).unwrap_or_default();
            let query = game
                .panel
                .search
                .as_ref()
                .map(|s| s.value.clone())
                .unwrap_or_default();
            NOTE_EDIT.with(|cell| *cell.borrow_mut() = Some((item, query)));
            if let Some(search) = game.panel.search.as_mut() {
                search.set_value(&note);
                search.focused = true;
                search.reset_blink();
            }
            return ControlFlow::Break(true);
        }
        if viewer::contains(details.area, game.cursor) {
            return ControlFlow::Break(true);
        }
        DETAILS.with(|cell| *cell.borrow_mut() = None);
    }
    // The calculator is modal: its own clicks, and the lists beside it close it.
    if let Some(plan) = PlanView::new(game, [batch.width, batch.height]) {
        if viewer::contains(plan.panel.area, game.cursor) {
            press_plan(game, &plan, right);
            return ControlFlow::Break(true);
        }
        let beside = screen
            .items
            .as_ref()
            .is_some_and(|(items, _)| inside(game.cursor, items.area))
            || screen
                .bookmarks
                .as_ref()
                .is_some_and(|list| inside(game.cursor, list.area));
        if !beside {
            return ControlFlow::Break(true);
        }
        // Searching and paging keep it open, to add more items; opening an item's
        // recipes from a list closes it.
        let opens = screen
            .items
            .as_ref()
            .is_some_and(|(_, layout)| hovered_item(game, layout).is_some())
            || screen
                .bookmarks
                .as_ref()
                .is_some_and(|list| hovered_bookmark(game, list).is_some());
        if opens {
            PLAN.with(|cell| *cell.borrow_mut() = None);
        }
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
    if let Some((stack, _)) = screen
        .recent
        .iter()
        .find(|(_, rect)| viewer::contains(*rect, game.cursor))
    {
        crate::game::aei_glue::lookup(game, stack, right);
        return ControlFlow::Break(true);
    }
    // A chip runs its search (right-click forgets it); the pin chip saves the search.
    if let Some((chip, _)) = screen
        .chips
        .iter()
        .find(|(_, rect)| viewer::contains(*rect, game.cursor))
    {
        match (chip, right) {
            (Chip::Saved(i, _), true) => crate::game::aei_glue::unpin_chip(game, *i),
            (Chip::Saved(_, query), false) => game.aei_set_search(query),
            (Chip::Pin(query), _) => crate::game::aei_glue::pin_chip(game, query),
            (Chip::New(_), false) => game.aei_set_search("is:new"),
            (Chip::New(_), true) => crate::game::aei_glue::clear_fresh(game),
        }
        game.audio
            .play("minecraft:ui.button.click", None, 0.25, 1.0);
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
        if right && viewer::contains(list.nav, game.cursor) {
            // Right-click names the list, typed in the search field.
            let list = crate::game::aei_glue::bookmark_list();
            let mut name = crate::game::aei_glue::list_name(game, list);
            // A list not named yet starts blank.
            if name == format!("L{}", list + 1) {
                name.clear();
            }
            let query = game
                .panel
                .search
                .as_ref()
                .map(|s| s.value.clone())
                .unwrap_or_default();
            NOTE_EDIT.with(|cell| *cell.borrow_mut() = Some((format!("aei:list/{list}"), query)));
            if let Some(search) = game.panel.search.as_mut() {
                search.set_value(&name);
                search.focused = true;
                search.reset_blink();
            }
            return ControlFlow::Break(true);
        }
        if viewer::contains(list.nav, game.cursor) {
            crate::game::aei_glue::cycle_bookmark_list(game);
            BOOKMARK_PAGE.with(|cell| *cell.borrow_mut() = 0);
            game.audio
                .play("minecraft:ui.button.click", None, 0.25, 1.0);
            return ControlFlow::Break(true);
        }
        if let Some((index, _)) = hovered_bookmark(game, list) {
            if right {
                let event = crate::game::aei::Event::ClickBookmark { index, right };
                crate::game::aei_glue::bookmark_event(game, event);
            } else {
                BOOKMARK_DRAG.with(|cell| *cell.borrow_mut() = Some(index));
            }
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
    // Keyboard focus: the keys act on the focused entry.
    if let Some(index) = KB_FOCUS.with(|cell| *cell.borrow())
        && let Some(stack) = game.panel_shown().and_then(|shown| shown.items.get(index))
    {
        return ControlFlow::Break(Some(stack.clone()));
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
    if let Some(plan) = PlanView::new(game, [batch.width, batch.height])
        && let Some((_, _, stack, _)) = plan
            .cells()
            .into_iter()
            .find(|(.., rect)| viewer::contains(*rect, game.cursor))
    {
        return ControlFlow::Break(Some(stack));
    }
    if let Some(list) = &screen.bookmarks
        && let Some((_, stack)) = hovered_bookmark(game, list)
    {
        return ControlFlow::Break(Some(stack));
    }
    if let Some((stack, _)) = screen
        .recent
        .iter()
        .find(|(_, rect)| viewer::contains(*rect, game.cursor))
    {
        return ControlFlow::Break(Some(stack.clone()));
    }
    match &screen.view {
        Some(view) => ControlFlow::Break(
            hovered_recipe(game, view)
                .map(|(_, _, stack)| stack)
                .or_else(|| hovered_station(game, view)),
        ),
        // A vanilla recipe book button: its result, as JEI reads the book.
        None => match game.recipe_under_pointer() {
            Some(index) => {
                let entries = game.recipe_book_entries();
                let stack = entries.get(index).map(|entry| BrowserStack {
                    item: entry.result.item.to_string(),
                    count: entry.result.count,
                    components: crate::game::native_recipes::components(&entry.result),
                });
                ControlFlow::Break(stack)
            }
            // Over the container itself, Horizon finds the slot's stack.
            None => ControlFlow::Continue(()),
        },
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
    if let Some(plan) = PlanView::new(game, [batch.width, batch.height])
        && viewer::contains(plan.panel.area, game.cursor)
    {
        if amount != 0.0 {
            change_plan_count(if amount > 0.0 { 1 } else { -1 });
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
    if amount != 0.0 && global_page(view).1 > 1 {
        turn_page(view, amount < 0.0);
    }
    true
}

pub(super) fn back(game: &mut Game<'_>) -> ControlFlow<bool> {
    // Escape cancels a note being typed: the search comes back.
    if owned(game).is_some()
        && let Some((_, query)) = NOTE_EDIT.with(|cell| cell.borrow_mut().take())
    {
        game.aei_set_search(&query);
        return ControlFlow::Break(true);
    }
    // Escape leaves keyboard focus first.
    if owned(game).is_some()
        && game.panel.recipes.is_none()
        && KB_FOCUS.with(|cell| cell.borrow_mut().take()).is_some()
    {
        return ControlFlow::Break(true);
    }
    // Escape closes the details panel and the calculator first.
    if owned(game).is_some() && DETAILS.with(|cell| cell.borrow_mut().take()).is_some() {
        return ControlFlow::Break(true);
    }
    if owned(game).is_some() && PLAN.with(|cell| cell.borrow_mut().take()).is_some() {
        return ControlFlow::Break(true);
    }
    // Escape leaves the recipes for the inventory, however deep the lookups went
    // (Backspace steps back one).
    if active(game).is_none() {
        return ControlFlow::Continue(());
    }
    game.mod_browser_closed();
    crate::game::aei_glue::close_recipes(game);
    ControlFlow::Break(true)
}

impl Game<'_> {
    /// Opens the material calculator for `stack` (the T key).
    /// Keyboard navigation of the item list: Tab turns focus on and off, the arrows
    /// move it (turning pages at the edges), Enter opens the focused item's recipes.
    /// True when the key was used.
    pub(in crate::game) fn aei_list_key(&mut self, key: &str) -> bool {
        if owned(self).is_none() || self.mod_browser_typing() || self.panel.recipes.is_some() {
            return false;
        }
        let Some(batch) = self.browser_batch() else {
            return false;
        };
        let Some((_, layout)) = screen(self, &batch, false).and_then(|s| s.items) else {
            return false;
        };
        let count = self.panel.stacks.len();
        let focus = KB_FOCUS.with(|cell| *cell.borrow());
        let per_page = layout.per_page().max(1);
        let columns = layout.columns.max(1) as i64;
        let first = self.panel.page * per_page;
        let moved = |by: i64| -> Option<usize> {
            let index = focus? as i64 + by;
            (0..count as i64).contains(&index).then_some(index as usize)
        };
        let next = match key {
            "key.keyboard.tab" => {
                let on = focus.is_none() && count > 0;
                KB_FOCUS.with(|cell| *cell.borrow_mut() = on.then_some(first.min(count - 1)));
                return true;
            }
            _ if focus.is_none() => return false,
            "key.keyboard.left" => moved(-1),
            "key.keyboard.right" => moved(1),
            "key.keyboard.up" => moved(-columns),
            "key.keyboard.down" => moved(columns),
            "key.keyboard.enter" | "key.keyboard.keypad.enter" => {
                let Some(owner) = owned(self) else {
                    return false;
                };
                let index = focus.unwrap_or_default() as u32;
                self.panel_event(
                    &owner,
                    BrowserEvent::Clicked {
                        index,
                        right: false,
                    },
                );
                return true;
            }
            _ => return false,
        };
        if let Some(index) = next {
            KB_FOCUS.with(|cell| *cell.borrow_mut() = Some(index));
            self.panel.page = index / per_page;
        }
        true
    }

    /// A controller in an inventory has no AEI keys, so two of its inventory buttons stand
    /// in over an AEI entry: View (the game's throw-one) bookmarks it, and the left stick
    /// click (throw-all) opens the material calculator. A, X and B already click,
    /// right-click and go back; the right stick turns pages. True when AEI took it.
    pub(in crate::game) fn aei_pad_button(
        &mut self,
        button: horizon_client::gamepad::Button,
        down: bool,
    ) -> bool {
        use horizon_client::gamepad::Button;
        let mapping = match button {
            Button::Select => crate::game::aei::KEY_BOOKMARK,
            Button::LeftStick => crate::game::aei::KEY_PLAN,
            _ => return false,
        };
        if !down || self.title.is_some() || self.prompt_open() {
            return false;
        }
        let Some(owner) = owned(self) else {
            return false;
        };
        // On the slots of the inventory the buttons keep throwing items.
        let ControlFlow::Break(Some(stack)) = pointed(self) else {
            return false;
        };
        let key = self.native_mod_key(crate::game::aei::ID, mapping[0], mapping[2]);
        self.panel_event(&owner, BrowserEvent::Key { key, stack });
        true
    }

    /// Whether a note is being typed in the search field (its text isn't a search).
    pub(in crate::game) fn aei_editing_note(&self) -> bool {
        NOTE_EDIT.with(|cell| cell.borrow().is_some())
    }

    /// Enter while typing a note: saves it and brings the search back. True when a
    /// note was being typed.
    pub(in crate::game) fn aei_finish_note(&mut self) -> bool {
        let Some((item, query)) = NOTE_EDIT.with(|cell| cell.borrow_mut().take()) else {
            return false;
        };
        let text = self
            .panel
            .search
            .as_ref()
            .map(|s| s.value.clone())
            .unwrap_or_default();
        match item.strip_prefix("aei:list/").and_then(|i| i.parse().ok()) {
            Some(list) => crate::game::aei_glue::set_list_name(self, list, &text),
            None => crate::game::aei_glue::save_note(self, &item, &text),
        }
        self.aei_set_search(&query);
        if let Some(search) = self.panel.search.as_mut() {
            search.focused = false;
        }
        true
    }

    /// Ctrl+C over the bookmark bar's label copies the shown list as text; Ctrl+V
    /// replaces it with a list from the clipboard.
    pub(in crate::game) fn aei_collection_key(&mut self, key: &str) -> bool {
        if owned(self).is_none() || !self.has_control_down() || self.mod_browser_typing() {
            return false;
        }
        let Some(batch) = self.browser_batch() else {
            return false;
        };
        let recipes = self.panel.recipes.is_some();
        let over = screen(self, &batch, recipes)
            .and_then(|screen| screen.bookmarks)
            .is_some_and(|list| viewer::contains(list.nav, self.cursor));
        if !over {
            return false;
        }
        match key {
            "key.keyboard.c" => crate::game::aei_glue::export_collection(self),
            "key.keyboard.v" => crate::game::aei_glue::import_collection(self),
            _ => return false,
        }
        true
    }

    /// The pinned recipes a session starts with (saved by id).
    pub(in crate::game) fn aei_set_pinned(
        &mut self,
        pinned: Vec<(crate::game::aei::RecipeCard, &'static str, String)>,
    ) {
        PINNED.with(|cell| *cell.borrow_mut() = pinned);
    }

    /// P over a recipe card pins it beside the screen (P over a pinned one unpins it).
    pub(in crate::game) fn aei_pin_key(&mut self) -> bool {
        if owned(self).is_none() || self.mod_browser_typing() {
            return false;
        }
        let Some(batch) = self.browser_batch() else {
            return false;
        };
        toggle_pin(self, &batch)
    }

    /// Beyond JEI (an option): while playing, the calculator's shopping list shows on the
    /// HUD: what it still lacks after the inventory, or that everything is ready.
    pub(in crate::game) fn aei_plan_hud(
        &mut self,
        batch: &mut HudBatch,
        corners: &mut horizon_client::extras_hud::Corners,
    ) {
        use horizon_client_render::extras::Corner;
        if !crate::game::aei_glue::plan_hud_enabled()
            || self.screen != crate::game::Screen::Playing
            || self.title.is_some()
            || !self.native_corner_shown(Corner::TopLeft)
        {
            return;
        }
        let Some((targets, _)) = plan_state() else {
            return;
        };
        let Some(plan) = crate::game::aei_glue::plan(self, &targets) else {
            return;
        };
        let mut lines = vec!["Shopping list".to_owned()];
        let short: Vec<_> = plan.materials.iter().filter(|m| m.have < m.need).collect();
        for m in short.iter().take(8) {
            let name = self.pack.names.display_item(&m.stack.id);
            lines.push(format!("\u{a7}c{} \u{d7} {name}", m.need - m.have));
        }
        if short.len() > 8 {
            lines.push(format!("\u{a7}7... and {} more", short.len() - 8));
        }
        if short.is_empty() {
            lines.push("\u{a7}aEverything is in the inventory".to_owned());
        }
        let font = &self.gui.atlas.font;
        let w = lines.iter().map(|l| font.width(l)).fold(0.0, f32::max) + 6.0;
        let h = lines.len() as f32 * 10.0 + 4.0;
        let (x, y) = corners.place(Corner::TopLeft, w, h);
        self.native_fill(batch, [x, y, w, h], 0x7800_0000);
        for (i, line) in lines.iter().enumerate() {
            let colour = if i == 0 {
                rgba(255, 255, 160, 255)
            } else {
                u32::MAX
            };
            let text = horizon_client::text::Text::legacy(line);
            batch.text_styled(
                &self.gui.atlas,
                &text,
                x + 3.0,
                y + 3.0 + i as f32 * 10.0,
                colour,
                true,
            );
        }
    }

    /// AEI's high-contrast theme (an option).
    pub(in crate::game) fn aei_set_high_contrast(&self, on: bool) {
        paint::set_high_contrast(on);
    }

    /// AEI's dark theme (an option).
    pub(in crate::game) fn aei_set_dark(&self, on: bool) {
        paint::set_dark(on);
    }

    /// The I key: the details panel for `stack`.
    pub(in crate::game) fn aei_open_details(&mut self, stack: BrowserStack) {
        DETAILS.with(|cell| *cell.borrow_mut() = Some(BrowserStack { count: 1, ..stack }));
    }

    /// The T key: opens the material calculator for `stack`, or, while it is open,
    /// adds `stack` to its shopping list (or focuses it if it is there).
    pub(in crate::game) fn aei_open_plan(&mut self, stack: BrowserStack) {
        let stack = BrowserStack { count: 1, ..stack };
        PLAN.with(|cell| {
            let mut state = cell.borrow_mut();
            match state.as_mut() {
                Some((targets, focus)) => match targets.iter().position(|(s, _)| *s == stack) {
                    Some(i) => *focus = i,
                    None => {
                        targets.push((stack, 1));
                        *focus = targets.len() - 1;
                    }
                },
                None => *state = Some((vec![(stack, 1)], 0)),
            }
        });
    }

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

    /// A left release after a press on a bookmark: a click on it when released over it
    /// (or off the list), else a move to the bookmark it is released over. True when
    /// a bookmark press was pending.
    pub(in crate::game) fn aei_bookmark_release(&mut self) -> bool {
        let Some(from) = BOOKMARK_DRAG.with(|cell| cell.borrow_mut().take()) else {
            return false;
        };
        let to = self.browser_batch().and_then(|batch| {
            let recipes = self.panel.recipes.is_some();
            let list = screen(self, &batch, recipes)?.bookmarks?;
            hovered_bookmark(self, &list).map(|(i, _)| i)
        });
        let event = match to {
            Some(to) if to != from => crate::game::aei::Event::MoveBookmark { from, to },
            _ => crate::game::aei::Event::ClickBookmark {
                index: from,
                right: false,
            },
        };
        crate::game::aei_glue::bookmark_event(self, event);
        true
    }

    /// The recipe card under the pointer when it isn't over one of its slots.
    pub(in crate::game) fn aei_hovered_card(&mut self) -> Option<usize> {
        let batch = self.browser_batch()?;
        active(self)?;
        let view = screen(self, &batch, true)?.view?;
        let cursor = self.cursor;
        view.cards.iter().find_map(|(i, card)| {
            let on_slot = card.slots.iter().any(|(_, r)| viewer::contains(*r, cursor));
            (viewer::contains(card.area, cursor) && !on_slot).then_some(*i)
        })
    }

    /// Ctrl+Shift+C over a recipe card: saves the card as a PNG in AEI's folder
    /// (`recipes/<recipe id>.png`), cut from a screenshot of the next frame.
    pub(in crate::game) fn aei_export_card(&mut self, card: usize) -> bool {
        let Some(batch) = self.browser_batch() else {
            return false;
        };
        if active(self).is_none() {
            return false;
        }
        let Some(view) = screen(self, &batch, true).and_then(|s| s.view) else {
            return false;
        };
        let Some((_, area)) = view
            .cards
            .iter()
            .find(|(i, _)| *i == card)
            .map(|(i, c)| (*i, c.area))
        else {
            return false;
        };
        let dir = self.native_data_dir(crate::game::aei::ID);
        let name = crate::game::aei_glue::shown_card(card)
            .map(|(_, _, id)| id)
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| "recipe".to_owned())
            .replace([':', '/', '\\', '#'], "_");
        let capture = dir.join(".capture.png");
        let _ = std::fs::remove_file(&capture);
        let Some(renderer) = self.renderer.as_mut() else {
            return false;
        };
        renderer.screenshot = Some(horizon_client::gfx::ScreenshotRequest {
            path: capture.clone(),
            size: None,
            scaled: false,
            // Written off the render thread; the frame loop waits for the file.
            background: true,
        });
        EXPORT.with(|cell| {
            *cell.borrow_mut() = Some(Export {
                capture,
                out: dir.join("recipes").join(format!("{name}.png")),
                area,
                gui_width: batch.width,
                since: std::time::Instant::now(),
            })
        });
        true
    }

    /// Each frame: once the screenshot asked for by `aei_export_card` is written, cuts
    /// the card out of it.
    pub(in crate::game) fn aei_finish_export(&mut self) {
        let Some(job) = EXPORT.with(|cell| cell.borrow().clone()) else {
            return;
        };
        let done = |game: &mut Game<'_>, text: String| {
            EXPORT.with(|cell| *cell.borrow_mut() = None);
            let _ = std::fs::remove_file(&job.capture);
            game.message = Some((text, std::time::Instant::now()));
        };
        let Ok((w, h, rgba)) = horizon_client::hud::decode(&job.capture) else {
            if job.since.elapsed().as_secs() > 5 {
                done(self, "The recipe image couldn't be taken".to_owned());
            }
            return;
        };
        // The screenshot is in window pixels; the card in GUI pixels.
        let scale = w as f32 / job.gui_width.max(1.0);
        let [x, y, cw, ch] = job.area.map(|v| (v * scale).round() as i64);
        let (x0, y0) = (x.clamp(0, w as i64) as u32, y.clamp(0, h as i64) as u32);
        let (x1, y1) = (
            (x + cw).clamp(0, w as i64) as u32,
            (y + ch).clamp(0, h as i64) as u32,
        );
        if x1 <= x0 || y1 <= y0 {
            done(self, "The recipe image couldn't be taken".to_owned());
            return;
        }
        let mut cut = Vec::with_capacity(((x1 - x0) * (y1 - y0) * 4) as usize);
        for row in y0..y1 {
            let from = ((row * w + x0) * 4) as usize;
            let to = ((row * w + x1) * 4) as usize;
            cut.extend_from_slice(&rgba[from..to]);
        }
        let written = job
            .out
            .parent()
            .map(std::fs::create_dir_all)
            .transpose()
            .ok()
            .and_then(|_| std::fs::File::create(&job.out).ok())
            .and_then(|file| {
                let mut encoder =
                    png::Encoder::new(std::io::BufWriter::new(file), x1 - x0, y1 - y0);
                encoder.set_color(png::ColorType::Rgba);
                encoder.set_depth(png::BitDepth::Eight);
                encoder.write_header().ok()?.write_image_data(&cut).ok()
            });
        let text = match written {
            Some(()) => format!("Saved the recipe image to {}", job.out.display()),
            None => "The recipe image couldn't be saved".to_owned(),
        };
        done(self, text);
    }

    /// Puts `text` in AEI's search field and searches it, as typing would.
    pub(in crate::game) fn aei_set_search(&mut self, text: &str) {
        let Some(search) = self.panel.search.as_mut() else {
            return;
        };
        search.set_value(text);
        search.reset_blink();
        self.browser_searched();
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
        }) || screen
            .recent
            .iter()
            .any(|(_, rect)| viewer::contains(*rect, cursor))
            || screen.bookmarks.as_ref().is_some_and(|list| {
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

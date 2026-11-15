use crate::game::Game;
use crate::game::aei as model;
use crate::game::native_hooks::{
    NativeItem, NativePanelEvent, NativeRecipe, NativeRecipeKind, NativeRecipeLayout,
    NativeRecipeStack, NativeRecipeView, NativeRecipes, NativeStack,
};

struct Session {
    tables: std::sync::Arc<NativeRecipes>,
    known: std::sync::Arc<[i32]>,
    browser: model::Browser,
}

/// Ctrl+O hid the overlay; the mod name tooltip line is on. Cached from the settings so
/// the panel and tooltip hooks don't read them every frame.
static HIDDEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static MOD_NAMES: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

std::thread_local! {
    static BROWSER: std::cell::RefCell<Option<Session>> = const { std::cell::RefCell::new(None) };
    /// The bookmarks as the overlay draws them: each stack, and whether it is a recipe's.
    static BOOKMARKS: std::cell::RefCell<Vec<(NativeStack, bool)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

const SETTING_BOOKMARKS: &str = "bookmarks";

/// The bookmark list: each stack, and whether it is a recipe bookmark.
pub(super) fn bookmarks() -> Vec<(NativeStack, bool)> {
    BOOKMARKS.with(|cell| cell.borrow().clone())
}

pub(super) fn bookmark_count() -> usize {
    BOOKMARKS.with(|cell| cell.borrow().len())
}

/// Runs a bookmark event (click, recipe bookmark, move) and shows what it changed.
pub(super) fn bookmark_event(game: &mut Game<'_>, event: model::Event) {
    let known = game.native_known_recipes();
    let give = flag(game, SETTING_GIVE);
    let updates = with_browser(|browser| browser.handle(event, give, &known)).unwrap_or_default();
    for update in updates {
        if matches!(&update, model::Update::RecipeList(_)) {
            game.aei_reset_viewer();
        }
        publish(game, update);
    }
}

/// Whether card `card` of the shown recipe page is bookmarked.
pub(super) fn recipe_bookmarked(card: usize) -> bool {
    with_browser(|browser| browser.shown_recipe_bookmarked(card)).unwrap_or(false)
}

/// Whether card `card` of the shown recipe page can be bookmarked (it has an id).
pub(super) fn recipe_bookmarkable(card: usize) -> bool {
    with_browser(|browser| browser.shown_recipe_id(card).is_some()).unwrap_or(false)
}

fn with_browser<T>(run: impl FnOnce(&mut model::Browser) -> T) -> Option<T> {
    BROWSER.with(|cell| {
        cell.borrow_mut()
            .as_mut()
            .map(|session| run(&mut session.browser))
    })
}

fn item(source: &NativeItem) -> model::ItemInfo {
    model::ItemInfo {
        id: source.id.clone(),
        name: source.name.clone(),
        tags: source.tags.clone(),
    }
}

/// The item list as JEI shows it: the creative search tab's stacks in tab order, variants
/// (enchanted books, potions) included, each with its tooltip and the tabs that list it.
fn catalogue(game: &Game<'_>) -> Vec<model::Entry> {
    let creative = &game.creative_screen;
    let key = |stack: &horizon_client::items::ItemStack| {
        (
            stack.item.to_string(),
            super::native_recipes::components(stack),
        )
    };
    let mut tabs: std::collections::HashMap<(String, String), Vec<String>> = Default::default();
    for tab in creative
        .tabs
        .iter()
        .filter(|tab| !tab.search && !tab.inventory && !tab.hotbar)
    {
        let title = game.creative_tab_title(tab);
        for stack in &tab.items {
            let titles = tabs.entry(key(stack)).or_default();
            if !titles.contains(&title) {
                titles.push(title.clone());
            }
        }
    }
    creative
        .search_items()
        .iter()
        .filter(|stack| stack.count > 0)
        .map(|stack| {
            // The tooltip the player sees: name first, then enchantments, effects, lore.
            let mut lines = game
                .stack_tooltip(stack)
                .into_iter()
                .map(|(text, _)| text.plain());
            let name = lines.next().unwrap_or_default();
            let (id, components) = key(stack);
            let tabs = tabs.get(&(id.clone(), components.clone())).cloned();
            model::Entry {
                stack: model::Stack {
                    id,
                    count: 1,
                    components,
                },
                name,
                tooltip: lines.collect(),
                tabs: tabs.unwrap_or_default(),
            }
        })
        .collect()
}

/// The search modes the settings page chose.
fn search_modes(game: &Game<'_>) -> model::SearchModes {
    let mut modes = model::SearchModes::default();
    for (field, (key, _, _)) in model::SearchModes::FIELDS.iter().enumerate() {
        if let Some(mode) = game
            .native_setting(model::ID, key)
            .and_then(|value| model::SearchMode::parse(&value))
        {
            modes.set(field, mode);
        }
    }
    modes
}

fn ingredient(source: &NativeRecipeStack) -> model::Ingredient {
    model::Ingredient {
        item: source.item as usize,
        count: source.count,
        components: source.components.clone(),
    }
}

fn recipe(source: &NativeRecipe) -> model::Recipe {
    let kind = match source.kind {
        NativeRecipeKind::CraftingShaped => model::RecipeKind::Shaped,
        NativeRecipeKind::CraftingShapeless => model::RecipeKind::Shapeless,
        NativeRecipeKind::Smelting => model::RecipeKind::Smelting,
        NativeRecipeKind::Blasting => model::RecipeKind::Blasting,
        NativeRecipeKind::Smoking => model::RecipeKind::Smoking,
        NativeRecipeKind::Campfire => model::RecipeKind::Campfire,
        NativeRecipeKind::Stonecutting => model::RecipeKind::Stonecutting,
        NativeRecipeKind::Smithing => model::RecipeKind::Smithing,
        NativeRecipeKind::Brewing => model::RecipeKind::Brewing,
    };
    model::Recipe {
        id: source.id.clone(),
        kind,
        width: source.width,
        height: source.height,
        inputs: source
            .slots
            .iter()
            .map(|slot| slot.iter().map(ingredient).collect())
            .collect(),
        outputs: source.results.iter().map(ingredient).collect(),
        ticks: source.cooking_time,
        experience: source.experience,
        book_id: source.book_id,
    }
}

fn stack(source: &NativeStack) -> model::Stack {
    model::Stack {
        id: source.item.clone(),
        count: source.count,
        components: source.components.clone(),
    }
}

fn native(source: &model::Stack) -> NativeStack {
    NativeStack {
        item: source.id.clone(),
        count: source.count,
        components: source.components.clone(),
    }
}

fn card(source: &model::RecipeCard) -> NativeRecipeView {
    NativeRecipeView {
        layout: match source.layout {
            model::Layout::Grid => NativeRecipeLayout::Grid,
            model::Layout::Cooking => NativeRecipeLayout::Cooking,
            model::Layout::Single => NativeRecipeLayout::Single,
            model::Layout::Smithing => NativeRecipeLayout::Smithing,
            model::Layout::Brewing => NativeRecipeLayout::Brewing,
        },
        width: source.width,
        height: source.height,
        slots: source
            .inputs
            .iter()
            .map(|slot| slot.iter().map(native).collect())
            .collect(),
        results: source.outputs.iter().map(native).collect(),
        note: source.note.clone(),
        action: source.place_enabled,
    }
}

fn publish(game: &mut Game<'_>, update: model::Update) {
    match update {
        model::Update::Bookmarks(list) => {
            BOOKMARKS.with(|cell| {
                *cell.borrow_mut() = list
                    .iter()
                    .map(|b| (native(&b.stack), b.recipe.is_some()))
                    .collect();
            });
            if let Some(text) = with_browser(|browser| browser.bookmarks_text()) {
                game.native_set_setting(model::ID, SETTING_BOOKMARKS, &text);
            }
        }
        model::Update::ItemList(items) => {
            game.native_panel_items(model::ID, items.iter().map(native).collect());
        }
        model::Update::RecipeList(page) => {
            game.native_panel_recipes(
                model::ID,
                &page.title,
                page.stations.iter().map(native).collect(),
                page.station as u32,
                page.cards.iter().map(card).collect(),
            );
        }
        model::Update::Give { item, count } => {
            let _ = game.native_give(&item, count);
        }
        model::Update::PlaceKnownRecipe(id) => {
            match game.native_place_recipe(id, game.has_shift_down()) {
                Err(message) => game.native_chat(&message),
                // Placed: the recipe view closes so the grid shows, as JEI does.
                Ok(()) => {
                    if let Some(update) = with_browser(|browser| browser.close_recipes()) {
                        game.aei_reset_viewer();
                        publish(game, update);
                    }
                }
            }
        }
    }
}

fn clear(game: &mut Game<'_>) {
    let had_session = BROWSER.with(|cell| cell.borrow_mut().take().is_some());
    BOOKMARKS.with(|cell| cell.borrow_mut().clear());
    if had_session {
        game.aei_reset_catalogue();
        game.native_panel_recipes(model::ID, "", Vec::new(), 0, Vec::new());
        game.native_panel_items(model::ID, Vec::new());
    }
}

pub(super) fn frame(game: &mut Game<'_>) {
    if !game.native_on(model::ID) {
        clear(game);
        return;
    }
    let Some(tables) = game.native_recipes() else {
        clear(game);
        return;
    };
    let known = game.native_known_recipes();
    let update = BROWSER.with(|cell| {
        let mut session = cell.borrow_mut();
        if session
            .as_ref()
            .is_none_or(|old| !std::sync::Arc::ptr_eq(&old.tables, &tables))
        {
            let (mut browser, _) = model::Browser::new(
                tables.items.iter().map(item).collect(),
                tables.recipes.iter().map(recipe).collect(),
            );
            browser.handle(model::Event::SetSearchModes(search_modes(game)), false, &[]);
            load_flags(game);
            let saved = game
                .native_setting(model::ID, SETTING_BOOKMARKS)
                .unwrap_or_default();
            if let model::Update::Bookmarks(list) = browser.load_bookmarks(&saved) {
                BOOKMARKS.with(|cell| {
                    *cell.borrow_mut() = list
                        .iter()
                        .map(|b| (native(&b.stack), b.recipe.is_some()))
                        .collect();
                });
            }
            let initial = browser.set_catalogue(catalogue(game));
            *session = Some(Session {
                tables,
                known,
                browser,
            });
            return Some((true, initial));
        }
        let session = session.as_mut()?;
        if game.aei_panel_missing() {
            // Lost (an empty list published outside a search): start again from the full
            // list, with the search cleared.
            let mut updates =
                session
                    .browser
                    .handle(model::Event::Search(String::new()), false, &known);
            return updates.pop().map(|update| (true, update));
        }
        if session.known != known {
            session.known = known;
            return session
                .browser
                .refresh(&session.known)
                .map(|update| (false, update));
        }
        None
    });
    if let Some((reset, update)) = update {
        if reset {
            game.aei_reset_catalogue();
            game.native_panel_recipes(model::ID, "", Vec::new(), 0, Vec::new());
        }
        publish(game, update);
    }
}

pub(super) fn event(game: &mut Game<'_>, event: &NativePanelEvent) {
    if !game.native_on(model::ID) {
        return;
    }
    let event = match event {
        NativePanelEvent::Search(query) => model::Event::Search(query.clone()),
        NativePanelEvent::Clicked { index, right } => model::Event::ClickItem {
            index: *index as usize,
            right: *right,
        },
        NativePanelEvent::Key { key, stack: target } => {
            let is_recipe =
                *key == game.native_mod_key(model::ID, model::KEY_RECIPE[0], model::KEY_RECIPE[2]);
            let is_uses =
                *key == game.native_mod_key(model::ID, model::KEY_USES[0], model::KEY_USES[2]);
            let is_back =
                *key == game.native_mod_key(model::ID, model::KEY_BACK[0], model::KEY_BACK[2]);
            let is_forward = *key
                == game.native_mod_key(model::ID, model::KEY_FORWARD[0], model::KEY_FORWARD[2]);
            let is_bookmark = *key
                == game.native_mod_key(model::ID, model::KEY_BOOKMARK[0], model::KEY_BOOKMARK[2]);
            if is_bookmark {
                model::Event::ToggleBookmark(stack(target))
            } else if is_back {
                model::Event::Back
            } else if is_forward {
                model::Event::Forward
            } else if is_recipe || is_uses {
                model::Event::Lookup {
                    stack: stack(target),
                    uses: is_uses,
                }
            } else {
                return;
            }
        }
        NativePanelEvent::SlotClicked {
            stack: target,
            right,
            ..
        } => model::Event::ClickRecipeIngredient {
            stack: stack(target),
            right: *right,
        },
        NativePanelEvent::Tab(index) => model::Event::SelectStation(*index as usize),
        NativePanelEvent::Action(index) => model::Event::Place(*index as usize),
        NativePanelEvent::Back => model::Event::Back,
        _ => return,
    };

    let original_event = event.clone();
    let known = game.native_known_recipes();
    let give_enabled = flag(game, SETTING_GIVE);
    let updates =
        with_browser(|browser| browser.handle(event, give_enabled, &known)).unwrap_or_default();
    for update in updates {
        if matches!(&update, model::Update::RecipeList(_)) {
            game.aei_reset_viewer();
        }
        if let model::Update::Give { item, count } = &update {
            if game.native_give(item, *count).is_err() {
                let fallback =
                    with_browser(|browser| browser.handle(original_event.clone(), false, &known))
                        .unwrap_or_default();
                for next in fallback {
                    publish(game, next);
                }
            }
            continue;
        }
        publish(game, update);
    }
}

/// The tooltip lines JEI adds to a recipe slot: "Accepts any" with the tag an input's
/// alternatives stand for, and the recipe's id on its output with advanced tooltips.
pub(super) fn slot_lines(card: usize, slot: usize, output: bool, advanced: bool) -> Vec<String> {
    with_browser(|browser| {
        let mut lines = Vec::new();
        if output {
            if advanced && let Some(id) = browser.shown_recipe_id(card) {
                lines.push(format!("\u{a7}8Recipe ID: {id}"));
            }
        } else if let Some(tag) = browser.shown_slot_tag(card, slot) {
            lines.push(format!("\u{a7}7Accepts any: #{tag}"));
        }
        lines
    })
    .unwrap_or_default()
}

/// The recipe-book ids of the shown recipes, in card order.
pub(super) fn shown_book_ids() -> Vec<i32> {
    with_browser(|browser| browser.shown_book_ids()).unwrap_or_default()
}

pub(super) fn show_category(game: &mut Game<'_>) {
    let known = game.native_known_recipes();
    let updates = with_browser(|browser| browser.handle(model::Event::ShowCategory, false, &known))
        .unwrap_or_default();
    game.aei_reset_viewer();
    for update in updates {
        publish(game, update);
    }
}

pub(super) fn navigation(game: &mut Game<'_>, key: &str) -> bool {
    if !game.native_on(model::ID) {
        return false;
    }
    let event = if key == game.native_mod_key(model::ID, model::KEY_BACK[0], model::KEY_BACK[2]) {
        model::Event::Back
    } else if key == game.native_mod_key(model::ID, model::KEY_FORWARD[0], model::KEY_FORWARD[2]) {
        model::Event::Forward
    } else {
        return false;
    };
    let known = game.native_known_recipes();
    let updates = with_browser(|browser| browser.handle(event, false, &known)).unwrap_or_default();
    game.aei_reset_viewer();
    for update in updates {
        publish(game, update);
    }
    true
}

const SETTING_GIVE: &str = "give";
const SETTING_HIDDEN: &str = "hidden";
const SETTING_MOD_NAMES: &str = "mod_names";

fn flag(game: &Game<'_>, key: &str) -> bool {
    game.native_setting(model::ID, key).as_deref() == Some("true")
}

fn load_flags(game: &Game<'_>) {
    use std::sync::atomic::Ordering::Relaxed;
    HIDDEN.store(flag(game, SETTING_HIDDEN), Relaxed);
    MOD_NAMES.store(flag(game, SETTING_MOD_NAMES), Relaxed);
}

/// Whether Ctrl+O hid AEI's overlay.
pub(super) fn overlay_hidden() -> bool {
    HIDDEN.load(std::sync::atomic::Ordering::Relaxed)
}

/// Ctrl+O: show or hide the overlay, remembered across sessions as JEI does.
pub(super) fn toggle_overlay(game: &mut Game<'_>) {
    let hidden = !overlay_hidden();
    HIDDEN.store(hidden, std::sync::atomic::Ordering::Relaxed);
    game.native_set_setting(
        model::ID,
        SETTING_HIDDEN,
        if hidden { "true" } else { "false" },
    );
}

/// The mod name line of an item's tooltip, when the setting is on: the mod that adds
/// `item`, by its namespace ("Minecraft" for vanilla's).
pub(super) fn tooltip_mod_name(item: &str) -> Option<String> {
    MOD_NAMES
        .load(std::sync::atomic::Ordering::Relaxed)
        .then(|| model::mod_name(item.split_once(':').map_or("minecraft", |(ns, _)| ns)))
}

fn on_off(on: bool) -> &'static str {
    if on { "ON" } else { "OFF" }
}

/// The settings page's buttons, in [`settings_click`]'s order.
pub(super) fn settings_labels(game: &Game<'_>) -> Vec<String> {
    let mut labels = vec![
        format!(
            "Panel clicks give items in Creative: {}",
            on_off(flag(game, SETTING_GIVE))
        ),
        format!(
            "Mod name in tooltips: {}",
            on_off(flag(game, SETTING_MOD_NAMES))
        ),
    ];
    let modes = search_modes(game);
    for (field, (_, prefix, name)) in model::SearchModes::FIELDS.iter().enumerate() {
        let mode = match modes.get(field) {
            model::SearchMode::Enabled => "Enabled",
            model::SearchMode::RequirePrefix => "Require prefix",
            model::SearchMode::Disabled => "Disabled",
        };
        labels.push(format!("Search {name} ({prefix}): {mode}"));
    }
    labels
}

pub(super) fn settings_click(game: &mut Game<'_>, button: usize) {
    match button {
        0 | 1 => {
            let key = [SETTING_GIVE, SETTING_MOD_NAMES][button];
            let on = flag(game, key);
            game.native_set_setting(model::ID, key, if on { "false" } else { "true" });
            load_flags(game);
        }
        n if n - 2 < model::SearchModes::FIELDS.len() => {
            let field = n - 2;
            let mut modes = search_modes(game);
            let next = modes.get(field).next();
            modes.set(field, next);
            game.native_set_setting(model::ID, model::SearchModes::FIELDS[field].0, next.name());
            let known = game.native_known_recipes();
            let updates = with_browser(|browser| {
                browser.handle(model::Event::SetSearchModes(modes), false, &known)
            })
            .unwrap_or_default();
            for update in updates {
                match update {
                    model::Update::ItemList(items) => {
                        game.aei_show_search_result(items.iter().map(native).collect());
                    }
                    other => publish(game, other),
                }
            }
        }
        _ => {}
    }
}

/// JEI's Backspace: back to the previous lookup, when a recipe page is open.
pub(super) fn back(game: &mut Game<'_>) -> bool {
    if !game.native_on(model::ID) || game.panel.recipes.is_none() {
        return false;
    }
    let known = game.native_known_recipes();
    let updates = with_browser(|browser| browser.handle(model::Event::Back, false, &known))
        .unwrap_or_default();
    game.aei_reset_viewer();
    for update in updates {
        publish(game, update);
    }
    true
}

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
static CHEAT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static CENTERED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static COLUMNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(9);

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
            // The English name too, for players who follow English guides.
            let english = game.pack.names.item(&id);
            let aliases = if english.to_lowercase() != name.to_lowercase() {
                vec![english]
            } else {
                Vec::new()
            };
            let info = game.items().get(&id);
            let props = game.enchantments().props(&id);
            let rarity = horizon_client::item_tooltip::rarity(
                stack,
                props.map_or("common", |p| p.rarity.as_str()),
            );
            model::Entry {
                stack: model::Stack {
                    id: id.clone(),
                    count: 1,
                    components,
                },
                name,
                tooltip: lines.collect(),
                tabs: tabs.unwrap_or_default(),
                aliases,
                props: model::EntryProps {
                    max_stack: info.map_or(64, |i| i.max_stack),
                    durability: info.map_or(0, |i| i.max_damage),
                    nutrition: info.and_then(|i| i.food).map(|(points, _)| points),
                    rarity: rarity.to_owned(),
                    enchantable: game.enchantments().enchantable(&id) > 0,
                    block: info.is_some_and(|i| i.block.is_some()),
                },
            }
        })
        .collect()
}

/// JEI's fuel and compostable categories, which the recipe tables don't hold: every item
/// that burns in a furnace (with its burn time) or goes into a composter (with its
/// chance), in registry order.
fn fuels_and_compostables(game: &Game<'_>, items: &[NativeItem]) -> Vec<model::Recipe> {
    let registry = game.items();
    let providers = game.res.providers();
    let mut fuels = Vec::new();
    let mut composts = Vec::new();
    for (index, native) in items.iter().enumerate() {
        let Some(info) = registry.get(&native.id) else {
            continue;
        };
        let input = vec![vec![model::Ingredient {
            item: index,
            count: 1,
            components: String::new(),
        }]];
        let card = |kind, id: String, ticks, experience| model::Recipe {
            id,
            kind,
            width: 0,
            height: 0,
            inputs: input.clone(),
            outputs: Vec::new(),
            ticks,
            experience,
            book_id: -1,
        };
        if info.fuel.is_some() {
            let stack = horizon_client::items::ItemStack::new(native.id.as_str(), 1);
            let ticks = providers.burn_time(registry, &stack, false);
            if ticks > 0 {
                let id = format!("aei:fuel/{}", native.id);
                fuels.push(card(model::RecipeKind::Fuel, id, ticks as u32, 0.0));
            }
        }
        if let Some(chance) = info.compostable.as_ref().and_then(compost_chance) {
            let id = format!("aei:compost/{}", native.id);
            composts.push(card(model::RecipeKind::Compost, id, 0, chance));
        }
    }
    fuels.extend(composts);
    fuels
}

/// JEI's anvil category, worked out by the anvil's own rules (`stations::anvil_result`):
/// each damageable item repaired with its material and with a second damaged copy, then
/// every item an enchantment applies to combined with that enchantment's highest-level
/// book from the creative tabs. The cost is the anvil's level cost.
fn anvil_recipes(game: &Game<'_>, items: &[NativeItem]) -> Vec<model::Recipe> {
    use horizon_client::items::ItemStack;
    let registry = game.items();
    let enchantments = game.enchantments();
    let index: std::collections::HashMap<&str, usize> = items
        .iter()
        .enumerate()
        .map(|(i, item)| (item.id.as_str(), i))
        .collect();
    let ingredient = |stack: &ItemStack| {
        Some(model::Ingredient {
            item: *index.get(&*stack.item)?,
            count: stack.count,
            components: super::native_recipes::components(stack),
        })
    };
    let mut recipes = Vec::new();
    let mut combine = |left: &ItemStack, right: &[ItemStack]| {
        let first = right.first()?;
        let result = horizon_client::stations::anvil_result(
            enchantments,
            registry,
            Some(left),
            Some(first),
            None,
            "",
            false,
        );
        let output = result.result?;
        if result.cost <= 0 {
            return None;
        }
        let rights: Vec<_> = right.iter().filter_map(ingredient).collect();
        recipes.push(model::Recipe {
            id: format!("aei:anvil/{}", recipes.len()),
            kind: model::RecipeKind::Anvil,
            width: 2,
            height: 1,
            inputs: vec![vec![ingredient(left)?], rights],
            outputs: vec![ingredient(&output)?],
            ticks: result.cost as u32,
            experience: 0.0,
            book_id: -1,
        });
        Some(())
    };

    for native in items {
        let Some(info) = registry.get(&native.id).filter(|info| info.max_damage > 0) else {
            continue;
        };
        let mut damaged = ItemStack::new(native.id.as_str(), 1);
        damaged.damage = info.max_damage * 3 / 4;
        if let Some(props) = enchantments
            .props(&native.id)
            .filter(|props| !props.repairable.is_empty())
        {
            let materials: Vec<ItemStack> = items
                .iter()
                .filter(|m| {
                    horizon_client::enchant::Enchantments::item_in(
                        registry,
                        &props.repairable,
                        &m.id,
                    )
                })
                .map(|m| ItemStack::new(m.id.as_str(), 1))
                .collect();
            combine(&damaged, &materials);
        }
        combine(&damaged, std::slice::from_ref(&damaged));
    }

    // The highest level of each enchantment's book, as the creative tabs list them.
    let mut books: Vec<(std::sync::Arc<str>, u32, ItemStack)> = Vec::new();
    for stack in game.creative_screen.search_items() {
        if &*stack.item != "minecraft:enchanted_book" {
            continue;
        }
        let [(id, level)] = stack.stored_enchantments() else {
            continue;
        };
        match books.iter_mut().find(|(known, _, _)| known == id) {
            Some(entry) if entry.1 < *level => *entry = (id.clone(), *level, stack.clone()),
            Some(_) => {}
            None => books.push((id.clone(), *level, stack.clone())),
        }
    }
    for (enchantment, _, book) in &books {
        for native in items {
            if enchantments.can_enchant(registry, enchantment, &native.id) {
                let item = ItemStack::new(native.id.as_str(), 1);
                combine(&item, std::slice::from_ref(book));
            }
        }
    }
    recipes
}

/// The chance a compostable item's `layers` provider adds a layer to a composter that
/// isn't empty (an empty one always takes it): a plain number, vanilla's named
/// providers, or a weighted list of 0 and 1 layers.
fn compost_chance(layers: &serde_json::Value) -> Option<f32> {
    use serde_json::Value;
    match layers {
        Value::Number(n) => Some(if n.as_f64()? >= 1.0 { 1.0 } else { 0.0 }),
        Value::String(name) => Some(match name.rsplit('/').next()? {
            "always_add_one" => 1.0,
            "low" => 0.3,
            "low_medium" => 0.5,
            "medium" => 0.65,
            "medium_high" => 0.85,
            "high" => 1.0,
            _ => return None,
        }),
        Value::Object(map) => {
            let list = map.get("default").unwrap_or(layers);
            let entries = list.get("distribution")?.as_array()?;
            let weight = |e: &Value| e.get("weight").and_then(Value::as_f64).unwrap_or(1.0);
            let total: f64 = entries.iter().map(weight).sum();
            let adding: f64 = entries
                .iter()
                .filter(|e| e.get("data").and_then(Value::as_i64).unwrap_or(0) > 0)
                .map(weight)
                .sum();
            (total > 0.0).then(|| (adding / total) as f32)
        }
        _ => None,
    }
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
        model::Update::Give { stack, count } => give(game, &stack, count),
        model::Update::Hidden => {
            if let Some(text) = with_browser(|browser| browser.hidden_text()) {
                game.native_set_setting(model::ID, SETTING_HIDDEN_ITEMS, &text);
            }
        }
        model::Update::PlaceKnownRecipe(id) => {
            match game.aei_place_recipe(id, game.has_shift_down()) {
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
            let mut recipes: Vec<model::Recipe> = tables.recipes.iter().map(recipe).collect();
            recipes.extend(fuels_and_compostables(game, &tables.items));
            recipes.extend(anvil_recipes(game, &tables.items));
            let (mut browser, _) =
                model::Browser::new(tables.items.iter().map(item).collect(), recipes);
            browser.handle(model::Event::SetSearchModes(search_modes(game)), false, &[]);
            browser.handle(model::Event::SetSort(sort_order(game)), false, &[]);
            browser.handle(
                model::Event::SetEditMode(flag(game, SETTING_EDIT)),
                false,
                &[],
            );
            let hidden = game
                .native_setting(model::ID, SETTING_HIDDEN_ITEMS)
                .unwrap_or_default();
            browser.load_hidden(&hidden);
            let history = game
                .native_setting(model::ID, SETTING_SEARCH_HISTORY)
                .unwrap_or_default();
            browser.load_search_history(&history);
            // JEI's editable category order: written once with every category, then read.
            match game.native_setting(model::ID, SETTING_CATEGORIES) {
                Some(text) => browser.load_categories(&text),
                None => {
                    let text = browser.categories_text();
                    game.native_set_setting(model::ID, SETTING_CATEGORIES, &text);
                }
            }
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
        NativePanelEvent::Clicked { index, .. } if flag(game, SETTING_EDIT) => {
            model::Event::ToggleHidden {
                index: *index as usize,
                wildcard: game.has_control_down(),
            }
        }
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

    let known = game.native_known_recipes();
    let give_enabled = flag(game, SETTING_GIVE);
    let before = with_browser(|browser| browser.search_history_text());
    let updates =
        with_browser(|browser| browser.handle(event, give_enabled, &known)).unwrap_or_default();
    let after = with_browser(|browser| browser.search_history_text());
    if let Some(text) = after.filter(|text| Some(text) != before.as_ref()) {
        game.native_set_setting(model::ID, SETTING_SEARCH_HISTORY, &text);
    }
    for update in updates {
        if matches!(&update, model::Update::RecipeList(_)) {
            game.aei_reset_viewer();
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
const SETTING_CURSOR: &str = "give_cursor";
const SETTING_EDIT: &str = "edit";
const SETTING_CENTERED: &str = "centered_search";
const SETTING_COLUMNS: &str = "columns";
const SETTING_SORT: &str = "sort";
const SETTING_HIDDEN_ITEMS: &str = "hidden_items";
const SETTING_CATEGORIES: &str = "categories";
const SETTING_SEARCH_HISTORY: &str = "search_history";

fn flag(game: &Game<'_>, key: &str) -> bool {
    game.native_setting(model::ID, key).as_deref() == Some("true")
}

fn load_flags(game: &Game<'_>) {
    use std::sync::atomic::Ordering::Relaxed;
    HIDDEN.store(flag(game, SETTING_HIDDEN), Relaxed);
    MOD_NAMES.store(flag(game, SETTING_MOD_NAMES), Relaxed);
    CHEAT.store(flag(game, SETTING_GIVE), Relaxed);
    CENTERED.store(flag(game, SETTING_CENTERED), Relaxed);
    COLUMNS.store(columns(game), Relaxed);
}

fn columns(game: &Game<'_>) -> usize {
    game.native_setting(model::ID, SETTING_COLUMNS)
        .and_then(|value| value.parse().ok())
        .filter(|n| (1..=9).contains(n))
        .unwrap_or(9)
}

/// The item list options the settings page chose.
pub(super) fn list_options() -> model::viewer::ListOptions {
    use std::sync::atomic::Ordering::Relaxed;
    model::viewer::ListOptions {
        max_columns: COLUMNS.load(Relaxed),
        centered_search: CENTERED.load(Relaxed),
    }
}

/// Whether cheat mode is on (clicks give items).
pub(super) fn cheat_mode() -> bool {
    CHEAT.load(std::sync::atomic::Ordering::Relaxed)
}

/// JEI's Ctrl+click on the settings button: cheat mode on or off.
pub(super) fn toggle_cheat(game: &mut Game<'_>) {
    let on = !flag(game, SETTING_GIVE);
    game.native_set_setting(model::ID, SETTING_GIVE, if on { "true" } else { "false" });
    load_flags(game);
}

/// Cheat mode's give, at most one full stack. A creative player gets it as the creative
/// inventory hands it out: on the cursor on the creative screen when that setting is on,
/// else into the inventory. Anyone else sends `/give`, which the server allows only to
/// players with that permission.
fn give(game: &mut Game<'_>, stack: &model::Stack, count: u32) {
    let Some(max) = game.items().get(&stack.id).map(|item| item.max_stack) else {
        return;
    };
    let count = count.clamp(1, max.max(1));
    if !game.creative {
        let command = format!("/give @s {}{} {count}", stack.id, stack.components);
        game.handle_chat_input(&command);
        return;
    }
    let mut native = native(stack);
    native.count = count;
    if !(flag(game, SETTING_CURSOR) && game.aei_pick_up(&native)) {
        game.mod_give(&stack.id, count, &stack.components);
    }
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

/// One button of the settings page.
#[derive(Clone, Copy)]
enum Setting {
    /// An ON/OFF flag: its key and label.
    Flag(&'static str, &'static str),
    Sort,
    Columns,
    /// The search mode of [`model::SearchModes::FIELDS`]'s field.
    Search(usize),
}

/// The settings page's buttons, in order.
fn settings() -> Vec<Setting> {
    let mut list = vec![
        Setting::Flag(SETTING_GIVE, "Cheat mode (clicks give items)"),
        Setting::Flag(SETTING_EDIT, "Edit mode (clicks hide items)"),
        Setting::Sort,
        Setting::Columns,
        Setting::Flag(SETTING_CENTERED, "Search bar under the screen"),
        Setting::Flag(SETTING_MOD_NAMES, "Mod name in tooltips"),
        Setting::Flag(SETTING_CURSOR, "Cheat items in Creative go to the cursor"),
    ];
    list.extend((0..model::SearchModes::FIELDS.len()).map(Setting::Search));
    list
}

fn sort_order(game: &Game<'_>) -> model::SortOrder {
    game.native_setting(model::ID, SETTING_SORT)
        .and_then(|value| model::SortOrder::parse(&value))
        .unwrap_or_default()
}

pub(super) fn settings_labels(game: &Game<'_>) -> Vec<String> {
    let modes = search_modes(game);
    settings()
        .into_iter()
        .map(|setting| match setting {
            Setting::Flag(key, label) => format!("{label}: {}", on_off(flag(game, key))),
            Setting::Sort => {
                let order = match sort_order(game) {
                    model::SortOrder::Creative => "Creative tabs",
                    model::SortOrder::Mod => "Mod",
                    model::SortOrder::Name => "Name",
                };
                format!("Item list order: {order}")
            }
            Setting::Columns => format!("Item list columns: at most {}", columns(game)),
            Setting::Search(field) => {
                let (_, prefix, name) = model::SearchModes::FIELDS[field];
                let mode = match modes.get(field) {
                    model::SearchMode::Enabled => "Enabled",
                    model::SearchMode::RequirePrefix => "Require prefix",
                    model::SearchMode::Disabled => "Disabled",
                };
                format!("Search {name} ({prefix}): {mode}")
            }
        })
        .collect()
}

pub(super) fn settings_click(game: &mut Game<'_>, button: usize) {
    let Some(setting) = settings().get(button).copied() else {
        return;
    };
    let event = match setting {
        Setting::Flag(key, _) => {
            let on = !flag(game, key);
            game.native_set_setting(model::ID, key, if on { "true" } else { "false" });
            load_flags(game);
            if key != SETTING_EDIT {
                return;
            }
            model::Event::SetEditMode(on)
        }
        Setting::Columns => {
            // 9, 8, ... 4, then 9 again.
            let next = if columns(game) <= 4 {
                9
            } else {
                columns(game) - 1
            };
            game.native_set_setting(model::ID, SETTING_COLUMNS, &next.to_string());
            load_flags(game);
            return;
        }
        Setting::Sort => {
            let next = sort_order(game).next();
            game.native_set_setting(model::ID, SETTING_SORT, next.name());
            model::Event::SetSort(next)
        }
        Setting::Search(field) => {
            let mut modes = search_modes(game);
            let next = modes.get(field).next();
            modes.set(field, next);
            game.native_set_setting(model::ID, model::SearchModes::FIELDS[field].0, next.name());
            model::Event::SetSearchModes(modes)
        }
    };
    let known = game.native_known_recipes();
    let updates = with_browser(|browser| browser.handle(event, false, &known)).unwrap_or_default();
    for update in updates {
        match update {
            model::Update::ItemList(items) => {
                game.aei_show_search_result(items.iter().map(native).collect());
            }
            other => publish(game, other),
        }
    }
}

/// Whether the shown list entry `index` is hidden (edit mode marks it).
pub(super) fn shown_hidden(index: usize) -> bool {
    with_browser(|browser| browser.shown_hidden(index)).unwrap_or(false)
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

/// A lookup started by AEI's own drawing (a catalyst): recipes, or uses when `uses`. In
/// cheat mode it gives the stack, as other ingredient clicks do.
pub(super) fn lookup(game: &mut Game<'_>, target: &NativeStack, uses: bool) {
    let event = model::Event::ClickRecipeIngredient {
        stack: stack(target),
        right: uses,
    };
    bookmark_event(game, event);
}

/// The shown category's recipe window: its offset and the category's total.
pub(super) fn recipe_window() -> (usize, usize) {
    with_browser(|browser| browser.shown_window()).unwrap_or((0, 0))
}

/// Shows another window of a long category, keeping the recipe window's own page.
pub(super) fn show_window(game: &mut Game<'_>, offset: usize, len: usize) {
    let known = game.native_known_recipes();
    let updates = with_browser(|browser| {
        browser.handle(model::Event::ShowWindow { offset, len }, false, &known)
    })
    .unwrap_or_default();
    for update in updates {
        publish(game, update);
    }
}

/// Up and Down in the search field recall earlier searches; Enter keeps the current
/// one. True when the key was used.
pub(super) fn search_key(game: &mut Game<'_>, key: &str) -> bool {
    if !game.native_on(model::ID) || !game.mod_browser_typing() {
        return false;
    }
    let older = match key {
        "key.keyboard.up" => true,
        "key.keyboard.down" => false,
        "key.keyboard.enter" | "key.keyboard.keypad.enter" => {
            save_search_history(game, |browser| browser.remember_search());
            return false;
        }
        _ => return false,
    };
    let Some(recalled) = with_browser(|browser| browser.recall_search(older)) else {
        return false;
    };
    game.aei_set_search(recalled.as_deref().unwrap_or(""));
    true
}

/// Runs `change` on the history and saves it when it changed.
fn save_search_history(game: &mut Game<'_>, change: impl FnOnce(&mut model::Browser) -> bool) {
    if let Some(text) =
        with_browser(|browser| change(browser).then(|| browser.search_history_text())).flatten()
    {
        game.native_set_setting(model::ID, SETTING_SEARCH_HISTORY, &text);
    }
}

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

std::thread_local! {
    static BROWSER: std::cell::RefCell<Option<Session>> = const { std::cell::RefCell::new(None) };
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
            if let Err(message) = game.native_place_recipe(id, game.has_shift_down()) {
                game.native_chat(&message);
            }
        }
    }
}

fn clear(game: &mut Game<'_>) {
    let had_session = BROWSER.with(|cell| cell.borrow_mut().take().is_some());
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
            let (browser, initial) = model::Browser::new(
                tables.items.iter().map(item).collect(),
                tables.recipes.iter().map(recipe).collect(),
            );
            *session = Some(Session {
                tables,
                known,
                browser,
            });
            return Some((true, initial));
        }
        let session = session.as_mut()?;
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
            if is_back {
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
    let give_enabled = game.native_setting(model::ID, "give").as_deref() == Some("true");
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

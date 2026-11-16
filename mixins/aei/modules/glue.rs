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
static RECENT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static ASSIST: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static MARK_NEW: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static COUNTS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static HIGHLIGHT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static PREVIEW: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static CENTERED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static COLUMNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(9);

std::thread_local! {
    static BROWSER: std::cell::RefCell<Option<Session>> = const { std::cell::RefCell::new(None) };
    /// The bookmarks as the overlay draws them: each stack, and whether it is a recipe's.
    static BOOKMARKS: std::cell::RefCell<Vec<(NativeStack, bool)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

const SETTING_BOOKMARKS: &str = "bookmarks";
const SETTING_BOOKMARK_LIST: &str = "bookmark_list";
/// Bookmark lists (collections) a player can switch between.
const BOOKMARK_LISTS: usize = 3;
static BOOKMARK_LIST: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// The settings key of the shown bookmark list: `bookmarks`, `bookmarks_2`, ...
fn bookmarks_key() -> String {
    match BOOKMARK_LIST.load(std::sync::atomic::Ordering::Relaxed) {
        0 => SETTING_BOOKMARKS.to_owned(),
        n => format!("{SETTING_BOOKMARKS}_{}", n + 1),
    }
}

/// Which bookmark list shows (0 for the first).
pub(super) fn bookmark_list() -> usize {
    BOOKMARK_LIST.load(std::sync::atomic::Ordering::Relaxed)
}

/// Shows the next bookmark list (a collection), wrapping after the last.
pub(super) fn cycle_bookmark_list(game: &mut Game<'_>) {
    let next = (bookmark_list() + 1) % BOOKMARK_LISTS;
    BOOKMARK_LIST.store(next, std::sync::atomic::Ordering::Relaxed);
    game.native_set_setting(model::ID, SETTING_BOOKMARK_LIST, &next.to_string());
    let saved = game
        .native_setting(model::ID, &bookmarks_key())
        .unwrap_or_default();
    if let Some(model::Update::Bookmarks(list)) =
        with_browser(|browser| browser.load_bookmarks(&saved))
    {
        BOOKMARKS.with(|cell| {
            *cell.borrow_mut() = list
                .iter()
                .map(|b| (native(&b.stack), b.recipe.is_some()))
                .collect();
        });
    }
}

std::thread_local! {
    /// The found items as last saved, so they are written only when they change.
    static SAVED_SEEN: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

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
    save_recent(game);
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
        // JEI lists the fluids after the items.
        .chain(FLUIDS.iter().enumerate().map(|(i, (id, _, _))| {
            let name = fluid_name(game, i);
            let english = game
                .pack
                .names
                .get(FLUIDS[i].1)
                .map(str::to_owned)
                .unwrap_or_default();
            model::Entry {
                stack: model::Stack {
                    id: (*id).to_owned(),
                    count: 1,
                    components: String::new(),
                },
                aliases: if english.is_empty() || english.to_lowercase() == name.to_lowercase() {
                    Vec::new()
                } else {
                    vec![english]
                },
                name,
                tooltip: vec!["Fluid".to_owned()],
                tabs: Vec::new(),
                props: model::EntryProps {
                    max_stack: 1,
                    rarity: "common".to_owned(),
                    block: true,
                    ..Default::default()
                },
            }
        }))
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

/// The fluids AEI lists after the items, as JEI does: their id, their name's language
/// key and the bucket that holds them. Their icons are the first frame of the still
/// textures, uploaded as AEI's own HUD image (`native_mod_image`), side by side.
const FLUIDS: [(&str, &str, &str); 2] = [
    (
        "minecraft:water",
        "block.minecraft.water",
        "minecraft:water_bucket",
    ),
    (
        "minecraft:lava",
        "block.minecraft.lava",
        "minecraft:lava_bucket",
    ),
];

/// Which of [`FLUIDS`] `id` is.
pub(super) fn fluid(id: &str) -> Option<usize> {
    FLUIDS.iter().position(|(fluid, _, _)| *fluid == id)
}

/// A fluid's name in the game's language.
pub(super) fn fluid_name(game: &Game<'_>, i: usize) -> String {
    game.pack
        .names
        .display_get(FLUIDS[i].1)
        .unwrap_or_else(|| FLUIDS[i].0.trim_start_matches("minecraft:").to_owned())
}

/// Where fluid `i`'s icon sits in AEI's HUD image (`[x, y, w, h]` in its pixels).
pub(super) fn fluid_sprite(i: usize) -> [f32; 4] {
    [i as f32 * 16.0, 0.0, 16.0, 16.0]
}

/// Uploads the fluids' icons: each still texture's first frame scaled to 16x16, water
/// tinted with its plain colour (0x3f76e4) as the game tints it.
fn upload_fluid_sprites(game: &mut Game<'_>) {
    let Some(root) = resource_root() else {
        return;
    };
    let width = 16 * FLUIDS.len();
    let mut image = vec![0u8; width * 16 * 4];
    for (i, (texture, tint)) in [
        ("water_still", [0x3f, 0x76, 0xe4]),
        ("lava_still", [255; 3]),
    ]
    .iter()
    .enumerate()
    {
        let path = root
            .join("assets/minecraft/textures/block")
            .join(format!("{texture}.png"));
        let Ok((w, h, rgba)) = horizon_client::hud::decode(&path) else {
            continue;
        };
        // An animation's frames stack downwards, each as wide as the texture.
        let side = w.min(h) as usize;
        if side == 0 {
            continue;
        }
        for y in 0..16 {
            for x in 0..16 {
                let from = ((y * side / 16) * w as usize + x * side / 16) * 4;
                let to = (y * width + i * 16 + x) * 4;
                for c in 0..3 {
                    image[to + c] = (u32::from(rgba[from + c]) * tint[c] / 255) as u8;
                }
                image[to + 3] = rgba[from + 3];
            }
        }
    }
    game.native_mod_image(width as u32, 16, &image);
}

/// Beyond JEI: the fluids at work in the world, two inputs to one output: a bucket or
/// a glass bottle filled, concrete powder set in water, water meeting lava.
fn fluid_recipes(game: &Game<'_>, items: &[NativeItem]) -> Vec<model::Recipe> {
    let index: std::collections::HashMap<&str, usize> = items
        .iter()
        .enumerate()
        .map(|(i, item)| (item.id.as_str(), i))
        .collect();
    // The fluids come after the items.
    let (water, lava) = (items.len(), items.len() + 1);
    let one = |item: usize, components: String| model::Ingredient {
        item,
        count: 1,
        components,
    };
    let mut recipes = Vec::new();
    let mut add =
        |id: String, first: model::Ingredient, second: usize, output: model::Ingredient| {
            recipes.push(model::Recipe {
                id: format!("aei:fluid/{id}"),
                kind: model::RecipeKind::Fluid,
                width: 2,
                height: 1,
                inputs: vec![vec![first], vec![one(second, String::new())]],
                outputs: vec![output],
                ticks: 0,
                experience: 0.0,
                book_id: -1,
            });
        };
    let item = |id: &str| index.get(id).copied();
    if let Some(bucket) = item("minecraft:bucket") {
        for (fluid, (_, _, filled)) in [water, lava].into_iter().zip(FLUIDS) {
            if let Some(filled) = item(filled) {
                let name = filled.to_owned();
                add(
                    items[name].id.trim_start_matches("minecraft:").to_owned(),
                    one(bucket, String::new()),
                    fluid,
                    one(name, String::new()),
                );
            }
        }
    }
    // The water bottle, as the creative tabs list it.
    if let (Some(bottle), Some(potion)) = (item("minecraft:glass_bottle"), item("minecraft:potion"))
        && let Some(components) = game
            .creative_screen
            .search_items()
            .iter()
            .filter(|stack| &*stack.item == "minecraft:potion")
            .map(super::native_recipes::components)
            .find(|c| c.contains("\"minecraft:water\""))
    {
        add(
            "water_bottle".to_owned(),
            one(bottle, String::new()),
            water,
            one(potion, components),
        );
    }
    for (i, native) in items.iter().enumerate() {
        if let Some(color) = native.id.strip_suffix("_concrete_powder")
            && let Some(concrete) = item(&format!("{color}_concrete"))
        {
            let name = items[concrete]
                .id
                .trim_start_matches("minecraft:")
                .to_owned();
            add(
                name,
                one(i, String::new()),
                water,
                one(concrete, String::new()),
            );
        }
    }
    // Lava meeting water: a source turns to obsidian, flowing lava to cobblestone, lava
    // flowing onto water to stone.
    for made in ["obsidian", "cobblestone", "stone"] {
        if let Some(output) = item(&format!("minecraft:{made}")) {
            add(
                made.to_owned(),
                one(lava, String::new()),
                water,
                one(output, String::new()),
            );
        }
    }
    recipes
}

/// The game's resource folder, as the launcher passes it (`--resources`).
fn resource_root() -> Option<std::path::PathBuf> {
    let mut args = std::env::args();
    while let Some(arg) = args.next() {
        if arg == "--resources" {
            return args.next().map(std::path::PathBuf::from);
        }
        if let Some(path) = arg.strip_prefix("--resources=") {
            return Some(path.into());
        }
    }
    None
}

/// The item tag files of every namespace in the game's resource folder: each tag's id
/// and its `values` (`#` before nested tags; an optional entry's `id`).
fn item_tag_files() -> std::collections::HashMap<String, Vec<String>> {
    fn walk(
        dir: &std::path::Path,
        prefix: &str,
        namespace: &str,
        out: &mut std::collections::HashMap<String, Vec<String>>,
    ) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if path.is_dir() {
                walk(&path, &format!("{prefix}{name}/"), namespace, out);
            } else if let Some(stem) = name.strip_suffix(".json")
                && let Some(json) = std::fs::read_to_string(&path)
                    .ok()
                    .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            {
                let values = json["values"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|value| value.as_str().or_else(|| value["id"].as_str()))
                    .map(str::to_owned)
                    .collect();
                out.insert(format!("{namespace}:{prefix}{stem}"), values);
            }
        }
    }
    let mut out = std::collections::HashMap::new();
    let Some(root) = resource_root() else {
        return out;
    };
    for namespace in std::fs::read_dir(root.join("data"))
        .into_iter()
        .flatten()
        .flatten()
    {
        let name = namespace.file_name().to_string_lossy().into_owned();
        walk(&namespace.path().join("tags/item"), "", &name, &mut out);
    }
    out
}

/// Every item a loot table's entries can give (`minecraft:item` entries, nested ones
/// included).
fn loot_items(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            if map.get("type").and_then(|t| t.as_str()) == Some("minecraft:item")
                && let Some(name) = map.get("name").and_then(|n| n.as_str())
                && !out.iter().any(|known| known == name)
            {
                out.push(name.to_owned());
            }
            for child in map.values() {
                loot_items(child, out);
            }
        }
        serde_json::Value::Array(list) => {
            for child in list {
                loot_items(child, out);
            }
        }
        _ => {}
    }
}

/// Beyond JEI: what breaking each block can drop, read from its loot table, for the
/// blocks that drop something other than themselves.
fn block_drops(game: &Game<'_>, items: &[NativeItem]) -> Vec<model::Recipe> {
    let Some(root) = resource_root() else {
        return Vec::new();
    };
    let registry = game.items();
    let index: std::collections::HashMap<&str, usize> = items
        .iter()
        .enumerate()
        .map(|(i, item)| (item.id.as_str(), i))
        .collect();
    let mut recipes = Vec::new();
    for (i, native) in items.iter().enumerate() {
        let Some(state) = registry.get(&native.id).and_then(|info| info.block) else {
            continue;
        };
        let Some(table) = registry.block_loot_table(game.pack.assets.registry, state) else {
            continue;
        };
        let (namespace, path) = table.split_once(':').unwrap_or(("minecraft", &table));
        let file = root
            .join("data")
            .join(namespace)
            .join("loot_table")
            .join(format!("{path}.json"));
        let Some(json) = std::fs::read_to_string(&file)
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        else {
            continue;
        };
        let mut drops = Vec::new();
        loot_items(&json, &mut drops);
        // One card per drop; the block itself (silk touch) is left out.
        for drop in drops.iter().filter(|d| **d != native.id) {
            if let Some(&output) = index.get(drop.as_str()) {
                recipes.push(loot_recipe(
                    model::RecipeKind::Drop,
                    format!("aei:drops/{}#{drop}", native.id),
                    i,
                    output,
                ));
            }
        }
    }
    recipes
}

/// A loot card: `input` (a block, a mob's egg, a chest) gives `output`.
fn loot_recipe(kind: model::RecipeKind, id: String, input: usize, output: usize) -> model::Recipe {
    let one = |item| model::Ingredient {
        item,
        count: 1,
        components: String::new(),
    };
    model::Recipe {
        id,
        kind,
        width: 1,
        height: 1,
        inputs: vec![vec![one(input)]],
        outputs: vec![one(output)],
        ticks: 0,
        experience: 0.0,
        book_id: -1,
    }
}

/// Every loot table file under `dir`, with its path below it (no `.json`).
fn loot_files(dir: &std::path::Path, prefix: &str, out: &mut Vec<(String, serde_json::Value)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            loot_files(&path, &format!("{prefix}{name}/"), out);
        } else if let Some(stem) = name.strip_suffix(".json")
            && let Some(json) = std::fs::read_to_string(&path)
                .ok()
                .and_then(|text| serde_json::from_str(&text).ok())
        {
            out.push((format!("{prefix}{stem}"), json));
        }
    }
}

/// Beyond JEI: what mobs drop (each mob shown as its spawn egg) and what the other loot
/// tables give (chests, barrels, archaeology, fishing, bartering, shearing), read from
/// the loot tables in the game's resource folder; one card per item.
fn world_loot(items: &[NativeItem]) -> Vec<model::Recipe> {
    let Some(root) = resource_root() else {
        return Vec::new();
    };
    let index: std::collections::HashMap<&str, usize> = items
        .iter()
        .enumerate()
        .map(|(i, item)| (item.id.as_str(), i))
        .collect();
    let mut recipes = Vec::new();
    for namespace in std::fs::read_dir(root.join("data"))
        .into_iter()
        .flatten()
        .flatten()
    {
        let ns = namespace.file_name().to_string_lossy().into_owned();
        let tables = namespace.path().join("loot_table");
        // Mobs: `entities/<mob>.json`, or `entities/<mob>/<variant>.json` (sheep wool).
        let mut mobs = Vec::new();
        loot_files(&tables.join("entities"), "", &mut mobs);
        for (path, json) in &mobs {
            let mob = path.split('/').next().unwrap_or(path);
            let Some(&egg) = index.get(format!("{ns}:{mob}_spawn_egg").as_str()) else {
                continue;
            };
            let mut drops = Vec::new();
            loot_items(json, &mut drops);
            for drop in &drops {
                if let Some(&output) = index.get(drop.as_str()) {
                    recipes.push(loot_recipe(
                        model::RecipeKind::MobDrop,
                        format!("aei:mob/{ns}:{path}#{drop}"),
                        egg,
                        output,
                    ));
                }
            }
        }
        // Other tables, each under what finds it.
        for folder in ["chests", "barrels", "archaeology", "gameplay", "shearing"] {
            let mut files = Vec::new();
            loot_files(&tables.join(folder), &format!("{folder}/"), &mut files);
            for (path, json) in &files {
                let finder = match path.as_str() {
                    p if p.starts_with("barrels/") => "minecraft:barrel",
                    p if p.starts_with("archaeology/") => "minecraft:brush",
                    p if p.starts_with("shearing/") => "minecraft:shears",
                    p if p.starts_with("gameplay/fishing") => "minecraft:fishing_rod",
                    "gameplay/piglin_bartering" => "minecraft:gold_ingot",
                    "gameplay/sniffer_digging" => "minecraft:sniffer_egg",
                    _ => "minecraft:chest",
                };
                let Some(&input) = index.get(finder) else {
                    continue;
                };
                let mut drops = Vec::new();
                loot_items(json, &mut drops);
                for drop in &drops {
                    if let Some(&output) = index.get(drop.as_str()) {
                        recipes.push(loot_recipe(
                            model::RecipeKind::Loot,
                            format!("aei:loot/{path}#{drop}"),
                            input,
                            output,
                        ));
                    }
                }
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
                game.native_set_setting(model::ID, &bookmarks_key(), &text);
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
            recipes.extend(block_drops(game, &tables.items));
            recipes.extend(world_loot(&tables.items));
            recipes.extend(fluid_recipes(game, &tables.items));
            let mut items: Vec<model::ItemInfo> = tables.items.iter().map(item).collect();
            items.extend(
                FLUIDS
                    .iter()
                    .enumerate()
                    .map(|(i, (id, _, _))| model::ItemInfo {
                        id: (*id).to_owned(),
                        name: fluid_name(game, i),
                        tags: Vec::new(),
                    }),
            );
            upload_fluid_sprites(game);
            let (mut browser, _) = model::Browser::new(items, recipes);
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
            let recent = game
                .native_setting(model::ID, SETTING_RECENT_ITEMS)
                .unwrap_or_default();
            browser.load_recent(&recent);
            let seen = game
                .native_setting(model::ID, SETTING_SEEN)
                .unwrap_or_default();
            browser.load_seen(&seen);
            let trades = game
                .native_setting(model::ID, SETTING_TRADES)
                .unwrap_or_default();
            browser.load_trades(&trades);
            let notes = game
                .native_setting(model::ID, SETTING_NOTES)
                .unwrap_or_default();
            browser.load_notes(&notes);
            browser.handle(model::Event::SetKnown(known.to_vec()), false, &[]);
            browser.handle(
                model::Event::SetSpoilerFree(flag(game, SETTING_SPOILER)),
                false,
                &[],
            );
            browser.handle(
                model::Event::SetGrouping(flag(game, SETTING_GROUPS)),
                false,
                &[],
            );
            // JEI's editable category order: written once with every category, then read.
            match game.native_setting(model::ID, SETTING_CATEGORIES) {
                Some(text) => browser.load_categories(&text),
                None => {
                    let text = browser.categories_text();
                    game.native_set_setting(model::ID, SETTING_CATEGORIES, &text);
                }
            }
            load_flags(game);
            let list = game
                .native_setting(model::ID, SETTING_BOOKMARK_LIST)
                .and_then(|n| n.parse().ok())
                .filter(|n| *n < BOOKMARK_LISTS)
                .unwrap_or(0);
            BOOKMARK_LIST.store(list, std::sync::atomic::Ordering::Relaxed);
            let saved = game
                .native_setting(model::ID, &bookmarks_key())
                .unwrap_or_default();
            if let model::Update::Bookmarks(list) = browser.load_bookmarks(&saved) {
                BOOKMARKS.with(|cell| {
                    *cell.borrow_mut() = list
                        .iter()
                        .map(|b| (native(&b.stack), b.recipe.is_some()))
                        .collect();
                });
            }
            // The pinned recipes, by id, as the player left them.
            let pinned: Vec<_> = game
                .native_setting(model::ID, SETTING_PINNED)
                .unwrap_or_default()
                .lines()
                .filter_map(|id| browser.card_by_id(id.trim()))
                .collect();
            game.aei_set_pinned(pinned);
            let initial = browser.set_catalogue(catalogue(game));
            *session = Some(Session {
                tables,
                known,
                browser,
            });
            return Some((true, initial));
        }
        let session = session.as_mut()?;
        // `is:craftable` follows the inventory and the open grid.
        let stacks = game
            .inventory
            .iter()
            .flatten()
            .map(|held| model::Stack {
                id: held.item.to_string(),
                count: held.count,
                components: String::new(),
            })
            .collect();
        let grid = game.craft_size as u32;
        let changed =
            session
                .browser
                .handle(model::Event::SetInventory { stacks, grid }, false, &known);
        // Offers on an open trading screen are remembered as Trading recipes.
        if let Some(merchant) = game.station.as_ref().and_then(|s| s.merchant.as_ref()) {
            let stack = |s: &horizon_client::items::ItemStack| model::Stack {
                id: s.item.to_string(),
                count: s.count,
                components: super::native_recipes::components(s),
            };
            let trades: Vec<model::Trade> = merchant
                .session
                .offers
                .iter()
                .map(|offer| model::Trade {
                    cost: stack(&offer.base_cost_a.stack),
                    cost_b: offer.cost_b.as_ref().map(|c| stack(&c.stack)),
                    result: stack(&offer.result),
                })
                .collect();
            if session.browser.add_trades(&trades) {
                game.native_set_setting(model::ID, SETTING_TRADES, &session.browser.trades_text());
            }
        }
        let seen = session.browser.seen_text();
        if SAVED_SEEN.with(|saved| *saved.borrow() != seen) {
            game.native_set_setting(model::ID, SETTING_SEEN, &seen);
            SAVED_SEEN.with(|saved| *saved.borrow_mut() = seen);
        }
        if let Some(model::Update::ItemList(items)) = changed.into_iter().next() {
            return Some((false, model::Update::ItemList(items)));
        }
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
            let listed =
                session
                    .browser
                    .handle(model::Event::SetKnown(session.known.to_vec()), false, &[]);
            if let Some(update) = listed.into_iter().next() {
                return Some((false, update));
            }
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
        match update {
            // A search result refreshed by the inventory: an empty one keeps the field.
            model::Update::ItemList(items) if !reset => {
                game.aei_show_search_result(items.iter().map(native).collect());
            }
            update => publish(game, update),
        }
    }
}

pub(super) fn event(game: &mut Game<'_>, event: &NativePanelEvent) {
    if !game.native_on(model::ID) {
        return;
    }
    let event = match event {
        // A note being typed isn't a search.
        NativePanelEvent::Search(_) if game.aei_editing_note() => return,
        NativePanelEvent::Search(query) => model::Event::Search(query.clone()),
        NativePanelEvent::Clicked { index, .. } if flag(game, SETTING_EDIT) => {
            model::Event::ToggleHidden {
                index: *index as usize,
                wildcard: game.has_control_down(),
            }
        }
        NativePanelEvent::Clicked { index, .. }
            if game.has_shift_down() && shown_group(*index as usize).is_some() =>
        {
            model::Event::ToggleGroup(*index as usize)
        }
        NativePanelEvent::Clicked { index, right } => model::Event::ClickItem {
            index: *index as usize,
            right: *right,
        },
        NativePanelEvent::Key { key, stack: target }
            if key == "key.keyboard.c" && game.has_control_down() =>
        {
            copy_stack(game, target, game.has_shift_down());
            return;
        }
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
            let is_plan =
                *key == game.native_mod_key(model::ID, model::KEY_PLAN[0], model::KEY_PLAN[2]);
            if is_plan {
                game.aei_open_plan(target.clone());
                return;
            }
            if *key == game.native_mod_key(model::ID, model::KEY_INFO[0], model::KEY_INFO[2]) {
                game.aei_open_details(target.clone());
                return;
            }
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
    save_recent(game);
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
        // A loot card names its table on both slots.
        if let Some(id) = browser.shown_recipe_id(card)
            && let Some((table, _)) = id
                .strip_prefix("aei:loot/")
                .or_else(|| id.strip_prefix("aei:mob/"))
                .and_then(|rest| rest.split_once('#'))
        {
            lines.push(format!("\u{a7}7Loot table: {table}"));
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
const SETTING_RECENT: &str = "recent";
const SETTING_ASSIST: &str = "assisted_crafting";
const SETTING_SPOILER: &str = "spoiler_free";
const SETTING_NEW: &str = "mark_new";
const SETTING_SEEN: &str = "seen_items";
const SETTING_COUNTS: &str = "tooltip_counts";
const SETTING_HIGHLIGHT: &str = "highlight_inventory";
const SETTING_GROUPS: &str = "group_variants";
const SETTING_DARK: &str = "dark_theme";
const SETTING_NOTES: &str = "notes";
const SETTING_PINNED: &str = "pinned_recipes";
const SETTING_PREVIEW: &str = "shift_preview";
const SETTING_TRADES: &str = "trades";
const SETTING_RECENT_ITEMS: &str = "recent_items";

fn flag(game: &Game<'_>, key: &str) -> bool {
    game.native_setting(model::ID, key).as_deref() == Some("true")
}

fn load_flags(game: &Game<'_>) {
    use std::sync::atomic::Ordering::Relaxed;
    HIDDEN.store(flag(game, SETTING_HIDDEN), Relaxed);
    MOD_NAMES.store(flag(game, SETTING_MOD_NAMES), Relaxed);
    CHEAT.store(flag(game, SETTING_GIVE), Relaxed);
    RECENT.store(flag(game, SETTING_RECENT), Relaxed);
    ASSIST.store(flag(game, SETTING_ASSIST), Relaxed);
    MARK_NEW.store(flag(game, SETTING_NEW), Relaxed);
    COUNTS.store(flag(game, SETTING_COUNTS), Relaxed);
    HIGHLIGHT.store(flag(game, SETTING_HIGHLIGHT), Relaxed);
    PREVIEW.store(flag(game, SETTING_PREVIEW), Relaxed);
    game.aei_set_dark(flag(game, SETTING_DARK));
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
    // A fluid comes in its bucket.
    if let Some(i) = fluid(&stack.id) {
        let bucket = model::Stack {
            id: FLUIDS[i].2.to_owned(),
            count: 1,
            components: String::new(),
        };
        return give(game, &bucket, count);
    }
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
    /// Writes the diagnostic report.
    Report,
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
        Setting::Flag(SETTING_RECENT, "Recently viewed items row"),
        Setting::Flag(SETTING_ASSIST, "Calculator places the next step"),
        Setting::Flag(SETTING_SPOILER, "Spoiler-free (only items you found)"),
        Setting::Flag(SETTING_NEW, "Mark newly found items"),
        Setting::Flag(SETTING_COUNTS, "Recipe and use counts in tooltips"),
        Setting::Flag(SETTING_HIGHLIGHT, "Search dims other inventory items"),
        Setting::Flag(SETTING_GROUPS, "Fold item variants into groups"),
        Setting::Flag(SETTING_DARK, "Dark theme for AEI's windows"),
        Setting::Flag(SETTING_PREVIEW, "Shift over an item previews its recipe"),
        Setting::Report,
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
            Setting::Report => "Write diagnostic report (report.md)".to_owned(),
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
            match key {
                SETTING_EDIT => model::Event::SetEditMode(on),
                SETTING_SPOILER => model::Event::SetSpoilerFree(on),
                SETTING_GROUPS => model::Event::SetGrouping(on),
                _ => return,
            }
        }
        Setting::Report => {
            // Next to AEI's settings, in its mod data folder.
            let tags = item_tag_files();
            if let Some(report) = with_browser(|browser| {
                browser.load_tag_files(&tags);
                browser.report()
            }) {
                game.native_set_setting(model::ID, "report.md", &report);
            }
            return;
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
            if game.aei_finish_note() {
                return true;
            }
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

/// The breadcrumb trail: each lookup's stack, whether it shows uses, and the shown one.
pub(super) fn trail() -> (Vec<(NativeStack, bool)>, Option<usize>) {
    with_browser(|browser| {
        let (trail, current) = browser.trail();
        let trail = trail
            .iter()
            .map(|(stack, uses)| (native(stack), *uses))
            .collect();
        (trail, current)
    })
    .unwrap_or_default()
}

/// A breadcrumb click: lookup `index` of the trail again.
pub(super) fn jump(game: &mut Game<'_>, index: usize) {
    let known = game.native_known_recipes();
    let updates =
        with_browser(|browser| browser.handle(model::Event::JumpHistory(index), false, &known))
            .unwrap_or_default();
    game.aei_reset_viewer();
    for update in updates {
        publish(game, update);
    }
}

/// Ctrl+C over an item copies its id; Ctrl+Shift+C a `/give` command for the exact
/// stack, components included. The action bar says what was copied.
fn copy_stack(game: &mut Game<'_>, target: &NativeStack, give: bool) {
    use horizon_client::clipboard::Clipboard;
    let text = if give {
        format!("/give @s {}{}", target.item, target.components)
    } else {
        target.item.clone()
    };
    game.clipboard.set(&text);
    game.message = Some((format!("Copied {text}"), std::time::Instant::now()));
}

/// Whether the recently-viewed row is on (a setting, off by default).
pub(super) fn recent_enabled() -> bool {
    RECENT.load(std::sync::atomic::Ordering::Relaxed)
}

/// The recently looked-up items, newest first.
pub(super) fn recent() -> Vec<NativeStack> {
    with_browser(|browser| browser.recent().iter().map(native).collect()).unwrap_or_default()
}

/// Saves the recent items when they changed.
fn save_recent(game: &mut Game<'_>) {
    std::thread_local! {
        static SAVED: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
    }
    let Some(text) = with_browser(|browser| browser.recent_text()) else {
        return;
    };
    if SAVED.with(|saved| *saved.borrow() != text) {
        game.native_set_setting(model::ID, SETTING_RECENT_ITEMS, &text);
        SAVED.with(|saved| *saved.borrow_mut() = text);
    }
}

/// The material plan for the calculator's targets, after the player's inventory.
pub(super) fn plan(game: &Game<'_>, targets: &[(NativeStack, u32)]) -> Option<model::Plan> {
    let inventory: Vec<model::Stack> = game
        .inventory
        .iter()
        .flatten()
        .map(|held| model::Stack {
            id: held.item.to_string(),
            count: held.count,
            components: String::new(),
        })
        .collect();
    let targets: Vec<(model::Stack, u32)> = targets
        .iter()
        .map(|(target, count)| (stack(target), *count))
        .collect();
    with_browser(|browser| browser.plan_all(&targets, &inventory))
}

/// Whether assisted crafting is on (a setting, off by default).
pub(super) fn assist_enabled() -> bool {
    ASSIST.load(std::sync::atomic::Ordering::Relaxed)
}

/// Assisted crafting: the recipe-book id of the plan's next step the open grid can
/// take from the inventory now.
pub(super) fn next_step(plan: &model::Plan) -> Option<i32> {
    // The session keeps the player's known recipes, refreshed every frame.
    BROWSER.with(|cell| {
        let session = cell.borrow();
        let session = session.as_ref()?;
        session.browser.next_book_step(plan, &session.known)
    })
}

/// Places a step through the vanilla recipe book, one craft at a time.
pub(super) fn place_step(game: &mut Game<'_>, book_id: i32) {
    if let Err(message) = game.aei_place_recipe(book_id, false) {
        game.native_chat(&message);
    }
}

/// The details of `target` for the I key's panel.
pub(super) fn details(target: &NativeStack) -> Option<model::Details> {
    with_browser(|browser| browser.details(&stack(target))).flatten()
}

/// A category's unlocked and total recipe-book recipes, by its station id.
pub(super) fn progress(station: &str) -> Option<(usize, usize)> {
    BROWSER.with(|cell| {
        let session = cell.borrow();
        let session = session.as_ref()?;
        session.browser.progress(station, &session.known)
    })
}

/// Whether list entry `index` gets the "newly found" mark (an option).
pub(super) fn shown_fresh(index: usize) -> bool {
    MARK_NEW.load(std::sync::atomic::Ordering::Relaxed)
        && with_browser(|browser| browser.shown_fresh(index)).unwrap_or(false)
}

/// R or U aimed at a block in the world (no screen open): opens the inventory on the
/// recipes or uses of the block's item (beyond JEI).
pub(super) fn world_key(game: &mut Game<'_>, key: &str) {
    if !game.native_on(model::ID) || game.screen != super::Screen::Playing || game.title.is_some() {
        return;
    }
    let recipes = game.native_mod_key(model::ID, model::KEY_RECIPE[0], model::KEY_RECIPE[2]);
    let uses = game.native_mod_key(model::ID, model::KEY_USES[0], model::KEY_USES[2]);
    let uses = match key {
        k if k == recipes => false,
        k if k == uses => true,
        _ => return,
    };
    let Some(hit) = game.pick() else {
        return;
    };
    let Some(item) = game.picked_stack(hit.pos, false) else {
        return;
    };
    let target = model::Stack {
        id: item.item.to_string(),
        count: 1,
        components: String::new(),
    };
    let known = game.native_known_recipes();
    let updates = with_browser(|browser| {
        browser.handle(
            model::Event::Lookup {
                stack: target,
                uses,
            },
            false,
            &known,
        )
    })
    .unwrap_or_default();
    if updates.is_empty() {
        return;
    }
    // Open the inventory as its key does, then show the lookup.
    if game.creative {
        game.screen = super::Screen::Creative;
        game.refresh_creative_tabs();
        game.creative_screen.reopen();
        game.refresh_saved_hotbars();
    } else {
        game.screen = super::Screen::Inventory;
    }
    game.capture(false);
    game.aei_reset_viewer();
    for update in updates {
        publish(game, update);
    }
}

/// The tooltip line with an item's recipe and use counts, when that option is on.
pub(super) fn tooltip_counts(item: &str) -> Option<String> {
    if !COUNTS.load(std::sync::atomic::Ordering::Relaxed) {
        return None;
    }
    let (made, used) = with_browser(|browser| browser.counts(item)).flatten()?;
    Some(format!("\u{a7}8R: {made} recipes \u{b7} U: {used} uses"))
}

/// Ctrl+C over a recipe card: the recipe as Markdown on the clipboard.
pub(super) fn copy_recipe(game: &mut Game<'_>, card: usize) -> bool {
    use horizon_client::clipboard::Clipboard;
    let Some(text) = with_browser(|browser| browser.recipe_text(card)).flatten() else {
        return false;
    };
    game.clipboard.set(&text);
    game.message = Some(("Copied the recipe".to_owned(), std::time::Instant::now()));
    true
}

/// With that option on and a search typed: whether `item` is among the results.
pub(super) fn highlight(item: &str) -> Option<bool> {
    if !HIGHLIGHT.load(std::sync::atomic::Ordering::Relaxed) {
        return None;
    }
    with_browser(|browser| browser.in_results(item)).flatten()
}

/// The variant group whose first entry is list entry `index`: its size and whether it
/// is unfolded.
pub(super) fn shown_group(index: usize) -> Option<(usize, bool)> {
    with_browser(|browser| browser.shown_group(index)).flatten()
}

/// The player's note on `item`.
pub(super) fn note(item: &str) -> Option<String> {
    with_browser(|browser| browser.note(item).map(str::to_owned)).flatten()
}

/// Saves the note on `item` (a blank one removes it).
pub(super) fn save_note(game: &mut Game<'_>, item: &str, note: &str) {
    if let Some(text) = with_browser(|browser| {
        browser.set_note(item, note);
        browser.notes_text()
    }) {
        game.native_set_setting(model::ID, SETTING_NOTES, &text);
        game.message = Some(("Note saved".to_owned(), std::time::Instant::now()));
    }
}

/// With that option on: the first recipe of `target`, to preview while Shift is held.
pub(super) fn preview(target: &NativeStack) -> Option<(model::RecipeCard, &'static str)> {
    if !PREVIEW.load(std::sync::atomic::Ordering::Relaxed) {
        return None;
    }
    with_browser(|browser| browser.preview(&stack(target))).flatten()
}

/// Card `card` of the shown page, to pin: its card, station and recipe id.
pub(super) fn shown_card(card: usize) -> Option<(model::RecipeCard, &'static str, String)> {
    with_browser(|browser| browser.shown_card(card)).flatten()
}

/// Saves the pinned recipes' ids, one per line.
pub(super) fn save_pinned(game: &mut Game<'_>, ids: &[String]) {
    game.native_set_setting(model::ID, SETTING_PINNED, &ids.join("\n"));
}

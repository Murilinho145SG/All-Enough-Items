use crate::{
    Entry, Event, Ingredient, ItemInfo, Layout, Lookup, RECIPE_LIMIT, Recipe, RecipeCard,
    RecipeKind, RecipePage, STACK_LIMIT, SearchModes, Stack, Station, Update,
};
pub use details::Details;
pub use plan::{Plan, PlanMaterial, PlanStep};
use search::{Indexed, SearchQuery};
use std::collections::HashMap;
pub use trades::Trade;

/// Recently looked-up items kept.
pub const RECENT: usize = 9;

mod bookmarks;
mod categories;
mod craftable;
mod details;
mod discovery;
mod history;
mod listing;
mod perf;
mod plan;
mod recipes;
mod regex;
mod report;
mod search;
mod search_history;
mod trades;

/// State for one open world. Visible lists and action indexes are derived from
/// this state, so interaction indexes stay aligned with what the panel draws.
#[derive(Default)]
pub struct Browser {
    /// The item registry, which recipe ingredients index.
    items: Vec<ItemInfo>,
    /// Registry index by item id.
    by_id: HashMap<String, usize>,
    recipes: Vec<Recipe>,
    recipe_index: recipes::RecipeIndex,
    /// Registry indexes of each tag's items, sorted.
    tag_members: HashMap<String, Vec<usize>>,
    /// The item list: every entry the search can show, in display order.
    catalogue: Vec<Entry>,
    indexed: Vec<Indexed>,
    /// Catalogue indexes of the entries the search shows, and their item ids.
    filtered_items: Vec<usize>,
    result_ids: std::collections::HashSet<String>,
    /// Collapsible groups (an option): the expanded items, and each group's size by
    /// its first catalogue entry.
    grouping: bool,
    expanded: std::collections::HashSet<String>,
    group_sizes: HashMap<usize, usize>,
    query: String,
    search: SearchQuery,
    /// The items looked up last, newest first.
    recent: Vec<Stack>,
    /// What the player holds and the open crafting grid's size, for `is:craftable`.
    inventory: Vec<Stack>,
    grid: u32,
    /// Items ever in the inventory, those found this session and not looked at, the
    /// player's unlocked recipe-book ids and whether spoiler-free mode is on.
    seen: std::collections::HashSet<String>,
    fresh: std::collections::HashSet<String>,
    known: Vec<i32>,
    spoiler_free: bool,
    /// Villager trades seen on trading screens (their recipes follow the others).
    trades: Vec<trades::Trade>,
    /// The player's notes, by item id.
    notes: HashMap<String, String>,
    /// Item tags no item fills (from the tag files), for the report.
    empty_tags: Vec<String>,
    /// The calculator's recipe the player chose for an item (registry index -> recipe).
    plan_choices: HashMap<usize, usize>,
    /// Whether newly unlocked recipes mark their items new (after joining settles).
    watch_unlocks: bool,
    /// Earlier searches, oldest first, and the one Up/Down recalled last.
    search_history: Vec<String>,
    history_cursor: Option<usize>,
    search_modes: SearchModes,
    bookmarks: Vec<crate::Bookmark>,
    /// Catalogue indexes in display order.
    order: Vec<usize>,
    sort_order: crate::SortOrder,
    /// Hidden `(item, components)`; `*` components hide every stack of the item.
    hidden: std::collections::HashSet<(String, String)>,
    edit_mode: bool,
    /// The recipe categories in tab order, without the hidden ones.
    categories: Vec<Station>,
    /// The window of the shown category's recipes sent to the panel: offset and length.
    window: (usize, usize),
    /// How many recipes the shown category has in all.
    window_total: usize,
    hidden_categories: Vec<Station>,
    history: history::History,
    shown_recipe_ids: Vec<usize>,
}

impl Browser {
    pub fn new(items: Vec<ItemInfo>, recipes: Vec<Recipe>) -> (Self, Update) {
        let recipe_index = recipes::RecipeIndex::new(&recipes);
        let by_id = items
            .iter()
            .enumerate()
            .map(|(index, item)| (item.id.clone(), index))
            .collect();
        let mut tag_members: HashMap<String, Vec<usize>> = HashMap::new();
        for (index, item) in items.iter().enumerate() {
            for tag in &item.tags {
                tag_members.entry(tag.clone()).or_default().push(index);
            }
        }
        let mut browser = Self {
            items,
            by_id,
            tag_members,
            recipes,
            recipe_index,
            ..Self::default()
        };
        browser.categories = Station::ALL.to_vec();
        browser.window = (0, RECIPE_LIMIT);
        browser.set_entries(Vec::new());
        let update = browser.item_list();
        (browser, update)
    }

    /// Shows `entries` (the creative tabs' stacks, variants included) as the item list, in
    /// their order, then every registry item none of them is. Entries of unknown items
    /// are left out.
    pub fn set_catalogue(&mut self, entries: Vec<Entry>) -> Update {
        self.set_entries(entries);
        self.refresh_locked();
        self.item_list()
    }

    fn set_entries(&mut self, entries: Vec<Entry>) {
        let mut seen = std::collections::HashSet::new();
        let mut catalogue: Vec<Entry> = entries
            .into_iter()
            .filter(|entry| self.by_id.contains_key(&entry.stack.id))
            .filter(|entry| seen.insert((entry.stack.id.clone(), entry.stack.components.clone())))
            .collect();
        let listed: std::collections::HashSet<String> =
            catalogue.iter().map(|e| e.stack.id.clone()).collect();
        catalogue.extend(
            self.items
                .iter()
                .filter(|item| item.id != "minecraft:air" && !listed.contains(&item.id))
                .map(|item| Entry {
                    stack: Stack {
                        id: item.id.clone(),
                        count: 1,
                        components: String::new(),
                    },
                    name: item.name.clone(),
                    ..Entry::default()
                }),
        );
        self.indexed = catalogue
            .iter()
            .map(|entry| {
                let index = self.by_id.get(&entry.stack.id).copied();
                let item = index.map(|i| &self.items[i]);
                let has_recipe = index.is_some_and(|i| self.recipe_index.makes(i));
                let mut indexed = Indexed::new(entry, item, has_recipe);
                indexed.item = index;
                indexed
            })
            .collect();
        self.catalogue = catalogue;
        self.sort(self.sort_order);
    }

    fn render_page(&mut self, known: &[i32]) -> Update {
        let Some(mut lookup) = self.history.current().cloned() else {
            self.shown_recipe_ids.clear();
            return Update::RecipeList(RecipePage {
                title: String::new(),
                stations: Vec::new(),
                station: 0,
                cards: Vec::new(),
            });
        };
        let all = self.page_recipe_ids(&lookup);
        let available: Vec<Station> = self
            .stations()
            .filter(|station| {
                all.iter()
                    .any(|&index| Station::of(self.recipes[index].kind) == *station)
            })
            .collect();
        if lookup.station >= available.len() {
            lookup.station = 0;
        }
        let current = available.get(lookup.station).copied();
        let full: Vec<usize> = all
            .into_iter()
            .filter(|&index| Some(Station::of(self.recipes[index].kind)) == current)
            .collect();
        self.window_total = full.len();
        let len = self.window.1.clamp(1, RECIPE_LIMIT);
        let offset = self.window.0.min(full.len().saturating_sub(1));
        self.window.0 = offset;
        let selected: Vec<usize> = full.into_iter().skip(offset).take(len).collect();
        let title_name = self
            .items
            .iter()
            .find(|item| item.id == lookup.item.id)
            .map_or(lookup.item.id.as_str(), |item| item.name.as_str());
        let title = if let Some(category) = lookup.category {
            format!("All: {}", category.icon())
        } else if let Some(tag) = &lookup.tag {
            format!("Uses: #{tag}")
        } else {
            format!(
                "{}: {}",
                if lookup.uses { "Uses" } else { "Recipes" },
                title_name
            )
        };
        let tabs = available
            .iter()
            .map(|station| Stack {
                id: station.icon().to_owned(),
                count: 1,
                components: String::new(),
            })
            .collect();
        let cards = selected
            .iter()
            .map(|&index| self.make_card(&self.recipes[index], known))
            .collect();
        self.shown_recipe_ids = selected;
        if let Some(current) = self.history.current_mut() {
            current.station = lookup.station;
        }
        Update::RecipeList(RecipePage {
            title,
            stations: tabs,
            station: lookup.station,
            cards,
        })
    }

    fn open_lookup(&mut self, target: Stack, uses: bool, known: &[i32]) -> Option<Update> {
        let candidate = Lookup {
            focus: None,
            category: None,
            item: target,
            uses,
            station: 0,
            tag: None,
        };
        if self.page_recipe_ids(&candidate).is_empty() {
            return None;
        }
        self.remember_recent(&candidate.item);
        self.fresh.remove(&candidate.item.id);
        self.history.push(candidate);
        Some(self.render_page(known))
    }

    /// JEI's cheat clicks: left gives a full stack, right gives one.
    fn give(mut stack: Stack, right: bool) -> Update {
        stack.count = 1;
        Update::Give {
            stack,
            count: if right { 1 } else { 64 },
        }
    }

    /// Items looked up last, newest first ([`RECENT`] at most).
    pub fn recent(&self) -> &[Stack] {
        &self.recent
    }

    /// Replaces the recent items with saved `text` (one `id<TAB>components` per line).
    pub fn load_recent(&mut self, text: &str) {
        self.recent = text
            .lines()
            .filter_map(|line| {
                let (id, components) = line.split_once('\t').unwrap_or((line, ""));
                id.contains(':').then(|| Stack {
                    id: id.to_owned(),
                    count: 1,
                    components: components.to_owned(),
                })
            })
            .take(RECENT)
            .collect();
    }

    pub fn recent_text(&self) -> String {
        self.recent
            .iter()
            .map(|s| format!("{}\t{}\n", s.id, s.components))
            .collect()
    }

    fn remember_recent(&mut self, stack: &Stack) {
        let stack = Stack {
            count: 1,
            ..stack.clone()
        };
        self.recent.retain(|s| *s != stack);
        self.recent.insert(0, stack);
        self.recent.truncate(RECENT);
    }

    fn push_lookup(&mut self, lookup: Lookup, known: &[i32]) -> Vec<Update> {
        self.history.push(lookup);
        vec![self.render_page(known)]
    }

    /// Closes the recipe view after a recipe was moved into the grid, as JEI does: back to
    /// the catalogue, with the lookup history cleared.
    pub fn close_recipes(&mut self) -> Update {
        self.history.clear();
        self.render_page(&[])
    }

    /// The recipe-book ids of the recipes on the shown page, in card order (`-1` for a
    /// recipe the book doesn't hold).
    pub fn shown_book_ids(&self) -> Vec<i32> {
        self.shown_recipe_ids
            .iter()
            .map(|&recipe| self.recipes[recipe].book_id)
            .collect()
    }

    pub fn refresh(&mut self, known: &[i32]) -> Option<Update> {
        self.history
            .current()
            .is_some()
            .then(|| self.render_page(known))
    }

    /// The lookup trail (JEI-style breadcrumbs): each lookup's item (a category's station
    /// for "all recipes"), whether it shows uses, and the shown lookup's index.
    pub fn trail(&self) -> (Vec<(Stack, bool)>, Option<usize>) {
        let (entries, current) = self.history.trail();
        let trail = entries
            .iter()
            .map(|lookup| match lookup.category {
                Some(station) => (
                    Stack {
                        id: station.icon().to_owned(),
                        count: 1,
                        components: String::new(),
                    },
                    false,
                ),
                None => (lookup.item.clone(), lookup.uses),
            })
            .collect();
        (trail, current)
    }

    /// The shown window of the category's recipes: its offset and the category's total.
    pub fn shown_window(&self) -> (usize, usize) {
        (self.window.0, self.window_total)
    }

    pub fn handle(&mut self, event: Event, give: bool, known: &[i32]) -> Vec<Update> {
        // A new view starts at its first window.
        if matches!(
            event,
            Event::ClickItem { .. }
                | Event::Lookup { .. }
                | Event::ClickRecipeIngredient { .. }
                | Event::ClickBookmark { .. }
                | Event::SelectStation(_)
                | Event::ShowCategory
                | Event::Back
                | Event::Close
                | Event::Forward
                | Event::JumpHistory(_)
        ) {
            self.window.0 = 0;
        }
        match event {
            Event::SetInventory { stacks, grid } => self.set_inventory(stacks, grid),
            Event::SetKnown(known) => self.set_known(known),
            Event::SetGrouping(on) => {
                if on == self.grouping {
                    return Vec::new();
                }
                self.grouping = on;
                vec![self.item_list()]
            }
            Event::ToggleGroup(index) => {
                let Some(&entry) = self.filtered_items.get(index) else {
                    return Vec::new();
                };
                let id = self.catalogue[entry].stack.id.clone();
                if !self.expanded.remove(&id) {
                    self.expanded.insert(id);
                }
                vec![self.item_list()]
            }
            Event::SetSpoilerFree(on) => self.set_spoiler_free(on),
            Event::ShowWindow { offset, len } => {
                if self.history.current().is_none() {
                    return Vec::new();
                }
                self.window = (offset, len.clamp(1, RECIPE_LIMIT));
                vec![self.render_page(known)]
            }
            Event::ToggleBookmark(stack) => self.toggle_bookmark(stack),
            Event::BookmarkRecipe(card) => self.bookmark_recipe(card),
            Event::MoveBookmark { from, to } => self.move_bookmark(from, to),
            Event::ClickBookmark { index, right } => {
                let Some(bookmark) = self.bookmarks.get(index).cloned() else {
                    return Vec::new();
                };
                match bookmark.recipe {
                    Some(id) => self.open_recipe(&bookmark.stack, &id, known),
                    None if give => vec![Self::give(bookmark.stack, right)],
                    None => self
                        .open_lookup(bookmark.stack, right, known)
                        .into_iter()
                        .collect(),
                }
            }
            Event::Search(query) => {
                self.search = SearchQuery::parse(&query, &self.search_modes);
                self.query = query;
                vec![self.item_list()]
            }
            Event::SetSearchModes(modes) => {
                if modes == self.search_modes {
                    return Vec::new();
                }
                self.search_modes = modes;
                self.search = SearchQuery::parse(&self.query, &modes);
                vec![self.item_list()]
            }
            Event::SetSort(order) => {
                if order == self.sort_order {
                    return Vec::new();
                }
                self.sort_order = order;
                self.sort(order);
                vec![self.item_list()]
            }
            Event::SetEditMode(on) => {
                if on == self.edit_mode {
                    return Vec::new();
                }
                self.edit_mode = on;
                vec![self.item_list()]
            }
            Event::ToggleHidden { index, wildcard } => self.toggle_hidden(index, wildcard),
            Event::ClickItem { index, right } => {
                let Some(&entry) = self.filtered_items.get(index) else {
                    return Vec::new();
                };
                // A search that led somewhere is worth recalling.
                self.remember_search();
                let stack = self.catalogue[entry].stack.clone();
                if give {
                    return vec![Self::give(stack, right)];
                }
                self.open_lookup(stack, right, known).into_iter().collect()
            }
            Event::ClickRecipeIngredient { stack, right } if give => {
                vec![Self::give(stack, right)]
            }
            Event::Lookup { stack, uses } | Event::ClickRecipeIngredient { stack, right: uses } => {
                self.open_lookup(stack, uses, known).into_iter().collect()
            }
            Event::SelectStation(station) => {
                if let Some(current) = self.history.current_mut() {
                    current.station = station;
                }
                vec![self.render_page(known)]
            }
            Event::TagUses(tag) => {
                let Some(first) = self.tag_members.get(&tag).and_then(|m| m.first()) else {
                    return Vec::new();
                };
                let candidate = Lookup {
                    focus: None,
                    category: None,
                    item: Stack {
                        id: self.items[*first].id.clone(),
                        count: 1,
                        components: String::new(),
                    },
                    uses: true,
                    station: 0,
                    tag: Some(tag),
                };
                if self.page_recipe_ids(&candidate).is_empty() {
                    return Vec::new();
                }
                self.history.push(candidate);
                vec![self.render_page(known)]
            }
            Event::ShowCategory => {
                // Already showing every recipe: nothing more to show.
                if self
                    .history
                    .current()
                    .is_none_or(|current| current.category.is_some())
                {
                    return Vec::new();
                }
                let Some(&recipe) = self.shown_recipe_ids.first() else {
                    return Vec::new();
                };
                let station = Station::of(self.recipes[recipe].kind);
                // Stay on the same category among the tabs of all recipes.
                let tab = self
                    .stations()
                    .filter(|s| self.recipes.iter().any(|r| Station::of(r.kind) == *s))
                    .position(|s| s == station)
                    .unwrap_or(0);
                let candidate = Lookup {
                    focus: None,
                    category: Some(station),
                    item: Stack::default(),
                    uses: false,
                    station: tab,
                    tag: None,
                };
                self.history.push(candidate);
                vec![self.render_page(known)]
            }
            Event::Place(index) => match self
                .shown_recipe_ids
                .get(index)
                .map(|&recipe| self.recipes[recipe].book_id)
            {
                Some(id) if id >= 0 && known.contains(&id) => {
                    vec![Update::PlaceKnownRecipe(id)]
                }
                _ => Vec::new(),
            },
            Event::Back => {
                self.history.back();
                vec![self.render_page(known)]
            }
            Event::Close => {
                self.history.close();
                vec![self.render_page(known)]
            }
            Event::Forward => {
                self.history.forward();
                vec![self.render_page(known)]
            }
            Event::JumpHistory(index) => {
                self.history.jump(index);
                vec![self.render_page(known)]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (Vec<ItemInfo>, Vec<Recipe>) {
        let items = vec![
            ItemInfo {
                id: "minecraft:air".into(),
                name: "Air".into(),
                tags: vec![],
            },
            ItemInfo {
                id: "minecraft:oak_log".into(),
                name: "Oak Log".into(),
                tags: vec!["minecraft:logs".into()],
            },
            ItemInfo {
                id: "minecraft:oak_planks".into(),
                name: "Oak Planks".into(),
                tags: vec!["minecraft:planks".into()],
            },
            ItemInfo {
                id: "minecraft:stick".into(),
                name: "Stick".into(),
                tags: vec![],
            },
        ];
        let ingredient = |item, count| Ingredient {
            item,
            count,
            components: String::new(),
        };
        let recipes = vec![
            Recipe {
                id: String::new(),
                kind: RecipeKind::Shapeless,
                width: 0,
                height: 0,
                inputs: vec![vec![ingredient(1, 1)]],
                outputs: vec![ingredient(2, 4)],
                ticks: 0,
                experience: 0.0,
                book_id: 7,
            },
            Recipe {
                id: String::new(),
                kind: RecipeKind::Shaped,
                width: 1,
                height: 2,
                inputs: vec![vec![ingredient(2, 1)], vec![ingredient(2, 1)]],
                outputs: vec![ingredient(3, 4)],
                ticks: 0,
                experience: 0.0,
                book_id: 8,
            },
        ];
        (items, recipes)
    }

    #[test]
    fn search_filters_names_ids_namespaces_tags_and_excludes_air() {
        let (items, recipes) = fixture();
        let (mut browser, initial) = Browser::new(items, recipes);
        let Update::ItemList(all) = initial else {
            panic!("expected item list")
        };
        assert_eq!(all.len(), 3);
        for (query, expected) in [
            ("oak", 2),
            ("@mine stick", 1),
            ("$logs", 1),
            ("@other", 0),
            ("@minecraft $logs", 1),
            ("&oak_planks", 1),
            ("oak -planks", 1),
            ("\"oak log\"", 1),
            ("oak | stick", 3),
        ] {
            let result = browser.handle(Event::Search(query.into()), false, &[]);
            assert!(
                matches!(&result[0], Update::ItemList(items) if items.len() == expected),
                "query {query:?}"
            );
        }
    }

    fn entry(id: &str, name: &str, components: &str, tooltip: &[&str], tabs: &[&str]) -> Entry {
        Entry {
            stack: Stack {
                id: id.into(),
                count: 1,
                components: components.into(),
            },
            name: name.into(),
            tooltip: tooltip.iter().map(|s| (*s).to_owned()).collect(),
            tabs: tabs.iter().map(|s| (*s).to_owned()).collect(),
            ..Entry::default()
        }
    }

    fn shown(update: &[Update]) -> Vec<String> {
        match update.first() {
            Some(Update::ItemList(stacks)) => stacks
                .iter()
                .map(|s| format!("{}{}", s.id, s.components))
                .collect(),
            other => panic!("expected item list, got {other:?}"),
        }
    }

    #[test]
    fn the_catalogue_lists_variants_in_tab_order_then_the_rest_of_the_registry() {
        let (items, recipes) = fixture();
        let (mut browser, _) = Browser::new(items, recipes);
        let update = browser.set_catalogue(vec![
            entry("minecraft:stick", "Stick", "", &[], &["Ingredients"]),
            entry(
                "minecraft:stick",
                "Stick",
                "[custom_name='a']",
                &[],
                &["Ingredients"],
            ),
            entry("minecraft:stick", "Stick", "", &[], &["Ingredients"]),
            entry("test:unknown", "Unknown", "", &[], &[]),
            entry(
                "minecraft:oak_log",
                "Oak Log",
                "",
                &[],
                &["Building Blocks"],
            ),
        ]);
        let Update::ItemList(stacks) = update else {
            panic!("expected item list")
        };
        let ids: Vec<_> = stacks
            .iter()
            .map(|s| format!("{}{}", s.id, s.components))
            .collect();
        assert_eq!(
            ids,
            [
                "minecraft:stick",
                "minecraft:stick[custom_name='a']",
                "minecraft:oak_log",
                "minecraft:oak_planks",
            ]
        );
        // Clicking a variant looks up its exact stack.
        let page = browser.handle(
            Event::ClickItem {
                index: 2,
                right: true,
            },
            false,
            &[],
        );
        assert!(matches!(&page[0], Update::RecipeList(page) if page.title == "Uses: Oak Log"));
    }

    #[test]
    fn jei_prefixes_search_their_fields_and_follow_their_modes() {
        let (items, recipes) = fixture();
        let (mut browser, _) = Browser::new(items, recipes);
        browser.set_catalogue(vec![
            entry("minecraft:stick", "Stick", "", &["Handy"], &["Ingredients"]),
            entry(
                "minecraft:oak_log",
                "Oak Log",
                "",
                &[],
                &["Building Blocks"],
            ),
        ]);
        let search =
            |b: &mut Browser, q: &str| shown(&b.handle(Event::Search(q.into()), false, &[]));
        assert_eq!(search(&mut browser, "#handy"), ["minecraft:stick"]);
        // Tooltips are searched by plain words too (JEI's default).
        assert_eq!(search(&mut browser, "handy"), ["minecraft:stick"]);
        assert_eq!(search(&mut browser, "%building"), ["minecraft:oak_log"]);
        assert_eq!(search(&mut browser, "$planks"), ["minecraft:oak_planks"]);
        assert_eq!(
            search(&mut browser, "@minecraft -$logs -%ingr"),
            ["minecraft:oak_planks"]
        );
        // Tabs need their prefix by default.
        assert!(search(&mut browser, "building").is_empty());

        let modes = SearchModes {
            tab: crate::SearchMode::Enabled,
            tooltip: crate::SearchMode::Disabled,
            ..SearchModes::default()
        };
        search(&mut browser, "building");
        let update = browser.handle(Event::SetSearchModes(modes), false, &[]);
        // The shown query is searched again under the new modes.
        assert_eq!(shown(&update), ["minecraft:oak_log"]);
        assert!(search(&mut browser, "handy").is_empty());
        // A disabled prefix is part of the word: nothing is called "#handy".
        assert!(search(&mut browser, "#handy").is_empty());
        assert!(
            browser
                .handle(Event::SetSearchModes(modes), false, &[])
                .is_empty()
        );
    }

    #[test]
    fn search_modes_round_trip_through_their_setting_names() {
        for mode in crate::SearchMode::ALL {
            assert_eq!(crate::SearchMode::parse(mode.name()), Some(mode));
            assert_ne!(mode.next(), mode);
        }
        let mut modes = SearchModes::default();
        for field in 0..SearchModes::FIELDS.len() {
            modes.set(field, crate::SearchMode::Disabled);
            assert_eq!(modes.get(field), crate::SearchMode::Disabled);
        }
    }

    #[test]
    fn slots_name_the_tag_their_alternatives_stand_for() {
        let item = |id: &str, tags: &[&str]| ItemInfo {
            id: id.into(),
            name: id.into(),
            tags: tags.iter().map(|t| (*t).to_owned()).collect(),
        };
        let items = vec![
            item("minecraft:oak_planks", &["minecraft:planks", "test:wood"]),
            item("minecraft:birch_planks", &["minecraft:planks", "test:wood"]),
            item("minecraft:oak_log", &["test:wood"]),
            item("minecraft:stick", &[]),
        ];
        let ingredient = |item| Ingredient {
            item,
            count: 1,
            components: String::new(),
        };
        let recipe = Recipe {
            id: "minecraft:stick".into(),
            kind: RecipeKind::Shaped,
            width: 1,
            height: 3,
            inputs: vec![
                vec![ingredient(0), ingredient(1)],
                vec![ingredient(1), ingredient(0), ingredient(2)],
                vec![ingredient(0)],
            ],
            outputs: vec![ingredient(3)],
            ticks: 0,
            experience: 0.0,
            book_id: -1,
        };
        let (mut browser, _) = Browser::new(items, vec![recipe]);
        let stick = Stack {
            id: "minecraft:stick".into(),
            count: 1,
            components: String::new(),
        };
        browser.handle(
            Event::Lookup {
                stack: stick,
                uses: false,
            },
            false,
            &[],
        );
        assert_eq!(browser.shown_recipe_id(0), Some("minecraft:stick"));
        assert_eq!(browser.shown_slot_tag(0, 0), Some("minecraft:planks"));
        // Order and extra tags don't matter; the members must match exactly.
        assert_eq!(browser.shown_slot_tag(0, 1), Some("test:wood"));
        assert_eq!(browser.shown_slot_tag(0, 2), None);
        assert_eq!(browser.shown_slot_tag(1, 0), None);
    }

    #[test]
    fn bookmarks_toggle_open_lookups_and_focus_bookmarked_recipes() {
        let (items, mut recipes) = fixture();
        recipes[0].id = "minecraft:oak_planks".into();
        // A second recipe for planks, so the bookmarked one must be put first.
        let mut other = recipes[0].clone();
        other.id = "test:planks_from_stick".into();
        other.inputs = vec![vec![Ingredient {
            item: 3,
            count: 1,
            components: String::new(),
        }]];
        recipes.insert(0, other);
        let (mut browser, _) = Browser::new(items, recipes);
        let planks = Stack {
            id: "minecraft:oak_planks".into(),
            count: 4,
            components: String::new(),
        };
        let added = browser.handle(Event::ToggleBookmark(planks.clone()), false, &[]);
        assert!(matches!(&added[0], Update::Bookmarks(b) if b.len() == 1 && b[0].stack.count == 1));
        // Clicking a bookmark opens its uses (right) or recipes (left).
        let page = browser.handle(
            Event::ClickBookmark {
                index: 0,
                right: false,
            },
            false,
            &[],
        );
        assert!(matches!(&page[0], Update::RecipeList(p) if p.title == "Recipes: Oak Planks"));
        assert_eq!(browser.shown_recipe_id(0), Some("test:planks_from_stick"));
        // Bookmark the second recipe; opening it shows that recipe first.
        browser.handle(Event::BookmarkRecipe(1), false, &[]);
        assert!(browser.shown_recipe_bookmarked(1));
        let given = browser.handle(
            Event::ClickBookmark {
                index: 0,
                right: false,
            },
            true,
            &[],
        );
        assert!(
            matches!(&given[0], Update::Give { stack, count: 64 } if stack.id == "minecraft:oak_planks")
        );
        browser.handle(
            Event::ClickBookmark {
                index: 1,
                right: false,
            },
            false,
            &[],
        );
        assert_eq!(browser.shown_recipe_id(0), Some("minecraft:oak_planks"));
        assert!(browser.shown_recipe_bookmarked(0));
        // Reorder, save and reload; toggling the stack again removes it.
        browser.handle(Event::MoveBookmark { from: 1, to: 0 }, false, &[]);
        let text = browser.bookmarks_text();
        assert!(text.starts_with("recipe\t"));
        let Update::Bookmarks(loaded) = browser.load_bookmarks(&text) else {
            panic!("expected bookmarks")
        };
        assert_eq!(loaded.len(), 2);
        let removed = browser.handle(Event::ToggleBookmark(planks), false, &[]);
        assert!(
            matches!(&removed[0], Update::Bookmarks(b) if b.len() == 1 && b[0].recipe.is_some())
        );
    }

    #[test]
    fn cheat_clicks_give_a_stack_or_one_and_keep_components() {
        let (items, recipes) = fixture();
        let (mut browser, _) = Browser::new(items, recipes);
        browser.set_catalogue(vec![entry(
            "minecraft:stick",
            "Stick",
            "[custom_name='x']",
            &[],
            &[],
        )]);
        let click =
            |b: &mut Browser, right| b.handle(Event::ClickItem { index: 0, right }, true, &[]);
        assert!(matches!(&click(&mut browser, false)[0],
            Update::Give { stack, count: 64 } if stack.components == "[custom_name='x']"));
        assert!(matches!(
            &click(&mut browser, true)[0],
            Update::Give { count: 1, .. }
        ));
        let ingredient = Stack {
            id: "minecraft:oak_log".into(),
            count: 3,
            components: String::new(),
        };
        let given = browser.handle(
            Event::ClickRecipeIngredient {
                stack: ingredient,
                right: true,
            },
            true,
            &[],
        );
        assert!(matches!(&given[0], Update::Give { stack, count: 1 } if stack.count == 1));
    }

    #[test]
    fn sorting_and_edit_mode_hide_entries_and_save_them() {
        let (mut items, recipes) = fixture();
        items.push(ItemInfo {
            id: "amod:apple".into(),
            name: "Apple".into(),
            tags: vec![],
        });
        let (mut browser, _) = Browser::new(items, recipes);
        browser.set_catalogue(vec![
            entry("amod:apple", "Apple", "", &[], &[]),
            entry("minecraft:stick", "Stick", "", &[], &[]),
            entry("minecraft:stick", "Stick", "[custom_name='x']", &[], &[]),
            entry("minecraft:oak_log", "Oak Log", "", &[], &[]),
        ]);
        let sort = |b: &mut Browser, order| shown(&b.handle(Event::SetSort(order), false, &[]));
        assert_eq!(
            sort(&mut browser, crate::SortOrder::Mod)[0],
            "minecraft:stick"
        );
        assert_eq!(sort(&mut browser, crate::SortOrder::Name)[0], "amod:apple");
        assert_eq!(
            sort(&mut browser, crate::SortOrder::Creative)[0],
            "amod:apple"
        );

        // Hidden entries leave the list, and edit mode shows them marked.
        browser.handle(Event::SetEditMode(true), false, &[]);
        let hide = |b: &mut Browser, index, wildcard| {
            b.handle(Event::ToggleHidden { index, wildcard }, false, &[])
        };
        assert_eq!(hide(&mut browser, 1, true)[0], Update::Hidden);
        assert!(browser.shown_hidden(1) && browser.shown_hidden(2));
        let saved = browser.hidden_text();
        assert_eq!(saved, "minecraft:stick\t*\n");
        let list = shown(&browser.handle(Event::SetEditMode(false), false, &[]));
        assert_eq!(
            list,
            ["amod:apple", "minecraft:oak_log", "minecraft:oak_planks"]
        );
        // Showing one stack of a wildcard-hidden item shows the item again.
        browser.handle(Event::SetEditMode(true), false, &[]);
        hide(&mut browser, 2, false);
        assert_eq!(browser.hidden_text(), "");
        hide(&mut browser, 2, false);
        assert_eq!(
            browser.hidden_text(),
            "minecraft:stick\t[custom_name='x']\n"
        );
        browser.handle(Event::SetEditMode(false), false, &[]);
        let Update::ItemList(list) = browser.load_hidden(&saved) else {
            panic!("expected item list")
        };
        assert_eq!(list.len(), 3);
    }

    #[test]
    fn categories_follow_the_saved_order_and_hidden_ones_disappear() {
        let (items, mut recipes) = fixture();
        recipes[1].kind = RecipeKind::Smelting;
        let (mut browser, _) = Browser::new(items, recipes);
        let all = |b: &mut Browser| b.handle(Event::ShowCategory, false, &[]);
        browser.handle(
            Event::ClickItem {
                index: 0,
                right: true,
            },
            false,
            &[],
        );
        browser.load_categories("# comment\nminecraft:furnace\n!minecraft:smoker\nbogus\n");
        let text = browser.categories_text();
        assert!(text.contains("minecraft:furnace\nminecraft:crafting_table\n"));
        assert!(text.ends_with("!minecraft:smoker\n"));
        // Oak planks are used by a smelting recipe (first tab now) and nothing else.
        let page = browser.handle(
            Event::Lookup {
                stack: Stack {
                    id: "minecraft:oak_planks".into(),
                    count: 1,
                    components: String::new(),
                },
                uses: true,
            },
            false,
            &[],
        );
        assert!(
            matches!(&page[0], Update::RecipeList(p) if p.stations[0].id == "minecraft:furnace")
        );
        let shown = all(&mut browser);
        assert!(matches!(&shown[0], Update::RecipeList(p)
            if p.stations.iter().map(|s| s.id.as_str()).collect::<Vec<_>>()
                == ["minecraft:furnace", "minecraft:crafting_table"]));
        // Hiding smelting leaves planks without uses.
        browser.load_categories("!minecraft:furnace\n");
        let none = browser.handle(
            Event::Lookup {
                stack: Stack {
                    id: "minecraft:oak_planks".into(),
                    count: 1,
                    components: String::new(),
                },
                uses: true,
            },
            false,
            &[],
        );
        assert!(none.is_empty());
    }

    #[test]
    fn fuels_and_compostables_are_categories_with_notes() {
        let (items, mut recipes) = fixture();
        let one = |item| {
            vec![vec![Ingredient {
                item,
                count: 1,
                components: String::new(),
            }]]
        };
        recipes.push(Recipe {
            id: "aei:fuel/minecraft:stick".into(),
            kind: RecipeKind::Fuel,
            width: 0,
            height: 0,
            inputs: one(3),
            outputs: vec![],
            ticks: 100,
            experience: 0.0,
            book_id: -1,
        });
        recipes.push(Recipe {
            id: "aei:compost/minecraft:stick".into(),
            kind: RecipeKind::Compost,
            width: 0,
            height: 0,
            inputs: one(3),
            outputs: vec![],
            ticks: 0,
            experience: 0.3,
            book_id: -1,
        });
        let (mut browser, _) = Browser::new(items, recipes);
        let stick = Stack {
            id: "minecraft:stick".into(),
            count: 1,
            components: String::new(),
        };
        let page = browser.handle(
            Event::Lookup {
                stack: stick,
                uses: true,
            },
            false,
            &[],
        );
        let Update::RecipeList(page) = &page[0] else {
            panic!("expected recipes")
        };
        let ids: Vec<_> = page.stations.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["minecraft:coal", "minecraft:composter"]);
        assert_eq!(page.cards[0].note, "Smelts 0.5 items\nBurn time: 5s");
        let page = browser.handle(Event::SelectStation(1), false, &[]);
        assert!(matches!(&page[0], Update::RecipeList(p) if p.cards[0].note == "\nChance: 30%"));
        assert_eq!(
            super::recipes::fuel_note(200),
            "Smelts 1 item\nBurn time: 10s"
        );
    }

    #[test]
    fn long_categories_are_shown_a_window_at_a_time() {
        let (items, mut recipes) = fixture();
        let template = recipes[0].clone();
        recipes.extend((0..600).map(|i| Recipe {
            id: format!("test:{i}"),
            ..template.clone()
        }));
        let (mut browser, _) = Browser::new(items, recipes);
        let log = Stack {
            id: "minecraft:oak_log".into(),
            count: 1,
            components: String::new(),
        };
        let first = browser.handle(
            Event::Lookup {
                stack: log,
                uses: true,
            },
            false,
            &[],
        );
        assert!(matches!(&first[0], Update::RecipeList(p) if p.cards.len() == 256));
        assert_eq!(browser.shown_window(), (0, 601));
        let next = browser.handle(
            Event::ShowWindow {
                offset: 480,
                len: 240,
            },
            false,
            &[],
        );
        assert!(matches!(&next[0], Update::RecipeList(p) if p.cards.len() == 121));
        assert_eq!(browser.shown_window(), (480, 601));
        assert_eq!(browser.shown_recipe_id(0), Some("test:479"));
        // A new view starts again at the top, keeping the window length.
        browser.handle(Event::SelectStation(0), false, &[]);
        assert_eq!(browser.shown_window(), (0, 601));
        assert_eq!(browser.shown_book_ids().len(), 240);
    }

    #[test]
    fn search_forgives_accents_and_typos_and_reads_filters() {
        let (mut items, recipes) = fixture();
        items.push(ItemInfo {
            id: "minecraft:diamond_pickaxe".into(),
            name: "Picareta de Diamante".into(),
            tags: vec![],
        });
        items.push(ItemInfo {
            id: "minecraft:apple".into(),
            name: "Maçã".into(),
            tags: vec![],
        });
        let (mut browser, _) = Browser::new(items, recipes);
        let props = |durability, nutrition: Option<u32>, rarity: &str| crate::EntryProps {
            max_stack: if durability > 0 { 1 } else { 64 },
            durability,
            nutrition,
            rarity: rarity.into(),
            enchantable: durability > 0,
            block: false,
        };
        browser.set_catalogue(vec![
            Entry {
                aliases: vec!["Diamond Pickaxe".into()],
                props: props(1561, None, "common"),
                ..entry(
                    "minecraft:diamond_pickaxe",
                    "Picareta de Diamante",
                    "",
                    &[],
                    &[],
                )
            },
            Entry {
                aliases: vec!["Apple".into()],
                props: props(0, Some(4), "common"),
                ..entry("minecraft:apple", "Maçã", "", &[], &[])
            },
            Entry {
                props: props(0, None, "common"),
                ..entry("minecraft:stick", "Stick", "", &[], &[])
            },
        ]);
        let search =
            |b: &mut Browser, q: &str| shown(&b.handle(Event::Search(q.into()), false, &[]));
        // Accents, the English alias and one typo.
        assert_eq!(search(&mut browser, "maca"), ["minecraft:apple"]);
        assert_eq!(
            search(&mut browser, "pickaxe"),
            ["minecraft:diamond_pickaxe"]
        );
        assert_eq!(
            search(&mut browser, "diamnod"),
            ["minecraft:diamond_pickaxe"]
        );
        assert_eq!(
            search(&mut browser, "picarte"),
            ["minecraft:diamond_pickaxe"]
        );
        assert!(search(&mut browser, "sti").contains(&"minecraft:stick".to_owned()));
        // Short terms need the exact letters.
        assert!(search(&mut browser, "stq").is_empty());
        // Filters.
        assert_eq!(search(&mut browser, "is:food"), ["minecraft:apple"]);
        assert_eq!(
            search(&mut browser, "dur>1000"),
            ["minecraft:diamond_pickaxe"]
        );
        assert_eq!(
            search(&mut browser, "stack:1"),
            ["minecraft:diamond_pickaxe"]
        );
        assert_eq!(
            search(&mut browser, "nutrition>3 -is:enchantable"),
            ["minecraft:apple"]
        );
        // Planks have a recipe; logs don't.
        assert!(search(&mut browser, "has:recipe").contains(&"minecraft:oak_planks".to_owned()));
        assert!(!search(&mut browser, "no:recipe").contains(&"minecraft:oak_planks".to_owned()));
        // Identifiers with a colon stay identifiers.
        assert_eq!(search(&mut browser, "minecraft:apple"), ["minecraft:apple"]);
    }

    #[test]
    fn exact_matches_come_before_typo_matches() {
        let (items, recipes) = fixture();
        let (mut browser, _) = Browser::new(items, recipes);
        browser.set_catalogue(vec![
            entry("minecraft:oak_log", "Stock", "", &[], &[]),
            entry("minecraft:stick", "Stick", "", &[], &[]),
        ]);
        let found = shown(&browser.handle(Event::Search("stick".into()), false, &[]));
        assert_eq!(found[0], "minecraft:stick");
        assert!(found.contains(&"minecraft:oak_log".to_owned()));
    }

    #[test]
    fn the_trail_lists_lookups_and_jumps_back_to_one() {
        let (items, recipes) = fixture();
        let (mut browser, _) = Browser::new(items, recipes);
        let lookup = |b: &mut Browser, id: &str, uses| {
            b.handle(
                Event::Lookup {
                    stack: Stack {
                        id: id.into(),
                        count: 1,
                        components: String::new(),
                    },
                    uses,
                },
                false,
                &[],
            )
        };
        lookup(&mut browser, "minecraft:oak_log", true);
        lookup(&mut browser, "minecraft:oak_planks", true);
        browser.handle(Event::ShowCategory, false, &[]);
        let (trail, current) = browser.trail();
        assert_eq!(trail.len(), 3);
        assert_eq!(current, Some(2));
        assert_eq!(trail[2].0.id, "minecraft:crafting_table");
        let page = browser.handle(Event::JumpHistory(0), false, &[]);
        assert!(matches!(&page[0], Update::RecipeList(p) if p.title == "Uses: Oak Log"));
        assert_eq!(browser.trail().1, Some(0));
        // Forward still reaches the later lookups.
        let next = browser.handle(Event::Forward, false, &[]);
        assert!(matches!(&next[0], Update::RecipeList(p) if p.title == "Uses: Oak Planks"));
    }

    #[test]
    fn recent_items_keep_the_last_lookups_newest_first() {
        let (items, recipes) = fixture();
        let (mut browser, _) = Browser::new(items, recipes);
        for (index, right) in [(0, true), (1, true), (0, true), (2, false)] {
            browser.handle(Event::ClickItem { index, right }, false, &[]);
        }
        let ids: Vec<_> = browser.recent().iter().map(|s| s.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "minecraft:stick",
                "minecraft:oak_log",
                "minecraft:oak_planks"
            ]
        );
    }

    #[test]
    fn progress_counts_unlocked_book_recipes_per_category() {
        let (items, recipes) = fixture();
        let (browser, _) = Browser::new(items, recipes);
        assert_eq!(
            browser.progress("minecraft:crafting_table", &[8]),
            Some((1, 2))
        );
        assert_eq!(browser.progress("minecraft:stonecutter", &[8]), None);
        assert_eq!(browser.progress("test:none", &[]), None);
        let missing = browser.missing("minecraft:crafting_table", &[8]);
        assert_eq!(missing.len(), 1);
        let mut browser = browser;
        browser.handle(Event::SetKnown(vec![8]), false, &[]);
        let updates = browser.handle(Event::Search("is:locked".into()), false, &[]);
        let Some(Update::ItemList(list)) = updates.first() else {
            panic!("expected the item list");
        };
        let names: Vec<_> = list
            .iter()
            .map(|s| browser.items[browser.by_id[&s.id]].name.clone())
            .collect();
        assert_eq!(names, missing);
        // Unlocking every book recipe empties both.
        let all: Vec<i32> = browser.recipes.iter().map(|r| r.book_id).collect();
        let updates = browser.handle(Event::SetKnown(all.clone()), false, &[]);
        assert!(matches!(updates.first(), Some(Update::ItemList(list)) if list.is_empty()));
        assert!(browser.missing("minecraft:crafting_table", &all).is_empty());
    }

    #[test]
    fn results_tell_which_items_a_search_keeps() {
        let (items, recipes) = fixture();
        let (mut browser, _) = Browser::new(items, recipes);
        assert_eq!(browser.in_results("minecraft:stick"), None);
        browser.handle(Event::Search("oak".into()), false, &[]);
        assert_eq!(browser.in_results("minecraft:oak_log"), Some(true));
        assert_eq!(browser.in_results("minecraft:stick"), Some(false));
    }

    #[test]
    fn slashes_search_with_a_regular_expression() {
        let (items, recipes) = fixture();
        let (mut browser, _) = Browser::new(items, recipes);
        let mut found = |query: &str| {
            browser.handle(Event::Search(query.into()), false, &[]);
            let mut ids: Vec<&str> = [
                "minecraft:oak_log",
                "minecraft:oak_planks",
                "minecraft:stick",
            ]
            .into_iter()
            .filter(|id| browser.in_results(id) == Some(true))
            .collect();
            ids.sort_unstable();
            ids.join(",")
        };
        assert_eq!(found("/^oak/"), "minecraft:oak_log,minecraft:oak_planks");
        assert_eq!(found("/log$|^stick$/"), "minecraft:oak_log,minecraft:stick");
        assert_eq!(found("&/:stick$/"), "minecraft:stick");
        assert_eq!(found(r#""/^oak \w+s$/""#), "minecraft:oak_planks");
        assert_eq!(found("/^oak/ -/log/"), "minecraft:oak_planks");
        // A bad pattern searches its text.
        assert_eq!(found("/zz(/"), "");
        assert_eq!(found("/stic/"), "minecraft:stick");
    }

    #[test]
    fn grouping_folds_variants_and_shift_clicks_unfold_them() {
        let (items, recipes) = fixture();
        let (mut browser, _) = Browser::new(items, recipes);
        browser.set_catalogue(vec![
            entry("minecraft:stick", "Stick", "", &[], &[]),
            entry("minecraft:stick", "Stick", "[custom_name='a']", &[], &[]),
            entry("minecraft:stick", "Stick", "[custom_name='b']", &[], &[]),
            entry("minecraft:oak_log", "Oak Log", "", &[], &[]),
        ]);
        let folded = shown(&browser.handle(Event::SetGrouping(true), false, &[]));
        assert_eq!(folded.len(), 3);
        assert_eq!(browser.shown_group(0), Some((3, false)));
        assert_eq!(browser.shown_group(1), None);
        let open = shown(&browser.handle(Event::ToggleGroup(0), false, &[]));
        assert_eq!(open.len(), 5);
        assert_eq!(browser.shown_group(0), Some((3, true)));
        assert_eq!(browser.shown_group(1), None);
        let closed = shown(&browser.handle(Event::ToggleGroup(0), false, &[]));
        assert_eq!(closed.len(), 3);
    }

    #[test]
    fn a_tag_s_uses_are_the_recipes_taking_any_of_its_items() {
        let tagged = |id: &str| ItemInfo {
            id: format!("test:{id}"),
            name: id.into(),
            tags: vec!["test:planks".into()],
        };
        let items = vec![
            tagged("oak"),
            tagged("birch"),
            ItemInfo {
                id: "test:chest".into(),
                name: "chest".into(),
                tags: vec![],
            },
        ];
        let one = |item| Ingredient {
            item,
            count: 1,
            components: String::new(),
        };
        let recipe = |id: &str, slot: Vec<usize>| Recipe {
            id: id.into(),
            kind: RecipeKind::Shapeless,
            width: 0,
            height: 0,
            inputs: vec![slot.into_iter().map(one).collect()],
            outputs: vec![one(2)],
            ticks: 0,
            experience: 0.0,
            book_id: -1,
        };
        let recipes = vec![
            recipe("test:any_planks", vec![0, 1]),
            recipe("test:oak_only", vec![0]),
        ];
        let (mut browser, _) = Browser::new(items, recipes);
        let updates = browser.handle(Event::TagUses("test:planks".into()), false, &[]);
        let Some(Update::RecipeList(page)) = updates.first() else {
            panic!("expected recipes");
        };
        assert_eq!(page.title, "Uses: #test:planks");
        assert_eq!(page.cards.len(), 1);
        assert!(
            browser
                .handle(Event::TagUses("test:none".into()), false, &[])
                .is_empty()
        );
    }

    #[test]
    fn previews_show_an_item_s_first_crafting_recipe() {
        let (items, recipes) = fixture();
        let (browser, _) = Browser::new(items, recipes);
        let stick = Stack {
            id: "minecraft:stick".into(),
            count: 1,
            components: String::new(),
        };
        let (card, station) = browser.preview(&stick).unwrap();
        assert_eq!(station, "minecraft:crafting_table");
        assert_eq!(card.outputs[0].id, "minecraft:stick");
        let log = Stack {
            id: "minecraft:oak_log".into(),
            ..stick
        };
        assert!(browser.preview(&log).is_none());
        let id = browser.recipes[0].id.clone();
        assert_eq!(browser.card_by_id(&id).unwrap().2, id);
        assert!(browser.card_by_id("test:missing").is_none());
        let book = browser.recipes.iter().find(|r| r.book_id >= 0).unwrap();
        let (book_id, slots) = (book.book_id, book.inputs.len());
        assert_eq!(browser.book_inputs(book_id).unwrap().len(), slots);
        assert!(browser.book_inputs(-1).is_none());
    }

    #[test]
    fn lookup_tabs_and_back_keep_a_navigation_stack() {
        let (items, recipes) = fixture();
        let (mut browser, _) = Browser::new(items, recipes);
        let output = browser.handle(
            Event::Lookup {
                stack: Stack {
                    id: "minecraft:oak_log".into(),
                    count: 1,
                    components: String::new(),
                },
                uses: true,
            },
            false,
            &[8],
        );
        let Update::RecipeList(page) = &output[0] else {
            panic!("expected recipe page")
        };
        assert_eq!(page.title, "Uses: Oak Log");
        assert_eq!(page.stations.len(), 1);
        assert_eq!(page.cards[0].place_enabled, Some(false));
        let output = browser.handle(Event::Place(0), false, &[7]);
        assert_eq!(output, [Update::PlaceKnownRecipe(7)]);
        let next = browser.handle(
            Event::Lookup {
                stack: Stack {
                    id: "minecraft:oak_planks".into(),
                    count: 1,
                    components: String::new(),
                },
                uses: true,
            },
            false,
            &[8],
        );
        assert!(matches!(&next[0], Update::RecipeList(page) if page.title == "Uses: Oak Planks"));
        let back = browser.handle(Event::Back, false, &[]);
        assert!(matches!(&back[0], Update::RecipeList(page) if page.title == "Uses: Oak Log"));
        let forward = browser.handle(Event::Forward, false, &[]);
        assert!(
            matches!(&forward[0], Update::RecipeList(page) if page.title == "Uses: Oak Planks")
        );
    }

    #[test]
    fn unresolved_lookups_leave_the_current_view_unchanged() {
        let (items, recipes) = fixture();
        let (mut browser, _) = Browser::new(items, recipes);
        let output = browser.handle(
            Event::Lookup {
                stack: Stack {
                    id: "minecraft:diamond".into(),
                    count: 1,
                    components: String::new(),
                },
                uses: false,
            },
            false,
            &[],
        );
        assert!(output.is_empty());
        let next = browser.handle(Event::Forward, false, &[]);
        assert!(matches!(&next[0], Update::RecipeList(page) if page.cards.is_empty()));
    }

    #[test]
    fn forward_reopens_first_lookup_after_returning_to_catalogue() {
        let (items, recipes) = fixture();
        let (mut browser, _) = Browser::new(items, recipes);
        browser.handle(
            Event::ClickItem {
                index: 0,
                right: true,
            },
            false,
            &[],
        );
        let back = browser.handle(Event::Back, false, &[]);
        assert!(matches!(&back[0], Update::RecipeList(page) if page.cards.is_empty()));
        let forward = browser.handle(Event::Forward, false, &[]);
        assert!(matches!(&forward[0], Update::RecipeList(page) if page.title == "Uses: Oak Log"));
        browser.handle(Event::Back, false, &[]);
        browser.handle(
            Event::ClickItem {
                index: 1,
                right: true,
            },
            false,
            &[],
        );
        let forward = browser.handle(Event::Forward, false, &[]);
        assert!(
            matches!(&forward[0], Update::RecipeList(page) if page.title == "Uses: Oak Planks")
        );
    }

    #[test]
    fn registry_indexes_survive_large_catalogues_and_searches() {
        let (mut items, mut recipes) = fixture();
        items.extend((4..9000).map(|i| ItemInfo {
            id: format!("test:item_{i}"),
            name: format!("Item {i}"),
            tags: vec![],
        }));
        recipes[0].outputs[0].item = 8999;
        let (mut browser, _) = Browser::new(items, recipes);
        let filtered = browser.handle(Event::Search("&test:item_8999".into()), false, &[]);
        assert!(matches!(&filtered[0], Update::ItemList(items) if items.len() == 1));
        let found = browser.handle(
            Event::ClickItem {
                index: 0,
                right: false,
            },
            false,
            &[],
        );
        assert!(
            matches!(&found[0], Update::RecipeList(page) if page.cards[0].outputs[0].id == "test:item_8999")
        );
    }

    #[test]
    fn category_lookup_and_components_keep_actions_aligned() {
        let (items, mut recipes) = fixture();
        recipes[0].outputs[0].components = "[custom_name='one']".into();
        let (mut browser, _) = Browser::new(items, recipes);
        let target = Stack {
            id: "minecraft:oak_planks".into(),
            count: 1,
            components: "[custom_name='two']".into(),
        };
        assert!(
            browser
                .handle(
                    Event::Lookup {
                        stack: target.clone(),
                        uses: false
                    },
                    false,
                    &[]
                )
                .is_empty()
        );
        let target = Stack {
            components: String::new(),
            ..target
        };
        let page = browser.handle(
            Event::Lookup {
                stack: target,
                uses: false,
            },
            false,
            &[],
        );
        assert!(
            matches!(&page[0], Update::RecipeList(page) if page.cards.len() == 1 && page.cards[0].place_enabled == Some(false))
        );
        let refreshed = browser.refresh(&[7]).unwrap();
        assert!(
            matches!(refreshed, Update::RecipeList(page) if page.cards[0].place_enabled == Some(true))
        );
        let all = browser.handle(Event::ShowCategory, false, &[7]);
        assert!(matches!(&all[0], Update::RecipeList(page) if page.cards.len() == 2));
        assert_eq!(browser.shown_book_ids().len(), 2);
        // Every recipe is already shown: a second click changes nothing.
        assert!(browser.handle(Event::ShowCategory, false, &[7]).is_empty());
        assert_eq!(
            browser.handle(Event::Place(1), false, &[8]),
            [Update::PlaceKnownRecipe(8)]
        );
        // Closing after a placement returns to the catalogue with no history left.
        assert!(
            matches!(browser.close_recipes(), Update::RecipeList(page) if page.cards.is_empty())
        );
        assert!(browser.shown_book_ids().is_empty());
        let forward = browser.handle(Event::Forward, false, &[7]);
        assert!(
            forward
                .iter()
                .all(|update| matches!(update, Update::RecipeList(page) if page.cards.is_empty()))
        );
    }

    #[test]
    fn escape_closes_every_lookup_and_forward_reopens_the_last() {
        let (items, recipes) = fixture();
        let (mut browser, _) = Browser::new(items, recipes);
        browser.handle(
            Event::ClickItem {
                index: 1,
                right: false,
            },
            false,
            &[],
        );
        browser.handle(
            Event::ClickItem {
                index: 1,
                right: true,
            },
            false,
            &[],
        );
        assert!(browser.history.current().is_some());
        let updates = browser.handle(Event::Close, false, &[]);
        assert!(browser.history.current().is_none());
        assert!(!updates.is_empty());
        browser.handle(Event::Forward, false, &[]);
        assert!(browser.history.current().is_some());
    }
}

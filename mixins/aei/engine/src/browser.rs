use crate::{
    Entry, Event, Ingredient, ItemInfo, Layout, Lookup, RECIPE_LIMIT, Recipe, RecipeCard,
    RecipeKind, RecipePage, STACK_LIMIT, SearchModes, Stack, Station, Update,
};
use search::{Indexed, SearchQuery};
use std::collections::HashMap;

mod history;
mod recipes;
mod search;

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
    /// Catalogue indexes of the entries the search shows.
    filtered_items: Vec<usize>,
    query: String,
    search: SearchQuery,
    search_modes: SearchModes,
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
        browser.set_entries(Vec::new());
        let update = browser.item_list();
        (browser, update)
    }

    /// Shows `entries` (the creative tabs' stacks, variants included) as the item list, in
    /// their order, then every registry item none of them is. Entries of unknown items
    /// are left out.
    pub fn set_catalogue(&mut self, entries: Vec<Entry>) -> Update {
        self.set_entries(entries);
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
                    tooltip: Vec::new(),
                    tabs: Vec::new(),
                }),
        );
        self.indexed = catalogue
            .iter()
            .map(|entry| {
                let item = self.by_id.get(&entry.stack.id).map(|&i| &self.items[i]);
                Indexed::new(entry, item)
            })
            .collect();
        self.catalogue = catalogue;
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
        let available: Vec<Station> = Station::ALL
            .into_iter()
            .filter(|station| {
                all.iter()
                    .any(|&index| Station::of(self.recipes[index].kind) == *station)
            })
            .collect();
        if lookup.station >= available.len() {
            lookup.station = 0;
        }
        let current = available.get(lookup.station).copied();
        let selected: Vec<usize> = all
            .into_iter()
            .filter(|&index| Some(Station::of(self.recipes[index].kind)) == current)
            .take(RECIPE_LIMIT)
            .collect();
        let title_name = self
            .items
            .iter()
            .find(|item| item.id == lookup.item.id)
            .map_or(lookup.item.id.as_str(), |item| item.name.as_str());
        let title = if let Some(category) = lookup.category {
            format!("All: {}", category.icon())
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
            category: None,
            item: target,
            uses,
            station: 0,
        };
        if self.page_recipe_ids(&candidate).is_empty() {
            return None;
        }
        self.history.push(candidate);
        Some(self.render_page(known))
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

    pub fn handle(&mut self, event: Event, give: bool, known: &[i32]) -> Vec<Update> {
        match event {
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
            Event::ClickItem { index, right } => {
                let Some(&entry) = self.filtered_items.get(index) else {
                    return Vec::new();
                };
                let stack = self.catalogue[entry].stack.clone();
                if give {
                    return vec![Update::Give {
                        item: stack.id,
                        count: if right { 99 } else { 1 },
                    }];
                }
                self.open_lookup(stack, right, known).into_iter().collect()
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
                let tab = Station::ALL
                    .into_iter()
                    .filter(|s| self.recipes.iter().any(|r| Station::of(r.kind) == *s))
                    .position(|s| s == station)
                    .unwrap_or(0);
                let candidate = Lookup {
                    category: Some(station),
                    item: Stack::default(),
                    uses: false,
                    station: tab,
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
            Event::Forward => {
                self.history.forward();
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
}

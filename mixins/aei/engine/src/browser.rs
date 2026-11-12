use crate::{
    Event, Ingredient, ItemInfo, Layout, Lookup, RECIPE_LIMIT, Recipe, RecipeCard, RecipeKind,
    RecipePage, STACK_LIMIT, Stack, Station, Update,
};
use search::SearchQuery;

mod history;
mod recipes;
mod search;

/// State for one open world. Visible lists and action indexes are derived from
/// this state, so interaction indexes stay aligned with what the panel draws.
#[derive(Default)]
pub struct Browser {
    items: Vec<ItemInfo>,
    recipes: Vec<Recipe>,
    recipe_index: recipes::RecipeIndex,
    filtered_items: Vec<usize>,
    search: SearchQuery,
    history: history::History,
    shown_recipe_ids: Vec<usize>,
}

impl Browser {
    pub fn new(items: Vec<ItemInfo>, recipes: Vec<Recipe>) -> (Self, Update) {
        let recipe_index = recipes::RecipeIndex::new(&recipes);
        let mut browser = Self {
            items,
            recipes,
            recipe_index,
            ..Self::default()
        };
        let update = browser.item_list();
        (browser, update)
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

    pub fn refresh(&mut self, known: &[i32]) -> Option<Update> {
        self.history
            .current()
            .is_some()
            .then(|| self.render_page(known))
    }

    pub fn handle(&mut self, event: Event, give: bool, known: &[i32]) -> Vec<Update> {
        match event {
            Event::Search(query) => {
                self.search = SearchQuery::parse(&query);
                vec![self.item_list()]
            }
            Event::ClickItem { index, right } => {
                let Some(&item_index) = self.filtered_items.get(index) else {
                    return Vec::new();
                };
                let id = self.items[item_index].id.clone();
                if give {
                    return vec![Update::Give {
                        item: id,
                        count: if right { 99 } else { 1 },
                    }];
                }
                self.open_lookup(
                    Stack {
                        id,
                        count: 1,
                        components: String::new(),
                    },
                    right,
                    known,
                )
                .into_iter()
                .collect()
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
                let Some(&recipe) = self.shown_recipe_ids.first() else {
                    return Vec::new();
                };
                let candidate = Lookup {
                    category: Some(Station::of(self.recipes[recipe].kind)),
                    item: Stack::default(),
                    uses: false,
                    station: 0,
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
            ("#logs", 1),
            ("@other", 0),
            ("@minecraft #logs", 1),
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
        assert_eq!(
            browser.handle(Event::Place(1), false, &[8]),
            [Update::PlaceKnownRecipe(8)]
        );
    }
}

//! Performance guards: a search and a lookup over a modded-size registry stay well
//! inside a frame. The limits are loose for debug builds; release is far faster.

#[cfg(test)]
mod tests {
    use crate::{Browser, Event, Ingredient, ItemInfo, Recipe, RecipeKind, Stack};
    use std::time::{Duration, Instant};

    fn big() -> Browser {
        let items: Vec<ItemInfo> = (0..5000)
            .map(|i| ItemInfo {
                id: format!("test:item_{i}"),
                name: format!("Test Item Number {i}"),
                tags: vec![format!("test:group_{}", i % 50)],
            })
            .collect();
        let one = |item| Ingredient {
            item,
            count: 1,
            components: String::new(),
        };
        let recipes: Vec<Recipe> = (1..5000)
            .map(|i| Recipe {
                id: format!("test:r{i}"),
                kind: RecipeKind::Shaped,
                width: 3,
                height: 3,
                inputs: (0..9).map(|k| vec![one((i * 7 + k) % 5000)]).collect(),
                outputs: vec![one(i)],
                ticks: 0,
                experience: 0.0,
                book_id: -1,
            })
            .collect();
        Browser::new(items, recipes).0
    }

    fn time(run: impl FnOnce()) -> Duration {
        let start = Instant::now();
        run();
        start.elapsed()
    }

    #[test]
    fn searches_and_lookups_stay_fast_on_five_thousand_items() {
        let mut browser = big();
        let limit = if cfg!(debug_assertions) {
            Duration::from_millis(250)
        } else {
            Duration::from_millis(20)
        };
        for query in ["number 4999", "numbr", "$group_7 -9", "is:craftable"] {
            let took = time(|| {
                browser.handle(Event::Search(query.into()), false, &[]);
            });
            assert!(took < limit, "search {query:?} took {took:?}");
        }
        let took = time(|| {
            let stack = Stack {
                id: "test:item_42".into(),
                count: 1,
                components: String::new(),
            };
            browser.handle(Event::Lookup { stack, uses: true }, false, &[]);
        });
        assert!(took < limit, "lookup took {took:?}");
    }
}

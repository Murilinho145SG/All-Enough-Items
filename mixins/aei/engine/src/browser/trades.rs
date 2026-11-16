//! Villager trades the player has seen, kept as recipes of a "Trading" category.
//! Only offers that showed on a trading screen are known: nothing is guessed.
//!
//! Saved as text, one trade per line: three stacks (first cost, second cost or empty,
//! result), each `id<TAB>count<TAB>components`, separated by tabs.

use super::Browser;
use crate::{Ingredient, Recipe, RecipeKind, Stack};

/// One seen offer: what it costs and what it gives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trade {
    pub cost: Stack,
    pub cost_b: Option<Stack>,
    pub result: Stack,
}

/// Trades kept at most.
const LIMIT: usize = 2048;

fn stack_fields(stack: Option<&Stack>) -> String {
    match stack {
        Some(s) => format!("{}\t{}\t{}", s.id, s.count, s.components),
        None => "\t\t".to_owned(),
    }
}

fn parse_stack(fields: &[&str]) -> Option<Option<Stack>> {
    match fields {
        ["", _, _] => Some(None),
        [id, count, components] => Some(Some(Stack {
            id: (*id).to_owned(),
            count: count.parse().ok()?,
            components: (*components).to_owned(),
        })),
        _ => None,
    }
}

impl Browser {
    /// Adds the trades not known yet as Trading recipes; true when any was new.
    pub fn add_trades(&mut self, trades: &[Trade]) -> bool {
        let mut added = false;
        for trade in trades {
            if self.trades.len() >= LIMIT || self.trades.contains(trade) {
                continue;
            }
            let ingredient = |stack: &Stack| {
                Some(Ingredient {
                    item: *self.by_id.get(&stack.id)?,
                    count: stack.count,
                    components: stack.components.clone(),
                })
            };
            let (Some(cost), Some(result)) = (ingredient(&trade.cost), ingredient(&trade.result))
            else {
                continue;
            };
            let mut inputs = vec![vec![cost]];
            if let Some(b) = trade.cost_b.as_ref().and_then(ingredient) {
                inputs.push(vec![b]);
            }
            self.recipes.push(Recipe {
                id: format!("aei:trade/{}", self.trades.len()),
                kind: RecipeKind::Trade,
                width: 2,
                height: 1,
                inputs,
                outputs: vec![result],
                ticks: 0,
                experience: 0.0,
                book_id: -1,
            });
            self.trades.push(trade.clone());
            added = true;
        }
        if added {
            self.recipe_index = super::recipes::RecipeIndex::new(&self.recipes);
        }
        added
    }

    pub fn trades_text(&self) -> String {
        self.trades
            .iter()
            .map(|t| {
                format!(
                    "{}\t{}\t{}\n",
                    stack_fields(Some(&t.cost)),
                    stack_fields(t.cost_b.as_ref()),
                    stack_fields(Some(&t.result))
                )
            })
            .collect()
    }

    /// Adds the trades saved as `text`.
    pub fn load_trades(&mut self, text: &str) {
        let trades: Vec<Trade> = text
            .lines()
            .filter_map(|line| {
                let mut fields: Vec<&str> = line.split('\t').collect();
                // Settings come back trimmed: empty fields at the end may be gone.
                if fields.len() > 9 {
                    return None;
                }
                fields.resize(9, "");
                Some(Trade {
                    cost: parse_stack(&fields[0..3])??,
                    cost_b: parse_stack(&fields[3..6])?,
                    result: parse_stack(&fields[6..9])??,
                })
            })
            .collect();
        self.add_trades(&trades);
    }
}

#[cfg(test)]
mod tests {
    use super::Trade;
    use crate::{Browser, Event, ItemInfo, Stack, Update};

    fn stack(id: &str, count: u32) -> Stack {
        Stack {
            id: format!("minecraft:{id}"),
            count,
            components: String::new(),
        }
    }

    #[test]
    fn trades_show_after_the_glue_s_start_up_sequence() {
        let items = ["emerald", "beetroot", "bread"]
            .map(|id| ItemInfo {
                id: format!("minecraft:{id}"),
                name: id.into(),
                tags: vec![],
            })
            .to_vec();
        let (mut browser, _) = Browser::new(items, Vec::new());
        browser.load_categories("minecraft:crafting_table\nminecraft:furnace\n");
        // As the settings return it: trimmed, the last empty fields gone.
        browser.load_trades(
            "minecraft:emerald\t1\t\t\t\t\tminecraft:bread\t6\t\n\
             minecraft:beetroot\t15\t\t\t\t\tminecraft:emerald\t1",
        );
        let page = browser.handle(
            Event::Lookup {
                stack: stack("beetroot", 1),
                uses: true,
            },
            false,
            &[],
        );
        assert!(
            matches!(&page.first(), Some(Update::RecipeList(p)) if p.stations[0].id == "minecraft:emerald")
        );
    }

    #[test]
    fn seen_trades_become_recipes_and_survive_a_round_trip() {
        let items = ["emerald", "wheat", "bread"]
            .map(|id| ItemInfo {
                id: format!("minecraft:{id}"),
                name: id.into(),
                tags: vec![],
            })
            .to_vec();
        let (mut browser, _) = Browser::new(items.clone(), Vec::new());
        let trade = Trade {
            cost: stack("wheat", 20),
            cost_b: None,
            result: stack("emerald", 1),
        };
        assert!(browser.add_trades(&[trade.clone(), trade.clone()]));
        assert!(!browser.add_trades(&[trade]));
        let page = browser.handle(
            Event::Lookup {
                stack: stack("emerald", 1),
                uses: false,
            },
            false,
            &[],
        );
        assert!(matches!(&page[0], Update::RecipeList(p)
            if p.stations[0].id == "minecraft:emerald" && p.cards[0].inputs[0][0].count == 20));
        let text = browser.trades_text();
        let (mut fresh, _) = Browser::new(items, Vec::new());
        fresh.load_trades(&text);
        assert_eq!(fresh.trades_text(), text);
    }
}

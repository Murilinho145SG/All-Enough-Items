//! Earlier searches, recalled with Up and Down in the search field and saved as text,
//! one per line.

use super::Browser;

/// Searches kept.
const LIMIT: usize = 30;

impl Browser {
    /// Replaces the history with the saved `text`.
    pub fn load_search_history(&mut self, text: &str) {
        self.search_history = text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(str::to_owned)
            .collect();
        let extra = self.search_history.len().saturating_sub(LIMIT);
        self.search_history.drain(..extra);
        self.history_cursor = None;
    }

    pub fn search_history_text(&self) -> String {
        self.search_history
            .iter()
            .map(|q| format!("{q}\n"))
            .collect()
    }

    /// Keeps the current search (Enter, or a click that opened recipes). True when the
    /// history changed.
    pub fn remember_search(&mut self) -> bool {
        self.history_cursor = None;
        let query = self.query.trim();
        if query.is_empty() || self.search_history.last().is_some_and(|q| q == query) {
            return false;
        }
        let query = query.to_owned();
        self.search_history.retain(|q| *q != query);
        self.search_history.push(query);
        let extra = self.search_history.len().saturating_sub(LIMIT);
        self.search_history.drain(..extra);
        true
    }

    /// Up (`older`) or Down: the search to put in the field, or None past the newest,
    /// which empties the field.
    pub fn recall_search(&mut self, older: bool) -> Option<String> {
        let last = self.search_history.len().checked_sub(1)?;
        let cursor = match (self.history_cursor, older) {
            (None, true) => Some(last),
            (None, false) => None,
            (Some(i), true) => Some(i.saturating_sub(1)),
            (Some(i), false) if i < last => Some(i + 1),
            (Some(_), false) => None,
        };
        self.history_cursor = cursor;
        cursor.map(|i| self.search_history[i].clone())
    }
}

#[cfg(test)]
mod tests {
    use crate::{Browser, Event};

    #[test]
    fn up_and_down_walk_the_remembered_searches() {
        let (mut browser, _) = Browser::new(Vec::new(), Vec::new());
        assert_eq!(browser.recall_search(true), None);
        for query in ["oak", "stone", "oak"] {
            browser.handle(Event::Search(query.into()), false, &[]);
            browser.remember_search();
        }
        assert_eq!(browser.search_history_text(), "stone\noak\n");
        assert_eq!(browser.recall_search(true).as_deref(), Some("oak"));
        assert_eq!(browser.recall_search(true).as_deref(), Some("stone"));
        assert_eq!(browser.recall_search(true).as_deref(), Some("stone"));
        assert_eq!(browser.recall_search(false).as_deref(), Some("oak"));
        assert_eq!(browser.recall_search(false), None);
        browser.load_search_history(&"q\n".repeat(40));
        assert_eq!(browser.search_history_text().lines().count(), 30);
    }
}

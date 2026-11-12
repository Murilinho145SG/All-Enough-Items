use crate::Lookup;

#[derive(Default)]
pub(super) struct History {
    entries: Vec<Lookup>,
    // Zero is the catalogue; otherwise this is one past the current lookup.
    cursor: usize,
}

impl History {
    pub(super) fn current(&self) -> Option<&Lookup> {
        self.cursor.checked_sub(1).and_then(|i| self.entries.get(i))
    }

    pub(super) fn current_mut(&mut self) -> Option<&mut Lookup> {
        self.cursor
            .checked_sub(1)
            .and_then(|i| self.entries.get_mut(i))
    }

    pub(super) fn push(&mut self, lookup: Lookup) {
        self.entries.truncate(self.cursor);
        self.entries.push(lookup);
        self.cursor = self.entries.len();
    }

    pub(super) fn back(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub(super) fn forward(&mut self) {
        self.cursor = (self.cursor + 1).min(self.entries.len());
    }
}

use super::Browser;
use crate::{ITEM_LIMIT, ItemInfo, Stack, Update};

#[derive(Default)]
pub(super) struct SearchQuery {
    groups: Vec<Vec<SearchTerm>>,
}

struct SearchTerm {
    field: SearchField,
    value: String,
    excluded: bool,
}

#[derive(Clone, Copy)]
enum SearchField {
    NameOrId,
    Namespace,
    Tag,
    Identifier,
}

impl SearchQuery {
    pub(super) fn parse(query: &str) -> Self {
        let mut groups = vec![Vec::new()];
        let mut token = String::new();
        let mut excluded = false;
        let mut token_started = false;
        let mut quoted = false;
        let mut escaped = false;

        for character in query.chars() {
            if escaped {
                token.push(character);
                token_started = true;
                escaped = false;
                continue;
            }
            if character == '\\' {
                escaped = true;
                token_started = true;
                continue;
            }
            if character == '"' {
                quoted = !quoted;
                token_started = true;
                continue;
            }
            if !quoted && character.is_whitespace() {
                Self::finish_token(&mut groups, &mut token, &mut excluded, &mut token_started);
                continue;
            }
            if !quoted && character == '|' {
                Self::finish_token(&mut groups, &mut token, &mut excluded, &mut token_started);
                if !groups.last().is_some_and(Vec::is_empty) {
                    groups.push(Vec::new());
                }
                continue;
            }
            if !quoted && !token_started && character == '-' {
                excluded = true;
                token_started = true;
                continue;
            }
            token.push(character);
            token_started = true;
        }

        if escaped {
            token.push('\\');
        }
        Self::finish_token(&mut groups, &mut token, &mut excluded, &mut token_started);
        groups.retain(|group| !group.is_empty());
        Self { groups }
    }

    fn finish_token(
        groups: &mut [Vec<SearchTerm>],
        token: &mut String,
        excluded: &mut bool,
        token_started: &mut bool,
    ) {
        if !token.is_empty() {
            let (field, value) = match token.chars().next().unwrap_or_default() {
                '@' => (SearchField::Namespace, &token[1..]),
                '#' => (SearchField::Tag, &token[1..]),
                '&' => (SearchField::Identifier, &token[1..]),
                _ => (SearchField::NameOrId, token.as_str()),
            };
            if !value.is_empty()
                && let Some(group) = groups.last_mut()
            {
                group.push(SearchTerm {
                    field,
                    value: value.to_lowercase(),
                    excluded: *excluded,
                });
            }
        }
        token.clear();
        *excluded = false;
        *token_started = false;
    }

    fn matches(&self, item: &ItemInfo) -> bool {
        if item.id == "minecraft:air" {
            return false;
        }
        self.groups.is_empty()
            || self.groups.iter().any(|group| {
                group.iter().all(|term| {
                    let matches = match term.field {
                        SearchField::NameOrId => {
                            item.name.to_lowercase().contains(&term.value)
                                || item.id.to_lowercase().contains(&term.value)
                        }
                        SearchField::Namespace => {
                            item.id.split_once(':').is_some_and(|(namespace, _)| {
                                namespace.to_lowercase().contains(&term.value)
                            })
                        }
                        SearchField::Tag => item
                            .tags
                            .iter()
                            .any(|tag| tag.to_lowercase().contains(&term.value)),
                        SearchField::Identifier => item.id.to_lowercase().contains(&term.value),
                    };
                    matches != term.excluded
                })
            })
    }
}

impl Browser {
    fn searchable(&self, item: &ItemInfo) -> bool {
        self.search.matches(item)
    }

    pub(super) fn item_list(&mut self) -> Update {
        self.filtered_items = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| self.searchable(item).then_some(index))
            .take(ITEM_LIMIT)
            .collect();
        Update::ItemList(
            self.filtered_items
                .iter()
                .map(|&index| Stack {
                    id: self.items[index].id.clone(),
                    count: 1,
                    components: String::new(),
                })
                .collect(),
        )
    }
}

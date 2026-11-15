//! The item list's search grammar, after JEI's: plain words match names, and prefixes
//! pick another field: `@` mod, `#` tooltip, `$` tag, `%` creative tab, `&` identifier.
//! Terms combine (all must match), `|` separates alternatives, `-` excludes a term and
//! quotes keep spaces. Each prefixed field has a [`SearchMode`].

use super::Browser;
use crate::{Entry, ITEM_LIMIT, ItemInfo, SearchMode, SearchModes, Update};

#[derive(Default)]
pub(super) struct SearchQuery {
    groups: Vec<Vec<SearchTerm>>,
}

struct SearchTerm {
    /// `None`: no prefix, matched against the name and every [`SearchMode::Enabled`] field.
    field: Option<Field>,
    value: String,
    excluded: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Field {
    Mod,
    Tooltip,
    Tag,
    Tab,
    Id,
}

impl Field {
    const ALL: [Self; 5] = [Self::Mod, Self::Tooltip, Self::Tag, Self::Tab, Self::Id];

    fn prefix(self) -> char {
        match self {
            Self::Mod => '@',
            Self::Tooltip => '#',
            Self::Tag => '$',
            Self::Tab => '%',
            Self::Id => '&',
        }
    }

    fn mode(self, modes: &SearchModes) -> SearchMode {
        match self {
            Self::Mod => modes.mod_name,
            Self::Tooltip => modes.tooltip,
            Self::Tag => modes.tag,
            Self::Tab => modes.tab,
            Self::Id => modes.id,
        }
    }
}

/// What one catalogue entry offers to the search, all lower-cased once.
pub(super) struct Indexed {
    name: String,
    namespace: String,
    id: String,
    tooltip: Vec<String>,
    tags: Vec<String>,
    tabs: Vec<String>,
}

impl Indexed {
    pub(super) fn new(entry: &Entry, item: Option<&ItemInfo>) -> Self {
        let id = entry.stack.id.to_lowercase();
        Self {
            name: entry.name.to_lowercase(),
            namespace: id
                .split_once(':')
                .map_or("minecraft", |(ns, _)| ns)
                .to_owned(),
            id,
            tooltip: entry.tooltip.iter().map(|l| l.to_lowercase()).collect(),
            tags: item.map_or_else(Vec::new, |item| {
                item.tags.iter().map(|t| t.to_lowercase()).collect()
            }),
            tabs: entry.tabs.iter().map(|t| t.to_lowercase()).collect(),
        }
    }

    fn field(&self, field: Field, value: &str) -> bool {
        let any = |list: &[String]| list.iter().any(|s| s.contains(value));
        match field {
            // The namespace is the mod's id; vanilla's mod is called "Minecraft".
            Field::Mod => self.namespace.contains(value),
            Field::Tooltip => any(&self.tooltip),
            Field::Tag => any(&self.tags),
            Field::Tab => any(&self.tabs),
            Field::Id => self.id.contains(value),
        }
    }
}

impl SearchQuery {
    pub(super) fn parse(query: &str, modes: &SearchModes) -> Self {
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
                Self::finish_token(&mut groups, &mut token, &mut excluded, modes);
                token_started = false;
                continue;
            }
            if !quoted && character == '|' {
                Self::finish_token(&mut groups, &mut token, &mut excluded, modes);
                token_started = false;
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
        Self::finish_token(&mut groups, &mut token, &mut excluded, modes);
        groups.retain(|group| !group.is_empty());
        Self { groups }
    }

    fn finish_token(
        groups: &mut [Vec<SearchTerm>],
        token: &mut String,
        excluded: &mut bool,
        modes: &SearchModes,
    ) {
        if !token.is_empty() {
            let first = token.chars().next().unwrap_or_default();
            // A disabled field's prefix is an ordinary character, as in JEI.
            let field = Field::ALL
                .into_iter()
                .find(|f| f.prefix() == first && f.mode(modes) != SearchMode::Disabled);
            let value = if field.is_some() {
                &token[first.len_utf8()..]
            } else {
                token.as_str()
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
    }

    fn matches(&self, entry: &Indexed, modes: &SearchModes) -> bool {
        self.groups.is_empty()
            || self.groups.iter().any(|group| {
                group.iter().all(|term| {
                    let value = term.value.as_str();
                    let matches = match term.field {
                        Some(field) => entry.field(field, value),
                        None => {
                            entry.name.contains(value)
                                || Field::ALL.into_iter().any(|f| {
                                    f.mode(modes) == SearchMode::Enabled && entry.field(f, value)
                                })
                        }
                    };
                    matches != term.excluded
                })
            })
    }
}

impl Browser {
    pub(super) fn item_list(&mut self) -> Update {
        let (search, modes) = (&self.search, &self.search_modes);
        self.filtered_items = self
            .indexed
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| search.matches(entry, modes).then_some(index))
            .take(ITEM_LIMIT)
            .collect();
        Update::ItemList(
            self.filtered_items
                .iter()
                .map(|&index| self.catalogue[index].stack.clone())
                .collect(),
        )
    }
}

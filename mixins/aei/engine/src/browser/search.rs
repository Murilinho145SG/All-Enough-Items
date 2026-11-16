//! The item list's search grammar, after JEI's: plain words match names, and prefixes
//! pick another field: `@` mod, `#` tooltip, `$` tag, `%` creative tab, `&` identifier.
//! Terms combine (all must match), `|` separates alternatives, `-` excludes a term and
//! quotes keep spaces. Each prefixed field has a [`SearchMode`].

use super::Browser;
use crate::{Entry, EntryProps, ITEM_LIMIT, ItemInfo, SearchMode, SearchModes, Update};

#[derive(Default)]
pub(super) struct SearchQuery {
    groups: Vec<Vec<SearchTerm>>,
}

struct SearchTerm {
    /// `None`: no prefix, matched against the name and every [`SearchMode::Enabled`] field.
    field: Option<Field>,
    /// An item filter (`is:food`) instead of text.
    filter: Option<Filter>,
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

/// What one catalogue entry offers to the search, folded once (lower case, accents off).
pub(super) struct Indexed {
    name: String,
    /// The name's and aliases' words, for typo-tolerant matches.
    words: Vec<String>,
    aliases: Vec<String>,
    namespace: String,
    id: String,
    tooltip: Vec<String>,
    tags: Vec<String>,
    tabs: Vec<String>,
    props: EntryProps,
    has_recipe: bool,
    /// Its registry index, for filters that change with the game (craftable now).
    pub(super) item: Option<usize>,
    /// The inventory holds what one of its crafting recipes needs, in the open grid.
    pub(super) craftable: bool,
    /// Found this session and not looked at yet.
    pub(super) fresh: bool,
}

/// Lower case without accents, so `pá` finds `pa` and `Ç` finds `c`.
pub(crate) fn fold(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
            'ç' | 'ć' | 'č' => 'c',
            'ď' | 'đ' => 'd',
            'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ė' | 'ę' | 'ě' => 'e',
            'ì' | 'í' | 'î' | 'ï' | 'ī' | 'į' | 'ı' => 'i',
            'ł' | 'ľ' | 'ĺ' => 'l',
            'ñ' | 'ń' | 'ň' => 'n',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ő' => 'o',
            'ř' | 'ŕ' => 'r',
            'ś' | 'š' | 'ş' | 'ß' => 's',
            'ť' | 'ţ' => 't',
            'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ů' | 'ű' => 'u',
            'ý' | 'ÿ' => 'y',
            'ź' | 'ż' | 'ž' => 'z',
            c => c,
        })
        .collect()
}

/// Whether `a` and `b` are at most one edit apart (insert, delete, replace or swap two
/// neighbours).
fn one_edit(a: &str, b: &str) -> bool {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    if a.len().abs_diff(b.len()) > 1 {
        return false;
    }
    let prefix = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    if prefix == a.len() && prefix == b.len() {
        return true;
    }
    let (ra, rb) = (&a[prefix..], &b[prefix..]);
    match (ra.len(), rb.len()) {
        (x, y) if x == y => {
            ra[1..] == rb[1..] || (x >= 2 && ra[0] == rb[1] && ra[1] == rb[0] && ra[2..] == rb[2..])
        }
        (x, y) if x > y => ra[1..] == *rb,
        _ => *ra == rb[1..],
    }
}

/// How well an entry matches: exactly, or only within one typo.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(super) enum Quality {
    Exact,
    Fuzzy,
}

impl Indexed {
    pub(super) fn new(entry: &Entry, item: Option<&ItemInfo>, has_recipe: bool) -> Self {
        let id = entry.stack.id.to_lowercase();
        let name = fold(&entry.name);
        let aliases: Vec<String> = entry.aliases.iter().map(|a| fold(a)).collect();
        let words = std::iter::once(&name)
            .chain(&aliases)
            .flat_map(|text| text.split(|c: char| !c.is_alphanumeric()))
            .filter(|word| word.chars().count() >= 3)
            .map(str::to_owned)
            .collect();
        Self {
            words,
            name,
            aliases,
            namespace: id
                .split_once(':')
                .map_or("minecraft", |(ns, _)| ns)
                .to_owned(),
            id,
            tooltip: entry.tooltip.iter().map(|l| fold(l)).collect(),
            tags: item.map_or_else(Vec::new, |item| {
                item.tags.iter().map(|t| t.to_lowercase()).collect()
            }),
            tabs: entry.tabs.iter().map(|t| fold(t)).collect(),
            props: entry.props.clone(),
            has_recipe,
            item: None,
            craftable: false,
            fresh: false,
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

    /// A plain word: the name or an alias, or, for four letters or more, a name word
    /// one typo away (a word longer than the term is compared by its start).
    fn name(&self, value: &str) -> Option<Quality> {
        if self.name.contains(value) || self.aliases.iter().any(|a| a.contains(value)) {
            return Some(Quality::Exact);
        }
        let length = value.chars().count();
        (length >= 4
            && self.words.iter().any(|word| {
                let start: String = word.chars().take(length + 1).collect();
                let cut: String = word.chars().take(length).collect();
                one_edit(word, value) || one_edit(&cut, value) || one_edit(&start, value)
            }))
        .then_some(Quality::Fuzzy)
    }

    fn filter(&self, filter: &Filter) -> bool {
        let p = &self.props;
        match filter {
            Filter::Is(what) => match what.as_str() {
                "food" => p.nutrition.is_some(),
                "enchantable" => p.enchantable,
                "block" => p.block,
                "tool" | "damageable" => p.durability > 0,
                "stackable" => p.max_stack > 1,
                "craftable" => self.craftable,
                "new" => self.fresh,
                _ => false,
            },
            Filter::Recipe(wanted) => self.has_recipe == *wanted,
            Filter::Rarity(rarity) => p.rarity.starts_with(rarity.as_str()),
            Filter::Number(field, op, n) => {
                let value = match field {
                    NumberField::Stack => Some(p.max_stack),
                    NumberField::Durability => (p.durability > 0).then_some(p.durability),
                    NumberField::Nutrition => p.nutrition,
                };
                value.is_some_and(|v| match op {
                    '<' => v < *n,
                    '>' => v > *n,
                    _ => v == *n,
                })
            }
        }
    }
}

/// AEI's item filters: `is:food`, `has:recipe`, `no:recipe`, `rarity:epic`,
/// `stack:1`, `dur>500` and `nutrition>4` (`<`, `>`, `=` or `:` compare numbers).
#[derive(Clone, Debug, PartialEq)]
enum Filter {
    Is(String),
    Recipe(bool),
    Rarity(String),
    Number(NumberField, char, u32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum NumberField {
    Stack,
    Durability,
    Nutrition,
}

impl Filter {
    fn parse(token: &str) -> Option<Self> {
        let at = token.find([':', '<', '>', '='])?;
        let (key, rest) = token.split_at(at);
        let mut chars = rest.chars();
        let op = chars.next()?;
        let value = chars.as_str().to_lowercase();
        let number = |field| Some(Self::Number(field, op, value.parse().ok()?));
        match (key.to_lowercase().as_str(), op) {
            ("is", ':') if !value.is_empty() => Some(Self::Is(value)),
            ("has", ':') if value == "recipe" => Some(Self::Recipe(true)),
            ("no", ':') if value == "recipe" => Some(Self::Recipe(false)),
            ("rarity", ':') if !value.is_empty() => Some(Self::Rarity(value)),
            ("stack", _) => number(NumberField::Stack),
            ("dur" | "durability", _) => number(NumberField::Durability),
            ("nutrition" | "food", _) => number(NumberField::Nutrition),
            _ => None,
        }
    }
}

impl SearchQuery {
    /// Whether the query reads `is:craftable`, which changes with the inventory.
    pub(super) fn reads_inventory(&self) -> bool {
        self.groups
            .iter()
            .flatten()
            .any(|term| matches!(&term.filter, Some(Filter::Is(what)) if what == "craftable"))
    }

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
        if let Some(filter) = Filter::parse(token)
            && let Some(group) = groups.last_mut()
        {
            group.push(SearchTerm {
                field: None,
                filter: Some(filter),
                value: String::new(),
                excluded: *excluded,
            });
        } else if !token.is_empty() {
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
                    filter: None,
                    value: fold(value),
                    excluded: *excluded,
                });
            }
        }
        token.clear();
        *excluded = false;
    }

    /// How well `entry` matches: the best alternative group, a group being as good as
    /// its worst term. Excluded terms count only exact matches.
    fn matches(&self, entry: &Indexed, modes: &SearchModes) -> Option<Quality> {
        if self.groups.is_empty() {
            return Some(Quality::Exact);
        }
        self.groups
            .iter()
            .filter_map(|group| {
                group.iter().try_fold(Quality::Exact, |worst, term| {
                    let value = term.value.as_str();
                    let found = if let Some(filter) = &term.filter {
                        entry.filter(filter).then_some(Quality::Exact)
                    } else {
                        match term.field {
                            Some(field) => entry.field(field, value).then_some(Quality::Exact),
                            None if Field::ALL.into_iter().any(|f| {
                                f.mode(modes) == SearchMode::Enabled && entry.field(f, value)
                            }) =>
                            {
                                Some(Quality::Exact)
                            }
                            None => entry.name(value),
                        }
                    };
                    match (term.excluded, found) {
                        (true, Some(Quality::Exact)) => None,
                        (true, _) => Some(worst),
                        (false, Some(quality)) => Some(worst.max(quality)),
                        (false, None) => None,
                    }
                })
            })
            .min()
    }
}

impl Browser {
    /// Whether a search is typed and `item` is among its results (for highlighting
    /// the inventory). None without a search.
    pub fn in_results(&self, item: &str) -> Option<bool> {
        (!self.query.trim().is_empty()).then(|| self.result_ids.contains(item))
    }

    pub(super) fn item_list(&mut self) -> Update {
        let (search, modes) = (&self.search, &self.search_modes);
        // Exact matches first, then the ones only a typo away, each in list order.
        let mut fuzzy = Vec::new();
        let mut exact = Vec::new();
        for &index in &self.order {
            if !self.edit_mode && self.is_hidden(index) {
                continue;
            }
            if self.spoiler_free && !self.discovered(&self.catalogue[index].stack.id) {
                continue;
            }
            match search.matches(&self.indexed[index], modes) {
                Some(Quality::Exact) => exact.push(index),
                Some(Quality::Fuzzy) => fuzzy.push(index),
                None => {}
            }
        }
        exact.extend(fuzzy);
        exact.truncate(ITEM_LIMIT);
        self.result_ids = exact
            .iter()
            .map(|&i| self.catalogue[i].stack.id.clone())
            .collect();
        self.filtered_items = exact;
        Update::ItemList(
            self.filtered_items
                .iter()
                .map(|&index| self.catalogue[index].stack.clone())
                .collect(),
        )
    }
}

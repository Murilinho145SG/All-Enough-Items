//! A small regular expression matcher for `/.../` searches, with no dependency: literals,
//! `.`, classes (`[a-z]`, `[^0-9]`), `\d \w \s` and their capitals, anchors `^ $`, groups,
//! alternatives `|`, and `* + ? {n} {n,} {n,m}`. Matching ignores case and finds the
//! pattern anywhere in the text (a Thompson NFA run as a Pike VM, so no pattern backtracks
//! for long).

/// A compiled pattern.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Regex {
    program: Vec<Inst>,
}

#[derive(Clone, Debug, PartialEq)]
enum Inst {
    Char(char),
    Any,
    Class(Class),
    Start,
    End,
    Split(usize, usize),
    Jump(usize),
    Match,
}

#[derive(Clone, Debug, PartialEq)]
struct Class {
    negated: bool,
    ranges: Vec<(char, char)>,
}

impl Class {
    fn matches(&self, c: char) -> bool {
        self.ranges.iter().any(|&(lo, hi)| lo <= c && c <= hi) != self.negated
    }
}

#[derive(Clone, Debug)]
enum Node {
    Char(char),
    Any,
    Class(Class),
    Start,
    End,
    Group(Vec<Vec<Node>>),
    Repeat(Box<Node>, usize, Option<usize>),
}

/// A repetition count above this is refused (it would grow the program too much).
const MAX_REPEAT: usize = 64;

impl Regex {
    /// Compiles `pattern`; `None` when it isn't a valid pattern.
    pub(crate) fn new(pattern: &str) -> Option<Self> {
        let chars: Vec<char> = pattern.chars().collect();
        let mut at = 0;
        let tree = parse_alternatives(&chars, &mut at)?;
        if at != chars.len() {
            return None;
        }
        let mut program = Vec::new();
        emit_alternatives(&tree, &mut program);
        program.push(Inst::Match);
        Some(Self { program })
    }

    /// Whether the pattern matches somewhere in `text`, ignoring case.
    pub(crate) fn is_match(&self, text: &str) -> bool {
        let text: Vec<char> = text.chars().flat_map(char::to_lowercase).collect();
        let mut current: Vec<usize> = Vec::new();
        let mut next: Vec<usize> = Vec::new();
        let mut seen = vec![usize::MAX; self.program.len()];
        for pos in 0..=text.len() {
            // A new thread at every position: the match may start anywhere.
            if self.add(&mut current, &mut seen, 0, pos, &text) {
                return true;
            }
            let Some(&c) = text.get(pos) else {
                break;
            };
            next.clear();
            for &pc in &current {
                let step = match &self.program[pc] {
                    Inst::Char(want) => *want == c,
                    Inst::Any => true,
                    Inst::Class(class) => class.matches(c),
                    _ => false,
                };
                if step && self.add(&mut next, &mut seen, pc + 1, pos + 1, &text) {
                    return true;
                }
            }
            std::mem::swap(&mut current, &mut next);
        }
        false
    }

    /// Adds the thread at `pc` (following jumps, splits and anchors) for position `pos`;
    /// true when it reaches the end of the pattern.
    fn add(
        &self,
        list: &mut Vec<usize>,
        seen: &mut [usize],
        pc: usize,
        pos: usize,
        text: &[char],
    ) -> bool {
        if seen[pc] == pos {
            return false;
        }
        seen[pc] = pos;
        match self.program[pc] {
            Inst::Match => true,
            Inst::Jump(to) => self.add(list, seen, to, pos, text),
            Inst::Split(a, b) => {
                self.add(list, seen, a, pos, text) || self.add(list, seen, b, pos, text)
            }
            Inst::Start => pos == 0 && self.add(list, seen, pc + 1, pos, text),
            Inst::End => pos == text.len() && self.add(list, seen, pc + 1, pos, text),
            _ => {
                list.push(pc);
                false
            }
        }
    }
}

fn parse_alternatives(chars: &[char], at: &mut usize) -> Option<Vec<Vec<Node>>> {
    let mut alternatives = vec![parse_sequence(chars, at)?];
    while chars.get(*at) == Some(&'|') {
        *at += 1;
        alternatives.push(parse_sequence(chars, at)?);
    }
    Some(alternatives)
}

fn parse_sequence(chars: &[char], at: &mut usize) -> Option<Vec<Node>> {
    let mut sequence = Vec::new();
    while let Some(&c) = chars.get(*at) {
        if c == '|' || c == ')' {
            break;
        }
        let atom = parse_atom(chars, at)?;
        sequence.push(parse_repeat(chars, at, atom)?);
    }
    Some(sequence)
}

fn parse_atom(chars: &[char], at: &mut usize) -> Option<Node> {
    let c = *chars.get(*at)?;
    *at += 1;
    Some(match c {
        '.' => Node::Any,
        '^' => Node::Start,
        '$' => Node::End,
        '(' => {
            // `(?:` groups the same way (nothing is captured).
            if chars.get(*at..*at + 2) == Some(&['?', ':']) {
                *at += 2;
            }
            let inner = parse_alternatives(chars, at)?;
            if chars.get(*at) != Some(&')') {
                return None;
            }
            *at += 1;
            Node::Group(inner)
        }
        '[' => Node::Class(parse_class(chars, at)?),
        '\\' => {
            let e = *chars.get(*at)?;
            *at += 1;
            match escape_class(e) {
                Some(class) => Node::Class(class),
                None => Node::Char(e.to_lowercase().next().unwrap_or(e)),
            }
        }
        '*' | '+' | '?' | '{' | ')' => return None,
        c => Node::Char(c.to_lowercase().next().unwrap_or(c)),
    })
}

/// `\d`, `\w`, `\s` and their negations.
fn escape_class(e: char) -> Option<Class> {
    let ranges = match e.to_ascii_lowercase() {
        'd' => vec![('0', '9')],
        'w' => vec![('a', 'z'), ('0', '9'), ('_', '_'), ('A', 'Z')],
        's' => vec![(' ', ' '), ('\t', '\t'), ('\n', '\n'), ('\r', '\r')],
        _ => return None,
    };
    Some(Class {
        negated: e.is_ascii_uppercase(),
        ranges,
    })
}

fn parse_class(chars: &[char], at: &mut usize) -> Option<Class> {
    let negated = chars.get(*at) == Some(&'^');
    if negated {
        *at += 1;
    }
    let mut ranges = Vec::new();
    let mut first = true;
    loop {
        let c = *chars.get(*at)?;
        *at += 1;
        if c == ']' && !first {
            break;
        }
        first = false;
        let lo = if c == '\\' {
            let e = *chars.get(*at)?;
            *at += 1;
            if let Some(class) = escape_class(e) {
                if class.negated {
                    return None;
                }
                ranges.extend(class.ranges);
                continue;
            }
            e
        } else {
            c
        };
        let hi = if chars.get(*at) == Some(&'-') && chars.get(*at + 1).is_some_and(|c| *c != ']') {
            let hi = chars[*at + 1];
            *at += 2;
            hi
        } else {
            lo
        };
        if lo > hi {
            return None;
        }
        // Case is ignored: a range of letters stands for both cases.
        ranges.push((lo, hi));
        let (l, h) = (lo.to_ascii_lowercase(), hi.to_ascii_lowercase());
        if (l, h) != (lo, hi) && lo.is_ascii_alphabetic() && hi.is_ascii_alphabetic() {
            ranges.push((l, h));
        }
    }
    Some(Class { negated, ranges })
}

fn parse_repeat(chars: &[char], at: &mut usize, atom: Node) -> Option<Node> {
    let (min, max) = match chars.get(*at) {
        Some('*') => (0, None),
        Some('+') => (1, None),
        Some('?') => (0, Some(1)),
        Some('{') => {
            let close = chars[*at..].iter().position(|c| *c == '}')? + *at;
            let inside: String = chars[*at + 1..close].iter().collect();
            let (min, max) = match inside.split_once(',') {
                None => {
                    let n = inside.trim().parse().ok()?;
                    (n, Some(n))
                }
                Some((lo, hi)) if hi.trim().is_empty() => (lo.trim().parse().ok()?, None),
                Some((lo, hi)) => (lo.trim().parse().ok()?, Some(hi.trim().parse().ok()?)),
            };
            if max.is_some_and(|m| m < min) || min > MAX_REPEAT || max.unwrap_or(0) > MAX_REPEAT {
                return None;
            }
            *at = close;
            (min, max)
        }
        _ => return Some(atom),
    };
    *at += 1;
    if matches!(atom, Node::Start | Node::End) {
        return None;
    }
    Some(Node::Repeat(Box::new(atom), min, max))
}

fn emit_alternatives(alternatives: &[Vec<Node>], program: &mut Vec<Inst>) {
    let mut jumps = Vec::new();
    for (i, sequence) in alternatives.iter().enumerate() {
        let last = i + 1 == alternatives.len();
        let split = program.len();
        if !last {
            program.push(Inst::Split(split + 1, 0));
        }
        for node in sequence {
            emit(node, program);
        }
        if !last {
            jumps.push(program.len());
            program.push(Inst::Jump(0));
            let next = program.len();
            program[split] = Inst::Split(split + 1, next);
        }
    }
    let end = program.len();
    for jump in jumps {
        program[jump] = Inst::Jump(end);
    }
}

fn emit(node: &Node, program: &mut Vec<Inst>) {
    match node {
        Node::Char(c) => program.push(Inst::Char(*c)),
        Node::Any => program.push(Inst::Any),
        Node::Class(class) => program.push(Inst::Class(class.clone())),
        Node::Start => program.push(Inst::Start),
        Node::End => program.push(Inst::End),
        Node::Group(alternatives) => emit_alternatives(alternatives, program),
        Node::Repeat(inner, min, max) => {
            for _ in 0..*min {
                emit(inner, program);
            }
            match max {
                None => {
                    // L: split(body, out); body; jump L
                    let split = program.len();
                    program.push(Inst::Split(split + 1, 0));
                    emit(inner, program);
                    program.push(Inst::Jump(split));
                    let out = program.len();
                    program[split] = Inst::Split(split + 1, out);
                }
                Some(max) => {
                    let mut splits = Vec::new();
                    for _ in *min..*max {
                        splits.push(program.len());
                        program.push(Inst::Split(program.len() + 1, 0));
                        emit(inner, program);
                    }
                    let out = program.len();
                    for split in splits {
                        program[split] = Inst::Split(split + 1, out);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Regex;

    fn m(pattern: &str, text: &str) -> bool {
        Regex::new(pattern).unwrap().is_match(text)
    }

    #[test]
    fn patterns_match_anywhere_ignoring_case() {
        assert!(m("oak", "Dark Oak Planks"));
        assert!(m("^dark", "Dark Oak"));
        assert!(!m("^oak", "Dark Oak"));
        assert!(m("planks$", "oak planks"));
        assert!(m("o.k", "oak"));
        assert!(m("(oak|birch)_log$", "minecraft:birch_log"));
        assert!(!m("(oak|birch)_log$", "minecraft:spruce_log"));
        assert!(m("^[a-c]", "Birch"));
        assert!(m("[^a-z ]", "a b1"));
        assert!(!m("[^a-z ]", "a b"));
        assert!(m(r"\d+", "level 12"));
        assert!(m(r"^\w+ \w+$", "oak log"));
        assert!(m("^a{2,3}$", "aaa"));
        assert!(!m("^a{2,3}$", "aaaa"));
        assert!(m("^ab*c?$", "abbb"));
        assert!(m(r"a\.b", "a.b"));
        assert!(!m(r"a\.b", "axb"));
        assert!(m("^(?:ab)+$", "ababab"));
        assert!(m("", "anything"));
    }

    #[test]
    fn bad_patterns_are_refused() {
        for bad in ["(", ")", "[a-", "*a", "a{3,1}", "a{999}", "[z-a]", "a|*"] {
            assert!(Regex::new(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn nested_repeats_stay_fast() {
        let start = std::time::Instant::now();
        assert!(!m("^(a+)+$", &format!("{}b", "a".repeat(40))));
        assert!(start.elapsed().as_millis() < 200);
    }
}

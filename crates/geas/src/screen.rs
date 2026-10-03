//! The screen (DESIGN §8): what a GUI target shows once it has settled, as its
//! accessibility tree, the same shape whatever drove the app. A node has a role, a
//! name, a value, states and children; claims say which nodes the screen contains,
//! by role and name, never by a widget's class, an element's id or a pixel.
//!
//! Here: the node and its forms (one node a line for a person, JSON for the
//! journal, the baseline and a driver), the rules every driver's tree is put
//! through (§8.1), node patterns and what they match (§8.6), `mask screen`, the
//! tree diff drift reads, and the cut of a large screen a failed check shows.

use crate::json::{self, J};
use crate::regex;

/// The roles a node pattern may name (PLAN Appendix D): WAI-ARIA 1.2's roles that
/// are not abstract, and `text` for static text, which ARIA has no role for.
pub const ROLES: &[&str] = &[
    "alert", "alertdialog", "application", "article", "banner", "blockquote", "button", "caption", "cell", "checkbox",
    "code", "columnheader", "combobox", "complementary", "contentinfo", "definition", "deletion", "dialog", "directory",
    "document", "emphasis", "feed", "figure", "form", "generic", "grid", "gridcell", "group", "heading", "img",
    "insertion", "link", "list", "listbox", "listitem", "log", "main", "marquee", "math", "menu", "menubar", "menuitem",
    "menuitemcheckbox", "menuitemradio", "meter", "navigation", "none", "note", "option", "paragraph", "presentation",
    "progressbar", "radio", "radiogroup", "region", "row", "rowgroup", "rowheader", "scrollbar", "search", "searchbox",
    "separator", "slider", "spinbutton", "status", "strong", "subscript", "superscript", "switch", "tab", "table",
    "tablist", "tabpanel", "term", "textbox", "time", "timer", "toolbar", "tooltip", "tree", "treegrid", "treeitem",
    "text",
];

/// The states a node can carry that claims can name, in the order they are written.
pub const NODE_STATES: &[&str] = &["disabled", "checked", "unchecked"];

/// The roles `click` presses (DESIGN §8.1).
pub const CLICKABLE: &[&str] = &["button", "link", "checkbox", "radio", "switch", "tab", "menuitem", "option"];

/// The roles `input` and `submit` type into: the text fields. A `combobox` is not
/// one: Chrome reports a `<select>` as one, and typing into it does nothing.
pub const FIELDS: &[&str] = &["textbox", "searchbox", "spinbutton"];

/// The most nodes a failed check shows of a screen.
pub const SHOWN: usize = 40;

#[derive(Debug, Clone, Default)]
pub struct Node {
    pub role: String,
    pub name: String,
    pub value: String,
    pub states: Vec<String>,
    pub children: Vec<Node>,
    /// Taken out by `mask screen`: recorded as its role alone (DESIGN §8.6).
    pub masked: bool,
    /// The driver's own hold on the node, for acting on it (Chrome's backend DOM
    /// node id). Never shown, written or compared.
    pub handle: Option<i64>,
}

impl PartialEq for Node {
    fn eq(&self, o: &Node) -> bool {
        self.role == o.role
            && self.name == o.name
            && self.value == o.value
            && self.states == o.states
            && self.masked == o.masked
            && self.children == o.children
    }
}

/// The screen as a node: a root without a role, whose children are what the
/// screen shows at its top level.
pub fn screen(children: Vec<Node>) -> Node {
    Node { children, ..Node::default() }
}

/// States in their written order: those claims name first, then any other a driver
/// reports, alphabetically, each once.
pub fn sort_states(states: &mut Vec<String>) {
    states.sort_by_key(|s| (NODE_STATES.iter().position(|k| k == s).unwrap_or(NODE_STATES.len()), s.clone()));
    states.dedup();
}

impl Node {
    #[cfg(test)]
    pub fn leaf(role: &str, name: &str) -> Node {
        Node { role: role.into(), name: name.into(), ..Node::default() }
    }

    pub fn has(&self, state: &str) -> bool {
        self.states.iter().any(|s| s == state)
    }

    /// The node on one line, as messages and the report write it:
    /// `textbox "type here" value "Ada"`, `button "retry" disabled`, `text <masked>`.
    pub fn line(&self) -> String {
        let mut s = self.role.clone();
        if self.masked {
            s.push_str(" <masked>");
            return s;
        }
        if !self.name.is_empty() {
            s.push(' ');
            s.push_str(&json::quote(&self.name));
        }
        if !self.value.is_empty() {
            s.push_str(" value ");
            s.push_str(&json::quote(&self.value));
        }
        for st in &self.states {
            s.push(' ');
            s.push_str(st);
        }
        s
    }

    /// The node and what is under it, one node a line, two spaces a level, `depth`
    /// levels in. A root without a role is the screen and is not a line itself.
    #[cfg(test)]
    pub fn text(&self, depth: usize) -> String {
        let mut out = String::new();
        let mut d = depth;
        if !self.role.is_empty() {
            out.push_str(&"  ".repeat(depth));
            out.push_str(&self.line());
            out.push('\n');
            d += 1;
        }
        for c in &self.children {
            out.push_str(&c.text(d));
        }
        out
    }

    /// How many nodes the tree holds, the root without a role left out.
    #[cfg(test)]
    pub fn count(&self) -> usize {
        usize::from(!self.role.is_empty()) + self.children.iter().map(Node::count).sum::<usize>()
    }

    /// Every node in tree order, with its ancestors (nearest last), the root
    /// without a role left out.
    pub fn walk(&self) -> Vec<(&Node, Vec<&Node>)> {
        fn go<'a>(n: &'a Node, above: &mut Vec<&'a Node>, out: &mut Vec<(&'a Node, Vec<&'a Node>)>) {
            let named = !n.role.is_empty();
            if named {
                out.push((n, above.clone()));
                above.push(n);
            }
            for c in &n.children {
                go(c, above, out);
            }
            if named {
                above.pop();
            }
        }
        let mut out = Vec::new();
        go(self, &mut Vec::new(), &mut out);
        out
    }

    /// The screen on one line for the run that gets there: the nodes in tree order,
    /// separated by commas, cut to `max` characters by the caller.
    pub fn summary(&self) -> String {
        let lines: Vec<String> = self.walk().iter().map(|(n, _)| n.line()).collect();
        if lines.is_empty() { "an empty screen".into() } else { lines.join(", ") }
    }

    /// As JSON, without the members that are empty: `{"role":…,"name":…,"value":…,
    /// "states":[…],"children":[…]}`; a masked node is `{"role":…,"masked":true}`.
    pub fn json(&self) -> String {
        let mut parts = Vec::new();
        if !self.role.is_empty() {
            parts.push(format!("\"role\":{}", json::quote(&self.role)));
        }
        if self.masked {
            parts.push("\"masked\":true".to_string());
        } else {
            if !self.name.is_empty() {
                parts.push(format!("\"name\":{}", json::quote(&self.name)));
            }
            if !self.value.is_empty() {
                parts.push(format!("\"value\":{}", json::quote(&self.value)));
            }
            if !self.states.is_empty() {
                let s: Vec<String> = self.states.iter().map(|x| json::quote(x)).collect();
                parts.push(format!("\"states\":[{}]", s.join(",")));
            }
        }
        if !self.children.is_empty() {
            let c: Vec<String> = self.children.iter().map(Node::json).collect();
            parts.push(format!("\"children\":[{}]", c.join(",")));
        }
        format!("{{{}}}", parts.join(","))
    }

    /// A node read from JSON, as a driver sends it or the baseline keeps it. A value
    /// may be sent as a number; members geas does not know are left alone.
    pub fn from_json(j: &J) -> Result<Node, String> {
        let J::Obj(pairs) = j else {
            return Err("a node is a JSON object".into());
        };
        let mut n = Node::default();
        for (k, v) in pairs {
            match (k.as_str(), v) {
                ("role", J::Str(s)) => n.role = s.clone(),
                ("name", J::Str(s)) => n.name = s.clone(),
                ("value", J::Str(s)) => n.value = s.clone(),
                ("value", J::Num(x)) => n.value = json::render_num(*x),
                ("value", J::Null) | ("name", J::Null) => {}
                ("states", J::Arr(items)) => {
                    for s in items {
                        match s {
                            J::Str(s) => n.states.push(s.clone()),
                            _ => return Err("`states` holds something that is not a string".into()),
                        }
                    }
                }
                ("children", J::Arr(items)) => {
                    for c in items {
                        n.children.push(Node::from_json(c)?);
                    }
                }
                ("masked", J::Bool(b)) => n.masked = *b,
                ("role" | "name" | "value" | "states" | "children" | "masked", _) => {
                    return Err(format!("`{k}` is not of the kind a node's `{k}` is"));
                }
                _ => {}
            }
        }
        sort_states(&mut n.states);
        Ok(n)
    }
}

/// The rules every driver's tree goes through (DESIGN §8.1): a node without a role
/// reports nothing and hands its children up; empty text reports nothing; a node
/// drops the text children that repeat its name, and a field the text child that
/// repeats its value, since those say again what the node already says.
pub fn normalize(nodes: Vec<Node>) -> Vec<Node> {
    let mut out = Vec::new();
    for mut n in nodes {
        n.children = normalize(std::mem::take(&mut n.children));
        if n.role.is_empty() {
            out.extend(n.children);
            continue;
        }
        if n.role == "text" && n.name.is_empty() && n.value.is_empty() && n.children.is_empty() {
            continue;
        }
        let repeats = |c: &Node| {
            c.role == "text"
                && c.children.is_empty()
                && c.value.is_empty()
                && !c.name.is_empty()
                && (c.name == n.name || c.name == n.value)
        };
        if n.children.iter().any(repeats) {
            let (name, value) = (n.name.clone(), n.value.clone());
            n.children.retain(|c| {
                !(c.role == "text" && c.children.is_empty() && c.value.is_empty() && !c.name.is_empty() && (c.name == name || c.name == value))
            });
        }
        sort_states(&mut n.states);
        out.push(n);
    }
    out
}

/// How a node pattern takes a node's name (DESIGN §6): the whole name, a part, or
/// a pattern the whole name matches.
#[derive(Debug, Clone)]
pub enum Label {
    Is(String),
    Containing(String),
    Matching(regex::Pattern),
}

impl Label {
    pub fn fits(&self, name: &str) -> bool {
        match self {
            Label::Is(s) => name == s,
            Label::Containing(s) => name.contains(s.as_str()),
            Label::Matching(p) => p.matches(name),
        }
    }

    pub fn shown(&self) -> String {
        match self {
            Label::Is(s) => json::quote(s),
            Label::Containing(s) => format!("containing {}", json::quote(s)),
            Label::Matching(p) => format!("matching {}", p.shown()),
        }
    }
}

/// A state a pattern asks for; `enabled` is a node that is not disabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Disabled,
    Enabled,
    Checked,
    Unchecked,
}

impl State {
    pub fn parse(w: &str) -> Option<State> {
        match w {
            "disabled" => Some(State::Disabled),
            "enabled" => Some(State::Enabled),
            "checked" => Some(State::Checked),
            "unchecked" => Some(State::Unchecked),
            _ => None,
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            State::Disabled => "disabled",
            State::Enabled => "enabled",
            State::Checked => "checked",
            State::Unchecked => "unchecked",
        }
    }

    fn fits(self, n: &Node) -> bool {
        match self {
            State::Disabled => n.has("disabled"),
            State::Enabled => !n.has("disabled"),
            State::Checked => n.has("checked"),
            State::Unchecked => n.has("unchecked"),
        }
    }
}

/// A node pattern (DESIGN §6, §8.6):
/// `["exactly" NUM] ROLE [label] ["with" "value" STR] [state] ["in" ROLE [label]]`.
#[derive(Debug, Clone)]
pub struct Pattern {
    pub count: Option<usize>,
    pub role: String,
    pub label: Option<Label>,
    pub value: Option<String>,
    pub state: Option<State>,
    pub within: Option<(String, Option<Label>)>,
}

impl Pattern {
    /// Whether the pattern matches a node, whose ancestors are `above`.
    pub fn matches(&self, n: &Node, above: &[&Node]) -> bool {
        n.role == self.role
            && !n.masked
            && self.label.as_ref().is_none_or(|l| l.fits(&n.name))
            && self.value.as_ref().is_none_or(|v| *v == n.value)
            && self.state.is_none_or(|s| s.fits(n))
            && self.within.as_ref().is_none_or(|(role, label)| {
                above.iter().any(|a| a.role == *role && !a.masked && label.as_ref().is_none_or(|l| l.fits(&a.name)))
            })
    }

    /// The nodes of a screen it matches, in tree order.
    pub fn found<'a>(&self, screen: &'a Node) -> Vec<&'a Node> {
        screen.walk().into_iter().filter(|(n, above)| self.matches(n, above)).map(|(n, _)| n).collect()
    }

    /// As the spec writes it: `exactly 2 button "greet" disabled in dialog "Confirm"`.
    pub fn shown(&self) -> String {
        let mut s = String::new();
        if let Some(n) = self.count {
            s.push_str(&format!("exactly {n} "));
        }
        s.push_str(&self.role);
        if let Some(l) = &self.label {
            s.push(' ');
            s.push_str(&l.shown());
        }
        if let Some(v) = &self.value {
            s.push_str(&format!(" with value {}", json::quote(v)));
        }
        if let Some(st) = self.state {
            s.push(' ');
            s.push_str(st.word());
        }
        if let Some((role, label)) = &self.within {
            s.push_str(&format!(" in {role}"));
            if let Some(l) = label {
                s.push(' ');
                s.push_str(&l.shown());
            }
        }
        s
    }

    /// Whether `contains` holds: some node matches, or with `exactly n`, n do.
    pub fn holds(&self, screen: &Node) -> bool {
        let n = self.found(screen).len();
        match self.count {
            Some(want) => n == want,
            None => n > 0,
        }
    }
}

/// The screen as the journal and the baseline record it: every node a `mask
/// screen` pattern matches, and what is under it, written as its role alone, so
/// that two runs write the same bytes where the spec declares noise.
pub fn masked(screen: &Node, masks: &[&Pattern]) -> Node {
    fn go(n: &Node, above: &mut Vec<Node>, masks: &[&Pattern]) -> Node {
        let refs: Vec<&Node> = above.iter().collect();
        if !n.role.is_empty() && masks.iter().any(|m| m.matches(n, &refs)) {
            return Node { role: n.role.clone(), masked: true, ..Node::default() };
        }
        let mut out = Node { children: vec![], ..n.clone() };
        if !n.role.is_empty() {
            above.push(Node { children: vec![], ..n.clone() });
        }
        out.children = n.children.iter().map(|c| go(c, above, masks)).collect();
        if !n.role.is_empty() {
            above.pop();
        }
        out
    }
    if masks.is_empty() {
        return screen.clone();
    }
    go(screen, &mut Vec::new(), masks)
}

/// One change between two screens (DESIGN §8.6): a node that appeared, one that
/// disappeared, or one whose value or states changed. The nodes come without their
/// children, each with its ancestors, nearest last.
#[derive(Debug, Clone)]
pub struct Change {
    pub old: Option<(Node, Vec<Node>)>,
    pub new: Option<(Node, Vec<Node>)>,
}

impl Change {
    /// Where the node is, for drift's report: `screen`, or `screen in list "Todos"`.
    pub fn place(&self) -> String {
        let above = self.new.as_ref().or(self.old.as_ref()).map(|(_, a)| a.as_slice()).unwrap_or(&[]);
        match above.last() {
            None => "screen".into(),
            Some(p) => {
                let mut parent = p.role.clone();
                if !p.name.is_empty() {
                    parent.push(' ');
                    parent.push_str(&json::quote(&p.name));
                }
                format!("screen in {parent}")
            }
        }
    }

    /// Whether a check's pattern speaks of the node, before or after.
    pub fn claimed_by(&self, p: &Pattern) -> bool {
        [&self.old, &self.new].into_iter().flatten().any(|(n, above)| {
            let refs: Vec<&Node> = above.iter().collect();
            p.matches(n, &refs)
        })
    }
}

/// The changes from `old` to `new`. The children of two aligned nodes are aligned
/// by role and name, as a longest common subsequence; a node with no partner
/// appeared or disappeared, and so did everything under it, each node its own
/// change. A masked node on either side is not compared, nor is a node a mask
/// matches now: a mask takes a node out of drift.
pub fn diff(old: &Node, new: &Node, masks: &[&Pattern]) -> Vec<Change> {
    let mut out = Vec::new();
    let mut above_old = Vec::new();
    let mut above_new = Vec::new();
    children_diff(old, new, &mut above_old, &mut above_new, masks, &mut out);
    out
}

fn bare(n: &Node) -> Node {
    Node { children: vec![], handle: None, ..n.clone() }
}

fn is_masked(n: &Node, above: &[Node], masks: &[&Pattern]) -> bool {
    let refs: Vec<&Node> = above.iter().collect();
    n.masked || masks.iter().any(|m| m.matches(n, &refs))
}

/// Every node of a subtree as an appearance or a disappearance.
fn all_of(n: &Node, above: &mut Vec<Node>, masks: &[&Pattern], appeared: bool, out: &mut Vec<Change>) {
    if is_masked(n, above, masks) {
        return;
    }
    let entry = Some((bare(n), above.clone()));
    out.push(if appeared { Change { old: None, new: entry } } else { Change { old: entry, new: None } });
    above.push(bare(n));
    for c in &n.children {
        all_of(c, above, masks, appeared, out);
    }
    above.pop();
}

fn children_diff(o: &Node, n: &Node, ao: &mut Vec<Node>, an: &mut Vec<Node>, masks: &[&Pattern], out: &mut Vec<Change>) {
    let (a, b) = (&o.children, &n.children);
    let key = |x: &Node| (x.role.clone(), x.name.clone(), x.masked);
    // the longest common subsequence by role and name
    let (len_a, len_b) = (a.len(), b.len());
    let mut lcs = vec![vec![0usize; len_b + 1]; len_a + 1];
    for i in (0..len_a).rev() {
        for j in (0..len_b).rev() {
            lcs[i][j] = if key(&a[i]) == key(&b[j]) { lcs[i + 1][j + 1] + 1 } else { lcs[i + 1][j].max(lcs[i][j + 1]) };
        }
    }
    let (mut i, mut j) = (0, 0);
    while i < len_a || j < len_b {
        if i < len_a && j < len_b && key(&a[i]) == key(&b[j]) {
            let (x, y) = (&a[i], &b[j]);
            let masked = is_masked(x, ao, masks) || is_masked(y, an, masks);
            if !masked {
                if x.value != y.value || x.states != y.states {
                    out.push(Change { old: Some((bare(x), ao.clone())), new: Some((bare(y), an.clone())) });
                }
                ao.push(bare(x));
                an.push(bare(y));
                children_diff(x, y, ao, an, masks, out);
                ao.pop();
                an.pop();
            }
            i += 1;
            j += 1;
        } else if i < len_a && (j == len_b || lcs[i + 1][j] >= lcs[i][j + 1]) {
            // what went comes before what came, as a diff reads
            all_of(&a[i], ao, masks, false, out);
            i += 1;
        } else {
            all_of(&b[j], an, masks, true, out);
            j += 1;
        }
    }
}

/// A screen cut for a failed check: whole when it has at most 40 nodes; else the
/// nodes the pattern matches, then those of its role, then the rest in tree order,
/// each with its ancestors so the shape stays, up to 40 lines, and a count of what
/// was left out. Returns the lines (two spaces a level) and how many nodes were
/// left out.
pub fn cut_for(screen: &Node, p: &Pattern) -> (Vec<String>, usize) {
    let all = screen.walk();
    let total = all.len();
    let lines_of = |keep: &dyn Fn(usize) -> bool| -> Vec<String> {
        all.iter()
            .enumerate()
            .filter(|(i, _)| keep(*i))
            .map(|(_, (n, above))| format!("{}{}", "  ".repeat(above.len()), n.line()))
            .collect()
    };
    if total <= SHOWN {
        return (lines_of(&|_| true), 0);
    }
    // the place of each node's ancestors in the walk
    let index_of = |target: &Node| all.iter().position(|(n, _)| std::ptr::eq(*n, target));
    let mut ranked: Vec<usize> = Vec::new();
    for pass in 0..3 {
        for (i, (n, above)) in all.iter().enumerate() {
            let fits = match pass {
                0 => p.matches(n, above),
                1 => n.role == p.role,
                _ => true,
            };
            if fits && !ranked.contains(&i) {
                ranked.push(i);
            }
        }
    }
    let mut keep = vec![false; total];
    let mut kept = 0;
    for i in ranked {
        let (_, above) = &all[i];
        let mut need: Vec<usize> = above.iter().filter_map(|a| index_of(a)).filter(|k| !keep[*k]).collect();
        need.push(i);
        need.dedup();
        if keep[i] {
            continue;
        }
        if kept + need.len() > SHOWN {
            continue;
        }
        for k in need {
            if !keep[k] {
                keep[k] = true;
                kept += 1;
            }
        }
    }
    (lines_of(&|i| keep[i]), total - kept)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(role: &str, name: &str) -> Node {
        Node::leaf(role, name)
    }

    fn with(mut node: Node, children: Vec<Node>) -> Node {
        node.children = children;
        node
    }

    fn greeter() -> Node {
        let mut field = n("textbox", "type here");
        field.value = "Ada".into();
        let mut retry = n("button", "retry");
        retry.states = vec!["disabled".into()];
        screen(vec![
            n("text", "Your name:"),
            field,
            n("button", "greet"),
            with(n("dialog", "Confirm"), vec![n("button", "OK"), retry]),
        ])
    }

    fn pat(role: &str, label: Option<Label>) -> Pattern {
        Pattern { count: None, role: role.into(), label, value: None, state: None, within: None }
    }

    #[test]
    fn rendering_one_node_a_line() {
        assert_eq!(
            greeter().text(0),
            "text \"Your name:\"\ntextbox \"type here\" value \"Ada\"\nbutton \"greet\"\ndialog \"Confirm\"\n  button \"OK\"\n  button \"retry\" disabled\n"
        );
        assert_eq!(greeter().count(), 6);
        assert_eq!(
            greeter().summary(),
            "text \"Your name:\", textbox \"type here\" value \"Ada\", button \"greet\", dialog \"Confirm\", button \"OK\", button \"retry\" disabled"
        );
        let m = Node { role: "text".into(), masked: true, ..Node::default() };
        assert_eq!(m.line(), "text <masked>");
    }

    #[test]
    fn json_both_ways() {
        let s = greeter();
        let text = s.json();
        assert!(text.starts_with("{\"children\":[{\"role\":\"text\",\"name\":\"Your name:\"}"), "{text}");
        assert!(text.contains("{\"role\":\"button\",\"name\":\"retry\",\"states\":[\"disabled\"]}"), "{text}");
        let back = Node::from_json(&json::parse(&text).unwrap()).unwrap();
        assert_eq!(back, s);
        assert_eq!(back.json(), text);
        // a value sent as a number, members geas does not know
        let j = json::parse(r#"{"role":"slider","value":0.5,"extra":1,"states":["checked","disabled"]}"#).unwrap();
        let back = Node::from_json(&j).unwrap();
        assert_eq!((back.value.as_str(), back.states.clone()), ("0.5", vec!["disabled".to_string(), "checked".to_string()]));
        assert!(Node::from_json(&json::parse(r#"{"role":1}"#).unwrap()).is_err());
        assert!(Node::from_json(&json::parse(r#"[1]"#).unwrap()).is_err());
    }

    #[test]
    fn the_rules_every_tree_goes_through() {
        let mut field = n("textbox", "Item");
        field.value = "milk".into();
        field.children = vec![n("text", "milk")];
        let tree = vec![with(
            Node::default(),
            vec![
                with(n("button", "Add"), vec![n("text", "Add")]),
                n("text", ""),
                field,
                with(Node::default(), vec![n("text", "inside a layout")]),
            ],
        )];
        let out = screen(normalize(tree));
        assert_eq!(out.text(0), "button \"Add\"\ntextbox \"Item\" value \"milk\"\ntext \"inside a layout\"\n");
    }

    #[test]
    fn patterns_against_a_screen() {
        let s = greeter();
        assert!(pat("button", Some(Label::Is("greet".into()))).holds(&s));
        assert!(!pat("button", Some(Label::Is("gree".into()))).holds(&s));
        assert!(pat("button", Some(Label::Containing("ree".into()))).holds(&s));
        assert!(pat("text", Some(Label::Matching(regex::Pattern::new("Your \\w+:").unwrap()))).holds(&s));
        let mut p = pat("button", None);
        assert_eq!(p.found(&s).len(), 3);
        p.count = Some(3);
        assert!(p.holds(&s));
        p.count = Some(2);
        assert!(!p.holds(&s));
        let mut v = pat("textbox", None);
        v.value = Some("Ada".into());
        assert!(v.holds(&s));
        v.value = Some("Bob".into());
        assert!(!v.holds(&s));
        let mut st = pat("button", Some(Label::Is("retry".into())));
        st.state = Some(State::Disabled);
        assert!(st.holds(&s));
        st.state = Some(State::Enabled);
        assert!(!st.holds(&s));
        let mut inside = pat("button", Some(Label::Is("OK".into())));
        inside.within = Some(("dialog".into(), Some(Label::Is("Confirm".into()))));
        assert!(inside.holds(&s));
        inside.within = Some(("dialog".into(), Some(Label::Is("Other".into()))));
        assert!(!inside.holds(&s));
        let mut g = pat("button", Some(Label::Is("greet".into())));
        g.within = Some(("dialog".into(), None));
        assert!(!g.holds(&s), "greet is not in the dialog");
        assert_eq!(inside.shown(), "button \"OK\" in dialog \"Other\"");
        let mut all = pat("button", Some(Label::Containing("r".into())));
        all.count = Some(2);
        all.value = Some("".into());
        all.state = Some(State::Enabled);
        assert_eq!(all.shown(), "exactly 2 button containing \"r\" with value \"\" enabled");
    }

    #[test]
    fn masks_record_a_node_as_its_role_alone() {
        let s = greeter();
        let mask = pat("dialog", None);
        let m = masked(&s, &[&mask]);
        assert_eq!(m.text(0), "text \"Your name:\"\ntextbox \"type here\" value \"Ada\"\nbutton \"greet\"\ndialog <masked>\n");
        assert!(m.json().ends_with("{\"role\":\"dialog\",\"masked\":true}]}"));
        // a masked node matches no pattern
        assert!(!pat("dialog", None).holds(&m));
    }

    fn changes(old: &Node, new: &Node, masks: &[&Pattern]) -> Vec<String> {
        diff(old, new, masks)
            .iter()
            .map(|c| match (&c.old, &c.new) {
                (None, Some((n, _))) => format!("+ {} {}", c.place(), n.line()),
                (Some((o, _)), None) => format!("- {} {}", c.place(), o.line()),
                (Some((o, _)), Some((n, _))) => format!("~ {} {} -> {}", c.place(), o.line(), n.line()),
                (None, None) => unreachable!(),
            })
            .collect()
    }

    #[test]
    fn the_diff_of_two_screens() {
        let old = greeter();
        assert!(changes(&old, &old, &[]).is_empty());
        // appeared, disappeared, value changed, state changed
        let mut new = greeter();
        new.children[1].value = "Bob".into();
        new.children.insert(3, n("text", "Hello, Bob!"));
        new.children[4].children[1].states.clear();
        new.children.remove(0);
        assert_eq!(
            changes(&old, &new, &[]),
            [
                "- screen text \"Your name:\"",
                "~ screen textbox \"type here\" value \"Ada\" -> textbox \"type here\" value \"Bob\"",
                "+ screen text \"Hello, Bob!\"",
                "~ screen in dialog \"Confirm\" button \"retry\" disabled -> button \"retry\"",
            ]
        );
        // siblings reordered: aligned by role and name, what moved leaves one place
        // and appears at the other
        let mut swapped = greeter();
        swapped.children.swap(0, 2);
        assert_eq!(
            changes(&old, &swapped, &[]),
            [
                "- screen text \"Your name:\"",
                "- screen textbox \"type here\" value \"Ada\"",
                "+ screen textbox \"type here\" value \"Ada\"",
                "+ screen text \"Your name:\"",
            ]
        );
        // a whole subtree that appears is a change for each of its nodes
        let mut more = greeter();
        more.children.push(with(n("list", "Todos"), vec![n("listitem", "milk")]));
        assert_eq!(changes(&old, &more, &[]), ["+ screen list \"Todos\"", "+ screen in list \"Todos\" listitem \"milk\""]);
        // a mask takes the node and what is under it out, on either side
        let mask = pat("list", None);
        assert!(changes(&old, &more, &[&mask]).is_empty());
        let recorded = masked(&more, &[&mask]);
        assert!(changes(&old, &recorded, &[]).is_empty());
    }

    #[test]
    fn claimed_by_a_pattern_before_or_after() {
        let old = greeter();
        let mut new = greeter();
        new.children[1].value = "Bob".into();
        let c = &diff(&old, &new, &[])[0];
        let mut was = pat("textbox", None);
        was.value = Some("Ada".into());
        assert!(c.claimed_by(&was));
        assert!(!c.claimed_by(&pat("button", None)));
    }

    #[test]
    fn a_large_screen_is_cut_with_the_matching_part_first() {
        let mut nodes: Vec<Node> = (0..60).map(|i| n("text", &format!("line {i}"))).collect();
        nodes.push(with(n("list", "Todos"), vec![n("listitem", "milk"), n("listitem", "eggs")]));
        let s = screen(nodes);
        let (lines, left) = cut_for(&s, &pat("listitem", Some(Label::Is("eggs".into()))));
        assert_eq!(lines.len(), 40);
        assert_eq!(left, 63 - 40);
        // the match comes with its ancestor, in tree order; the other item of its role too
        assert!(lines.contains(&"list \"Todos\"".to_string()));
        assert!(lines.contains(&"  listitem \"eggs\"".to_string()));
        assert!(lines.contains(&"  listitem \"milk\"".to_string()));
        assert_eq!(lines[0], "text \"line 0\"");
        let small = greeter();
        assert_eq!(cut_for(&small, &pat("button", None)).0.len(), 6);
    }
}

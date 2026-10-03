//! pixie, the replayed driver (DESIGN §8.3). A pixie app reads `PIXIE_SCRIPT`, a
//! comma-separated list of steps, runs it headless, and prints its trees; geas runs
//! a claim's actions on an app as one script with `a11y` after each, reads the
//! accessibility trees out of what pixie printed, and hands each `when` its tree.
//!
//! pixie prints names and values without escaping them, so a tree is read with
//! pixie's role names as anchors, trying the shortest reading of each name and value
//! first and going back when the rest does not read; a tree no reading fits is
//! E037. When the app refuses a step it exits with 101 and its trees are lost, so
//! geas runs the script's prefixes until the first that fails, which finds the
//! refused action and the screen just before it (E035).

use crate::diag;
use ritsu_base::text::Text;
use crate::gui;
use crate::model::{Call, Place, Target};
use crate::proc::{self, Env, Failure, Launch};
use crate::screen::{self, Node};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// How long an app has for each action of its script.
const PER_ACTION: Duration = Duration::from_secs(5);

/// The roles pixie prints (its `a11y.rs`), and what each is called on a screen.
const ROLES: &[(&str, &str)] = &[
    ("button", "button"),
    ("label", "text"),
    ("heading", "heading"),
    ("textInput", "textbox"),
    ("image", "img"),
    ("list", "list"),
    ("listItem", "listitem"),
    ("table", "table"),
    ("dialog", "dialog"),
    ("progress", "progressbar"),
    ("slider", "slider"),
    ("group", "group"),
    ("checkbox", "checkbox"),
    ("switch", "switch"),
    ("comboBox", "combobox"),
    ("radioGroup", "radiogroup"),
    ("tabList", "tablist"),
    ("link", "link"),
];

/// A step's text, with `\` and `,` kept from splitting the script.
fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace(',', "\\,")
}

/// The step an action is, or "" for `open()`, the first screen.
fn step(call: &Call) -> String {
    let at = |p: &Place| match p {
        Place::Field(n) if *n > 1 => format!("@{}", n - 1),
        _ => String::new(),
    };
    match call {
        Call::Open(_) => String::new(),
        Call::Click { name, nth: Some(n) } if *n > 1 => format!("click@{}:{}", n - 1, esc(name)),
        Call::Click { name, .. } => format!("click:{}", esc(name)),
        Call::Input { text, place } => format!("input{}:{}", at(place), esc(text)),
        Call::Submit(place) => format!("submit{}", at(place)),
        Call::Press(k) => format!("key:{}", esc(k)),
        Call::Advance(ms) => format!("advance:{ms}"),
        Call::Run(_) | Call::Get(_) | Call::Post { .. } => unreachable!("E006 refuses these on pixie"),
    }
}

/// The script of a claim's actions on an app: each step followed by `a11y`.
pub fn script(calls: &[&Call]) -> String {
    let mut steps = Vec::new();
    for c in calls {
        let s = step(c);
        if !s.is_empty() {
            steps.push(s);
        }
        steps.push("a11y".into());
    }
    steps.join(",")
}

// ---------- reading pixie's trees ----------

/// The most readings a position may hold while a tree is read: enough for any
/// name a person writes, and a bound on a tree written to mislead.
const READINGS: usize = 16;

struct Reader<'a> {
    c: &'a [char],
    nodes: HashMap<usize, Vec<(Node, usize)>>,
    lists: HashMap<usize, Vec<(Vec<Node>, usize)>>,
}

impl Reader<'_> {
    fn at(&self, i: usize, s: &str) -> bool {
        let mut k = i;
        for ch in s.chars() {
            if self.c.get(k) != Some(&ch) {
                return false;
            }
            k += 1;
        }
        true
    }

    /// Where a role ends a node's head: what may follow a role.
    fn role_end(&self, k: usize) -> bool {
        matches!(self.c.get(k), None | Some(' ' | '[' | ',' | ']' | '\n'))
    }

    /// Whether a node starts at `k`: one of pixie's roles.
    fn node_starts(&self, k: usize) -> bool {
        ROLES.iter().any(|(r, _)| self.at(k, r) && self.role_end(k + r.chars().count()))
    }

    /// What may follow a name: a value, `disabled`, children, the next node, or the
    /// end of the list.
    fn after_name(&self, k: usize) -> bool {
        self.at(k, " =") || self.after_value(k)
    }

    fn after_value(&self, k: usize) -> bool {
        (self.at(k, " disabled") && matches!(self.c.get(k + 9), Some('[' | ',' | ']')))
            || self.at(k, "[")
            || self.at(k, "]")
            || (self.at(k, ", ") && self.node_starts(k + 2))
    }

    /// Every reading of one node starting at `i`, shortest names and values first.
    fn node(&mut self, i: usize) -> Vec<(Node, usize)> {
        if let Some(r) = self.nodes.get(&i) {
            return r.clone();
        }
        self.nodes.insert(i, Vec::new()); // a node cannot hold itself
        let mut out = Vec::new();
        for (pixie, role) in ROLES {
            if !(self.at(i, pixie) && self.role_end(i + pixie.chars().count())) {
                continue;
            }
            let j = i + pixie.chars().count();
            // the name: none, or up to a `"` the rest can follow
            let mut names: Vec<(String, usize)> = Vec::new();
            if self.at(j, " \"") {
                for k in j + 2..self.c.len() {
                    if self.c[k] == '"' && self.after_name(k + 1) {
                        names.push((self.c[j + 2..k].iter().collect(), k + 1));
                    }
                }
            } else {
                names.push((String::new(), j));
            }
            for (name, p) in names {
                let mut values: Vec<(String, usize)> = Vec::new();
                if self.at(p, " =") {
                    for k in p + 3..=self.c.len() {
                        if self.after_value(k) {
                            values.push((self.c[p + 2..k].iter().collect(), k));
                        }
                    }
                } else {
                    values.push((String::new(), p));
                }
                for (value, q) in values {
                    let mut heads = vec![(false, q)];
                    if self.at(q, " disabled") {
                        heads.push((true, q + 9));
                    }
                    for (disabled, r) in heads {
                        let mut node = Node { role: role.to_string(), name: name.clone(), value: value.clone(), ..Node::default() };
                        if disabled {
                            node.states.push("disabled".into());
                        }
                        if self.at(r, "[") {
                            for (kids, e) in self.list(r + 1) {
                                let mut n = node.clone();
                                n.children = kids;
                                out.push((n, e + 1));
                            }
                        } else if !self.at(r, "[") {
                            out.push((node, r));
                        }
                        if out.len() >= READINGS {
                            break;
                        }
                    }
                }
            }
        }
        out.truncate(READINGS);
        self.nodes.insert(i, out.clone());
        out
    }

    /// Every reading of the nodes from `i` up to a `]`, which the end points at.
    fn list(&mut self, i: usize) -> Vec<(Vec<Node>, usize)> {
        if let Some(r) = self.lists.get(&i) {
            return r.clone();
        }
        self.lists.insert(i, Vec::new());
        let mut out = Vec::new();
        for (n, e) in self.node(i) {
            if self.at(e, "]") {
                out.push((vec![n.clone()], e));
            }
            if self.at(e, ", ") {
                for (rest, e2) in self.list(e + 2) {
                    let mut v = vec![n.clone()];
                    v.extend(rest);
                    out.push((v, e2));
                }
            }
            if out.len() >= READINGS {
                break;
            }
        }
        out.truncate(READINGS);
        self.lists.insert(i, out.clone());
        out
    }
}

/// pixie's dump of a node, as `a11y.rs` prints it, to hold a reading to the text.
fn dump(n: &Node, pixie_role: &dyn Fn(&str) -> &'static str) -> String {
    let mut out = pixie_role(&n.role).to_string();
    if !n.name.is_empty() {
        out.push_str(&format!(" \"{}\"", n.name));
    }
    if !n.value.is_empty() {
        out.push_str(&format!(" ={}", n.value));
    }
    if n.has("disabled") {
        out.push_str(" disabled");
    }
    if !n.children.is_empty() {
        let inner: Vec<String> = n.children.iter().map(|c| dump(c, pixie_role)).collect();
        out.push_str(&format!("[{}]", inner.join(", ")));
    }
    out
}

/// The accessibility trees in pixie's output: each starts a line with `group`, the
/// window, and runs to the `]` that closes it, its names and values free to hold
/// line breaks. Err holds the first line of a tree that does not read.
pub fn trees(out: &str) -> Result<Vec<Node>, String> {
    let c: Vec<char> = out.chars().collect();
    let mut found = Vec::new();
    let mut i = 0;
    while i < c.len() {
        let line_start = i == 0 || c[i - 1] == '\n';
        let starts = line_start && c[i..].starts_with(&['g', 'r', 'o', 'u', 'p']) && matches!(c.get(i + 5), None | Some('[' | '\n'));
        if !starts {
            i += 1;
            continue;
        }
        if matches!(c.get(i + 5), None | Some('\n')) {
            found.push(Vec::new());
            i += 5;
            continue;
        }
        let mut r = Reader { c: &c, nodes: HashMap::new(), lists: HashMap::new() };
        let reading = r.list(i + 6).into_iter().find(|(_, e)| matches!(c.get(e + 1), None | Some('\n')));
        match reading {
            Some((kids, e)) => {
                let back: Vec<String> = kids.iter().map(|k| dump(k, &pixie_role)).collect();
                let text: String = c[i..=e].iter().collect();
                if format!("group[{}]", back.join(", ")) != text {
                    return Err(text.lines().next().unwrap_or("").to_string());
                }
                found.push(kids);
                i = e + 1;
            }
            None => {
                let line: String = c[i..].iter().take_while(|ch| **ch != '\n').collect();
                return Err(line);
            }
        }
    }
    Ok(found.into_iter().map(to_screen).collect())
}

fn pixie_role(role: &str) -> &'static str {
    ROLES.iter().find(|(_, r)| *r == role).map(|(p, _)| *p).unwrap_or("group")
}

/// A window's nodes as a screen: a checkbox's or a switch's value `true` or
/// `false` becomes the state `checked` or `unchecked`, then §8.1's rules.
fn to_screen(nodes: Vec<Node>) -> Node {
    fn states(mut n: Node) -> Node {
        if matches!(n.role.as_str(), "checkbox" | "switch") && matches!(n.value.as_str(), "true" | "false") {
            n.states.push(if n.value == "true" { "checked".into() } else { "unchecked".into() });
            n.value.clear();
        }
        n.children = n.children.into_iter().map(states).collect();
        n
    }
    screen::screen(screen::normalize(nodes.into_iter().map(states).collect()))
}

// ---------- running a script ----------

/// How a script ended.
enum Ended {
    /// It ran; the trees it printed, one per `a11y`.
    Trees(Vec<Node>),
    /// The app refused a step: exit 101.
    Refused,
}

/// Where an app's transcript goes, and the app's command and environment.
pub struct App<'a> {
    pub target: &'a Target,
    pub words: &'a [String],
    pub dir: &'a Path,
    pub dump: PathBuf,
}

impl App<'_> {
    /// Runs one script to its end, with `PIXIE_SCRIPT` always set (an app without
    /// it opens a window) and `PIXIE_DUMP` naming the file the transcript goes to
    /// in newer kernels; older ones print it on stdout, which is read instead.
    fn run(&self, script: &str, actions: usize) -> Result<Ended, Failure> {
        let mut env = Env::of(&self.target.pins, None);
        env.set("PIXIE_SCRIPT", script);
        env.set("PIXIE_DUMP", self.dump.as_os_str());
        let _ = std::fs::remove_file(&self.dump);
        if let Some(parent) = self.dump.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let words = self.program();
        let launch = Launch { dir: self.dir, env: &env, term: false };
        let mut p = proc::start(&self.target.name, &words, &launch, false, true)?;
        let mut stdout = p.stdout.take().expect("piped");
        let reader = std::thread::spawn(move || {
            let mut v = Vec::new();
            let _ = stdout.read_to_end(&mut v);
            v
        });
        let limit = PER_ACTION * actions.max(1) as u32;
        let status = p.wait_until(Instant::now() + limit);
        let stderr = p.finish();
        let out = String::from_utf8_lossy(&reader.join().unwrap_or_default()).into_owned();
        let transcript = std::fs::read_to_string(&self.dump).unwrap_or(out);
        let _ = std::fs::remove_file(&self.dump);
        let name = &self.target.name;
        let Some(status) = status else {
            let secs = limit.as_secs();
            return Err(Failure {
                code: "E037",
                msg: tr!(
                    "pixie のアプリ `{name}` がスクリプトを {secs} 秒のうちに終えなかったので、geas が止めました",
                    "the pixie app `{name}` did not finish its script within {secs} s, so geas stopped it",
                ),
                notes: script_notes(script, &stderr),
            });
        };
        match status.code() {
            Some(0) => {}
            Some(101) => return Ok(Ended::Refused),
            _ => {
                let how = proc::how_it_ended(status);
                return Err(Failure {
                    code: "E037",
                    msg: tr!(
                        "pixie のアプリ `{name}` のスクリプトが、終了コード 0 と 101 以外で終わりました（{}）。geas がツリーを読むのは、スクリプトが最後まで動いたとき（終了コード 0）か、ステップが拒否されたとき（終了コード 101）だけです",
                        "the pixie app `{name}` ended its script with {}, and geas reads trees only from a script that ran (exit 0) or a refused step (exit 101)",
                        how.ja;
                        how.en,
                    ),
                    notes: script_notes(script, &stderr),
                });
            }
        }
        match trees(&transcript) {
            Ok(found) if found.len() == actions => Ok(Ended::Trees(found)),
            Ok(found) => Err(Failure {
                code: "E037",
                msg: tr!(
                    "pixie の出力にあるアクセシビリティツリーは {} 個で、スクリプトが求めたのは {actions} 個です",
                    "pixie's output holds {} accessibility trees, and the script asked for {actions}",
                    found.len(),
                ),
                notes: script_notes(script, &stderr),
            }),
            Err(line) => {
                let l = diag::cut(&line, 100);
                let mut notes = vec![tr!(
                    "pixie は名前と値をエスケープせずに出力するので、geas はロールの名前を手がかりにツリーを読みます。`\", ` や `]` を含む名前があると、どう読んでもツリーにならないことがあります",
                    "geas reads pixie's tree with its role names as anchors, since pixie prints names and values without escaping them; a name holding `\", ` or `]` can make a tree read no way at all",
                )];
                notes.extend(script_notes(script, &stderr));
                Err(Failure {
                    code: "E037",
                    msg: tr!(
                        "pixie が出力したアクセシビリティツリーが読めません: `{l}`",
                        "an accessibility tree pixie printed does not read: `{l}`",
                    ),
                    notes,
                })
            }
        }
    }

    /// The command, its first word read as a path from the spec's directory when a
    /// file of that name is there: a pixie app is something built, not something
    /// installed on PATH.
    fn program(&self) -> Vec<String> {
        let mut words = self.words.to_vec();
        if !words[0].contains('/') && self.dir.join(&words[0]).is_file() {
            words[0] = format!("./{}", words[0]);
        }
        words
    }

    /// Runs a claim's actions on the app: a screen for each, up to the first that
    /// fails. When the app refuses one, the script's prefixes find which, and the
    /// screen before it.
    pub fn replay(&self, calls: &[&Call]) -> Vec<Result<Node, Failure>> {
        match self.run(&script(calls), calls.len()) {
            Ok(Ended::Trees(found)) => found.into_iter().map(Ok).collect(),
            Err(f) => vec![Err(f)],
            Ok(Ended::Refused) => {
                let mut before: Vec<Node> = Vec::new();
                for k in 1..=calls.len() {
                    match self.run(&script(&calls[..k]), k) {
                        Ok(Ended::Trees(found)) => before = found,
                        Err(f) => {
                            let mut out: Vec<Result<Node, Failure>> = before.into_iter().take(k - 1).map(Ok).collect();
                            out.push(Err(f));
                            return out;
                        }
                        Ok(Ended::Refused) => {
                            // the screen just before the refused action
                            let screen = match before.last() {
                                Some(s) => Ok(s.clone()),
                                None => match self.run("a11y", 1) {
                                    Ok(Ended::Trees(mut s)) if s.len() == 1 => Ok(s.remove(0)),
                                    Ok(_) => Err(()),
                                    Err(_) => Err(()),
                                },
                            };
                            let mut out: Vec<Result<Node, Failure>> = before.into_iter().map(Ok).collect();
                            out.push(Err(self.refused(calls[k - 1], screen.ok().as_ref())));
                            return out;
                        }
                    }
                }
                // every prefix ran, so the whole script did not fail at an action
                let name = &self.target.name;
                vec![Err(Failure {
                    code: "E037",
                    msg: tr!(
                        "pixie のアプリ `{name}` は、一度はスクリプトを拒否し（終了コード 101）、次は最後まで動かしました。走らせるたびに振る舞いが変わります",
                        "the pixie app `{name}` refused its script once (exit 101) and ran it to the end the next time: it does not do the same on every run",
                    ),
                    notes: vec![],
                })]
            }
        }
    }

    /// E035 for a refused action, with the screen before it.
    fn refused(&self, call: &Call, screen: Option<&Node>) -> Failure {
        let name = &self.target.name;
        let shown = call.display();
        let s = step(call);
        let s = if s.is_empty() { "a11y".to_string() } else { s };
        Failure {
            code: "E035",
            msg: tr!(
                "`{name}` が `{shown}` を拒否しました。pixie はスクリプトを `{s}` で止め、終了コード 101 で終了しました",
                "`{name}` refused `{shown}`: pixie stopped the script at `{s}` and exited with 101",
            ),
            notes: gui::refused_notes(screen, call),
        }
    }
}

/// The script and the last lines of stderr, as notes.
fn script_notes(script: &str, stderr: &str) -> Vec<Text> {
    let s = diag::cut(script, 100);
    let mut notes = vec![tr!("スクリプト: {s}", "the script: {s}")];
    notes.extend(proc::stderr_tail(stderr));
    notes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_claims_actions_as_one_script() {
        let calls = [
            Call::Open(None),
            Call::Input { text: "a, b\\c".into(), place: Place::First },
            Call::Input { text: "x".into(), place: Place::Field(2) },
            Call::Click { name: "greet".into(), nth: None },
            Call::Click { name: "ok".into(), nth: Some(3) },
            Call::Submit(Place::Field(1)),
            Call::Press("cmd-s".into()),
            Call::Advance(500),
        ];
        let refs: Vec<&Call> = calls.iter().collect();
        assert_eq!(
            script(&refs),
            "a11y,input:a\\, b\\\\c,a11y,input@1:x,a11y,click:greet,a11y,click@2:ok,a11y,submit,a11y,key:cmd-s,a11y,advance:500,a11y"
        );
    }

    const START: &str = "Column[Text(Your name:), Row[TextField(), Button(greet)]]\n";

    #[test]
    fn trees_out_of_a_transcript() {
        let out = format!(
            "{START}group[label \"Your name:\", textInput \"type here\" =Ada, button \"greet\", label, checkbox \"agree\" =true, button \"go\" disabled]\ngroup\n{START}"
        );
        let found = trees(&out).unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(
            found[0].text(0),
            "text \"Your name:\"\ntextbox \"type here\" value \"Ada\"\nbutton \"greet\"\ncheckbox \"agree\" checked\nbutton \"go\" disabled\n"
        );
        assert_eq!(found[1].count(), 0);
    }

    #[test]
    fn names_and_values_pixie_did_not_escape() {
        // a value holding `, ` and the next node's look, as measured: `=a, b`
        let out = "group[textInput \"type here\" =a, b, button \"greet\", label \"name: a, b\"]\n";
        let s = trees(out).unwrap().remove(0);
        assert_eq!(s.text(0), "textbox \"type here\" value \"a, b\"\nbutton \"greet\"\ntext \"name: a, b\"\n");
        // a name holding a quote, a bracket and a line break
        let out = "group[label \"say \"hi\"], ok\nthere\", button \"x\"]\n";
        let s = trees(out).unwrap().remove(0);
        assert_eq!(s.children[0].name, "say \"hi\"], ok\nthere");
        assert_eq!(s.children[1].line(), "button \"x\"");
        // nested children
        let out = "group[dialog \"Confirm\"[button \"OK\", button \"Cancel\"], label \"after\"]\n";
        assert_eq!(trees(out).unwrap()[0].text(0), "dialog \"Confirm\"\n  button \"OK\"\n  button \"Cancel\"\ntext \"after\"\n");
    }

    #[test]
    fn a_tree_that_reads_no_way() {
        assert_eq!(trees("group[widget \"x\"]\n").unwrap_err(), "group[widget \"x\"]");
        assert!(trees("group[button \"x\"\n").is_err());
    }
}

//! A target's `contract` (DESIGN §20): the OpenAPI document of what the service answers. The spec
//! is held to it before anything runs (a file that cannot be read is E015, a call the document does
//! not have is E016, what it says and geas does not read is W015), and each `get` and `post` of a
//! run is held to it as one more check of what its `when` observed: the status, the headers the
//! response must send, the media type, and the body against the schema.

use crate::check::CheckResult;
use crate::diag::{self, Diag, Severity};
use crate::json::{self, J};
use crate::model::{Call, Contract, Pos, Spec, Step};
use crate::regex::Pattern;
use crate::run::Obs;
use ritsu_base::modules::refer;
use ritsu_base::openapi::{self, Responding, Responses, Shape};
use ritsu_base::text::Text;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// How many schemas one value is held to, one inside another, before a schema that holds itself is
/// taken for a loop.
const MAX_DEPTH: usize = 256;
/// How many problems of one answer a failed check says.
const SHOWN: usize = 3;
/// How many paths of a document a diagnostic lists.
const LISTED: usize = 12;

/// A contract, read.
#[derive(Debug)]
pub struct Held {
    responses: Responses,
    /// Each shape's `enum`, as values, and its `pattern`, as a pattern when geas can run it.
    values: Vec<Option<Vec<J>>>,
    patterns: Vec<Option<Pattern>>,
}

/// Reads the contract of every target that has one, and holds the spec's calls to them. What it
/// says: errors, when the spec does not run, and warnings.
pub fn read(spec: &mut Spec, dir: &Path) -> Vec<Diag> {
    let mut out = Vec::new();
    for target in &mut spec.targets {
        let Some(c) = &mut target.contract else { continue };
        match load(dir, &c.file, c.pos) {
            Ok((held, warnings)) => {
                c.held = Some(Arc::new(held));
                out.extend(warnings);
            }
            Err(d) => out.push(d),
        }
    }
    for claim in &spec.claims {
        for step in &claim.steps {
            let Step::When { target, call, pos, .. } = step else { continue };
            let Some(c) = spec.target(target).and_then(|t| t.contract.as_ref()) else { continue };
            let (Some(held), Some((method, path))) = (&c.held, request(call)) else { continue };
            if let Some(d) = outside(c, held, method, path, *pos) {
                out.push(d);
            }
        }
    }
    diag::sort(&mut out);
    out
}

/// The files a spec's contracts are read from, in `dir` (the spec's directory), whether or not they
/// can be read: what `geas affected` looks for in a diff, since a change to one of them changes what
/// the claims are held to. A spec that does not parse has none.
pub fn files(src: &str, dir: &Path) -> Vec<PathBuf> {
    let Ok(spec) = crate::parse::parse(src) else { return vec![] };
    let mut out = Vec::new();
    for c in spec.targets.iter().filter_map(|t| t.contract.as_ref()) {
        let Ok(path) = refer::refer(dir, &c.file) else { continue };
        let asked = RefCell::new(vec![path.clone()]);
        if let Ok(text) = ritsu_base::fs::read_to_string(&path) {
            let read_ref = |p: &str| {
                asked.borrow_mut().push(PathBuf::from(p));
                ritsu_base::fs::read_to_string(p).ok()
            };
            // only what it asks for is wanted: a document that cannot be read is said by `check`
            let _ = openapi::responses(&path.to_string_lossy(), &text, &read_ref);
        }
        out.extend(asked.into_inner());
    }
    out
}

/// The method and the path of a call a service answers with HTTP.
fn request(call: &Call) -> Option<(&'static str, &str)> {
    match call {
        Call::Get(path) => Some(("GET", path)),
        Call::Post { path, .. } => Some(("POST", path)),
        _ => None,
    }
}

fn e015(at: Pos, msg: Text) -> Diag {
    diag::error("E015", at.line, at.col, msg)
}

fn load(dir: &Path, file: &str, at: Pos) -> Result<(Held, Vec<Diag>), Diag> {
    let path = refer::refer(dir, file).map_err(|r| e015(at, r.text()))?;
    let on_disk = path.to_string_lossy().to_string();
    let text = ritsu_base::fs::read_to_string(&path).map_err(|e| e015(at, tr!("契約のファイル {file} を読めません: {e}", "cannot read the contract {file}: {e}")))?;
    let read_ref = |p: &str| ritsu_base::fs::read_to_string(p).ok();
    let responses = openapi::responses(&on_disk, &text, &read_ref).map_err(|p| {
        e015(
            at,
            tr!(
                "契約のファイル {file} を読めません（{}:{}）: {}",
                "cannot read the contract {file} ({}:{}): {}",
                p.line, p.col, p.message.ja; p.line, p.col, p.message.en
            ),
        )
    })?;
    if let Some(u) = responses.unresolved.first() {
        let (line, col, written) = (u.line, u.col, &u.written);
        return Err(e015(at, tr!("契約 {file} の `$ref` を辿れません（{line}:{col}）: {written}", "a `$ref` of the contract {file} cannot be followed ({line}:{col}): {written}")));
    }
    let values = responses.shapes.iter().map(|s| s.values.as_ref().map(|vs| vs.iter().filter_map(|v| json::parse(v).ok()).collect())).collect();
    let patterns: Vec<Option<Pattern>> = responses.shapes.iter().map(|s| s.pattern.as_deref().and_then(pattern)).collect();
    let mut unheld: Vec<String> = Vec::new();
    for u in &responses.unheld {
        let word = format!("`{}`", u.keyword);
        if !unheld.contains(&word) {
            unheld.push(word);
        }
    }
    for (s, compiled) in responses.shapes.iter().zip(&patterns) {
        if let (Some(p), None) = (&s.pattern, compiled) {
            let word = format!("`pattern` {}", json::quote(p));
            if !unheld.contains(&word) {
                unheld.push(word);
            }
        }
    }
    let mut warnings = Vec::new();
    if !unheld.is_empty() {
        let (list_en, list_ja) = (unheld.join(", "), unheld.join("、"));
        let mut d = diag::error(
            "W015",
            at.line,
            at.col,
            tr!(
                "契約 {file} の {list_ja} は、geas が答えと突き合わせません",
                "geas does not hold an answer to {list_en} of the contract {file}",
                ;
            ),
        );
        d.severity = Severity::Warning;
        warnings.push(d);
    }
    Ok((Held { responses, values, patterns }, warnings))
}

/// A `pattern` of a schema, which finds its match anywhere in the value and may say `^` and `$`, as
/// a pattern of geas, which always matches the whole value. None when it cannot say the same: an
/// anchor anywhere but the two ends, one end anchored beside an `|` at the top, or what geas's
/// patterns do not have.
fn pattern(source: &str) -> Option<Pattern> {
    let body = source.strip_prefix('^');
    let starts = body.is_some();
    let body = body.unwrap_or(source);
    let ends = body.ends_with('$') && !body.ends_with("\\$");
    let body = if ends { &body[..body.len() - 1] } else { body };
    let (mut escaped, mut class, mut depth, mut top_or) = (false, false, 0usize, false);
    for c in body.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '\\' => escaped = true,
            '[' if !class => class = true,
            ']' if class => class = false,
            '(' if !class => depth += 1,
            ')' if !class => depth = depth.saturating_sub(1),
            '|' if !class && depth == 0 => top_or = true,
            '^' | '$' if !class => return None,
            _ => {}
        }
    }
    if top_or && (starts || ends) {
        return None;
    }
    let any = r"[\s\S]*";
    let whole = format!("{}({body}){}", if starts { "" } else { any }, if ends { "" } else { any });
    Pattern::new(&whole).ok()
}

/// How the path of a call fits a path of the document: how many of its segments are written out,
/// or None. A segment of the document may hold `{name}`, which stands for one or more characters
/// of a segment.
fn fits(template: &str, path: &str) -> Option<usize> {
    let (ts, ps): (Vec<&str>, Vec<&str>) = (template.split('/').collect(), path.split('/').collect());
    if ts.len() != ps.len() || !ts.iter().zip(&ps).all(|(t, p)| segment(t, p)) {
        return None;
    }
    Some(ts.iter().filter(|t| !t.contains('{')).count())
}

fn segment(template: &str, s: &str) -> bool {
    let Some(open) = template.find('{') else {
        return template == s || unpercent(s) == template;
    };
    let Some(close) = template[open..].find('}') else {
        return template == s;
    };
    let (written, rest) = (&template[..open], &template[open + close + 1..]);
    let Some(after) = s.strip_prefix(written) else { return false };
    after.char_indices().map(|(i, _)| i).skip(1).chain([after.len()]).any(|i| i > 0 && segment(rest, &after[i..]))
}

/// A path's `%XX` read back.
fn unpercent(s: &str) -> String {
    let b = s.as_bytes();
    let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2]))
        {
            out.push(h * 16 + l);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

/// How the path of a call fits an operation, with or without what its `servers` put in front: a
/// service under test may or may not be reached under them.
fn fits_operation(op: &Responding, path: &str) -> Option<usize> {
    let under = op.bases.iter().filter_map(|b| fits(&format!("{b}{}", op.path), path));
    fits(&op.path, path).into_iter().chain(under).max()
}

/// The operation a call is, and the operations on its path whatever their method: the one written
/// out furthest, the first written when two are alike.
fn find<'a>(r: &'a Responses, method: &str, path: &str) -> (Option<&'a Responding>, Vec<&'a Responding>) {
    let path = path.split(['?', '#']).next().unwrap_or(path);
    let (mut best, mut same_path): (Option<(usize, &Responding)>, Vec<&Responding>) = (None, Vec::new());
    for op in &r.operations {
        let Some(score) = fits_operation(op, path) else { continue };
        same_path.push(op);
        if op.method == method && best.is_none_or(|(b, _)| score > b) {
            best = Some((score, op));
        }
    }
    (best.map(|(_, op)| op), same_path)
}

/// E016: a call the document does not have.
fn outside(c: &Contract, held: &Held, method: &str, path: &str, at: Pos) -> Option<Diag> {
    let (found, same_path) = find(&held.responses, method, path);
    if found.is_some() {
        return None;
    }
    let file = &c.file;
    let shown = path.split(['?', '#']).next().unwrap_or(path);
    let mut d = diag::error("E016", at.line, at.col, tr!("契約 {file} に {method} {shown} はありません", "the contract {file} has no {method} {shown}"));
    if let Some(first) = same_path.first() {
        let methods: Vec<&str> = same_path.iter().map(|op| op.method.as_str()).collect();
        let (en, ja, template) = (methods.join(", "), methods.join("、"), &first.path);
        d = d.note(tr!("{template} にあるのは {ja} です", "{template} has {en}"));
    } else {
        let mut paths: Vec<&str> = Vec::new();
        for op in &held.responses.operations {
            if !paths.contains(&op.path.as_str()) {
                paths.push(&op.path);
            }
        }
        if !paths.is_empty() {
            let more = paths.len().saturating_sub(LISTED);
            let list = paths.iter().take(LISTED).copied().collect::<Vec<_>>();
            let (en, ja) = (list.join(", "), list.join("、"));
            d = d.note(if more > 0 { tr!("契約のパス: {ja} ほか {more} 個", "the contract's paths: {en} and {more} more") } else { tr!("契約のパス: {ja}", "the contract's paths: {en}") });
        }
    }
    Some(d.note(tr!(
        "契約に足すか、`contract` の行が無いターゲットから呼んでください",
        "add it to the contract, or call it through a target with no `contract` line",
    )))
}

/// A `when`'s answer held to the target's contract: one more check of what it observed, on the
/// `when`'s line. None when the target has no contract, or the call is not one HTTP answers.
pub fn held_to(contract: Option<&Contract>, call: &Call, line: usize, obs: &Obs) -> Option<CheckResult> {
    let held = contract?.held.as_ref()?;
    let (method, path) = request(call)?;
    let Obs::Http { status, headers, body } = obs else { return None };
    let (Some(op), _) = find(&held.responses, method, path) else { return None };
    let problems = held.problems(op, *status, headers, body);
    let ok = problems.is_empty();
    Some(CheckResult { line, label: "contract".to_string(), expected: format!("{method} {}", op.path), actual: if ok { Text::same(status.to_string()) } else { together(&problems) }, ok, screen: None })
}

/// The problems of an answer as one line: the first few, and how many more.
fn together(problems: &[Text]) -> Text {
    let first = &problems[..problems.len().min(SHOWN)];
    let mut en = first.iter().map(|t| t.en.as_str()).collect::<Vec<_>>().join("; ");
    let mut ja = first.iter().map(|t| t.ja.as_str()).collect::<Vec<_>>().join("。");
    let more = problems.len() - first.len();
    if more > 0 {
        en.push_str(&format!("; and {more} more"));
        ja.push_str(&format!("。ほかに {more} 件あります"));
    }
    Text::new(ja, en)
}

/// A media type without what follows it (`; charset=utf-8`), in small letters.
fn media_of(s: &str) -> String {
    s.split(';').next().unwrap_or(s).trim().to_ascii_lowercase()
}

fn is_json(media: &str) -> bool {
    media.ends_with("/json") || media.ends_with("+json")
}

impl Held {
    fn problems(&self, op: &Responding, status: u16, headers: &[(String, String)], body: &str) -> Vec<Text> {
        let code = status.to_string();
        let range = format!("{}XX", status / 100);
        let of = |word: &str| op.responses.iter().find(|r| r.status.eq_ignore_ascii_case(word));
        let Some(r) = of(&code).or_else(|| of(&range)).or_else(|| of("default")) else {
            let listed: Vec<&str> = op.responses.iter().map(|r| r.status.as_str()).collect();
            let (en, ja) = (listed.join(", "), listed.join("、"));
            return vec![tr!("{code} で、契約にない状態です（あるのは {ja}）", "status {code}, which the contract does not list (it lists {en})")];
        };
        let mut out = Vec::new();
        for h in &r.required_headers {
            if !headers.iter().any(|(k, _)| k.eq_ignore_ascii_case(h)) {
                out.push(tr!("{code} の必須のヘッダー {h} がありません", "status {code} without the required header {h}"));
            }
        }
        if r.media.is_empty() {
            return out;
        }
        let listed: Vec<&str> = r.media.iter().map(|(m, _)| m.as_str()).collect();
        let (en, ja) = (listed.join(", "), listed.join("、"));
        let Some(media) = headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("content-type")).map(|(_, v)| media_of(v)) else {
            out.push(tr!("{code} に Content-Type がありません（契約にあるのは {ja}）", "status {code} with no Content-Type (the contract lists {en})"));
            return out;
        };
        let wanted = |m: &str| media_of(m) == media;
        let family = format!("{}/*", media.split('/').next().unwrap_or(""));
        let found = r.media.iter().find(|(m, _)| wanted(m)).or_else(|| r.media.iter().find(|(m, _)| media_of(m) == family)).or_else(|| r.media.iter().find(|(m, _)| m == "*/*"));
        let Some((_, schema)) = found else {
            out.push(tr!("{code} の Content-Type が {media} で、契約にありません（あるのは {ja}）", "status {code} as {media}, which the contract does not list (it lists {en})"));
            return out;
        };
        if let (Some(schema), true) = (schema, is_json(&media)) {
            match json::parse(body) {
                Ok(v) => self.fit(*schema, &v, "$", 0, &mut out),
                Err(_) => out.push(tr!("{code} の本体が JSON ではありません", "status {code} with a body that is not JSON")),
            }
        }
        out
    }

    fn shape(&self, i: usize) -> &Shape {
        &self.responses.shapes[i]
    }

    /// Whether the schema is, or through its `allOf` and `$ref`s has, `writeOnly`.
    fn write_only(&self, i: usize, depth: usize) -> bool {
        let s = self.shape(i);
        depth < MAX_DEPTH && (s.write_only || s.all_of.iter().any(|&a| self.write_only(a, depth + 1)))
    }

    fn fits(&self, i: usize, v: &J, at: &str, depth: usize) -> bool {
        let mut out = Vec::new();
        self.fit(i, v, at, depth, &mut out);
        out.is_empty()
    }

    /// What does not fit in `v`, the value at `at`, against the schema `i`.
    fn fit(&self, i: usize, v: &J, at: &str, depth: usize, out: &mut Vec<Text>) {
        if depth > MAX_DEPTH {
            return;
        }
        let s = self.shape(i);
        let shown = show(v);
        if s.never {
            out.push(tr!("{at}: 契約はここに値を認めていません", "{at}: the contract allows no value here"));
            return;
        }
        // 3.0 writes `nullable` beside an `allOf` or a `$ref` as well as beside a `type`
        if matches!(v, J::Null) && s.nullable {
            return;
        }
        if !s.types.is_empty() && !s.types.iter().any(|t| is_type(t, v)) {
            let (kind, en, ja) = (kind_of(v), s.types.join(" or "), s.types.join(" か "));
            out.push(tr!("{at}: {shown} は {kind} で、契約は {ja} です", "{at}: {shown} is {kind}; the contract says {en}"));
            return;
        }
        if let Some(values) = &self.values[i]
            && !values.iter().any(|x| same(x, v))
        {
            let list: Vec<String> = values.iter().map(json::render).collect();
            let (en, ja) = (list.join(", "), list.join("、"));
            out.push(tr!("{at}: {shown} は {ja} のどれでもありません", "{at}: {shown} is not one of {en}"));
        }
        match v {
            J::Num(n) => self.fit_number(s, *n, &shown, at, out),
            J::Str(t) => self.fit_text(i, s, t, &shown, at, out),
            J::Arr(xs) => self.fit_list(s, xs, at, depth, out),
            J::Obj(members) => self.fit_object(s, members, at, depth, out),
            _ => {}
        }
        for &a in &s.all_of {
            self.fit(a, v, at, depth + 1, out);
        }
        if !s.any_of.is_empty() && !s.any_of.iter().any(|&a| self.fits(a, v, at, depth + 1)) {
            out.push(tr!("{at}: {shown} は anyOf のどれにも合いません", "{at}: {shown} fits none of its anyOf"));
        }
        if !s.one_of.is_empty() {
            let n = s.one_of.iter().filter(|&&a| self.fits(a, v, at, depth + 1)).count();
            if n != 1 {
                out.push(tr!("{at}: {shown} は oneOf のうち {n} 個に合います。合うのは一つのはずです", "{at}: {shown} fits {n} of its oneOf, not exactly one"));
            }
        }
        if let Some(not) = s.not
            && self.fits(not, v, at, depth + 1)
        {
            out.push(tr!("{at}: {shown} は not が認めない値です", "{at}: {shown} fits what its not forbids"));
        }
    }

    fn fit_number(&self, s: &Shape, n: f64, shown: &str, at: &str, out: &mut Vec<Text>) {
        if let Some(b) = &s.minimum {
            let m = value_of(&b.value);
            if b.exclusive && n <= m {
                out.push(tr!("{at}: {shown} は {m} より大きくありません", "{at}: {shown} is not above {m}"));
            } else if n < m {
                out.push(tr!("{at}: {shown} は下限の {m} より小さいです", "{at}: {shown} is below the lowest value, {m}"));
            }
        }
        if let Some(b) = &s.maximum {
            let m = value_of(&b.value);
            if b.exclusive && n >= m {
                out.push(tr!("{at}: {shown} は {m} より小さくありません", "{at}: {shown} is not below {m}"));
            } else if n > m {
                out.push(tr!("{at}: {shown} は上限の {m} より大きいです", "{at}: {shown} is above the highest value, {m}"));
            }
        }
        if let Some(m) = &s.multiple_of {
            let m = value_of(m);
            let q = n / m;
            if m > 0.0 && (q - q.round()).abs() > 1e-9 {
                out.push(tr!("{at}: {shown} は {m} の倍数ではありません", "{at}: {shown} is not a multiple of {m}"));
            }
        }
        let range = match s.format.as_deref() {
            Some("int32") => Some((i32::MIN as f64, i32::MAX as f64)),
            Some("int64") => Some((i64::MIN as f64, i64::MAX as f64)),
            _ => None,
        };
        if let (Some((low, high)), Some(format)) = (range, &s.format)
            && (n.fract() != 0.0 || n < low || n > high)
        {
            out.push(tr!("{at}: {shown} は {format} ではありません", "{at}: {shown} is not {format}"));
        }
    }

    fn fit_text(&self, i: usize, s: &Shape, t: &str, shown: &str, at: &str, out: &mut Vec<Text>) {
        let chars = t.chars().count() as u64;
        let many = diag::count(chars as usize, "character", "characters");
        if let Some(m) = s.min_length
            && chars < m
        {
            out.push(tr!("{at}: {shown} は {chars} 文字で、minLength の {m} より短いです", "{at}: {shown} has {many}, fewer than its minLength, {m}"));
        }
        if let Some(m) = s.max_length
            && chars > m
        {
            out.push(tr!("{at}: {shown} は {chars} 文字で、maxLength の {m} より長いです", "{at}: {shown} has {many}, more than its maxLength, {m}"));
        }
        if let (Some(p), Some(source)) = (&self.patterns[i], &s.pattern)
            && !p.matches(t)
        {
            out.push(tr!("{at}: {shown} はパターン {source} に合いません", "{at}: {shown} does not match the pattern {source}"));
        }
        if let Some(format) = s.format.as_deref()
            && !format_holds(format, t)
        {
            out.push(tr!("{at}: {shown} は {format} ではありません", "{at}: {shown} is not {format}"));
        }
    }

    fn fit_list(&self, s: &Shape, xs: &[J], at: &str, depth: usize, out: &mut Vec<Text>) {
        let n = xs.len() as u64;
        let many = diag::count(xs.len(), "item", "items");
        if let Some(m) = s.min_items
            && n < m
        {
            out.push(tr!("{at}: 要素が {n} 個で、minItems の {m} より少ないです", "{at}: {many}, fewer than its minItems, {m}"));
        }
        if let Some(m) = s.max_items
            && n > m
        {
            out.push(tr!("{at}: 要素が {n} 個で、maxItems の {m} より多いです", "{at}: {many}, more than its maxItems, {m}"));
        }
        if s.unique_items {
            let mut seen: Vec<String> = Vec::new();
            for x in xs {
                let text = json::render(x);
                if seen.contains(&text) {
                    let shown = show(x);
                    out.push(tr!("{at}: {shown} が繰り返されています（uniqueItems）", "{at}: {shown} comes twice, and its uniqueItems forbids it"));
                    break;
                }
                seen.push(text);
            }
        }
        for (k, x) in xs.iter().enumerate() {
            let shape = s.prefix_items.get(k).copied().or(s.items);
            if let Some(shape) = shape {
                self.fit(shape, x, &format!("{at}[{k}]"), depth + 1, out);
            }
        }
    }

    fn fit_object(&self, s: &Shape, members: &[(String, J)], at: &str, depth: usize, out: &mut Vec<Text>) {
        let n = members.len() as u64;
        let many = diag::count(members.len(), "member", "members");
        if let Some(m) = s.min_properties
            && n < m
        {
            out.push(tr!("{at}: メンバーが {n} 個で、minProperties の {m} より少ないです", "{at}: {many}, fewer than its minProperties, {m}"));
        }
        if let Some(m) = s.max_properties
            && n > m
        {
            out.push(tr!("{at}: メンバーが {n} 個で、maxProperties の {m} より多いです", "{at}: {many}, more than its maxProperties, {m}"));
        }
        for name in &s.required {
            let write_only = s.properties.iter().any(|(p, shape)| p == name && self.write_only(*shape, 0));
            if !write_only && !members.iter().any(|(k, _)| k == name) {
                out.push(tr!("{at}: 必須の {name} がありません", "{at}: the required {name} is missing"));
            }
        }
        for (k, x) in members {
            let child = format!("{at}.{k}");
            match s.properties.iter().find(|(p, _)| p == k) {
                Some((_, shape)) if self.write_only(*shape, 0) => {
                    out.push(tr!("{child}: 契約では、サービスへ送るだけのメンバーです（writeOnly）", "{child}: the contract says it is only ever sent to the service (writeOnly)"));
                }
                Some((_, shape)) => self.fit(*shape, x, &child, depth + 1, out),
                None => {
                    if let Some(a) = s.additional {
                        if self.shape(a).never {
                            out.push(tr!("{child}: 契約にないメンバーです", "{child}: the contract has no such member"));
                        } else {
                            self.fit(a, x, &child, depth + 1, out);
                        }
                    }
                }
            }
        }
    }
}

/// A value as a failed check shows it, cut.
fn show(v: &J) -> String {
    let text = json::render(v);
    if text.chars().count() <= 40 {
        return text;
    }
    let cut: String = text.chars().take(40).collect();
    format!("{cut}…")
}

fn kind_of(v: &J) -> &'static str {
    match v {
        J::Null => "null",
        J::Bool(_) => "boolean",
        J::Num(_) => "number",
        J::Str(_) => "string",
        J::Arr(_) => "array",
        J::Obj(_) => "object",
    }
}

fn is_type(word: &str, v: &J) -> bool {
    match word {
        "integer" => matches!(v, J::Num(n) if n.fract() == 0.0),
        other => other == kind_of(v),
    }
}

fn value_of(n: &openapi::Number) -> f64 {
    match n {
        openapi::Number::Int(i) => *i as f64,
        openapi::Number::Decimal(s) => s.parse().unwrap_or(f64::NAN),
    }
}

/// Whether two values are the same value, whatever the order of an object's members.
fn same(a: &J, b: &J) -> bool {
    match (a, b) {
        (J::Null, J::Null) => true,
        (J::Bool(x), J::Bool(y)) => x == y,
        (J::Num(x), J::Num(y)) => x == y,
        (J::Str(x), J::Str(y)) => x == y,
        (J::Arr(x), J::Arr(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same(p, q)),
        (J::Obj(x), J::Obj(y)) => x.len() == y.len() && x.iter().all(|(k, p)| y.iter().any(|(l, q)| k == l && same(p, q))),
        _ => false,
    }
}

/// Whether a string is what a format says. The formats of a string a document can only mean one way
/// are read (`date`, `date-time`, `uuid`); any other is the document's own.
fn format_holds(format: &str, t: &str) -> bool {
    match format {
        "date" => is_date(t),
        "date-time" => is_date_time(t),
        "uuid" => is_uuid(t),
        _ => true,
    }
}

fn digits(t: &str) -> Option<u32> {
    if t.is_empty() || !t.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    t.parse().ok()
}

/// `2026-11-07`, a day of that month.
fn is_date(t: &str) -> bool {
    if t.len() != 10 || !t.is_ascii() || t.as_bytes()[4] != b'-' || t.as_bytes()[7] != b'-' {
        return false;
    }
    let (Some(y), Some(m), Some(d)) = (digits(&t[..4]), digits(&t[5..7]), digits(&t[8..10])) else { return false };
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let last = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=last).contains(&d)
}

/// RFC 3339: `2026-11-07T10:00:00Z`, `2026-11-07T10:00:00.5+09:00`.
fn is_date_time(t: &str) -> bool {
    if !t.is_ascii() || t.len() < 20 || !is_date(&t[..10]) || !matches!(t.as_bytes()[10], b'T' | b't') {
        return false;
    }
    let rest = &t[11..];
    let hms = |s: &str| -> bool {
        s.len() == 8 && s.as_bytes()[2] == b':' && s.as_bytes()[5] == b':' && matches!((digits(&s[..2]), digits(&s[3..5]), digits(&s[6..8])), (Some(h), Some(m), Some(c)) if h <= 23 && m <= 59 && c <= 60)
    };
    if rest.len() < 9 || !hms(&rest[..8]) {
        return false;
    }
    let mut after = &rest[8..];
    if let Some(fraction) = after.strip_prefix('.') {
        let n = fraction.bytes().take_while(u8::is_ascii_digit).count();
        if n == 0 {
            return false;
        }
        after = &fraction[n..];
    }
    match after.as_bytes().first() {
        Some(b'Z' | b'z') => after.len() == 1,
        Some(b'+' | b'-') => after.len() == 6 && after.as_bytes()[3] == b':' && matches!((digits(&after[1..3]), digits(&after[4..6])), (Some(h), Some(m)) if h <= 23 && m <= 59),
        _ => false,
    }
}

fn is_uuid(t: &str) -> bool {
    let parts: Vec<&str> = t.split('-').collect();
    parts.iter().map(|p| p.len()).eq([8, 4, 4, 4, 12]) && parts.iter().all(|p| p.bytes().all(|b| b.is_ascii_hexdigit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn held(doc: &str) -> Held {
        let responses = openapi::responses("c.json", doc, &|_| None).expect("a document");
        let values = responses.shapes.iter().map(|s| s.values.as_ref().map(|vs| vs.iter().filter_map(|v| json::parse(v).ok()).collect())).collect();
        let patterns = responses.shapes.iter().map(|s| s.pattern.as_deref().and_then(pattern)).collect();
        Held { responses, values, patterns }
    }

    /// A contract of one operation whose 200 is JSON of this schema.
    fn with_schema(schema: &str) -> Held {
        held(&format!(r#"{{"openapi":"3.1.0","paths":{{"/x":{{"get":{{"responses":{{"200":{{"description":"","content":{{"application/json":{{"schema":{schema}}}}}}}}}}}}}}}}}"#))
    }

    /// What is wrong with a body under a schema, in English.
    fn wrong(schema: &str, body: &str) -> Vec<String> {
        let h = with_schema(schema);
        let headers = [("content-type".to_string(), "application/json; charset=utf-8".to_string())];
        h.problems(&h.responses.operations[0], 200, &headers, body).into_iter().map(|t| t.en).collect()
    }

    #[test]
    fn a_path_of_the_document_fits_the_path_of_a_call() {
        assert_eq!(fits("/a/{id}", "/a/1"), Some(2));
        assert_eq!(fits("/a/b", "/a/b"), Some(3));
        assert_eq!(fits("/a/{id}", "/a/"), None);
        assert_eq!(fits("/a/{id}", "/a/1/2"), None);
        assert_eq!(fits("/a/{id}.json", "/a/1.json"), Some(2));
        assert_eq!(fits("/a/{id}.json", "/a/.json"), None);
        assert_eq!(fits("/a/{x}-{y}", "/a/1-2-3"), Some(2));
        assert_eq!(fits("/a%20b", "/a%20b"), Some(2));
        assert_eq!(fits("/a b", "/a%20b"), Some(2));
    }

    #[test]
    fn the_operation_written_out_furthest_is_the_one() {
        let h = held(
            r#"{"openapi":"3.0.3","servers":[{"url":"https://example.com/v1/"}],"paths":{
              "/a/{id}":{"get":{"responses":{"200":{"description":""}}}},
              "/a/mine":{"get":{"responses":{"204":{"description":""}}},"post":{"responses":{"201":{"description":""}}}}
            }}"#,
        );
        let status = |m: &str, p: &str| find(&h.responses, m, p).0.map(|op| op.responses[0].status.clone());
        assert_eq!(status("GET", "/a/mine").as_deref(), Some("204"));
        assert_eq!(status("GET", "/a/1?x=1").as_deref(), Some("200"));
        assert_eq!(status("GET", "/v1/a/mine").as_deref(), Some("204"));
        assert_eq!(status("POST", "/a/mine").as_deref(), Some("201"));
        assert_eq!(status("POST", "/a/1"), None);
        assert_eq!(find(&h.responses, "POST", "/a/1").1.len(), 1);
        assert!(find(&h.responses, "GET", "/nope").1.is_empty());
    }

    #[test]
    fn a_status_is_found_as_written_then_by_its_hundreds_then_by_default() {
        let h = held(
            r#"{"openapi":"3.0.3","paths":{"/a":{"get":{"responses":{
              "200":{"description":""},"4XX":{"description":""}
            }}},"/b":{"get":{"responses":{"default":{"description":""}}}}}}"#,
        );
        let (a, b) = (&h.responses.operations[0], &h.responses.operations[1]);
        assert!(h.problems(a, 200, &[], "").is_empty());
        assert!(h.problems(a, 404, &[], "").is_empty());
        assert!(h.problems(b, 500, &[], "").is_empty());
        let wrong = h.problems(a, 500, &[], "");
        assert_eq!(wrong[0].en, "status 500, which the contract does not list (it lists 200, 4XX)");
    }

    #[test]
    fn the_media_type_and_the_headers_are_held() {
        let h = held(
            r#"{"openapi":"3.1.0","paths":{"/a":{"get":{"responses":{
              "200":{"description":"","headers":{"X-Id":{"required":true,"schema":{"type":"string"}},"X-No":{"schema":{"type":"string"}}},
                     "content":{"application/json":{"schema":{"type":"object"}},"text/*":{}}},
              "201":{"description":"","content":{"*/*":{}}}
            }}}}}"#,
        );
        let op = &h.responses.operations[0];
        let answer = |status, media: &str, id: bool, body| {
            let mut headers = vec![("content-type".to_string(), media.to_string())];
            if id {
                headers.push(("x-id".to_string(), "1".to_string()));
            }
            h.problems(op, status, &headers, body).into_iter().map(|t| t.en).collect::<Vec<_>>()
        };
        assert!(answer(200, "application/json", true, "{}").is_empty());
        assert!(answer(200, "text/csv", true, "a,b").is_empty());
        assert!(answer(201, "image/png", false, "x").is_empty());
        assert_eq!(answer(200, "application/json", false, "{}"), ["status 200 without the required header X-Id"]);
        assert_eq!(answer(200, "image/png", true, "x"), ["status 200 as image/png, which the contract does not list (it lists application/json, text/*)"]);
        assert_eq!(answer(200, "application/json", true, "<html>"), ["status 200 with a body that is not JSON"]);
        assert_eq!(h.problems(op, 201, &[], "").iter().map(|t| t.en.as_str()).collect::<Vec<_>>(), ["status 201 with no Content-Type (the contract lists */*)"]);
    }

    #[test]
    fn a_body_is_held_to_its_schema() {
        assert!(wrong(r#"{"type":"object","required":["a"],"properties":{"a":{"type":"integer"}}}"#, r#"{"a":1,"b":2}"#).is_empty());
        assert_eq!(wrong(r#"{"type":"object","required":["a"]}"#, "{}"), ["$: the required a is missing"]);
        assert_eq!(wrong(r#"{"type":"object","properties":{"a":{"type":"string"}}}"#, r#"{"a":1}"#), ["$.a: 1 is number; the contract says string"]);
        assert_eq!(wrong(r#"{"type":"object","additionalProperties":false}"#, r#"{"a":1}"#), ["$.a: the contract has no such member"]);
        assert_eq!(wrong(r#"{"type":"object","additionalProperties":{"type":"string"}}"#, r#"{"a":1}"#), ["$.a: 1 is number; the contract says string"]);
        assert_eq!(wrong(r#"{"type":"array","items":{"type":"integer"}}"#, "[1,1.5]"), ["$[1]: 1.5 is number; the contract says integer"]);
        assert_eq!(wrong(r#"{"enum":["a","b"]}"#, r#""c""#), [r#"$: "c" is not one of "a", "b""#]);
        assert_eq!(wrong(r#"{"const":1}"#, "2"), ["$: 2 is not one of 1"]);
        assert!(wrong(r#"{"type":["string","null"]}"#, "null").is_empty());
        assert_eq!(wrong(r#"{"type":["string","null"]}"#, "1"), ["$: 1 is number; the contract says string or null"]);
        assert!(wrong(r#"{"type":"string","nullable":true}"#, "null").is_empty());
        // `nullable` beside an `allOf`, as 3.0 documents write it
        assert!(wrong(r#"{"nullable":true,"allOf":[{"type":"object"}]}"#, "null").is_empty());
        assert_eq!(wrong(r#"true"#, "1").len(), 0);
        assert_eq!(wrong(r#"false"#, "1"), ["$: the contract allows no value here"]);
    }

    #[test]
    fn the_ranges_of_a_value_are_held() {
        assert_eq!(wrong(r#"{"minimum":1}"#, "0"), ["$: 0 is below the lowest value, 1"]);
        assert!(wrong(r#"{"minimum":1}"#, "1").is_empty());
        assert_eq!(wrong(r#"{"exclusiveMinimum":1}"#, "1"), ["$: 1 is not above 1"]);
        assert_eq!(wrong(r#"{"maximum":1,"exclusiveMaximum":true}"#, "1"), ["$: 1 is not below 1"]);
        assert_eq!(wrong(r#"{"multipleOf":0.5}"#, "0.75"), ["$: 0.75 is not a multiple of 0.5"]);
        assert!(wrong(r#"{"multipleOf":0.1}"#, "0.3").is_empty());
        assert_eq!(wrong(r#"{"minLength":2}"#, r#""あ""#), [r#"$: "あ" has 1 character, fewer than its minLength, 2"#]);
        assert_eq!(wrong(r#"{"maxLength":1}"#, r#""ab""#), [r#"$: "ab" has 2 characters, more than its maxLength, 1"#]);
        assert_eq!(wrong(r#"{"minItems":1}"#, "[]"), ["$: 0 items, fewer than its minItems, 1"]);
        assert_eq!(wrong(r#"{"maxItems":0}"#, "[1]"), ["$: 1 item, more than its maxItems, 0"]);
        assert_eq!(wrong(r#"{"uniqueItems":true}"#, "[1,2,1]"), ["$: 1 comes twice, and its uniqueItems forbids it"]);
        assert_eq!(wrong(r#"{"minProperties":1}"#, "{}"), ["$: 0 members, fewer than its minProperties, 1"]);
        assert_eq!(wrong(r#"{"maxProperties":0}"#, r#"{"a":1}"#), ["$: 1 member, more than its maxProperties, 0"]);
        assert_eq!(wrong(r#"{"prefixItems":[{"type":"string"}],"items":{"type":"integer"}}"#, r#"["a",1,"b"]"#), [r#"$[2]: "b" is string; the contract says integer"#]);
    }

    #[test]
    fn the_combinations_of_schemas_are_held() {
        assert_eq!(wrong(r#"{"allOf":[{"required":["a"]},{"required":["b"]}]}"#, "{}"), ["$: the required a is missing", "$: the required b is missing"]);
        assert!(wrong(r#"{"anyOf":[{"type":"string"},{"type":"integer"}]}"#, "1").is_empty());
        assert_eq!(wrong(r#"{"anyOf":[{"type":"string"},{"type":"integer"}]}"#, "true"), ["$: true fits none of its anyOf"]);
        assert!(wrong(r#"{"oneOf":[{"minimum":1},{"maximum":0}]}"#, "1").is_empty());
        assert_eq!(wrong(r#"{"oneOf":[{"minimum":1},{"minimum":0}]}"#, "1"), ["$: 1 fits 2 of its oneOf, not exactly one"]);
        assert_eq!(wrong(r#"{"not":{"type":"string"}}"#, r#""a""#), [r#"$: "a" fits what its not forbids"#]);
    }

    #[test]
    fn what_only_goes_to_the_service_does_not_come_back() {
        let schema = r#"{"type":"object","required":["id","password"],"properties":{"id":{"type":"string"},"password":{"type":"string","writeOnly":true}}}"#;
        assert!(wrong(schema, r#"{"id":"a"}"#).is_empty());
        assert_eq!(wrong(schema, r#"{"id":"a","password":"x"}"#), ["$.password: the contract says it is only ever sent to the service (writeOnly)"]);
    }

    #[test]
    fn a_schema_that_holds_itself_is_read_and_held() {
        let h = held(
            r##"{"openapi":"3.1.0","paths":{"/a":{"get":{"responses":{"200":{"description":"","content":{"application/json":{"schema":{"$ref":"#/components/schemas/Tree"}}}}}}}},
            "components":{"schemas":{
              "Tree":{"type":"object","required":["name"],"properties":{"name":{"type":"string"},"kids":{"type":"array","items":{"$ref":"#/components/schemas/Tree"}}}},
              "Loop":{"allOf":[{"$ref":"#/components/schemas/Loop"}]}
            }}}"##,
        );
        let headers = [("content-type".to_string(), "application/json".to_string())];
        let op = &h.responses.operations[0];
        assert!(h.problems(op, 200, &headers, r#"{"name":"a","kids":[{"name":"b","kids":[]}]}"#).is_empty());
        let got = h.problems(op, 200, &headers, r#"{"name":"a","kids":[{"kids":[{"name":1}]}]}"#);
        let got: Vec<&str> = got.iter().map(|t| t.en.as_str()).collect();
        assert_eq!(got, ["$.kids[0]: the required name is missing", "$.kids[0].kids[0].name: 1 is number; the contract says string"]);
    }

    #[test]
    fn a_pattern_finds_its_match_anywhere_unless_it_says_where() {
        assert!(wrong(r#"{"pattern":"b"}"#, r#""abc""#).is_empty());
        assert!(wrong(r#"{"pattern":"^ab"}"#, r#""abc""#).is_empty());
        assert!(wrong(r#"{"pattern":"bc$"}"#, r#""abc""#).is_empty());
        assert_eq!(wrong(r#"{"pattern":"^b"}"#, r#""abc""#), [r#"$: "abc" does not match the pattern ^b"#]);
        assert_eq!(wrong(r#"{"pattern":"^[0-9]+$"}"#, r#""12a""#), [r#"$: "12a" does not match the pattern ^[0-9]+$"#]);
        assert!(wrong(r#"{"pattern":"^(a|b)$"}"#, r#""b""#).is_empty());
        assert!(wrong(r#"{"pattern":"a|b"}"#, r#""xbx""#).is_empty());
        assert!(wrong(r#"{"pattern":"^[^x]$"}"#, r#""a""#).is_empty());
        // what geas's patterns cannot say the same is not held
        for source in ["^a|b$", "a$b", "(?=a)", r"\bword\b"] {
            assert!(pattern(source).is_none(), "{source}");
        }
        assert!(pattern(r"a\$").is_some());
    }

    #[test]
    fn the_formats_a_document_can_only_mean_one_way_are_held() {
        for (format, good, bad) in [
            ("date", "2024-02-29", "2023-02-29"),
            ("date", "2026-11-07", "2026/11/07"),
            ("date-time", "2026-11-07T10:00:00Z", "2026-11-07 10:00:00Z"),
            ("date-time", "2026-11-07T10:00:00.5+09:00", "2026-11-07T10:00:00"),
            ("date-time", "2026-11-07T10:00:00.123456789Z", "2026-11-07T25:00:00Z"),
            ("uuid", "123e4567-e89b-12d3-a456-426614174000", "123e4567"),
        ] {
            assert!(wrong(&format!(r#"{{"format":"{format}"}}"#), &format!("\"{good}\"")).is_empty(), "{good}");
            assert_eq!(wrong(&format!(r#"{{"format":"{format}"}}"#), &format!("\"{bad}\"")), [format!("$: \"{bad}\" is not {format}")]);
        }
        assert!(wrong(r#"{"format":"my-own"}"#, r#""x""#).is_empty());
        assert_eq!(wrong(r#"{"format":"int32"}"#, "2147483648"), ["$: 2147483648 is not int32"]);
        assert!(wrong(r#"{"format":"int64"}"#, "2147483648").is_empty());
    }
}

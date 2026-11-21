//! Where the arguments of an `http` task go — a part of the URL's path, the query, a header, a
//! cookie or the body — and how a list or a record is written there, as OpenAPI's `style` and
//! `explode` say (DESIGN 43). The reference interpreter writes its calls with these functions; the
//! code dandori generates writes the same text with its own (io.ts, io.py and io.go, and the
//! JSONata of a state machine), and the comparisons of the tests hold each to these.
//!
//! The text of a value is the one a URL takes everywhere (`render::value_text`): a string as it
//! is, a number or a bool as JSON writes it, a `json` value that is not a string as its JSON. In
//! the path, the query and a cookie it is escaped as JavaScript's encodeURIComponent escapes it,
//! and the marks a style puts around the items (`,` `.` `;` `=` `&`) are not; a header has the text
//! as it is. A value that is not there, a list with no item there and a record with no field there
//! are RFC 6570's undefined: they put nothing where they go, not even the name.

use crate::model::*;
use crate::render::{jsonata_field, jsonata_string, url_segment, value_text};
use ritsu_base::text::Text;
use serde_json::{Map, Value};

/// What a value is, for how it is written: one value, a `json` value (which goes as its text), a
/// list of values, or a record with its fields in the order the record declares them.
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    One,
    Json,
    List,
    Record(Vec<String>),
}

/// The kind of a value of `t`: None for a list or a record that holds a list or a record, which
/// no style writes outside a JSON body.
pub fn kind(m: &Model, t: &Ty) -> Option<Kind> {
    let flat = |t: &Ty| !matches!(t.inner(), Ty::List(_) | Ty::Record(_));
    match t.inner() {
        Ty::List(x) => flat(x).then_some(Kind::List),
        Ty::Record(r) => {
            let fields = &m.records[*r].fields;
            fields.iter().all(|(_, ft)| flat(ft)).then(|| Kind::Record(fields.iter().map(|(f, _)| f.clone()).collect()))
        }
        Ty::Json => Some(Kind::Json),
        _ => Some(Kind::One),
    }
}

/// Where the parameters of an `http` task go when no description says (DESIGN 43): a parameter the
/// URL names to the path, the others to the query of a `GET` or a `DELETE` and to the body of the
/// other methods, each written as OpenAPI's defaults write it.
pub fn defaults(method: &str, url: &str, params: &[(String, Ty)]) -> Vec<WireParam> {
    let in_path = crate::lower::placeholders(url);
    let get = method == "GET" || method == "DELETE";
    params
        .iter()
        .map(|(p, _)| {
            let place = if in_path.contains(p) {
                Place::Path
            } else if get {
                Place::Query
            } else {
                Place::Body
            };
            WireParam::plain(p, place, p)
        })
        .collect()
}

/// Where a parameter goes, in words: `the query`, `the URL-encoded body`.
pub fn place_words(place: Place, form: bool) -> Text {
    match place {
        Place::Path => tr!("パス", "the path"),
        Place::Query => tr!("クエリ", "the query"),
        Place::Header => tr!("ヘッダー", "a header"),
        Place::Cookie => tr!("クッキー", "a cookie"),
        Place::Body if form => tr!("URL エンコードの本文", "the URL-encoded body"),
        Place::Body => tr!("JSON の本文", "the JSON body"),
    }
}

/// Why a value of `t` cannot be written where `w` says, in both languages; None when it can. `form`:
/// the body is URL-encoded (a JSON body takes any value).
pub fn unwritable(m: &Model, w: &WireParam, t: &Ty, form: bool) -> Option<Text> {
    if w.place == Place::Body && !form {
        return None;
    }
    let at = place_words(w.place, form);
    let Some(k) = kind(m, t) else {
        return Some(tr!("{}には、リストやレコードを中に持つリストやレコードを書く書き方がありません", "no style writes a list or a record that holds a list or a record in {}", at.ja; at.en));
    };
    let (s, x) = (w.style.word(), w.explode);
    let styles: &[Style] = match w.place {
        Place::Path => &[Style::Simple, Style::Label, Style::Matrix],
        Place::Header => &[Style::Simple],
        Place::Cookie => &[Style::Form],
        Place::Query | Place::Body => &[Style::Form, Style::SpaceDelimited, Style::PipeDelimited, Style::DeepObject],
    };
    if !styles.contains(&w.style) {
        let list: Vec<&str> = styles.iter().map(|s| s.word()).collect();
        let en = match list.split_last() {
            Some((last, rest)) if !rest.is_empty() => format!("{} or {last}", rest.join(", ")),
            _ => list.join(""),
        };
        let ja = if list.len() > 1 { format!("{} のどれかで", list.join("・")) } else { format!("{} で", list.join("")) };
        return Some(tr!("{}の style は {ja}、{s} は書けません", "{} takes the style {en}, not {s}", at.ja; at.en));
    }
    let composite = matches!(k, Kind::List | Kind::Record(_));
    match (&k, w.style, x) {
        (Kind::Json, Style::SpaceDelimited | Style::PipeDelimited | Style::DeepObject, _) => Some(tr!(
            "`json` の値は JSON の文字列で送るので、{s} では書けません。リストかレコードの型にしてください",
            "a `json` value goes as its JSON text, which {s} does not write; declare it a list or a record"
        )),
        (_, Style::Form, true) if w.place == Place::Cookie && composite => Some(tr!(
            "explode: true の form では、リストやレコードがクッキーの書き方になりません。explode: false にしてください",
            "form with explode: true does not write a list or a record as a cookie; it takes explode: false"
        )),
        (Kind::List, Style::DeepObject, _) => Some(tr!("deepObject はオブジェクトの書き方で、リストを書く決まりがありません", "deepObject writes an object, and has no form for a list")),
        (Kind::List | Kind::Record(_), Style::SpaceDelimited | Style::PipeDelimited, true) => Some(tr!("{s} の explode: true は、決まりに無い書き方です。explode: false にしてください", "{s} with explode: true is a form OpenAPI does not define; it takes explode: false")),
        (Kind::Record(_), Style::DeepObject, false) => Some(tr!("deepObject の explode: false は、決まりに無い書き方です。explode: true にしてください", "deepObject with explode: false is a form OpenAPI does not define; it takes explode: true")),
        _ => None,
    }
}

/// The names the parameters at `place` (the query, or a URL-encoded body) put there, with the
/// parameter each comes from: a record that `form` writes with `explode` puts its fields' names,
/// `deepObject` `name[field]`, the others their own.
fn keys(m: &Model, task: &TaskDef, place: Place) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for w in task.wire.iter().filter(|w| w.place == place) {
        let Some((_, t)) = task.params.iter().find(|(p, _)| *p == w.param) else { continue };
        match (kind(m, t), w.style, w.explode) {
            (Some(Kind::Record(fields)), Style::Form, true) => out.extend(fields.into_iter().map(|f| (f, w.param.clone()))),
            (Some(Kind::Record(fields)), Style::DeepObject, _) => out.extend(fields.into_iter().map(|f| (format!("{}[{f}]", w.name), w.param.clone()))),
            _ => out.push((w.name.clone(), w.param.clone())),
        }
    }
    out
}

/// Two parameters that put a value under one name in the query or in a URL-encoded body (`form`),
/// which the other side cannot tell apart: the name, the two, and where.
pub fn clash(m: &Model, task: &TaskDef, form: bool) -> Option<(String, String, String, Text)> {
    let mut places = vec![Place::Query];
    if form {
        places.push(Place::Body);
    }
    for place in places {
        let keys = keys(m, task, place);
        for (i, (k, a)) in keys.iter().enumerate() {
            if let Some((_, b)) = keys[i + 1..].iter().find(|(k2, b)| k2 == k && b != a) {
                return Some((k.clone(), a.clone(), b.clone(), place_words(place, form)));
            }
        }
    }
    None
}

/// What two parameters that put a value under one name (`clash`) are told, in both languages.
pub fn clash_words((key, a, b, at): &(String, String, String, Text)) -> Text {
    tr!(
        "`{a}` と `{b}` が、{}の同じ名前 `{key}` に値を入れます。受け取る側は二つを分けられません",
        "`{a}` and `{b}` both put a value under `{key}` in {}, where the other side cannot tell them apart",
        at.ja;
        at.en
    )
}

/// The parameter of the path that the URL's `{name}` takes: the one that goes by that name.
pub fn path_param<'a>(m: &Model, task: &'a TaskDef, name: &str) -> Option<(&'a WireParam, Kind, &'a Ty)> {
    at(m, task, Place::Path).into_iter().find(|(w, _, _)| w.name == name)
}

/// One value put into the path as `simple` writes it: its escaped text, which the code dandori
/// writes as it did before DESIGN 43 (`io.segment`, `ddEscape`, `render::jsonata_segment`).
pub fn plain_segment(w: &WireParam, k: &Kind) -> bool {
    matches!(k, Kind::One | Kind::Json) && w.style == Style::Simple
}

// ── The text, as the reference interpreter writes it ────────────────────────

fn enc(s: &str) -> String {
    url_segment(s)
}

/// The items of a list that are there, as text (one value that is not a list is its one item).
fn items(v: &Value) -> Vec<String> {
    match v {
        Value::Array(a) => a.iter().filter(|x| !x.is_null()).map(value_text).collect(),
        Value::Null => vec![],
        x => vec![value_text(x)],
    }
}

/// The fields of a record that are there, in the record's order, with their text.
fn fields(v: &Value, names: &[String]) -> Vec<(String, String)> {
    names.iter().filter_map(|f| v.get(f).filter(|x| !x.is_null()).map(|x| (f.clone(), value_text(x)))).collect()
}

/// The items of a list, or the names and values of a record's fields in turn, escaped when `escape`
/// says, between `sep`; None when there are none.
fn joined(k: &Kind, v: &Value, sep: &str, escape: bool) -> Option<String> {
    let parts: Vec<String> = match k {
        Kind::Record(names) => fields(v, names).into_iter().flat_map(|(f, t)| [f, t]).collect(),
        _ => items(v),
    };
    if parts.is_empty() {
        return None;
    }
    Some(parts.iter().map(|s| if escape { enc(s) } else { s.clone() }).collect::<Vec<_>>().join(sep))
}

/// `;name=text`, or `;name` when the text is empty (RFC 6570's matrix).
fn named(n: &str, t: &str) -> String {
    if t.is_empty() {
        format!(";{n}")
    } else {
        format!(";{n}={t}")
    }
}

/// The pairs a parameter puts in the query, each `name=value` with both escaped; none for a value
/// that is not there.
pub fn query_pairs(w: &WireParam, k: &Kind, v: &Value) -> Vec<String> {
    if v.is_null() {
        return vec![];
    }
    let n = enc(&w.name);
    match (k, w.style, w.explode) {
        (Kind::One | Kind::Json, _, _) => vec![format!("{n}={}", enc(&value_text(v)))],
        (Kind::List, Style::Form, true) => items(v).iter().map(|t| format!("{n}={}", enc(t))).collect(),
        (Kind::Record(names), Style::Form, true) => fields(v, names).iter().map(|(f, t)| format!("{}={}", enc(f), enc(t))).collect(),
        (Kind::Record(names), Style::DeepObject, _) => fields(v, names).iter().map(|(f, t)| format!("{}={}", enc(&format!("{}[{f}]", w.name)), enc(t))).collect(),
        (_, style, _) => {
            let sep = match style {
                Style::SpaceDelimited => "%20",
                Style::PipeDelimited => "%7C",
                _ => ",",
            };
            joined(k, v, sep, true).map(|j| vec![format!("{n}={j}")]).unwrap_or_default()
        }
    }
}

/// The query of a call's parameters after `mark` (`?`, or `&` after a query the URL has of its own):
/// empty when no pair is there.
pub fn query_text(mark: &str, pairs: &[String]) -> String {
    if pairs.is_empty() {
        String::new()
    } else {
        format!("{mark}{}", pairs.join("&"))
    }
}

/// A parameter put into the path, as its style writes it.
pub fn path_text(w: &WireParam, k: &Kind, v: &Value) -> String {
    if v.is_null() {
        return String::new();
    }
    let n = enc(&w.name);
    match k {
        Kind::One | Kind::Json => {
            let t = enc(&value_text(v));
            match w.style {
                Style::Label => format!(".{t}"),
                Style::Matrix => named(&n, &t),
                _ => t,
            }
        }
        Kind::List => {
            let e: Vec<String> = items(v).iter().map(|s| enc(s)).collect();
            if e.is_empty() {
                return String::new();
            }
            match (w.style, w.explode) {
                (Style::Label, false) => format!(".{}", e.join(",")),
                (Style::Label, true) => format!(".{}", e.join(".")),
                (Style::Matrix, false) => named(&n, &e.join(",")),
                (Style::Matrix, true) => e.iter().map(|t| named(&n, t)).collect(),
                _ => e.join(","),
            }
        }
        Kind::Record(names) => {
            let fs: Vec<(String, String)> = fields(v, names).iter().map(|(f, t)| (enc(f), enc(t))).collect();
            if fs.is_empty() {
                return String::new();
            }
            let flat = fs.iter().flat_map(|(f, t)| [f.clone(), t.clone()]).collect::<Vec<_>>().join(",");
            let pairs = |sep: &str| fs.iter().map(|(f, t)| format!("{f}={t}")).collect::<Vec<_>>().join(sep);
            match (w.style, w.explode) {
                (Style::Label, false) => format!(".{flat}"),
                (Style::Label, true) => format!(".{}", pairs(".")),
                (Style::Matrix, false) => named(&n, &flat),
                (Style::Matrix, true) => fs.iter().map(|(f, t)| named(f, t)).collect(),
                (_, true) => pairs(","),
                (_, false) => flat,
            }
        }
    }
}

/// A parameter's header, as `simple` writes it, with the text as it is; None for a value that is not
/// there.
pub fn header_text(w: &WireParam, k: &Kind, v: &Value) -> Option<String> {
    if v.is_null() {
        return None;
    }
    match k {
        Kind::One | Kind::Json => Some(value_text(v)),
        Kind::List => joined(k, v, ",", false),
        Kind::Record(names) if w.explode => {
            let fs = fields(v, names);
            (!fs.is_empty()).then(|| fs.iter().map(|(f, t)| format!("{f}={t}")).collect::<Vec<_>>().join(","))
        }
        Kind::Record(_) => joined(k, v, ",", false),
    }
}

/// A parameter's cookie, `name=value` as `form` writes it, the value escaped; None for a value that is
/// not there.
pub fn cookie_pair(w: &WireParam, k: &Kind, v: &Value) -> Option<String> {
    if v.is_null() {
        return None;
    }
    match k {
        Kind::One | Kind::Json => Some(format!("{}={}", w.name, enc(&value_text(v)))),
        _ => joined(k, v, ",", true).map(|j| format!("{}={j}", w.name)),
    }
}

/// What a parameter puts in a URL-encoded body, as the Transport is handed it: one value as it is (a
/// `json` one as its text), a list `form` writes with `explode` as the texts of its items, which the
/// key repeats for, a record's fields by their names or by `name[field]`, and the joined text of the
/// other styles. The Transport (Step Functions, on a state machine) escapes it.
pub fn form_entries(w: &WireParam, k: &Kind, v: &Value, out: &mut Map<String, Value>) {
    if v.is_null() {
        return;
    }
    match (k, w.style, w.explode) {
        (Kind::One, _, _) => {
            out.insert(w.name.clone(), v.clone());
        }
        (Kind::Json, _, _) => {
            out.insert(w.name.clone(), Value::String(value_text(v)));
        }
        (Kind::List, Style::Form, true) => {
            let its = items(v);
            if !its.is_empty() {
                out.insert(w.name.clone(), Value::Array(its.into_iter().map(Value::String).collect()));
            }
        }
        (Kind::Record(names), Style::Form, true) => {
            for (f, t) in fields(v, names) {
                out.insert(f, Value::String(t));
            }
        }
        (Kind::Record(names), Style::DeepObject, _) => {
            for (f, t) in fields(v, names) {
                out.insert(format!("{}[{f}]", w.name), Value::String(t));
            }
        }
        (_, style, _) => {
            let sep = match style {
                Style::SpaceDelimited => " ",
                Style::PipeDelimited => "|",
                _ => ",",
            };
            if let Some(j) = joined(k, v, sep, false) {
                out.insert(w.name.clone(), Value::String(j));
            }
        }
    }
}

/// The parameters of `task` that go to `place`, with the kind and the type of each, in their order.
pub fn at<'a>(m: &Model, task: &'a TaskDef, place: Place) -> Vec<(&'a WireParam, Kind, &'a Ty)> {
    task.wire
        .iter()
        .filter(|w| w.place == place)
        .filter_map(|w| {
            let (_, t) = task.params.iter().find(|(p, _)| *p == w.param)?;
            Some((w, kind(m, t)?, t))
        })
        .collect()
}

/// A URL-encoded body whose parameters are all values that are always there, which goes as an object
/// of the values as they are, as it went before DESIGN 43; the others are put together at the call.
pub fn plain_form(m: &Model, task: &TaskDef) -> bool {
    at(m, task, Place::Body).iter().all(|(_, k, t)| *k == Kind::One && !matches!(t, Ty::Opt(_)))
}

/// The call of an `http` task with concrete arguments, as every target sends it (DESIGN 43).
pub struct Call {
    /// the URL with the path's arguments put in, and the query after it
    pub url: String,
    /// dandori's own headers first, then the parameters', then the cookies' as one `Cookie`
    pub headers: Map<String, Value>,
    /// the body, when the task has a parameter that goes there: a JSON body by the parameters'
    /// names (a value that is not there as null), or a URL-encoded one as `form_entries` puts it
    pub body: Option<Value>,
}

/// The call of `task` at `url` (the task's, with its `{placeholders}`) with `args`, by the
/// parameters' names: `own` are dandori's own headers.
pub fn compose(m: &Model, task: &TaskDef, url: &str, form: bool, args: &Map<String, Value>, own: Map<String, Value>) -> Call {
    let value = |p: &str| args.get(p).cloned().unwrap_or(Value::Null);
    let mut filled = url.to_string();
    for p in crate::lower::placeholders(url) {
        let text = match path_param(m, task, &p) {
            Some((w, k, _)) => path_text(w, &k, &value(&w.param)),
            None => enc(&value_text(&value(&p))),
        };
        filled = filled.replace(&format!("{{{p}}}"), &text);
    }
    let pairs: Vec<String> = at(m, task, Place::Query).iter().flat_map(|(w, k, _)| query_pairs(w, k, &value(&w.param))).collect();
    let mark = if url.contains('?') { "&" } else { "?" };
    filled.push_str(&query_text(mark, &pairs));
    let mut headers = own;
    for (w, k, _) in at(m, task, Place::Header) {
        if let Some(t) = header_text(w, &k, &value(&w.param)) {
            headers.insert(w.name.clone(), Value::String(t));
        }
    }
    let cookies: Vec<String> = at(m, task, Place::Cookie).iter().filter_map(|(w, k, _)| cookie_pair(w, k, &value(&w.param))).collect();
    if !cookies.is_empty() {
        headers.insert("Cookie".into(), Value::String(cookies.join("; ")));
    }
    let body_params: Vec<&WireParam> = task.wire.iter().filter(|w| w.place == Place::Body).collect();
    let body = (!body_params.is_empty()).then(|| {
        let mut b = Map::new();
        if form {
            for (w, k, _) in at(m, task, Place::Body) {
                form_entries(w, &k, &value(&w.param), &mut b);
            }
        } else {
            for w in body_params {
                b.insert(w.param.clone(), value(&w.param));
            }
        }
        Value::Object(b)
    });
    Call { url: filled, headers, body }
}

// ── The same text, in the JSONata of a state machine ─────────────────────────

/// The text of the value `x` (a JSONata variable) of type `t`, as `value_text` writes it.
fn jtext(x: &str, t: &Ty) -> String {
    match t.inner() {
        Ty::Str | Ty::Timestamp | Ty::Date | Ty::Enum(_) => x.to_string(),
        Ty::Int | Ty::Num(_) | Ty::Bool => format!("$string({x})"),
        _ => format!("($type({x}) = \"string\" ? {x} : $string({x}))"),
    }
}

fn jenc(s: &str) -> String {
    format!("$encodeUrlComponent({s})")
}

/// The type of an item of a list of type `t`.
fn item_ty(t: &Ty) -> Ty {
    match t.inner() {
        Ty::List(x) => (**x).clone(),
        other => other.clone(),
    }
}

/// The types of a record's fields, by their names.
fn field_ty(m: &Model, t: &Ty, f: &str) -> Ty {
    match t.inner() {
        Ty::Record(r) => m.records[*r].fields.iter().find(|(n, _)| n == f).map(|(_, ft)| ft.clone()).unwrap_or(Ty::Json),
        _ => Ty::Json,
    }
}

/// The items of the list `e` that are there, bound to `$dd_l` (none, one, or an array of them).
fn jlist(e: &str) -> String {
    format!("$dd_l := $filter({e}, function($dd_i) {{ $dd_i != null }})")
}

/// `f` of each item of `$dd_l`, the item being `$dd_i`.
fn jmap(f: &str) -> String {
    format!("$map($dd_l, function($dd_i) {{ {f} }})")
}

/// The record `$dd_r`'s field `f`, and whether it is there.
fn jfield(f: &str) -> (String, String) {
    let x = format!("$dd_r.{}", jsonata_field(f));
    (format!("{x} != null"), x)
}

/// What a parameter puts in the query, in JSONata (`e` its value): nothing, one `name=value` or an
/// array of them, as `query_pairs` writes them.
pub fn jsonata_query(m: &Model, w: &WireParam, k: &Kind, t: &Ty, e: &str) -> String {
    let n = enc(&w.name);
    let it = item_ty(t);
    match (k, w.style, w.explode) {
        (Kind::One | Kind::Json, _, _) => format!("($dd_v := {e}; $dd_v != null ? {} & {})", jsonata_string(&format!("{n}=")), jenc(&jtext("$dd_v", t))),
        (Kind::List, Style::Form, true) => format!("({}; {})", jlist(e), jmap(&format!("{} & {}", jsonata_string(&format!("{n}=")), jenc(&jtext("$dd_i", &it))))),
        (Kind::Record(names), Style::Form | Style::DeepObject, _) if w.style == Style::DeepObject || w.explode => {
            let parts: Vec<String> = names
                .iter()
                .map(|f| {
                    let key = if w.style == Style::DeepObject { enc(&format!("{}[{f}]", w.name)) } else { enc(f) };
                    let (there, x) = jfield(f);
                    format!("{there} ? {} & {}", jsonata_string(&format!("{key}=")), jenc(&jtext(&x, &field_ty(m, t, f))))
                })
                .collect();
            format!("($dd_r := {e}; [{}])", parts.join(", "))
        }
        (Kind::List, style, _) => {
            let sep = sep_escaped(style);
            format!("({}; $count($dd_l) > 0 ? {} & $join({}, {}))", jlist(e), jsonata_string(&format!("{n}=")), jmap(&jenc(&jtext("$dd_i", &it))), jsonata_string(sep))
        }
        (Kind::Record(names), style, _) => {
            let sep = sep_escaped(style);
            format!("($dd_r := {e}; $dd_p := [{}]; $count($dd_p) > 0 ? {} & $join($dd_p, {}))", jflat(m, t, names, sep, true), jsonata_string(&format!("{n}=")), jsonata_string(sep))
        }
    }
}

/// The mark a style puts between the items in the query, escaped.
fn sep_escaped(style: Style) -> &'static str {
    match style {
        Style::SpaceDelimited => "%20",
        Style::PipeDelimited => "%7C",
        _ => ",",
    }
}

/// The mark a style puts between the items in a URL-encoded body, before the Transport escapes it.
fn sep_plain(style: Style) -> &'static str {
    match style {
        Style::SpaceDelimited => " ",
        Style::PipeDelimited => "|",
        _ => ",",
    }
}

/// Each field of the record `$dd_r` there, as `name<sep>value`, escaped when `escape` says: the
/// items of the array a record that is not exploded is joined from.
fn jflat(m: &Model, t: &Ty, names: &[String], sep: &str, escape: bool) -> String {
    names
        .iter()
        .map(|f| {
            let (there, x) = jfield(f);
            let text = jtext(&x, &field_ty(m, t, f));
            let (name, value) = if escape { (enc(f), jenc(&text)) } else { (f.clone(), text) };
            format!("{there} ? {} & {value}", jsonata_string(&format!("{name}{sep}")))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The query of a call in JSONata, from what each parameter puts in it (`jsonata_query`): `""`, or
/// `mark` and the pairs.
pub fn jsonata_query_text(mark: &str, parts: &[String]) -> String {
    let mut all = parts[0].clone();
    for p in &parts[1..] {
        all = format!("$append({all}, {p})");
    }
    format!("($dd_q := $join({all}, \"&\"); $dd_q ? {} & $dd_q : \"\")", jsonata_string(mark))
}

/// A parameter put into the path, in JSONata, as `path_text` writes it (not for a value `simple`
/// writes, which `render::jsonata_segment` does).
pub fn jsonata_path(m: &Model, w: &WireParam, k: &Kind, t: &Ty, e: &str) -> String {
    let n = enc(&w.name);
    let it = item_ty(t);
    let named = |text: &str| format!("({text} = \"\" ? {} : {} & {text})", jsonata_string(&format!(";{n}")), jsonata_string(&format!(";{n}=")));
    match k {
        Kind::One | Kind::Json => {
            let t = jenc(&jtext("$dd_v", t));
            let body = match w.style {
                Style::Label => format!("\".\" & {t}"),
                Style::Matrix => format!("($dd_t := {t}; {})", named("$dd_t")),
                _ => t,
            };
            format!("($dd_v := {e}; $dd_v != null ? {body} : \"\")")
        }
        Kind::List => {
            let each = jmap(&jenc(&jtext("$dd_i", &it)));
            let body = match (w.style, w.explode) {
                (Style::Label, false) => format!("\".\" & $join({each}, \",\")"),
                (Style::Label, true) => format!("\".\" & $join({each}, \".\")"),
                (Style::Matrix, false) => format!("($dd_t := $join({each}, \",\"); {})", named("$dd_t")),
                (Style::Matrix, true) => format!("$join({}, \"\")", jmap(&format!("($dd_t := {}; {})", jenc(&jtext("$dd_i", &it)), named("$dd_t")))),
                _ => format!("$join({each}, \",\")"),
            };
            format!("({}; $count($dd_l) > 0 ? {body} : \"\")", jlist(e))
        }
        Kind::Record(names) => {
            let pairs = |sep: &str| -> String {
                names
                    .iter()
                    .map(|f| {
                        let (there, x) = jfield(f);
                        format!("{there} ? {} & {}", jsonata_string(&format!("{}{sep}", enc(f))), jenc(&jtext(&x, &field_ty(m, t, f))))
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let body = match (w.style, w.explode) {
                (Style::Label, false) => format!("\".\" & $join($dd_p, \",\")"),
                (Style::Label, true) => format!("\".\" & $join($dd_p, \".\")"),
                (Style::Matrix, false) => format!("{} & $join($dd_p, \",\")", jsonata_string(&format!(";{n}="))),
                (Style::Matrix, true) => "$join($dd_p, \"\")".to_string(),
                _ => "$join($dd_p, \",\")".to_string(),
            };
            let items = match (w.style, w.explode) {
                (Style::Matrix, true) => names
                    .iter()
                    .map(|f| {
                        let (there, x) = jfield(f);
                        let ef = enc(f);
                        format!("{there} ? ($dd_t := {}; $dd_t = \"\" ? {} : {} & $dd_t)", jenc(&jtext(&x, &field_ty(m, t, f))), jsonata_string(&format!(";{ef}")), jsonata_string(&format!(";{ef}=")))
                    })
                    .collect::<Vec<_>>()
                    .join(", "),
                (_, true) => pairs("="),
                (_, false) => jflat(m, t, names, ",", true),
            };
            format!("($dd_r := {e}; $dd_p := [{items}]; $count($dd_p) > 0 ? {body} : \"\")")
        }
    }
}

/// A parameter's header in JSONata, as `header_text` writes it: nothing, or `{name: text}`.
pub fn jsonata_header(m: &Model, w: &WireParam, k: &Kind, t: &Ty, e: &str) -> String {
    let name = jsonata_string(&w.name);
    let it = item_ty(t);
    match k {
        Kind::One | Kind::Json => format!("($dd_v := {e}; $dd_v != null ? {{{name}: {}}})", jtext("$dd_v", t)),
        Kind::List => format!("({}; $count($dd_l) > 0 ? {{{name}: $join({}, \",\")}})", jlist(e), jmap(&jtext("$dd_i", &it))),
        Kind::Record(names) => {
            let items = if w.explode {
                names
                    .iter()
                    .map(|f| {
                        let (there, x) = jfield(f);
                        format!("{there} ? {} & {}", jsonata_string(&format!("{f}=")), jtext(&x, &field_ty(m, t, f)))
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            } else {
                jflat(m, t, names, ",", false)
            };
            format!("($dd_r := {e}; $dd_p := [{items}]; $count($dd_p) > 0 ? {{{name}: $join($dd_p, \",\")}})")
        }
    }
}

/// A parameter's cookie in JSONata, as `cookie_pair` writes it: nothing, or `name=value`.
pub fn jsonata_cookie(m: &Model, w: &WireParam, k: &Kind, t: &Ty, e: &str) -> String {
    let lead = jsonata_string(&format!("{}=", w.name));
    let it = item_ty(t);
    match k {
        Kind::One | Kind::Json => format!("($dd_v := {e}; $dd_v != null ? {lead} & {})", jenc(&jtext("$dd_v", t))),
        Kind::List => format!("({}; $count($dd_l) > 0 ? {lead} & $join({}, \",\"))", jlist(e), jmap(&jenc(&jtext("$dd_i", &it)))),
        Kind::Record(names) => format!("($dd_r := {e}; $dd_p := [{}]; $count($dd_p) > 0 ? {lead} & $join($dd_p, \",\"))", jflat(m, t, names, ",", true)),
    }
}

/// The headers of a call in JSONata: `own` (an object, in JSONata), then each parameter's
/// (`jsonata_header`), then the cookies (`jsonata_cookie`) as one `Cookie`.
pub fn jsonata_headers(own: &str, headers: &[String], cookies: &[String]) -> String {
    let mut parts = vec![own.to_string()];
    parts.extend(headers.iter().cloned());
    if !cookies.is_empty() {
        parts.push(format!("($dd_c := [{}]; $count($dd_c) > 0 ? {{\"Cookie\": $join($dd_c, \"; \")}})", cookies.join(", ")));
    }
    format!("$merge([{}])", parts.join(", "))
}

/// What a parameter puts in a URL-encoded body, in JSONata, as `form_entries` puts it: nothing, or
/// an object of the names and values.
pub fn jsonata_form(m: &Model, w: &WireParam, k: &Kind, t: &Ty, e: &str) -> String {
    let name = jsonata_string(&w.name);
    let it = item_ty(t);
    match (k, w.style, w.explode) {
        (Kind::One, _, _) => format!("($dd_v := {e}; $dd_v != null ? {{{name}: $dd_v}})"),
        (Kind::Json, _, _) => format!("($dd_v := {e}; $dd_v != null ? {{{name}: {}}})", jtext("$dd_v", t)),
        (Kind::List, Style::Form, true) => format!("({}; $count($dd_l) > 0 ? {{{name}: [{}]}})", jlist(e), jmap(&jtext("$dd_i", &it))),
        (Kind::Record(names), Style::Form | Style::DeepObject, _) if w.style == Style::DeepObject || w.explode => {
            let parts: Vec<String> = names
                .iter()
                .map(|f| {
                    let key = if w.style == Style::DeepObject { format!("{}[{f}]", w.name) } else { f.clone() };
                    let (there, x) = jfield(f);
                    format!("{there} ? {{{}: {}}}", jsonata_string(&key), jtext(&x, &field_ty(m, t, f)))
                })
                .collect();
            format!("($dd_r := {e}; $merge([{}]))", parts.join(", "))
        }
        (Kind::List, style, _) => format!("({}; $count($dd_l) > 0 ? {{{name}: $join({}, {})}})", jlist(e), jmap(&jtext("$dd_i", &it)), jsonata_string(sep_plain(style))),
        (Kind::Record(names), style, _) => {
            let sep = sep_plain(style);
            format!("($dd_r := {e}; $dd_p := [{}]; $count($dd_p) > 0 ? {{{name}: $join($dd_p, {})}})", jflat(m, t, names, sep, false), jsonata_string(sep))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn w(place: Place, style: Style, explode: bool) -> WireParam {
        WireParam { param: "color".into(), place, name: "color".into(), style, explode }
    }

    fn rgb() -> Kind {
        Kind::Record(vec!["R".into(), "G".into(), "B".into()])
    }

    /// OpenAPI's table of style examples (3.1.1, "Style Examples"), for `color` as `blue`,
    /// `["blue", "black", "brown"]` and `{"R": 100, "G": 200, "B": 150}`; the label style of a list
    /// and an object not exploded as RFC 6570 writes them.
    #[test]
    fn the_styles_write_what_openapi_says() {
        let one = json!("blue");
        let list = json!(["blue", "black", "brown"]);
        let obj = json!({ "R": 100, "G": 200, "B": 150 });
        let path = |s, x, k: &Kind, v: &Value| path_text(&w(Place::Path, s, x), k, v);
        assert_eq!(path(Style::Matrix, false, &Kind::One, &one), ";color=blue");
        assert_eq!(path(Style::Matrix, false, &Kind::List, &list), ";color=blue,black,brown");
        assert_eq!(path(Style::Matrix, false, &rgb(), &obj), ";color=R,100,G,200,B,150");
        assert_eq!(path(Style::Matrix, true, &Kind::One, &one), ";color=blue");
        assert_eq!(path(Style::Matrix, true, &Kind::List, &list), ";color=blue;color=black;color=brown");
        assert_eq!(path(Style::Matrix, true, &rgb(), &obj), ";R=100;G=200;B=150");
        assert_eq!(path(Style::Label, false, &Kind::One, &one), ".blue");
        assert_eq!(path(Style::Label, false, &Kind::List, &list), ".blue,black,brown");
        assert_eq!(path(Style::Label, false, &rgb(), &obj), ".R,100,G,200,B,150");
        assert_eq!(path(Style::Label, true, &Kind::List, &list), ".blue.black.brown");
        assert_eq!(path(Style::Label, true, &rgb(), &obj), ".R=100.G=200.B=150");
        assert_eq!(path(Style::Simple, false, &Kind::One, &one), "blue");
        assert_eq!(path(Style::Simple, false, &Kind::List, &list), "blue,black,brown");
        assert_eq!(path(Style::Simple, false, &rgb(), &obj), "R,100,G,200,B,150");
        assert_eq!(path(Style::Simple, true, &Kind::List, &list), "blue,black,brown");
        assert_eq!(path(Style::Simple, true, &rgb(), &obj), "R=100,G=200,B=150");
        // an empty string (the table's "empty")
        assert_eq!(path(Style::Matrix, false, &Kind::One, &json!("")), ";color");
        assert_eq!(path(Style::Label, false, &Kind::One, &json!("")), ".");
        let q = |s, x, k: &Kind, v: &Value| query_text("?", &query_pairs(&w(Place::Query, s, x), k, v));
        assert_eq!(q(Style::Form, false, &Kind::One, &one), "?color=blue");
        assert_eq!(q(Style::Form, false, &Kind::List, &list), "?color=blue,black,brown");
        assert_eq!(q(Style::Form, false, &rgb(), &obj), "?color=R,100,G,200,B,150");
        assert_eq!(q(Style::Form, true, &Kind::List, &list), "?color=blue&color=black&color=brown");
        assert_eq!(q(Style::Form, true, &rgb(), &obj), "?R=100&G=200&B=150");
        assert_eq!(q(Style::Form, true, &Kind::One, &json!("")), "?color=");
        assert_eq!(q(Style::SpaceDelimited, false, &Kind::List, &list), "?color=blue%20black%20brown");
        assert_eq!(q(Style::SpaceDelimited, false, &rgb(), &obj), "?color=R%20100%20G%20200%20B%20150");
        assert_eq!(q(Style::PipeDelimited, false, &Kind::List, &list), "?color=blue%7Cblack%7Cbrown");
        assert_eq!(q(Style::PipeDelimited, false, &rgb(), &obj), "?color=R%7C100%7CG%7C200%7CB%7C150");
        assert_eq!(q(Style::DeepObject, true, &rgb(), &obj), "?color%5BR%5D=100&color%5BG%5D=200&color%5BB%5D=150");
        let h = |x, k: &Kind, v: &Value| header_text(&w(Place::Header, Style::Simple, x), k, v);
        assert_eq!(h(false, &Kind::List, &list).as_deref(), Some("blue,black,brown"));
        assert_eq!(h(false, &rgb(), &obj).as_deref(), Some("R,100,G,200,B,150"));
        assert_eq!(h(true, &rgb(), &obj).as_deref(), Some("R=100,G=200,B=150"));
        let c = |k: &Kind, v: &Value| cookie_pair(&w(Place::Cookie, Style::Form, false), k, v);
        assert_eq!(c(&Kind::One, &one).as_deref(), Some("color=blue"));
        assert_eq!(c(&Kind::List, &list).as_deref(), Some("color=blue,black,brown"));
        assert_eq!(c(&rgb(), &obj).as_deref(), Some("color=R,100,G,200,B,150"));
    }

    /// A value that is not there, an empty list and a record with no field there put nothing; an item
    /// or a field that is not there is left out; the marks a style puts are not escaped, and the
    /// text is.
    #[test]
    fn what_is_not_there_puts_nothing_and_the_text_is_escaped() {
        let q = |s, x, k: &Kind, v: &Value| query_pairs(&w(Place::Query, s, x), k, v);
        assert!(q(Style::Form, true, &Kind::One, &Value::Null).is_empty());
        assert!(q(Style::Form, true, &Kind::List, &json!([])).is_empty());
        assert!(q(Style::Form, false, &Kind::List, &json!([])).is_empty());
        assert!(q(Style::Form, true, &rgb(), &json!({ "R": null })).is_empty());
        assert_eq!(q(Style::Form, true, &rgb(), &json!({ "R": 1, "G": null, "B": "a b" })), vec!["R=1", "B=a%20b"]);
        assert_eq!(q(Style::Form, false, &Kind::List, &json!(["a,b", "c&d", "東"])), vec!["color=a%2Cb,c%26d,%E6%9D%B1"]);
        assert_eq!(header_text(&w(Place::Header, Style::Simple, false), &Kind::One, &json!("a b,c")).as_deref(), Some("a b,c"));
        assert_eq!(cookie_pair(&w(Place::Cookie, Style::Form, false), &Kind::One, &json!("a b;c")).as_deref(), Some("color=a%20b%3Bc"));
        assert_eq!(path_text(&w(Place::Path, Style::Simple, false), &Kind::List, &json!([])), "");
        let mut b = Map::new();
        form_entries(&w(Place::Body, Style::Form, true), &Kind::List, &json!([1, null, 2]), &mut b);
        form_entries(&WireParam::plain("note", Place::Body, "note"), &Kind::One, &Value::Null, &mut b);
        form_entries(&WireParam { param: "meta".into(), place: Place::Body, name: "meta".into(), style: Style::DeepObject, explode: true }, &Kind::Record(vec!["a".into()]), &json!({ "a": 3 }), &mut b);
        assert_eq!(Value::Object(b), json!({ "color": ["1", "2"], "meta[a]": "3" }));
    }
}

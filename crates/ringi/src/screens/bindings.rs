//! What a screen sends, held to what it shows (DESIGN 13.3). From the root, every part the screen
//! reaches is read: a button's event says what it sends (`context`, `userMessage`), any other part
//! what it shows (every place of the data its properties read, but its checks). A place read in a
//! row's template is read in each row (`/form/items/*/price`). A value sent that no part shows is
//! E701; a number shown with other decimals than its field is sent in is W702.
//!
//! The functions of ritsu's catalog that make what a button sends are read by what they send, not
//! by what they read: `<kind>.given`, `.changes` and `.cleared` send the fields the requester gives
//! (from the place `form` names), `.edits` and `.edits_cleared` the fields an approver gives (from
//! `edit`); what they compare with (`before`) is not sent. `lastOf` sends the value of `key` of the
//! last row of `values` (the ID of a list's last ticket, `/tickets/*/id`), which the rows show. The
//! number and the revision of the ticket (`/ticket/id`, `/ticket/revision`) are the screen's own,
//! which say which ticket and which of its states the event is for, not what a person decided: they
//! are not held to the check.

use super::{basic_format, decimals, step};
use crate::r#gen::{Fld, Kind};
use ritsu_base::json::Json;
use std::collections::{BTreeMap, BTreeSet};

/// The places a button sends that are not a person's values.
pub const OWN: [&str; 2] = ["/ticket/id", "/ticket/revision"];

/// What the check found: the code, the place, and why (for W702).
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub code: &'static str,
    pub path: String,
    pub why: String,
}

/// The parts of a screen, by ID, as its `updateComponents` gives them.
pub fn parts(screen: &Json) -> BTreeMap<String, &Json> {
    let mut out = BTreeMap::new();
    for m in screen.get("messages").and_then(Json::as_arr).unwrap_or(&[]) {
        for c in m.get("updateComponents").and_then(|u| u.get("components")).and_then(Json::as_arr).unwrap_or(&[]) {
            if let Some(id) = c.get("id").and_then(Json::as_str) {
                out.insert(id.to_string(), c);
            }
        }
    }
    out
}

/// A place as the part at `scope` reads it: from the root as it is, else in the row.
fn resolve(p: &str, scope: &str) -> String {
    if p.starts_with('/') || scope.is_empty() { p.to_string() } else { format!("{scope}/{p}") }
}

/// Every place `j` reads, at `scope`.
fn read(j: &Json, scope: &str, out: &mut Vec<String>) {
    match j {
        Json::Obj(pairs) => {
            if let Some(p) = j.get("@path").and_then(Json::as_str) {
                out.push(resolve(p, scope));
                return;
            }
            for (_, v) in pairs {
                read(v, scope, out);
            }
        }
        Json::Arr(items) => items.iter().for_each(|v| read(v, scope, out)),
        _ => {}
    }
}

/// The fields of `k` a function of the kind's sends, from the place its argument `arg` names; and
/// the value of `key` in a row of `values`, which `lastOf` sends.
fn sent_by(k: &Kind, call: &Json, scope: &str, out: &mut Vec<String>) -> bool {
    let Some(name) = call.get("@call").and_then(Json::as_str) else { return false };
    if name == "lastOf" {
        let args = call.get("args");
        let (Some(at), Some(key)) = (args.and_then(|a| a.get("values")).and_then(|v| v.get("@path")).and_then(Json::as_str), args.and_then(|a| a.get("key")).and_then(Json::as_str)) else { return false };
        out.push(format!("{}/*/{key}", resolve(at, scope)));
        return true;
    }
    let Some(what) = name.strip_prefix(&format!("{}.", k.named.name)) else { return false };
    let (arg, fields): (&str, Vec<&Fld>) = match what {
        "given" | "changes" | "cleared" => ("form", k.given().collect()),
        "edits" | "edits_cleared" => ("edit", k.edited().collect()),
        _ => return false,
    };
    let Some(at) = call.get("args").and_then(|a| a.get(arg)).and_then(|a| a.get("@path")).and_then(Json::as_str) else { return false };
    let at = resolve(at, scope);
    out.extend(fields.iter().map(|f| format!("{at}/{}", f.named.name)));
    true
}

/// Every place a value sent reads, at `scope`.
fn sends(k: &Kind, j: &Json, scope: &str, out: &mut Vec<String>) {
    match j {
        Json::Obj(pairs) => {
            if j.get("@path").is_some() {
                read(j, scope, out);
                return;
            }
            if sent_by(k, j, scope, out) {
                return;
            }
            for (_, v) in pairs {
                sends(k, v, scope, out);
            }
        }
        Json::Arr(items) => items.iter().for_each(|v| sends(k, v, scope, out)),
        _ => {}
    }
}

/// What the screen shows and sends, and the numbers it shows or reads, each with how.
#[derive(Default)]
struct Read {
    shown: BTreeSet<String>,
    sent: BTreeSet<String>,
    numbers: Vec<(String, Counted)>,
}

/// How a part counts a number's integer on the wire: with `decimals` digits after the point, one
/// integer `step` of the last of them (an `AmountField`, ritsu's `formatSteps`); or as whole units,
/// the basic catalog's `formatCurrency` and `formatNumber`.
#[derive(Clone, Copy, PartialEq)]
struct Counted {
    decimals: i128,
    step: i128,
    basic: bool,
}

impl Counted {
    /// One integer as the part writes it: `0.25` (2 decimals, a step of 25), `1.00` (a basic
    /// format with 2 decimals).
    fn one(self) -> String {
        let d = self.decimals.clamp(0, 18) as usize;
        let s = if self.basic { 10i128.pow(d as u32) } else { self.step };
        let digits = format!("{:0>w$}", s, w = d + 1);
        let (whole, frac) = digits.split_at(digits.len() - d);
        if d == 0 { whole.to_string() } else { format!("{whole}.{frac}") }
    }
}

fn walk(k: &Kind, all: &BTreeMap<String, &Json>, id: &str, scope: &str, seen: &mut BTreeSet<(String, String)>, r: &mut Read) {
    if !seen.insert((id.to_string(), scope.to_string())) {
        return;
    }
    let Some(c) = all.get(id) else { return };
    let component = c.get("component").and_then(Json::as_str).unwrap_or_default();
    if component == "Button" {
        let mut sent = Vec::new();
        if let Some(e) = c.get("action").and_then(|a| a.get("event")) {
            for key in ["context", "userMessage"] {
                if let Some(v) = e.get(key) {
                    sends(k, v, scope, &mut sent);
                }
            }
        }
        r.sent.extend(sent);
    } else if let Json::Obj(pairs) = c {
        for (key, v) in pairs {
            if ["id", "component", "@catalogId", "@checks", "action", "children", "child"].contains(&key.as_str()) {
                continue;
            }
            let mut shown = Vec::new();
            read(v, scope, &mut shown);
            r.shown.extend(shown);
        }
        // the numbers shown or read, each with how it counts their integers
        let counted = |j: &Json, basic: bool| Counted { decimals: j.get("decimals").and_then(Json::as_int).unwrap_or(0), step: j.get("step").and_then(Json::as_int).unwrap_or(1), basic };
        if component == "AmountField"
            && let Some(p) = c.get("value").and_then(|v| v.get("@path")).and_then(Json::as_str)
        {
            r.numbers.push((resolve(p, scope), counted(c, false)));
        }
        let mut calls = Vec::new();
        number_calls(c, &mut calls);
        for (args, basic) in calls {
            if let Some(p) = args.get("value").and_then(|v| v.get("@path")).and_then(Json::as_str) {
                r.numbers.push((resolve(p, scope), counted(args, basic)));
            }
        }
    }
    let template = |j: &Json| -> Option<(String, String)> { Some((j.get("componentId")?.as_str()?.to_string(), j.get("path")?.as_str()?.to_string())) };
    match c.get("children") {
        Some(Json::Arr(ids)) => {
            for x in ids.iter().filter_map(Json::as_str) {
                walk(k, all, x, scope, seen, r);
            }
        }
        Some(t) => {
            if let Some((x, p)) = template(t) {
                walk(k, all, &x, &format!("{}/*", resolve(&p, scope)), seen, r);
            }
        }
        None => {}
    }
    if let Some(x) = c.get("child").and_then(Json::as_str) {
        // a row's template is read in each row of the place the rows are at
        let inner = match (component, c.get("value").and_then(|v| v.get("@path")).and_then(Json::as_str)) {
            ("LineItems", Some(p)) => format!("{}/*", resolve(p, scope)),
            _ => scope.to_string(),
        };
        walk(k, all, x, &inner, seen, r);
    }
}

/// The arguments of each call of `formatCurrency`, `formatNumber` and `formatSteps` in `j`, each
/// with whether it is one of the basic catalog's, which count an integer as whole units.
fn number_calls<'a>(j: &'a Json, out: &mut Vec<(&'a Json, bool)>) {
    match j {
        Json::Obj(pairs) => {
            let basic = match j.get("@call").and_then(Json::as_str) {
                Some("formatCurrency" | "formatNumber") => Some(true),
                Some("formatSteps") => Some(false),
                _ => None,
            };
            if let (Some(basic), Some(a)) = (basic, j.get("args")) {
                out.push((a, basic));
            }
            for (_, v) in pairs {
                number_calls(v, out);
            }
        }
        Json::Arr(items) => items.iter().for_each(|v| number_calls(v, out)),
        _ => {}
    }
}

/// The field of `k` a place of the data holds (`/form/items/*/price` is the price of a row of `items`).
fn field_at<'a>(k: &'a Kind, p: &str) -> Option<&'a Fld> {
    let rest = p.strip_prefix("/form/").or_else(|| p.strip_prefix("/edit/"))?;
    let mut parts = rest.split('/').filter(|s| *s != "*");
    let first = parts.next()?;
    let mut f = k.form.iter().find(|f| f.named.name == first)?;
    for name in parts {
        f = f.fields.iter().find(|c| c.named.name == name)?;
    }
    Some(f)
}

/// The check of the screen `j` of `k`.
pub fn check(k: &Kind, j: &Json) -> Vec<Found> {
    let all = parts(j);
    let mut r = Read::default();
    walk(k, &all, "root", "", &mut BTreeSet::new(), &mut r);
    let mut out = Vec::new();
    for p in r.sent.iter().filter(|p| !OWN.contains(&p.as_str())) {
        if !r.shown.contains(p) {
            out.push(Found { code: "E701", path: p.clone(), why: String::new() });
        }
    }
    for (p, d) in &r.numbers {
        let sent = r.sent.iter().any(|s| p == s || p.starts_with(&format!("{s}/")));
        if !sent {
            continue;
        }
        let Some(f) = field_at(k, p) else { continue };
        let want = Counted { decimals: i128::from(decimals(&f.ty)), step: step(&f.ty), basic: d.basic && basic_format(&f.ty) };
        if *d != want {
            out.push(Found { code: "W702", path: p.clone(), why: format!("{} / {}", d.one(), want.one()) });
        }
    }
    out
}

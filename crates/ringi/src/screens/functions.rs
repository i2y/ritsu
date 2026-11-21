//! `a2ui/functions.js` (DESIGN 13.2, 13.7): the functions of ritsu's catalog, as one ES module a
//! browser imports as it is, with no tool to build it. What a form works out (its totals and
//! computations, whether a field is shown and asked for) is each kind's `values.rule` as rulec
//! writes it in JavaScript: the TypeScript rulec writes for the server's side with its types taken
//! off (rulec's §15.36), the enums it reads inside it as for a page. Each module goes into a function
//! of its own here, so the kinds' names cannot meet. The checks of the fields (filled in, length in
//! code points, count of rows, range, a choice's value among its values) are read from a
//! description of each kind's form made from the same declarations as `types/<name>.json`, by one
//! piece of code for every kind, as the machine reads the form (DESIGN 13.7): a text without the
//! white space at its ends; a computation is held to its range as the glue holds it (DESIGN 4.4),
//! by the outputs the glue reads.
//!
//! Each output is worked out apart, from the fields it reads (DESIGN 13.7): the rule is called with
//! the form's values of what the output reads, through the computations it reads, and a value of
//! its range for every other argument, which cannot change it. It waits while a field it reads that
//! the requester has to fill in is empty, or while a value it reads is outside its range (that
//! field's check says so); a field that may be empty is read as empty, as the machine reads it
//! (DESIGN 4.9), and so is a field the form hides. It reads the rows there so far (a row whose field
//! it reads that the requester fills in is empty is not there yet), fewer than the field asks for
//! too: `values.rule` takes a total over any number of rows up to the most, from none, so a total
//! of the rows filled in so far is shown, and a condition reads it. It waits too while the rows there
//! are more than the field takes (the check of the rows' count says so), as the server's code gives
//! `values.rule` no more rows than the field takes: what the screens work out is what the server
//! works out (`worked`, DESIGN 12.4), on every form. While a condition waits, its
//! field is hidden and not asked for, and a computation that waits shows no value. What the form
//! works out is held to its range once the rows it reads are as many as the field asks for, and a
//! field of rows says it has too few only then (the buttons hold both before).

use super::{Given, Param};
use crate::ast::TypeExpr;
use crate::r#gen::{Fld, Kind};
use ritsu_base::json::Json;
use ritsu_base::text::Text;

/// The type of a field as the functions read it.
fn kind_of(f: &Fld) -> &'static str {
    match f.ty {
        TypeExpr::Text => "text",
        TypeExpr::LongText => "longtext",
        TypeExpr::Num(_) => "number",
        TypeExpr::Date => "date",
        TypeExpr::Choice => "choice",
        TypeExpr::ChoiceFrom(_) if !f.choices.is_empty() => "choice",
        TypeExpr::ChoiceFrom(_) => "master",
        TypeExpr::Attachment => "attachment",
        TypeExpr::Person => "person",
        TypeExpr::Team => "team",
        TypeExpr::Bool => "bool",
        TypeExpr::Rows => "rows",
    }
}

fn int(n: Option<i128>) -> Json {
    n.map(Json::int).unwrap_or(Json::Null)
}

/// A field as the functions read it. `required` is whether the requester fills it in wherever the
/// form shows it, as the machine asks for it (DESIGN 4.2, 4.3); `requiredWhen` the output that says
/// whether it is asked for on a condition (never for a field of the approvers', which the machine
/// asks for on no condition); `choices` the codes a choice takes (none for a master's rows, which
/// the server's data decide).
fn field(k: &Kind, f: &Fld, top: bool) -> Json {
    let output = |c: &Option<crate::r#gen::Cond>| c.as_ref().and_then(|c| k.screen.worked.as_ref()?.outputs.iter().find(|(n, _)| *n == c.worked).map(|(_, code)| Json::str(code))).unwrap_or(Json::Null);
    let date = matches!(f.ty, TypeExpr::Date);
    let end = |n: Option<i128>| match n {
        Some(d) if date => i64::try_from(d).map(|d| Json::str(crate::lex::date_text(d))).unwrap_or(Json::Null),
        n => int(n),
    };
    let edited = top && (f.approvers_only || k.approvers_edit.contains(&f.named.name));
    Json::obj([
        ("name", Json::str(&f.named.name)),
        ("type", Json::str(kind_of(f))),
        ("given", Json::from(f.given())),
        ("edited", Json::from(edited)),
        ("required", Json::from(f.asked(!top))),
        ("requiredWhen", output(&f.required_when)),
        ("shown", output(&f.shown_when)),
        ("choices", Json::arr(f.choices.iter().map(|c| Json::str(&c.name)))),
        ("min", end(f.min)),
        ("max", end(f.max)),
        ("countMin", int(f.count_min)),
        ("countMax", int(f.count_max)),
        ("worked", Json::from(f.worked_on_screen)),
        ("fields", Json::arr(f.fields.iter().map(|c| field(k, c, false)))),
    ])
}

/// What the functions know of a kind: its fields, and how its `values.rule` is called and read: its
/// arguments (`params`, each with what it is given while its field is empty: `none`, a value of its
/// range with whether the field is filled in (`standin`), or nothing (`wait`)), what each takes
/// where the output called does not read it (`standIns`), its outputs, what each output reads
/// (`reads`: each argument by its place, with the fields of a row it reads), the fields that may be
/// empty a computation has no value without (`valueWhen`), and the computations held to their
/// ranges (`held`, each with such fields as `when`).
fn spec(k: &Kind) -> Json {
    let worked = k.screen.worked.as_ref().map(|w| {
        let params = w.params.iter().map(|p| match p {
            Param::Value(f, named, empty) => {
                let named = if named.is_empty() { Json::Null } else { Json::obj(named.iter().map(|(v, l)| (v.as_str(), Json::str(l)))) };
                Json::arr([Json::str("value"), Json::str(f), named, Json::str(empty.word())])
            }
            Param::Given(f) => Json::arr([Json::str("given"), Json::str(f)]),
            Param::Rows(f, cols) => Json::arr([Json::str("rows"), Json::str(f), Json::arr(cols.iter().map(|(code, name, given)| Json::arr([Json::str(code), Json::str(name), Json::from(*given)])))]),
        });
        let reads = w.reads.iter().map(|(code, rs)| (code.as_str(), Json::arr(rs.iter().map(|(i, cols)| Json::arr([Json::int(*i as i128), Json::arr(cols.iter().map(Json::str))])))));
        let value_when = w.value_when.iter().map(|(code, fs)| (code.as_str(), Json::arr(fs.iter().map(Json::str))));
        let end = |e: &Option<(String, i128)>| e.as_ref().map(|(o, n)| Json::arr([Json::str(o), Json::int(*n)])).unwrap_or(Json::Null);
        let held = w.held.iter().map(|h| Json::obj([("code", Json::str(format!("{}.worked", h.whole()))), ("lo", end(&h.lo)), ("hi", end(&h.hi)), ("when", Json::arr(h.when.iter().map(Json::str)))]));
        Json::obj([
            ("params", Json::arr(params)),
            ("standIns", Json::arr(w.stand_ins.iter().cloned())),
            ("outputs", Json::obj(w.outputs.iter().map(|(n, c)| (n.as_str(), Json::str(c))))),
            ("reads", Json::obj(reads)),
            ("valueWhen", Json::obj(value_when)),
            ("held", Json::arr(held)),
        ])
    });
    Json::obj([
        ("fields", Json::arr(k.form.iter().map(|f| field(k, f, true)))),
        ("worked", worked.unwrap_or(Json::Null)),
        // the events whose functions the catalog names: making the request again, and an approver's edit
        ("resubmit", Json::from(k.has("resubmit"))),
        ("edit", Json::from(k.has("edit") && k.edited().next().is_some())),
    ])
}

/// A module rulec writes, as the body of a function that answers its function `name`: its
/// declarations without `export`, its lines that hand names on left out. Err when it imports
/// anything, which a page's module does not.
fn inside(js: &str, name: &str) -> Result<String, Text> {
    let mut body = String::new();
    for line in js.lines() {
        if line.starts_with("import ") {
            return Err(tr!("`{name}` のモジュールが、ほかのモジュールを読みます", "the module of `{name}` imports another module"));
        }
        if line.starts_with("export {") || line.starts_with("// Code generated by ") || line.starts_with("// Source: ") {
            continue;
        }
        let line = ["export const ", "export function ", "export class ", "export let "].iter().find(|k| line.starts_with(**k)).map(|_| &line["export ".len()..]).unwrap_or(line);
        if line.is_empty() {
            body.push('\n');
        } else {
            body.push_str("    ");
            body.push_str(line);
            body.push('\n');
        }
    }
    Ok(format!("(() => {{\n{}\n    return {name};\n  }})()", body.trim_end()))
}

/// The module.
pub fn module(kinds: &[Kind], given: &Given) -> Result<String, Text> {
    let mut rules = String::new();
    for k in kinds {
        let Some(w) = &k.screen.worked else { continue };
        let Some((_, js)) = given.rules.iter().find(|(n, _)| *n == k.named.name) else {
            let n = &k.named.name;
            return Err(tr!("`{n}` の values.rule の JavaScript がありません", "no JavaScript of the values.rule of `{n}`"));
        };
        rules.push_str(&format!("  {}: {},\n", Json::str(&k.named.name).compact(), inside(js, &w.function)?));
    }
    let kinds_json = Json::obj(kinds.iter().map(|k| (k.named.name.as_str(), spec(k))));
    Ok(format!(
        "// {head}\n// The functions of the screens' catalog (catalog.json beside this file): `functions` maps\n// each name the catalog gives to the function, called with the arguments of the call.\n\nexport const catalogId = {id};\n\n// What each kind's form works out: its values.rule, as rulec writes it in JavaScript.\nconst RULES = {{\n{rules}}};\n\n// Each kind's form, from its declarations.\nconst KINDS = {kinds};\n{RUNTIME}",
        head = crate::r#gen::head(),
        id = Json::str(&given.catalog).compact(),
        kinds = kinds_json.pretty(),
    ))
}

/// The functions themselves: the same for every kind, reading `RULES` and `KINDS`.
const RUNTIME: &str = r#"
function isEmpty(v) {
  return v === null || v === undefined || v === "" || (Array.isArray(v) && v.length === 0);
}

// The white space at the ends of a text or a comment: Unicode's White_Space, the 25 code points
// the ticket machine reads as white space too (a reason asked with a rejection or a send-back is
// refused when nothing else is written). JavaScript's own trim takes U+FEFF off and leaves U+0085.
const SPACE_ENDS = /^[\u0009-\u000D\u0020\u0085\u00A0\u1680\u2000-\u200A\u2028\u2029\u202F\u205F\u3000]+|[\u0009-\u000D\u0020\u0085\u00A0\u1680\u2000-\u200A\u2028\u2029\u202F\u205F\u3000]+$/g;

function trimmed(v) {
  return String(v).replace(SPACE_ENDS, "");
}

// The value a choice holds: a ChoicePicker writes a list of one.
function chosen(v) {
  const x = Array.isArray(v) ? v[0] : v;
  return x === undefined || x === null || x === "" ? null : String(x);
}

// The day number of a day written `YYYY-MM-DD` (days from 1970-01-01), as the rules count days;
// null when it is no day.
function dayNumber(s) {
  const m = /^([0-9]{4})-([0-9]{2})-([0-9]{2})$/.exec(String(s));
  if (!m) return null;
  const [y, mo, d] = [Number(m[1]), Number(m[2]), Number(m[3])];
  const leap = (y % 4 === 0 && y % 100 !== 0) || y % 400 === 0;
  const days = [31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
  if (mo < 1 || mo > 12 || d < 1 || d > days[mo - 1]) return null;
  const yy = mo <= 2 ? y - 1 : y;
  const era = Math.floor(yy / 400);
  const yoe = yy - era * 400;
  const doy = Math.floor((153 * (mo + (mo > 2 ? -3 : 9)) + 2) / 5) + d - 1;
  const doe = yoe * 365 + Math.floor(yoe / 4) - Math.floor(yoe / 100) + doy;
  return era * 146097 + doe - 719468;
}

// A field's value as the API takes it, null when it is empty; a value that is no value of its
// type is kept as it is, for its check to refuse. A text is sent without the white space at its
// ends, as a comment is (`SPACE_ENDS`): nothing left is nothing written. A number written in a
// text box is read without the same white space at its ends.
function wire(f, v) {
  switch (f.type) {
    case "rows":
      return Array.isArray(v) ? v.map((r) => rowOf(f, r)) : [];
    case "choice":
      return chosen(v);
    case "bool":
      return v === true;
  }
  if (isEmpty(v)) return null;
  if (f.type === "number") {
    if (typeof v === "number") return v;
    // the same white space as a text's (`SPACE_ENDS`), not JavaScript's own trim; nothing left is
    // nothing written, as of a text
    const t = trimmed(v);
    if (t === "") return null;
    return /^-?[0-9]+$/.test(t) ? Number(t) : t;
  }
  if (f.type === "date") return String(v).slice(0, 10);
  if (f.type === "text" || f.type === "longtext") {
    const t = trimmed(v);
    return t === "" ? null : t;
  }
  return String(v);
}

// A row as the API takes it: the fields the requester gives.
function rowOf(f, r) {
  const out = {};
  for (const c of f.fields.filter((c) => c.given)) {
    const x = wire(c, r ? r[c.name] : null);
    if (x !== null) out[c.name] = x;
  }
  return out;
}

function fieldOf(k, name) {
  return KINDS[k].fields.find((f) => f.name === name);
}

// A value as the rule takes it: a number and a day as an integer of the rule's, a value of a choice
// the rule writes under another name (`named`, from the value to that name) by that name.
function forRule(f, x, named) {
  if (x === null) return null;
  if (f.type === "number") return Number.isInteger(x) ? BigInt(x) : x;
  if (f.type === "date") {
    const d = dayNumber(x);
    return d === null ? x : BigInt(d);
  }
  return named && Object.prototype.hasOwnProperty.call(named, x) ? named[x] : x;
}

// What a field holds as the rules read it (DESIGN 4.9, 13.7): its value as the API takes it, or null
// for a field that is empty or that the form hides, which the machine is never given. A field shown
// on a condition is hidden while its condition cannot be worked out. `seen` holds the fields whose
// condition is being worked out, so that none is worked out from itself.
function held(k, f, form, seen) {
  if (f.shown && !shownIn(k, f, form, seen)) return null;
  return wire(f, form[f.name]);
}

// What the rule is given for the argument `p` where the output called does not read it: the value
// of its range the description gives (`stand`), which cannot change the output.
function standIn(k, p, stand) {
  if (p[0] === "given") return false;
  const f = fieldOf(k, p[1]);
  if (p[0] === "value") return stand === null ? null : forRule(f, stand, p[2]);
  const cells = Object.fromEntries(p[2].map(([code, name, given]) => [code, given ? false : forRule(f.fields.find((c) => c.name === name), stand.cells[code])]));
  return Array.from({ length: stand.count }, () => Object.assign({}, cells));
}

// The rows the rule is given for an output that reads the fields `cols` of them (by the rule's
// names): each row of the form with the value of each, a row whose field the requester has to fill
// in is empty left out (it is not there yet); a field that may be empty is given with whether it
// is filled in, and a field the output does not read as `stand` holds it. null while the rows there
// are more than the field takes: the rule is given no more, and the output waits (the check of the
// rows' count says so).
function rowsFor(k, p, stand, cols, rows) {
  const f = fieldOf(k, p[1]);
  const out = [];
  for (const r of rows) {
    const row = {};
    let there = true;
    for (const [code, name, given] of p[2]) {
      const c = f.fields.find((x) => x.name === name);
      const x = r[name] === undefined ? null : r[name];
      if (!cols.includes(code)) row[code] = given ? false : forRule(c, stand.cells[code]);
      else if (given) row[code] = x !== null;
      else if (x !== null) row[code] = forRule(c, x);
      else if (p[2].some(([, n, g]) => g && n === name)) row[code] = forRule(c, stand.cells[code]);
      else there = false;
    }
    if (there) out.push(row);
  }
  return f.countMax !== null && out.length > f.countMax ? null : out;
}

// The output `code` of the kind's values.rule for the form, worked out from what it reads alone
// (DESIGN 13.7): the rule is given the form's value of each argument the output reads, the rows of
// a field there so far (a row is not there while a field of it the output reads and the requester
// fills in is empty), and a value of its range of every other. undefined while it cannot be worked
// out: a field it reads that the requester has to fill in is empty, a value it reads is outside its
// range or no value of its type, or the rows it reads are more than their field takes, which that
// field's check says.
function outputOf(k, form, code, seen) {
  const w = KINDS[k].worked;
  if (!w || !RULES[k]) return undefined;
  const reads = new Map((w.reads[code] || []).map(([i, cols]) => [i, cols]));
  const args = [];
  for (let i = 0; i < w.params.length; i++) {
    const p = w.params[i];
    const stand = w.standIns[i];
    if (!reads.has(i)) {
      args.push(standIn(k, p, stand));
      continue;
    }
    const f = fieldOf(k, p[1]);
    const x = held(k, f, form, seen);
    if (p[0] === "given") {
      args.push(x !== null);
    } else if (p[0] === "rows") {
      // the rows there so far, fewer than the field asks for too: the rule works them out from none;
      // more than it takes, and the output waits
      const rows = rowsFor(k, p, stand, reads.get(i), x === null ? [] : x);
      if (rows === null) return undefined;
      args.push(rows);
    } else if (x !== null) {
      args.push(forRule(f, x, p[2]));
    } else if (p[3] === "wait") {
      return undefined;
    } else {
      args.push(p[3] === "none" ? null : forRule(f, stand, p[2]));
    }
  }
  try {
    const got = RULES[k](...args);
    // a rule of one output answers with it alone
    return Object.keys(w.outputs).length === 1 ? got : got[code];
  } catch (e) {
    return undefined;
  }
}

// The value of a computation the screens work out: none while a field that may be empty it reads is
// empty or hidden (DESIGN 4.9), or while it cannot be worked out.
function valueOf(k, form, code) {
  const w = KINDS[k].worked;
  if (!w || (w.valueWhen[code] || []).some((n) => held(k, fieldOf(k, n), form, []) === null)) return null;
  const v = outputOf(k, form, code, []);
  return v === null || v === undefined ? null : v;
}

function shownIn(k, f, form, seen) {
  if (!f.shown) return true;
  if (seen.includes(f.name)) return false;
  return outputOf(k, form, f.shown, seen.concat([f.name])) === true;
}

// Whether a field is shown: one shown on a condition while the condition holds, and hidden while it
// cannot be worked out.
function isShown(k, f, form) {
  return shownIn(k, f, form, []);
}

// Whether the form asks for a field it shows: always, or while its condition holds (not while it
// cannot be worked out). The machine asks for no field of the approvers'.
function isRequired(k, f, form) {
  if (!f.given) return false;
  if (f.required) return true;
  if (!f.requiredWhen) return false;
  return outputOf(k, form, f.requiredWhen, []) === true;
}

// Whether the rows of the field `f` an output reads, those there so far (`rowsFor`), are fewer than
// it asks for, of any field of rows the output `code` reads: what is worked out of them is not held
// to its range until they are as many.
function short(k, form, code) {
  const w = KINDS[k].worked;
  return (w.reads[code] || []).some(([i, cols]) => {
    const p = w.params[i];
    if (p[0] !== "rows") return false;
    const f = fieldOf(k, p[1]);
    const x = held(k, f, form, []);
    const rows = rowsFor(k, p, w.standIns[i], cols, x === null ? [] : x);
    return rows !== null && rows.length < (f.countMin || 0);
  });
}

// Whether a value passes a check of its field (`required`, `count`, `range`, `choice`). With
// `filling`, the check a field of rows shows below itself while it is filled in: fewer rows than
// it asks for pass, which the buttons hold.
function passes(k, f, v, what, form, filling) {
  const x = wire(f, v);
  switch (what) {
    case "required":
      return !(form ? isRequired(k, f, form) : f.required) || !isEmpty(x);
    case "choice":
      return x === null || f.choices.includes(x);
    case "count": {
      if (x === null) return true;
      const n = f.type === "rows" ? x.length : [...x].length;
      const least = filling && f.type === "rows" ? null : f.countMin;
      return (least === null || n >= least) && (f.countMax === null || n <= f.countMax);
    }
    case "range": {
      if (x === null) return true;
      if (f.type === "date") {
        if (dayNumber(x) === null) return false;
        return (f.min === null || x >= f.min) && (f.max === null || x <= f.max);
      }
      // a number past 2^53 is not the one written: a screen holds no such number as it is
      if (!Number.isSafeInteger(x)) return false;
      return (f.min === null || x >= f.min) && (f.max === null || x <= f.max);
    }
  }
  return false;
}

// Whether the form passes the check `code`: `<field>.<what>`, `<rows>.<field>.<what>` (for the
// row `row`, or every row), or `<field>.worked` for a computation held to its range. `filling` is
// the check a field shows below itself while the form is filled in (`passes`).
function check(k, form, code, row, filling) {
  const parts = String(code).split(".");
  const what = parts.pop();
  if (what === "worked") {
    // held where it has a value and can be worked out, and the rows it reads are as many as their
    // field asks for
    const h = (KINDS[k].worked ? KINDS[k].worked.held : []).find((h) => h.code === code);
    if (!h || h.when.some((n) => held(k, fieldOf(k, n), form, []) === null)) return true;
    if ([h.lo, h.hi].some((end) => end && short(k, form, end[0]))) return true;
    const past = (end, beyond) => {
      if (!end) return false;
      const v = outputOf(k, form, end[0], []);
      return v !== undefined && v !== null && beyond(Number(v), end[1]);
    };
    return !past(h.lo, (v, e) => v < e) && !past(h.hi, (v, e) => v > e);
  }
  const f = fieldOf(k, parts[0]);
  if (!f) return false;
  if (!isShown(k, f, form)) return true;
  if (parts.length === 1) return passes(k, f, form[f.name], what, form, filling === true);
  const c = f.fields.find((c) => c.name === parts[1]);
  if (!c) return false;
  const rows = Array.isArray(form[f.name]) ? form[f.name] : [];
  const which = row === undefined || row === null ? rows : rows.slice(row, row + 1);
  return which.every((r) => passes(k, c, r ? r[c.name] : null, what, null));
}

function same(a, b) {
  return JSON.stringify(a) === JSON.stringify(b);
}

// The form as it is sent to make the request: each field given and shown that is not empty.
function given(k, form) {
  const out = {};
  for (const f of KINDS[k].fields.filter((f) => f.given)) {
    if (!isShown(k, f, form)) continue;
    const x = wire(f, form[f.name]);
    if (x !== null) out[f.name] = x;
  }
  return out;
}

// The fields changed from `before`, when the request is made again.
function changes(k, form, before) {
  const out = {};
  for (const f of KINDS[k].fields.filter((f) => f.given)) {
    if (!isShown(k, f, form)) continue;
    const x = wire(f, form[f.name]);
    if (x !== null && !same(x, wire(f, (before || {})[f.name]))) out[f.name] = x;
  }
  return out;
}

// Whether a field that held a value is made empty: hidden, or emptied. Rows are never emptied, for
// they are sent as a list, with no row in it.
function emptied(k, f, v, form) {
  return !isShown(k, f, form) || (f.type !== "rows" && isEmpty(wire(f, v)));
}

// The fields `before` held that are empty, or hidden, now. One the form asks for is never sent
// empty: its check stops the button first.
function cleared(k, form, before) {
  return KINDS[k].fields
    .filter((f) => f.given && !isEmpty(wire(f, (before || {})[f.name])))
    .filter((f) => emptied(k, f, form[f.name], form))
    .map((f) => f.name);
}

// The form as an approver's edit leaves it: each field they give or correct as `edit` holds it
// (the server fills `edit` with the ticket's values of them), every other as `before` does.
function edited(k, edit, before) {
  const out = Object.assign({}, before || {});
  for (const f of KINDS[k].fields.filter((f) => f.edited)) {
    const v = (edit || {})[f.name];
    out[f.name] = v === undefined ? null : v;
  }
  return out;
}

// The fields an approver gave or changed from `before`, of those the form shows after the edit.
function edits(k, edit, before) {
  const after = edited(k, edit, before);
  const out = {};
  for (const f of KINDS[k].fields.filter((f) => f.edited)) {
    if (!isShown(k, f, after)) continue;
    const x = wire(f, (edit || {})[f.name]);
    if (x !== null && !same(x, wire(f, (before || {})[f.name]))) out[f.name] = x;
  }
  return out;
}

// The fields an approver gives or corrects that `before` held and that are empty, or hidden, after
// the edit.
function editsCleared(k, edit, before) {
  const after = edited(k, edit, before);
  return KINDS[k].fields
    .filter((f) => f.edited && !isEmpty(wire(f, (before || {})[f.name])))
    .filter((f) => emptied(k, f, after[f.name], after))
    .map((f) => f.name);
}

// A comment as it is sent: without the white space at its ends (`SPACE_ENDS`), null when nothing
// is left.
function commentOf(text) {
  const t = trimmed(text === null || text === undefined ? "" : text);
  return t === "" ? null : t;
}

export const functions = {
  lookup({ value, table, separator }) {
    const t = table !== null && typeof table === "object" && !Array.isArray(table) ? table : {};
    const one = (v) => {
      const key = String(v);
      return Object.prototype.hasOwnProperty.call(t, key) ? String(t[key]) : key;
    };
    if (Array.isArray(value)) return value.map(one).join(separator === undefined || separator === null ? ", " : separator);
    return value === null || value === undefined ? "" : one(value);
  },
  equals({ a, b }) {
    return same(a, b);
  },
  formatSteps({ value, decimals, step, unit }) {
    if (value === null || value === undefined || value === "") return "";
    const n = Number(value);
    if (!Number.isSafeInteger(n)) return String(value);
    const d = Number.isInteger(decimals) && decimals > 0 ? decimals : 0;
    const v = BigInt(n) * BigInt(Number.isSafeInteger(step) && step > 0 ? step : 1);
    const digits = (v < 0n ? -v : v).toString().padStart(d + 1, "0");
    let whole = digits.slice(0, digits.length - d);
    if (whole.length > 4) whole = whole.replace(/\B(?=(\d{3})+$)/g, ",");
    const point = d > 0 ? "." + digits.slice(digits.length - d) : "";
    return (v < 0n ? "-" : "") + whole + point + (typeof unit === "string" ? unit : "");
  },
  numberFrom({ text }) {
    // without Unicode's White_Space at its ends, as a text is read (`trimmed`): U+0085 is white
    // space, U+FEFF is not
    const t = trimmed(text === null || text === undefined ? "" : text);
    return /^-?[0-9]+$/.test(t) ? Number(t) : null;
  },
  one({ values }) {
    return chosen(values);
  },
  comment({ text }) {
    return commentOf(text);
  },
  commentFits({ text }) {
    const t = commentOf(text);
    return t === null || [...t].length <= 2000;
  },
  commentGiven({ text }) {
    return commentOf(text) !== null;
  },
  lastOf({ values, key }) {
    if (!Array.isArray(values) || values.length === 0) return null;
    const x = values[values.length - 1];
    return x !== null && typeof x === "object" && Object.prototype.hasOwnProperty.call(x, key) ? x[key] : null;
  },
};

for (const [k, spec] of Object.entries(KINDS)) {
  functions[`${k}.check`] = ({ form, code, row, filling }) => check(k, form || {}, code, row, filling);
  functions[`${k}.given`] = ({ form }) => given(k, form || {});
  if (spec.resubmit) {
    functions[`${k}.changes`] = ({ form, before }) => changes(k, form || {}, before);
    functions[`${k}.cleared`] = ({ form, before }) => cleared(k, form || {}, before);
  }
  if (spec.edit) {
    functions[`${k}.edited`] = ({ edit, before }) => edited(k, edit, before);
    functions[`${k}.edits`] = ({ edit, before }) => edits(k, edit, before);
    functions[`${k}.edits_cleared`] = ({ edit, before }) => editsCleared(k, edit, before);
  }
  for (const f of spec.fields) {
    if (f.shown) functions[`${k}.shown_${f.name}`] = ({ form }) => isShown(k, f, form || {});
    if (f.requiredWhen) functions[`${k}.required_${f.name}`] = ({ form }) => isRequired(k, f, form || {});
    if (f.worked) {
      functions[`${k}.${f.name}`] = ({ form }) => {
        const v = valueOf(k, form || {}, spec.worked.outputs[f.name]);
        return v === null ? null : Number(v);
      };
    }
  }
}
"#;

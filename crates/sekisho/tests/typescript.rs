//! The TypeScript `sekisho gen --target typescript` writes (DESIGN 5.3, 6.3), held to the
//! reference evaluation. For every `.gate` of the examples and the tests that passes its check and
//! has an action, the module is generated (as `ritsu sekisho gen` generates it, in English and in
//! Japanese, and with `--authorizer avp`), beside the TypeScript rulec and koyomi generate for the
//! rules, the dates and the calendars it reads (their own generators, called here, as `ritsu gen`
//! calls them):
//!
//! - `tsc --strict` (TypeScript 5.9.3, `tools/runner-ts`) passes every module, the code of
//!   `--authorizer avp` with the AWS SDK's client of Verified Permissions (it is not run: that
//!   needs an account);
//! - the data of every combination the check walks (`src/raw.rs`: both ends of each cell, the
//!   inputs rulec gives for a rule's values, the days where a date's truth turns), and of each
//!   fault the code is to refuse, is handed to the code, which asks Cedar's own WebAssembly build
//!   (`@cedar-policy/cedar-wasm` 4.13.0) in Node; every answer — allowed or not, the determining
//!   policies as a set, the kind of a refusal, the context given to Cedar — is the reference
//!   evaluation's of the same data.
//!
//!
//! A gate whose types and enum are named as the module's own types (`Store`, `Request`, `Value`)
//! and as a global (`Date`) is held to the same, with the names the module gives them: the gate's
//! keep theirs, and the module's own take `_2`, as in the Python and the Go (DESIGN 5.3).
//!
//! Without Node or the runner (`npm ci --prefix tools/runner-ts`), the tests say SKIP and pass.

mod common;

use ritsu_base::text::Lang;
use ritsu_testkit::{Need, TempDir};
use sekisho::r#gen::{Authorizer, typescript};
use sekisho::raw;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// The longest a run of tsc or of Node may take.
const LIMIT: Duration = Duration::from_secs(600);

fn runner() -> PathBuf {
    std::fs::canonicalize("tools/runner-ts").unwrap_or_else(|_| PathBuf::from("tools/runner-ts"))
}

/// tsc and Node, or None with the SKIP line saying why.
fn tools() -> Option<PathBuf> {
    if !ritsu_testkit::need(Need::Node) {
        return None;
    }
    let tsc = runner().join("node_modules/.bin/tsc");
    let wasm = runner().join("node_modules/@cedar-policy/cedar-wasm/nodejs/cedar_wasm.js");
    if !ritsu_testkit::tools::runs("node", &["--version"]) {
        ritsu_testkit::skip("node is not on the PATH; the TypeScript sekisho generates is not run");
        return None;
    }
    if !tsc.exists() || !wasm.exists() {
        ritsu_testkit::skip("the runner of the TypeScript is not installed (npm ci --prefix tools/runner-ts); the TypeScript sekisho generates is not checked");
        return None;
    }
    Some(tsc)
}

/// Every `.gate` under the examples and the tests, in order.
fn gates() -> Vec<String> {
    fn walk(d: &Path, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "gate") {
                out.push(p.to_string_lossy().to_string());
            }
        }
    }
    let mut out = Vec::new();
    walk(Path::new("examples"), &mut out);
    walk(Path::new("tests"), &mut out);
    out.sort();
    out
}

/// The TypeScript of the rules, the dates and the calendars a gate reads, as `ritsu gen` writes
/// them into a package: `rules/<module>.ts` and `dates/<alias>.ts`.
fn languages(g: &sekisho::model::Gate, lang: Lang) -> Vec<(String, String)> {
    use sekisho::model::UseKind;
    let mut out = Vec::new();
    let rulec_lang = if lang == Lang::Ja { rulec::i18n::Lang::Ja } else { rulec::i18n::Lang::En };
    let mut loader = koyomi::calendar::Loader::default();
    for u in &g.uses {
        let path = u.file.to_string_lossy().to_string();
        match u.kind {
            UseKind::Rule => {
                let src = std::fs::read_to_string(&u.file).unwrap();
                let (rel, body) = rulec::i18n::with(rulec_lang, || rulec::codegen::package_module(&src, &path, &u.path, "typescript")).unwrap();
                out.push((format!("rules/{rel}"), body));
            }
            UseKind::Dates | UseKind::Calendar => {
                let o = koyomi::check::check_file(&path, &koyomi::check::Options::default(), &mut loader).unwrap();
                let checked = o.checked.as_ref().expect("the dates file passes koyomi's check");
                let unit = koyomi::codegen::unit_shown(checked, lang, Some(&u.path));
                out.push((format!("dates/{}.ts", unit.alias), koyomi::codegen::typescript::module(&unit)));
            }
            _ => {}
        }
    }
    out
}

/// The harness of a gate: it reads the data, builds a store of each case's entities, hands the case
/// to the generated code, and prints its answer, a JSON line a case. `names` are the module's.
fn harness(g: &sekisho::model::Gate, module: &str, names: &typescript::Names) -> String {
    let mut fields = Vec::new();
    for t in &g.types {
        for f in &t.attrs {
            let name = typescript::field(&f.named.alias);
            if name != f.named.alias {
                fields.push(format!("{}: {}", q(&f.named.alias), q(&name)));
            }
        }
    }
    let methods: Vec<String> = g.types.iter().map(|t| format!("    {}: async (id: string) => find({}, id),", typescript::store_method(&t.named.alias), q(&t.named.alias))).collect();
    let (store, principal, answer) = (&names.store, &names.principal, &names.answer);
    let actions: Vec<String> = g.actions.iter().map(|a| format!("  {}: (s, p, i, n) => authz.{}(s, p as authz.{principal}, i as never, n),", q(&a.named.alias), typescript::authorize_fn(&a.named.alias))).collect();
    format!(
        r#"// The harness of sekisho's tests/typescript.rs: each case of the data, through the generated code.
import {{ readFileSync }} from "node:fs";
import * as authz from "./authz/{module}";

type Raw = {{ type: string; id: string; roles: string[]; member_of: {{ type: string; id: string }}[]; attrs: Record<string, unknown> }};
type Kinds = {{ roles: string[]; member_of: string[]; attrs: Record<string, string> }};
type Case = {{ name: string; action: string; principal: {{ type: string; id: string }}; store: Raw[]; input: Record<string, unknown>; now: string }};
const data = JSON.parse(readFileSync(process.argv[2], "utf8")) as {{ types: Record<string, Kinds>; actions: Record<string, {{ inputs: Record<string, string> }}>; cases: Case[] }};
const FIELDS: Record<string, string> = {{ {fields} }};

function value(kind: string | undefined, v: unknown): unknown {{
  return kind === "number" && typeof v === "number" ? BigInt(v) : v;
}}

function store(entities: Raw[]): authz.{store} {{
  const find = (type: string, id: string): Record<string, unknown> | undefined => {{
    const e = entities.find((x) => x.type === type && x.id === id);
    if (e === undefined) return undefined;
    const k = data.types[type];
    const out: Record<string, unknown> = {{}};
    if (k.roles.length > 0) out.roles = e.roles;
    if (k.member_of.length > 0) out.member_of = e.member_of;
    for (const [n, v] of Object.entries(e.attrs)) out[FIELDS[n] ?? n] = value(k.attrs[n], v);
    return out;
  }};
  return {{
{methods}
  }} as unknown as authz.{store};
}}

const ACTIONS: Record<string, (s: authz.{store}, p: unknown, i: unknown, n: Date) => Promise<authz.{answer}>> = {{
{actions}
}};

async function main(): Promise<void> {{
  for (const c of data.cases) {{
    const kinds = data.actions[c.action].inputs;
    const input: Record<string, unknown> = {{}};
    for (const [n, v] of Object.entries(c.input)) input[n] = value(kinds[n], v);
    const a = await ACTIONS[c.action](store(c.store), c.principal, input, new Date(c.now));
    console.log(JSON.stringify({{ name: c.name, allowed: a.allowed, policies: a.policies, error: a.error ? a.error.kind : null, context: a.error ? null : (a.context ?? null) }}));
  }}
}}

main().catch((e: unknown) => {{
  console.error(e);
  process.exit(1);
}});
"#,
        fields = fields.join(", "),
        methods = methods.join("\n"),
        actions = actions.join("\n"),
    )
}

fn q(s: &str) -> String {
    ritsu_base::json::quote(s)
}

/// One gate's directory: its module in English (run), in Japanese and for Verified Permissions
/// (type-checked), what it reads, the harness and the data.
struct Made {
    gate: String,
    dir: String,
    raw: raw::Raw,
}

fn make(root: &Path, n: usize, gate: &str, suite: &sekisho::suite::Suite) -> Option<Made> {
    let o = common::check(gate);
    if o.has_errors() {
        return None;
    }
    let (scope, checked) = (o.scope.as_ref()?, o.walked.as_ref()?);
    let g = &checked.gate;
    if g.actions.is_empty() {
        return None;
    }
    let r = raw::raw(scope, checked, suite);
    let dir = format!("g{n}");
    let shown = ritsu_emit::header::file_name(gate);
    let write = |rel: &str, body: &str| {
        let p = root.join(&dir).join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    };
    for (rel, body) in languages(g, Lang::En) {
        write(&rel, &body);
    }
    let alias = g.named.alias.clone();
    for (sub, lang, authorizer) in [("authz", Lang::En, Authorizer::Cedar), ("authz_ja", Lang::Ja, Authorizer::Cedar), ("authz_avp", Lang::En, Authorizer::Avp)] {
        let text = typescript::module(scope, checked, suite, &shown, lang, authorizer).unwrap_or_else(|e| panic!("{gate}: {e}"));
        write(&format!("{sub}/{alias}.ts"), &text);
    }
    let names = typescript::names(g, &sekisho::cedar::shape(g, scope, checked));
    write("harness.ts", &harness(g, &alias, &names));
    write("raw.json", &r.json(g));
    Some(Made { gate: gate.to_string(), dir, raw: r })
}

/// Type-checks the modules of the gates `made` wrote under `root` as one program (`tsc --strict`;
/// `rootDir` keeps each gate's directory under `out/`, one gate or many), then hands each gate's
/// data to its code, run in Node against cedar-wasm, and holds every answer to the reference's:
/// the number of cases.
fn type_check_and_run(root: &Path, made: &[Made], tsc: &Path) -> usize {
    std::os::unix::fs::symlink(runner().join("node_modules"), root.join("node_modules")).unwrap();
    std::fs::write(root.join("package.json"), "{ \"private\": true }\n").unwrap();
    let tsconfig = format!(
        "{{\n  \"compilerOptions\": {{\n    \"strict\": true,\n    \"module\": \"nodenext\",\n    \"moduleResolution\": \"nodenext\",\n    \"target\": \"es2022\",\n    \"rootDir\": \".\",\n    \"outDir\": \"out\",\n    \"types\": [\"node\"],\n    \"typeRoots\": [{}],\n    \"skipLibCheck\": true\n  }},\n  \"include\": [\"g*/**/*.ts\"]\n}}\n",
        q(&runner().join("node_modules/@types").to_string_lossy())
    );
    std::fs::write(root.join("tsconfig.json"), tsconfig).unwrap();
    let started = std::time::Instant::now();
    let r = ritsu_testkit::run(Command::new(tsc).arg("-p").arg(root.join("tsconfig.json")).current_dir(root), LIMIT);
    assert!(r.ok, "tsc --strict finds fault with the generated TypeScript:\n{}", r.both());
    let gates = if made.len() == 1 { "1 gate".to_string() } else { format!("{} gates", made.len()) };
    println!("tsc --strict ({}) passes the modules of {gates}, in English, in Japanese and for Verified Permissions ({:.1} s)", ritsu_testkit::tools::version(tsc, "--version"), started.elapsed().as_secs_f64());
    let mut wrong = Vec::new();
    let mut cases = 0;
    for m in made {
        let started = std::time::Instant::now();
        let out = root.join("out").join(&m.dir);
        let r = ritsu_testkit::run(Command::new("node").arg(out.join("harness.js")).arg(root.join(&m.dir).join("raw.json")).current_dir(&out), LIMIT);
        if !r.ok {
            wrong.push(format!("{}: node failed:\n{}", m.gate, r.both()));
            continue;
        }
        match m.raw.compare(&r.stdout) {
            Ok(n) => {
                cases += n;
                let (refused, allowed) = m.raw.counts();
                println!("compared {}: {n} cases ({allowed} allowed, {refused} refused) agree with the reference ({:.1} s)", m.gate, started.elapsed().as_secs_f64());
            }
            Err(e) => wrong.push(format!("{}: {e}", m.gate)),
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n\n"));
    cases
}

#[test]
fn the_generated_typescript_answers_as_the_reference_does() {
    let Some(tsc) = tools() else { return };
    let t = TempDir::new("typescript");
    let root = t.path();
    let suite = common::joined();
    let made: Vec<Made> = gates().iter().enumerate().filter_map(|(n, g)| make(root, n, g, &suite)).collect();
    assert!(made.len() >= 15, "{} gates made", made.len());
    let cases = type_check_and_run(root, &made, &tsc);
    println!("the TypeScript of {} gates answers {cases} cases as the reference does, through cedar-wasm", made.len());
}

/// A gate whose types and enum take the names of the module's own types, and of a global of
/// JavaScript the module calls (`Date`).
const CLASH: &str = r#"gate clash v1
description "Types and an enum named as the generated code's own types: the gate's keep their names"
namespace Clash

enum value = low | high

role clerk
  description "Answers requests"
  can reply

principal Store
  description "A shop, signed in as itself"
  roles clerk
  attributes
    open : bool

principal Date
  description "A date of the calendar of events, signed in as itself"
  attributes
    kept : bool

resource Request
  description "A request to the shop"
  attributes
    priority : value

action reply
  description "Reply to a request"
  principal Store, Date
  resource Request

permit clerks_reply
  description "A clerk replies to a request"
  principal in clerk
  action reply

forbid closed_stores_do_nothing
  description "A shop that is closed does nothing"
  principal is Store
  action any
  unless principal.open

permit kept_dates_reply_to_high
  description "A kept date replies to a request of high priority"
  principal is Date
  action reply
  when principal.kept
  when resource.priority is high
"#;

#[test]
fn the_gates_names_are_kept_and_the_modules_own_take_2() {
    let Some(tsc) = tools() else { return };
    let t = TempDir::new("typescript-names");
    let root = t.path();
    let gate = root.join("clash.gate");
    std::fs::write(&gate, CLASH).unwrap();
    let suite = common::joined();
    let m = make(root, 0, &gate.to_string_lossy(), &suite).expect("clash.gate passes its check and has an action");
    let text = std::fs::read_to_string(root.join("g0/authz/clash.ts")).unwrap();
    for line in [
        // the gate's types and enum, by their own names (Date is a global the module calls)
        "export interface Store {",
        "export interface Request {",
        "export interface Date_2 {",
        "export type Value = \"low\" | \"high\";",
        // the module's own
        "export interface Store_2 {",
        "export interface Request_2 {",
        "export type Value_2 = boolean | number | string | { __entity: Uid };",
        "export async function replyRequest(store: Store_2, principal: Principal, input: ReplyInput, now: Date = new Date()): Promise<Request_2> {",
        "export async function authorizeReply(store: Store_2, principal: Principal, input: ReplyInput, now: Date = new Date()): Promise<Answer> {",
    ] {
        assert!(text.lines().any(|l| l == line), "the module has no line {line}:\n{text}");
    }
    let cases = type_check_and_run(root, &[m], &tsc);
    println!("the TypeScript of clash.gate, its names moved aside, answers {cases} cases as the reference does");
}

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
//!   evaluation's of the same data. Each case is handed over twice: with what may be absent left
//!   out, as the data has it, and given as null, which the code reads as absent too (the Python and
//!   the Go read both as one: their records hold `None` and `nil`);
//! - the entry by the action's name (`authorize`, DESIGN 5.7) is given each case too, the
//!   resource's id under its type and an id of every other type the gate's actions take beside it,
//!   and answers as the action's own function does (an action with inputs of its own, refused as
//!   `input`); a name the gate has no action of is refused as `action`, and no id, or ids of two of
//!   an action's types, as `resource`.
//!
//! A gate W402 names for TypeScript (tests/spellings.rs) is left out: `gen` stops at it.
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
                // not into another module of ritsu's, as the commands' walks go (ritsu's DESIGN 17.6): its
                // gates read their dependencies' files, and ritsu's `tests/modules_gen.rs` generates them
                if ritsu_base::paths::not_entered(&p).is_none() {
                    walk(&p, out);
                }
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
/// them into a package: `rules/<module>.ts`, `dates/<alias>.ts`, and `units.ts` with the types of
/// the units the rules take amounts in.
fn languages(g: &sekisho::model::Gate, lang: Lang) -> Vec<(String, String)> {
    use sekisho::model::UseKind;
    let mut out = Vec::new();
    let mut units = Vec::new();
    let rulec_lang = if lang == Lang::Ja { rulec::i18n::Lang::Ja } else { rulec::i18n::Lang::En };
    let mut loader = koyomi::calendar::Loader::default();
    for u in &g.uses {
        let path = u.file.to_string_lossy().to_string();
        match u.kind {
            UseKind::Rule => {
                let src = std::fs::read_to_string(&u.file).unwrap();
                let (rel, body, used) = rulec::i18n::with(rulec_lang, || rulec::codegen::package_parts(&src, &path, &u.path, "typescript", "")).unwrap();
                units.extend(used);
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
    out.extend(rulec::codegen::units_module("typescript", &units));
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
    // what may be absent, which the second run gives as null
    let quoted = |fs: Vec<&str>| fs.iter().map(|f| q(f)).collect::<Vec<_>>().join(", ");
    let nullable: Vec<String> = g.types.iter().map(|t| format!("{}: [{}]", q(&t.named.alias), quoted(t.attrs.iter().filter(|f| f.optional).map(|f| f.named.alias.as_str()).collect()))).collect();
    let nullable_inputs: Vec<String> = g.actions.iter().map(|a| format!("{}: [{}]", q(&a.named.alias), quoted(a.inputs.iter().filter(|f| f.optional).map(|f| f.named.alias.as_str()).collect()))).collect();
    let (store, principal, answer, roles, roles_of) = (&names.store, &names.principal, &names.answer, &names.roles, &names.roles_of);
    // the names rolesOf is given: the gate's types, and one it does not declare
    let type_names: Vec<String> = g.types.iter().map(|t| q(&t.named.alias)).chain([q("no such type")]).collect();
    let type_names = type_names.join(", ");
    let actions: Vec<String> = g.actions.iter().map(|a| format!("  {}: (s, p, i, n) => authz.{}(s, p as authz.{principal}, i as never, n),", q(&a.named.alias), typescript::authorize_fn(&a.named.alias))).collect();
    // for the entry by the action's name: the types each action takes, the other types the gate's
    // actions take, and the actions with inputs of their own
    let every: std::collections::BTreeSet<usize> = g.actions.iter().flat_map(|a| a.resources.iter().copied()).collect();
    let listed = |ts: &mut dyn Iterator<Item = usize>| ts.map(|t| q(&g.types[t].named.alias)).collect::<Vec<_>>().join(", ");
    let takes: Vec<String> = g.actions.iter().map(|a| format!("  {}: [{}],", q(&a.named.alias), listed(&mut a.resources.iter().copied()))).collect();
    let others: Vec<String> = g.actions.iter().map(|a| format!("  {}: [{}],", q(&a.named.alias), listed(&mut every.iter().copied().filter(|t| !a.resources.contains(t))))).collect();
    let own: Vec<String> = g.actions.iter().filter(|a| !a.inputs.is_empty()).map(|a| q(&a.named.alias)).collect();
    format!(
        r#"// The harness of sekisho's tests/typescript.rs: each case of the data, through the generated code.
import {{ readFileSync }} from "node:fs";
import * as authz from "./authz/{module}";

type Raw = {{ type: string; id: string; roles: string[]; member_of: {{ type: string; id: string }}[]; attrs: Record<string, unknown> }};
type Kinds = {{ roles: string[]; member_of: string[]; attrs: Record<string, string> }};
type Case = {{ name: string; action: string; principal: {{ type: string; id: string }}; store: Raw[]; input: Record<string, unknown>; now: string }};
const data = JSON.parse(readFileSync(process.argv[2], "utf8")) as {{ types: Record<string, Kinds>; actions: Record<string, {{ resource: string; inputs: Record<string, string> }}>; cases: Case[] }};
const FIELDS: Record<string, string> = {{ {fields} }};
// given "null", what may be absent and is left out of a case is given as null instead
const NULLS = process.argv[3] === "null";
const NULLABLE: Record<string, string[]> = {{ {nullable} }};
const NULLABLE_INPUTS: Record<string, string[]> = {{ {nullable_inputs} }};

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
    if (NULLS) for (const n of NULLABLE[type] ?? []) if (!(n in e.attrs)) out[FIELDS[n] ?? n] = null;
    return out;
  }};
  return {{
{methods}
  }} as unknown as authz.{store};
}}

const ACTIONS: Record<string, (s: authz.{store}, p: unknown, i: unknown, n: Date) => Promise<authz.{answer}>> = {{
{actions}
}};

const TAKES: Record<string, string[]> = {{
{takes}
}};
const OTHERS: Record<string, string[]> = {{
{others}
}};
const OWN: string[] = [{own}];
const TYPES: string[] = [{type_names}];

function said(a: authz.{answer}): string {{
  return JSON.stringify([a.allowed, a.policies, a.error ? a.error.kind : null, a.error ? null : (a.context ?? null)]);
}}

async function main(): Promise<void> {{
  let same = 0;
  let refused = 0;
  let narrowed = 0;
  for (const c of data.cases) {{
    const kinds = data.actions[c.action].inputs;
    const input: Record<string, unknown> = {{}};
    for (const [n, v] of Object.entries(c.input)) input[n] = value(kinds[n], v);
    if (NULLS) for (const n of NULLABLE_INPUTS[c.action] ?? []) if (!(n in c.input)) input[n] = null;
    const a = await ACTIONS[c.action](store(c.store), c.principal, input, new Date(c.now));
    console.log(JSON.stringify({{ name: c.name, allowed: a.allowed, policies: a.policies, error: a.error ? a.error.kind : null, context: a.error ? null : (a.context ?? null) }}));
    // the entry by the action's name, given the same: the resource's id under its type, beside an
    // id of each type of the gate's the action does not take
    const resources: Record<string, string> = {{}};
    for (const t of OTHERS[c.action]) resources[t] = "other-" + t;
    const ts = TAKES[c.action];
    const given = c.input.resource_type;
    const rt = ts.length === 1 ? ts[0] : typeof given === "string" ? given : "";
    const id = c.input[data.actions[c.action].resource];
    if (typeof id === "string" && rt !== "") resources[rt] = id;
    const b = await authz.authorize(store(c.store), c.principal as unknown as authz.{principal}, c.action, resources, new Date(c.now));
    if (OWN.includes(c.action)) {{
      if (b.error?.kind !== "input" || b.allowed || b.policies.length !== 0) console.log(`by name differs: ${{c.name}}: ${{said(b)}}, not a refusal of the kind input`);
      else refused++;
    }} else if (said(b) !== said(a)) {{
      console.log(`by name differs: ${{c.name}}: ${{said(b)}}, not ${{said(a)}}`);
    }} else {{
      same++;
    }}
    // the roles a sign-in carries, every role of the gate's and one it does not declare, passed
    // through rolesOf of the principal's type as a service does: asked again, the entry does not
    // refuse the principal
    const holds = data.types[c.principal.type]?.roles.length ?? 0;
    if (holds > 0 && !OWN.includes(c.action) && !b.error) {{
      const may = authz.{roles_of}(c.principal.type);
      const kept = [...authz.{roles}(), "no_such_role"].filter((x) => may.includes(x));
      const again = c.store.map((e) => (e.type === c.principal.type && e.id === c.principal.id ? {{ ...e, roles: kept }} : e));
      const d = await authz.authorize(store(again), c.principal as unknown as authz.{principal}, c.action, resources, new Date(c.now));
      if (d.error?.kind === "principal") console.log(`roles of differs: ${{c.name}}: ${{JSON.stringify(kept)}} refused as the principal: ${{d.error.message}}`);
      else narrowed++;
    }}
  }}
  // what the entry refuses before any function is asked
  let probes = 0;
  const none = new Date("2026-01-01T00:00:00Z");
  const nobody = {{ type: "", id: "" }} as unknown as authz.{principal};
  const probe = (what: string, b: authz.{answer}, kind: string, message: string): void => {{
    probes++;
    if (b.error?.kind !== kind || b.allowed || b.policies.length !== 0 || (message !== "" && b.error?.message !== message)) console.log(`by name differs: ${{what}}: ${{said(b)}}, not a refusal of the kind ${{kind}} ${{message}}`);
  }};
  probe("a name of no action", await authz.authorize(store([]), nobody, "no such action", {{}}, none), "action", 'the gate has no action "no such action"');
  for (const [action, ts] of Object.entries(TAKES)) {{
    if (OWN.includes(action)) continue;
    probe(`${{action}} with no id`, await authz.authorize(store([]), nobody, action, {{}}, none), "resource", "");
    if (ts.length > 1) probe(`${{action}} with an id of each of its types`, await authz.authorize(store([]), nobody, action, Object.fromEntries(ts.map((t) => [t, "x"])), none), "resource", "");
  }}
  console.log(`by name: ${{same}} answered as the action's own function answers, ${{refused}} refused as taking inputs of their own, ${{probes}} refusals before any function`);
  // the list of the roles the gate declares, a new array each call
  const listed = authz.{roles}();
  const again = authz.{roles}();
  if (again.length > 0) again[0] = "changed";
  console.log(`roles: ${{JSON.stringify(listed)}} ${{JSON.stringify(authz.{roles}())}}`);
  // the roles of each type, and of a name the gate does not declare
  console.log(`roles of: ${{narrowed}} asked again with the roles rolesOf leaves`);
  for (const t of TYPES) console.log(`roles of ${{t}}: ${{JSON.stringify(authz.{roles_of}(t))}}`);
}}

main().catch((e: unknown) => {{
  console.error(e);
  process.exit(1);
}});
"#,
        fields = fields.join(", "),
        nullable = nullable.join(", "),
        nullable_inputs = nullable_inputs.join(", "),
        methods = methods.join("\n"),
        actions = actions.join("\n"),
        takes = takes.join("\n"),
        others = others.join("\n"),
        own = own.join(", "),
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
    /// the aliases of the roles the gate declares, as `JSON.stringify` writes them
    roles: String,
    /// what the harness prints of `rolesOf` for each type of the gate and for a name it does not
    /// declare: the roles of the type's `roles` line, in the order the gate declares the roles
    roles_of: Vec<String>,
}

fn make(root: &Path, n: usize, gate: &str, suite: &sekisho::suite::Suite) -> Option<Made> {
    let o = common::check(gate);
    if o.has_errors() || !sekisho::spellings::stops(&o, "typescript").is_empty() {
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
        let text = typescript::module(scope, checked, suite, &shown, lang, authorizer, &ritsu_emit::package::Place::here()).unwrap_or_else(|e| panic!("{gate}: {e}"));
        write(&format!("{sub}/{alias}.ts"), &text);
    }
    let names = typescript::names(g, &sekisho::cedar::shape(g, scope, checked));
    write("harness.ts", &harness(g, &alias, &names));
    write("raw.json", &r.json(g));
    let each: Vec<String> = g.roles.iter().map(|r| ritsu_base::json::quote(&r.named.alias)).collect();
    let mut roles_of: Vec<String> = g
        .types
        .iter()
        .map(|t| {
            let mut rs = t.roles.clone();
            rs.sort_unstable();
            rs.dedup();
            let each: Vec<String> = rs.iter().map(|&r| ritsu_base::json::quote(&g.roles[r].named.alias)).collect();
            format!("roles of {}: [{}]", t.named.alias, each.join(","))
        })
        .collect();
    roles_of.push("roles of no such type: []".to_string());
    Some(Made { gate: gate.to_string(), dir, raw: r, roles: format!("[{}]", each.join(",")), roles_of })
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
    let (mut cases, mut narrowed) = (0, 0usize);
    for m in made {
        // each case twice: what may be absent left out, as the data has it, and given as null,
        // which the code reads as absent too (DESIGN 31)
        for nulls in [false, true] {
            let started = std::time::Instant::now();
            let out = root.join("out").join(&m.dir);
            let mut c = Command::new("node");
            c.arg(out.join("harness.js")).arg(root.join(&m.dir).join("raw.json")).current_dir(&out);
            if nulls {
                c.arg("null");
            }
            let r = ritsu_testkit::run(&mut c, LIMIT);
            let how = if nulls { ", what may be absent given as null" } else { "" };
            if !r.ok {
                wrong.push(format!("{}{how}: node failed:\n{}", m.gate, r.both()));
                continue;
            }
            match m.raw.compare(&r.stdout) {
                Ok(n) => {
                    if !nulls {
                        cases += n;
                    }
                    let (refused, allowed) = m.raw.counts();
                    println!("compared {}{how}: {n} cases ({allowed} allowed, {refused} refused) agree with the reference ({:.1} s)", m.gate, started.elapsed().as_secs_f64());
                }
                Err(e) => wrong.push(format!("{}{how}: {e}", m.gate)),
            }
            // the entry by the action's name, in both runs
            let differs: Vec<&str> = r.stdout.lines().filter(|l| l.starts_with("by name differs: ")).collect();
            if !differs.is_empty() {
                wrong.push(format!("{}{how}: {} answers of authorize differ:\n{}", m.gate, differs.len(), differs.iter().take(20).copied().collect::<Vec<_>>().join("\n")));
            }
            match r.stdout.lines().find(|l| l.starts_with("by name: ")) {
                Some(l) => println!("{}{how}: {}", m.gate, l.trim_start_matches("by name: ")),
                None => wrong.push(format!("{}{how}: the harness does not say what authorize answered", m.gate)),
            }
            // roles() is the gate's roles as it declares them, and a change to what it returned is
            // not in the next
            match r.stdout.lines().find_map(|l| l.strip_prefix("roles: ")) {
                Some(l) if l == format!("{0} {0}", m.roles) => {}
                other => wrong.push(format!("{}{how}: roles() gave {other:?}, not {} twice", m.gate, m.roles)),
            }
            // rolesOf of each type, and the cases asked again with the roles it leaves
            for want in &m.roles_of {
                if !r.stdout.lines().any(|l| l == want) {
                    wrong.push(format!("{}{how}: no line {want:?}", m.gate));
                }
            }
            let differs: Vec<&str> = r.stdout.lines().filter(|l| l.starts_with("roles of differs: ")).collect();
            if !differs.is_empty() {
                wrong.push(format!("{}{how}: {} cases refused with the roles rolesOf leaves:\n{}", m.gate, differs.len(), differs.iter().take(20).copied().collect::<Vec<_>>().join("\n")));
            }
            match r.stdout.lines().find_map(|l| l.strip_prefix("roles of: ")).and_then(|l| l.split(' ').next()?.parse::<usize>().ok()) {
                Some(n) => {
                    if !nulls {
                        narrowed += n;
                    }
                    if m.gate.ends_with("tests/gen/roles_of.gate") && n == 0 {
                        wrong.push(format!("{}{how}: no case asked again with the roles rolesOf leaves", m.gate));
                    }
                }
                None => wrong.push(format!("{}{how}: the harness does not say what it asked with the roles rolesOf leaves", m.gate)),
            }
        }
    }
    println!("{narrowed} cases asked again with every role of the gate's and one more, passed through rolesOf, and none refused as the principal");
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

//! The function that counts the answers over the points of the random inputs (§15.225), in each of
//! the nine languages with modules, and in the NumPy runtime.
//!
//! The function takes the inputs that are not random (and the sequences), runs the rule on every
//! point of the random inputs — the first random input outermost, each from its lowest value — and
//! counts each answer: how many points there are (`total`), and each answer with how many points
//! give it (`ways`), in the order first met. A point the door refuses (a `constraint`, a drawn value
//! past the weights of a pick) is no point. It is the rule's own function that is run, so the counts
//! never disagree with it. A rule whose one random input is read by the `draw` line of its pick by
//! weight and nothing else is counted from the weights instead (§15.226): the walk is run once for
//! the running totals of the weights, and the rule once per element weighed, at the first value that
//! takes it.
//!
//! The names follow each language's own: `pick_chances` in Python, Rust, Ruby and PHP,
//! `pickChances` in TypeScript, JavaScript, Swift and Java, `PickChances` in Go. Every runner answers
//! `--chances`: for each line of inputs, a line `{"total":N}` and a line per answer, its count, a tab
//! and its outputs as the record's `observed` writes them, which `rulec test` holds to what the
//! reference counted (`crate::chances::vector_files`).

use super::{camel, pascal, pub_name, snake, Gen};
use crate::chances::Method;
use crate::types::Ty;

/// What the function that counts the answers of a rule whose alias is `alias` is called in `lang`:
/// `pick_chances` in Python, Rust, Ruby and PHP, `pickChances` in TypeScript, JavaScript, Swift and
/// Java, `PickChances` in Go.
pub fn chances_fn_name(alias: &str, lang: &str) -> String {
    let base = format!("{}_chances", snake(alias));
    match lang {
        "typescript" | "javascript" | "swift" | "java" => camel(&base),
        "go" => pascal(&base),
        _ => base,
    }
}

/// One random input, as the function walks it: by its declared name, its alias, and what it is drawn
/// from — whole counts of its step between two ends (as the function takes them), an enum's values
/// in order, or a truth value's two.
pub(crate) struct Drawn {
    pub name: String,
    pub alias: String,
    pub ty: Ty,
    pub from: From,
}

pub(crate) enum From {
    Range(i128, i128),
    Values(Vec<String>),
    Bool,
}

impl<'a> Gen<'a> {
    /// The random inputs, in the order declared.
    pub(crate) fn drawn(&self) -> Vec<Drawn> {
        let mut out = Vec::new();
        for i in self.f.random_inputs() {
            let ty = self.ty_of(&i.name.text);
            let from = match &ty {
                Ty::Enum(en) => From::Values(match self.c.enum_ranges.get(&i.name.text) {
                    Some(vs) => vs.clone(),
                    None => self.c.enums.get(en).cloned().unwrap_or_default(),
                }),
                Ty::Bool => From::Bool,
                _ => {
                    let Some((Some(lo), Some(hi))) = self.c.ranges.get(&i.name.text).copied() else { continue };
                    let sc = self.c.wire_scale(&i.name.text);
                    From::Range(crate::types::wire_int(lo, sc), crate::types::wire_int(hi, sc))
                }
            };
            out.push(Drawn { name: i.name.text.clone(), alias: pub_name(&i.name), ty, from });
        }
        out
    }

    /// The function's name in `lang`.
    pub(crate) fn chances_name(&self, lang: &str) -> String {
        chances_fn_name(&pub_name(&self.f.name), lang)
    }

    /// How the answers are counted (§15.226): from the weights, with the drawn input's name and its
    /// two ends, or over every point.
    pub(crate) fn chances_method(&self) -> Option<(String, i128, i128)> {
        match crate::chances::method(self.f) {
            Method::Weighted { draw, .. } => {
                let d = self.drawn().into_iter().find(|d| d.name == draw)?;
                match d.from {
                    From::Range(lo, hi) => Some((d.alias, lo, hi)),
                    _ => None,
                }
            }
            Method::Points => None,
        }
    }

    /// [`Gen::chances_doc`], for the emitters of the languages in files of their own.
    pub(crate) fn chances_docs(&self) -> (String, String) {
        self.chances_doc()
    }

    /// [`Gen::weighted_draw`], for the emitters of the languages in files of their own.
    pub(crate) fn weighted_draw_pub(&self) -> Option<(String, i128, i128)> {
        self.weighted_draw()
    }

    /// What the function and its result say of themselves.
    fn chances_doc(&self) -> (String, String) {
        let names: Vec<String> = self.drawn().iter().map(|d| d.name.clone()).collect();
        let rule = &self.f.name.text;
        let joined = names.join(tr!("・", ", ").as_str());
        let verb = if names.len() == 1 { "is" } else { "are" };
        (
            tr!(
                "{joined} を一様に引いたときの、規則 {rule} の答えの数え上げ。答えごとに、それを出す引き方の数と、引き方の全部の数を返す。答えの確率は、その数を全部の数で割った値。",
                "The answers of rule {rule} when {joined} {verb} drawn uniformly: each answer with how many of the draws give it, and how many draws there are. An answer's chance is its count over the total."
            ),
            tr!(
                "答えごとの引き方の数（ways）と、引き方の全部の数（total）。確率は ways を total で割った値（rulec の §15.225）。",
                "How many draws give each answer (ways), and how many draws there are (total). A chance is ways over total (rulec's §15.225)."
            ),
        )
    }

    // ── Python ────────────────────────────────────────────────────────────────

    /// The `Chances` type and the function, in Python.
    pub(crate) fn py_chances(&self) -> String {
        if !self.f.has_random() {
            return String::new();
        }
        let (doc_fn, doc_ty) = self.chances_doc();
        let outs = &self.f.outputs;
        let ret = if outs.len() == 1 { self.py_ty(&self.ty_of(&outs[0].name.text)) } else { "Output".into() };
        let rule = pub_name(&self.f.name);
        let mut params: Vec<String> = Vec::new();
        for i in self.f.inputs.iter().filter(|i| i.random.is_none()) {
            params.push(format!("{}: {}", pub_name(&i.name), self.py_ty(&self.ty_of(&i.name.text))));
        }
        for (s, el) in self.f.elements.iter().enumerate() {
            params.push(format!("{}: list[{}]", pub_name(&el.name), self.element_type(s)));
        }
        let (total, ways, out, k, last, e) = (self.temp("total"), self.temp("ways"), self.temp("out"), self.temp("k"), self.temp("last"), self.temp("e"));
        let call = |draw: Option<(&str, String)>| -> String {
            let mut args: Vec<String> = self
                .f
                .inputs
                .iter()
                .map(|i| match draw {
                    Some((d, ref v)) if pub_name(&i.name) == d => v.clone(),
                    _ => pub_name(&i.name),
                })
                .collect();
            for el in &self.f.elements {
                args.push(pub_name(&el.name));
            }
            args.join(", ")
        };
        // One answer more, at its place in the order first met.
        let add = |ind: &str, n: &str| -> String {
            format!(
                "{ind}{k} = 0\n{ind}while {k} < _len({ways}) and {ways}[{k}][0] != {out}:\n{ind}    {k} += 1\n{ind}if {k} == _len({ways}):\n{ind}    {ways}.append(({out}, {n}))\n{ind}else:\n{ind}    {ways}[{k}] = ({out}, {ways}[{k}][1] + {n})\n"
            )
        };
        let name = self.chances_name("python");
        let mut o = format!("\n\nclass Chances(NamedTuple):\n    \"\"\"{doc_ty}\"\"\"\n\n    total: int\n    ways: list[tuple[{ret}, int]]\n\n\n");
        o.push_str(&format!("def {name}({}) -> Chances:\n    \"\"\"{doc_fn}\"\"\"\n", params.join(", ")));
        o.push_str(&format!("    {total} = 0\n    {ways}: list[tuple[{ret}, int]] = []\n"));
        match self.chances_method() {
            Some((draw, lo, hi)) => {
                let cuts = self.temp("cuts");
                let (low, cut, n) = (self.temp("low"), self.temp("cut"), self.temp("n"));
                o.push_str(&format!("    _, _, {cuts} = {}({})\n", self.walked_fn("python"), call(Some((&draw, lo.to_string())))));
                o.push_str(&format!(
                    "    if _len({cuts}) == 0 or {cuts}[_len({cuts}) - 1] == 0:\n        return Chances({}, [({rule}({}), {})])\n",
                    hi - lo + 1,
                    call(Some((&draw, lo.to_string()))),
                    hi - lo + 1
                ));
                o.push_str(&format!("    {low} = {lo}\n    for {cut} in {cuts}:\n        if {cut} > {low} and {low} <= {hi}:\n"));
                o.push_str(&format!("            {n} = _min({cut}, {}) - {low}\n", hi + 1));
                o.push_str(&format!("            {out} = {rule}({})\n", call(Some((&draw, low.clone())))));
                o.push_str(&add("            ", &n));
                o.push_str(&format!("            {total} += {n}\n        if {cut} > {low}:\n            {low} = {cut}\n"));
                o.push_str(&format!("    return Chances({total}, {ways})\n"));
            }
            None => {
                o.push_str(&format!("    {last}: RuleInputError | None = None\n"));
                let mut ind = "    ".to_string();
                for (j, d) in self.drawn().iter().enumerate() {
                    match &d.from {
                        From::Range(lo, hi) => {
                            let ctr = self.temp(&format!("_i{j}"));
                            o.push_str(&format!("{ind}{ctr} = {}\n{ind}while {ctr} < {hi}:\n{ind}    {ctr} += 1\n", lo - 1));
                            let v = if matches!(d.ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) { format!("{}({ctr})", super::brand_of(&d.ty)) } else { ctr.clone() };
                            o.push_str(&format!("{ind}    {} = {v}\n", d.alias));
                        }
                        From::Values(vs) => {
                            let members: Vec<String> = vs.iter().map(|v| self.py_value(v, &d.ty)).collect();
                            o.push_str(&format!("{ind}for {} in ({}{}):\n", d.alias, members.join(", "), if members.len() == 1 { "," } else { "" }));
                        }
                        From::Bool => o.push_str(&format!("{ind}for {} in (False, True):\n", d.alias)),
                    }
                    ind.push_str("    ");
                }
                o.push_str(&format!("{ind}try:\n{ind}    {out} = {rule}({})\n{ind}except RuleInputError as {e}:\n{ind}    {last} = {e}\n{ind}    continue\n", call(None)));
                o.push_str(&format!("{ind}{total} += 1\n"));
                o.push_str(&add(&ind, "1"));
                o.push_str(&format!("    if {total} == 0 and {last} is not None:\n        raise {last}\n    return Chances({total}, {ways})\n"));
            }
        }
        o
    }

    /// What the Python runner answers with `--chances`: `args` reads every input of a line, and
    /// `prelude` the sequences, as the vectors are read.
    pub(crate) fn py_chances_mode(&self, args: &[String], prelude: &str) -> String {
        if !self.f.has_random() {
            return String::new();
        }
        let alias = pub_name(&self.f.name);
        let name = self.chances_name("python");
        let n_in = self.f.inputs.len();
        let mut pass: Vec<String> = Vec::new();
        for (k, i) in self.f.inputs.iter().enumerate() {
            if i.random.is_none() {
                pass.push(format!("args[{k}]"));
            }
        }
        for s in 0..self.f.elements.len() {
            pass.push(format!("args[{}]", n_in + s));
        }
        let tuple = if args.len() == 1 { format!("{},", args[0]) } else { args.join(", ") };
        format!(
            "if \"--chances\" in sys.argv[1:]:\n    for line in sys.stdin:\n        line = line.strip()\n        if not line:\n            continue\n        d = json.loads(line)[\"in\"]\n{prelude}        args = ({tuple})\n        c = m.{name}({})\n        print('{{\"total\":' + str(c.total) + \"}}\")\n        for out, n in c.ways:\n            rec = m.{alias}_record(*args, out, [])\n            print(str(n) + \"\\t\" + rec[rec.rindex(',\"observed\":') + 12 : rec.rindex(',\"trace\":')])\n    sys.exit(0)\n\n",
            pass.join(", ")
        )
    }
}

/// The arguments of one call of the rule in a language: every input by `name`, the drawn input of a
/// pick by weight by the value given, and the sequences.
fn call_args(g: &Gen, name: &dyn Fn(&crate::ast::VarDecl) -> String, seq: &dyn Fn(&crate::ast::ElementsDecl) -> String, draw: Option<(&str, &str)>) -> Vec<String> {
    let mut args: Vec<String> = g
        .f
        .inputs
        .iter()
        .map(|i| match draw {
            Some((d, v)) if i.name.text == d => v.to_string(),
            _ => name(i),
        })
        .collect();
    for el in &g.f.elements {
        args.push(seq(el));
    }
    args
}

impl<'a> Gen<'a> {
    /// The declared name of the drawn input of a pick by weight, with its two ends, when the answers
    /// are counted from the weights.
    fn weighted_draw(&self) -> Option<(String, i128, i128)> {
        match crate::chances::method(self.f) {
            Method::Weighted { draw, .. } => {
                let d = self.drawn().into_iter().find(|d| d.name == draw)?;
                match d.from {
                    From::Range(lo, hi) => Some((d.name, lo, hi)),
                    _ => None,
                }
            }
            Method::Points => None,
        }
    }

    // ── TypeScript and JavaScript ─────────────────────────────────────────────

    /// The `Chances` type and the function, in TypeScript (and, with the types taken off, in
    /// JavaScript).
    pub(crate) fn ts_chances(&self) -> String {
        if !self.f.has_random() {
            return String::new();
        }
        let (doc_fn, doc_ty) = self.chances_doc();
        let outs = &self.f.outputs;
        let ret = if outs.len() == 1 { self.ts_ty(&self.ty_of(&outs[0].name.text)) } else { "Output".into() };
        let rule = pub_name(&self.f.name);
        let name = self.chances_name("typescript");
        let mut params: Vec<String> = self.f.inputs.iter().filter(|i| i.random.is_none()).map(|i| format!("{}: {}", pub_name(&i.name), self.ts_ty(&self.ty_of(&i.name.text)))).collect();
        for (s, el) in self.f.elements.iter().enumerate() {
            params.push(format!("{}: readonly {}[]", pub_name(&el.name), self.element_type(s)));
        }
        let (total, ways, out, k, last, e) = (self.temp("total"), self.temp("ways"), self.temp("out"), self.temp("k"), self.temp("last"), self.temp("e"));
        let same = if outs.len() == 1 {
            format!("{ways}[{k}][0] === {out}")
        } else {
            outs.iter().map(|o| format!("{ways}[{k}][0].{f} === {out}.{f}", f = pub_name(&o.name))).collect::<Vec<_>>().join(" && ")
        };
        let add = |ind: &str, n: &str| -> String {
            format!("{ind}let {k} = 0;\n{ind}while ({k} < {ways}.length && !({same})) {{\n{ind}  {k} += 1;\n{ind}}}\n{ind}if ({k} === {ways}.length) {{\n{ind}  {ways}.push([{out}, {n}]);\n{ind}}} else {{\n{ind}  {ways}[{k}][1] += {n};\n{ind}}}\n")
        };
        let call = |draw: Option<(&str, &str)>| call_args(self, &|i| pub_name(&i.name), &|el| pub_name(&el.name), draw).join(", ");
        let mut o = format!("/** {doc_ty} */\nexport interface Chances {{\n  readonly total: bigint;\n  readonly ways: [{ret}, bigint][];\n}}\n\n");
        o.push_str(&format!("/** {doc_fn} */\nexport function {name}({}): Chances {{\n", params.join(", ")));
        o.push_str(&format!("  let {total} = 0n;\n  const {ways}: [{ret}, bigint][] = [];\n"));
        match self.weighted_draw() {
            Some((draw, lo, hi)) => {
                let (cuts, low, cut, n) = (self.temp("cuts"), self.temp("low"), self.temp("cut"), self.temp("n"));
                let at_lo = format!("{lo}n");
                o.push_str(&format!("  const [, , {cuts}] = {}({});\n", self.walked_fn("typescript"), call(Some((&draw, &at_lo)))));
                o.push_str(&format!(
                    "  if ({cuts}.length === 0 || {cuts}[{cuts}.length - 1] === 0n) {{\n    return {{ total: {all}n, ways: [[{rule}({}), {all}n]] }};\n  }}\n",
                    call(Some((&draw, &at_lo))),
                    all = hi - lo + 1
                ));
                o.push_str(&format!("  let {low} = {lo}n;\n  for (const {cut} of {cuts}) {{\n    if ({cut} > {low} && {low} <= {hi}n) {{\n"));
                o.push_str(&format!("      const {n} = ({cut} < {top}n ? {cut} : {top}n) - {low};\n", top = hi + 1));
                o.push_str(&format!("      const {out} = {rule}({});\n", call(Some((&draw, &low)))));
                o.push_str(&add("      ", &n));
                o.push_str(&format!("      {total} += {n};\n    }}\n    if ({cut} > {low}) {{\n      {low} = {cut};\n    }}\n  }}\n"));
                o.push_str(&format!("  return {{ total: {total}, ways: {ways} }};\n}}\n\n"));
            }
            None => {
                o.push_str(&format!("  let {last}: unknown = null;\n"));
                let mut ind = "  ".to_string();
                for (j, d) in self.drawn().iter().enumerate() {
                    match &d.from {
                        From::Range(lo, hi) => {
                            let ctr = self.temp(&format!("_i{j}"));
                            o.push_str(&format!("{ind}for (let {ctr} = {lo}n; {ctr} <= {hi}n; {ctr} += 1n) {{\n"));
                            let v = if matches!(d.ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) { format!("{ctr} as {}", super::brand_of(&d.ty)) } else { ctr.clone() };
                            o.push_str(&format!("{ind}  const {} = {v};\n", d.alias));
                        }
                        From::Values(vs) => {
                            let members: Vec<String> = vs.iter().map(|v| self.ts_value(v, &d.ty)).collect();
                            o.push_str(&format!("{ind}for (const {} of [{}]) {{\n", d.alias, members.join(", ")));
                        }
                        From::Bool => o.push_str(&format!("{ind}for (const {} of [false, true]) {{\n", d.alias)),
                    }
                    ind.push_str("  ");
                }
                o.push_str(&format!(
                    "{ind}let {out}: {ret};\n{ind}try {{\n{ind}  {out} = {rule}({});\n{ind}}} catch ({e}) {{\n{ind}  if (!({e} instanceof RuleInputError)) {{\n{ind}    throw {e};\n{ind}  }}\n{ind}  {last} = {e};\n{ind}  continue;\n{ind}}}\n{ind}{total} += 1n;\n",
                    call(None)
                ));
                o.push_str(&add(&ind, "1n"));
                for _ in self.drawn() {
                    ind.truncate(ind.len() - 2);
                    o.push_str(&format!("{ind}}}\n"));
                }
                o.push_str(&format!("  if ({total} === 0n && {last} !== null) {{\n    throw {last};\n  }}\n  return {{ total: {total}, ways: {ways} }};\n}}\n\n"));
            }
        }
        o
    }

    /// What the TypeScript runner answers with `--chances`: `d` and `lines` are the runner's own
    /// locals, `args` reads every input of a line and `prelude` the sequences, as the vectors are read.
    pub(crate) fn ts_chances_mode(&self, d: &str, lines: &str, args: &[String], prelude: &str, imports: &mut Vec<String>) -> String {
        if !self.f.has_random() {
            return String::new();
        }
        let alias = pub_name(&self.f.name);
        let name = self.chances_name("typescript");
        if !imports.contains(&name) {
            imports.push(name.clone());
        }
        let n_in = self.f.inputs.len();
        let mut pass: Vec<String> = Vec::new();
        for (k, i) in self.f.inputs.iter().enumerate() {
            if i.random.is_none() {
                pass.push(format!("ca[{k}]"));
            }
        }
        for s in 0..self.f.elements.len() {
            pass.push(format!("ca[{}]", n_in + s));
        }
        r#"if (process.argv.includes("--chances")) {
  for (const cl of @LINES@) {
    if (cl.trim() === "") {
      continue;
    }
    const @D@ = JSON.parse(cl).in as unknown;
@PRELUDE@    const ca = [@ARGS@] as const;
    const cc = @NAME@(@PASS@);
    console.log(`{"total":${cc.total}}`);
    for (const [co, cn] of cc.ways) {
      const rec = @ALIAS@_record(...ca, co, []);
      console.log(`${cn}\t` + rec.slice(rec.lastIndexOf(',"observed":') + 12, rec.lastIndexOf(',"trace":')));
    }
  }
  process.exit(0);
}

"#
        .replace("@LINES@", lines)
        .replace("@D@", d)
        .replace("@PRELUDE@", prelude)
        .replace("@ARGS@", &args.join(", "))
        .replace("@NAME@", &name)
        .replace("@PASS@", &pass.join(", "))
        .replace("@ALIAS@", &alias)
    }

    // ── Rust ──────────────────────────────────────────────────────────────────

    /// The `Chances` type and the function, in Rust.
    pub(crate) fn rs_chances(&self) -> String {
        if !self.f.has_random() {
            return String::new();
        }
        let (doc_fn, doc_ty) = self.chances_doc();
        let outs = &self.f.outputs;
        let ret = if outs.len() == 1 { self.rs_ty(&self.ty_of(&outs[0].name.text)) } else { "Output".into() };
        let rule = pub_name(&self.f.name);
        let name = self.chances_name("rust");
        let mut params: Vec<String> = self.f.inputs.iter().filter(|i| i.random.is_none()).map(|i| format!("{}: {}", pub_name(&i.name), self.rs_ty(&self.ty_of(&i.name.text)))).collect();
        for (s, el) in self.f.elements.iter().enumerate() {
            params.push(format!("{}: &[{}]", pub_name(&el.name), self.element_type(s)));
        }
        // A string is handed to each call afresh: the rule takes it by value.
        let pass = |i: &crate::ast::VarDecl| -> String {
            if matches!(self.ty_of(&i.name.text).present(), Ty::Str) { format!("{}.clone()", pub_name(&i.name)) } else { pub_name(&i.name) }
        };
        let call = |draw: Option<(&str, &str)>| call_args(self, &pass, &|el| pub_name(&el.name), draw).join(", ");
        let (total, ways, out, last, e) = (self.temp("total"), self.temp("ways"), self.temp("out"), self.temp("last"), self.temp("e"));
        let add = |ind: &str, n: &str| -> String {
            format!("{ind}match {ways}.iter_mut().find(|w| w.0 == {out}) {{\n{ind}    Some(w) => w.1 += {n},\n{ind}    None => {ways}.push(({out}, {n})),\n{ind}}}\n")
        };
        let mut o = format!("/// {doc_ty}\n#[derive(Clone, PartialEq, Debug)]\npub struct Chances {{\n    pub total: i64,\n    pub ways: Vec<({ret}, i64)>,\n}}\n\n");
        o.push_str(&format!("/// {doc_fn}\npub fn {name}({}) -> Result<Chances, RuleError> {{\n", params.join(", ")));
        o.push_str(&format!("    let mut {total}: i64 = 0;\n    let mut {ways}: Vec<({ret}, i64)> = Vec::new();\n"));
        match self.weighted_draw() {
            Some((draw, lo, hi)) => {
                let (cuts, low, cut, n) = (self.temp("cuts"), self.temp("low"), self.temp("cut"), self.temp("n"));
                o.push_str(&format!("    let (_, _, {cuts}) = {}({})?;\n", self.walked_fn("rust"), call(Some((&draw, &lo.to_string())))));
                o.push_str(&format!(
                    "    if {cuts}.last().copied().unwrap_or(0) == 0 {{\n        return Ok(Chances {{ total: {all}, ways: vec![({rule}({})?, {all})] }});\n    }}\n",
                    call(Some((&draw, &lo.to_string()))),
                    all = hi - lo + 1
                ));
                o.push_str(&format!("    let mut {low}: i64 = {lo};\n    for {cut} in {cuts} {{\n        if {cut} > {low} && {low} <= {hi} {{\n"));
                o.push_str(&format!("            let {n} = {cut}.min({}) - {low};\n", hi + 1));
                o.push_str(&format!("            let {out} = {rule}({})?;\n", call(Some((&draw, &low)))));
                o.push_str(&add("            ", &n));
                o.push_str(&format!("            {total} += {n};\n        }}\n        if {cut} > {low} {{\n            {low} = {cut};\n        }}\n    }}\n"));
                o.push_str(&format!("    Ok(Chances {{ total: {total}, ways: {ways} }})\n}}\n\n"));
            }
            None => {
                o.push_str(&format!("    let mut {last}: Option<RuleError> = None;\n"));
                let mut ind = "    ".to_string();
                for (j, d) in self.drawn().iter().enumerate() {
                    match &d.from {
                        From::Range(lo, hi) => {
                            let ctr = self.temp(&format!("_i{j}"));
                            o.push_str(&format!("{ind}for {ctr} in {lo}i64..={hi}i64 {{\n"));
                            let v = if matches!(d.ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) { format!("{}({ctr})", super::brand_of(&d.ty)) } else { ctr.clone() };
                            o.push_str(&format!("{ind}    let {} = {v};\n", d.alias));
                        }
                        From::Values(vs) => {
                            let members: Vec<String> = vs.iter().map(|v| self.rs_value(v, &d.ty)).collect();
                            o.push_str(&format!("{ind}for {} in [{}] {{\n", d.alias, members.join(", ")));
                        }
                        From::Bool => o.push_str(&format!("{ind}for {} in [false, true] {{\n", d.alias)),
                    }
                    ind.push_str("    ");
                }
                o.push_str(&format!(
                    "{ind}let {out} = match {rule}({}) {{\n{ind}    Ok(o) => o,\n{ind}    Err({e} @ RuleError::Input {{ .. }}) => {{\n{ind}        {last} = Some({e});\n{ind}        continue;\n{ind}    }}\n{ind}    Err({e}) => return Err({e}),\n{ind}}};\n{ind}{total} += 1;\n",
                    call(None)
                ));
                o.push_str(&add(&ind, "1"));
                for _ in self.drawn() {
                    ind.truncate(ind.len() - 4);
                    o.push_str(&format!("{ind}}}\n"));
                }
                o.push_str(&format!("    if {total} == 0 {{\n        if let Some({e}) = {last} {{\n            return Err({e});\n        }}\n    }}\n    Ok(Chances {{ total: {total}, ways: {ways} }})\n}}\n\n"));
            }
        }
        o
    }

    /// What the Rust runner answers with `--chances`: `binds` reads every input of a line into
    /// `a0`, `a1`, … as the vectors are read.
    pub(crate) fn rs_chances_mode(&self, binds: &str) -> String {
        if !self.f.has_random() {
            return String::new();
        }
        let alias = pub_name(&self.f.name);
        let name = self.chances_name("rust");
        let n_in = self.f.inputs.len();
        let mut pass: Vec<String> = Vec::new();
        for (k, i) in self.f.inputs.iter().enumerate() {
            if i.random.is_none() {
                pass.push(if matches!(self.ty_of(&i.name.text).present(), Ty::Str) { format!("a{k}.clone()") } else { format!("a{k}") });
            }
        }
        for s in 0..self.f.elements.len() {
            pass.push(format!("&a{}", n_in + s));
        }
        let mut second: Vec<String> = (0..n_in).map(|k| if matches!(self.ty_of(&self.f.inputs[k].name.text).present(), Ty::Str) { format!("a{k}.clone()") } else { format!("a{k}") }).collect();
        for s in 0..self.f.elements.len() {
            second.push(format!("&a{}", n_in + s));
        }
        r#"    if std::env::args().any(|a| a == "--chances") {
        for line in src.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let d = fields(line);
            let res = (|| -> Result<Vec<String>, r::RuleError> {
@BINDS@                let c = r::@NAME@(@PASS@)?;
                let mut o = vec![format!("{{\"total\":{}}}", c.total)];
                for (out, n) in &c.ways {
                    let rec = r::@ALIAS@_record(@SECOND@, out.clone(), &[], "");
                    let a = rec.rfind(",\"observed\":").map(|i| i + 12).unwrap_or(0);
                    let b = rec.rfind(",\"trace\":").unwrap_or(rec.len());
                    o.push(format!("{n}\t{}", &rec[a..b]));
                }
                Ok(o)
            })();
            match res {
                Ok(o) => {
                    for l in o {
                        println!("{l}");
                    }
                }
                Err(_) => println!("refused"),
            }
        }
        return;
    }
"#
        .replace("@BINDS@", binds)
        .replace("@NAME@", &name)
        .replace("@PASS@", &pass.join(", "))
        .replace("@ALIAS@", &alias)
        .replace("@SECOND@", &second.join(", "))
    }

    // ── Go ────────────────────────────────────────────────────────────────────

    /// The `Chances` and `Way` types and the function, in Go. The function takes the rule's `Input`
    /// and walks the random fields itself: what the caller put there is not read.
    pub(crate) fn go_chances(&self) -> String {
        if !self.f.has_random() {
            return String::new();
        }
        let (doc_fn, doc_ty) = self.chances_doc();
        let outs = &self.f.outputs;
        let ret = if outs.len() == 1 { self.go_ty(&self.ty_of(&outs[0].name.text)) } else { "Output".into() };
        let fname = pascal(&pub_name(&self.f.name));
        let name = self.chances_name("go");
        let field = |n: &str| -> String {
            self.f.inputs.iter().find(|i| i.name.text == n).map(|i| pascal(&pub_name(&i.name))).unwrap_or_default()
        };
        let add = |ind: &str, n: &str| -> String {
            let inc = if n == "1" { format!("{ind}\tc.Ways[k].Count++\n") } else { format!("{ind}\tc.Ways[k].Count += {n}\n") };
            format!("{ind}k := 0\n{ind}for k < len(c.Ways) && c.Ways[k].Out != out {{\n{ind}\tk++\n{ind}}}\n{ind}if k == len(c.Ways) {{\n{ind}\tc.Ways = append(c.Ways, Way{{Out: out, Count: {n}}})\n{ind}}} else {{\n{inc}{ind}}}\n")
        };
        let (lt, lf) = (lower_first(&doc_ty), lower_first(&doc_fn));
        let mut o = format!(
            "// Chances {}\ntype Chances struct {{\n\tTotal int64\n\tWays  []Way\n}}\n\n// Way {}\ntype Way struct {{\n\tOut   {ret}\n\tCount int64\n}}\n\n",
            tr!("は{doc_ty}", "holds {lt}"),
            tr!("は答えの一つと、それを出す引き方の数。", "is one answer, and how many draws give it.")
        );
        o.push_str(&format!(
            "// {name} {} {}\nfunc {name}(in Input) (Chances, error) {{\n\tvar c Chances\n",
            tr!("は{doc_fn}", "returns {lf}"),
            tr!("in の乱数の入力のフィールドは読まず、とりうる値を全部回す。", "The random fields of in are not read: every value they can take is drawn.")
        ));
        match self.weighted_draw() {
            Some((draw, lo, hi)) => {
                let f = field(&draw);
                o.push_str(&format!("\tin.{f} = {lo}\n\t_, _, cuts, err := {}(in)\n\tif err != nil {{\n\t\treturn Chances{{}}, err\n\t}}\n", self.walked_fn("go")));
                o.push_str(&format!(
                    "\tif len(cuts) == 0 || cuts[len(cuts)-1] == 0 {{\n\t\tout, err := {fname}(in)\n\t\tif err != nil {{\n\t\t\treturn Chances{{}}, err\n\t\t}}\n\t\treturn Chances{{Total: {all}, Ways: []Way{{{{Out: out, Count: {all}}}}}}}, nil\n\t}}\n",
                    all = hi - lo + 1
                ));
                o.push_str(&format!("\tlow := int64({lo})\n\tfor _, cut := range cuts {{\n\t\tif cut > low && low <= {hi} {{\n\t\t\tn := cut\n\t\t\tif n > {top} {{\n\t\t\t\tn = {top}\n\t\t\t}}\n\t\t\tn -= low\n", top = hi + 1));
                o.push_str(&format!("\t\t\tin.{f} = low\n\t\t\tout, err := {fname}(in)\n\t\t\tif err != nil {{\n\t\t\t\treturn Chances{{}}, err\n\t\t\t}}\n"));
                o.push_str(&add("\t\t\t", "n"));
                o.push_str("\t\t\tc.Total += n\n\t\t}\n\t\tif cut > low {\n\t\t\tlow = cut\n\t\t}\n\t}\n\treturn c, nil\n}\n\n");
            }
            None => {
                o.push_str("\tvar last error\n");
                let mut ind = "\t".to_string();
                for (j, d) in self.drawn().iter().enumerate() {
                    let f = field(&d.name);
                    let v = format!("v{j}");
                    match &d.from {
                        From::Range(lo, hi) => {
                            o.push_str(&format!("{ind}for {v} := int64({lo}); {v} <= {hi}; {v}++ {{\n"));
                            let x = if matches!(d.ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) { format!("{}({v})", super::brand_of(&d.ty)) } else { v.clone() };
                            o.push_str(&format!("{ind}\tin.{f} = {x}\n"));
                        }
                        From::Values(vs) => {
                            let members: Vec<String> = vs.iter().map(|x| self.go_value(x, &d.ty)).collect();
                            o.push_str(&format!("{ind}for _, {v} := range []{}{{{}}} {{\n{ind}\tin.{f} = {v}\n", self.go_ty(&d.ty), members.join(", ")));
                        }
                        From::Bool => o.push_str(&format!("{ind}for _, {v} := range []bool{{false, true}} {{\n{ind}\tin.{f} = {v}\n")),
                    }
                    ind.push('\t');
                }
                o.push_str(&format!(
                    "{ind}out, err := {fname}(in)\n{ind}if err != nil {{\n{ind}\tif _, ok := err.(*RuleInputError); ok {{\n{ind}\t\tlast = err\n{ind}\t\tcontinue\n{ind}\t}}\n{ind}\treturn Chances{{}}, err\n{ind}}}\n{ind}c.Total++\n"
                ));
                o.push_str(&add(&ind, "1"));
                for _ in self.drawn() {
                    ind.pop();
                    o.push_str(&format!("{ind}}}\n"));
                }
                o.push_str("\tif c.Total == 0 && last != nil {\n\t\treturn Chances{}, last\n\t}\n\treturn c, nil\n}\n\n");
            }
        }
        o
    }

    /// What the Go runner answers with `--chances`: `fields` reads every input of a line into `in`,
    /// as the vectors are read.
    pub(crate) fn go_chances_mode(&self, fields: &str) -> String {
        if !self.f.has_random() {
            return String::new();
        }
        let fname = pascal(&pub_name(&self.f.name));
        let name = self.chances_name("go");
        r#"	if len(os.Args) > 1 && os.Args[1] == "--chances" {
		for sc.Scan() {
			if len(sc.Bytes()) == 0 {
				continue
			}
			var rec struct {
				In map[string]any `json:"in"`
			}
			if err := decode(sc.Bytes(), &rec); err != nil {
				panic(err)
			}
			d := rec.In
			var in r.Input
@FIELDS@			c, err := r.@NAME@(in)
			if err != nil {
				panic(err)
			}
			fmt.Printf("{\"total\":%d}\n", c.Total)
			for _, w := range c.Ways {
				s := r.@FNAME@Record(in, w.Out, nil, "")
				fmt.Printf("%d\t%s\n", w.Count, s[strings.LastIndex(s, ",\"observed\":")+12:strings.LastIndex(s, ",\"trace\":")])
			}
		}
		return
	}
"#
        .replace("@FIELDS@", fields)
        .replace("@NAME@", &name)
        .replace("@FNAME@", &fname)
    }

    // ── Ruby ──────────────────────────────────────────────────────────────────

    /// The `Chances` struct and the method, in Ruby.
    pub(crate) fn rb_chances(&self) -> String {
        if !self.f.has_random() {
            return String::new();
        }
        let (doc_fn, doc_ty) = self.chances_doc();
        let outs = &self.f.outputs;
        let ret = if outs.len() == 1 { self.rbs_ty(&self.ty_of(&outs[0].name.text)) } else { "Output".into() };
        let rule = pub_name(&self.f.name);
        let name = self.chances_name("ruby");
        let mut params: Vec<String> = self.f.inputs.iter().filter(|i| i.random.is_none()).map(|i| pub_name(&i.name)).collect();
        for el in &self.f.elements {
            params.push(pub_name(&el.name));
        }
        let call = |draw: Option<(&str, &str)>| call_args(self, &|i| pub_name(&i.name), &|el| pub_name(&el.name), draw).join(", ");
        let (total, ways, out, k, last, e) = (self.temp("total"), self.temp("ways"), self.temp("out"), self.temp("k"), self.temp("last"), self.temp("e"));
        let add = |ind: &str, n: &str| -> String {
            format!("{ind}{k} = {ways}.index {{ |w| w[0] == {out} }}\n{ind}if {k}.nil?\n{ind}  {ways} << [{out}, {n}]\n{ind}else\n{ind}  {ways}[{k}] = [{out}, {ways}[{k}][1] + {n}]\n{ind}end\n")
        };
        let mut o = format!("  # {doc_ty}\n  Chances = Struct.new(:total, :ways)\n\n");
        o.push_str(&format!("  # {doc_fn}\n  def self.{name}({})\n", params.join(", ")));
        o.push_str(&format!("    {total} = 0\n    {ways} = [] #: Array[[{ret}, Integer]]\n"));
        match self.weighted_draw() {
            Some((draw, lo, hi)) => {
                let (cuts, low, n) = (self.temp("cuts"), self.temp("low"), self.temp("n"));
                o.push_str(&format!("    _, _, {cuts} = {}({})\n", self.walked_fn("ruby"), call(Some((&draw, &lo.to_string())))));
                o.push_str(&format!(
                    "    return Chances.new({all}, [[{rule}({}), {all}]]) if {cuts}.empty? || {cuts}[-1].zero?\n\n",
                    call(Some((&draw, &lo.to_string()))),
                    all = hi - lo + 1
                ));
                o.push_str(&format!("    {low} = {lo}\n    {cuts}.each do |cut|\n      if cut > {low} && {low} <= {hi}\n"));
                o.push_str(&format!("        {n} = [cut, {}].min - {low}\n", hi + 1));
                o.push_str(&format!("        {out} = {rule}({})\n", call(Some((&draw, &low)))));
                o.push_str(&add("        ", &n));
                o.push_str(&format!("        {total} += {n}\n      end\n      {low} = cut if cut > {low}\n    end\n    Chances.new({total}, {ways})\n  end\n\n"));
            }
            None => {
                o.push_str(&format!("    {last} = nil #: RuleInputError?\n"));
                let mut ind = "    ".to_string();
                for d in self.drawn().iter() {
                    match &d.from {
                        From::Range(lo, hi) => o.push_str(&format!("{ind}({lo}..{hi}).each do |{}|\n", d.alias)),
                        From::Values(vs) => {
                            let members: Vec<String> = vs.iter().map(|v| self.rb_value(v, &d.ty)).collect();
                            o.push_str(&format!("{ind}[{}].each do |{}|\n", members.join(", "), d.alias));
                        }
                        From::Bool => o.push_str(&format!("{ind}[false, true].each do |{}|\n", d.alias)),
                    }
                    ind.push_str("  ");
                }
                o.push_str(&format!(
                    "{ind}begin\n{ind}  {out} = {rule}({})\n{ind}rescue RuleInputError => {e}\n{ind}  {last} = {e}\n{ind}  next\n{ind}end\n{ind}{total} += 1\n",
                    call(None)
                ));
                o.push_str(&add(&ind, "1"));
                for _ in self.drawn() {
                    ind.truncate(ind.len() - 2);
                    o.push_str(&format!("{ind}end\n"));
                }
                o.push_str(&format!("    raise {last} if {total}.zero? && !{last}.nil?\n\n    Chances.new({total}, {ways})\n  end\n\n"));
            }
        }
        o
    }

    /// The signatures of `Chances` and of the method, in the Ruby module's `.rbs`.
    pub(crate) fn rbs_chances(&self) -> String {
        if !self.f.has_random() {
            return String::new();
        }
        let outs = &self.f.outputs;
        let ret = if outs.len() == 1 { self.rbs_ty(&self.ty_of(&outs[0].name.text)) } else { "Output".into() };
        let mut params: Vec<String> = self.f.inputs.iter().filter(|i| i.random.is_none()).map(|i| self.rbs_ty(&self.ty_of(&i.name.text))).collect();
        for (s, _) in self.f.elements.iter().enumerate() {
            params.push(format!("Array[{}]", self.element_type(s)));
        }
        format!(
            "  class Chances < Struct[untyped]\n    attr_reader total: Integer\n    attr_reader ways: Array[[{ret}, Integer]]\n    def self.new: (Integer, Array[[{ret}, Integer]]) -> Chances\n  end\n\n  def self.{}: ({}) -> Chances\n",
            self.chances_name("ruby"),
            params.join(", ")
        )
    }

    /// The signature of the method that runs the walk of a pick by weight, in the `.rbs`.
    pub(crate) fn rbs_walked(&self) -> String {
        if self.weighted_fold().is_none() {
            return String::new();
        }
        let outs = &self.f.outputs;
        let ret = if outs.len() == 1 { self.rbs_ty(&self.ty_of(&outs[0].name.text)) } else { "Output".into() };
        let mut params: Vec<String> = self.f.inputs.iter().map(|i| self.rbs_ty(&self.ty_of(&i.name.text))).collect();
        for (s, _) in self.f.elements.iter().enumerate() {
            params.push(format!("Array[{}]", self.element_type(s)));
        }
        format!("  def self.{}: ({}) -> [{ret}, Array[Fired], Array[Integer]]\n", self.walked_fn("ruby"), params.join(", "))
    }

    /// What the Ruby runner answers with `--chances`: `args` reads every input of a line, as the
    /// vectors are read.
    pub(crate) fn rb_chances_mode(&self, args: &[String]) -> String {
        if !self.f.has_random() {
            return String::new();
        }
        let m = self.rb_module();
        let alias = pub_name(&self.f.name);
        let name = self.chances_name("ruby");
        let n_in = self.f.inputs.len();
        let mut pass: Vec<String> = Vec::new();
        for (k, i) in self.f.inputs.iter().enumerate() {
            if i.random.is_none() {
                pass.push(format!("args[{k}]"));
            }
        }
        for s in 0..self.f.elements.len() {
            pass.push(format!("args[{}]", n_in + s));
        }
        r##"if ARGV.include?("--chances")
  STDIN.each_line do |line|
    line = line.strip
    next if line.empty?

    d = JSON.parse(line)["in"]
    args = [@ARGS@]
    c = @M@.@NAME@(@PASS@)
    puts "{\"total\":#{c.total}}"
    c.ways.each do |out, n|
      rec = @M@.@ALIAS@_record(*args, out, [])
      puts "#{n}\t" + rec[(rec.rindex(',"observed":') + 12)...rec.rindex(',"trace":')]
    end
  end
  exit 0
end

"##
        .replace("@ARGS@", &args.join(", "))
        .replace("@M@", &m)
        .replace("@NAME@", &name)
        .replace("@PASS@", &pass.join(", "))
        .replace("@ALIAS@", &alias)
    }

    // ── Swift ─────────────────────────────────────────────────────────────────

    /// The `Chances` and `Way` types and the function, in Swift.
    pub(crate) fn sw_chances(&self) -> String {
        if !self.f.has_random() {
            return String::new();
        }
        let (doc_fn, doc_ty) = self.chances_doc();
        let outs = &self.f.outputs;
        let ret = if outs.len() == 1 { self.sw_ty(&self.ty_of(&outs[0].name.text)) } else { "Output".into() };
        let fname = super::sw_name(&pub_name(&self.f.name));
        let name = self.chances_name("swift");
        let mut params: Vec<String> = self.f.inputs.iter().filter(|i| i.random.is_none()).map(|i| format!("{}: {}", super::sw_name(&pub_name(&i.name)), self.sw_ty(&self.ty_of(&i.name.text)))).collect();
        for (s, el) in self.f.elements.iter().enumerate() {
            params.push(format!("{}: [{}]", super::sw_name(&pub_name(&el.name)), self.element_type(s)));
        }
        // Swift calls with the labels.
        let call = |draw: Option<(&str, &str)>| {
            let mut args: Vec<String> = self
                .f
                .inputs
                .iter()
                .map(|i| {
                    let v = match draw {
                        Some((d, v)) if i.name.text == d => v.to_string(),
                        _ => super::sw_name(&pub_name(&i.name)),
                    };
                    format!("{}: {v}", super::sw_label(&pub_name(&i.name)))
                })
                .collect();
            for el in &self.f.elements {
                args.push(format!("{}: {}", super::sw_label(&pub_name(&el.name)), super::sw_name(&pub_name(&el.name))));
            }
            args.join(", ")
        };
        let (total, ways, out, last, e) = (self.temp("total"), self.temp("ways"), self.temp("out"), self.temp("last"), self.temp("e"));
        let add = |ind: &str, n: &str| -> String {
            format!("{ind}if let k = {ways}.firstIndex(where: {{ $0.out == {out} }}) {{\n{ind}    {ways}[k] = Way(out: {out}, count: {ways}[k].count + {n})\n{ind}}} else {{\n{ind}    {ways}.append(Way(out: {out}, count: {n}))\n{ind}}}\n")
        };
        let mut o = format!(
            "/// {}\npublic struct Way {{\n    public let out: {ret}\n    public let count: Int64\n}}\n\n/// {doc_ty}\npublic struct Chances {{\n    public let total: Int64\n    public let ways: [Way]\n}}\n\n",
            tr!("答えの一つと、それを出す引き方の数。", "One answer, and how many draws give it.")
        );
        o.push_str(&format!("/// {doc_fn}\npublic func {name}({}) throws -> Chances {{\n", params.join(", ")));
        o.push_str(&format!("    var {total}: Int64 = 0\n    var {ways}: [Way] = []\n"));
        match self.weighted_draw() {
            Some((draw, lo, hi)) => {
                let (cuts, low, n) = (self.temp("cuts"), self.temp("low"), self.temp("n"));
                o.push_str(&format!("    let (_, _, {cuts}) = try {}({})\n", self.walked_fn("swift"), call(Some((&draw, &lo.to_string())))));
                o.push_str(&format!(
                    "    if {cuts}.last ?? 0 == 0 {{\n        return Chances(total: {all}, ways: [Way(out: try {fname}({}), count: {all})])\n    }}\n",
                    call(Some((&draw, &lo.to_string()))),
                    all = hi - lo + 1
                ));
                o.push_str(&format!("    var {low}: Int64 = {lo}\n    for cut in {cuts} {{\n        if cut > {low} && {low} <= {hi} {{\n"));
                o.push_str(&format!("            let {n} = min(cut, {}) - {low}\n", hi + 1));
                o.push_str(&format!("            let {out} = try {fname}({})\n", call(Some((&draw, &low)))));
                o.push_str(&add("            ", &n));
                o.push_str(&format!("            {total} += {n}\n        }}\n        if cut > {low} {{\n            {low} = cut\n        }}\n    }}\n    return Chances(total: {total}, ways: {ways})\n}}\n\n"));
            }
            None => {
                o.push_str(&format!("    var {last}: RuleError? = nil\n"));
                let mut ind = "    ".to_string();
                for (j, d) in self.drawn().iter().enumerate() {
                    let v = super::sw_name(&d.alias);
                    match &d.from {
                        From::Range(lo, hi) => {
                            let ctr = self.temp(&format!("_i{j}"));
                            o.push_str(&format!("{ind}for {ctr} in Int64({lo})...Int64({hi}) {{\n"));
                            let x = if matches!(d.ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) { format!("{}({ctr})", super::brand_of(&d.ty)) } else { ctr.clone() };
                            o.push_str(&format!("{ind}    let {v} = {x}\n"));
                        }
                        From::Values(vs) => {
                            let members: Vec<String> = vs.iter().map(|x| self.sw_value(x, &d.ty)).collect();
                            o.push_str(&format!("{ind}for {v} in [{}] as [{}] {{\n", members.join(", "), self.sw_ty(&d.ty)));
                        }
                        From::Bool => o.push_str(&format!("{ind}for {v} in [false, true] {{\n")),
                    }
                    ind.push_str("    ");
                }
                o.push_str(&format!(
                    "{ind}let {out}: {ret}\n{ind}do {{\n{ind}    {out} = try {fname}({})\n{ind}}} catch let {e} as RuleError {{\n{ind}    guard case .input = {e} else {{\n{ind}        throw {e}\n{ind}    }}\n{ind}    {last} = {e}\n{ind}    continue\n{ind}}}\n{ind}{total} += 1\n",
                    call(None)
                ));
                o.push_str(&add(&ind, "1"));
                for _ in self.drawn() {
                    ind.truncate(ind.len() - 4);
                    o.push_str(&format!("{ind}}}\n"));
                }
                o.push_str(&format!("    if {total} == 0, let {e} = {last} {{\n        throw {e}\n    }}\n    return Chances(total: {total}, ways: {ways})\n}}\n\n"));
            }
        }
        o
    }
}

impl<'a> Gen<'a> {
    /// What the Swift runner answers with `--chances`: `binds` reads every input of a line into a
    /// local, `pass` is what the function is called with, `args` what the record function is.
    pub(crate) fn sw_chances_mode(&self, d: &str, binds: &str, pass: &str, args: &str, p_out: &str, p_trace: &str) -> String {
        if !self.f.has_random() {
            return String::new();
        }
        let fname = super::sw_name(&pub_name(&self.f.name));
        let name = self.chances_name("swift");
        r#"        if CommandLine.arguments.contains("--chances") {
            while let cl = readLine(strippingNewline: true) {
                if cl.trimmingCharacters(in: .whitespaces).isEmpty {
                    continue
                }
                let croot = try JSONSerialization.jsonObject(with: Data(cl.utf8)) as! [String: Any]
                let @D@: Any? = croot["in"]
@BINDS@                let c = try @NAME@(@PASS@)
                print("{\"total\":\(c.total)}")
                for w in c.ways {
                    let rec = @FNAME@Record(@ARGS@, @OUT@: w.out, @TRACE@: [])
                    let a = rec.range(of: ",\"observed\":", options: .backwards)!.upperBound
                    let b = rec.range(of: ",\"trace\":", options: .backwards)!.lowerBound
                    print("\(w.count)\t\(rec[a..<b])")
                }
            }
            return
        }
"#
        .replace("@D@", d)
        .replace("@BINDS@", binds)
        .replace("@NAME@", &name)
        .replace("@PASS@", pass)
        .replace("@FNAME@", &fname)
        .replace("@ARGS@", args)
        .replace("@OUT@", p_out)
        .replace("@TRACE@", p_trace)
    }
}

/// A sentence with its first letter in lower case, for a Go doc comment that starts with the name.
fn lower_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) if f.is_ascii_uppercase() => f.to_ascii_lowercase().to_string() + c.as_str(),
        Some(f) => f.to_string() + c.as_str(),
        None => String::new(),
    }
}

impl<'a> Gen<'a> {
    // ── NumPy ─────────────────────────────────────────────────────────────────

    /// The random inputs in the plan, each with the values it is drawn from as they travel (§15.225):
    /// what `rulec_np.Rule.chances` lays out as one column of every point. `None` for a rule with
    /// no random input, so no other plan changes.
    pub(crate) fn np_random(&self) -> Option<String> {
        if !self.f.has_random() {
            return None;
        }
        let items: Vec<String> = self
            .drawn()
            .iter()
            .map(|d| {
                let values = match &d.from {
                    From::Range(lo, hi) => format!("[{}]", (*lo..=*hi).map(|k| k.to_string()).collect::<Vec<_>>().join(",")),
                    From::Values(vs) => crate::json::strs(vs),
                    From::Bool => "[false,true]".to_string(),
                };
                crate::json::Obj::new().str("name", &d.name).raw("values", values).finish()
            })
            .collect();
        Some(format!("[{}]", items.join(",")))
    }

    /// What the NumPy runner answers with `--chances`, from the plan loaded as `rule`.
    pub(crate) fn np_chances_mode(&self) -> String {
        if !self.f.has_random() {
            return String::new();
        }
        r#"if "--chances" in sys.argv[1:]:
    for l in sys.stdin:
        if not l.strip():
            continue
        x = json.loads(l)["in"]
        total, ways = rule.chances(**{k: x[k] for k in rule.inputs if k not in rule.random})
        print(json.dumps({"total": total}, separators=(",", ":")))
        for out, n in ways:
            print(str(n) + "\t" + json.dumps(out, ensure_ascii=False, separators=(",", ":")))
    sys.exit(0)
"#
        .to_string()
    }
}

impl<'a> Gen<'a> {
    // ── `rulec api` ───────────────────────────────────────────────────────────

    /// `chances` in `rulec api` (§15.225): the random inputs and what each is drawn from, how the
    /// answers are counted and how many runs of the rule one call takes, and the function in every
    /// language that has it, by name and signature. SQL and Wasm say why they lack it. `None` for a
    /// rule with no random input, so no other inventory changes.
    pub(crate) fn chances_api(&self) -> Option<String> {
        use crate::json::{arr, Obj};
        if !self.f.has_random() {
            return None;
        }
        let outs = &self.f.outputs;
        let one_or = |single: String| if outs.len() == 1 { single } else { "Output".to_string() };
        let ty0 = outs.first().map(|o| self.ty_of(&o.name.text)).unwrap_or(Ty::Unknown);
        let fixed: Vec<&crate::ast::VarDecl> = self.f.inputs.iter().filter(|i| i.random.is_none()).collect();
        let seqs = |elem: &dyn Fn(usize) -> String| -> Vec<String> { self.f.elements.iter().enumerate().map(|(s, el)| format!("{}{}", pub_name(&el.name), elem(s))).collect() };
        // Python
        let mut py: Vec<String> = fixed.iter().map(|i| format!("{}: {}", pub_name(&i.name), self.py_ty(&self.ty_of(&i.name.text)))).collect();
        py.extend(seqs(&|s| format!(": list[{}]", self.element_type(s))));
        let py_ret = one_or(self.py_ty(&ty0));
        // TypeScript and JavaScript
        let mut ts: Vec<String> = fixed.iter().map(|i| format!("{}: {}", pub_name(&i.name), self.ts_ty(&self.ty_of(&i.name.text)))).collect();
        ts.extend(seqs(&|s| format!(": readonly {}[]", self.element_type(s))));
        let mut js: Vec<String> = fixed.iter().map(|i| pub_name(&i.name)).collect();
        js.extend(self.f.elements.iter().map(|el| pub_name(&el.name)));
        let ts_ret = one_or(self.ts_ty(&ty0));
        // Rust
        let mut rs: Vec<String> = fixed.iter().map(|i| format!("{}: {}", pub_name(&i.name), self.rs_ty(&self.ty_of(&i.name.text)))).collect();
        rs.extend(seqs(&|s| format!(": &[{}]", self.element_type(s))));
        let rs_ret = one_or(self.rs_ty(&ty0));
        // Ruby
        let mut rb: Vec<String> = fixed.iter().map(|i| self.rbs_ty(&self.ty_of(&i.name.text))).collect();
        rb.extend(self.f.elements.iter().enumerate().map(|(s, _)| format!("Array[{}]", self.element_type(s))));
        let rb_ret = one_or(self.rbs_ty(&ty0));
        // PHP
        let mut ph: Vec<String> = fixed.iter().map(|i| format!("{} ${}", self.php_ty(&self.ty_of(&i.name.text)), pub_name(&i.name))).collect();
        ph.extend(self.f.elements.iter().map(|el| format!("array ${}", pub_name(&el.name))));
        let ph_ret = one_or(self.php_ty(&ty0));
        // Go
        let go_ret = one_or(self.go_ty(&ty0));
        // Swift
        let mut sw: Vec<String> = fixed.iter().map(|i| format!("{}: {}", super::sw_name(&pub_name(&i.name)), self.sw_ty(&self.ty_of(&i.name.text)))).collect();
        sw.extend(self.f.elements.iter().enumerate().map(|(s, el)| format!("{}: [{}]", super::sw_name(&pub_name(&el.name)), self.element_type(s))));
        let sw_ret = one_or(self.sw_ty(&ty0));
        // Java
        let mut jv: Vec<String> = fixed.iter().map(|i| format!("{} {}", self.java_ty(&self.ty_of(&i.name.text)), pub_name(&i.name))).collect();
        jv.extend(self.f.elements.iter().enumerate().map(|(s, el)| format!("List<{}> {}", self.element_type(s), pub_name(&el.name))));
        let jv_ret = one_or(self.java_ty(&ty0));
        let cls = self.java_class();
        let (m, ns) = (self.rb_module(), self.php_ns());
        let entry = |function: String, signature: String, result: String| -> String {
            Obj::new().str("function", &function).str("signature", &signature).str("result", &result).finish()
        };
        let n = |lang: &str| self.chances_name(lang);
        let fns = Obj::new()
            .raw("python", entry(n("python"), format!("def {}({}) -> Chances:", n("python"), py.join(", ")), format!("class Chances(NamedTuple): total: int; ways: list[tuple[{py_ret}, int]]")))
            .raw("typescript", entry(n("typescript"), format!("export function {}({}): Chances", n("typescript"), ts.join(", ")), format!("interface Chances {{ readonly total: bigint; readonly ways: [{ts_ret}, bigint][] }}")))
            .raw("javascript", entry(n("javascript"), format!("export function {}({})", n("javascript"), js.join(", ")), "{ total: bigint, ways: [answer, bigint][] }".to_string()))
            .raw("rust", entry(n("rust"), format!("pub fn {}({}) -> Result<Chances, RuleError>", n("rust"), rs.join(", ")), format!("pub struct Chances {{ pub total: i64, pub ways: Vec<({rs_ret}, i64)> }}")))
            .raw("ruby", entry(format!("{m}.{}", n("ruby")), format!("def self.{}: ({}) -> Chances", n("ruby"), rb.join(", ")), format!("Chances = Struct.new(:total, :ways) # ways: Array[[{rb_ret}, Integer]]")))
            .raw("php", entry(format!("\\{ns}\\{}", n("php")), format!("function {}({}): Chances", n("php"), ph.join(", ")), format!("final class Chances {{ int $total; list<array{{0: {ph_ret}, 1: int}}> $ways }}")))
            .raw("go", entry(n("go"), format!("func {}(in Input) (Chances, error)", n("go")), format!("type Chances struct {{ Total int64; Ways []Way }}; type Way struct {{ Out {go_ret}; Count int64 }}")))
            .raw("swift", entry(n("swift"), format!("public func {}({}) throws -> Chances", n("swift"), sw.join(", ")), format!("public struct Chances {{ total: Int64; ways: [Way] }}; public struct Way {{ out: {sw_ret}; count: Int64 }}")))
            .raw("java", entry(format!("{cls}.{}", n("java")), format!("public static Chances {}({})", n("java"), jv.join(", ")), format!("record Chances(long total, List<Way> ways); record Way({jv_ret} out, long count)")));
        // NumPy has the rule only where it does not walk (§15.56); the targets with no room for the
        // function are the registry's, each with the reason `rulec gen` gives.
        let mut without = Obj::new();
        let fns = if self.f.walks() {
            without = without.str("numpy", &super::chances_why("numpy"));
            fns
        } else {
            fns.raw("numpy", entry("Rule.chances(**fixed)".to_string(), "def chances(self, **fixed) -> tuple[int, list[tuple[dict, int]]]".to_string(), "(total, [(outputs as they travel, count), …])".to_string()))
        };
        for b in crate::backend::ALL.iter().filter(|b| !b.chances) {
            without = without.str(b.id, &super::chances_why(b.id));
        }
        let random: Vec<String> = self
            .drawn()
            .iter()
            .map(|d| {
                let o = Obj::new().str("name", &d.name).str("alias", &d.alias).str("type", &d.ty.to_string());
                match &d.from {
                    From::Range(lo, hi) => o.raw("from", lo.to_string()).raw("to", hi.to_string()).finish(),
                    From::Values(vs) => o.raw("values", crate::json::strs(vs)).finish(),
                    From::Bool => o.raw("values", "[false,true]".to_string()).finish(),
                }
            })
            .collect();
        // How many points the random inputs have, and how one call counts them: over every point,
        // running the rule as many times (`runs`), or from the weights of the pick, running the walk
        // once and the rule once per element weighed.
        let space = crate::chances::space(self.f, self.c).unwrap_or(0);
        let o = match crate::chances::method(self.f) {
            Method::Weighted { draw, .. } => Obj::new()
                .raw("random", arr(&random))
                .raw("points", space.to_string())
                .str("counted", "weights")
                .str("draw", &draw),
            Method::Points => Obj::new()
                .raw("random", arr(&random))
                .raw("points", space.to_string())
                .str("counted", "points")
                .raw("runs", space.to_string()),
        };
        Some(o.raw("functions", fns.finish()).raw("without", without.finish()).finish())
    }
}

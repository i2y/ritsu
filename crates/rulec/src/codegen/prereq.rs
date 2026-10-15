//! What a `prerequisites` block is generated as (§15.222): two functions in each of the nine
//! languages with modules, written into the module of the file that holds the block — a rule's,
//! or an `enums` file's.
//!
//! - `<name>_requires(value)`: what the value needs, as the groups of its line — every group, each
//!   by any one of its values. A value that needs nothing has no group (`[]`); one no case can
//!   take has one group with no value in it (`[[]]`), which nothing held can meet, so the second
//!   function needs no case of its own for it. A list of lists returned by a chain over the
//!   values rather than a constant map: a compiler of typed Python takes it as it is.
//! - `<name>_available(value, held)`: whether the value can be taken now — it is not held yet, and
//!   every group has a value in `held`.
//!
//! The names follow each language's own: `enrollment_requires` in Python, Rust, Ruby and PHP,
//! `enrollmentRequires` in TypeScript, JavaScript, Swift and Java, `EnrollmentRequires` in Go.
//! NumPy's plan carries the groups as data and its runtime answers both. SQL and Wasm have one
//! entry and no room for a second function: `rulec gen` says so, and `rulec api` says they lack
//! it. Every runner answers `--prerequisites`: for each line `<block>\t<value>\t<held,…>`, whether
//! the value is available and its groups as `[a][b|c]`, which `rulec test` holds to what the
//! reference ([`crate::prereq::Graph`]) answers.

use super::{camel, pascal, pub_name, rb_str, snake, str_lit, Gen};
use crate::prereq::Graph;
use crate::types::Ty;

/// The names one block's two functions have in a language.
pub(crate) struct Names {
    pub requires: String,
    pub available: String,
}

/// What the two functions of a block whose alias is `alias` are called in `lang` (`rulec api`'s
/// ids): `enrollment_requires` in Python, Rust, Ruby and PHP, `enrollmentRequires` in TypeScript,
/// JavaScript, Swift and Java, `EnrollmentRequires` in Go.
pub fn prereq_fn_names(alias: &str, lang: &str) -> (String, String) {
    let base = snake(alias);
    let (r, a) = (format!("{base}_requires"), format!("{base}_available"));
    match lang {
        "typescript" | "javascript" | "swift" | "java" => (camel(&r), camel(&a)),
        "go" => (pascal(&r), pascal(&a)),
        _ => (r, a),
    }
}

/// The groups of a value as the runners print them: `[a][b|c]`, nothing for no group, `[]` for a
/// group with no value in it.
pub(crate) fn groups_text(gs: &[Vec<String>]) -> String {
    gs.iter().map(|g| format!("[{}]", g.join("|"))).collect()
}

impl<'a> Gen<'a> {
    /// The blocks of the file, read over their enums.
    pub(crate) fn prereq_graphs(&self) -> Vec<Graph> {
        Graph::all(self.f, self.c)
    }

    /// What one block's functions are called in `lang`.
    pub(crate) fn prereq_names(&self, g: &Graph, lang: &str) -> Names {
        let (requires, available) = prereq_fn_names(&g.alias, lang);
        Names { requires, available }
    }

    /// The enum's type, and each value's member, in one language.
    fn prereq_ty(&self, g: &Graph) -> Ty {
        Ty::Enum(g.over.clone())
    }

    /// What the two functions say of themselves. The branches are written in the enum's order,
    /// and the last value is the one every chain ends in, as the order and display name functions
    /// end.
    fn prereq_doc(&self) -> (String, String) {
        (
            tr!(
                "value を取る前に要るもの。どのグループも、その中のどれか一つの値で満たす。何も要らない値はグループを持たず、手に入らない値は値の無いグループを一つ持つ。",
                "What value needs before it can be taken: every group, each by any one of its values. A value that needs nothing has no group; one no case can take has a group with no value in it."
            ),
            tr!(
                "value をいま取れるか。held にまだ無く、要るもののどのグループにも held の値があるとき。",
                "Whether value can be taken now: it is not in held yet, and every group of what it needs has a value in held."
            ),
        )
    }

    // ── Python ────────────────────────────────────────────────────────────────

    /// The two functions of every block, in Python.
    pub(crate) fn py_prereqs(&self) -> String {
        let (doc_r, doc_a) = self.prereq_doc();
        let mut o = String::new();
        for g in self.prereq_graphs() {
            let ty = self.prereq_ty(&g);
            let t = self.enum_type(&g.over);
            let n = self.prereq_names(&g, "python");
            let list = |gs: Vec<Vec<String>>| -> String {
                let inner: Vec<String> = gs.iter().map(|gr| format!("[{}]", gr.iter().map(|p| self.py_value(p, &ty)).collect::<Vec<_>>().join(", "))).collect();
                format!("[{}]", inner.join(", "))
            };
            o.push_str(&format!("\n\ndef {}(value: {t}) -> list[list[{t}]]:\n    \"\"\"{doc_r}\"\"\"\n", n.requires));
            let last = g.values.len().saturating_sub(1);
            for (i, v) in g.values.iter().enumerate() {
                let ret = list(g.requires(v));
                if g.values.len() == 1 {
                    o.push_str(&format!("    return {ret}\n"));
                } else if i == last {
                    o.push_str(&format!("    else:\n        return {ret}\n"));
                } else {
                    let kw = if i == 0 { "if" } else { "elif" };
                    o.push_str(&format!("    {kw} value is {}:\n        return {ret}\n", self.py_value(v, &ty)));
                }
            }
            o.push_str(&format!(
                "\n\ndef {}(value: {t}, held: list[{t}]) -> bool:\n    \"\"\"{doc_a}\"\"\"\n    if value in held:\n        return False\n    for group in {}(value):\n        met = False\n        for p in group:\n            if p in held:\n                met = True\n        if not met:\n            return False\n    return True\n",
                n.available, n.requires
            ));
        }
        if !o.is_empty() {
            o.push_str("\n\n");
        }
        o
    }

    /// What the Python runner answers with `--prerequisites`, the module imported as `m`.
    pub(crate) fn py_prereq_mode(&self) -> String {
        let gs = self.prereq_graphs();
        if gs.is_empty() {
            return String::new();
        }
        let mut arms = String::new();
        for (i, g) in gs.iter().enumerate() {
            let t = self.enum_type(&g.over);
            let n = self.prereq_names(g, "python");
            let kw = if i == 0 { "if" } else { "elif" };
            arms.push_str(&format!(
                "        {kw} name == {}:\n            x = m.{t}(v)\n            hs = [m.{t}(h) for h in held.split(\",\") if h != \"\"]\n            gs = \"\".join(\"[\" + \"|\".join(p.value for p in gr) + \"]\" for gr in m.{}(x))\n            print((\"true\" if m.{}(x, hs) else \"false\") + \"\\t\" + gs)\n",
                str_lit(&g.alias),
                n.requires,
                n.available
            ));
        }
        format!("if \"--prerequisites\" in sys.argv[1:]:\n    for line in sys.stdin:\n        name, v, held = line.rstrip(\"\\n\").split(\"\\t\")\n{arms}    sys.exit(0)\n\n")
    }

    // ── TypeScript and JavaScript ─────────────────────────────────────────────

    /// The two functions of every block, in TypeScript (and, with the types taken off, in
    /// JavaScript).
    pub(crate) fn ts_prereqs(&self) -> String {
        let (doc_r, doc_a) = self.prereq_doc();
        let mut o = String::new();
        for g in self.prereq_graphs() {
            let ty = self.prereq_ty(&g);
            let t = self.enum_type(&g.over);
            let n = self.prereq_names(&g, "typescript");
            let list = |gs: Vec<Vec<String>>| -> String {
                let inner: Vec<String> = gs.iter().map(|gr| format!("[{}]", gr.iter().map(|p| self.ts_value(p, &ty)).collect::<Vec<_>>().join(", "))).collect();
                format!("[{}]", inner.join(", "))
            };
            o.push_str(&format!("/** {doc_r} */\nexport function {}(value: {t}): {t}[][] {{\n", n.requires));
            let last = g.values.len().saturating_sub(1);
            for (i, v) in g.values.iter().enumerate() {
                let ret = list(g.requires(v));
                if i == last {
                    o.push_str(&format!("  return {ret};\n"));
                } else {
                    o.push_str(&format!("  if (value === {}) {{\n    return {ret};\n  }}\n", self.ts_value(v, &ty)));
                }
            }
            o.push_str(&format!(
                "}}\n\n/** {doc_a} */\nexport function {}(value: {t}, held: readonly {t}[]): boolean {{\n  if (held.includes(value)) {{\n    return false;\n  }}\n  for (const group of {}(value)) {{\n    let met = false;\n    for (const p of group) {{\n      if (held.includes(p)) {{\n        met = true;\n      }}\n    }}\n    if (!met) {{\n      return false;\n    }}\n  }}\n  return true;\n}}\n\n",
                n.available, n.requires
            ));
        }
        o
    }

    /// What the TypeScript runner answers with `--prerequisites`. The functions are imported
    /// from `module` (the rule's, or the `enums` file's own); a parser of an enum read from an
    /// `enums` file, from that file's module.
    pub(crate) fn ts_prereq_mode(&self, lines: &str, imports: &mut Vec<String>, owners: &mut Vec<(String, Vec<String>)>) -> String {
        let gs = self.prereq_graphs();
        if gs.is_empty() {
            return String::new();
        }
        let mut arms = String::new();
        for g in &gs {
            let t = self.enum_type(&g.over);
            let n = self.prereq_names(g, "typescript");
            let parse = format!("parse{t}");
            for f in [&n.requires, &n.available] {
                if !imports.contains(f) {
                    imports.push(f.clone());
                }
            }
            match self.c.owners.get(&g.over).map(|o| pub_name(&o.module)) {
                Some(m) => match owners.iter_mut().find(|(x, _)| *x == m) {
                    Some((_, ns)) => {
                        if !ns.contains(&parse) {
                            ns.push(parse.clone());
                        }
                    }
                    None => owners.push((m, vec![parse.clone()])),
                },
                None => {
                    if !imports.contains(&parse) {
                        imports.push(parse.clone());
                    }
                }
            }
            arms.push_str(&format!(
                "    if (name === {}) {{\n      const x = {parse}(v);\n      const hs = held === \"\" ? [] : held.split(\",\").map((h) => {parse}(h));\n      const gs = {}(x).map((gr) => \"[\" + gr.join(\"|\") + \"]\").join(\"\");\n      console.log(({}(x, hs) ? \"true\" : \"false\") + \"\\t\" + gs);\n    }}\n",
                str_lit(&g.alias),
                n.requires,
                n.available
            ));
        }
        format!(
            "if (process.argv.includes(\"--prerequisites\")) {{\n  for (const l of {lines}) {{\n    if (l === \"\") {{\n      continue;\n    }}\n    const [name, v, held] = l.split(\"\\t\");\n{arms}  }}\n  process.exit(0);\n}}\n\n"
        )
    }

    // ── Rust ──────────────────────────────────────────────────────────────────

    /// The two functions of every block, in Rust: a `match` over every value, which the compiler
    /// holds to the enum.
    pub(crate) fn rs_prereqs(&self) -> String {
        let (doc_r, doc_a) = self.prereq_doc();
        let mut o = String::new();
        for g in self.prereq_graphs() {
            let ty = self.prereq_ty(&g);
            let t = self.enum_type(&g.over);
            let n = self.prereq_names(&g, "rust");
            let list = |gs: Vec<Vec<String>>| -> String {
                let inner: Vec<String> = gs.iter().map(|gr| format!("vec![{}]", gr.iter().map(|p| self.rs_value(p, &ty)).collect::<Vec<_>>().join(", "))).collect();
                format!("vec![{}]", inner.join(", "))
            };
            o.push_str(&format!("/// {doc_r}\npub fn {}(value: {t}) -> Vec<Vec<{t}>> {{\n    match value {{\n", n.requires));
            for v in &g.values {
                o.push_str(&format!("        {} => {},\n", self.rs_value(v, &ty), list(g.requires(v))));
            }
            o.push_str(&format!(
                "    }}\n}}\n\n/// {doc_a}\npub fn {}(value: {t}, held: &[{t}]) -> bool {{\n    if held.contains(&value) {{\n        return false;\n    }}\n    {}(value).iter().all(|group| group.iter().any(|p| held.contains(p)))\n}}\n\n",
                n.available, n.requires
            ));
        }
        o
    }

    /// What the Rust runner answers with `--prerequisites`, the module included as `r`.
    pub(crate) fn rs_prereq_mode(&self) -> String {
        let gs = self.prereq_graphs();
        if gs.is_empty() {
            return String::new();
        }
        let mut arms = String::new();
        for g in &gs {
            let t = self.enum_type(&g.over);
            let n = self.prereq_names(g, "rust");
            arms.push_str(&format!(
                "                {} => {{\n                    let x = r::{t}::parse(v).unwrap();\n                    let hs: Vec<r::{t}> = held.split(',').filter(|h| !h.is_empty()).map(|h| r::{t}::parse(h).unwrap()).collect();\n                    let gs: String = r::{}(x).iter().map(|gr| format!(\"[{{}}]\", gr.iter().map(|p| p.as_str()).collect::<Vec<_>>().join(\"|\"))).collect();\n                    println!(\"{{}}\\t{{}}\", r::{}(x, &hs), gs);\n                }}\n",
                str_lit(&g.alias),
                n.requires,
                n.available
            ));
        }
        format!(
            "    if std::env::args().any(|a| a == \"--prerequisites\") {{\n        for line in src.lines() {{\n            let mut parts = line.split('\\t');\n            let (Some(name), Some(v), Some(held)) = (parts.next(), parts.next(), parts.next()) else {{ continue }};\n            match name {{\n{arms}                _ => {{}}\n            }}\n        }}\n        return;\n    }}\n"
        )
    }

    // ── Go ────────────────────────────────────────────────────────────────────

    /// The two functions of every block, in Go.
    pub(crate) fn go_prereqs(&self) -> String {
        // `// Name …`, as Go's comments on what a package exports begin.
        let doc_r = tr!(
            "は、value を取る前に要るものを返す。どのグループも、その中のどれか一つの値で満たす。何も要らない値はグループを持たず、手に入らない値は値の無いグループを一つ持つ。",
            "returns what value needs before it can be taken: every group, each by any one of its values. A value that needs nothing has no group; one no case can take has a group with no value in it."
        );
        let doc_a = tr!(
            "は、value をいま取れるかを返す。held にまだ無く、要るもののどのグループにも held の値があるとき。",
            "reports whether value can be taken now: it is not in held yet, and every group of what it needs has a value in held."
        );
        let mut o = String::new();
        for g in self.prereq_graphs() {
            let ty = self.prereq_ty(&g);
            // The type as the package writes it: by its owner's package where the rule's own
            // functions take its name (§15.239).
            let t = self.go_enum_ty(&g.over);
            let n = self.prereq_names(&g, "go");
            let list = |gs: Vec<Vec<String>>| -> String {
                if gs.is_empty() {
                    return "nil".to_string();
                }
                let inner: Vec<String> = gs.iter().map(|gr| format!("{{{}}}", gr.iter().map(|p| self.go_value(p, &ty)).collect::<Vec<_>>().join(", "))).collect();
                format!("[][]{t}{{{}}}", inner.join(", "))
            };
            o.push_str(&format!("// {} {doc_r}\nfunc {}(value {t}) [][]{t} {{\n\tswitch value {{\n", n.requires, n.requires));
            let last = g.values.len().saturating_sub(1);
            for (i, v) in g.values.iter().enumerate() {
                if i == last {
                    o.push_str(&format!("\tdefault:\n\t\treturn {}\n", list(g.requires(v))));
                } else {
                    o.push_str(&format!("\tcase {}:\n\t\treturn {}\n", self.go_value(v, &ty), list(g.requires(v))));
                }
            }
            o.push_str(&format!(
                "\t}}\n}}\n\n// {} {}\nfunc {}(value {t}, held []{t}) bool {{\n\tfor _, h := range held {{\n\t\tif h == value {{\n\t\t\treturn false\n\t\t}}\n\t}}\n\tfor _, group := range {}(value) {{\n\t\tmet := false\n\t\tfor _, p := range group {{\n\t\t\tfor _, h := range held {{\n\t\t\t\tif h == p {{\n\t\t\t\t\tmet = true\n\t\t\t\t}}\n\t\t\t}}\n\t\t}}\n\t\tif !met {{\n\t\t\treturn false\n\t\t}}\n\t}}\n\treturn true\n}}\n\n",
                n.available,
                doc_a,
                n.available,
                n.requires
            ));
        }
        o
    }

    /// What the Go runner answers with `--prerequisites`, the package imported as `r`.
    pub(crate) fn go_prereq_mode(&self) -> String {
        let gs = self.prereq_graphs();
        if gs.is_empty() {
            return String::new();
        }
        let mut arms = String::new();
        for g in &gs {
            let (t, rt) = (self.enum_type(&g.over), self.go_runner_enum_ty(&g.over));
            let n = self.prereq_names(g, "go");
            arms.push_str(&format!(
                "\t\t\tcase {}:\n\t\t\t\tx, _ := r.Parse{t}(parts[1])\n\t\t\t\ths := []{rt}{{}}\n\t\t\t\tfor _, h := range strings.Split(parts[2], \",\") {{\n\t\t\t\t\tif h != \"\" {{\n\t\t\t\t\t\ty, _ := r.Parse{t}(h)\n\t\t\t\t\t\ths = append(hs, y)\n\t\t\t\t\t}}\n\t\t\t\t}}\n\t\t\t\tgs := \"\"\n\t\t\t\tfor _, gr := range r.{}(x) {{\n\t\t\t\t\tps := []string{{}}\n\t\t\t\t\tfor _, p := range gr {{\n\t\t\t\t\t\tps = append(ps, p.String())\n\t\t\t\t\t}}\n\t\t\t\t\tgs += \"[\" + strings.Join(ps, \"|\") + \"]\"\n\t\t\t\t}}\n\t\t\t\tfmt.Printf(\"%t\\t%s\\n\", r.{}(x, hs), gs)\n",
                str_lit(&g.alias),
                n.requires,
                n.available
            ));
        }
        format!(
            "\tif len(os.Args) > 1 && os.Args[1] == \"--prerequisites\" {{\n\t\tfor sc.Scan() {{\n\t\t\tparts := strings.Split(sc.Text(), \"\\t\")\n\t\t\tif len(parts) != 3 {{\n\t\t\t\tcontinue\n\t\t\t}}\n\t\t\tswitch parts[0] {{\n{arms}\t\t\t}}\n\t\t}}\n\t\treturn\n\t}}\n"
        )
    }

    // ── Ruby ──────────────────────────────────────────────────────────────────

    /// The two functions of every block, as module functions in Ruby.
    pub(crate) fn rb_prereqs(&self) -> String {
        let (doc_r, doc_a) = self.prereq_doc();
        let mut o = String::new();
        for g in self.prereq_graphs() {
            let ty = self.prereq_ty(&g);
            let n = self.prereq_names(&g, "ruby");
            let list = |gs: Vec<Vec<String>>| -> String {
                let inner: Vec<String> = gs.iter().map(|gr| format!("[{}]", gr.iter().map(|p| self.rb_value(p, &ty)).collect::<Vec<_>>().join(", "))).collect();
                format!("[{}]", inner.join(", "))
            };
            o.push_str(&format!("  # {doc_r}\n  def self.{}(value)\n    case value\n", n.requires));
            let last = g.values.len().saturating_sub(1);
            for (i, v) in g.values.iter().enumerate() {
                if i == last {
                    o.push_str(&format!("    else {}\n", list(g.requires(v))));
                } else {
                    o.push_str(&format!("    when {} then {}\n", self.rb_value(v, &ty), list(g.requires(v))));
                }
            }
            o.push_str(&format!(
                "    end\n  end\n\n  # {doc_a}\n  def self.{}(value, held)\n    return false if held.include?(value)\n\n    {}(value).all? {{ |group| group.any? {{ |p| held.include?(p) }} }}\n  end\n\n",
                n.available, n.requires
            ));
        }
        o
    }

    /// The two functions' signatures in the `.rbs`.
    pub(crate) fn rbs_prereqs(&self) -> String {
        let mut o = String::new();
        for g in self.prereq_graphs() {
            let t = self.rbs_ty(&self.prereq_ty(&g));
            let n = self.prereq_names(&g, "ruby");
            o.push_str(&format!("  def self.{}: ({t}) -> Array[Array[{t}]]\n  def self.{}: ({t}, Array[{t}]) -> bool\n\n", n.requires, n.available));
        }
        o
    }

    /// What the Ruby runner answers with `--prerequisites`: the values are their spellings, as
    /// every Ruby enum's are.
    pub(crate) fn rb_prereq_mode(&self) -> String {
        let gs = self.prereq_graphs();
        if gs.is_empty() {
            return String::new();
        }
        let m = self.rb_module();
        let mut arms = String::new();
        for g in &gs {
            let n = self.prereq_names(g, "ruby");
            arms.push_str(&format!(
                "    when {}\n      gs = {m}.{}(v).map {{ |gr| \"[#{{gr.join(\"|\")}}]\" }}.join\n      puts \"#{{{m}.{}(v, hs)}}\\t#{{gs}}\"\n",
                rb_str(&g.alias),
                n.requires,
                n.available
            ));
        }
        format!(
            "if ARGV.include?(\"--prerequisites\")\n  STDIN.each_line do |line|\n    name, v, held = line.chomp.split(\"\\t\", -1)\n    hs = held.to_s.split(\",\")\n    case name\n{arms}    end\n  end\n  exit 0\nend\n\n"
        )
    }

    // ── PHP ───────────────────────────────────────────────────────────────────

    /// The two functions of every block, as functions of the module's namespace in PHP.
    pub(crate) fn php_prereqs(&self) -> String {
        let (doc_r, doc_a) = self.prereq_doc();
        let mut o = String::new();
        for g in self.prereq_graphs() {
            let ty = self.prereq_ty(&g);
            let t = self.php_ty(&ty);
            let n = self.prereq_names(&g, "php");
            let list = |gs: Vec<Vec<String>>| -> String {
                let inner: Vec<String> = gs.iter().map(|gr| format!("[{}]", gr.iter().map(|p| self.php_value(p, &ty)).collect::<Vec<_>>().join(", "))).collect();
                format!("[{}]", inner.join(", "))
            };
            o.push_str(&format!("/**\n * {doc_r}\n *\n * @return list<list<{t}>>\n */\nfunction {}({t} $value): array\n{{\n    return match ($value) {{\n", n.requires));
            for v in &g.values {
                o.push_str(&format!("        {} => {},\n", self.php_value(v, &ty), list(g.requires(v))));
            }
            o.push_str(&format!(
                "    }};\n}}\n\n/**\n * {doc_a}\n *\n * @param list<{t}> $held\n */\nfunction {}({t} $value, array $held): bool\n{{\n    if (in_array($value, $held, true)) {{\n        return false;\n    }}\n    foreach ({}($value) as $group) {{\n        $met = false;\n        foreach ($group as $p) {{\n            if (in_array($p, $held, true)) {{\n                $met = true;\n            }}\n        }}\n        if (!$met) {{\n            return false;\n        }}\n    }}\n    return true;\n}}\n\n",
                n.available, n.requires
            ));
        }
        o
    }

    /// What the PHP runner answers with `--prerequisites`. The functions are the namespace's of
    /// the module that holds the block.
    pub(crate) fn php_prereq_mode(&self) -> String {
        let gs = self.prereq_graphs();
        if gs.is_empty() {
            return String::new();
        }
        let ns = self.php_ns();
        let mut arms = String::new();
        for g in &gs {
            let n = self.prereq_names(g, "php");
            let e = self.php_enum_fqn(&g.over);
            arms.push_str(&format!(
                "        if ($name === {}) {{\n            $x = {e}::from($v);\n            $hs = $held === '' ? [] : array_map(fn ($h) => {e}::from($h), explode(',', $held));\n            $gs = '';\n            foreach (\\{ns}\\{}($x) as $g) {{\n                $gs .= '[' . implode('|', array_map(fn ($p) => $p->value, $g)) . ']';\n            }}\n            echo \\{ns}\\{}($x, $hs) ? 'true' : 'false', \"\\t\", $gs, \"\\n\";\n        }}\n",
                super::php::php_str(&g.alias),
                n.requires,
                n.available
            ));
        }
        format!(
            "if (in_array('--prerequisites', $argv, true)) {{\n    while (($l = fgets(STDIN)) !== false) {{\n        [$name, $v, $held] = explode(\"\\t\", rtrim($l, \"\\n\"), 3);\n{arms}    }}\n    exit(0);\n}}\n\n"
        )
    }

    // ── Swift ─────────────────────────────────────────────────────────────────

    /// The two functions of every block, in Swift: a `switch` over every value, which the
    /// compiler holds to the enum.
    pub(crate) fn sw_prereqs(&self) -> String {
        let (doc_r, doc_a) = self.prereq_doc();
        let mut o = String::new();
        for g in self.prereq_graphs() {
            let ty = self.prereq_ty(&g);
            let t = self.enum_type(&g.over);
            let n = self.prereq_names(&g, "swift");
            let list = |gs: Vec<Vec<String>>| -> String {
                let inner: Vec<String> = gs.iter().map(|gr| format!("[{}]", gr.iter().map(|p| self.sw_value(p, &ty)).collect::<Vec<_>>().join(", "))).collect();
                format!("[{}]", inner.join(", "))
            };
            o.push_str(&format!("/// {doc_r}\npublic func {}(_ value: {t}) -> [[{t}]] {{\n    switch value {{\n", n.requires));
            for v in &g.values {
                let member = self.sw_value(v, &ty);
                let case = member.rsplit_once('.').map(|(_, m)| format!(".{m}")).unwrap_or(member.clone());
                o.push_str(&format!("    case {case}: return {}\n", list(g.requires(v))));
            }
            o.push_str(&format!(
                "    }}\n}}\n\n/// {doc_a}\npublic func {}(_ value: {t}, held: [{t}]) -> Bool {{\n    if held.contains(value) {{\n        return false\n    }}\n    return {}(value).allSatisfy {{ group in group.contains {{ p in held.contains(p) }} }}\n}}\n\n",
                n.available, n.requires
            ));
        }
        o
    }

    /// What the Swift runner answers with `--prerequisites`: the module's functions are in scope,
    /// built with it.
    pub(crate) fn sw_prereq_mode(&self) -> String {
        let gs = self.prereq_graphs();
        if gs.is_empty() {
            return String::new();
        }
        let mut arms = String::new();
        for g in &gs {
            let t = self.enum_type(&g.over);
            let n = self.prereq_names(g, "swift");
            arms.push_str(&format!(
                "                case {}:\n                    let x = {t}(rawValue: parts[1])!\n                    let hs = parts[2].split(separator: \",\").map {{ h in {t}(rawValue: String(h))! }}\n                    let gs = {}(x).map {{ gr in \"[\" + gr.map {{ p in p.rawValue }}.joined(separator: \"|\") + \"]\" }}.joined()\n                    print(\"\\({}(x, held: hs))\\t\\(gs)\")\n",
                str_lit(&g.alias),
                n.requires,
                n.available
            ));
        }
        format!(
            "        if CommandLine.arguments.contains(\"--prerequisites\") {{\n            while let l = readLine(strippingNewline: true) {{\n                let parts = l.split(separator: \"\\t\", maxSplits: 2, omittingEmptySubsequences: false).map(String.init)\n                guard parts.count == 3 else {{ continue }}\n                switch parts[0] {{\n{arms}                default: print(\"?\")\n                }}\n            }}\n            return\n        }}\n"
        )
    }

    // ── Java ──────────────────────────────────────────────────────────────────

    /// The two functions of every block, as static methods of the module's class in Java. The list
    /// type is written in full, so that the class of an `enums` file needs no import.
    pub(crate) fn java_prereqs(&self) -> String {
        let (doc_r, doc_a) = self.prereq_doc();
        let mut o = String::new();
        for g in self.prereq_graphs() {
            let ty = self.prereq_ty(&g);
            let t = self.java_enum(&g.over);
            let n = self.prereq_names(&g, "java");
            let list = |gs: Vec<Vec<String>>| -> String {
                let inner: Vec<String> = gs.iter().map(|gr| format!("java.util.List.of({})", gr.iter().map(|p| self.java_value(p, &ty)).collect::<Vec<_>>().join(", "))).collect();
                format!("java.util.List.of({})", inner.join(", "))
            };
            o.push_str(&format!("    /** {doc_r} */\n    public static java.util.List<java.util.List<{t}>> {}({t} value) {{\n", n.requires));
            let last = g.values.len().saturating_sub(1);
            for (i, v) in g.values.iter().enumerate() {
                if i == last {
                    o.push_str(&format!("        return {};\n", list(g.requires(v))));
                } else {
                    o.push_str(&format!("        if (value == {}) {{\n            return {};\n        }}\n", self.java_value(v, &ty), list(g.requires(v))));
                }
            }
            o.push_str(&format!(
                "    }}\n\n    /** {doc_a} */\n    public static boolean {}({t} value, java.util.List<{t}> held) {{\n        if (held.contains(value)) {{\n            return false;\n        }}\n        for (java.util.List<{t}> group : {}(value)) {{\n            boolean met = false;\n            for ({t} p : group) {{\n                if (held.contains(p)) {{\n                    met = true;\n                }}\n            }}\n            if (!met) {{\n                return false;\n            }}\n        }}\n        return true;\n    }}\n\n",
                n.available, n.requires
            ));
        }
        o
    }

    /// What the Java runner answers with `--prerequisites`, reading lines into `line`.
    pub(crate) fn java_prereq_mode(&self, line: &str) -> String {
        let gs = self.prereq_graphs();
        if gs.is_empty() {
            return String::new();
        }
        let cls = self.java_class();
        let mut arms = String::new();
        for g in &gs {
            let t = self.java_enum_outside(&g.over);
            let n = self.prereq_names(g, "java");
            arms.push_str(&format!(
                "                    case {} -> {{\n                        {t} x = {t}.from(parts[1]);\n                        java.util.List<{t}> hs = new java.util.ArrayList<>();\n                        for (String h : parts[2].split(\",\")) {{\n                            if (!h.isEmpty()) {{\n                                hs.add({t}.from(h));\n                            }}\n                        }}\n                        StringBuilder gs = new StringBuilder();\n                        for (java.util.List<{t}> gr : {cls}.{}(x)) {{\n                            java.util.List<String> ps = new java.util.ArrayList<>();\n                            for ({t} p : gr) {{\n                                ps.add(p.value());\n                            }}\n                            gs.append(\"[\").append(String.join(\"|\", ps)).append(\"]\");\n                        }}\n                        out.println({cls}.{}(x, hs) + \"\\t\" + gs);\n                    }}\n",
                super::java::java_str(&g.alias),
                n.requires,
                n.available
            ));
        }
        format!(
            "        if (args.length > 0 && args[0].equals(\"--prerequisites\")) {{\n            while (({line} = in.readLine()) != null) {{\n                String[] parts = {line}.split(\"\\t\", -1);\n                if (parts.length != 3) {{\n                    continue;\n                }}\n                switch (parts[0]) {{\n{arms}                    default -> out.println(\"?\");\n                }}\n            }}\n            return;\n        }}\n"
        )
    }

    // ── NumPy ─────────────────────────────────────────────────────────────────

    /// The groups of every value of every block, as data in the plan that the runtime answers
    /// `requires` and `available` from.
    pub(crate) fn np_prereqs(&self) -> Option<String> {
        let gs = self.prereq_graphs();
        if gs.is_empty() {
            return None;
        }
        let mut o = crate::json::Obj::new();
        for g in &gs {
            let mut vs = crate::json::Obj::new();
            for v in &g.values {
                let groups: Vec<String> = g.requires(v).iter().map(|gr| crate::json::strs(gr)).collect();
                vs = vs.raw(v, crate::json::arr(&groups));
            }
            o = o.raw(&g.alias, crate::json::Obj::new().str("enum", &g.over).raw("requires", vs.finish()).finish());
        }
        Some(o.finish())
    }

    /// What the NumPy runner answers with `--prerequisites`, from the plan loaded as `rule`.
    pub(crate) fn np_prereq_mode(&self) -> String {
        if self.prereq_graphs().is_empty() {
            return String::new();
        }
        "if \"--prerequisites\" in sys.argv[1:]:\n    for l in sys.stdin:\n        name, v, held = l.rstrip(\"\\n\").split(\"\\t\")\n        hs = [h for h in held.split(\",\") if h != \"\"]\n        gs = \"\".join(\"[\" + \"|\".join(g) + \"]\" for g in rule.requires(name, v))\n        print((\"true\" if rule.available(name, v, hs) else \"false\") + \"\\t\" + gs)\n    sys.exit(0)\n".to_string()
    }

    // ── `rulec api` ───────────────────────────────────────────────────────────

    /// `prerequisites` in `rulec api`: each block — its enum, the values that need nothing, what
    /// the others need, the ones no case can take and why, the table that orders them — and the
    /// two functions in every language that has them, by name and signature. The languages that
    /// lack them say why. `None` for a file with no block, so no other inventory changes.
    pub(crate) fn prereq_api(&self) -> Option<String> {
        use crate::json::{arr, strs, Obj};
        let gs = self.prereq_graphs();
        if gs.is_empty() {
            return None;
        }
        let enums_file = self.f.kind == crate::ast::FileKind::Enums;
        let mut out = Vec::new();
        for (g, d) in gs.iter().zip(self.f.prerequisites.iter().filter(|d| d.over.as_ref().is_some_and(|o| self.c.enums.contains_key(&o.text)))) {
            let ty = self.prereq_ty(g);
            let one = |lang: &str, req_sig: String, avail_sig: String, qualify: &dyn Fn(&str) -> String| -> String {
                let n = self.prereq_names(g, lang);
                Obj::new()
                    .str("requires", &qualify(&n.requires))
                    .str("requires_signature", &req_sig)
                    .str("available", &qualify(&n.available))
                    .str("available_signature", &avail_sig)
                    .finish()
            };
            let plain = |f: &str| f.to_string();
            let (py, ts, rs, rb, php, go, sw, java) = (
                self.prereq_names(g, "python"),
                self.prereq_names(g, "typescript"),
                self.prereq_names(g, "rust"),
                self.prereq_names(g, "ruby"),
                self.prereq_names(g, "php"),
                self.prereq_names(g, "go"),
                self.prereq_names(g, "swift"),
                self.prereq_names(g, "java"),
            );
            let t = self.enum_type(&g.over);
            let rbt = self.rbs_ty(&ty);
            let pht = self.php_ty(&ty);
            let jt = self.java_enum(&g.over);
            let m = self.rb_module();
            let ns = self.php_ns();
            let cls = self.java_class();
            let mut fns = Obj::new()
                .raw("python", one("python", format!("def {}(value: {t}) -> list[list[{t}]]:", py.requires), format!("def {}(value: {t}, held: list[{t}]) -> bool:", py.available), &plain))
                .raw("typescript", one("typescript", format!("export function {}(value: {t}): {t}[][]", ts.requires), format!("export function {}(value: {t}, held: readonly {t}[]): boolean", ts.available), &plain))
                .raw("javascript", one("javascript", format!("export function {}(value)", ts.requires), format!("export function {}(value, held)", ts.available), &plain))
                .raw("rust", one("rust", format!("pub fn {}(value: {t}) -> Vec<Vec<{t}>>", rs.requires), format!("pub fn {}(value: {t}, held: &[{t}]) -> bool", rs.available), &plain))
                .raw("ruby", one("ruby", format!("def self.{}: ({rbt}) -> Array[Array[{rbt}]]", rb.requires), format!("def self.{}: ({rbt}, Array[{rbt}]) -> bool", rb.available), &|f| format!("{m}.{f}")))
                .raw("php", one("php", format!("function {}({pht} $value): array", php.requires), format!("function {}({pht} $value, array $held): bool", php.available), &|f| format!("\\{ns}\\{f}")))
                .raw("go", one("go", format!("func {}(value {t}) [][]{t}", go.requires), format!("func {}(value {t}, held []{t}) bool", go.available), &plain))
                .raw("swift", one("swift", format!("public func {}(_ value: {t}) -> [[{t}]]", sw.requires), format!("public func {}(_ value: {t}, held: [{t}]) -> Bool", sw.available), &plain))
                .raw("java", one("java", format!("public static java.util.List<java.util.List<{jt}>> {}({jt} value)", java.requires), format!("public static boolean {}({jt} value, java.util.List<{jt}> held)", java.available), &|f| format!("{cls}.{f}")));
            let mut without = Obj::new();
            if enums_file {
                without = without.str("numpy", &tr!("`enums` のファイルは NumPy の計画を持たない", "an `enums` file has no NumPy plan"));
            } else {
                fns = fns.raw(
                    "numpy",
                    Obj::new()
                        .str("requires", &format!("Rule.requires({:?}, value)", g.alias))
                        .str("requires_signature", "def requires(self, name: str, value: str) -> list[list[str]]")
                        .str("available", &format!("Rule.available({:?}, value, held)", g.alias))
                        .str("available_signature", "def available(self, name: str, value: str, held: Iterable[str]) -> bool")
                        .finish(),
                );
            }
            for id in ["sql", "wasm"] {
                without = without.str(id, &tr!("入口が一つで、二つ目の関数を置く場所が無い", "one entry, and no room for a second function"));
            }
            let requires: Vec<(&String, &Vec<Vec<String>>)> = g.values.iter().filter_map(|v| g.groups.get(v).map(|gr| (v, gr))).collect();
            let mut req = Obj::new();
            for (v, gr) in requires {
                req = req.raw(v, arr(&gr.iter().map(|x| strs(x)).collect::<Vec<_>>()));
            }
            let mut never = Obj::new();
            for v in g.values.iter().filter(|v| g.never.contains_key(*v)) {
                never = never.str(v, &g.never[v]);
            }
            out.push(
                Obj::new()
                    .str("name", &g.name)
                    .str("alias", &g.alias)
                    .str("over", &g.over)
                    .bool("open", d.open.is_some())
                    .raw("start", strs(&g.start))
                    .raw("requires", req.finish())
                    .raw("never", never.finish())
                    .raw("ordered_by", g.ordered_by.as_ref().map(|t| crate::json::quote(t)).unwrap_or_else(|| "null".into()))
                    .raw("functions", fns.finish())
                    .raw("without", without.finish())
                    .finish(),
            );
        }
        Some(arr(&out))
    }

    // ── The runners of an `enums` file ────────────────────────────────────────

    /// The runners of an `enums` file that holds a `prerequisites` block: one beside its module in
    /// each language that has one, answering `--prerequisites` and nothing else, so that `rulec
    /// test` holds the functions of the file's module to the reference as it holds a rule's. Built
    /// and run by the same plans as a rule's runners. Nothing for a file with no block.
    pub(crate) fn enums_runner_files(&self) -> Vec<(String, String)> {
        if self.prereq_graphs().is_empty() {
            return Vec::new();
        }
        let alias = pub_name(&self.f.name);
        let pkg = crate::backend::go_package(&alias);
        let cls = self.java_class();
        let ver = ritsu_emit::header::VERSION;
        let head = |c: &str| format!("{c} Code generated by rulec {ver}. DO NOT EDIT.\n");
        let python = format!("{}import sys\n\nimport {alias} as m\n\n{}", head("#"), self.py_prereq_mode());
        let mut imports: Vec<String> = Vec::new();
        let mut owners: Vec<(String, Vec<String>)> = Vec::new();
        let ts_mode = self.ts_prereq_mode("lines", &mut imports, &mut owners);
        let typescript = format!(
            "{}import {{ readFileSync }} from \"node:fs\";\nimport {{ {} }} from \"./{alias}.ts\";\n\nconst lines = readFileSync(0, \"utf8\").split(\"\\n\");\n{ts_mode}",
            head("//"),
            imports.join(", ")
        );
        let javascript = super::strip_types(&typescript).replace(".ts\";", ".mjs\";");
        let rust = format!(
            "{}#![allow(non_snake_case, uncommon_codepoints, unused_parens, dead_code)]\n\n#[path = \"{alias}.rs\"]\nmod r;\n\nuse std::io::Read;\n\nfn main() {{\n    let mut src = String::new();\n    std::io::stdin().read_to_string(&mut src).unwrap();\n{}}}\n",
            head("//"),
            self.rs_prereq_mode()
        );
        let ruby = format!("{}# frozen_string_literal: true\n\nrequire_relative \"{alias}\"\n\n{}", head("#"), self.rb_prereq_mode());
        let php = format!(
            "<?php\n\n{}\ndeclare(strict_types=1);\n\nini_set('display_errors', 'stderr');\n\nrequire_once __DIR__ . '/{alias}.php';\n\n{}",
            head("//"),
            self.php_prereq_mode()
        );
        let go = format!(
            "{}package main\n\nimport (\n\t\"bufio\"\n\t\"fmt\"\n\t\"os\"\n\t\"strings\"\n\n\tr \"{pkg}\"\n)\n\nfunc main() {{\n\tsc := bufio.NewScanner(os.Stdin)\n\tsc.Buffer(make([]byte, 1<<20), 1<<20)\n{}}}\n",
            head("//"),
            self.go_prereq_mode()
        );
        let swift = format!("{}import Foundation\n\n@main\nenum Runner {{\n    static func main() throws {{\n{}    }}\n}}\n", head("//"), self.sw_prereq_mode());
        let java = format!(
            "{}\nimport java.io.BufferedReader;\nimport java.io.IOException;\nimport java.io.InputStreamReader;\nimport java.io.PrintStream;\nimport java.nio.charset.StandardCharsets;\n\npublic final class {cls}Runner {{\n    private {cls}Runner() {{\n    }}\n\n    public static void main(String[] args) throws IOException {{\n        BufferedReader in = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));\n        PrintStream out = new PrintStream(System.out, true, StandardCharsets.UTF_8);\n        String line;\n{}    }}\n}}\n",
            head("//"),
            self.java_prereq_mode("line")
        );
        vec![
            (format!("python/{alias}_runner.py"), python),
            (format!("typescript/{alias}_runner.ts"), typescript),
            (format!("javascript/{alias}_runner.mjs"), javascript),
            (format!("rust/{alias}_runner.rs"), rust),
            (format!("ruby/{alias}_runner.rb"), ruby),
            (format!("php/{alias}_runner.php"), php),
            (format!("go/{pkg}runner/main.go"), go),
            (format!("go/{pkg}runner/go.mod"), self.go_mod(&pkg, true)),
            (format!("swift/{alias}_runner.swift"), swift),
            (format!("java/{cls}Runner.java"), java),
        ]
    }

    // ── The vectors ───────────────────────────────────────────────────────────

    /// The input of a runner's `--prerequisites` and what it has to print, from the reference:
    /// for each value of each block, a handful of what may be held — nothing; one value of each
    /// group, which meets them all; that less the value of one group; each other value of an
    /// `or`; the value itself besides; every other value. `None` for a file with no block.
    pub fn prereq_input(&self) -> Option<(String, String)> {
        let gs = self.prereq_graphs();
        if gs.is_empty() {
            return None;
        }
        let (mut input, mut expected) = (String::new(), String::new());
        for g in &gs {
            for (v, held) in prereq_cases(g) {
                input.push_str(&format!("{}\t{v}\t{}\n", g.alias, held.join(",")));
                expected.push_str(&format!("{}\t{}\n", g.available(&v, &held), groups_text(&g.requires(&v))));
            }
        }
        Some((input, expected))
    }
}

/// What is held in the cases of one block, value by value, each list once.
pub(crate) fn prereq_cases(g: &Graph) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    let mut add = |v: &str, mut held: Vec<String>| {
        // in the enum's order, each once
        held.sort_by_key(|h| g.values.iter().position(|x| x == h));
        held.dedup();
        if !out.iter().any(|(w, h)| w == v && *h == held) {
            out.push((v.to_string(), held));
        }
    };
    for v in &g.values {
        let groups = g.requires(v);
        add(v, Vec::new());
        let firsts: Vec<String> = groups.iter().filter_map(|gr| gr.first().cloned()).collect();
        add(v, firsts.clone());
        for (k, gr) in groups.iter().enumerate() {
            if let Some(p) = gr.first() {
                // one group left unmet, unless another group's value meets it too
                add(v, firsts.iter().enumerate().filter(|(j, q)| *j != k || *q != p).map(|(_, q)| q.clone()).collect());
            }
            for alt in gr.iter().skip(1) {
                let mut h = firsts.clone();
                if let Some(slot) = h.get_mut(k) {
                    *slot = alt.clone();
                }
                add(v, h);
            }
        }
        let mut with_self = firsts.clone();
        with_self.push(v.clone());
        add(v, with_self);
        add(v, g.values.iter().filter(|x| *x != v).cloned().collect());
    }
    out
}

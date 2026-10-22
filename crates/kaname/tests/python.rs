//! The Python kaname generates (DESIGN 10, 16.4), held to the reference interpreter. Every module of
//! the examples and of `tests/modules/` goes into one package, beside the Python rulec, koyomi
//! and chobo write for the rules, the dates and the books they read (each language's own generator,
//! called as `ritsu gen` calls it), with the messages in English and in Japanese:
//!
//! - `mypy --strict` (`tools/requirements.txt`) passes the package and the harness;
//! - the harness runs every vector of every function, the inputs outside what the parameters declare,
//!   and the values of types that hold themselves nested as deep as a host may give one and one
//!   deeper (DESIGN 28.1), and every answer is the reference interpreter's: the value, or the kind,
//!   the module, the line and the message of the failure (a case that does not stop in the
//!   interpreter, past its 200 calls, is not run);
//! - a call of kaname is one frame of Python (measured with `sys.setrecursionlimit`), a call of a
//!   function value too (DESIGN 27.3), so that a recursion as deep as the reference interpreter goes
//!   (200 calls, DESIGN 8) answers within CPython's own limit; and a value a host gives that nests a
//!   type that holds itself as deep as it may (200, DESIGN 28.1) is held at the entry and compared
//!   whole within it too.
//!
//! Without python3, the test says SKIP and passes; without mypy, the package is run but not
//! type-checked (SKIP).

mod common;

use common::targets::*;
use kaname::r#gen::Target;
use ritsu_base::text::Lang;
use ritsu_testkit::{Need, TempDir};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

const LIMIT: Duration = Duration::from_secs(900);

/// The probe of the frames a call of kaname takes: the smallest limit of recursion at which
/// `countdown(n)` answers, for two depths, and what CPython's default limit is; and the same of
/// `steps_down(n)`, whose recursion goes through a function value given to another function (three
/// calls of kaname for each step down: `steps_down`, `via`, and the lambda that calls `steps_down`); and
/// of `same(a, b)` given two chains of categories as deep, which its entry holds to their nesting and
/// their fields and which it compares whole (DESIGN 28.1).
const PROBE: &str = r#""""How deep the generated Python goes: the frames a call of kaname takes."""

import sys
from typing import Callable

from pkg.glue import hof, rec, trees


def lowest(call: Callable[..., int], n: int) -> int:
    """The smallest limit of recursion at which call(n=n) answers (the function itself, so that the
    probe adds no frame of its own)."""
    lo, hi = 64, 20_000
    while lo < hi:
        mid = (lo + hi) // 2
        sys.setrecursionlimit(mid)
        try:
            call(n=n)
            answered = True
        except RecursionError:
            answered = False
        finally:
            sys.setrecursionlimit(20_000)
        if answered:
            hi = mid
        else:
            lo = mid + 1
    return lo


def chain(n: int) -> trees.Category:
    """Categories n deep, each the only one under the one above."""
    c = trees.Category(name="a", children=[])
    for _ in range(n - 1):
        c = trees.Category(name="a", children=[c])
    return c


def lowest_same(n: int) -> int:
    """The smallest limit of recursion at which `same` answers for two chains n deep."""
    x, y = chain(n), chain(n)
    lo, hi = 64, 20_000
    while lo < hi:
        mid = (lo + hi) // 2
        sys.setrecursionlimit(mid)
        try:
            answered = trees.same(a=x, b=y)
        except RecursionError:
            answered = False
        finally:
            sys.setrecursionlimit(20_000)
        if answered:
            hi = mid
        else:
            lo = mid + 1
    return lo


default = sys.getrecursionlimit()
a, b = lowest(rec.countdown, 99), lowest(rec.countdown, 199)
c, d = lowest(hof.steps_down, 33), lowest(hof.steps_down, 66)
e, f = lowest_same(100), lowest_same(200)
print(default, a, b, c, d, e, f)
"#;

/// A call 200 deep (199 below the first), at CPython's own limit; `steps_down(66)`, 199 calls deep
/// through a function value; and two chains of categories 200 deep, held at the entry and compared.
const AT_DEFAULT: &str = "from pkg.glue import hof, rec, trees\n\n\ndef chain(n: int) -> trees.Category:\n    c = trees.Category(name=\"a\", children=[])\n    for _ in range(n - 1):\n        c = trees.Category(name=\"a\", children=[c])\n    return c\n\n\nprint(rec.countdown(n=199), hof.steps_down(n=66), trees.same(a=chain(200), b=chain(200)))\n";

#[test]
fn the_generated_python_answers_as_the_reference_does() {
    if !ritsu_testkit::need(Need::Python) {
        return;
    }
    if !ritsu_testkit::tools::runs("python3", &["--version"]) {
        ritsu_testkit::skip("python3 is not on the PATH; the Python kaname generates is not run");
        return;
    }
    let mypy = ritsu_testkit::tools::find("MYPY", Some(Path::new("tools/.venv/bin/mypy")), "mypy", &["--version"]);
    let python = ritsu_testkit::tools::version("python3", "--version");
    let t = TempDir::new("kaname-python");
    let root = keep_in().unwrap_or_else(|| t.path().to_path_buf());
    let checker = mypy.clone();
    let report = deep(move || {
        let modules = modules();
        let checked: Vec<_> = modules.iter().map(|(c, _)| c.clone()).collect();
        let mut out = Vec::new();
        for lang in [Lang::En, Lang::Ja] {
            let dir = root.join(format!("py-{}", if lang == Lang::Ja { "ja" } else { "en" }));
            write_package(&dir, Target::Python, lang, &modules);
            std::fs::write(dir.join("harness.py"), kaname::r#gen::harness::program(&checked, Target::Python, PY_NAME, GO_PACKAGE)).unwrap();
            let (cases, counts) = cases(&modules, lang, Target::Python);
            std::fs::write(dir.join("cases.json"), cases_json(&cases)).unwrap();
            if let Some(mypy) = &checker {
                let mut c = Command::new(mypy);
                c.args(["--strict", "--no-incremental", "--cache-dir"]).arg(dir.join(".mypy_cache")).args(["harness.py", PY_NAME]).current_dir(&dir);
                let r = ritsu_testkit::run(&mut c, LIMIT);
                assert!(r.ok, "mypy --strict finds fault with the Python kaname generates ({lang:?}):\n{}", r.both());
            }
            let r = ritsu_testkit::run(Command::new("python3").arg("harness.py").arg("cases.json").current_dir(&dir), LIMIT);
            assert!(r.ok, "python3 failed ({lang:?}):\n{}", r.both());
            let (agree, wrong) = compare(&cases, &r.stdout);
            assert!(wrong.is_empty(), "the Python ({lang:?}) answers otherwise than the reference:\n{}", wrong.join("\n"));
            out.push(counts.line(lang, agree, modules.len()));
            if lang == Lang::En {
                // how deep it goes, in the package of English
                std::fs::write(dir.join("probe.py"), PROBE).unwrap();
                let r = ritsu_testkit::run(Command::new("python3").arg("probe.py").current_dir(&dir), LIMIT);
                assert!(r.ok, "the probe of the depth failed:\n{}", r.both());
                let ns: Vec<usize> = r.stdout.split_whitespace().map(|x| x.parse().unwrap()).collect();
                let (default, at99, at199, at33, at66, deep100, deep200) = (ns[0], ns[1], ns[2], ns[3], ns[4], ns[5], ns[6]);
                // 100 calls more of kaname take this many frames more of Python
                let per = (at199 - at99) as f64 / 100.0;
                assert!(per <= 1.0, "a call of kaname takes {per} frames of Python");
                assert!(at199 <= default, "200 calls of kaname ask a limit of {at199}, past CPython's {default}");
                // 33 steps more of `steps_down` are 99 calls more of kaname, a third of them of a
                // function value (DESIGN 27.3)
                let per_fn = (at66 - at33) as f64 / 99.0;
                assert!(per_fn <= 1.0, "a call of kaname through a function value takes {per_fn} frames of Python");
                assert!(at66 <= default, "199 calls of kaname through function values ask a limit of {at66}, past CPython's {default}");
                std::fs::write(dir.join("at_default.py"), AT_DEFAULT).unwrap();
                let r = ritsu_testkit::run(Command::new("python3").arg("at_default.py").current_dir(&dir), LIMIT);
                assert!(r.ok && r.stdout.trim() == "199 66 True", "countdown(199), steps_down(66) and same of two chains 200 deep at CPython's own limit:\n{}", r.both());
                // a value nested 100 deeper takes this many frames more (DESIGN 28.1)
                let per_nest = (deep200 - deep100) as f64 / 100.0;
                assert!(deep200 <= default, "same of two chains 200 deep asks a limit of {deep200}, past CPython's {default}");
                out.push(format!("a call of kaname is {per} frame of Python: 200 calls answer at a limit of {at199} (100 at {at99}), within CPython's {default}"));
                out.push(format!("a call through a function value is {per_fn} frame of Python too: steps_down(66), 199 calls, answers at a limit of {at66} (steps_down(33), 100 calls, at {at33})"));
                out.push(format!("a value nested one deeper is {per_nest} frames of Python held at the entry and compared: same of two chains of categories 200 deep answers at a limit of {deep200} (100 deep at {deep100}), within CPython's {default}"));
            }
        }
        out
    });
    match &mypy {
        Some(m) => println!("mypy --strict ({}) passes the package and the harness in English and in Japanese", ritsu_testkit::tools::version(m, "--version")),
        None => ritsu_testkit::skip("mypy is not installed (tools/requirements.txt); the Python is run but not type-checked"),
    }
    println!("{python}");
    for line in report {
        println!("{line}");
    }
}

//! Where a path written in a file leads (DESIGN 17.6): in a module, from the root of the module the
//! file is in, into a dependency by `@<name>/`, never out of the module nor into a module below
//! without an `@`, and each file of the tree written one way (the fewest `@`, then the order of the
//! `require`s); the view of the layer that reads the files (`R/@pricing/…`); the root, the walk and
//! the shown path of a module.
//!
//! And M14: in a tree with no `ritsu.mod`, `refer` is `dir.join`, `join` is `paths::join`, the
//! functions of `paths` answer as they did (their code before modules is copied below), and the
//! layer reads what it read, byte for byte, names that start with `@` too.

use ritsu_base::fs::{self, Memory};
use ritsu_base::modules::graph::{self, Problem};
use ritsu_base::modules::lock::Lock;
use ritsu_base::modules::store::{self, Store};
use ritsu_base::modules::{self, Refused};
use ritsu_base::paths::{self, PathError, Shown};
use ritsu_testkit::TempDir;
use std::path::{Path, PathBuf};
use std::rc::Rc;

// ── The functions of `paths` as they were before modules, on the files without the view ──────

mod before {
    use ritsu_base::fs;
    use ritsu_base::paths;
    use std::path::{Component, Path, PathBuf};

    pub fn absolute(p: &Path) -> PathBuf {
        let joined = if paths::rooted(p) { p.to_path_buf() } else { fs::current_dir().unwrap_or_default().join(p) };
        let mut out = PathBuf::new();
        for c in joined.components() {
            match c {
                Component::CurDir => {}
                Component::ParentDir => {
                    out.pop();
                }
                other => out.push(other.as_os_str()),
            }
        }
        out
    }

    pub fn find_root(start: &Path) -> PathBuf {
        let abs = absolute(start);
        let dir = if fs::raw::metadata(&abs).is_ok_and(|m| m.is_dir()) { abs.clone() } else { abs.parent().map(Path::to_path_buf).unwrap_or_else(|| abs.clone()) };
        for d in dir.ancestors() {
            if fs::raw::metadata(&d.join(".git")).is_ok() {
                return d.to_path_buf();
            }
        }
        dir
    }

    pub fn shown(root: &Path, cwd: &Path, given: &str, p: &str) -> String {
        let (root, cwd, absolute) = (absolute(root), absolute(cwd), paths::rooted(Path::new(given)));
        let (p, slash) = match p.strip_suffix('/') {
            Some(q) if !q.is_empty() => (q, "/"),
            _ => (p, ""),
        };
        let target = paths::on_disk(&root, p);
        let s = if absolute { target.to_string_lossy().to_string() } else { paths::between(&cwd, &target) };
        format!("{s}{slash}")
    }

    pub fn walk(root: &Path, dir: &str, except: &[String], out: &mut Vec<String>) {
        let disk = paths::on_disk(root, dir);
        let Ok(meta) = fs::raw::symlink_metadata(&disk) else { return };
        if meta.is_file() {
            if !except.iter().any(|e| paths::contains(e, dir)) && !paths::has_skipped_part(dir) {
                out.push(dir.to_string());
            }
            return;
        }
        if !meta.is_dir() {
            return;
        }
        let Ok(rd) = fs::raw::read_dir(&disk) else { return };
        let mut names: Vec<String> = rd.into_iter().map(|(n, _)| n.to_string_lossy().to_string()).collect();
        names.sort();
        for n in names {
            if paths::skipped_name(&n) {
                continue;
            }
            let p = if dir == "." { n.clone() } else { format!("{dir}/{n}") };
            if except.iter().any(|e| paths::contains(e, &p)) {
                continue;
            }
            let Ok(m) = fs::raw::symlink_metadata(&paths::on_disk(root, &p)) else { continue };
            if m.is_dir() {
                walk(root, &p, except, out);
            } else if m.is_file() {
                out.push(p);
            }
        }
    }
}

/// What each reading function of the layer answers at `p`, written out: through the view (`now`)
/// or without it (what the layer read before modules).
fn answers(p: &Path, now: bool) -> String {
    let err = |e: std::io::Error| format!("error {:?} {e}", e.kind());
    let mut out = Vec::new();
    let read = if now { fs::read(p) } else { fs::raw::read(p) };
    out.push(format!("read {:?}", read.map_err(err)));
    let text = if now { fs::read_to_string(p) } else { fs::raw::read_to_string(p) };
    out.push(format!("read_to_string {:?}", text.map_err(err)));
    let meta = if now { fs::metadata(p) } else { fs::raw::metadata(p) };
    out.push(format!("metadata {:?}", meta.map_err(err)));
    let link = if now { fs::symlink_metadata(p) } else { fs::raw::symlink_metadata(p) };
    out.push(format!("symlink_metadata {:?}", link.map_err(err)));
    let names: Result<Vec<String>, String> = if now {
        fs::read_dir(p).map_err(err).and_then(|rd| {
            rd.map(|e| e.map_err(err).map(|e| format!("{:?} {:?} {:?}", e.file_name(), e.file_type().map_err(err), e.path().as_os_str())))
                .collect()
        })
    } else {
        fs::raw::read_dir(p).map_err(err).map(|v| v.into_iter().map(|(n, k)| format!("{n:?} Ok({:?}) {:?}", meta_of(k), p.join(&n).as_os_str())).collect())
    };
    out.push(format!("read_dir {names:?}"));
    let canonical = if now { fs::canonicalize(p) } else { fs::raw::canonicalize(p) };
    out.push(format!("canonicalize {:?}", canonical.map(|c| c.into_os_string()).map_err(err)));
    let (exists, file, dir) = if now {
        (fs::exists(p), fs::is_file(p), fs::is_dir(p))
    } else {
        let m = fs::raw::metadata(p);
        (m.is_ok(), m.as_ref().is_ok_and(|m| m.is_file()), m.as_ref().is_ok_and(|m| m.is_dir()))
    };
    out.push(format!("exists {exists} is_file {file} is_dir {dir}"));
    out.join("\n")
}

/// An entry's kind as the layer's `file_type` writes it (a symbolic link is not followed).
fn meta_of(k: fs::Kind) -> fs::Meta {
    fs::Meta { kind: k, len: 0 }
}

/// A tree with no `ritsu.mod`, with names that start with `@` at every depth, under `base` (on the
/// disk, or in memory): the files, a path from `base` each.
const PLAIN: &[(&str, &str)] = &[
    (".git/HEAD", "ref: refs/heads/main\n"),
    ("@x/a.rule", "rule a\n"),
    ("@y.rule", "rule y\n"),
    ("sub/@z/b.rule", "rule b\n"),
    ("sub/c.rule", "rule c\n"),
    ("日本語/@規則.rule", "規則\n"),
    ("target/out.txt", "skipped\n"),
    ("@x/.hidden/h.rule", "skipped\n"),
];

/// The paths whose answers are compared, from `base`.
const LOOKED_AT: &[&str] = &[
    "",
    "@x",
    "@x/a.rule",
    "@x/../@y.rule",
    "@x/./a.rule/",
    "@y.rule",
    "@y.rule/x",
    "@nothing",
    "@nothing/a.rule",
    "sub",
    "sub/@z",
    "sub/@z/b.rule",
    "sub/@z/../c.rule",
    "日本語/@規則.rule",
    "@x/.hidden",
];

/// The written paths `refer` and `join` are asked for, outside a module.
const WRITTEN: &[&str] = &["a.rule", "@x/a.rule", "../@y.rule", "./@x", "@x/..", "@x/../../..", "", "/etc/hosts", "sub//@z/", "日本語/@規則.rule", "..", "."];

#[test]
fn outside_a_module_the_layer_reads_what_it_read() {
    let t = TempDir::new("resolve-m14-disk");
    for (p, b) in PLAIN {
        t.write(p, b);
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink("@x", t.path().join("link")).unwrap();
    for p in LOOKED_AT.iter().chain(&["link", "link/a.rule"]) {
        let p = t.path().join(p);
        assert_eq!(answers(&p, true), answers(&p, false), "{}", p.display());
    }
    // what is written with an `@` in its path is written, as before
    fs::create_dir_all(t.path().join("@new/@deeper")).unwrap();
    fs::write(t.path().join("@new/@deeper/x.rule"), "x").unwrap();
    assert_eq!(std::fs::read(t.path().join("@new/@deeper/x.rule")).unwrap(), b"x");
    // and in memory
    let m = Memory::new("/plain/sub");
    for (p, b) in PLAIN {
        m.add(format!("/plain/{p}"), *b);
    }
    let m = Rc::new(m);
    fs::with(m.clone(), || {
        for p in LOOKED_AT {
            for p in [PathBuf::from("/plain").join(p), PathBuf::from(p), PathBuf::from("..").join(p)] {
                assert_eq!(answers(&p, true), answers(&p, false), "{}", p.display());
            }
        }
        fs::create_dir_all("@new").unwrap();
        fs::write("@new/x.rule", "x").unwrap();
    });
    assert_eq!(m.written(), [("@new/x.rule".to_string(), b"x".to_vec())]);
}

#[test]
fn outside_a_module_the_paths_answer_as_they_did() {
    let t = TempDir::new("resolve-m14-paths");
    for (p, b) in PLAIN {
        t.write(p, b);
    }
    t.write("nogit/@w/d.rule", "rule d\n");
    let root = t.path();
    let except = ["sub/@z".to_string()];
    let check = |root: &Path| {
        for dir in [".", "@x", "sub", "sub/@z", "@y.rule", "@nothing"] {
            for except in [&[][..], &except[..]] {
                let (mut now, mut then) = (Vec::new(), Vec::new());
                let mut modules = Vec::new();
                paths::walk_noting(root, dir, except, &mut now, &mut modules);
                before::walk(root, dir, except, &mut then);
                assert_eq!(now, then, "{dir} {except:?}");
                assert!(modules.is_empty());
            }
        }
        for p in LOOKED_AT.iter().chain(&["@x/a.rule/..", "@nothing/@more/x"]) {
            for start in [root.join(p), PathBuf::from(p)] {
                assert_eq!(paths::find_root(&start).as_os_str(), before::find_root(&start).as_os_str(), "{}", start.display());
                assert_eq!(paths::absolute(&start).as_os_str(), before::absolute(&start).as_os_str(), "{}", start.display());
            }
        }
        for cwd in [root.to_path_buf(), root.join("sub"), root.join("@x"), root.join("sub/@z")] {
            for given in ["@x/a.rule", "/abs/@x"] {
                let shown = Shown::run_in(root, &cwd, given);
                for p in [".", "@x", "@x/", "@x/a.rule", "@y.rule", "sub/@z/b.rule", "@nothing/x", "日本語/@規則.rule"] {
                    assert_eq!(shown.path(p), before::shown(root, &cwd, given, p), "{} {given} {p}", cwd.display());
                }
            }
        }
    };
    check(root);
    check(&root.join("nogit"));
    let m = Memory::new("/plain/sub/@z");
    for (p, b) in PLAIN {
        m.add(format!("/plain/{p}"), *b);
    }
    m.add("/nogit/@w/d.rule", "rule d\n");
    fs::with(Rc::new(m), || {
        check(Path::new("/plain"));
        check(Path::new("/nogit"));
    });
}

#[test]
fn outside_a_module_refer_is_dir_join_and_join_is_paths_join() {
    let t = TempDir::new("resolve-m14-refer");
    for (p, b) in PLAIN {
        t.write(p, b);
    }
    for dir in [t.path().to_path_buf(), t.path().join("@x"), t.path().join("sub/@z"), t.path().join("@nothing"), PathBuf::from("relative/@dir")] {
        for w in WRITTEN {
            let got = modules::refer(&dir, w).unwrap_or_else(|e| panic!("{w}: {}", e.text().en));
            assert_eq!(got.as_os_str(), dir.join(w).as_os_str(), "{} {w}", dir.display());
        }
    }
    for from in ["x.rule", "@x/a.rule", "sub/@z/b.rule", "@nothing/@more/x.rule"] {
        for w in WRITTEN {
            let got = match modules::join(t.path(), from, w) {
                Ok(s) => Ok(s),
                Err(Refused::Path(e, written)) => {
                    assert_eq!(written, *w);
                    Err(e)
                }
                Err(other) => panic!("{from} {w}: {}", other.text().en),
            };
            assert_eq!(got, paths::join(&paths::parent(from), w), "{from} {w}");
        }
    }
    assert!(matches!(modules::join(t.path(), "x.rule", "../out"), Err(Refused::Path(PathError::Outside, _))));
}

// ── A module and its dependencies by paths ──────────────────────────────

/// A worktree (`.git`) with the module `shop`, which requires `pricing` and `tax` by their paths,
/// and `pricing` requires `tax`; `shop` has a module of its own below it (`inner`) and a file that
/// starts with `@` in a directory below its root.
fn worktree(t: &TempDir) -> PathBuf {
    for (p, b) in [
        (".git/HEAD", "ref: refs/heads/main\n"),
        ("shop/ritsu.mod", "module example.com/acme/shop\nversion 0.1.0\n\nrequire pricing path \"../pricing\"\nrequire tax path \"../tax\"\n"),
        ("shop/rules/order_total.rule", "order total\n"),
        ("shop/sub/@notes.md", "notes\n"),
        ("shop/inner/ritsu.mod", "module inner\n"),
        ("shop/inner/x.rule", "inner x\n"),
        ("pricing/ritsu.mod", "module example.com/acme/pricing\nversion 1.1.0\n\nrequire tax path \"../tax\"\n"),
        ("pricing/rules/shipping_fee.rule", "shipping fee\n"),
        ("pricing/shared.rule", "shared\n"),
        ("tax/ritsu.mod", "module example.com/acme/tax\nversion 1.0.0\n"),
        ("tax/rules/vat_rate.rule", "vat rate\n"),
        ("plain/@x/a.rule", "plain a\n"),
    ] {
        t.write(p, b);
    }
    t.path().join("shop")
}

/// What `refer` refuses, in English.
fn refused(r: Result<PathBuf, Refused>) -> String {
    match r {
        Ok(p) => panic!("not refused: {}", p.display()),
        Err(e) => e.text().en,
    }
}

#[test]
fn in_a_module_a_path_is_written_one_way() {
    let t = TempDir::new("resolve-refer");
    let shop = worktree(&t);
    let rules = shop.join("rules");
    let ok = |dir: &Path, w: &str| modules::refer(dir, w).unwrap_or_else(|e| panic!("{w}: {}", e.text().en));
    // the module's own files, from the file's directory
    assert_eq!(ok(&rules, "../sub/@notes.md"), shop.join("sub/@notes.md"));
    assert_eq!(ok(&rules, "./x.rule"), shop.join("rules/x.rule"));
    // a dependency's, from the root of the module, wherever the file is
    assert_eq!(ok(&rules, "@pricing/rules/shipping_fee.rule"), shop.join("@pricing/rules/shipping_fee.rule"));
    assert_eq!(ok(&shop.join("sub"), "@pricing/rules/../shared.rule"), shop.join("@pricing/shared.rule"));
    assert_eq!(ok(&rules, "../@pricing/shared.rule"), shop.join("@pricing/shared.rule"));
    // the fewest `@`: `shop` requires `tax` itself
    assert_eq!(ok(&rules, "@pricing/@tax/rules/vat_rate.rule"), shop.join("@tax/rules/vat_rate.rule"));
    // in a dependency, its own files and its dependencies, written from the module at hand
    let dep = shop.join("@pricing/rules");
    assert_eq!(ok(&dep, "../shared.rule"), shop.join("@pricing/shared.rule"));
    assert_eq!(ok(&dep, "@tax/rules/vat_rate.rule"), shop.join("@tax/rules/vat_rate.rule"));
    // `./@…` in a directory below the root is a file there; a leading `@` is a dependency
    assert_eq!(ok(&shop.join("sub"), "./@notes.md"), shop.join("sub/@notes.md"));
    let shop_name = "example.com/acme/shop";
    assert_eq!(
        refused(modules::refer(&shop.join("sub"), "@notes.md")),
        format!("the `@notes.md` of `@notes.md` is no dependency: the `ritsu.mod` of the module `{shop_name}` has no `require notes.md …`")
    );
    assert_eq!(refused(modules::refer(&shop, "@nothing/x.rule")), format!("the `@nothing` of `@nothing/x.rule` is no dependency: the `ritsu.mod` of the module `{shop_name}` has no `require nothing …`"));
    // out of the module, out of a dependency, and back out through `@<name>/..`
    let outside = |w: &str, m: &str| format!("`{w}` leads outside the module `{m}`; a file in a module names files under the module's root, and in a dependency by `@<name>/`");
    assert_eq!(refused(modules::refer(&rules, "../../pricing/shared.rule")), outside("../../pricing/shared.rule", shop_name));
    assert_eq!(refused(modules::refer(&rules, "../..")), outside("../..", shop_name));
    assert_eq!(refused(modules::refer(&shop, "@pricing/../rules/order_total.rule")), outside("@pricing/../rules/order_total.rule", shop_name));
    assert_eq!(refused(modules::refer(&dep, "../../rules/order_total.rule")), outside("../../rules/order_total.rule", "example.com/acme/pricing"));
    assert_eq!(refused(modules::refer(&dep, "../@tax/../x")), outside("../@tax/../x", "example.com/acme/pricing"));
    // into a module below without an `@`
    assert_eq!(
        refused(modules::refer(&rules, "../inner/x.rule")),
        "`../inner/x.rule` leads into `inner`, a directory below with a `ritsu.mod` of its own, without an `@`; require that module, and name its files by `@<name>/`"
    );
    // that module's own files are its own
    assert_eq!(ok(&shop.join("inner"), "x.rule"), shop.join("inner/x.rule"));
    // absolute and empty
    assert_eq!(refused(modules::refer(&rules, "/etc/hosts")), "`/etc/hosts` is an absolute path; in a module, write a path from the directory of the file it is in, or one that starts with `@<name>/`");
    assert_eq!(refused(modules::refer(&rules, "")), "the path is empty");
    // the same answers in Japanese
    assert_eq!(
        modules::refer(&shop, "@nothing/x.rule").unwrap_err().text().ja,
        "`@nothing/x.rule` の `@nothing` は依存ではありません。モジュール `example.com/acme/shop` の `ritsu.mod` に `require nothing …` がありません"
    );
    // a file outside any module, beside them, is read as before
    assert_eq!(ok(&t.path().join("plain"), "@x/a.rule"), t.path().join("plain/@x/a.rule"));
}

#[test]
fn join_answers_the_canonical_path_from_the_root() {
    let t = TempDir::new("resolve-join");
    let shop = worktree(&t);
    let join = |from: &str, w: &str| modules::join(&shop, from, w).map_err(|e| e.text().en);
    assert_eq!(join("rules/order_total.rule", "@pricing/rules/shipping_fee.rule"), Ok("@pricing/rules/shipping_fee.rule".into()));
    assert_eq!(join("rules/order_total.rule", "../@pricing/@tax/rules/vat_rate.rule"), Ok("@tax/rules/vat_rate.rule".into()));
    assert_eq!(join("rules/order_total.rule", "x.rule"), Ok("rules/x.rule".into()));
    assert_eq!(join("@pricing/rules/shipping_fee.rule", "../shared.rule"), Ok("@pricing/shared.rule".into()));
    assert_eq!(join("@pricing/rules/shipping_fee.rule", "@tax/rules/vat_rate.rule"), Ok("@tax/rules/vat_rate.rule".into()));
    assert!(join("@pricing/rules/shipping_fee.rule", "../../rules/order_total.rule").unwrap_err().contains("leads outside the module `example.com/acme/pricing`"));
    // the root of a run above the module: the path from there
    assert_eq!(modules::join(t.path(), "shop/rules/order_total.rule", "@pricing/shared.rule").map_err(|e| e.text().en), Ok("shop/@pricing/shared.rule".into()));
}

#[test]
fn of_two_chains_as_short_the_first_require_is_taken() {
    let t = TempDir::new("resolve-order");
    for (p, b) in [
        ("two/ritsu.mod", "module two\nrequire b path \"../b\"\nrequire a path \"../a\"\n"),
        ("a/ritsu.mod", "module a\nrequire x path \"../x\"\n"),
        ("b/ritsu.mod", "module b\nrequire xx path \"../x\"\n"),
        ("x/ritsu.mod", "module x\n"),
        ("x/f.rule", "f\n"),
    ] {
        t.write(p, b);
    }
    let two = t.path().join("two");
    assert_eq!(modules::refer(&two, "@a/@x/f.rule").unwrap(), two.join("@b/@xx/f.rule"));
    assert_eq!(modules::join(&two, "r.rule", "@a/@x/f.rule").unwrap(), "@b/@xx/f.rule");
    // both read the same file
    assert_eq!(fs::read(two.join("@a/@x/f.rule")).unwrap(), b"f\n");
    assert_eq!(fs::read(two.join("@b/@xx/f.rule")).unwrap(), b"f\n");
}

#[test]
fn a_dependency_writes_a_path_from_its_own_root() {
    let t = TempDir::new("resolve-own-root");
    let shop = worktree(&t);
    let own = |from: &str, p: &str| modules::from_own_root(&shop, from, p);
    // a file of the module at hand writes a path as it is read from the root
    assert_eq!(own("rules/order_total.rule", "@pricing/shared.rule"), "@pricing/shared.rule");
    assert_eq!(own("rules/order_total.rule", "rules/x.rule"), "rules/x.rule");
    // a dependency's own files, without the `@<name>/` they are seen through
    assert_eq!(own("@pricing/rules/shipping_fee.rule", "@pricing/shared.rule"), "shared.rule");
    assert_eq!(own("@pricing/rules/shipping_fee.rule", "@pricing"), ".");
    assert_eq!(own("@tax/rules/vat_rate.rule", "@tax/rules/vat_rate.rule"), "rules/vat_rate.rule");
    // its dependencies by its own `require` names
    assert_eq!(own("@pricing/rules/shipping_fee.rule", "@tax/rules/vat_rate.rule"), "@tax/rules/vat_rate.rule");
    // a module that names a dependency otherwise than the module at hand writes its own name
    let t2 = TempDir::new("resolve-own-root-names");
    for (p, b) in [
        ("two/ritsu.mod", "module two\nrequire b path \"../b\"\nrequire a path \"../a\"\n"),
        ("a/ritsu.mod", "module a\nrequire x path \"../x\"\n"),
        ("b/ritsu.mod", "module b\nrequire xx path \"../x\"\n"),
        ("x/ritsu.mod", "module x\n"),
        ("x/f.rule", "f\n"),
    ] {
        t2.write(p, b);
    }
    let two = t2.path().join("two");
    assert_eq!(modules::from_own_root(&two, "@a/r.rule", "@b/@xx/f.rule"), "@x/f.rule");
    assert_eq!(modules::from_own_root(&two, "@b/r.rule", "@b/@xx/f.rule"), "@xx/f.rule");
    // M14: with no `ritsu.mod` at the root, the path as it is
    assert_eq!(modules::from_own_root(&t.path().join("plain"), "@x/a.rule", "@x/b.rule"), "@x/b.rule");
}

#[test]
fn a_tree_that_cannot_be_read_refuses_what_goes_through_it() {
    let t = TempDir::new("resolve-bad-tree");
    t.write("shop/ritsu.mod", "module shop\nrequire pricing path \"../none\"\n");
    let shop = t.path().join("shop");
    let e = refused(modules::refer(&shop, "@pricing/x.rule"));
    assert!(e.starts_with("the tree of dependencies cannot be read: `../none`, where `require pricing path"), "{e}");
    // what stays in the module's own files does not read the tree
    assert_eq!(modules::refer(&shop.join("rules"), "../x.rule").unwrap(), shop.join("x.rule"));
    assert_eq!(modules::join(&shop, "rules/a.rule", "b.rule").unwrap(), "rules/b.rule");
    assert!(refused(modules::refer(&shop, "../out.rule")).starts_with("`../out.rule` leads outside the module `shop`"));
    assert!(refused(modules::refer(&shop.join("rules"), "../@pricing/x.rule")).starts_with("the tree of dependencies cannot be read"));
    // and the layer reads the plain path
    assert!(fs::read(shop.join("@pricing/x.rule")).is_err());
    match modules::tree(&shop).as_ref() {
        Err(Problem::Diags(ds)) => assert_eq!(ds[0].code, "E302"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_missing_file_in_a_dependency_says_the_dependency() {
    let t = TempDir::new("resolve-missing");
    let shop = worktree(&t);
    let m = modules::missing(&shop.join("@pricing/rules/none.rule")).unwrap();
    assert_eq!(m.en, "the dependency `@pricing` (`example.com/acme/pricing`, `../pricing`) has no `rules/none.rule`");
    assert_eq!(m.ja, "依存 `@pricing`（`example.com/acme/pricing`、`../pricing`）に `rules/none.rule` はありません");
    assert!(modules::missing(&shop.join("rules/none.rule")).is_none());
    assert!(modules::missing(&t.path().join("plain/@x/none.rule")).is_none());
    assert_eq!(modules::refer::prefix_of("@pricing/@tax/rules/x.rule").as_deref(), Some("@pricing/@tax"));
    assert_eq!(modules::refer::prefix_of("rules/x.rule"), None);
}

#[test]
fn a_dependency_is_seen_under_the_root_and_read_only() {
    let t = TempDir::new("resolve-view");
    let shop = worktree(&t);
    let at = |p: &str| shop.join(p);
    assert_eq!(fs::read_to_string(at("@pricing/rules/shipping_fee.rule")).unwrap(), "shipping fee\n");
    // a dependency's dependency, by its canonical chain and by the other
    assert_eq!(fs::read_to_string(at("@tax/rules/vat_rate.rule")).unwrap(), "vat rate\n");
    assert_eq!(fs::read_to_string(at("@pricing/@tax/rules/vat_rate.rule")).unwrap(), "vat rate\n");
    assert!(fs::is_dir(at("@pricing")) && fs::is_file(at("@pricing/ritsu.mod")) && fs::exists(at("@pricing/rules")));
    assert_eq!(modules::map(&at("@pricing/rules/x.rule")), Some(t.path().join("pricing/rules/x.rule")));
    // a listing of the root does not show the dependencies; one of a dependency names its entries
    // by the path it was given
    let names = |p: &Path| -> Vec<String> { fs::read_dir(p).unwrap().map(|e| e.unwrap()).map(|e| e.path().strip_prefix(&shop).unwrap().to_string_lossy().into_owned()).collect::<std::collections::BTreeSet<_>>().into_iter().collect() };
    assert_eq!(names(&shop), ["inner", "ritsu.mod", "rules", "sub"]);
    assert_eq!(names(&at("@pricing")), ["@pricing/ritsu.mod", "@pricing/rules", "@pricing/shared.rule"]);
    // the canonical path of a dependency's file: the root's, then the chain
    assert_eq!(fs::canonicalize(at("@pricing/rules/./shipping_fee.rule")).unwrap(), std::fs::canonicalize(&shop).unwrap().join("@pricing/rules/shipping_fee.rule"));
    assert_eq!(fs::canonicalize(at("@pricing/rules/none.rule")).unwrap_err().kind(), std::io::ErrorKind::NotFound);
    // the comparison of the M14 tests sees the view
    assert_ne!(answers(&at("@pricing/shared.rule"), true), answers(&at("@pricing/shared.rule"), false));
    // writing under a dependency is refused, and nothing is made on the disk
    let e = fs::write(at("@pricing/rules/new.rule"), "x").unwrap_err();
    assert_eq!(e.kind(), std::io::ErrorKind::PermissionDenied);
    assert!(e.to_string().ends_with("is in a dependency, whose files are read only"), "{e}");
    assert_eq!(fs::create_dir_all(at("@pricing/new")).unwrap_err().kind(), std::io::ErrorKind::PermissionDenied);
    assert_eq!(fs::create_dir_all(at("@nothing")).unwrap_err().kind(), std::io::ErrorKind::PermissionDenied);
    assert!(!t.path().join("pricing/rules/new.rule").exists() && !at("@pricing").exists() && !at("@nothing").exists());
    // a file that starts with `@` in a directory below the root is the module's own
    fs::write(at("sub/@more.md"), "more\n").unwrap();
    assert_eq!(fs::read_to_string(at("sub/@more.md")).unwrap(), "more\n");
    // a name no `require` gives is read as it is (and is not there)
    assert_eq!(fs::read(at("@nothing/x.rule")).unwrap_err().kind(), std::io::ErrorKind::NotFound);
}

#[test]
fn the_root_the_walk_and_the_shown_path_of_a_module() {
    let t = TempDir::new("resolve-paths");
    let shop = worktree(&t);
    // the nearest `ritsu.mod` is the root, below the `.git`; a dependency's root is not one
    assert_eq!(paths::find_root(&shop.join("rules/order_total.rule")), shop);
    assert_eq!(paths::find_root(&shop.join("@pricing/rules/shipping_fee.rule")), shop);
    assert_eq!(paths::find_root(&shop.join("@pricing")), shop);
    assert_eq!(paths::find_root(&t.path().join("pricing/rules")), t.path().join("pricing"));
    assert_eq!(paths::find_root(&t.path().join("plain/@x/a.rule")), t.path());
    assert_eq!(paths::find_root(&shop.join("inner/x.rule")), shop.join("inner"));
    // a walk of the module passes over the module below; one of a dependency walks its files
    let (mut out, mut nested) = (Vec::new(), Vec::new());
    paths::walk_noting(&shop, ".", &[], &mut out, &mut nested);
    assert_eq!((out, nested), (vec!["ritsu.mod".to_string(), "rules/order_total.rule".into(), "sub/@notes.md".into()], vec!["inner".to_string()]));
    let mut out = Vec::new();
    paths::walk(&shop, "@pricing", &[], &mut out);
    assert_eq!(out, ["@pricing/ritsu.mod", "@pricing/rules/shipping_fee.rule", "@pricing/shared.rule"]);
    // a walk from above every module passes over each
    let (mut out, mut nested) = (Vec::new(), Vec::new());
    paths::walk_noting(t.path(), ".", &[], &mut out, &mut nested);
    assert_eq!((out, nested), (vec!["plain/@x/a.rule".to_string()], vec!["pricing".to_string(), "shop".into(), "tax".into()]));
    // a dependency's file is shown as it is from the root, wherever the run is
    for given in ["rules/order_total.rule", "/abs"] {
        let shown = Shown::run_in(&shop, &shop.join("rules"), given);
        assert_eq!(shown.path("@pricing/rules/shipping_fee.rule"), "@pricing/rules/shipping_fee.rule");
        assert_eq!(shown.path("@pricing/@tax/"), "@pricing/@tax/");
    }
    assert_eq!(Shown::run_in(&shop, &shop.join("rules"), "x").path("rules/order_total.rule"), "order_total.rule");
    assert_eq!(Shown::run_in(&shop, &shop.join("rules"), "x").root(), "..");
    // run from a directory that is not in the module (above it, or in the module below it), a
    // dependency's file is written from there, the path a command run there reads it by
    assert_eq!(Shown::run_in(&shop, t.path(), "shop").path("@pricing/rules/shipping_fee.rule"), "shop/@pricing/rules/shipping_fee.rule");
    assert_eq!(Shown::run_in(&shop, &t.path().join("plain"), "../shop").path("@pricing/@tax/"), "../shop/@pricing/@tax/");
    assert_eq!(Shown::run_in(&shop, &shop.join("inner"), "..").path("@pricing/shared.rule"), "../@pricing/shared.rule");
    let from_above = Shown::run_in(&shop, t.path(), "shop").path("@pricing/rules/shipping_fee.rule");
    assert_eq!(ritsu_base::fs::read_to_string(t.path().join(from_above)).unwrap(), "shipping fee\n");
    // above the module, `@` is a name like another
    assert_eq!(Shown::run_in(t.path(), &t.path().join("plain"), "x").path("plain/@x/a.rule"), "@x/a.rule");
}

#[test]
fn a_walk_of_its_own_does_not_enter_another_module_or_the_copies() {
    use ritsu_base::paths::{NotEntered, not_entered};
    let t = TempDir::new("resolve-not-entered");
    let shop = worktree(&t);
    t.write("shop/.ritsu/modules/example.com/acme/tax/1.0.0/rules/vat_rate.rule", "vat rate\n");
    t.write("plain/.ritsu/x.rule", "plain\n");
    assert_eq!(not_entered(&shop.join("inner")), Some(NotEntered::Module));
    assert_eq!(not_entered(&t.path().join("pricing")), Some(NotEntered::Module));
    assert_eq!(not_entered(&shop.join(".ritsu")), Some(NotEntered::State));
    // the root of a dependency seen through the module is its files; elsewhere, `.ritsu` is a name
    assert_eq!(not_entered(&shop.join("@pricing")), None);
    assert_eq!(not_entered(&shop.join("rules")), None);
    assert_eq!(not_entered(&t.path().join("plain/.ritsu")), None);
    assert_eq!(not_entered(&t.path().join("plain")), None);
    let note = ritsu_base::paths::other_module_note("shop", "rulec check");
    assert_eq!(note.en, "note: `shop` has a `ritsu.mod` of its own: it is another module, and was not read; run `rulec check shop` to read it");
}

#[test]
fn a_relative_path_that_starts_with_at_is_read_from_the_root_of_the_module() {
    let m = Memory::new("/w/shop/rules");
    for (p, b) in [
        ("/w/.git/HEAD", "ref\n"),
        ("/w/shop/ritsu.mod", "module shop\nrequire pricing path \"../pricing\"\n"),
        ("/w/shop/rules/order_total.rule", "order total\n"),
        ("/w/shop/sub/@notes.md", "notes\n"),
        ("/w/pricing/ritsu.mod", "module pricing\n"),
        ("/w/pricing/rules/shipping_fee.rule", "shipping fee\n"),
    ] {
        m.add(p, b);
    }
    fs::with(Rc::new(m), || {
        assert_eq!(paths::absolute(Path::new("@pricing/rules/shipping_fee.rule")), PathBuf::from("/w/shop/@pricing/rules/shipping_fee.rule"));
        assert_eq!(fs::read_to_string("@pricing/rules/shipping_fee.rule").unwrap(), "shipping fee\n");
        assert_eq!(fs::read_to_string("../@pricing/rules/shipping_fee.rule").unwrap(), "shipping fee\n");
        assert_eq!(paths::find_root(Path::new("@pricing/rules/shipping_fee.rule")), PathBuf::from("/w/shop"));
        // `./@…` is read from the working directory
        assert_eq!(paths::absolute(Path::new("./@x")), PathBuf::from("/w/shop/rules/@x"));
        assert_eq!(fs::read_to_string("../sub/@notes.md").unwrap(), "notes\n");
        assert_eq!(modules::refer(Path::new("/w/shop/rules"), "@pricing/rules/shipping_fee.rule").unwrap(), PathBuf::from("/w/shop/@pricing/rules/shipping_fee.rule"));
    });
}

#[test]
fn the_copy_of_the_store_in_a_module_is_read_from_memory_too() {
    // the entry made on the disk, as a fetch makes it
    let t = TempDir::new("resolve-vendor");
    t.write("src/tax/ritsu.mod", "module example.com/acme/tax\nversion 1.0.0\n");
    t.write("src/tax/rules/vat_rate.rule", "vat rate\n");
    let store = Store::at(t.path().join("store"));
    let tax = store::put_directory(&store, &t.path().join("src/tax"), "2026-10-28", &[]).unwrap_or_else(|e| panic!("{}", e.en));
    let mut lock = Lock::new(vec![tax.locked()]);
    lock.ritsu = ritsu_base::modules::semver::Version::parse("0.37.0").unwrap();
    // a module held in memory, with that entry in its copy of the store
    let m = Memory::new("/m/shop/rules");
    m.add("/m/shop/ritsu.mod", "module shop\nrequire tax oci \"example.com/acme/tax\" 1.0.0\n");
    m.add("/m/shop/ritsu.lock", lock.text());
    m.add("/m/shop/rules/order_total.rule", "order total\n");
    let entry = store.entry(&tax.digest);
    let hex = entry.file_name().unwrap().to_string_lossy().into_owned();
    let mut copied = 0;
    copy_into(&m, &entry, &PathBuf::from("/m/shop/.ritsu/modules/sha256").join(&hex), &mut copied);
    assert_eq!(copied, 5, "manifest.json, ritsu.mod, record.json and two files");
    store::with_location(Path::new("/m/no-store"), || {
        fs::with(Rc::new(m), || {
            assert_eq!(fs::read_to_string("@tax/rules/vat_rate.rule").unwrap(), "vat rate\n");
            assert_eq!(modules::map(Path::new("/m/shop/@tax/rules/vat_rate.rule")), Some(PathBuf::from(format!("/m/shop/.ritsu/modules/sha256/{hex}/files/rules/vat_rate.rule"))));
            let tree = modules::tree(Path::new("/m/shop"));
            let tree = tree.as_ref().as_ref().unwrap_or_else(|p| panic!("{}", p.text().en));
            assert_eq!(tree.nodes[1].at.word(), "vendor");
            assert_eq!(tree.nodes[1].prefix, "@tax");
        })
    });
}

/// The example projects of modules (`crates/ritsu/tests/projects/modules` and `modules.ja`, DESIGN
/// 17.26), read as the languages will read them once they go through `refer`: each module's tree
/// reads by its paths alone, every path its files write with a leading `@` leads to a file of a
/// dependency, and a walk from above them passes over every module.
#[test]
fn the_example_projects_read_as_trees() {
    let projects = paths::absolute(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../ritsu/tests/projects"));
    let names = |t: &modules::Tree| t.nodes.iter().map(|n| (n.name.to_string(), n.prefix.clone())).collect::<Vec<_>>();
    let acme = |m: &str| format!("example.com/acme/{m}");
    for (dir, shop, pricing, tax, versions) in [("modules", "shop", "pricing", "tax", "versions/pricing-"), ("modules.ja", "店", "価格", "税", "版/価格-")] {
        let base = projects.join(dir);
        let tree = |m: &str| match graph::load_with(&base.join(m), None).0 {
            Ok(t) => t,
            Err(p) => panic!("{dir}/{m}: {}", p.text().en),
        };
        let t = tree(shop);
        assert_eq!(names(&t), [(acme("shop"), String::new()), (acme("pricing"), "@pricing".into()), (acme("tax"), "@pricing/@tax".into())], "{dir}");
        assert!(t.found.is_empty() && t.lock.is_none(), "{dir}");
        assert_eq!(t.nodes.iter().map(|n| n.version().map(|v| v.to_string()).unwrap_or_default()).collect::<Vec<_>>(), ["0.1.0", "1.1.0", "1.0.0"]);
        for (m, version) in [(pricing.to_string(), "1.1.0"), (format!("{versions}1.0.0"), "1.0.0"), (format!("{versions}2.0.0"), "2.0.0")] {
            let t = tree(&m);
            assert_eq!(names(&t), [(acme("pricing"), String::new()), (acme("tax"), "@tax".into())], "{dir}/{m}");
            assert_eq!(t.main().version().map(|v| v.to_string()).as_deref(), Some(version));
        }
        assert_eq!(names(&tree(tax)), [(acme("tax"), String::new())]);
        // from above, every module is passed over, and nothing else is there
        let (mut out, mut nested) = (Vec::new(), Vec::new());
        paths::walk_noting(&base, ".", &[], &mut out, &mut nested);
        let mut want = vec![shop.to_string(), pricing.to_string(), tax.to_string(), format!("{versions}1.0.0"), format!("{versions}2.0.0")];
        want.sort();
        assert_eq!((out, nested), (Vec::<String>::new(), want), "{dir}");
        // each path written with a leading `@` leads to a file of a dependency
        let mut written = 0;
        for m in [shop.to_string(), pricing.to_string(), format!("{versions}1.0.0"), format!("{versions}2.0.0")] {
            let root = base.join(&m);
            let mut files = Vec::new();
            paths::walk(&root, ".", &[], &mut files);
            for f in files {
                let text = std::fs::read_to_string(root.join(&f)).unwrap();
                for at in text.split('"').skip(1).step_by(2).filter(|s| s.starts_with('@')) {
                    let dir = root.join(paths::parent(&f));
                    let target = modules::refer(&dir, at).unwrap_or_else(|e| panic!("{dir:?} {at}: {}", e.text().en));
                    assert!(fs::is_file(&target), "{m}/{f}: {at} leads to {}, which is not a file", target.display());
                    assert!(modules::map(&target).is_some_and(|real| real.is_file()), "{m}/{f}: {at}");
                    written += 1;
                }
            }
        }
        assert_eq!(written, 6, "{dir}: the three `apply`s of pricing, the two `use rule`s of the flow and the one of the basket");
    }
}

/// The files under `from` on the disk added to `m` under `to`.
fn copy_into(m: &Memory, from: &Path, to: &Path, n: &mut usize) {
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        if e.file_type().unwrap().is_dir() {
            copy_into(m, &e.path(), &to.join(e.file_name()), n);
        } else {
            m.add(to.join(e.file_name()), std::fs::read(e.path()).unwrap());
            *n += 1;
        }
    }
}

#[test]
fn the_tree_is_kept_while_its_files_read_the_same() {
    let t = TempDir::new("resolve-cache");
    let shop = worktree(&t);
    let a = modules::tree(&shop);
    assert!(a.is_ok());
    assert!(Rc::ptr_eq(&a, &modules::tree(&shop)));
    assert!(Rc::ptr_eq(&a, &modules::tree(&shop.join("rules/.."))), "the same root however written");
    // a `ritsu.mod` a path reaches changed: read again
    t.write("pricing/ritsu.mod", "module example.com/acme/pricing\nversion 1.1.0\n# changed\nrequire tax path \"../tax\"\n");
    let b = modules::tree(&shop);
    assert!(!Rc::ptr_eq(&a, &b));
    assert!(Rc::ptr_eq(&b, &modules::tree(&shop)));
    // a lock put beside it
    t.write("shop/ritsu.lock", "lock 1\nritsu 0.37.0\npath example.com/acme/pricing \"../pricing\"\npath example.com/acme/tax \"../tax\"\n");
    let c = modules::tree(&shop);
    assert!(!Rc::ptr_eq(&b, &c) && c.is_ok());
    // another store, other files, or forgotten: read again
    let d = store::with_location(&t.path().join("store"), || modules::tree(&shop));
    assert!(!Rc::ptr_eq(&c, &d));
    modules::forget();
    assert!(!Rc::ptr_eq(&c, &modules::tree(&shop)));
}

#[test]
fn only_a_part_that_starts_with_at_is_looked_at() {
    assert!(modules::refer::has_at(Path::new("a/@b/c")) && modules::refer::has_at(Path::new("@b")));
    assert!(!modules::refer::has_at(Path::new("a/b@c/d")) && !modules::refer::has_at(Path::new("a/b.rule")));
}

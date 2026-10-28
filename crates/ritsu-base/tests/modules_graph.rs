//! The tree of modules (DESIGN 17.7): Minimal Version Selection (its properties over trees made at
//! random from a fixed seed, and answers solved by hand), the tree a check reads — by paths, from the
//! store and from a module's copy of it — held to the lock (E303–E306, E308, W302), what stops it,
//! and how each module's files are written (the fewest `@`, then the order of the `require`s).

use ritsu_base::modules::digest;
use ritsu_base::modules::graph::{self, Problem, Tree};
use ritsu_base::modules::lock::{Lock, Locked, Pinned};
use ritsu_base::modules::names::ModuleName;
use ritsu_base::modules::semver::Version;
use ritsu_base::modules::store::{self, Store};
use ritsu_base::modules::vendor::Where;
use ritsu_testkit::TempDir;
use std::collections::BTreeMap;
use std::path::Path;

fn v(s: &str) -> Version {
    Version::parse(s).unwrap()
}

/// SplitMix64: numbers from a seed, the same on every machine.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// A registry made at random: for each module and version, what it requires.
type Registry = BTreeMap<(String, Version), Vec<(String, Version)>>;

fn registry(rng: &mut Rng) -> (Registry, Vec<(String, Version)>) {
    let modules = 2 + rng.below(6) as usize;
    let versions = ["1.0.0", "1.1.0", "1.2.0-rc.1", "1.2.0", "2.0.0"];
    let mut reg: Registry = BTreeMap::new();
    for m in 0..modules {
        for ver in versions {
            if rng.below(4) == 0 && ver != "1.0.0" {
                continue;
            }
            reg.insert((format!("m{m}"), v(ver)), Vec::new());
        }
    }
    let keys: Vec<(String, Version)> = reg.keys().cloned().collect();
    for k in &keys {
        let n = rng.below(4);
        let reqs: Vec<(String, Version)> = (0..n).map(|_| keys[rng.below(keys.len() as u64) as usize].clone()).filter(|r| r.0 != k.0).collect();
        reg.insert(k.clone(), reqs);
    }
    let roots: Vec<(String, Version)> = (0..1 + rng.below(3)).map(|_| keys[rng.below(keys.len() as u64) as usize].clone()).collect();
    (reg, roots)
}

/// MVS the slow way: every version reached, by passes until a pass adds none, and the highest of each.
fn by_passes(reg: &Registry, roots: &[(String, Version)]) -> BTreeMap<String, Version> {
    let mut reached: Vec<(String, Version)> = roots.to_vec();
    loop {
        let before = reached.len();
        for k in reached.clone() {
            for r in &reg[&k] {
                if !reached.contains(r) {
                    reached.push(r.clone());
                }
            }
        }
        if reached.len() == before {
            break;
        }
    }
    let mut out: BTreeMap<String, Version> = BTreeMap::new();
    for (m, ver) in reached {
        let e = out.entry(m).or_insert(ver.clone());
        if ver > *e {
            *e = ver;
        }
    }
    out
}

#[test]
fn minimal_version_selection_holds_its_properties_on_trees_made_at_random() {
    let seed = 0x6d76_7300_u64;
    let mut rng = Rng(seed);
    for round in 0..500 {
        let (reg, roots) = registry(&mut rng);
        let tree = || format!("seed {seed:#x}, round {round}\nroots {roots:?}\n{}", reg.iter().map(|(k, r)| format!("{} {} -> {r:?}\n", k.0, k.1)).collect::<String>());
        let chosen = graph::mvs(&roots, |m, ver| reg[&(m.clone(), ver.clone())].clone());
        // the same as the slow way
        assert_eq!(chosen, by_passes(&reg, &roots), "{}", tree());
        // every require of every version chosen is met
        for (m, ver) in &chosen {
            for (r, min) in &reg[&(m.clone(), ver.clone())] {
                assert!(chosen.get(r).is_some_and(|c| c >= min), "{m} {ver} requires {r} {min}\n{}", tree());
            }
        }
        // and the roots' too
        for (r, min) in &roots {
            assert!(chosen[r] >= *min, "{}", tree());
        }
        // the order the requires are read in changes nothing
        let mut shuffle = Rng(seed ^ round);
        let again = graph::mvs(&roots.iter().rev().cloned().collect::<Vec<_>>(), |m, ver| {
            let mut r = reg[&(m.clone(), ver.clone())].clone();
            for i in (1..r.len()).rev() {
                r.swap(i, shuffle.below(i as u64 + 1) as usize);
            }
            r
        });
        assert_eq!(again, chosen, "{}", tree());
        // choosing again from what was chosen chooses the same
        let pinned: Vec<(String, Version)> = chosen.iter().map(|(m, ver)| (m.clone(), ver.clone())).collect();
        assert_eq!(graph::mvs(&pinned, |m, ver| reg[&(m.clone(), ver.clone())].clone()), chosen, "{}", tree());
        // a pre-release is chosen only when something requires it
        for (m, ver) in &chosen {
            if ver.is_prerelease() {
                assert!(roots.iter().chain(reg.values().flatten()).any(|(r, x)| r == m && x == ver), "{}", tree());
            }
        }
    }
}

#[test]
fn minimal_version_selection_solved_by_hand() {
    let reg: Registry = [
        (("a", "1.2.0"), vec![("c", "1.3.0")]),
        (("b", "1.2.0"), vec![("c", "1.4.0")]),
        // a version not chosen still counts what it requires
        (("c", "1.3.0"), vec![("d", "1.5.0")]),
        (("c", "1.4.0"), vec![("d", "1.2.0")]),
        (("d", "1.2.0"), vec![]),
        (("d", "1.5.0"), vec![]),
        (("d", "1.6.0-rc.1"), vec![]),
        (("x", "1.0.0"), vec![]),
        (("y", "1.0.0"), vec![("x", "1.1.0-rc.1")]),
        (("x", "1.1.0-rc.1"), vec![]),
    ]
    .into_iter()
    .map(|((m, ver), rs)| ((m.to_string(), v(ver)), rs.into_iter().map(|(r, x)| (r.to_string(), v(x))).collect()))
    .collect();
    let pick = |roots: &[(&str, &str)]| {
        let roots: Vec<(String, Version)> = roots.iter().map(|(m, x)| (m.to_string(), v(x))).collect();
        graph::mvs(&roots, |m, ver| reg[&(m.clone(), ver.clone())].clone()).into_iter().map(|(m, x)| format!("{m} {x}")).collect::<Vec<_>>()
    };
    assert_eq!(pick(&[("a", "1.2.0"), ("b", "1.2.0")]), ["a 1.2.0", "b 1.2.0", "c 1.4.0", "d 1.5.0"]);
    // the newest version (a pre-release here) is not taken unless required
    assert_eq!(pick(&[("x", "1.0.0")]), ["x 1.0.0"]);
    assert_eq!(pick(&[("x", "1.0.0"), ("y", "1.0.0")]), ["x 1.1.0-rc.1", "y 1.0.0"]);
}

/// A directory of files written from (path, text) pairs.
fn files(t: &TempDir, fs: &[(&str, &str)]) {
    for (p, b) in fs {
        t.write(p, b);
    }
}

fn load(t: &TempDir, root: &str, store: Option<&Store>) -> Result<Tree, Problem> {
    graph::load_with(&t.path().join(root), store).0
}

/// The code and the English message of each diagnostic of a tree that does not read, or of each
/// thing that stops it.
fn problem(r: Result<Tree, Problem>) -> Vec<String> {
    match r {
        Ok(t) => panic!("read: {:?}", t.nodes.iter().map(|n| n.name.to_string()).collect::<Vec<_>>()),
        Err(Problem::Diags(ds)) => ds.iter().map(|d| format!("{} {}{}: {}", d.code, d.file, d.line.map(|l| format!(":{l}")).unwrap_or_default(), d.message.en)).collect(),
        Err(Problem::Stop(ts)) => ts.iter().map(|t| format!("stop: {}", t.en)).collect(),
    }
}

fn names_and_prefixes(t: &Tree) -> Vec<(String, String)> {
    t.nodes.iter().map(|n| (n.name.to_string(), n.prefix.clone())).collect()
}

#[test]
fn a_tree_of_paths_needs_no_lock_and_writes_each_module_the_shortest_way() {
    let t = TempDir::new("graph-paths");
    files(&t, &[
        ("shop/ritsu.mod", "module example.com/acme/shop\nversion 0.1.0\nrequire pricing path \"../pricing\"\n"),
        ("pricing/ritsu.mod", "module example.com/acme/pricing\nversion 1.1.0\nrequire tax path \"../tax\"\n"),
        ("tax/ritsu.mod", "module example.com/acme/tax\nversion 1.0.0\n"),
    ]);
    let tree = load(&t, "shop", None).unwrap();
    assert_eq!(names_and_prefixes(&tree), [("example.com/acme/shop".into(), String::new()), ("example.com/acme/pricing".into(), "@pricing".into()), ("example.com/acme/tax".into(), "@pricing/@tax".into())]);
    assert_eq!(tree.nodes.iter().map(|n| (n.at, n.path.clone())).collect::<Vec<_>>(), [(Where::Here, Some(".".into())), (Where::Path, Some("../pricing".into())), (Where::Path, Some("../tax".into()))]);
    assert_eq!(tree.walk(&["pricing", "tax"]), Some(2));
    assert_eq!(tree.by_prefix("@pricing/@tax"), Some(2));
    assert_eq!(tree.nodes[2].version(), Some(&v("1.0.0")));
    assert!(tree.lock.is_none() && tree.found.is_empty());
    // required directly too, it is written the short way
    t.write("shop/ritsu.mod", "module example.com/acme/shop\nrequire pricing path \"../pricing\"\nrequire tax path \"../tax\"\n");
    assert_eq!(load(&t, "shop", None).unwrap().nodes[2].prefix, "@tax");
    // of two chains as short, the one whose `require` comes first
    files(&t, &[
        ("two/ritsu.mod", "module two\nrequire b path \"../b\"\nrequire a path \"../a\"\n"),
        ("a/ritsu.mod", "module a\nrequire x path \"../x\"\n"),
        ("b/ritsu.mod", "module b\nrequire xx path \"../x\"\n"),
        ("x/ritsu.mod", "module x\n"),
    ]);
    let tree = load(&t, "two", None).unwrap();
    assert_eq!(names_and_prefixes(&tree).into_iter().map(|(_, p)| p).collect::<Vec<_>>(), ["", "@b", "@a", "@b/@xx"]);
    // a cycle back to the module at hand reads it once
    t.write("x/ritsu.mod", "module x\nrequire two path \"../two\"\n");
    let tree = load(&t, "two", None).unwrap();
    assert_eq!(tree.nodes.len(), 4);
    assert_eq!(tree.dep(tree.by_prefix("@b/@xx").unwrap(), "two"), Some(0));
}

#[test]
fn what_a_tree_of_paths_refuses() {
    let t = TempDir::new("graph-paths-bad");
    files(&t, &[
        ("shop/ritsu.mod", "module shop\nrequire p1 path \"../p1\"\nrequire p2 path \"../p2\"\nrequire none path \"../none\"\n"),
        ("p1/ritsu.mod", "module pricing\n"),
        ("p2/ritsu.mod", "module pricing\n"),
        ("none/x.rule", "# no ritsu.mod\n"),
    ]);
    assert_eq!(
        problem(load(&t, "shop", None)),
        [
            "E306 ritsu.mod:3: the module `pricing` comes from two places, `../p1` and `../p2`; a module is read from one",
            "E302 ritsu.mod:4: `../none`, where `require none path \"../none\"` leads, has no `ritsu.mod`; a dependency by a path names a directory with a `ritsu.mod` in it",
        ]
    );
    // a dependency's own `ritsu.mod` that does not read is said at its path
    files(&t, &[("bad/ritsu.mod", "module shop2\nrequire pricing path \"../p3\"\n"), ("p3/ritsu.mod", "module pricing\nversion one\n")]);
    assert_eq!(problem(load(&t, "bad", None)), ["E302 @pricing/ritsu.mod:2: `one` is not a version: write three numbers with dots between them, as `1.4.0`"]);
    // a module that asks for a newer ritsu stops, even when it does not read
    files(&t, &[("new/ritsu.mod", "module new\nrequire n path \"../n\"\n"), ("n/ritsu.mod", "module n\nritsu >=99.0.0\nworkspace \"x\"\n")]);
    let now = env!("CARGO_PKG_VERSION");
    assert_eq!(problem(load(&t, "new", None)), [format!("stop: `@n/ritsu.mod` asks for ritsu 99.0.0 or later, and the ritsu that runs is {now}; run a newer ritsu")]);
    t.write("n/ritsu.mod", "module n\nritsu >=99.0.0\n");
    assert_eq!(problem(load(&t, "new", None)), [format!("stop: `n` asks for ritsu 99.0.0 or later, and the ritsu that runs is {now}; run a newer ritsu")]);
}

#[test]
fn a_name_that_starts_with_at_under_a_root_is_e307_and_does_not_stop_the_tree() {
    let t = TempDir::new("graph-e307");
    files(&t, &[("shop/ritsu.mod", "module shop\nrequire p path \"../p\"\n"), ("shop/@p/x.rule", "x"), ("p/ritsu.mod", "module p\n"), ("p/@notes.md", "x")]);
    let tree = load(&t, "shop", None).unwrap();
    let found: Vec<String> = tree.found.iter().map(|d| format!("{} {} {}", d.code, d.file, d.message.en)).collect();
    assert_eq!(
        found,
        [
            "E307 ritsu.mod the root of the module has `@p` in it; a name that starts with `@` right under the root names a dependency, so no file or directory takes one: rename it",
            "E307 @p/ritsu.mod the root of the module has `@notes.md` in it; a name that starts with `@` right under the root names a dependency, so no file or directory takes one: rename it",
        ]
    );
}

#[test]
fn a_path_wins_over_a_registry_for_the_same_module() {
    let t = TempDir::new("graph-path-wins");
    files(&t, &[
        ("shop/ritsu.mod", "module example.com/acme/shop\nrequire pricing path \"../pricing\"\nrequire tax path \"../tax\"\n"),
        ("pricing/ritsu.mod", "module example.com/acme/pricing\nrequire tax oci \"example.com/acme/tax\" 1.0.0\n"),
        ("tax/ritsu.mod", "module example.com/acme/tax\nversion 1.2.0\n"),
    ]);
    let tree = load(&t, "shop", None).unwrap();
    assert_eq!(tree.path_won, [ModuleName::parse("example.com/acme/tax").unwrap()]);
    let tax = tree.by_name(&ModuleName::parse("example.com/acme/tax").unwrap()).unwrap();
    assert_eq!(tree.dep(tree.by_prefix("@pricing").unwrap(), "tax"), Some(tax));
    assert_eq!(tree.nodes[tax].prefix, "@tax");
}

/// The store `store` with the module under `dir` placed in it, and its lock line.
fn put(store: &Store, dir: &Path, verified: &[String]) -> Locked {
    store::put_directory(store, dir, "2026-10-28", verified).unwrap_or_else(|e| panic!("{}", e.en)).locked()
}

fn write_lock(t: &TempDir, root: &str, lines: Vec<Locked>) {
    let mut l = Lock::new(lines);
    l.ritsu = v("0.37.0");
    t.write(&format!("{root}/ritsu.lock"), l.text());
}

#[test]
fn modules_from_a_registry_are_read_from_the_store_and_held_to_the_lock() {
    let t = TempDir::new("graph-store");
    let store = Store::at(t.path().join("store"));
    files(&t, &[
        ("src/tax/ritsu.mod", "module example.com/acme/tax\nversion 1.0.0\n"),
        ("src/tax/rules/vat_rate.rule", "# vat\n"),
        ("src/pricing/ritsu.mod", "module example.com/acme/pricing\nversion 1.1.0\nrequire tax oci \"example.com/acme/tax\" 1.0.0\n"),
        ("src/pricing/rules/fee.rule", "# fee\n"),
        ("shop/ritsu.mod", "module example.com/acme/shop\nversion 0.1.0\nrequire pricing oci \"example.com/acme/pricing\" 1.1.0\n"),
    ]);
    let tax = put(&store, &t.path().join("src/tax"), &[]);
    let pricing = put(&store, &t.path().join("src/pricing"), &[]);
    // the version index says the digest of each version
    assert_eq!(store.find(&tax.name, &v("1.0.0")), tax.pinned_digest());
    // no lock
    assert_eq!(
        problem(load(&t, "shop", Some(&store))),
        ["E303 ritsu.mod:3: there is no `ritsu.lock`: the dependency `pricing` comes from a registry, so the version chosen and its digests are pinned in `ritsu.lock`; run `ritsu mod lock`"]
    );
    // the lock pins both: read from the store
    write_lock(&t, "shop", vec![pricing.clone(), tax.clone()]);
    let tree = load(&t, "shop", Some(&store)).unwrap();
    assert_eq!(names_and_prefixes(&tree), [("example.com/acme/shop".into(), String::new()), ("example.com/acme/pricing".into(), "@pricing".into()), ("example.com/acme/tax".into(), "@pricing/@tax".into())]);
    assert_eq!(tree.nodes[1].at, Where::Store);
    assert_eq!(tree.nodes[1].dir, store.files(pricing.pinned_digest().as_deref().unwrap()));
    assert!(tree.nodes[2].dir.join("rules/vat_rate.rule").is_file());
    // a lock with no line for a module
    write_lock(&t, "shop", vec![pricing.clone()]);
    assert_eq!(
        problem(load(&t, "shop", Some(&store))),
        ["E304 @pricing/ritsu.mod:3: `ritsu.lock` does not match `ritsu.mod`: the lock has no line for `require tax oci \"example.com/acme/tax\" 1.0.0`; run `ritsu mod lock`"]
    );
    // a lock whose version is below what is required
    t.write("shop/ritsu.mod", "module example.com/acme/shop\nrequire pricing oci \"example.com/acme/pricing\" 1.2.0\n");
    write_lock(&t, "shop", vec![pricing.clone(), tax.clone()]);
    assert_eq!(
        problem(load(&t, "shop", Some(&store))),
        ["E304 ritsu.mod:2: `ritsu.lock` does not match `ritsu.mod`: `require pricing oci \"example.com/acme/pricing\" 1.2.0` asks for 1.2.0 or later, and the lock has 1.1.0; run `ritsu mod lock`"]
    );
    // a line nothing requires now
    t.write("shop/ritsu.mod", "module example.com/acme/shop\nrequire tax oci \"example.com/acme/tax\" 1.0.0\n");
    let tree = load(&t, "shop", Some(&store)).unwrap();
    assert_eq!(tree.found.iter().map(|d| format!("{} {}:{}", d.code, d.file, d.line.unwrap_or(0))).collect::<Vec<_>>(), ["W302 ritsu.lock:4"]);
    assert_eq!(tree.found[0].message.en, "`ritsu.lock` has a line for `example.com/acme/pricing`, which nothing requires now; run `ritsu mod lock` to write it again");
    // the store without the entry: stop
    t.write("shop/ritsu.mod", "module example.com/acme/shop\nrequire pricing oci \"example.com/acme/pricing\" 1.1.0\n");
    let empty = Store::at(t.path().join("empty"));
    assert_eq!(problem(load(&t, "shop", Some(&empty))), ["stop: the dependency `example.com/acme/pricing` 1.1.0 is neither in the store nor in this module's copy of it; run `ritsu mod fetch`"]);
    assert_eq!(problem(load(&t, "shop", None)).len(), 1, "no store at all stops the same way");
}

#[test]
fn files_changed_in_the_store_are_e305_and_the_copy_in_the_module_comes_first() {
    let t = TempDir::new("graph-e305");
    let store = Store::at(t.path().join("store"));
    files(&t, &[("src/tax/ritsu.mod", "module example.com/acme/tax\nversion 1.0.0\n"), ("src/tax/r.rule", "# r\n"), ("shop/ritsu.mod", "module shop\nrequire tax oci \"example.com/acme/tax\" 1.0.0\n")]);
    let tax = put(&store, &t.path().join("src/tax"), &[]);
    write_lock(&t, "shop", vec![tax.clone()]);
    let digest = tax.pinned_digest().unwrap();
    // the module's own copy is read before the store
    let own = Store::vendored(&t.path().join("shop"));
    copy_dir(&store.entry(&digest), &own.entry(&digest));
    assert_eq!(load(&t, "shop", Some(&store)).unwrap().nodes[1].at, Where::Vendor);
    std::fs::remove_dir_all(t.path().join("shop/.ritsu")).unwrap();
    assert_eq!(load(&t, "shop", Some(&store)).unwrap().nodes[1].at, Where::Store);
    // a file of the entry changed by hand
    let f = store.files(&digest).join("r.rule");
    let mut p = std::fs::metadata(&f).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    p.set_readonly(false);
    std::fs::set_permissions(&f, p).unwrap();
    std::fs::write(&f, "# changed\n").unwrap();
    assert_eq!(
        problem(load(&t, "shop", Some(&store))),
        ["E305 ritsu.lock:4: the files of `example.com/acme/tax` 1.0.0 in the store differ from the digest of `ritsu.lock`; run `ritsu mod clean`, then `ritsu mod fetch` to fetch them again"]
    );
}

#[test]
fn what_a_module_from_a_registry_has_to_be() {
    let t = TempDir::new("graph-fetched");
    let store = Store::at(t.path().join("store"));
    files(&t, &[
        ("src/a/ritsu.mod", "module example.com/acme/a\nversion 1.0.0\nrequire local path \"../local\"\n"),
        ("src/b/ritsu.mod", "module example.com/acme/b\nversion 1.1.0\n"),
        ("shop/ritsu.mod", "module shop\nrequire a oci \"example.com/acme/a\" 1.0.0\n"),
    ]);
    let a = put(&store, &t.path().join("src/a"), &[]);
    let b = put(&store, &t.path().join("src/b"), &[]);
    // a `require … path` in a module from a registry
    write_lock(&t, "shop", vec![a.clone()]);
    assert_eq!(
        problem(load(&t, "shop", Some(&store))),
        ["E308 @a/ritsu.mod:3: the module `example.com/acme/a` 1.0.0, from a registry, has `require local path \"../local\"`; a dependency by a path is not where the module is used, so it cannot be read"]
    );
    // the lock names `a`, and the entry it pins is `b`: two places give one name
    let mut wrong = b.clone();
    wrong.name = a.name.clone();
    if let Pinned::Oci { version, .. } = &mut wrong.pinned {
        *version = v("1.0.0");
    }
    write_lock(&t, "shop", vec![wrong]);
    let got = problem(load(&t, "shop", Some(&store)));
    assert_eq!(got[0], "E306 @a/ritsu.mod:1: the `ritsu.mod` fetched as `example.com/acme/a` says `module example.com/acme/b`: two places give a module of one name");
    assert_eq!(got[1], "E304 ritsu.lock:4: `ritsu.lock` pins `example.com/acme/a` at 1.0.0, and the `version` of the `ritsu.mod` fetched is 1.1.0; run `ritsu mod lock`");
    // the lock reads by a path what `ritsu.mod` takes from a registry
    write_lock(&t, "shop", vec![Locked { name: a.name.clone(), pinned: Pinned::Path { path: "../src/a".into() }, line: 0 }]);
    assert_eq!(
        problem(load(&t, "shop", Some(&store))),
        ["E304 ritsu.mod:2: `ritsu.lock` does not match `ritsu.mod`: `require a oci \"example.com/acme/a\" 1.0.0` comes from a registry, and the lock reads `example.com/acme/a` from the path `../src/a`; run `ritsu mod lock`"]
    );
    // a lock that cannot be read
    t.write("shop/ritsu.lock", "lock 1\nritsu 0.37.0\noci example.com/acme/a\n");
    assert_eq!(problem(load(&t, "shop", Some(&store)))[0].split(':').next(), Some("E303 ritsu.lock"));
    t.write("shop/ritsu.lock", "lock 7\n");
    assert_eq!(problem(load(&t, "shop", Some(&store))), ["stop: `ritsu.lock` is in the format `lock 7`, and this ritsu reads `lock 1` at most: a newer ritsu wrote it, so run a newer ritsu"]);
}

#[test]
fn a_signature_asked_for_needs_the_record_of_its_check() {
    let t = TempDir::new("graph-signature");
    let store = Store::at(t.path().join("store"));
    let line = "signature \"example.com/acme/\" key \"keys/acme.pub\"";
    files(&t, &[
        ("src/tax/ritsu.mod", "module example.com/acme/tax\nversion 1.0.0\n"),
        ("shop/ritsu.mod", &format!("module shop\nrequire tax oci \"example.com/acme/tax\" 1.0.0\n{line}\n")),
        ("shop/keys/acme.pub", "key"),
    ]);
    // what a record of the check says: the line, and the digest of the key file
    let checked = format!("{line} {}", digest::of(b"key"));
    let tax = put(&store, &t.path().join("src/tax"), &[]);
    write_lock(&t, "shop", vec![tax]);
    assert_eq!(problem(load(&t, "shop", Some(&store))), [format!("stop: there is no record that the signature of `example.com/acme/tax` 1.0.0 was checked against `{checked}`; run `ritsu mod fetch`")]);
    // checked: the record says so
    let other = Store::at(t.path().join("checked"));
    let tax = put(&other, &t.path().join("src/tax"), std::slice::from_ref(&checked));
    write_lock(&t, "shop", vec![tax.clone()]);
    let tree = load(&t, "shop", Some(&other)).unwrap();
    assert_eq!(tree.nodes[1].verified, std::slice::from_ref(&checked));
    // a record of the line alone, with no key's digest, is not one
    let bare = Store::at(t.path().join("bare"));
    write_lock(&t, "shop", vec![put(&bare, &t.path().join("src/tax"), &[line.to_string()])]);
    assert_eq!(problem(load(&t, "shop", Some(&bare))).len(), 1);
    // the key file changed: the check is asked for again
    t.write("shop/keys/acme.pub", "another key");
    write_lock(&t, "shop", vec![tax]);
    let again = format!("{line} {}", digest::of(b"another key"));
    assert_eq!(problem(load(&t, "shop", Some(&other))), [format!("stop: there is no record that the signature of `example.com/acme/tax` 1.0.0 was checked against `{again}`; run `ritsu mod fetch`")]);
    t.write("shop/keys/acme.pub", "key");
    // no key file: E302 at the line
    std::fs::remove_file(t.path().join("shop/keys/acme.pub")).unwrap();
    assert_eq!(
        problem(load(&t, "shop", Some(&other))),
        [format!("E302 ritsu.mod:3: the key file `keys/acme.pub` of `{line}` cannot be read; a key file is written from the root of the module whose line it is")]
    );
    t.write("shop/keys/acme.pub", "key");
    // a line that covers another module is not asked of this one
    t.write("shop/ritsu.mod", "module shop\nrequire tax oci \"example.com/acme/tax\" 1.0.0\nsignature \"example.com/other/\" key \"keys/acme.pub\"\n");
    load(&t, "shop", Some(&store)).unwrap();
}

#[test]
fn the_same_line_over_two_keys_is_two_checks() {
    // pricing requires tax and writes the line shop writes, over another key: each is checked
    let t = TempDir::new("graph-two-keys");
    let line = "signature \"example.com/acme/tax\" key \"keys/acme.pub\"";
    files(&t, &[
        ("src/tax/ritsu.mod", "module example.com/acme/tax\nversion 1.0.0\n"),
        ("src/pricing/ritsu.mod", &format!("module example.com/acme/pricing\nversion 1.0.0\nrequire tax oci \"example.com/acme/tax\" 1.0.0\n{line}\n")),
        ("src/pricing/keys/acme.pub", "pricing's key"),
        ("shop/ritsu.mod", &format!("module shop\nrequire pricing oci \"example.com/acme/pricing\" 1.0.0\nrequire tax oci \"example.com/acme/tax\" 1.0.0\n{line}\n")),
        ("shop/keys/acme.pub", "shop's key"),
    ]);
    let (shops, pricings) = (format!("{line} {}", digest::of(b"shop's key")), format!("{line} {}", digest::of(b"pricing's key")));
    let store = Store::at(t.path().join("store"));
    let pricing = put(&store, &t.path().join("src/pricing"), &[]);
    let tax = put(&store, &t.path().join("src/tax"), std::slice::from_ref(&shops));
    write_lock(&t, "shop", vec![pricing.clone(), tax]);
    assert_eq!(problem(load(&t, "shop", Some(&store))), [format!("stop: there is no record that the signature of `example.com/acme/tax` 1.0.0 was checked against `{pricings}`; run `ritsu mod fetch`")]);
    let both = Store::at(t.path().join("both"));
    let pricing = put(&both, &t.path().join("src/pricing"), &[]);
    let tax = put(&both, &t.path().join("src/tax"), &[shops.clone(), pricings.clone()]);
    write_lock(&t, "shop", vec![pricing, tax]);
    let tree = load(&t, "shop", Some(&both)).unwrap();
    assert_eq!(tree.nodes[tree.by_prefix("@tax").unwrap()].verified, [shops, pricings]);
}

#[test]
fn choose_reads_each_version_reached_once_and_writes_the_lock() {
    let t = TempDir::new("graph-choose");
    files(&t, &[
        ("shop/ritsu.mod", "module example.com/acme/shop\nrequire pricing oci \"example.com/acme/pricing\" 1.0.0\nrequire local path \"../local\"\n"),
        ("local/ritsu.mod", "module local\nrequire tax oci \"example.com/acme/tax\" 1.1.0\n"),
    ]);
    let mods: BTreeMap<(String, String), &str> = [
        (("example.com/acme/pricing", "1.0.0"), "module example.com/acme/pricing\nversion 1.0.0\nrequire tax oci \"example.com/acme/tax\" 1.0.0\n"),
        (("example.com/acme/tax", "1.0.0"), "module example.com/acme/tax\nversion 1.0.0\n"),
        (("example.com/acme/tax", "1.1.0"), "module example.com/acme/tax\nversion 1.1.0\n"),
    ]
    .into_iter()
    .map(|((m, x), b)| ((m.to_string(), x.to_string()), b))
    .collect();
    let mut asked: Vec<String> = Vec::new();
    let chosen = graph::choose(&t.path().join("shop"), |m, x| {
        asked.push(format!("{m} {x}"));
        Ok(mods[&(m.to_string(), x.to_string())].as_bytes().to_vec())
    })
    .unwrap_or_else(|p| panic!("{}", p.text().en));
    asked.sort();
    assert_eq!(asked, ["example.com/acme/pricing 1.0.0", "example.com/acme/tax 1.0.0", "example.com/acme/tax 1.1.0"]);
    assert_eq!(chosen.paths, [(ModuleName::parse("local").unwrap(), "../local".to_string())]);
    assert_eq!(chosen.versions.iter().map(|(m, x)| format!("{m} {x}")).collect::<Vec<_>>(), ["example.com/acme/pricing 1.0.0", "example.com/acme/tax 1.1.0"]);
    let d = "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_string();
    let lock = graph::lock_of(&chosen, |_, _| Some((d.clone(), d.clone()))).unwrap();
    assert_eq!(lock.modules.len(), 3);
    // a version fetched that names another module, or has a path dependency
    let e = graph::choose(&t.path().join("shop"), |m, x| Ok(format!("module example.com/acme/other\nversion {x}\nrequire p path \"../p\"\n# {m}\n").into_bytes())).unwrap_err();
    assert_eq!(problem(Err(e)).len(), 2);
}

/// `from` copied to `to`, file by file (the copy writable).
fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let (src, dst) = (e.path(), to.join(e.file_name()));
        if e.file_type().unwrap().is_dir() {
            copy_dir(&src, &dst);
        } else {
            std::fs::write(&dst, std::fs::read(&src).unwrap()).unwrap();
        }
    }
}

/// The digest of a lock line from a registry.
trait Digest {
    fn pinned_digest(&self) -> Option<String>;
}

impl Digest for Locked {
    fn pinned_digest(&self) -> Option<String> {
        match &self.pinned {
            Pinned::Oci { digest, .. } => Some(digest.clone()),
            Pinned::Path { .. } => None,
        }
    }
}

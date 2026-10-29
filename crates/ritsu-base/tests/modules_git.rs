//! The dependencies from git repositories (DESIGN 17.12.1), as far as they are decided without
//! running git: the `require … git` line and the URLs it takes (E302, a URL that may carry a
//! credential not shown), the name a URL and a `dir` give, one source for a module in a tree (E306),
//! what a module from git or from a registry cannot require (E308), a `signature` line that covers a
//! module from git (E309), the choice of the versions and the lock with the commit, the files of a
//! commit chosen by the rule the files of a directory are, and the store's index of the versions
//! from git, apart from `refs`.

use ritsu_base::modules::digest;
use ritsu_base::modules::git;
use ritsu_base::modules::graph::{self, Origin, Problem, Tree};
use ritsu_base::modules::lock::{Lock, Locked, Pinned};
use ritsu_base::modules::manifest::{self, EditError, Source};
use ritsu_base::modules::names::ModuleName;
use ritsu_base::modules::semver::Version;
use ritsu_base::modules::store::{self, Record, Store};
use ritsu_base::text::Lang;
use ritsu_testkit::TempDir;
use std::path::Path;

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

fn v(s: &str) -> Version {
    Version::parse(s).unwrap()
}

fn name(s: &str) -> ModuleName {
    ModuleName::parse(s).unwrap()
}

fn read(text: &str) -> manifest::Manifest {
    manifest::parse(text.as_bytes(), "ritsu.mod").unwrap_or_else(|ds| panic!("{text}\n{}", ds.iter().map(|d| d.render(Lang::En)).collect::<String>()))
}

/// The one diagnostic of a `ritsu.mod` that does not read: its code and its English message.
fn one(text: &str) -> (String, String) {
    match manifest::parse(text.as_bytes(), "ritsu.mod") {
        Ok(_) => panic!("read, and should not have:\n{text}"),
        Err(ds) => {
            assert_eq!(ds.len(), 1, "{text}\n{ds:?}");
            (ds[0].code.to_string(), ds[0].message.en.clone())
        }
    }
}

fn load(t: &TempDir, root: &str, store: Option<&Store>) -> Result<Tree, Problem> {
    graph::load_with(&t.path().join(root), store).0
}

/// The code, the file, the line and the English message of each diagnostic of a tree that does not
/// read, or of each thing that stops it.
fn problem(r: Result<Tree, Problem>) -> Vec<String> {
    match r {
        Ok(t) => panic!("read: {:?}", t.nodes.iter().map(|n| n.name.to_string()).collect::<Vec<_>>()),
        Err(Problem::Diags(ds)) => ds.iter().map(|d| format!("{} {}{}: {}", d.code, d.file, d.line.map(|l| format!(":{l}")).unwrap_or_default(), d.message.en)).collect(),
        Err(Problem::Stop(ts)) => ts.iter().map(|t| format!("stop: {}", t.en)).collect(),
    }
}

/// The module whose root is `dir` placed in `store` as a fetch from the git repository `url` places
/// it (the entry, its record and its line of the git index), and its line of a lock.
fn put_git(store: &Store, dir: &Path, url: &str) -> Locked {
    let placed = store::put_directory(store, dir, "2026-10-29", &[]).unwrap_or_else(|e| panic!("{}", e.en));
    store::put_git_index(store, &placed.name, &placed.version, COMMIT, &placed.digest).unwrap();
    Locked {
        name: placed.name.clone(),
        pinned: Pinned::Git { version: placed.version, digest: placed.digest, files: placed.files_digest, commit: COMMIT.into(), url: url.into() },
        line: 0,
    }
}

fn put_oci(store: &Store, dir: &Path) -> Locked {
    store::put_directory(store, dir, "2026-10-29", &[]).unwrap_or_else(|e| panic!("{}", e.en)).locked()
}

fn write_lock(t: &TempDir, root: &str, lines: Vec<Locked>) {
    let mut l = Lock::new(lines);
    l.ritsu = v("0.37.0");
    t.write(&format!("{root}/ritsu.lock"), l.text());
}

#[test]
fn a_git_require_is_read_written_and_changed_a_word_at_a_time() {
    let m = read("module shop\nrequire rates git \"https://example.com/acme/rates\" 1.2.0\nrequire tax git \"git@example.com:acme/mono.git\" 1.0.0 dir \"modules/tax\"  # the tax module\n");
    assert_eq!(m.requires[0].source, Source::Git { url: "https://example.com/acme/rates".into(), min: v("1.2.0"), dir: None });
    assert_eq!(m.requires[1].source, Source::Git { url: "git@example.com:acme/mono.git".into(), min: v("1.0.0"), dir: Some("modules/tax".into()) });
    // the name is the host, the path without `.git`, and the `dir`
    assert_eq!(m.requires[0].source.module(), Some(name("example.com/acme/rates")));
    assert_eq!(m.requires[1].source.module(), Some(name("example.com/acme/mono/modules/tax")));
    assert_eq!(m.requires[1].source.words(), "git \"git@example.com:acme/mono.git\" 1.0.0 dir \"modules/tax\"");
    // `get` changes the version word alone, `dir` and the comment kept
    let got = m.with_version("tax", &v("1.1.0")).unwrap();
    assert!(got.contains("\nrequire tax git \"git@example.com:acme/mono.git\" 1.1.0 dir \"modules/tax\"  # the tax module\n"), "{got}");
    // `add` writes the line
    let added = read("module shop\n").added("rates", &Source::Git { url: "ssh://git@example.com:2222/acme/rates.git".into(), min: v("2.0.0"), dir: Some("rates".into()) }).unwrap();
    assert_eq!(added, "module shop\n\nrequire rates git \"ssh://git@example.com:2222/acme/rates.git\" 2.0.0 dir \"rates\"\n");
    assert_eq!(read(&added).requires[0].source.module(), Some(name("example.com/acme/rates/rates")));
    // a dependency by a path has no version word to change
    assert_eq!(read("module shop\nrequire p path \"../p\"\n").with_version("p", &v("1.0.0")), Err(EditError::ByPath("p".into())));
    assert_eq!(
        EditError::ByPath("p".into()).text().en,
        "the dependency `p` is read from a path, which has no version to change; to take it from a registry or a git repository, write its line in `ritsu.mod` with `oci` or `git`"
    );
}

#[test]
fn the_urls_a_git_require_does_not_take_are_e302() {
    let req = |url: &str| format!("module shop\nrequire r git \"{url}\" 1.0.0\n");
    let cases: &[(&str, &str)] = &[
        ("git://example.com/acme/rates", "a `git://` URL is not taken (`git://example.com/acme/rates`): the git protocol has neither authentication nor a check of what it sends; write an https or an ssh URL"),
        ("file:///srv/git/rates", "a `file://` URL is not taken (`file:///srv/git/rates`); a module in a directory of this machine is required with `require … path`"),
        ("ext::ssh-proxy %S example.com", "the git URL has a space or a control character in it; write the URL as one word"),
        ("ext::ssh-proxy", "the git URL `ext::ssh-proxy` is of the form `ext::`, for which git runs a program of its own; write an https or an ssh URL"),
        ("http://example.com/acme/rates", "the git URL `http://example.com/acme/rates` is plain http, which is taken for a server on this machine only; write an https or an ssh URL"),
        ("ftp://example.com/acme/rates", "the scheme `ftp` of the git URL `ftp://example.com/acme/rates` is not taken; write an https or an ssh URL"),
        ("-oProxyCommand=x", "the git URL `-oProxyCommand=x` starts with a `-`, which git reads as an option"),
        ("git@-oProxyCommand=x:a", "the host of the git URL `git@-oProxyCommand=x:a` starts with a `-`"),
        ("example.com:acme/rates", "the git URL `example.com:acme/rates` has no user; write the scp form with a user, as `git@example.com:acme/rates.git`, or an `ssh://` or `https://` URL"),
        ("../rates", "`../rates` is no git URL; write an https or an ssh URL, and a module in a directory of this machine with `require … path`"),
        ("https://Example.com/acme/rates", "the host of the git URL `https://Example.com/acme/rates` has an uppercase letter in it; write it in lowercase (it is part of the module's name, which is lowercase)"),
        ("https://example.com", "the git URL `https://example.com` names no repository on its host"),
        ("https://example.com:0/acme/rates", "the port `0` of the git URL `https://example.com:0/acme/rates` is a number from 1 to 65535"),
    ];
    for (url, en) in cases {
        let (code, message) = one(&req(url));
        assert_eq!((code.as_str(), message.as_str()), ("E302", *en), "{url}");
    }
    // the name a URL gives: lowercase, and of the form of a published name
    let (code, message) = one(&req("https://example.com/Acme/Rates"));
    assert_eq!(code, "E302");
    assert!(message.starts_with("the module name `example.com/Acme/Rates` the git URL and `dir` give has an uppercase letter in it, and a module's name is lowercase: write them in lowercase."), "{message}");
    assert!(one(&req("https://example.com/acme/ra..tes")).1.starts_with("the module name `example.com/acme/ra..tes` the git URL and `dir` give is not a published module's name: "));
    // `dir`: a path in the repository, said at its column
    let ds = manifest::parse(b"module shop\nrequire r git \"https://example.com/acme/mono\" 1.0.0 dir \"../tax\"\n", "ritsu.mod").unwrap_err();
    assert_eq!(
        (ds[0].code, ds[0].col, ds[0].message.en.as_str()),
        ("E302", Some(57), "the `dir` `../tax` is a directory from the root of the repository, `/` between its parts, with neither `.` nor `..`")
    );
    // the version, at its column
    let ds = manifest::parse(b"module shop\nrequire r git \"https://example.com/acme/rates\" v1.0.0\n", "ritsu.mod").unwrap_err();
    assert_eq!((ds[0].col, ds[0].message.en.as_str()), (Some(48), "`v1.0.0` is not a version: write it with no `v` before it"));
    // what is taken: an ssh user, a port, `.git` and a `/` at the end, a loopback http
    for (url, module) in [
        ("ssh://git@example.com:2222/acme/rates.git", "example.com/acme/rates"),
        ("git@example.com:acme/rates.git/", "example.com/acme/rates"),
        ("https://example.com:8443/acme/rates/", "example.com/acme/rates"),
        ("http://127.0.0.1:8080/acme/rates", "127.0.0.1/acme/rates"),
        ("http://localhost/acme/rates.git", "localhost/acme/rates"),
    ] {
        assert_eq!(read(&req(url)).requires[0].source.module(), Some(name(module)), "{url}");
    }
}

/// A URL with a user or a password in it, or a query, may carry a credential: the diagnostic shows
/// neither the URL nor its line, in either language, and the note says why.
#[test]
fn a_git_url_that_may_carry_a_credential_is_not_shown() {
    for url in [
        "https://ci:hunter2@example.com/acme/rates",
        "https://hunter2@example.com/acme/rates",
        "ssh://git:hunter2@example.com/acme/rates",
        "https://example.com/acme/rates?private_token=hunter2",
        "https://example.com/acme/rates#hunter2",
    ] {
        let text = format!("module shop\nrequire r git \"{url}\" 1.0.0\n");
        let ds = manifest::parse(text.as_bytes(), "ritsu.mod").unwrap_err();
        assert_eq!((ds.len(), ds[0].code), (1, "E302"), "{url}");
        assert!(!ds[0].notes.is_empty(), "{url}");
        for lang in [Lang::En, Lang::Ja] {
            let said = format!("{}{}", ds[0].render(lang), ds[0].to_json(lang).compact());
            assert!(!said.contains("hunter2") && !said.contains("example.com"), "{url}: {said}");
        }
    }
    // an ssh user is no credential
    read("module shop\nrequire r git \"ssh://deploy@example.com/acme/rates\" 1.0.0\n");
}

#[test]
fn the_versions_are_the_tags_of_their_form_alone() {
    let out = format!(
        "{a}\trefs/tags/v1.0.0\n{b}\trefs/tags/v1.1.0\n{c}\trefs/tags/v1.1.0^{{}}\n{a}\trefs/tags/v01.0.0\n{a}\trefs/tags/v1.2.0+build.5\n{a}\trefs/tags/1.3.0\n{a}\trefs/tags/modules/tax/v2.0.0\n{a}\trefs/tags/v2.0.0-rc.1\n",
        a = "a".repeat(40),
        b = "b".repeat(40),
        c = "c".repeat(40)
    );
    let got: Vec<(String, String)> = git::versions_of(&out, None).into_iter().map(|(v, c)| (v.to_string(), c[..1].to_string())).collect();
    // the annotated tag's commit, `^{}`; no `v01.0.0`, no `+`, no tag without `v`, no tag of a directory
    assert_eq!(got, [("1.0.0".into(), "a".into()), ("1.1.0".into(), "c".into()), ("2.0.0-rc.1".into(), "a".into())]);
    let got: Vec<String> = git::versions_of(&out, Some("modules/tax")).into_iter().map(|(v, _)| v.to_string()).collect();
    assert_eq!(got, ["2.0.0"]);
    assert_eq!(git::tag_of(Some("modules/tax"), &v("1.0.0")), "modules/tax/v1.0.0");
    assert_eq!(git::tag_pattern(None), "refs/tags/v*");
}

#[test]
fn a_module_has_one_source_in_a_tree() {
    let t = TempDir::new("git-one-source");
    t.write("shop/ritsu.mod", "module shop\nrequire a path \"../a\"\nrequire b path \"../b\"\n");
    t.write("a/ritsu.mod", "module a\nrequire rates oci \"example.com/acme/rates\" 1.0.0\n");
    t.write("b/ritsu.mod", "module b\nrequire rates git \"https://example.com/acme/rates\" 1.1.0\n");
    // the module at hand names neither: E306 at the second, with both (and with no lock, the E303
    // of the first)
    let want = "E306 @b/ritsu.mod:2: the module `example.com/acme/rates` comes from two sources, `require rates oci \"example.com/acme/rates\" 1.0.0` in `@a/ritsu.mod` and `require rates git \"https://example.com/acme/rates\" 1.1.0` in `@b/ritsu.mod`; write the one to take in a `require` of the module at hand";
    let got = problem(load(&t, "shop", None));
    assert_eq!((got.len(), got[0].split(':').next(), got[1].as_str()), (2, Some("E303 @a/ritsu.mod"), want));
    let chose = graph::choose(&t.path().join("shop"), |_, _, _| unreachable!("nothing is read before the sources are one"));
    assert_eq!(problem(chose.map(|_| unreachable!())), [want]);
    // two git URLs of one module are two sources too
    t.write("a/ritsu.mod", "module a\nrequire rates git \"ssh://git@example.com/acme/rates\" 1.0.0\n");
    assert_eq!(problem(load(&t, "shop", None))[1].split(':').next(), Some("E306 @b/ritsu.mod"));
    // the module at hand chooses: git, from its URL, at the highest version any `require` asks for
    t.write("shop/ritsu.mod", "module shop\nrequire a path \"../a\"\nrequire b path \"../b\"\nrequire rates git \"https://example.com/acme/rates\" 1.0.0\n");
    let mut asked = Vec::new();
    let chosen = graph::choose(&t.path().join("shop"), |n, o, x| {
        asked.push(format!("{n} {} {x}", o.url().unwrap_or("oci")));
        Ok(format!("module {n}\nversion {x}\n").into_bytes())
    })
    .unwrap_or_else(|p| panic!("{}", p.text().en));
    asked.sort();
    assert_eq!(asked, ["example.com/acme/rates https://example.com/acme/rates 1.0.0", "example.com/acme/rates https://example.com/acme/rates 1.1.0"]);
    assert_eq!(chosen.versions[&name("example.com/acme/rates")], v("1.1.0"));
    assert_eq!(chosen.origin(&name("example.com/acme/rates")), Origin::Git { url: "https://example.com/acme/rates".into(), dir: None });
    // the lock of it: a git line, with the commit; none, and it cannot be written
    let d = format!("sha256:{}", "0".repeat(64));
    let lock = graph::lock_of(&chosen, |_, o, _| Some((d.clone(), d.clone(), o.url().map(|_| COMMIT.to_string())))).unwrap();
    assert!(lock.text().contains(&format!("\ngit example.com/acme/rates 1.1.0 {d} files {d} commit {COMMIT} \"https://example.com/acme/rates\"\n")), "{}", lock.text());
    assert_eq!(graph::lock_of(&chosen, |_, _, _| Some((d.clone(), d.clone(), None))).unwrap_err().en, "the commit of `example.com/acme/rates` 1.1.0 is not known");
    // two `require`s of the module at hand: keep one
    t.write("shop/ritsu.mod", "module shop\nrequire rates git \"https://example.com/acme/rates\" 1.0.0\nrequire rates2 oci \"example.com/acme/rates\" 1.0.0\n");
    assert_eq!(
        problem(load(&t, "shop", None))[1..],
        ["E306 ritsu.mod:3: the module `example.com/acme/rates` comes from two sources, `require rates git \"https://example.com/acme/rates\" 1.0.0` and `require rates2 oci \"example.com/acme/rates\" 1.0.0` in `ritsu.mod`; keep one `require` of it in the module at hand"]
    );
    // a path still wins over both
    t.write("rates/ritsu.mod", "module example.com/acme/rates\n");
    t.write("shop/ritsu.mod", "module shop\nrequire a path \"../a\"\nrequire b path \"../b\"\nrequire rates path \"../rates\"\n");
    let tree = load(&t, "shop", None).unwrap();
    assert_eq!(tree.path_won, [name("example.com/acme/rates")]);
}

#[test]
fn a_module_from_git_is_read_from_the_store_and_held_to_the_lock() {
    let t = TempDir::new("git-store");
    let store = Store::at(t.path().join("store"));
    let url = "https://example.com/acme/rates";
    t.write("src/rates/ritsu.mod", "module example.com/acme/rates\nversion 1.2.0\nrequire tax oci \"example.com/acme/tax\" 1.0.0\n");
    t.write("src/rates/rules/rate.rule", "# rate\n");
    t.write("src/tax/ritsu.mod", "module example.com/acme/tax\nversion 1.0.0\n");
    t.write("shop/ritsu.mod", format!("module shop\nrequire rates git \"{url}\" 1.2.0\n"));
    // no lock: E303, which says git
    assert_eq!(
        problem(load(&t, "shop", Some(&store))),
        ["E303 ritsu.mod:2: there is no `ritsu.lock`: the dependency `rates` comes from a git repository, so the version chosen, its digests and its commit are pinned in `ritsu.lock`; run `ritsu mod lock`"]
    );
    let rates = put_git(&store, &t.path().join("src/rates"), url);
    let tax = put_oci(&store, &t.path().join("src/tax"));
    write_lock(&t, "shop", vec![rates.clone(), tax.clone()]);
    let tree = load(&t, "shop", Some(&store)).unwrap();
    let r = &tree.nodes[tree.by_prefix("@rates").unwrap()];
    assert_eq!((r.git(), r.version().map(|x| x.to_string())), (Some((COMMIT, url)), Some("1.2.0".into())));
    assert_eq!(r.origin(), Some(Origin::Git { url: url.into(), dir: None }));
    assert!(r.dir.join("rules/rate.rule").is_file());
    assert_eq!(tree.nodes[tree.by_prefix("@rates/@tax").unwrap()].origin(), Some(Origin::Oci));
    // the lock takes it from a registry, or from another URL: E304
    write_lock(&t, "shop", vec![put_oci(&store, &t.path().join("src/rates")), tax.clone()]);
    assert_eq!(
        problem(load(&t, "shop", Some(&store))),
        [format!("E304 ritsu.mod:2: `ritsu.lock` does not match `ritsu.mod`: `example.com/acme/rates` comes from the git repository `{url}`, and the lock takes it from a registry; run `ritsu mod lock`")]
    );
    let Pinned::Git { version, digest, files, commit, .. } = rates.pinned.clone() else { unreachable!() };
    let elsewhere = Locked { name: rates.name.clone(), pinned: Pinned::Git { version, digest, files, commit, url: "ssh://git@example.com/acme/rates".into() }, line: 0 };
    write_lock(&t, "shop", vec![elsewhere, tax.clone()]);
    assert_eq!(
        problem(load(&t, "shop", Some(&store))),
        [format!("E304 ritsu.mod:2: `ritsu.lock` does not match `ritsu.mod`: `example.com/acme/rates` comes from the git repository `{url}`, and the lock takes it from the git repository `ssh://git@example.com/acme/rates`; run `ritsu mod lock`")]
    );
    // a module from git may require from a registry and from git, not by a path (E308)
    t.write("src/rates/ritsu.mod", "module example.com/acme/rates\nversion 1.2.0\nrequire local path \"../local\"\n");
    let rates = put_git(&store, &t.path().join("src/rates"), url);
    write_lock(&t, "shop", vec![rates]);
    assert_eq!(
        problem(load(&t, "shop", Some(&store))),
        ["E308 @rates/ritsu.mod:3: the module `example.com/acme/rates` 1.2.0, from a git repository, has `require local path \"../local\"`; a dependency by a path is not where the module is used, so it cannot be read"]
    );
    // a module from a registry cannot require from git (E308)
    t.write("src/tax/ritsu.mod", "module example.com/acme/tax\nversion 1.0.0\nrequire rates git \"https://example.com/acme/rates\" 1.0.0\n");
    t.write("shop/ritsu.mod", "module shop\nrequire tax oci \"example.com/acme/tax\" 1.0.0\n");
    write_lock(&t, "shop", vec![put_oci(&store, &t.path().join("src/tax"))]);
    assert_eq!(
        problem(load(&t, "shop", Some(&store))),
        ["E308 @tax/ritsu.mod:3: the module `example.com/acme/tax` 1.0.0, from a registry, has `require rates git \"https://example.com/acme/rates\" 1.0.0`; a module from a registry takes its dependencies from registries, since the one who takes it may not reach the git repository, and a dependency from git has no signature to check"]
    );
}

#[test]
fn a_signature_line_over_a_module_from_git_is_e309() {
    let t = TempDir::new("git-e309");
    let store = Store::at(t.path().join("store"));
    let url = "https://example.com/acme/rates";
    t.write("src/rates/ritsu.mod", "module example.com/acme/rates\nversion 1.2.0\n");
    t.write("shop/keys/acme.pub", "key");
    t.write("shop/ritsu.mod", format!("module shop\nrequire rates git \"{url}\" 1.2.0\nsignature \"example.com/acme/\" key \"keys/acme.pub\"\n"));
    write_lock(&t, "shop", vec![put_git(&store, &t.path().join("src/rates"), url)]);
    let r = load(&t, "shop", Some(&store));
    let Err(Problem::Diags(ds)) = &r else { panic!("{:?}", r.map(|_| ())) };
    assert_eq!(
        problem(r.clone()),
        ["E309 ritsu.mod:3: the start `example.com/acme/` of a `signature` line covers `example.com/acme/rates`, a module from a git repository; ritsu checks the signatures of modules from registries only. Narrow the start so that it does not cover `example.com/acme/rates`, or take `example.com/acme/rates` from a registry"]
    );
    assert_eq!(
        ds[0].message.ja,
        "`signature` の頭の部分 `example.com/acme/` が、git のリポジトリから取るモジュール `example.com/acme/rates` に当たります。ritsu が署名を確かめられるのは、レジストリから取るモジュールだけです。頭の部分を `example.com/acme/rates` に当たらないように狭めるか、`example.com/acme/rates` をレジストリから取ってください"
    );
    // a line that covers another module is not about this one
    t.write("shop/ritsu.mod", format!("module shop\nrequire rates git \"{url}\" 1.2.0\nsignature \"example.com/other/\" key \"keys/acme.pub\"\n"));
    load(&t, "shop", Some(&store)).unwrap();
}

/// Two modules require one from a registry that the store lacks: the tree stops once, and ritsu
/// does not fall over the name it took for the module it could not read.
#[test]
fn two_requires_of_a_module_the_store_lacks_stop_once() {
    let t = TempDir::new("git-missing-twice");
    t.write("shop/ritsu.mod", "module shop\nrequire a path \"../a\"\nrequire x oci \"example.com/acme/x\" 1.0.0\n");
    t.write("a/ritsu.mod", "module a\nrequire x oci \"example.com/acme/x\" 1.0.0\n");
    let d = format!("sha256:{}", "0".repeat(64));
    write_lock(&t, "shop", vec![
        Locked { name: name("a"), pinned: Pinned::Path { path: "../a".into() }, line: 0 },
        Locked { name: name("example.com/acme/x"), pinned: Pinned::Oci { version: v("1.0.0"), digest: d.clone(), files: d }, line: 0 },
    ]);
    let empty = Store::at(t.path().join("empty"));
    assert_eq!(problem(load(&t, "shop", Some(&empty))), ["stop: the dependency `example.com/acme/x` 1.0.0 is neither in the store nor in this module's copy of it; run `ritsu mod fetch`"]);
}

/// The files of a commit are chosen by the rule the files of a directory are (DESIGN 17.12, step 2):
/// the same tree, listed as `git ls-tree -r -t` lists it, gives the same files.
#[test]
fn the_files_of_a_tree_of_git_are_those_of_the_directory() {
    let t = TempDir::new("git-carried");
    let files = [
        ("ritsu.mod", "module example.com/acme/rates\nversion 1.0.0\nexclude \"tests\"\nexclude \"docs/drafts\"\n"),
        ("rules/rate.rule", "# rate\n"),
        ("rules/.hidden.rule", "# hidden\n"),
        ("node_modules/x/index.js", "x\n"),
        ("tests/rate_test.rule", "# test\n"),
        ("docs/guide.md", "guide\n"),
        ("docs/drafts/next.md", "next\n"),
        ("examples/shop/ritsu.mod", "module shop\n"),
        ("examples/shop/main.rule", "# main\n"),
        ("examples/readme.md", "examples\n"),
        (".github/workflows/ci.yml", "ci\n"),
    ];
    for (p, b) in files {
        t.write(&format!("rates/{p}"), b);
    }
    let dir = t.path().join("rates");
    let m = read(files[0].1);
    let on_disk: Vec<String> = digest::module_files(&dir, &m).unwrap().into_iter().map(|(p, _)| p).collect();
    assert_eq!(on_disk, ["docs/guide.md", "examples/readme.md", "ritsu.mod", "rules/rate.rule"]);
    // the listing of a tree: each directory, then what is in it
    let mut listing: Vec<(String, bool)> = Vec::new();
    for (p, _) in files {
        let parts: Vec<&str> = p.split('/').collect();
        for k in 1..parts.len() {
            let d = parts[..k].join("/");
            if !listing.contains(&(d.clone(), true)) {
                listing.push((d, true));
            }
        }
        listing.push((p.to_string(), false));
    }
    let entries: Vec<(&str, bool)> = listing.iter().map(|(p, d)| (p.as_str(), *d)).collect();
    let excludes = digest::excludes_of(&m);
    let chosen: Vec<&str> = digest::carried_of_tree(&entries, &excludes).unwrap().into_iter().map(|i| entries[i].0).collect();
    assert_eq!(chosen, on_disk);
    // a name that starts with `@` right under the root is refused by both
    let at = [("ritsu.mod", false), ("@tax", true), ("@tax/a.rule", false)];
    assert!(digest::carried_of_tree(&at, &[]).unwrap_err().en.starts_with("the root of the module has `@tax` in it"));
}

#[test]
fn the_store_keeps_the_versions_from_git_apart_from_refs() {
    let t = TempDir::new("git-index");
    let store = Store::at(t.path().join("store"));
    let rates = name("example.com/acme/rates");
    let d = format!("sha256:{}", "a".repeat(64));
    assert_eq!(store.find_git(&rates, &v("1.0.0")), None);
    store::put_git_index(&store, &rates, &v("1.0.0"), COMMIT, &d).unwrap();
    assert_eq!(store.find_git(&rates, &v("1.0.0")), Some((COMMIT.to_string(), d.clone())));
    assert_eq!(std::fs::read_to_string(t.path().join("store/git/example.com/acme/rates/1.0.0")).unwrap(), format!("{COMMIT} {d}\n"));
    // the index of the registries is another
    assert_eq!(store.find(&rates, &v("1.0.0")), None);
    // a record of a module from git says the URL and the commit; one from a registry is as it was
    let mut r = Record { module: rates.to_string(), version: "1.0.0".into(), files: d.clone(), fetched: "2026-10-29".into(), verified: Vec::new(), git: None };
    assert!(!r.to_json().compact().contains("git"));
    r.git = Some(("https://example.com/acme/rates".into(), COMMIT.into()));
    let j = r.to_json();
    assert!(j.compact().ends_with(&format!(",\"git\":{{\"url\":\"https://example.com/acme/rates\",\"commit\":\"{COMMIT}\"}}}}")), "{}", j.compact());
    assert_eq!(Record::from_json(&j), Some(r));
    // a line that is not a commit and a digest is no line
    std::fs::write(t.path().join("store/git/example.com/acme/rates/1.0.0"), "HEAD sha256:x\n").unwrap();
    assert_eq!(store.find_git(&rates, &v("1.0.0")), None);
}

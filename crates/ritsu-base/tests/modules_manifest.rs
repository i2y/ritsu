//! `ritsu.mod` (DESIGN 17.3, 17.5): read back to the same bytes, changed a line at a time with the
//! comments, the order and the line endings kept, each wrong line E301 or E302 with the reason, the
//! order of versions (SemVer's section 11), and the edges of the forms of the names.

use ritsu_base::modules::manifest::{self, EditError, Policy, Source};
use ritsu_base::modules::names::{self, ModuleName, Target};
use ritsu_base::modules::semver::{self, Version};

/// The example of DESIGN 17.3 (the English example project's `pricing`).
const PRICING: &str = r#"# The prices of the Acme shop, shared by its services.
module example.com/acme/pricing
version 1.1.0
description "Shipping fees and order totals of the Acme shop"
ritsu >=0.37.0
license "MIT OR Apache-2.0"
source "https://example.com/acme/pricing"

require tax oci "example.com/acme/tax" 1.0.0

signature "example.com/acme/" key "keys/acme.pub"

exclude "generated"
exclude "tests"

library typescript "@acme/pricing"
library python "acme-pricing"
library go "example.com/acme/pricing-go"
"#;

fn v(s: &str) -> Version {
    Version::parse(s).unwrap_or_else(|e| panic!("{s}: {:?}", e))
}

fn read(text: &str) -> manifest::Manifest {
    manifest::parse(text.as_bytes(), "ritsu.mod").unwrap_or_else(|ds| panic!("{text}\n{}", ds.iter().map(|d| d.render(ritsu_base::text::Lang::En)).collect::<String>()))
}

/// The code and the English message of each diagnostic of a text that does not read.
fn refused(text: &str) -> Vec<(String, String)> {
    match manifest::parse(text.as_bytes(), "ritsu.mod") {
        Ok(_) => panic!("read, and should not have:\n{text}"),
        Err(ds) => ds.iter().map(|d| (d.code.to_string(), d.message.en.clone())).collect(),
    }
}

/// The one diagnostic of a text: its code and its English message.
fn one(text: &str) -> (String, String) {
    let ds = refused(text);
    assert_eq!(ds.len(), 1, "{text}\n{ds:?}");
    ds.into_iter().next().unwrap()
}

#[test]
fn the_example_reads_every_statement() {
    let m = read(PRICING);
    assert_eq!(m.module.value, ModuleName::Published { registry: "example.com".into(), repository: "acme/pricing".into() });
    assert_eq!((m.module.line, m.module.col), (2, 8));
    assert_eq!(m.version.as_ref().map(|x| x.value.clone()), Some(v("1.1.0")));
    assert_eq!(m.description.as_ref().map(|x| x.value.as_str()), Some("Shipping fees and order totals of the Acme shop"));
    assert_eq!(m.ritsu.as_ref().map(|x| x.value.clone()), Some(v("0.37.0")));
    assert_eq!(m.license.as_ref().map(|x| x.value.as_str()), Some("MIT OR Apache-2.0"));
    assert_eq!(m.source.as_ref().map(|x| x.value.as_str()), Some("https://example.com/acme/pricing"));
    assert_eq!(m.requires.len(), 1);
    assert_eq!(m.requires[0].name, "tax");
    assert_eq!(m.requires[0].source, Source::Oci { repository: "example.com/acme/tax".into(), min: v("1.0.0") });
    assert_eq!(m.excludes.iter().map(|e| e.value.as_str()).collect::<Vec<_>>(), ["generated", "tests"]);
    assert_eq!(m.signatures.len(), 1);
    assert_eq!(m.signatures[0].policy, Policy::Key { path: "keys/acme.pub".into(), tlog: true });
    assert_eq!(m.signatures[0].text(), r#"signature "example.com/acme/" key "keys/acme.pub""#);
    let libs: Vec<(Target, &str)> = m.libraries.iter().map(|l| (l.target, l.name.as_str())).collect();
    assert_eq!(libs, [(Target::Typescript, "@acme/pricing"), (Target::Python, "acme-pricing"), (Target::Go, "example.com/acme/pricing-go")]);
}

#[test]
fn what_is_read_is_written_back_byte_for_byte() {
    let texts = [
        PRICING.to_string(),
        PRICING.replace('\n', "\r\n"),
        PRICING.trim_end().to_string(),
        "module shop\n".to_string(),
        "  # a comment first\n\nmodule shop   # the name\n\trequire pricing path \"../pricing\"  # beside it\n".to_string(),
        "module shop\nrequire a path \"../a\"\r\nrequire b path \"../b\"".to_string(),
    ];
    for t in texts {
        assert_eq!(read(&t).text(), t);
    }
}

#[test]
fn every_form_of_signature_and_require_reads() {
    let m = read(concat!(
        "module shop\n",
        "require pricing path \"../pricing\"\n",
        "require tax oci \"localhost:5000/acme/tax\" 1.2.0-rc.1\n",
        "signature \"localhost:5000/acme/tax\" key \"keys/tax.pub\" tlog off\n",
        "signature \"example.com/\" by \"https://github.com/acme/tax/.github/workflows/release.yml@refs/heads/main\" issuer \"https://token.actions.githubusercontent.com\"\n",
        "signature \"example.com/acme/\" by matching \"^https://github.com/acme/\" issuer \"https://token.actions.githubusercontent.com\"\n",
    ));
    assert_eq!(m.requires[0].source, Source::Path { path: "../pricing".into() });
    assert_eq!(m.requires[1].source, Source::Oci { repository: "localhost:5000/acme/tax".into(), min: v("1.2.0-rc.1") });
    assert_eq!(m.signatures[0].policy, Policy::Key { path: "keys/tax.pub".into(), tlog: false });
    assert!(matches!(&m.signatures[1].policy, Policy::By { identity, issuer } if identity.ends_with("@refs/heads/main") && issuer.starts_with("https://token")));
    assert!(matches!(&m.signatures[2].policy, Policy::Matching { pattern, .. } if pattern == "^https://github.com/acme/"));
    for s in &m.signatures {
        assert_eq!(read(&format!("module shop\n{}\n", s.text())).signatures[0], manifest::Signature { line: 2, ..s.clone() }, "a signature line written back reads the same");
    }
    assert_eq!(m.module.value, ModuleName::Local("shop".into()));
}

#[test]
fn add_puts_a_line_after_the_last_require_and_keeps_the_rest() {
    let m = read(PRICING);
    let got = m.added("rates", &Source::Path { path: "../rates".into() }).unwrap();
    assert_eq!(got, PRICING.replace("1.0.0\n\nsignature", "1.0.0\nrequire rates path \"../rates\"\n\nsignature"));
    // CRLF kept, and the new line takes it
    let crlf = PRICING.replace('\n', "\r\n");
    let got = read(&crlf).added("rates", &Source::Path { path: "../rates".into() }).unwrap();
    assert_eq!(got, crlf.replace("1.0.0\r\n\r\nsignature", "1.0.0\r\nrequire rates path \"../rates\"\r\n\r\nsignature"));
    // no require: after one blank line at the end
    assert_eq!(read("module shop\nversion 0.1.0\n").added("pricing", &Source::Path { path: "../pricing".into() }).unwrap(), "module shop\nversion 0.1.0\n\nrequire pricing path \"../pricing\"\n");
    assert_eq!(read("module shop\n\n").added("pricing", &Source::Path { path: "../pricing".into() }).unwrap(), "module shop\n\nrequire pricing path \"../pricing\"\n", "a blank last line is the blank line");
    // no ending on the last line, kept so
    assert_eq!(read("module shop").added("pricing", &Source::Path { path: "../pricing".into() }).unwrap(), "module shop\n\nrequire pricing path \"../pricing\"");
    assert_eq!(read("module shop\nrequire a path \"../a\"").added("b", &Source::Path { path: "../b".into() }).unwrap(), "module shop\nrequire a path \"../a\"\nrequire b path \"../b\"");
    // an oci line, its repository quoted
    let oci = Source::Oci { repository: "example.com/acme/rates".into(), min: v("2.0.0") };
    assert!(read(PRICING).added("rates", &oci).unwrap().contains("\nrequire rates oci \"example.com/acme/rates\" 2.0.0\n"));
    // refused
    assert_eq!(m.added("tax", &oci), Err(EditError::Required("tax".into(), 9)));
    assert!(matches!(m.added("Rates", &oci), Err(EditError::BadName(..))));
}

#[test]
fn get_changes_the_version_word_and_no_other_byte() {
    let t = "module shop\nrequire  tax   oci \"example.com/acme/tax\"   1.0.0   # pinned for the audit\r\nrequire p path \"../p\"\n";
    let got = read(t).with_version("tax", &v("1.10.0")).unwrap();
    assert_eq!(got, "module shop\nrequire  tax   oci \"example.com/acme/tax\"   1.10.0   # pinned for the audit\r\nrequire p path \"../p\"\n");
    assert_eq!(read(t).with_version("p", &v("1.0.0")), Err(EditError::ByPath("p".into())));
    assert_eq!(read(t).with_version("q", &v("1.0.0")), Err(EditError::NotRequired("q".into())));
}

#[test]
fn remove_takes_the_line_out_and_leaves_the_comment_above() {
    let t = "module shop\n# the tax rates\nrequire tax oci \"example.com/acme/tax\" 1.0.0\nrequire p path \"../p\"\n";
    assert_eq!(read(t).removed("tax").unwrap(), "module shop\n# the tax rates\nrequire p path \"../p\"\n");
    // the last line, with no ending: the line before loses its ending, so the file still has none
    assert_eq!(read("module shop\r\nrequire p path \"../p\"").removed("p").unwrap(), "module shop");
    assert_eq!(read(t).removed("q"), Err(EditError::NotRequired("q".into())));
}

#[test]
fn init_writes_the_name_the_version_the_least_ritsu_and_exclude_generated() {
    let name = ModuleName::parse("example.com/acme/shop").unwrap();
    let t = manifest::init(&name, &v("0.1.0"), &v("0.37.0"));
    assert_eq!(t, "module example.com/acme/shop\nversion 0.1.0\nritsu >=0.37.0\n\nexclude \"generated\"\n");
    assert_eq!(read(&t).excludes[0].value, "generated");
}

#[test]
fn a_line_that_cannot_be_read_is_e301() {
    let cases: &[(&str, &str)] = &[
        ("module shop\nrequires x path \"../x\"\n", "no statement starts with `requires`; a statement starts with one of `module`, `version`, `description`, `ritsu`, `license`, `source`, `require`, `exclude`, `signature` and `library`"),
        ("module shop\nversion\n", "write the `version` line as `version <version>`"),
        ("module shop\nversion 1.0.0 1.1.0\n", "write the `version` line as `version <version>`"),
        ("module shop\ndescription prices\n", "write the `description` line as `description \"<text>\"`"),
        ("module \"shop\"\n", "write the `module` line as `module <module name>`"),
        ("module shop\nversion 1.0.0\nversion 1.1.0\n", "`version` is written once, and it is already on line 2"),
        ("module shop\nmodule shop\n", "`module` is written once, and it is already on line 1"),
        ("module shop\nlibrary go \"example.com/a\"\nlibrary go \"example.com/b\"\n", "`library go` is written once, and it is already on line 2"),
        ("module shop\nlibrary rust \"x\"\n", "`rust` is not a target of `library`; the targets are typescript, python and go"),
        ("version 1.0.0\nmodule shop\n", "the first statement is `module`: start `ritsu.mod` with the line `module <module name>`"),
        ("# only a comment\n", "there is no `module` line: start `ritsu.mod` with the line `module <module name>`"),
        ("module\u{3000}shop\n", "there is a full-width space outside a string; a value with a blank in it is written in `\"…\"`"),
        ("module shop\ndescription \"a\\tb\"\n", "the escape `\\t` is not taken in a string; only `\\\"` and `\\\\` are"),
        ("module shop\ndescription \"open\n", "a string is not closed"),
        ("module shop\u{7}\n", "the control character U+0007 cannot be written here (the tab is the only one taken)"),
        ("\"module\" shop\n", "a statement starts with a word, and `\"module\"` is a string"),
        ("module shop\nrequire x oci \"example.com/x\"\n", "write the `require` line as `require <name> oci \"<repository>\" <version>`, `require <name> git \"<URL>\" <version>` (with `dir \"<directory>\"` after it, if need be) or `require <name> path \"<path>\"`"),
        ("module shop\nrequire x git \"https://example.com/x\" 1.0.0 dir\n", "write the `require` line as `require <name> oci \"<repository>\" <version>`, `require <name> git \"<URL>\" <version>` (with `dir \"<directory>\"` after it, if need be) or `require <name> path \"<path>\"`"),
        ("module shop\nsignature \"example.com/\" key\n", "write the `signature` line as `signature \"<start>\" key \"<path>\"` (with `tlog off` after it, if need be), `signature \"<start>\" by \"<identity>\" issuer \"<issuer>\"` or `signature \"<start>\" by matching \"<pattern>\" issuer \"<issuer>\"`"),
        ("module shop\nsignature \"example.com/\" key \"k.pub\" tlog on\n", "write the `signature` line as `signature \"<start>\" key \"<path>\"` (with `tlog off` after it, if need be), `signature \"<start>\" by \"<identity>\" issuer \"<issuer>\"` or `signature \"<start>\" by matching \"<pattern>\" issuer \"<issuer>\"`"),
    ];
    for (text, en) in cases {
        let (code, message) = one(text);
        assert_eq!((code.as_str(), message.as_str()), ("E301", *en), "{text}");
    }
    // bytes that are not UTF-8, at their line
    let ds = manifest::parse(b"module shop\nversion \xff\n", "ritsu.mod").unwrap_err();
    assert_eq!((ds[0].code, ds[0].line, ds[0].message.en.as_str()), ("E301", Some(2), "there are bytes that are not UTF-8"));
    // each wrong line is said, not only the first
    assert_eq!(refused("module shop\nversion\nlicense\n").len(), 2);
}

#[test]
fn a_value_of_the_wrong_form_is_e302() {
    let cases: &[(&str, &str)] = &[
        ("module Shop\n", "`Shop` is not a module's name: the `S` at character 1 is not taken"),
        ("module example/acme/shop\n", "`example/acme/shop` is not a module's name: the registry's host `example` has no `.` in it; a host has one, or is `localhost`, so that it is not taken for a short name"),
        ("module shop\nversion v1.0.0\n", "`v1.0.0` is not a version: write it with no `v` before it"),
        ("module shop\nversion 1.0.0+build.5\n", "`1.0.0+build.5` is not a version: build metadata after a `+` is not taken, since an OCI tag cannot hold a `+`"),
        ("module shop\nversion 1.02.0\n", "`1.02.0` is not a version: the number `02` has a leading zero"),
        ("module shop\nversion 1.0\n", "`1.0` is not a version: write three numbers with dots between them, as `1.4.0`"),
        ("module shop\nritsu 0.37.0\n", "the value of `ritsu` is `>=` and a version as one word, as `>=0.37.0`"),
        ("module shop\nritsu >=0.37\n", "`0.37` is not a version: write three numbers with dots between them, as `1.4.0`"),
        ("module shop\nsource \"http://example.com/acme\"\n", "the `source` `http://example.com/acme` is an https URL"),
        ("module shop\nsource \"https://u:t@github.com/acme\"\n", "the `source` URL has a user or a password in it, which a `source` cannot have"),
        ("module shop\nsource \"https://github.com/acme?tab=x\"\n", "the `source` URL has a query (from the `?` on) in it, which a `source` cannot have"),
        ("module shop\nlicense \"  \"\n", "the `license` is empty; write an SPDX expression (`MIT OR Apache-2.0`, say)"),
        ("module shop\nrequire Pricing path \"../p\"\n", "`Pricing` cannot name a dependency: the `P` at character 1 is not taken"),
        ("module shop\nrequire acme-pricing path \"../p\"\n", "`acme-pricing` cannot name a dependency: a `-` is not taken; join words with `_`"),
        ("module shop\nrequire 料金 path \"../p\"\n", "`料金` cannot name a dependency: the `料` at character 1 is not taken"),
        ("module shop\nrequire p path \"../p\"\nrequire p path \"../q\"\n", "the dependency `p` is already on line 2; a name is required once in a `ritsu.mod`"),
        ("module shop\nrequire p path \"/srv/p\"\n", "`/srv/p` is an absolute path; write it from the root of the module"),
        ("module shop\nrequire p path \"..\\\\p\"\n", "`..\\p` has a `\\` in it; the parts of a path have `/` between them"),
        ("module shop\nrequire p path \"\"\n", "the path is empty"),
        ("module shop\nrequire p oci \"pricing\" 1.0.0\n", "`pricing` is the name of a module that is not published; `oci` takes `<registry>/<repository>`"),
        ("module shop\nrequire p oci \"example.com/Acme\" 1.0.0\n", "`example.com/Acme` is not a module's name: the repository `Acme` is words of lowercase letters and digits joined by `.`, `_`, `__` or `-`, with `/` between them"),
        ("module shop\nrequire p oci \"example.com/acme/p\" 1.0\n", "`1.0` is not a version: write three numbers with dots between them, as `1.4.0`"),
        ("module shop\nexclude \"../other\"\n", "`../other` leads outside the module"),
        ("module shop\nexclude \".\"\n", "`.` is the root of the module itself"),
        ("module shop\nsignature \"pricing\" key \"k.pub\"\n", "the start `pricing` of a `signature` line is the name of a published module, or a start of one that ends with `/` (`\"example.com/acme/\"`, say): it is the name of a module that is not published, and only one published to a registry has a signature to ask for"),
        ("module shop\nsignature \"example.com/\" key \"../k.pub\"\n", "`../k.pub` leads outside the module"),
        ("module shop\nsignature \"example.com/\" by \"\" issuer \"https://x\"\n", "neither who signed nor the issuer can be empty"),
        ("module shop\nlibrary typescript \"Acme\"\n", "`Acme` cannot name the `library typescript`: it is not an npm package's name (`name` or `@scope/name`: lowercase letters, digits, `-`, `.`, `_` and `~`, 214 characters at most)"),
        ("module shop\nlibrary python \"-acme\"\n", "`-acme` cannot name the `library python`: it is not a Python distribution's name (letters and digits, with `.`, `_` and `-` between, starting and ending with a letter or a digit)"),
        ("module shop\nlibrary go \"example.com/acme/pricing/v2\"\n", "`example.com/acme/pricing/v2` cannot name the `library go`: leave out the `/vN` at its end; ritsu adds it from version 2 on"),
        ("module shop\nlibrary go \"acme/pricing\"\n", "`acme/pricing` cannot name the `library go`: it is not a Go module path (a host with a `.` in it first, then parts of letters, digits, `-`, `.`, `_` and `~`, with `/` between them)"),
    ];
    for (text, en) in cases {
        let (code, message) = one(text);
        assert_eq!((code.as_str(), message.as_str()), ("E302", *en), "{text}");
    }
    // a path dependency may lead beside the module, an exclude may not
    read("module shop\nrequire p path \"../../p\"\nexclude \"generated/old\"\n");
}

#[test]
fn the_japanese_texts_say_the_same() {
    let ds = manifest::parse("module shop\nversion v1.0.0\nrequire x-y path \"../x\"\n".as_bytes(), "ritsu.mod").unwrap_err();
    let ja: Vec<&str> = ds.iter().map(|d| d.message.ja.as_str()).collect();
    assert_eq!(ja, ["`v1.0.0` は版として読めません。頭に `v` を付けずに書いてください", "依存の名前 `x-y` は使えません。`-` は使えません。語を区切るときは `_` を使ってください"]);
    assert_eq!(ds[0].render(ritsu_base::text::Lang::Ja), "エラー[E302]: ritsu.mod:2:9: `v1.0.0` は版として読めません。頭に `v` を付けずに書いてください\n     2 | version v1.0.0\n");
}

/// A `source` is published with the module and the packages made from it (DESIGN 17.3), so a URL
/// with a user, a password or a query is E302, and the note says why; the diagnostic shows neither
/// the URL nor its line, which go to terminals and logs. The user is looked for where either the
/// URL Standard or RFC 3986 reads one; an `@` or a `?` elsewhere is no user and no query.
#[test]
fn a_source_has_no_user_no_password_and_no_query() {
    let note_en = "ritsu writes the `source` into the module `ritsu mod push` sends, the packages `ritsu gen` makes (package.json and pyproject.toml) and the list site, all of which are meant to be published: keep credentials such as tokens and passwords out of it";
    let note_ja = "ritsu は `source` を、`ritsu mod push` で送り出すモジュール、`ritsu gen` で生成するパッケージ（package.json と pyproject.toml）、一覧のサイトに書きます。どれも公開するものなので、トークンやパスワードのような資格情報は書かないでください";
    let ds = manifest::parse("module shop\nsource \"https://user:token@github.com/acme/pricing\"\n".as_bytes(), "ritsu.mod").unwrap_err();
    assert_eq!(
        ds[0].render(ritsu_base::text::Lang::Ja),
        format!(
            "エラー[E302]: ritsu.mod:2:8: `source` の URL にユーザーかパスワードが入っています。`source` には書けません\n  = {note_ja}\n"
        )
    );
    assert_eq!(ds[0].notes.iter().map(|n| n.en.as_str()).collect::<Vec<_>>(), [note_en]);
    let ds = manifest::parse("module shop\nsource \"https://gitlab.com/acme/pricing?private_token=x#main\"\n".as_bytes(), "ritsu.mod").unwrap_err();
    assert_eq!(ds[0].message.ja, "`source` の URL にクエリ（`?` から後）が入っています。`source` には書けません");
    for lang in [ritsu_base::text::Lang::En, ritsu_base::text::Lang::Ja] {
        let said = format!("{}{}", ds[0].render(lang), ds[0].to_json(lang).compact());
        assert!(!said.contains("private_token") && !said.contains("gitlab.com"), "{said}");
    }
    assert_eq!(ds[0].notes.iter().map(|n| n.ja.as_str()).collect::<Vec<_>>(), [note_ja]);
    // a URL that is not https has no note: it is not published
    assert!(manifest::parse(b"module shop\nsource \"http://example.com/acme\"\n", "ritsu.mod").unwrap_err()[0].notes.is_empty());
    // a user however the authority is written: the URL Standard skips the `/` and `\` after `https://` and drops a tab
    for url in [
        "https://:token@github.com/acme",
        "https://user@github.com",
        "https://a@b@github.com:443/acme",
        "https://\\\\/user@github.com/acme",
        "https://\t/user@github.com/acme",
        "https://github.com\\\\user@evil.example/acme",
    ] {
        let (code, message) = one(&format!("module shop\nsource \"{url}\"\n"));
        assert_eq!(code, "E302", "{url}");
        assert!(message.ends_with("has a user or a password in it, which a `source` cannot have"), "{url}: {message}");
    }
    assert!(one("module shop\nsource \"https://github.com/acme/pricing?\"\n").1.contains("has a query"), "an empty query is a query");
    // an `@` in the path, a `?` in the fragment, an escaped `@` or `?`, a port: neither user nor query
    for url in ["https://www.npmjs.com/package/@acme/pricing", "https://github.com/acme/pricing#a?b", "https://github.com/acme/pri%3Fce%40x", "https://github.com:443/acme/pricing#v1.2.0"] {
        assert_eq!(read(&format!("module shop\nsource \"{url}\"\n")).source.map(|s| s.value), Some(url.to_string()));
    }
}

#[test]
fn a_newer_ritsu_is_read_from_a_file_that_does_not_read() {
    assert_eq!(manifest::wants(b"module shop\nritsu >=9.0.0\nworkspace \"x\"\n"), Some(v("9.0.0")));
    assert_eq!(manifest::wants(b"module shop\n"), None);
}

#[test]
fn versions_are_in_the_order_of_semvers_section_11() {
    let ordered = [
        "1.0.0-alpha",
        "1.0.0-alpha.1",
        "1.0.0-alpha.beta",
        "1.0.0-beta",
        "1.0.0-beta.2",
        "1.0.0-beta.11",
        "1.0.0-rc.1",
        "1.0.0",
        "2.0.0",
        "2.1.0",
        "2.1.1",
        "10.0.0",
        "18446744073709551616.0.0",
    ];
    for (i, a) in ordered.iter().enumerate() {
        for (j, b) in ordered.iter().enumerate() {
            assert_eq!(v(a).cmp(&v(b)), i.cmp(&j), "{a} and {b}");
        }
    }
    assert_eq!(v("1.0.0-x.7.z.92").to_string(), "1.0.0-x.7.z.92");
    assert!(v("1.0.0-0.3.7").is_prerelease() && !v("1.0.0").is_prerelease());
    // the longest tag, and one over
    let long = format!("1.0.0-{}", "a".repeat(semver::MAX_LEN - 6));
    assert_eq!(v(&long).to_string().len(), semver::MAX_LEN);
    assert_eq!(Version::parse(&format!("{long}b")), Err(semver::Error::TooLong(semver::MAX_LEN + 1)));
    assert_eq!(Version::parse("1.0.0-01"), Err(semver::Error::LeadingZero("01".into())));
    assert_eq!(Version::parse("1.0.0-"), Err(semver::Error::PreRelease(String::new())));
    assert_eq!(Version::parse("1.0.0-a..b"), Err(semver::Error::PreRelease("a..b".into())));
    assert_eq!(Version::parse(""), Err(semver::Error::Empty));
}

#[test]
fn the_edges_of_the_names() {
    // 255 characters in all, and one over
    let repo = |n: usize| format!("example.com/{}", "a".repeat(n - "example.com/".len()));
    assert!(ModuleName::parse(&repo(names::MAX_PUBLISHED)).is_ok());
    assert_eq!(ModuleName::parse(&repo(names::MAX_PUBLISHED + 1)), Err(names::NameError::TooLong { len: 256, max: 255 }));
    // the host: a `.`, or localhost; a port from 1 to 65535
    for ok in ["localhost/x", "localhost:5000/acme/x", "127.0.0.1:5000/x", "registry-1.docker.io/library/x", "ghcr.io/i2y/ritsu-holidays", "a.b/c__d/e--f/g.h_i"] {
        assert!(ModuleName::parse(ok).is_ok(), "{ok}");
    }
    let bad = [
        ("registry/x", names::NameError::ShortHost("registry".into())),
        ("localhost:0/x", names::NameError::Port("0".into())),
        ("localhost:65536/x", names::NameError::Port("65536".into())),
        ("localhost:05000/x", names::NameError::Port("05000".into())),
        ("localhost:/x", names::NameError::Port(String::new())),
        ("-a.b/x", names::NameError::Host("-a.b".into())),
        ("a..b/x", names::NameError::Host("a..b".into())),
        ("A.b/x", names::NameError::Character('A', 0)),
        ("a.b/x_", names::NameError::Repository("x_".into())),
        ("a.b/x___y", names::NameError::Repository("x___y".into())),
        ("a.b/x//y", names::NameError::Repository("x//y".into())),
        ("a.b/", names::NameError::Empty),
    ];
    for (s, e) in bad {
        assert_eq!(ModuleName::parse(s), Err(e), "{s}");
    }
    // a short name: 64 characters, a letter first, `-` taken in a module's name and not in a dependency's
    assert!(ModuleName::parse(&"a".repeat(64)).is_ok());
    assert_eq!(ModuleName::parse(&"a".repeat(65)), Err(names::NameError::TooLong { len: 65, max: 64 }));
    assert!(ModuleName::parse("acme-shop").is_ok());
    assert_eq!(names::dependency_ok("acme-shop"), Err(names::NameError::Dash));
    assert_eq!(names::dependency_ok("_shop"), Err(names::NameError::Start));
    assert_eq!(names::dependency_ok("shop2_eu"), Ok(()));
    // the start of a signature line
    assert!(names::prefix_ok("example.com/").is_ok() && names::prefix_ok("example.com/acme/").is_ok() && names::prefix_ok("example.com/acme/tax").is_ok());
    assert!(names::covers("example.com/acme/", "example.com/acme/tax") && !names::covers("example.com/acme/", "example.com/acmeco/tax"));
    assert!(names::covers("example.com/acme/tax", "example.com/acme/tax") && !names::covers("example.com/acme/tax", "example.com/acme/tax2"));
}

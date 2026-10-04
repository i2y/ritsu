//! The first-level divisions of thirteen countries in the built-in namespace (DESIGN §15.182):
//! how many values each holds, that every spelling of a division is the one value, that the
//! generated code names them as the prelude freezes them in every target, and what a rule hears
//! when it writes a division of a country it does not import.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use ritsu_testkit::TempDir;
use rulec::prelude::{Import, NAMESPACES, lookup};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rulec(lang: &str, args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", lang).current_dir(root()).args(args).output().expect("rulec を起動できない");
    (o.status.code().unwrap_or(-1), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)))
}

fn write(dir: &Path, name: &str, text: &str) -> String {
    let p = dir.join(name);
    std::fs::write(&p, text).unwrap();
    p.to_string_lossy().into_owned()
}

/// A rule with one row per division, the division written as `spell` says, and the row's
/// position as the answer. The examples name the first and the last division the same way.
fn rule(im: &Import, spell: impl Fn(usize, &rulec::prelude::Division) -> String) -> String {
    let ds = im.ns.divisions;
    let mut s = format!("rule pick v1\n\nimport {}\n\ninputs\n  place : {}\n\noutputs\n  n : number  round down(1)\n\ntable t\npolicy unique\n| place | -> n : number |\n", im.path(), im.ty);
    for (i, d) in ds.iter().enumerate() {
        s.push_str(&format!("| {} | {} |\n", spell(i, d), i + 1));
    }
    s.push_str("\nexamples\n| place | -> n |\n");
    for i in [0, ds.len() - 1] {
        s.push_str(&format!("| {} | {} |\n", spell(i, &ds[i]), i + 1));
    }
    s
}

/// Every spelling a division takes, the value first.
fn spellings(im: &Import, d: &rulec::prelude::Division) -> Vec<String> {
    let mut v = vec![im.value(d).to_string(), d.name.to_string()];
    if !d.code.is_empty() {
        v.push(d.code.to_string());
    }
    v.extend(d.spellings.iter().map(|s| s.to_string()));
    v.dedup();
    v
}

/// Every import there is: the thirteen countries, and the prefectures under their Japanese name.
fn all_imports() -> Vec<Import> {
    let mut v: Vec<Import> = NAMESPACES.iter().map(|n| lookup(n.path).unwrap()).collect();
    v.push(lookup("std/都道府県").unwrap());
    v
}

#[test]
fn 名前空間ごとの値の数は作者の決めた数() {
    let want: BTreeMap<&str, usize> = [
        ("std/us/states", 56),
        ("std/gb/nations", 4),
        ("std/cn/provinces", 33),
        ("std/tw/divisions", 22),
        ("std/kr/provinces", 17),
        ("std/in/states", 36),
        ("std/fr/regions", 18),
        ("std/es/communities", 19),
        ("std/it/regions", 20),
        ("std/de/states", 16),
        ("std/au/states", 8),
        ("std/br/states", 27),
        ("std/jp/prefectures", 47),
        ("std/都道府県", 47),
    ]
    .into_iter()
    .collect();
    assert_eq!(all_imports().len(), want.len());
    for im in all_imports() {
        let n = want[im.path()];
        assert_eq!(im.ns.divisions.len(), n, "{}", im.path());
        // The checker holds the same set: one value per division, as the schema lists them.
        let src = rule(&im, |_, d| im.value(d).to_string());
        let (f, c) = rulec::prepare(&src, "pick.rule").unwrap_or_else(|d| panic!("{}: {:?}", im.path(), d.iter().map(|d| d.title.clone()).collect::<Vec<_>>()));
        assert_eq!(c.enums[im.ty].len(), n, "{}", im.path());
        assert_eq!(f.imports.len(), 1);
    }
    // Taiwan is its own namespace, not a province of China's (ISO 3166-2:CN lists it; §15.182).
    let cn = lookup("std/cn/provinces").unwrap();
    assert!(cn.division("Taiwan").is_none() && cn.division("TW").is_none());
    assert!(lookup("std/tw/divisions").unwrap().division("Taipei").is_some());
    // The prefectures are the frozen table, under both names.
    for im in [lookup("std/jp/prefectures").unwrap(), lookup("std/都道府県").unwrap()] {
        for (d, (ja, ascii)) in im.ns.divisions.iter().zip(rulec::prelude::PREFECTURES) {
            assert_eq!((d.name, d.member, d.spellings[0]), (*ascii, *ascii, *ja));
        }
    }
}

#[test]
fn 一つの綴りは一つの区分を指し名前として読める() {
    for im in all_imports() {
        let mut seen: BTreeMap<String, &str> = BTreeMap::new();
        for d in im.ns.divisions {
            for s in spellings(&im, d) {
                assert_eq!(im.resolve(&s), Some(im.value(d)), "{}: {s}", im.path());
                if let Some(other) = seen.insert(s.clone(), d.name) {
                    assert_eq!(other, d.name, "{}: `{s}` names two divisions", im.path());
                }
                assert!(rulec::lex::is_bare_word(&s), "{}: `{s}` is not one name", im.path());
            }
            // The value is ASCII (it is on the wire and in every generated file), and so is the
            // member, a name every target can hold.
            assert!(d.name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'), "{}", d.name);
            assert!(d.member.chars().all(|c| c.is_ascii_alphanumeric()) && d.member.starts_with(|c: char| c.is_ascii_uppercase()), "{}", d.member);
            // A code is written when it starts with a letter; the numeric ones are left out.
            assert!(d.code.is_empty() || d.code.starts_with(|c: char| c.is_ascii_uppercase()), "{}", d.code);
        }
    }
}

#[test]
fn 英語と現地の綴りと符号は同じ値になる() {
    let t = TempDir::new("regions-spell");
    for (k, im) in all_imports().into_iter().enumerate() {
        let names = rule(&im, |_, d| im.value(d).to_string());
        // Each row in another of its spellings, taken in turn: the code, a local name, an
        // English form, the name.
        let mixed = rule(&im, |i, d| {
            let v = spellings(&im, d);
            if v.len() == 1 { v[0].clone() } else { v[(i % (v.len() - 1)) + 1].clone() }
        });
        // Each row in its last spelling: the local name where there is one.
        let local = rule(&im, |_, d| spellings(&im, d).last().unwrap().clone());
        let mut got = Vec::new();
        for (tag, src) in [("names", &names), ("mixed", &mixed), ("local", &local)] {
            let p = write(t.path(), &format!("{k}-{tag}.rule"), src);
            let (c, out) = rulec("en", &["check", &p]);
            assert_eq!(c, 0, "{} {tag}:\n{out}\n{src}", im.path());
            let (c, v) = rulec("en", &["vectors", &p]);
            assert_eq!(c, 0, "{v}");
            got.push(v);
        }
        assert_eq!(got[0], got[1], "{}: the mixed spellings answer differently", im.path());
        assert_eq!(got[0], got[2], "{}: the local spellings answer differently", im.path());
        // The vectors name the values as the import spells them.
        let first = im.value(&im.ns.divisions[0]);
        assert!(got[0].contains(&format!("\"place\":\"{first}\"")), "{}", got[0]);
    }
}

#[test]
fn 生成コードの型と値の名前は凍結した表のとおり() {
    let t = TempDir::new("regions-gen");
    for (k, im) in all_imports().into_iter().enumerate() {
        let src = rule(&im, |_, d| im.value(d).to_string());
        let p = write(t.path(), &format!("pick{k}.rule"), &src.replace("rule pick v1", &format!("rule pick{k} v1")));
        let out = t.path().join(format!("out{k}"));
        let (c, o) = rulec("en", &["gen", &p, "--out", out.to_str().unwrap()]);
        assert_eq!(c, 0, "{o}");
        let ty = im.ns.generated;
        let read = |rel: &str| std::fs::read_to_string(out.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));
        let a = format!("pick{k}");
        // The twelve targets of `backend::ALL`, and the `.proto` of the Connect service.
        let typed: Vec<(&str, String, String)> = vec![
            ("python", format!("python/{a}.py"), format!("class {ty}(enum.Enum):")),
            ("typescript", format!("typescript/{a}.ts"), format!("export const {ty} = {{")),
            ("javascript", format!("javascript/{a}.mjs"), format!("export const {ty} = {{")),
            ("rust", format!("rust/{a}.rs"), format!("pub enum {ty} {{")),
            ("wasm", format!("wasm/{a}.rs"), format!("pub enum {ty} {{")),
            ("ruby", format!("ruby/{a}.rb"), format!("module {ty}")),
            ("php", format!("php/{a}.php"), format!("enum {ty}: string")),
            ("go", format!("go/pick{k}/{a}.go"), format!("type {ty} int")),
            ("swift", format!("swift/{a}.swift"), format!("public enum {ty}: String")),
            ("java", format!("java/Pick{k}.java"), format!("public enum {ty} {{")),
            ("connect", format!("proto/rulec/{a}/v1/{a}.proto"), format!("enum {ty} {{")),
        ];
        for (target, rel, head) in &typed {
            let text = read(rel);
            assert!(text.contains(head.as_str()), "{} {target}: no `{head}` in {rel}", im.path());
            let tokens: std::collections::HashSet<String> =
                text.to_lowercase().split(|c: char| !c.is_ascii_alphanumeric()).map(String::from).collect();
            for d in im.ns.divisions {
                let m = d.member.to_lowercase();
                let prefixed = format!("{}{m}", ty.to_lowercase());
                assert!(tokens.contains(&m) || tokens.contains(&prefixed), "{} {target}: no member {} in {rel}", im.path(), d.member);
                // The `.proto` names the member alone; the wire value is the member's name there.
                if *target != "connect" {
                    assert!(text.contains(im.value(d)), "{} {target}: no value {} in {rel}", im.path(), im.value(d));
                }
            }
        }
        // The two that carry the values and no type: SQL's guard and numpy's plan.
        for (target, rel) in [("sql", format!("sql/{a}.sql")), ("numpy", format!("numpy/{a}.json"))] {
            let text = read(&rel);
            for d in im.ns.divisions {
                assert!(text.contains(im.value(d)), "{} {target}: no value {} in {rel}", im.path(), im.value(d));
            }
        }
        // `api` names the type the callers write.
        let (c, api) = rulec("en", &["api", &p]);
        assert_eq!(c, 0, "{api}");
        assert!(api.contains(&format!("place: {ty}")) || api.contains(&format!("{ty} place")), "{}: {api}", im.path());
        assert_eq!(rulec::backend::ALL.len(), 12, "a new target joins this test");
    }
}

#[test]
fn 別の国の区分を混ぜると取り込んでいる名前空間を挙げて止まる() {
    let t = TempDir::new("regions-mix");
    let base = "rule fee v1\n\nimport std/us/states\n\ninputs\n  state : us_state\n\noutputs\n  fee : number  round down(1)\n\ntable t\npolicy first\n| state | -> fee : number |\n| CELL  | 1 |\n| -     | 0 |\n";
    // A division of a country the rule does not import.
    let p = write(t.path(), "mix.rule", &base.replace("CELL", "Bavaria"));
    let (c, out) = rulec("en", &["check", &p]);
    assert_eq!(c, 1, "{out}");
    assert!(out.contains("error[E012]") && out.contains("`Bavaria` is a division of `std/de/states`. This rule imports `std/us/states`."), "{out}");
    let (_, ja) = rulec("ja", &["check", &p]);
    assert!(ja.contains("`Bavaria` は `std/de/states` の区分です。この規則が取り込んでいるのは `std/us/states` です。"), "{ja}");
    // A local spelling of another country is the same mistake.
    let p = write(t.path(), "mix2.rule", &base.replace("CELL", "北京"));
    let (_, out) = rulec("en", &["check", &p]);
    assert!(out.contains("`北京` is a division of `std/cn/provinces`"), "{out}");
    // Both imported: the other country's value in this column is E103, named by its enum.
    let both = base.replace("import std/us/states\n", "import std/us/states\nimport std/de/states\n").replace("CELL", "Bayern");
    let p = write(t.path(), "both.rule", &both);
    let (_, out) = rulec("en", &["check", &p]);
    assert!(out.contains("error[E103]") && out.contains("`Bavaria` (a value of de_state)"), "{out}");
    // A misspelling: the closest spelling of an imported one.
    let p = write(t.path(), "typo.rule", &base.replace("CELL", "Calfornia"));
    let (_, out) = rulec("en", &["check", &p]);
    assert!(out.contains("Did you mean `California`?"), "{out}");
    // A code two imported countries share: the rule says which one by its name.
    let amb = base.replace("import std/us/states\n", "import std/us/states\nimport std/au/states\n").replace("CELL", "WA");
    let p = write(t.path(), "amb.rule", &amb);
    let (c, out) = rulec("en", &["check", &p]);
    assert_eq!(c, 1, "{out}");
    assert!(out.contains("`WA` reads as a division of two imports") && out.contains("`Washington` (us_state), `Western_Australia` (au_state)"), "{out}");
    // A rule's own enum keeps its values: `CA` here is this enum's, not California.
    let own = base.replace("import std/us/states\n", "import std/us/states\n\nenum region = CA | NY\n").replace("  state : us_state\n", "  state : us_state\n  region : region\n").replace("| state | -> fee : number |\n| CELL  | 1 |\n| -     | 0 |\n", "| state | region | -> fee : number |\n| CA    | CA     | 1 |\n| -     | -      | 0 |\n");
    let p = write(t.path(), "own.rule", &own);
    let (c, out) = rulec("en", &["check", &p]);
    assert_eq!(c, 1, "{out}");
    assert!(out.contains("This column is us_state, but `CA` (a value of region) is written here"), "{out}");
    // One country under both of its names.
    let p = write(t.path(), "twice.rule", "rule x v1\n\nimport std/都道府県\nimport std/jp/prefectures\n");
    let (_, out) = rulec("en", &["check", &p]);
    assert!(out.contains("error[E013]") && out.contains("`std/jp/prefectures` holds the same divisions as `std/都道府県`"), "{out}");
    // A path that names nothing, and the closest one.
    let p = write(t.path(), "path.rule", "rule x v1\n\nimport std/de/state\n");
    let (_, out) = rulec("en", &["check", &p]);
    assert!(out.contains("error[E013]") && out.contains("Did you mean `std/de/states`?"), "{out}");
}

#[test]
fn 都道府県はどちらの名前で取り込んでも同じ区分で綴りだけが違う() {
    let t = TempDir::new("regions-jp");
    let ja = lookup("std/都道府県").unwrap();
    let en = lookup("std/jp/prefectures").unwrap();
    // Each written in the other's spelling.
    let a = write(t.path(), "ja.rule", &rule(&ja, |_, d| d.name.to_string()));
    let b = write(t.path(), "en.rule", &rule(&en, |_, d| d.spellings[0].to_string()));
    let (_, va) = rulec("en", &["vectors", &a]);
    let (_, vb) = rulec("en", &["vectors", &b]);
    // The same cases, the values in each import's own spelling.
    let mut swapped = va.clone();
    for (j, e) in rulec::prelude::PREFECTURES {
        swapped = swapped.replace(&format!("\"{j}\""), &format!("\"{e}\""));
    }
    assert_eq!(swapped, vb);
    // The same generated type and members; the wire value is each import's spelling.
    let out = t.path().join("out");
    for (p, w) in [(&a, "TOKYO = \"東京都\""), (&b, "TOKYO = \"Tokyo\"")] {
        let (c, o) = rulec("en", &["gen", p, "--out", out.to_str().unwrap()]);
        assert_eq!(c, 0, "{o}");
        let py = std::fs::read_to_string(out.join("python/pick.py")).unwrap();
        assert!(py.contains("class Prefecture(enum.Enum):") && py.contains(w), "{py}");
    }
}

#[test]
fn 準用は同じ国を別の名前で取り込んでいても区分で読み替える() {
    let t = TempDir::new("regions-apply");
    // The callee spells the prefectures in English; the rule that applies it, in Japanese.
    let callee = "rule zone_fee v1\n\nimport std/jp/prefectures\n\ngroup remote = Hokkaido, OKINAWA_IS_NOT_A_SPELLING\n\ninputs\n  dest : jp_prefecture\n\noutputs\n  fee : money[JPY]  round down(1JPY)\n\ntable fee_table\npolicy unique\n| dest        | -> fee : money[JPY] |\n| remote      | 1200JPY |\n| not: remote | 800JPY  |\n"
        .replace("OKINAWA_IS_NOT_A_SPELLING", "沖縄県");
    write(t.path(), "zone_fee.rule", &callee);
    let h = rulec::sha256::short(callee.as_bytes());
    let caller = format!(
        "rule order_fee v1\n\nimport std/都道府県\n\ninputs\n  to : 都道府県\n\noutputs\n  order_fee : money[JPY]  round down(1JPY)\n\napply zone = \"zone_fee.rule\" sha256:{h}\n  dest = to\n  fee -> order_fee\n\nexamples\n| to     | -> order_fee |\n| 沖縄県 | 1200JPY      |\n| Tokyo  | 800JPY       |\n"
    );
    let p = write(t.path(), "order_fee.rule", &caller);
    let (c, out) = rulec("en", &["check", &p]);
    assert_eq!(c, 0, "{out}");
    // One enum, in the caller's spelling: the callee's import is not added beside it.
    let (_, schema) = rulec("en", &["schema", &p]);
    assert!(schema.contains("\"東京都\"") && !schema.contains("\"Tokyo\""), "{schema}");
    // The same import on both sides: a callee whose input takes a built-in enum could not be
    // applied at all before §15.182 (its imported enum was not among the callee's enums, so
    // every value came out as E042).
    let same = callee.replace("import std/jp/prefectures", "import std/都道府県").replace("jp_prefecture", "都道府県").replace("Hokkaido", "北海道");
    write(t.path(), "zone_fee2.rule", &same);
    let h2 = rulec::sha256::short(same.as_bytes());
    let p2 = write(t.path(), "order_fee2.rule", &caller.replace("zone_fee.rule", "zone_fee2.rule").replace(&h, &h2));
    let (c, out) = rulec("en", &["check", &p2]);
    assert_eq!(c, 0, "{out}");
}

#[test]
fn 名前の出どころとライセンスを書いている() {
    let notices = std::fs::read_to_string(root().join("THIRD_PARTY_NOTICES")).expect("THIRD_PARTY_NOTICES");
    assert!(notices.contains("Unicode CLDR") && notices.contains("48.2") && notices.contains("UNICODE LICENSE V3"), "{notices}");
    let design = std::fs::read_to_string(root().join("DESIGN.md")).unwrap();
    assert!(design.contains("### 15.182"), "DESIGN.md has no §15.182");
}

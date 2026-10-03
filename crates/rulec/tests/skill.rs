//! The distributable agent skill (`skills/rulec/`).
//!
//! It is for people and agents **using** rulec, not for working on rulec, so it is copied
//! into someone else's project and has to stand up there: every link it carries must land
//! inside the skill directory, and nothing in it may quietly fall behind the documents it
//! was built from. `skills/sync.sh` assembles it; this repeats the same assembly and fails
//! if the committed files differ, which is the only thing that keeps a skill shipped to
//! other people from describing a tool that no longer behaves that way.

use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("読めない: {rel}: {e}"))
}

const BUNDLED: &[&str] =
    &["SKILL.md", "reference.md", "formats.md", "generated-code.md", "backends.md", "examples.md", "compatibility.md"];

/// The same assembly `skills/sync.sh` does, in one place so the two cannot disagree.
fn built_skill_md() -> String {
    let agents = read("AGENTS.md");
    let cut = agents.find("\n## 7. Where to look\n").expect("AGENTS.md に 7 節が無い");
    let body = &agents[..cut + 1];
    let body = body
        .replace("(docs/reference.md)", "(reference.md)")
        .replace("(docs/formats.md", "(formats.md")
        .replace("(docs/generated-code.md)", "(generated-code.md)")
        .replace("(docs/backends.md)", "(backends.md)")
        .replace(" ([docs/codes.md](docs/codes.md))", "")
        .replace("[docs/reference.md]", "[reference.md]")
        .replace("[docs/formats.md]", "[formats.md]")
        .replace("[docs/generated-code.md]", "[generated-code.md]")
        .replace("[docs/backends.md]", "[backends.md]");
    format!("{}{}{}", read("skills/header.md"), body, read("skills/footer.md"))
}

#[test]
fn スキルはいまの文書から組み立てたものと同じ() {
    assert_eq!(
        read("skills/rulec/SKILL.md"),
        built_skill_md(),
        "skills/rulec/SKILL.md が古いか手で編集されています。`skills/sync.sh` で作り直してください"
    );
    for (bundled, source) in [
        ("formats.md", "docs/formats.md"),
        ("generated-code.md", "docs/generated-code.md"),
        ("compatibility.md", "docs/compatibility.md"),
    ] {
        assert_eq!(
            read(&format!("skills/rulec/{bundled}")),
            read(source),
            "skills/rulec/{bundled} が {source} と違います。`skills/sync.sh` で作り直してください"
        );
    }
    // The grammar loses its one link to the ledger, which is not bundled.
    assert_eq!(
        read("skills/rulec/reference.md"),
        read("docs/reference.md").replace("[codes.md](codes.md)", "`rulec explain --all`"),
        "skills/rulec/reference.md が古いです。`skills/sync.sh` で作り直してください"
    );
    // backends.md loses the ledger the same way, and the link to the corpus rule its worked
    // example is built from: inside someone else's project that path leads nowhere.
    assert_eq!(
        read("skills/rulec/backends.md"),
        read("docs/backends.md")
            .replace("[`tests/corpus/ec261.rule`](../tests/corpus/ec261.rule)", "`ec261.rule`")
            .replace("[codes.md](codes.md)", "`rulec explain --all`"),
        "skills/rulec/backends.md が古いです。`skills/sync.sh` で作り直してください"
    );
    // The examples page loses the site's own navigation buttons.
    // Cut where the buttons point, not at what they are labelled: the label has been
    // renamed once, and matching on it made this test fail for the wrong reason.
    let want = read("website/docs/examples.md");
    let want = want
        .lines()
        .take_while(|l| !(l.starts_with('[') && l.contains("](tour.md)")))
        .collect::<Vec<_>>()
        .join("\n");
    let want = want.as_str();
    assert_eq!(
        read("skills/rulec/examples.md").trim_end(),
        want.trim_end().trim_end_matches("---").trim_end(),
        "skills/rulec/examples.md が古いです。`skills/sync.sh` で作り直してください"
    );
}

#[test]
fn スキルのfrontmatterは配布できる形をしている() {
    let s = read("skills/rulec/SKILL.md");
    assert!(s.starts_with("---\n"), "先頭行が `---` でないと frontmatter として読まれません");
    let fm = s[4..].split("\n---\n").next().expect("frontmatter が閉じていない");
    // Only the six fields of the Agent Skills spec, so the same directory can be uploaded
    // to claude.ai or the Skills API without being rejected.
    const SPEC: &[&str] = &["name", "description", "license", "compatibility", "metadata", "allowed-tools"];
    for line in fm.lines().filter(|l| !l.starts_with(' ') && !l.is_empty()) {
        let key = line.split(':').next().unwrap();
        assert!(SPEC.contains(&key), "`{key}` は Agent Skills の仕様外の鍵で、配布時に弾かれます");
    }
    assert!(fm.contains("name: rulec"), "{fm}");
    let desc = fm
        .lines()
        .find(|l| l.starts_with("description:"))
        .expect("description が無いと、いつ使うかを判断できません");
    assert!(desc.len() < 1536, "description が長すぎます（{} 字）", desc.chars().count());
    // The description is what decides whether the skill gets loaded at all, so it has to
    // name the shapes of rule this is for, not just the tool.
    for w in ["tariff", "fee", "discount", ".rule", "E101"] {
        assert!(desc.contains(w), "description が `{w}` に触れていません: {desc}");
    }
}

#[test]
fn スキルの中のリンクはスキルの中で閉じている() {
    for f in BUNDLED {
        let body = read(&format!("skills/rulec/{f}"));
        let mut rest = body.as_str();
        while let Some(i) = rest.find("](") {
            let target = &rest[i + 2..];
            let end = target.find(')').unwrap_or(0);
            let target = &target[..end];
            rest = &rest[i + 2 + end..];
            if target.starts_with("http") || target.starts_with('#') {
                continue;
            }
            let file = target.split('#').next().unwrap();
            assert!(
                BUNDLED.contains(&file),
                "{f}: `{target}` はスキルの外を指しています。利用者のプロジェクトには存在しません"
            );
        }
    }
}

/// SKILL.md is loaded whole whenever the skill fires; the rest is read on demand. The
/// documented ceiling is 500 lines.
#[test]
fn skill_mdは十分に短い() {
    let n = read("skills/rulec/SKILL.md").lines().count();
    assert!(n < 500, "SKILL.md が {n} 行あります。詳しいものは横のファイルへ出してください");
}

/// The skill's description is what decides whether it gets loaded at all, and it names the
/// range of diagnostic codes. A code added outside that range is a code the skill will not
/// be reached for.
#[test]
fn スキルのdescriptionが名指しする範囲に全コードが入る() {
    let s = read("skills/rulec/SKILL.md");
    let desc = s
        .lines()
        .find(|l| l.starts_with("description:"))
        .expect("description が無い");
    // Each `Xnnn-Xmmm` in the description is an inclusive range of codes.
    let mut ranges: Vec<(char, u32, u32)> = Vec::new();
    for part in desc.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-')) {
        let Some((a, b)) = part.split_once('-') else { continue };
        let (Some(fa), Some(fb)) = (a.chars().next(), b.chars().next()) else { continue };
        let (Ok(na), Ok(nb)) = (a[1..].parse::<u32>(), b[1..].parse::<u32>()) else { continue };
        if fa == fb {
            ranges.push((fa, na, nb));
        }
    }
    assert!(!ranges.is_empty(), "description に範囲が書かれていない: {desc}");
    for e in rulec::codes::ledger() {
        let code = e.code;
        let (f, n) = (code.chars().next().unwrap(), code[1..].parse::<u32>().unwrap());
        let named = desc.contains(code) || ranges.iter().any(|(rf, a, b)| *rf == f && (*a..=*b).contains(&n));
        assert!(named, "{code} が skill の description のどの範囲にも入っていない: {desc}");
    }
}

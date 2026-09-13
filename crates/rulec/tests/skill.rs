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

const BUNDLED: &[&str] = &["SKILL.md", "reference.md", "formats.md", "generated-code.md", "examples.md"];

/// The same assembly `skills/sync.sh` does, in one place so the two cannot disagree.
fn built_skill_md() -> String {
    let agents = read("AGENTS.md");
    let cut = agents.find("\n## 7. Where to look\n").expect("AGENTS.md に 7 節が無い");
    let body = &agents[..cut + 1];
    let body = body
        .replace("(docs/reference.md)", "(reference.md)")
        .replace("(docs/formats.md", "(formats.md")
        .replace("(docs/generated-code.md)", "(generated-code.md)")
        .replace(" ([docs/codes.md](docs/codes.md))", "")
        .replace("[docs/reference.md]", "[reference.md]")
        .replace("[docs/formats.md]", "[formats.md]")
        .replace("[docs/generated-code.md]", "[generated-code.md]");
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
    // The examples page loses the site's own navigation buttons.
    let want = read("website/docs/examples.md");
    let want = want.split("\n[Write a table](tour.md)").next().unwrap();
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

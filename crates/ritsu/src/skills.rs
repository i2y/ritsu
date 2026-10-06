//! `ritsu skills` (PLAN F.3): the nine Agent Skills, carried in the binary, so that an agent that
//! has only `ritsu` gets the guides this repository has. The files are `skills/<name>/` at the root,
//! taken in as they are (`include_str!`), one skill for ritsu and one for each of the eight
//! languages, as geas's `geas skill --install` takes in its own.
//!
//! `ritsu skills list` names them; `ritsu skills install` writes them as `<dir>/<name>/`, where
//! agents that read Agent Skills look: a project's `.claude/skills/` by default, `~/.claude/skills/`
//! with `--user`, or any directory with `--dir`. A file that is there and differs from the one
//! ritsu carries (changed by hand, or written by another version) stops the install before
//! anything is written, unless `--force` is given; a file the same as ritsu's is left as it is, and
//! a file ritsu does not carry is never touched. `tests/skill.rs` holds this list to the files of
//! `skills/`, so a page added there and not here fails.

use crate::cli;
use ritsu_base::text::{Lang, Text, pad, width};
use ritsu_base::tr;
use std::path::{Path, PathBuf};

/// One skill: its name, which is the name of its folder, and its files, `SKILL.md` first.
pub struct Skill {
    pub name: &'static str,
    pub files: &'static [(&'static str, &'static str)],
}

/// The nine skills, ritsu's first and then the languages in the order `ritsu --help` lists them.
pub const SKILLS: &[Skill] = &[
    Skill {
        name: "ritsu",
        files: &[
            ("SKILL.md", include_str!("../../../skills/ritsu/SKILL.md")),
        ],
    },
    Skill {
        name: "rulec",
        files: &[
            ("SKILL.md", include_str!("../../../skills/rulec/SKILL.md")),
            ("backends.md", include_str!("../../../skills/rulec/backends.md")),
            ("compatibility.md", include_str!("../../../skills/rulec/compatibility.md")),
            ("examples.md", include_str!("../../../skills/rulec/examples.md")),
            ("formats.md", include_str!("../../../skills/rulec/formats.md")),
            ("generated-code.md", include_str!("../../../skills/rulec/generated-code.md")),
            ("reference.md", include_str!("../../../skills/rulec/reference.md")),
        ],
    },
    Skill {
        name: "dandori",
        files: &[
            ("SKILL.md", include_str!("../../../skills/dandori/SKILL.md")),
            ("agents.md", include_str!("../../../skills/dandori/agents.md")),
            ("checks.md", include_str!("../../../skills/dandori/checks.md")),
            ("codes.md", include_str!("../../../skills/dandori/codes.md")),
            ("commands.md", include_str!("../../../skills/dandori/commands.md")),
            ("dates-and-books.md", include_str!("../../../skills/dandori/dates-and-books.md")),
            ("design.md", include_str!("../../../skills/dandori/design.md")),
            ("diagrams.md", include_str!("../../../skills/dandori/diagrams.md")),
            ("examples.md", include_str!("../../../skills/dandori/examples.md")),
            ("jev.md", include_str!("../../../skills/dandori/jev.md")),
            ("platforms.md", include_str!("../../../skills/dandori/platforms.md")),
            ("secrets.md", include_str!("../../../skills/dandori/secrets.md")),
            ("services.md", include_str!("../../../skills/dandori/services.md")),
            ("tasks.md", include_str!("../../../skills/dandori/tasks.md")),
            ("tour.md", include_str!("../../../skills/dandori/tour.md")),
        ],
    },
    Skill {
        name: "koyomi",
        files: &[
            ("SKILL.md", include_str!("../../../skills/koyomi/SKILL.md")),
            ("codes.md", include_str!("../../../skills/koyomi/codes.md")),
            ("generated-code.md", include_str!("../../../skills/koyomi/generated-code.md")),
            ("reference.md", include_str!("../../../skills/koyomi/reference.md")),
        ],
    },
    Skill {
        name: "chobo",
        files: &[
            ("SKILL.md", include_str!("../../../skills/chobo/SKILL.md")),
            ("codes.md", include_str!("../../../skills/chobo/codes.md")),
            ("formats.md", include_str!("../../../skills/chobo/formats.md")),
            ("reference.md", include_str!("../../../skills/chobo/reference.md")),
            ("targets.md", include_str!("../../../skills/chobo/targets.md")),
        ],
    },
    Skill {
        name: "geas",
        files: &[
            ("SKILL.md", include_str!("../../../skills/geas/SKILL.md")),
            ("codes.md", include_str!("../../../skills/geas/codes.md")),
            ("commands.md", include_str!("../../../skills/geas/commands.md")),
            ("examples.md", include_str!("../../../skills/geas/examples.md")),
            ("gui.md", include_str!("../../../skills/geas/gui.md")),
            ("language.md", include_str!("../../../skills/geas/language.md")),
            ("map.md", include_str!("../../../skills/geas/map.md")),
        ],
    },
    Skill {
        name: "yuen",
        files: &[
            ("SKILL.md", include_str!("../../../skills/yuen/SKILL.md")),
            ("codes.md", include_str!("../../../skills/yuen/codes.md")),
            ("reference.md", include_str!("../../../skills/yuen/reference.md")),
        ],
    },
    Skill {
        name: "sakai",
        files: &[
            ("SKILL.md", include_str!("../../../skills/sakai/SKILL.md")),
            ("codes.md", include_str!("../../../skills/sakai/codes.md")),
            ("reference.md", include_str!("../../../skills/sakai/reference.md")),
            ("targets.md", include_str!("../../../skills/sakai/targets.md")),
        ],
    },
    Skill {
        name: "sekisho",
        files: &[
            ("SKILL.md", include_str!("../../../skills/sekisho/SKILL.md")),
            ("codes.md", include_str!("../../../skills/sekisho/codes.md")),
            ("reference.md", include_str!("../../../skills/sekisho/reference.md")),
        ],
    },
];

/// What `ritsu skills list` says of a skill, in a line.
pub fn purpose(name: &str) -> Text {
    match name {
        "ritsu" => tr!(
            "二つ以上の言語のファイルを持つプロジェクト（`ritsu check`、`ritsu run`、`ritsu gen`、言語をまたぐ診断）",
            "a project with files of more than one language (`ritsu check`, `ritsu run`, `ritsu gen`, the diagnostics across the languages)"
        ),
        "rulec" => tr!("業務の規則（.rule）", "business rules (.rule)"),
        "dandori" => tr!("ワークフロー（.flow）", "workflows (.flow)"),
        "koyomi" => tr!("締め日、支払日、営業日（.cal）", "closing days, payment days and business days (.cal)"),
        "chobo" => tr!("在庫、お金、ポイント、予約の枠の帳簿（.book）", "books of stock, money, points and booking slots (.book)"),
        "geas" => tr!("人が読んだ主張に、コードを従わせる（.geas）", "claims a person has read, held over the code (.geas)"),
        "yuen" => tr!("要件の来歴（.req）", "where requirements come from (.req)"),
        "sakai" => tr!("境界づけられたコンテキストの地図（.ctx）", "maps of bounded contexts (.ctx)"),
        _ => tr!("だれが何をしてよいかを書き、Cedar を生成する（.gate）", "who may do what, compiled to Cedar (.gate)"),
    }
}

/// The skill named `name`.
pub fn find(name: &str) -> Option<&'static Skill> {
    SKILLS.iter().find(|s| s.name == name)
}

/// What an install found in the directory, before it writes anything.
#[derive(Default)]
pub struct Plan {
    /// For each skill, its folder, the files to write there, and how many are already the same.
    pub folders: Vec<(PathBuf, Vec<(PathBuf, &'static str)>, usize)>,
    /// The files that are there and differ from the ones ritsu carries.
    pub differ: Vec<PathBuf>,
}

/// Looks at `<dir>/<name>/` for each skill: which files are missing, which are the same as the ones
/// ritsu carries, and which differ.
pub fn plan(dir: &Path, skills: &[&'static Skill]) -> Plan {
    let mut plan = Plan::default();
    for s in skills {
        let folder = dir.join(s.name);
        let mut writes = Vec::new();
        let mut same = 0;
        for (name, text) in s.files {
            let p = folder.join(name);
            match std::fs::read(&p) {
                Ok(there) if there == text.as_bytes() => same += 1,
                Ok(_) => {
                    plan.differ.push(p.clone());
                    writes.push((p, *text));
                }
                Err(_) => writes.push((p, *text)),
            }
        }
        plan.folders.push((folder, writes, same));
    }
    plan
}

fn refuse(msg: Text, lang: Lang) -> u8 {
    let head = if lang == Lang::Ja { "エラー" } else { "error" };
    eprintln!("{head}: {}", msg.get(lang));
    2
}

/// `ritsu skills list | install [<name>...] [--dir <dir> | --user] [--force]`: the exit code.
pub fn command(args: &[String], lang: Lang) -> u8 {
    let table = cli::table();
    let cmd = table.command("skills").expect("skills is in the table");
    let asked = args.iter().enumerate().find_map(|(i, x)| if x == "--lang" { args.get(i + 1).cloned() } else { x.strip_prefix("--lang=").map(str::to_string) });
    let lang = if asked.is_some() { Lang::pick(asked.as_deref(), "RITSU_LANG") } else { lang };
    let a = match table.parse(cmd, args) {
        Ok(a) => a,
        Err(e) => return refuse(e, lang),
    };
    if a.has("--help") {
        print!("{}", table.help_cmd(cmd, lang));
        return 0;
    }
    let usage = tr!(
        "使い方: ritsu skills list | ritsu skills install [<name>...] [--dir <dir> | --user] [--force]",
        "usage: ritsu skills list | ritsu skills install [<name>...] [--dir <dir> | --user] [--force]"
    );
    match a.pos.first().map(String::as_str) {
        Some("list") => {
            if a.pos.len() > 1 || a.has("--dir") || a.has("--user") || a.has("--force") {
                return refuse(tr!("`ritsu skills list` は引数もフラグも取りません", "`ritsu skills list` takes no arguments and no flags"), lang);
            }
            let w = SKILLS.iter().map(|s| width(s.name)).max().unwrap_or(0);
            for s in SKILLS {
                println!("{}  {}", pad(s.name, w), purpose(s.name).get(lang));
            }
            0
        }
        Some("install") => install(&a.pos[1..], a.get("--dir"), a.has("--user"), a.has("--force"), lang),
        Some(other) => {
            let other = other.to_string();
            refuse(tr!("`ritsu skills {other}` というコマンドはありません。{}", "there is no `ritsu skills {other}`; {}", usage.get(lang)), lang)
        }
        None => refuse(usage, lang),
    }
}

/// `1 file`, `7 files`.
fn files(n: usize) -> String {
    if n == 1 { "1 file".to_string() } else { format!("{n} files") }
}

fn install(names: &[String], dir: Option<&str>, user: bool, force: bool, lang: Lang) -> u8 {
    let mut skills: Vec<&'static Skill> = Vec::new();
    for n in names {
        match find(n) {
            Some(s) if !skills.iter().any(|t| t.name == s.name) => skills.push(s),
            Some(_) => {}
            None => {
                let all = SKILLS.iter().map(|s| s.name).collect::<Vec<_>>().join(", ");
                return refuse(tr!("`{n}` というスキルはありません。スキルは {all} です", "there is no skill `{n}`; the skills are {all}"), lang);
            }
        }
    }
    if skills.is_empty() {
        skills = SKILLS.iter().collect();
    }
    let dir: PathBuf = match (dir, user) {
        (Some(_), true) => return refuse(tr!("`--dir` と `--user` は、どちらか一つです", "give `--dir` or `--user`, not both"), lang),
        (Some(d), false) => PathBuf::from(d),
        (None, true) => match std::env::var_os("HOME").filter(|h| !h.is_empty()) {
            Some(home) => Path::new(&home).join(".claude/skills"),
            None => return refuse(tr!("`--user` が書く ~/.claude/skills を決める HOME がありません", "`--user` writes to ~/.claude/skills, and HOME is not set"), lang),
        },
        (None, false) => PathBuf::from(".claude/skills"),
    };
    let plan = plan(&dir, &skills);
    if !plan.differ.is_empty() && !force {
        let n = plan.differ.len();
        let head = if lang == Lang::Ja { "エラー" } else { "error" };
        let (some, them) = if n == 1 { ("1 file there differs from the one".to_string(), "it") } else { (format!("{n} files there differ from the ones"), "them") };
        let msg = tr!(
            "{n} 個のファイルが、この ritsu の持つものと違います（手で変えたか、別の版の ritsu が書いたもの）。何も書いていません。`--force` を付けると上書きします:",
            "{some} this ritsu carries (changed by hand, or written by another version of ritsu), so nothing was written; `--force` writes over {them}:"
        );
        eprintln!("{head}: {}", msg.get(lang));
        for p in &plan.differ {
            eprintln!("  {}", p.display());
        }
        return 1;
    }
    for (folder, writes, same) in &plan.folders {
        let shown = folder.display().to_string();
        if writes.is_empty() {
            println!("{}", tr!("{shown} はすでに同じです（ファイル {} 個）", "{shown} is already the same ({})", same ; files(*same)).get(lang));
            continue;
        }
        if let Err(e) = std::fs::create_dir_all(folder) {
            return refuse(tr!("{shown} を作れません: {e}", "cannot make {shown}: {e}"), lang);
        }
        for (p, text) in writes {
            if let Err(e) = std::fs::write(p, text) {
                let p = p.display();
                return refuse(tr!("{p} を書けません: {e}", "cannot write {p}: {e}"), lang);
            }
        }
        let n = writes.len();
        println!("{}", tr!("{shown} に書きました（ファイル {} 個）", "wrote {shown} ({})", n ; files(n)).get(lang));
    }
    0
}

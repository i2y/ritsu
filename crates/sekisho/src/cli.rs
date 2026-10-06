//! The commands and their flags, in one table (ritsu's DESIGN 4.4). `--help` is drawn from it and
//! the command line is read against it, so a flag cannot be documented and ignored, or taken and
//! left undocumented; an unknown flag, a value outside a closed set, a flag without its value and a
//! flag given twice stop the run with exit 2. The table is sekisho's; how it is drawn and read is
//! ritsu-base's ([`ritsu_base::cli`]).
//!
//! `check` and `explain` came with stage A, and `gen` (Cedar), `vectors` and `api` with stage B;
//! `doc` joins the table when it is written (DESIGN 11).

use crate::check::{self, Options};
use crate::suite::Suite;
use ritsu_base::cli::{Args, Cmd, Flag, Reading, Table, flag, help_flag, lang_flag};
use ritsu_base::text::{Lang, Text};
use std::io::Write;
use std::path::PathBuf;

fn every_code() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = crate::codes::ledger().entries.iter().map(|e| e.code).collect();
    v.sort_by_key(|c| (c.starts_with('W'), *c));
    v
}

/// `--root`, as yuen and sakai take it (DESIGN 2.6): where the paths of the references an action's
/// guards are written from.
fn root_flag() -> Flag {
    flag(
        "--root",
        Some("<dir>"),
        tr!(
            "参照のパスを数えるルート。無ければ、最初に渡したパスの上で .git を持つ一番近いディレクトリ（それも無ければ、そのファイルのあるディレクトリ）",
            "the root the paths of references count from; else the nearest directory above the first path given that holds .git (else the directory of that file)"
        ),
    )
}

pub fn table() -> Table {
    Table {
        tool: "sekisho",
        version: env!("CARGO_PKG_VERSION"),
        summary: tr!(
            "認可を書く小さな言語。だれが何に何をしてよいかを、役割・属性・関係と、rulec の規則と koyomi の日付を条件にして書き、宣言した範囲のすべての組み合わせで確かめて、Cedar を生成する",
            "A small language for who may do what: roles, attributes and relations, with rulec's rules and koyomi's dates as conditions, checked on every combination it declares and compiled to Cedar"
        ),
        globals: vec![lang_flag("SEKISHO_LANG"), help_flag()],
        commands: vec![
            Cmd {
                usage: None,
                name: "check",
                args: "<file.gate>...",
                purpose: tr!("検査する。構文、名前と型、宣言した範囲のすべての組み合わせ", "check the words, the names and the types, and every combination the file declares"),
                params: vec![("<file.gate>...", tr!("検査する .gate のファイル", "the .gate files to check"))],
                flags: vec![
                    flag("--format", Some("json"), tr!("ファイルごとに一つの JSON", "one JSON object a file")).choices(&["json"]),
                    flag("--budget", Some("<n>"), tr!("一つの action で数える組み合わせの上限", "the most combinations the check walks for one action")).default("100000000"),
                    root_flag(),
                ],
                exits: vec![
                    (0, tr!("エラーなし（警告はありうる）", "no errors (there may be warnings)")),
                    (1, tr!("エラーが一つ以上", "at least one error")),
                    (2, tr!("引数の誤り、読めないファイル、規則・日付・カレンダー・帳簿・フローを読むファイルを、それらを読めないこの sekisho で走らせた（E209。ritsu sekisho で走らせる）", "bad arguments, a file that cannot be read, or a file that reads a rule, a dates file, a calendar, a book or a flow, run with this sekisho, which reads none of them (E209: run it as ritsu sekisho)")),
                ],
                examples: vec!["sekisho check shop.gate", "ritsu sekisho check examples/refunds/refunds.gate --format json --lang ja"],
                codes: every_code(),
            },
            Cmd {
                usage: Some("sekisho gen <file.gate>... --target cedar [--out <dir>] [--check] [--root <dir>] [--lang ja|en]"),
                name: "gen",
                args: "<file.gate>...",
                purpose: tr!(
                    "Cedar のスキーマとポリシーを生成する。検査を通らないファイルからは生成しない",
                    "generate the Cedar schema and policies; nothing is generated from a file that does not pass check"
                ),
                params: vec![("<file.gate>...", tr!(".gate のファイル", "the .gate files"))],
                flags: vec![
                    flag(
                        "--target",
                        Some("cedar"),
                        tr!(
                            "生成するもの。cedar は <out>/cedar/ に、別名ごとに四つのファイル（.cedar、.cedarschema、.cedarschema.json、.policies.json）を書く。sekisho が書く文（頭と、計算した値の @doc）は --lang の言語になる",
                            "what to generate: cedar writes four files a gate into <out>/cedar/, named by its alias (.cedar, .cedarschema, .cedarschema.json, .policies.json); what sekisho writes in them (the head, the @doc of a computed value) is in the language of --lang"
                        ),
                    )
                    .choices(&crate::r#gen::TARGETS),
                    flag("--out", Some("<dir>"), tr!("生成物を書くディレクトリ", "the directory the generated files go to")).default("generated"),
                    flag("--check", None, tr!("書き出さずに、ディスクの生成物が、いま生成するものと一字一句同じかを見る。違えば 1（CI 用）", "write nothing; exit 1 if a file on disk differs from what gen would write (for CI)")),
                    root_flag(),
                ],
                exits: vec![
                    (0, tr!("生成した、または --check で全部が同じだった", "generated, or --check found every file the same")),
                    (1, tr!("検査を通らないファイルがある、または --check で違う・無いファイルがある", "a file does not pass check, or --check found a file that differs or is missing")),
                    (2, tr!("引数の誤り、読めないファイル、書けないファイル、ほかの言語を読むファイルを、それを読めないこの sekisho で走らせた（E209）", "bad arguments, a file that cannot be read or written, or a file that reads another language run with this sekisho, which reads none (E209)")),
                ],
                examples: vec!["ritsu sekisho gen examples/refunds/refunds.gate --target cedar", "ritsu sekisho gen examples/refunds/refunds.gate --target cedar --check", "sekisho gen shop.gate --target cedar --out generated --lang ja"],
                codes: vec![],
            },
            Cmd {
                usage: None,
                name: "vectors",
                args: "<file.gate>",
                purpose: tr!(
                    "全部の組み合わせを、cedar run-tests が読むテストにして出す（リクエスト、エンティティ、sekisho の参照の評価が言う答えと決めたポリシー）",
                    "print every combination as a test of cedar run-tests: the request, the entities, and the decision and the deciding policies by sekisho's reference evaluation"
                ),
                params: vec![("<file.gate>", tr!("検査を通る .gate のファイル", "a .gate file that passes check"))],
                flags: vec![flag("--action", Some("<action>"), tr!("この action の組み合わせだけ（名前か別名）", "the combinations of this action only (its name or its alias)")), root_flag()],
                exits: vec![
                    (0, tr!("出した", "printed")),
                    (1, tr!("ファイルが検査を通らない", "the file does not pass check")),
                    (2, tr!("引数の誤り、読めないファイル、無い action、ほかの言語を読むファイルを、それを読めないこの sekisho で走らせた（E209）", "bad arguments, a file that cannot be read, no such action, or a file that reads another language run with this sekisho, which reads none (E209)")),
                ],
                examples: vec![
                    "ritsu sekisho vectors examples/refunds/refunds.gate > refunds.tests.json",
                    "cedar run-tests --policies generated/cedar/refunds.cedar --schema generated/cedar/refunds.cedarschema --tests refunds.tests.json",
                    "ritsu sekisho vectors examples/refunds/refunds.gate --action refund_order",
                ],
                codes: vec![],
            },
            Cmd {
                usage: None,
                name: "api",
                args: "<file.gate>",
                purpose: tr!(
                    "ほかのツールのための JSON を出す。名前空間と生成するファイル、役割・型・ワークフローと Cedar でのエンティティ、action と守る操作と context、ポリシーの @id、期待と職務の分離",
                    "print JSON for other tools: the namespace and the files gen writes, the roles, types and workflows with their entities in Cedar, the actions with the operations they guard and their context, the policies with their @id, the expectations and the separations"
                ),
                params: vec![("<file.gate>", tr!("検査を通る .gate のファイル", "a .gate file that passes check"))],
                flags: vec![root_flag()],
                exits: vec![
                    (0, tr!("出した", "printed")),
                    (1, tr!("ファイルが検査を通らない", "the file does not pass check")),
                    (2, tr!("引数の誤り、読めないファイル、ほかの言語を読むファイルを、それを読めないこの sekisho で走らせた（E209）", "bad arguments, a file that cannot be read, or a file that reads another language run with this sekisho, which reads none (E209)")),
                ],
                examples: vec!["ritsu sekisho api examples/refunds/refunds.gate --root examples/refunds"],
                codes: vec![],
            },
            Cmd {
                usage: Some("sekisho explain <code> | --all [--format markdown|json] [--lang ja|en]"),
                name: "explain",
                args: "<code>",
                purpose: tr!("診断のコードを説明する。いつ出るか、直し方、それを出す一番小さな例", "explain a diagnostic code: when it is printed, how to fix it, and the smallest file that prints it"),
                params: vec![("<code>", tr!("`E101` のようなコード", "a code such as `E101`"))],
                flags: vec![
                    flag("--all", None, tr!("全部のコード", "every code")),
                    flag("--format", Some("markdown|json"), tr!("Markdown（docs/codes.md の形）か JSON", "Markdown (as docs/codes.md is) or JSON")).choices(&["markdown", "json"]),
                ],
                exits: vec![(0, tr!("説明した", "explained")), (2, tr!("知らないコード、引数の誤り", "an unknown code, or bad arguments"))],
                examples: vec!["sekisho explain E101", "sekisho explain --all --format markdown --lang ja"],
                codes: vec![],
            },
        ],
        footer: vec![
            tr!("どのコマンドも `--lang ja|en` と `--help` を取ります。", "Every command takes `--lang ja|en` and `--help`."),
            tr!(
                "exit code: 0 エラーなし / 1 エラーあり / 2 引数の誤りか読めないファイル、またはほかの言語を読むファイルを、それを読めないこの sekisho で走らせた（E209）",
                "Exit codes: 0 no errors / 1 errors found / 2 bad arguments or an unreadable file, or a file that reads another language run with this sekisho, which reads none (E209)"
            ),
        ],
        reading: Reading::default(),
    }
}

/// Say what is wrong with the command line, or with a file it cannot read: one line on `w`, exit 2.
pub(crate) fn refuse(w: &mut dyn Write, msg: Text, lang: Lang) -> u8 {
    let head = if lang == Lang::Ja { "エラー" } else { "error" };
    let _ = writeln!(w, "{head}: {}", msg.get(lang));
    2
}

/// `--lang ja`, `--lang=ja`, anywhere on the line: decided before anything is printed.
fn lang_word(args: &[String]) -> Option<String> {
    for (i, a) in args.iter().enumerate() {
        if a == "--lang" {
            return args.get(i + 1).cloned();
        }
        if let Some(v) = a.strip_prefix("--lang=") {
            return Some(v.to_string());
        }
    }
    None
}

/// The command line, read against the table and run: the exit code.
pub fn run(args: &[String], suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let lang = Lang::pick(lang_word(args).as_deref(), "SEKISHO_LANG");
    let table = table();
    let Some(first) = args.first() else {
        let _ = write!(err, "{}", table.help_all(lang));
        return 2;
    };
    match first.as_str() {
        "--help" | "-h" => {
            let _ = write!(out, "{}", table.help_all(lang));
            return 0;
        }
        "--version" | "-V" => {
            let _ = writeln!(out, "sekisho {}", env!("CARGO_PKG_VERSION"));
            return 0;
        }
        "help" => {
            let rest: Vec<&String> = args[1..].iter().filter(|a| !a.starts_with("--lang")).collect();
            let rest: Vec<&String> = rest.into_iter().filter(|a| lang_word(args).as_deref() != Some(a.as_str())).collect();
            return match rest.first() {
                None => {
                    let _ = write!(out, "{}", table.help_all(lang));
                    0
                }
                Some(n) => match table.command(n) {
                    Some(c) => {
                        let _ = write!(out, "{}", table.help_cmd(c, lang));
                        0
                    }
                    None => refuse(err, tr!("`{n}` というコマンドはありません。`sekisho --help` を読んでください", "there is no command `{n}`; run `sekisho --help`"), lang),
                },
            };
        }
        _ => {}
    }
    let Some(cmd) = table.command(first) else {
        return refuse(err, tr!("`{first}` というコマンドはありません。`sekisho --help` を読んでください", "there is no command `{first}`; run `sekisho --help`"), lang);
    };
    let a = match table.parse(cmd, &args[1..]) {
        Ok(a) => a,
        Err(e) => return refuse(err, e, lang),
    };
    if a.has("--help") {
        let _ = write!(out, "{}", table.help_cmd(cmd, lang));
        return 0;
    }
    match cmd.name {
        "check" => check_cmd(&a, lang, suite, out, err),
        "gen" => crate::r#gen::run(&a, lang, suite, out, err),
        "vectors" => vectors_cmd(&a, lang, suite, out, err),
        "api" => api_cmd(&a, lang, suite, out, err),
        "explain" => explain_cmd(&a, lang, out, err),
        _ => unreachable!("every command in the table is dispatched"),
    }
}

fn check_cmd(a: &Args, lang: Lang, suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    if a.pos.is_empty() {
        return refuse(err, tr!("`sekisho check` には .gate のファイルが要ります", "`sekisho check` needs .gate files"), lang);
    }
    let budget = match a.get("--budget") {
        None => check::DEFAULT_BUDGET,
        Some(b) => match b.replace([',', '_'], "").parse::<u64>() {
            Ok(n) if n > 0 => n,
            _ => return refuse(err, tr!("`--budget {b}` は正の整数ではありません", "`--budget {b}` is not a positive integer"), lang),
        },
    };
    let root = match root(a, &a.pos[0], lang, err) {
        Ok(r) => r,
        Err(code) => return code,
    };
    let opts = Options { budget, root: Some(root) };
    let json = a.get("--format") == Some("json");
    let mut worst = 0u8;
    for f in &a.pos {
        match check::check_file(f, suite, &opts) {
            Ok(o) => {
                worst = worst.max(o.exit());
                if json {
                    let _ = writeln!(out, "{}", check::to_json(&o, lang).pretty());
                } else {
                    let _ = write!(out, "{}", check::render(&o, lang));
                }
            }
            Err(e) => {
                worst = 2;
                refuse(err, tr!("`{f}` を読めません: {e}", "cannot read `{f}`: {e}"), lang);
            }
        }
    }
    worst
}

/// The root of a command (DESIGN 2.6): `--root`, which has to be a directory (else exit 2, said on
/// `err`), or found from `first`, the first file given.
pub(crate) fn root(a: &Args, first: &str, lang: Lang, err: &mut dyn Write) -> Result<PathBuf, u8> {
    match a.get("--root") {
        Some(r) if !ritsu_base::fs::is_dir(std::path::Path::new(r)) => Err(refuse(err, tr!("`--root {r}` はディレクトリではありません", "`--root {r}` is not a directory"), lang)),
        given => Ok(check::root_of(given.map(std::path::Path::new), first)),
    }
}

/// A file a command reads whole (`vectors`, `api`): checked with the languages `suite` joins, its
/// references written from the root ([`root`]). Err is the exit code, the reason said: a root that
/// is no directory, or a file that cannot be read (2), one that reads a language the run does not
/// join (E209, 2), one that does not pass (1).
pub(crate) fn passing(a: &Args, f: &str, what: Text, suite: &Suite, lang: Lang, out: &mut dyn Write, err: &mut dyn Write) -> Result<check::Outcome, u8> {
    let opts = Options { root: Some(root(a, f, lang, err)?), ..Options::default() };
    let o = match check::check_file(f, suite, &opts) {
        Ok(o) => o,
        Err(e) => return Err(refuse(err, tr!("`{f}` を読めません: {e}", "cannot read `{f}`: {e}"), lang)),
    };
    if o.unjoined() {
        let _ = write!(out, "{}", check::render(&o, lang));
        return Err(2);
    }
    if o.has_errors() || o.walked.is_none() || o.scope.is_none() {
        let _ = write!(out, "{}", check::render(&o, lang));
        let head = if lang == Lang::Ja { "エラー" } else { "error" };
        let _ = writeln!(err, "{head}: {}", what.get(lang));
        return Err(1);
    }
    Ok(o)
}

fn vectors_cmd(a: &Args, lang: Lang, suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let [f] = a.pos.as_slice() else {
        return refuse(err, tr!("`sekisho vectors` には .gate のファイルを一つ渡してください", "`sekisho vectors` takes one .gate file"), lang);
    };
    let o = match passing(a, f, tr!("`{f}` は検査を通らないので、ベクタを出しません", "`{f}` does not pass check, so it has no vectors"), suite, lang, out, err) {
        Ok(o) => o,
        Err(code) => return code,
    };
    let (Some(scope), Some(checked)) = (o.scope.as_ref(), o.walked.as_ref()) else { return 1 };
    let g = &checked.gate;
    let only = match a.get("--action") {
        None => None,
        Some(w) => match g.actions.iter().position(|x| x.named.is(w)) {
            Some(i) => Some(i),
            None => {
                let names: Vec<String> = g.actions.iter().map(|x| format!("`{}`", x.named.name)).collect();
                let l = names.join(", ");
                return refuse(err, tr!("`{f}` に action `{w}` はありません（{l}）", "`{f}` has no action `{w}` ({l})"), lang);
            }
        },
    };
    let shape = crate::cedar::shape(g, scope, checked);
    let tests = crate::vectors::tests(g, &shape, &checked.report, only);
    let _ = write!(out, "{}", crate::vectors::text(&tests));
    0
}

fn api_cmd(a: &Args, lang: Lang, suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let [f] = a.pos.as_slice() else {
        return refuse(err, tr!("`sekisho api` には .gate のファイルを一つ渡してください", "`sekisho api` takes one .gate file"), lang);
    };
    let o = match passing(a, f, tr!("`{f}` は検査を通らないので、api を出しません", "`{f}` does not pass check, so it has no api"), suite, lang, out, err) {
        Ok(o) => o,
        Err(code) => return code,
    };
    let (Some(scope), Some(checked)) = (o.scope.as_ref(), o.walked.as_ref()) else { return 1 };
    let _ = writeln!(out, "{}", crate::api::json(scope, checked).pretty());
    0
}

fn explain_cmd(a: &Args, lang: Lang, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let format = a.get("--format");
    let ledger = crate::codes::ledger();
    if a.has("--all") {
        match format {
            Some("markdown") => {
                let _ = write!(out, "{}", ledger.render_markdown(lang));
            }
            Some("json") => {
                let _ = writeln!(out, "{}", ritsu_base::json::Json::arr(ledger.entries.iter().map(|e| ledger.to_json(e, lang))).pretty());
            }
            _ => {
                for (i, e) in ledger.entries.iter().enumerate() {
                    if i > 0 {
                        let _ = writeln!(out);
                    }
                    let _ = write!(out, "{}", ledger.render_text(e, lang));
                }
            }
        }
        return 0;
    }
    let Some(code) = a.pos.first() else {
        return refuse(err, tr!("`sekisho explain` には `E101` のようなコードか `--all` が要ります", "`sekisho explain` needs a code such as `E101`, or `--all`"), lang);
    };
    match ledger.find(code) {
        Some(e) => {
            match format {
                Some("markdown") => {
                    let _ = write!(out, "{}", ledger.render_markdown_one(e, lang));
                }
                Some("json") => {
                    let _ = writeln!(out, "{}", ledger.to_json(e, lang).pretty());
                }
                _ => {
                    let _ = write!(out, "{}", ledger.render_text(e, lang));
                }
            }
            0
        }
        None => refuse(err, tr!("`{code}` という診断のコードはありません。`sekisho explain --all` で全部を見られます", "there is no diagnostic code `{code}`; `sekisho explain --all` lists them all"), lang),
    }
}

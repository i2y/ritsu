//! The table of `ritsu mod` (DESIGN 17.19): every command, with its arguments, its flags and its
//! exit codes, and the one place its command line is read. Each command is one function of one
//! file ([`run`]); the commands of the stages after this one are in the table already, so that a
//! stage fills its function and touches neither the table nor how a command line is read.

use super::{Ctx, diff, edit, fetch, list, lock, push, registry, report, site, vendor};
use ritsu_base::cli::{Args, Cmd, Flag, Reading, Table, flag, help_flag, lang_flag};
use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;

/// `--format` with the two forms most commands print.
fn format() -> Flag {
    flag("--format", Some("text|json"), tr!("出力の形", "the form of the output")).choices(&["text", "json"]).default("text")
}

/// `--format` of `diff` and `report`, which print Markdown too.
fn format_markdown() -> Flag {
    flag("--format", Some("text|markdown|json"), tr!("出力の形（markdown は PR のコメントに貼る形）", "the form of the output (markdown is the one to paste in a pull request's comment)"))
        .choices(&["text", "markdown", "json"])
        .default("text")
}

fn budget() -> Flag {
    flag(
        "--budget",
        Some("<n>"),
        tr!(
            "振る舞いの差を求めるときに調べる量の上限（rulec の検査が訪れるノード数、koyomi の日付のファイルの入力の組み合わせの数、sekisho の組み合わせの数）。書かなければ言語ごとの既定",
            "the cap on how much is walked to find a difference of behavior (the nodes rulec's check visits, the input combinations of koyomi's dates files, the combinations of sekisho); without it, each language's default"
        ),
    )
}

fn cmd(name: &'static str, args: &'static str, purpose: Text, params: Vec<(&'static str, Text)>, flags: Vec<Flag>, exits: Vec<(u8, Text)>, examples: Vec<&'static str>) -> Cmd {
    Cmd { usage: None, name, args, purpose, params, flags, exits, examples, codes: vec![] }
}

/// The commands, in the order of DESIGN 17.19.
pub fn commands() -> Vec<Cmd> {
    vec![
        cmd(
            "init",
            "<module>",
            tr!(
                "新しいモジュールの `ritsu.mod` を書く（名前、版、読める ritsu の下限、`exclude \"generated\"`）",
                "write the `ritsu.mod` of a new module: its name, its version, the least ritsu that reads it, and `exclude \"generated\"`"
            ),
            vec![("<module>", tr!("モジュールの名前（`example.com/acme/pricing` か、配らないモジュールの短い名前）", "the module's name (`example.com/acme/pricing`, or a short name for a module that is not published)"))],
            vec![flag("--version", Some("<version>"), tr!("モジュールの版", "the module's version")).default("0.1.0")],
            vec![(0, tr!("`ritsu.mod` を書いた", "`ritsu.mod` was written")), (2, tr!("`ritsu.mod` がもうある、名前か版の形の誤り、または引数の誤り", "a `ritsu.mod` is there already, a name or a version not of its form, or bad arguments"))],
            vec!["ritsu mod init example.com/acme/pricing", "ritsu mod init shop --version 1.0.0"],
        ),
        cmd(
            "add",
            "<name> <source>",
            tr!(
                "依存を足す。`ritsu.mod` の最後の `require` の行のあとに一行書き、木に lock が要れば lock も書き直す",
                "require a module: a line after the last `require` of `ritsu.mod`, and the lock written again when the tree needs one"
            ),
            vec![
                ("<name>", tr!("依存の名前。ファイルは `@<name>/` で依存のファイルを指す", "the dependency's name: the files name its files by `@<name>/`")),
                (
                    "<source>",
                    tr!(
                        "取り先。`<リポジトリ>`、`<リポジトリ>@<版>`、`<リポジトリ>@latest`（版を書かなければ @latest）か、`./…`・`../…` で始まるディレクトリ（走らせたディレクトリから）",
                        "where it comes from: `<repository>`, `<repository>@<version>`, `<repository>@latest` (@latest when no version is written), or a directory that starts with `./` or `../` (from where ritsu runs)"
                    ),
                ),
            ],
            vec![format()],
            vec![
                (0, tr!("足した", "added")),
                (2, tr!("引数の誤り、もう求めている名前、`ritsu.mod` の無いディレクトリ、読めない依存の木、届かないレジストリ", "bad arguments, a name required already, a directory with no `ritsu.mod`, a tree that cannot be read, or a registry that cannot be reached")),
            ],
            vec!["ritsu mod add pricing ../pricing", "ritsu mod add tax example.com/acme/tax@1.0.0"],
        ),
        cmd(
            "get",
            "<name>@<version>",
            tr!("レジストリの依存の版を変える（`require` の行の版と lock）", "change the version of a dependency from a registry: the version on its `require` line, and the lock"),
            vec![("<name>@<version>", tr!("依存の名前と版。`<名前>@latest` は、プレリリースでないいちばん高い版", "the dependency's name and version; `<name>@latest` is the highest version that is not a prerelease"))],
            vec![format()],
            vec![
                (0, tr!("版を変えた", "the version was changed")),
                (2, tr!("引数の誤り、求めていない名前かパスの依存、レジストリに無い版、届かないレジストリ", "bad arguments, a name not required or required by a path, a version the registry does not have, or a registry that cannot be reached")),
            ],
            vec!["ritsu mod get tax@1.1.0", "ritsu mod get tax@latest"],
        ),
        cmd(
            "remove",
            "<name>",
            tr!("依存の `require` の行を消し、木に lock が要れば lock も書き直す", "remove a dependency's `require` line, and write the lock again when the tree needs one"),
            vec![("<name>", tr!("依存の名前", "the dependency's name"))],
            vec![],
            vec![(0, tr!("消した", "removed")), (2, tr!("引数の誤り、求めていない名前、読めない依存の木、届かないレジストリ", "bad arguments, a name not required, a tree that cannot be read, or a registry that cannot be reached"))],
            vec!["ritsu mod remove pricing"],
        ),
        cmd(
            "lock",
            "",
            tr!(
                "依存の版を選び（最小の版の選び方）、`ritsu.lock` を書く。レジストリから取る依存が無ければ lock は要らない",
                "choose the versions of the dependencies (Minimal Version Selection) and write `ritsu.lock`; a tree with no dependency from a registry needs no lock"
            ),
            vec![],
            vec![flag("--check", None, tr!("書かずに、lock が書き直しになるなら 1 で終わる（CI 用）", "write nothing, and exit 1 if the lock would be written again (for CI)")), format()],
            vec![
                (0, tr!("書いた、または変わらない", "written, or the same")),
                (1, tr!("--check で、lock が書き直しになる（何も書いていない）", "with --check, the lock would be written again (nothing was written)")),
                (2, tr!("引数の誤り、読めない依存の木、新しい書式の lock、置き場に無く届くレジストリにも無い版", "bad arguments, a tree that cannot be read, a lock of a newer format, or a version in neither the store nor a registry that can be reached")),
            ],
            vec!["ritsu mod lock", "ritsu mod lock --check"],
        ),
        cmd(
            "fetch",
            "[<name>@<version>...]",
            tr!("lock のモジュール（または挙げた版）をレジストリから取り出して置き場に置く", "fetch the modules of the lock (or the versions named) from their registries into the store"),
            vec![("<name>@<version>...", tr!("取る版（依存の名前かモジュールの名前）。無ければ lock のうち、そろっていないもの", "the versions to fetch, by a dependency's name or a module's; else those of the lock that are not here"))],
            vec![flag("--rev", Some("<revision>"), tr!("lock をこの git の版から読む", "read the lock of this git revision")), format()],
            vec![
                (0, tr!("取り出した、またはそろっていた", "fetched, or all there")),
                (1, tr!("ダイジェスト、署名、名前か版の食い違い（lock が合わないときの E304 も）", "a digest, a signature, a name or a version that does not match (and E304 for a lock that does not match)")),
                (2, tr!("引数の誤り、届かないレジストリ、認証の誤り、確かめが要るのに cosign が無い、lock が無い", "bad arguments, a registry that cannot be reached or does not take the credentials, a signature to check with no cosign, or no lock")),
            ],
            vec!["ritsu mod fetch", "ritsu mod fetch example.com/acme/pricing@1.2.0"],
        ),
        cmd(
            "list",
            "",
            tr!(
                "依存の木のモジュールを並べる（取り先、版とダイジェスト、求めているモジュール、ファイルがそろっているか）",
                "list the modules of the tree: where each comes from, its version and digests, what requires it, and whether its files are here"
            ),
            vec![],
            vec![format()],
            vec![
                (0, tr!("並べた", "listed")),
                (1, tr!("lock が依存の木に合わない（E304 と、木のほかのエラー）", "the lock does not match the tree (E304, and the other errors of the tree)")),
                (2, tr!("引数の誤り、モジュールが無い、新しい書式の lock", "bad arguments, no module, or a lock of a newer format")),
            ],
            vec!["ritsu mod list", "ritsu mod list --format json"],
        ),
        cmd(
            "versions",
            "<name>",
            tr!("レジストリにあるモジュールの版を並べる", "list the versions a registry has of a module"),
            vec![("<name>", tr!("依存の名前か、モジュールの名前", "a dependency's name, or a module's"))],
            vec![format()],
            vec![(0, tr!("並べた", "listed")), (2, tr!("引数の誤り、依存でもモジュールでもない名前、届かないレジストリ", "bad arguments, a name of no dependency and no module, or a registry that cannot be reached"))],
            vec!["ritsu mod versions tax", "ritsu mod versions example.com/acme/tax --format json"],
        ),
        cmd(
            "vendor",
            "",
            tr!(
                "lock のレジストリのモジュールを、置き場からモジュールの `.ritsu/modules/` に置く（同梱のコピー）",
                "put the modules of the lock that come from a registry, from the store, in the module's `.ritsu/modules/` (the module's copy of them)"
            ),
            vec![],
            vec![flag("--check", None, tr!("置かずに、同梱のコピーが lock に合わなければ 1 で終わる", "put nothing, and exit 1 if the copy does not match the lock"))],
            vec![
                (0, tr!("同梱のコピーが lock のモジュールを持っている", "the copy holds the modules of the lock")),
                (1, tr!("--check で、同梱のコピーが lock に合わない", "with --check, the copy does not match the lock")),
                (2, tr!("引数の誤り、lock が無い、置き場に無いモジュール", "bad arguments, no lock, or a module that is not in the store")),
            ],
            vec!["ritsu mod vendor", "ritsu mod vendor --check"],
        ),
        cmd(
            "clean",
            "",
            tr!("置き場の項目を消す（全部か、--keep の lock が固定していないもの）", "remove the entries of the store: all of them, or those no lock given with --keep pins"),
            vec![],
            vec![
                flag("--keep", Some("<lock>"), tr!("この `ritsu.lock` が固定するものは残す", "keep what this `ritsu.lock` pins")).repeats(),
                flag("--dry-run", None, tr!("消さずに、消すものを並べる", "remove nothing, and list what would be removed")),
            ],
            vec![(0, tr!("消した（--dry-run では並べた）", "removed (listed, with --dry-run)")), (2, tr!("引数の誤り、読めない --keep の lock、置き場の場所が分からない", "bad arguments, a lock given with --keep that cannot be read, or no place for the store"))],
            vec!["ritsu mod clean --dry-run", "ritsu mod clean --keep ritsu.lock --keep ../shop/ritsu.lock"],
        ),
        cmd(
            "where",
            "<@path>",
            tr!("`@` で始まるパスが、ディスクのどこにあるかを言う", "say where a path that starts with `@` is on the disk"),
            vec![("<@path>", tr!("`@pricing/rules/fee.rule` のような、依存のファイルかディレクトリを指すパス", "a path that names a dependency's file or directory, such as `@pricing/rules/fee.rule`"))],
            vec![],
            vec![(0, tr!("言った", "said")), (2, tr!("引数の誤り、`@` で始まらないパス、モジュールが読めないパス", "bad arguments, a path that does not start with `@`, or one the module cannot read"))],
            vec!["ritsu mod where @pricing/rules/fee.rule", "ritsu mod where @pricing"],
        ),
        cmd(
            "push",
            "",
            tr!("モジュールを検査してレジストリに送り出す（--dry-run では、検査して組み立てるだけで送らない）", "check the module and push it to its registry (with --dry-run, check and build it without sending it)"),
            vec![],
            vec![flag("--dry-run", None, tr!("送らない", "send nothing")), format()],
            vec![
                (0, tr!("送り出した、または送り出し済み", "pushed, or pushed already")),
                (1, tr!("受け付けない（検査、秘密、上限、パスの依存、版のタグに別の中身）", "refused: the check, a secret, a limit, a dependency by a path, or another content at the version's tag")),
                (2, tr!("届かないレジストリ、認証の誤り、lock が無い", "a registry that cannot be reached, the credentials, or no lock")),
            ],
            vec!["ritsu mod push --dry-run", "ritsu mod push"],
        ),
        cmd(
            "diff",
            "<old> [<new>]",
            tr!("モジュールの二つの版の差（ファイル、require、振る舞い、インターフェース）", "the differences between two versions of a module: files, requires, behavior and interfaces"),
            vec![
                ("<old>", tr!("`<モジュールの名前>@<版>`、ディレクトリ、`.` のどれか", "`<module>@<version>`, a directory, or `.`")),
                ("<new>", tr!("同じ形。無ければ `.`（作業ツリー）", "the same; else `.` (the working tree)")),
            ],
            vec![format_markdown(), budget()],
            vec![
                (0, tr!("違いが無い", "no difference")),
                (1, tr!("振る舞いかインターフェースに、示せた違いがある", "a difference shown in behavior or in an interface")),
                (2, tr!("比べられない", "cannot compare")),
            ],
            vec!["ritsu mod diff example.com/acme/pricing@1.0.0", "ritsu mod diff example.com/acme/pricing@1.0.0 example.com/acme/pricing@1.1.0 --format markdown"],
        ),
        cmd(
            "report",
            "",
            tr!(
                "依存を上げると何が変わるかの報告（PR 向け）。古い lock と新しい lock での検査と、変わったモジュールの差",
                "what raising the dependencies changes, for a pull request: the checks with the old lock and the new one, and the differences of the modules that changed"
            ),
            vec![],
            vec![
                flag("--base", Some("<revision>"), tr!("比べる前の git の版（要る）", "the git revision to compare with (required)")),
                flag("--head", Some("<revision>"), tr!("比べるあとの git の版。無ければ作業ツリー", "the git revision compared; else the working tree")),
                format_markdown(),
                flag("--fail-on-change", None, tr!("振る舞いに示せた違いがあるときも 1 で終わる", "exit 1 when a difference is shown in behavior too")),
                budget(),
            ],
            vec![
                (0, tr!("新しい lock での検査にエラーが無く、変わったモジュールをどれも比べられた（振る舞いが変わっていても）", "no error in the check with the new lock, and every module that changed compared (whatever the behavior)")),
                (1, tr!("エラーがある。--fail-on-change では、振る舞いに示せた違いがあるときも", "an error; with --fail-on-change, a difference shown in behavior too")),
                (2, tr!("走らせられない。または、1 にならず、比べられなかったモジュールがある", "cannot run; or nothing makes it 1, and a module that changed could not be compared")),
            ],
            vec!["ritsu mod report --base origin/main --format markdown"],
        ),
        cmd(
            "site",
            "<list>",
            tr!("モジュールの一覧のサイトを書く（版、ファイルのページ、版のあいだの差）", "write the site that lists modules: their versions, the pages of their files and the differences between versions"),
            vec![("<list>", tr!("載せるモジュールを書いた一覧のファイル", "the list file that names the modules to show"))],
            vec![
                flag("--out", Some("<dir>"), tr!("サイトを書く先（要る）", "where the site goes (required)")),
                flag("--playground", Some("<URL>"), tr!("ブラウザで試すページの URL", "the URL of the page that runs ritsu in the browser")),
            ],
            vec![
                (0, tr!("書いた", "written")),
                (1, tr!("確かめられないモジュールがある（そのページにそう書いた）", "a module could not be verified (its page says so)")),
                (2, tr!("引数の誤り、届かないレジストリ", "bad arguments, or a registry that cannot be reached")),
            ],
            vec!["ritsu mod site modules.list --out site"],
        ),
    ]
}

/// How many arguments each command takes, at least and at most.
fn arity(name: &str) -> (usize, usize) {
    match name {
        "init" | "get" | "remove" | "versions" | "where" | "site" => (1, 1),
        "add" => (2, 2),
        "diff" => (1, 2),
        "fetch" => (0, usize::MAX),
        _ => (0, 0),
    }
}

/// The flags every command takes: `--root`, `--lang` and `--help`.
fn globals() -> Vec<Flag> {
    vec![
        flag(
            "--root",
            Some("<dir>"),
            tr!(
                "手元のモジュールのルート。無ければ、走らせたディレクトリの上で `ritsu.mod` を持ついちばん近いディレクトリ",
                "the root of the module at hand; else the nearest directory at or above the one ritsu runs in that holds a `ritsu.mod`"
            ),
        ),
        lang_flag("RITSU_LANG"),
        help_flag(),
    ]
}

/// The whole table.
pub fn table() -> Table {
    Table {
        tool: "ritsu mod",
        version: env!("CARGO_PKG_VERSION"),
        summary: tr!(
            "ritsu のモジュールのコマンド。`ritsu.mod` を持つディレクトリがモジュールで、そのファイルは依存のファイルを `@<名前>/` で指す。下のコマンドが `ritsu.mod` と `ritsu.lock` を書き、依存を置き場に置く。",
            "The commands of ritsu's modules. A directory with a `ritsu.mod` is a module, and its files name a dependency's files by `@<name>/`. These commands write `ritsu.mod` and `ritsu.lock`, and put the dependencies in the store."
        ),
        globals: globals(),
        commands: commands(),
        footer: vec![tr!(
            "どのコマンドも --root <ディレクトリ>、--lang ja|en、--help を取ります。exit code は、0 が成功、1 が確かめた結果の食い違い、2 が走らせられないことです。",
            "Every command takes --root <dir>, --lang ja|en and --help. Exit codes: 0 success, 1 a check that found something that does not match, 2 what could not be run."
        )],
        reading: Reading { no_dashes_in_values: true, ..Reading::default() },
    }
}

/// A command's page of `--help`: the table's, and under the purpose of `diff` and `report`, that
/// what they read is fetched when it is not here (DESIGN 17.9 and 17.10; `Cmd` has no place for a
/// note).
fn page(table: &Table, cmd: &Cmd, lang: Lang) -> String {
    let p = table.help_cmd(cmd, lang);
    if !matches!(cmd.name, "diff" | "report") {
        return p;
    }
    let note = tr!(
        "読むモジュールの版のうち、置き場にも同梱のコピーにも無いものは、`ritsu mod fetch` と同じに取ってくる。そろっていれば通信しない。",
        "The versions it reads that are in neither the store nor the module's copy are fetched, as `ritsu mod fetch` fetches them; when they are all here, it does not use the network."
    );
    match p.split_once("\n\n") {
        Some((head, rest)) => format!("{head}\n\n{}\n\n{rest}", note.get(lang)),
        None => p,
    }
}

/// `--lang ja`, `--lang=ja`, anywhere on the command line.
fn lang_in(args: &[String]) -> Option<String> {
    args.iter().enumerate().find_map(|(i, x)| if x == "--lang" { args.get(i + 1).cloned() } else { x.strip_prefix("--lang=").map(str::to_string) })
}

fn refuse(t: Text, lang: Lang) -> u8 {
    let head = if lang == Lang::Ja { "エラー" } else { "error" };
    eprintln!("{head}: {}", t.get(lang));
    2
}

/// `ritsu mod <command> …`, on the words after `mod`: the exit code.
pub fn command(args: &[String], lang: Lang) -> u8 {
    let table = table();
    let lang = match lang_in(args) {
        Some(l) => Lang::pick(Some(l.as_str()), "RITSU_LANG"),
        None => lang,
    };
    // the command is the first word that is not a flag every command takes, with its value
    let mut at = 0;
    let mut before: Vec<String> = Vec::new();
    while let Some(a) = args.get(at) {
        if a == "--lang" || a == "--root" {
            before.extend(args[at..(at + 2).min(args.len())].iter().cloned());
            at += 2;
        } else if a.starts_with("--lang=") || a.starts_with("--root=") {
            before.push(a.clone());
            at += 1;
        } else {
            break;
        }
    }
    let Some(first) = args.get(at) else {
        eprint!("{}", table.help_all(lang));
        return 2;
    };
    let no_command = |c: &str| refuse(tr!("`{c}` というコマンドはありません。`ritsu mod --help` を読んでください", "there is no command `{c}`; run `ritsu mod --help`"), lang);
    match first.as_str() {
        "--help" | "-h" => {
            print!("{}", table.help_all(lang));
            0
        }
        "help" => match args.get(at + 1) {
            None => {
                print!("{}", table.help_all(lang));
                0
            }
            Some(c) => match table.command(c) {
                Some(cmd) => {
                    print!("{}", page(&table, cmd, lang));
                    0
                }
                None => no_command(c),
            },
        },
        name => {
            let Some(cmd) = table.command(name) else { return no_command(name) };
            let rest: Vec<String> = before.into_iter().chain(args[at + 1..].iter().cloned()).collect();
            let a = match table.parse(cmd, &rest) {
                Ok(a) => a,
                Err(e) => return refuse(e, lang),
            };
            if a.has("--help") {
                print!("{}", page(&table, cmd, lang));
                return 0;
            }
            let (least, most) = arity(cmd.name);
            if a.pos.len() < least || a.pos.len() > most {
                eprintln!("{}", table.usage_line(cmd));
                return 2;
            }
            let ctx = Ctx { command: cmd.name, lang, format: a.get("--format").unwrap_or("text").to_string(), root_flag: a.get("--root").map(str::to_string) };
            run(&ctx, &a)
        }
    }
}

/// Whether the source of `add` is a directory: `./…`, `../…` (DESIGN 17.19); else a registry's
/// repository.
pub fn is_path(source: &str) -> bool {
    source == "." || source == ".." || source.starts_with("./") || source.starts_with("../")
}

/// The command's function, with what its command line said.
fn run(ctx: &Ctx, a: &Args) -> u8 {
    match ctx.command {
        "init" => edit::init(ctx, a),
        "add" if is_path(&a.pos[1]) => edit::add(ctx, a),
        "add" => registry::add(ctx, a),
        "get" => registry::get(ctx, a),
        "remove" => edit::remove(ctx, a),
        "lock" => lock::lock(ctx, a),
        "fetch" => fetch::fetch(ctx, a),
        "list" => list::list(ctx, a),
        "versions" => registry::versions(ctx, a),
        "vendor" => vendor::vendor(ctx, a),
        "clean" => vendor::clean(ctx, a),
        "where" => list::where_is(ctx, a),
        "push" => push::push(ctx, a),
        "diff" => diff::diff(ctx, a),
        "report" => report::report(ctx, a),
        "site" => site::site(ctx, a),
        _ => unreachable!("every command of the table is here"),
    }
}

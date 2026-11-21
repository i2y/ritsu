//! The commands and their flags, in one table (ritsu's DESIGN 4.4). `--help` is drawn from it and
//! the command line is read against it, so a flag cannot be documented and ignored, or taken and
//! left undocumented; an unknown flag, a value outside a closed set, a flag without its value and a
//! flag given twice stop the run with exit 2. The table is ringi's; how it is drawn and read is
//! ritsu-base's ([`ritsu_base::cli`]).
//!
//! `check` and `explain` come with stage B (PLAN 2), `extract` with stage C1, the organization's
//! flags of `check` and `simulate` with stage C2; the other commands of DESIGN 16.1 come with the
//! stages that make what they print.

use crate::check::{self, Options, Run};
use crate::suite::Suite;
use ritsu_base::cli::{Args, Cmd, Flag, Reading, Table, flag, help_flag, lang_flag, made_lang_flag};
use ritsu_base::text::{Lang, Text};
use std::io::Write;

fn every_code() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = crate::codes::ledger().entries.iter().map(|e| e.code).collect();
    v.sort_by_key(|c| (c.starts_with('W'), *c));
    v
}

/// `--root`, as the other languages take it (ritsu's DESIGN 6.2): where the paths in the JSON are
/// written from.
fn root_flag() -> Flag {
    flag(
        "--root",
        Some("<dir>"),
        tr!(
            "JSON に書くパスを数えるルート。無ければ、最初に渡したパスの上で .git か ritsu.mod を持つ一番近いディレクトリ（どちらも無ければ、そのファイルのあるディレクトリ）",
            "the root the paths in the JSON count from; else the nearest directory above the first path given that holds .git or ritsu.mod (else the directory of that file)"
        ),
    )
}

/// `--org`, `--at` and `--through` of `check` (DESIGN 8.3, 16.1).
fn org_flag() -> Flag {
    flag(
        "--org",
        Some("<dir>"),
        tr!(
            "組織のデータのディレクトリ（版のディレクトリと proxies.csv を持つ）。組織のファイルの `data` の代わりに、渡したどの組織のファイルにも使う",
            "the directory of the organization's data (its versions' directories and proxies.csv), read instead of the `data` line of every organization's file of the run"
        ),
    )
}

/// The `.ringi` files under a directory a person names, in the order of their paths: every
/// directory below it, but the hidden ones and the lowered directories (`expense.lowered`).
fn ringi_files(dir: &str, out: &mut Vec<String>) {
    let Ok(rd) = ritsu_base::fs::read_dir(std::path::Path::new(dir)) else { return };
    let mut names: Vec<String> = rd.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).collect();
    names.sort();
    for n in names {
        if n.starts_with('.') || n.ends_with(".lowered") {
            continue;
        }
        let p = if dir == "." { n.clone() } else { format!("{}/{n}", dir.trim_end_matches('/')) };
        if ritsu_base::fs::is_dir(std::path::Path::new(&p)) {
            ringi_files(&p, out);
        } else if n.ends_with(".ringi") {
            out.push(p);
        }
    }
}

/// The file of a run's JSON, from the root above it (ritsu's DESIGN 4.15, 6.2 item 3): `run` and
/// `simulate` take no `--root`, and name the file as `check` does without one.
fn from_root(f: &str) -> String {
    let file = std::path::Path::new(f);
    ritsu_base::paths::from_root(&check::root_of(None, f), file).unwrap_or_else(|| f.to_string())
}

/// A date a flag takes (`--at 2027-04-01`): Err with what to say when it is not one.
fn date_flag(a: &Args, name: &str) -> Result<Option<i64>, Text> {
    match a.get(name) {
        None => Ok(None),
        Some(v) => ritsu_ports::day_of(v).map(Some).ok_or_else(|| tr!("`{name} {v}` は 2027-04-01 の形の日付ではありません", "`{name} {v}` is not a date such as 2027-04-01")),
    }
}

/// The options of a run, from `--org`, `--at` and `--through`.
fn options(a: &Args) -> Result<Options, Text> {
    let org = a.get("--org").map(str::to_string);
    if let Some(o) = &org
        && !ritsu_base::fs::is_dir(std::path::Path::new(o))
    {
        return Err(tr!("`--org {o}` はディレクトリではありません", "`--org {o}` is not a directory"));
    }
    Ok(Options { org, at: date_flag(a, "--at")?, through: date_flag(a, "--through")?, unlisted: false, unrun: false })
}

/// The instant the computer's clock says, in seconds since 1970-01-01T00:00:00Z: `simulate` without
/// `--on` reads today from it, in the UTC offset of the request's calendars
/// ([`crate::simulate::On::Now`]).
fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

pub fn table() -> Table {
    Table {
        tool: "ringi",
        version: env!("CARGO_PKG_VERSION"),
        summary: tr!(
            "会社の申請と承認を書く小さな言語。申請の種類ごとに、フォーム、経路と各ステップの承認者、期限、終わったときにすることを一つのファイルに書き、ritsu の型で確かめる",
            "A small language for a company's applications and approvals: the form of a kind of request, the route it takes and who approves at each stage, its deadlines and what happens when it ends, written in one file and checked in ritsu's types"
        ),
        globals: vec![lang_flag("RINGI_LANG"), help_flag()],
        commands: vec![
            Cmd {
                usage: None,
                name: "check",
                args: "<path>...",
                purpose: tr!(
                    "検査する。構文、名前、`use` で読むファイル、フィールドと式の型、範囲と長さと丸め、フィールドが入る順、議決の数と指名、共有の経路に渡すもの、経路に無いステップ。下ろしたファイルで、経路の表の行、主張と例、期限と催促を、rulec と koyomi に確かめさせる。組織のデータとマスタの行を読み、版ごと、申請者ごとに各ステップの承認者を決めて、空くステップ、届かない議決、二つのステップに当たる同じ人を言い、申請の種類をいつからいつまで使えるかを言う",
                    "check the words and the names, the files the `use` lines read, the types of the fields and the expressions, the ranges, lengths and rounding, the order the fields are filled in, the quorums and the choices of approvers, what a shared route is given, and the stages the route does not have; through the files a request is lowered to, have rulec and koyomi check the rows of the route's table, the claims and the examples, and the deadlines and their reminders; and, reading the organization's data and the masters' rows, find who answers each stage for each version and requester, say the stages with nobody, the quorums out of reach and one person at two stages, and say from and through which day each request can be used"
                ),
                params: vec![("<path>...", tr!("検査する .ringi のファイルかディレクトリ（ディレクトリは、下の .ringi を全部。下ろしたディレクトリ `*.lowered` は除く）", "the .ringi files to check, or directories: a directory stands for every .ringi under it (but the lowered directories, `*.lowered`)"))],
                flags: vec![
                    flag("--format", Some("json"), tr!("ファイルごとに一つの JSON", "one JSON object a file")).choices(&["json"]),
                    root_flag(),
                    org_flag(),
                    flag(
                        "--at",
                        Some("<date>"),
                        tr!("その日（2027-04-01 の形）に効いている組織の版だけを検査する。無ければ、置いてあるどの版も（先の日付の版も）", "check only the version of the organization in effect on that day (a date such as 2027-04-01); else every version there is, the ones to come too"),
                    ),
                    flag(
                        "--through",
                        Some("<date>"),
                        tr!(
                            "どの申請の種類も、その日まで使えることを確かめる。マスタの行かカレンダーのデータがその日の前に終われば、E107 か E404",
                            "hold every request to being usable through that day: a master whose rows or a calendar whose data end before it is E107 or E404"
                        ),
                    ),
                ],
                exits: vec![
                    (0, tr!("エラーなし（警告はありうる）", "no errors (there may be warnings)")),
                    (1, tr!("エラーが一つ以上", "at least one error")),
                    (
                        2,
                        tr!(
                            "引数の誤り、読めないファイル、ほかの言語が要るファイル（申請の種類と共有の経路）を、ほかの言語を持たないこの ringi で走らせた（E008。ritsu ringi で走らせる）",
                            "bad arguments, a file that cannot be read, or a file that needs another language (a request, a shared route) run with this ringi, which holds none (E008: run it as ritsu ringi)"
                        ),
                    ),
                ],
                examples: vec![
                    "ritsu ringi check expense.ringi",
                    "ritsu ringi check purchase.ja.ringi --format json --lang ja",
                    "ritsu ringi check design --at 2027-04-01",
                    "ritsu ringi check design --through 2028-03-31",
                    "ringi check shared/company.ringi shared/masters.ringi --org ../org-next",
                ],
                codes: every_code(),
            },
            Cmd {
                usage: Some("ringi simulate <file.ringi> (--requester <id> | --every-requester) [--form <values>] [--on <date>] [--org <dir>] [--format json]"),
                name: "simulate",
                args: "<file.ringi>",
                purpose: tr!(
                    "人とフォームの値と日付を選んで、経路の各ステップの承認者と、最初のステップの期限を見せる。`--every-requester` は組織のどの人についても見せ、同じ承認者になる人をまとめる",
                    "pick a requester, the values of the form and a day, and see who answers each stage of the route and when the first stage is due; `--every-requester` shows every person of the organization, the ones who come to the same approvers together"
                ),
                params: vec![("<file.ringi>", tr!("申請の種類の .ringi", "a request's .ringi"))],
                flags: vec![
                    flag("--requester", Some("<id>"), tr!("申請者の ID", "the requester's ID")),
                    flag("--every-requester", None, tr!("組織のどの人も、申請者として", "every person of the organization, as the requester")),
                    flag(
                        "--form",
                        Some("<values>"),
                        tr!(
                            "フォームの値を、シナリオの `form` の行と同じ形で（`category = purchase, items = [{{what: \"Server\", price: 800_000JPY}}]`）。無ければ、経路が来うるステップを全部見せる",
                            "the values of the form, written as a scenario's `form` line writes them (`category = purchase, items = [{{what: \"Server\", price: 800_000JPY}}]`); else every stage the route can come to"
                        ),
                    ),
                    flag(
                        "--on",
                        Some("<date>"),
                        tr!(
                            "申請する日（最初のステップが始まる日）。無ければ今日を、申請が使うカレンダーの UTC オフセットで読む（カレンダーが無ければ UTC）",
                            "the day of the request, when its first stage starts; else today, read in the UTC offset of the request's calendars (UTC without one)"
                        ),
                    ),
                    org_flag(),
                    flag("--format", Some("json"), tr!("一つの JSON", "one JSON object")).choices(&["json"]),
                ],
                exits: vec![
                    (0, tr!("見せた", "shown")),
                    (1, tr!("ファイルが検査を通らない", "the file does not pass its check")),
                    (2, tr!("引数の誤り、読めないファイル、組織のデータが無い、その日に効く版が無い、ほかの言語が要るファイルを、ほかの言語を持たないこの ringi で走らせた（E008）", "bad arguments, a file that cannot be read, no data of the organization or no version of it in effect that day, or a file that needs another language run with this ringi, which holds none (E008)")),
                ],
                examples: vec![
                    "ritsu ringi simulate expense.ringi --requester e1201 --form 'category = purchase, items = [{what: \"Server\", price: 800_000JPY}, {what: \"Storage\", price: 400_000JPY}]' --on 2026-11-16",
                    "ritsu ringi simulate leave.ringi --every-requester --on 2027-04-01",
                ],
                codes: vec![],
            },
            Cmd {
                usage: Some("ringi run <file.ringi> [--scenario <name>] [--org <dir>] [--format json]"),
                name: "run",
                args: "<file.ringi>",
                purpose: tr!(
                    "シナリオを流す。表の行を一つずつ、書いた時刻にチケットの機械へ渡し、行ごとに、チケットがどのステップにあり、だれを待ち、期限がいつで、帳簿と flow に何をさせたかを見せる。ステップは下ろした規則、期限は下ろした日付、残高は帳簿が答え、シナリオは答えを持たない",
                    "run the scenarios: each row of a table given to the ticket machine at the time it is written at, and for each row where the ticket is, whom it waits for, when it is due, and what it had the books and the flows do. The stages are the lowered rules', the deadlines the lowered dates', the balances the books': a scenario holds none of them"
                ),
                params: vec![("<file.ringi>", tr!("申請の種類の .ringi", "a request's .ringi"))],
                flags: vec![
                    flag("--scenario", Some("<name>"), tr!("そのシナリオだけを流す。無ければ、ファイルのシナリオを全部", "run that scenario alone; else every scenario of the file")),
                    org_flag(),
                    flag("--format", Some("json"), tr!("一つの JSON。行ごとに、機械に渡した物（前のチケット、イベント、組織の部分、時刻）と答え（チケット、することの一覧）", "one JSON object: for each row, what the machine was given (the ticket before, the event, the part of the organization, the time) and what it answered (the ticket, the things to do)")).choices(&["json"]),
                ],
                exits: vec![
                    (0, tr!("どのシナリオも、書いたとおりに進んだ", "every scenario went as written")),
                    (1, tr!("書いたとおりに進まないシナリオがある（E602〜E604。止まった行までを見せる）か、ファイルが検査を通らない", "a scenario does not go as written (E602–E604; its rows are shown as far as they go), or the file does not pass its check")),
                    (2, tr!("引数の誤り、読めないファイル、シナリオが無い、その名前のシナリオが無い、組織のデータが無い、ほかの言語が要るファイルを、ほかの言語を持たないこの ringi で走らせた（E008）", "bad arguments, a file that cannot be read, no scenario, no scenario of that name, no data of the organization, or a file that needs another language run with this ringi, which holds none (E008)")),
                ],
                examples: vec!["ritsu ringi run leave.ringi", "ritsu ringi run leave.ringi --scenario a_long_paid_leave --format json"],
                codes: vec!["E602", "E603", "E604"],
            },
            Cmd {
                usage: Some("ringi vectors <file.ringi> [--list] [--org <dir>]"),
                name: "vectors",
                args: "<file.ringi>",
                purpose: tr!(
                    "ベクタを書く。チケットの機械の遷移を、一行に一つの JSON で（JSON Lines）。行ごとに、機械に渡した物（前のチケット、イベント、組織の部分、時刻）と答え（チケットとすることの一覧、または断り）。生成したコードとサーバーは、同じ物を渡されて同じ答えを返す。シナリオの行と、そこから作った入力（議決の境、差し戻しの上限、休みの前の日の申請と時計、組織のどの人も申請者に、代理、組織の版をまたぐチケット、フォームの数と行の数の境、例の行）を、どれも機械に流して書く",
                    "write the vectors: transitions of the ticket machine, one JSON object a line (JSON Lines), each with what the machine was given (the ticket before, the event, the part of the organization, the time) and what it answered (the ticket and the things to do, or a refusal). The generated code and a server, given the same, answer the same. They are the rows of the scenarios and what is made from them (the quorums' edges, the limit of the send-backs, the request made before closed days and its clocks, every person as the requester, the proxies, a ticket across the versions of the organization, the edges of the form's numbers and of its rows, the rows of the examples), each run on the machine"
                ),
                params: vec![("<file.ringi>", tr!("申請の種類の .ringi", "a request's .ringi"))],
                flags: vec![
                    flag("--list", None, tr!("JSON の代わりに、ベクタを一行ずつ言葉で（どこから作ったか、イベント、時刻、答え）", "instead of the JSON, each vector in a line of words (where it comes from, the event, the time, the answer)")),
                    org_flag(),
                ],
                exits: vec![
                    (0, tr!("書いた", "written")),
                    (1, tr!("書いたとおりに進まないシナリオがある（E602〜E604）か、ファイルが検査を通らない（何も書かない）", "a scenario does not go as written (E602–E604), or the file does not pass its check (nothing is written)")),
                    (2, tr!("引数の誤り、読めないファイル、シナリオが無い、組織のデータが無い、ほかの言語が要るファイルを、ほかの言語を持たないこの ringi で走らせた（E008）", "bad arguments, a file that cannot be read, no scenario, no data of the organization, or a file that needs another language run with this ringi, which holds none (E008)")),
                ],
                examples: vec!["ritsu ringi vectors leave.ringi > leave.vectors.jsonl", "ritsu ringi vectors leave.ringi --list"],
                codes: vec!["E602", "E603", "E604"],
            },
            Cmd {
                usage: Some("ringi api <file.ringi> [--org <dir>] [--at <date>] [--through <date>] [--root <dir>]"),
                name: "api",
                args: "<file.ringi>",
                purpose: tr!(
                    "ファイルが宣言することを JSON で書く。種類と名前、`use` の行、フォームのフィールド（型と範囲）、中の物（表、ステップ、承認者の決め方、期限、チケットの決まり、主張、例、シナリオ。それぞれの行と定義の文）、使える日の範囲、チケットについて分かること（操作、終わり方、いちばん長い営業日、帳簿の仮押さえ、flow に渡す値）。サーバーや画面を作る側が読む",
                    "write what the file declares as JSON: its kind and name, its `use` lines, the fields of its form (types and ranges), the things it holds (tables, stages, ways to choose an approver, deadlines, rules of the ticket, claims, examples, scenarios, each with its lines and its definition), the days it can be used on, and what is known of its ticket (the operations, the endings, the most business days, the holds in the books, the values given to the flows). For whoever builds a server or a screen"
                ),
                params: vec![("<file.ringi>", tr!("申請の種類、共有の経路、組織、マスタの .ringi", "a .ringi: a request, a shared route, an organization or masters"))],
                flags: vec![
                    org_flag(),
                    flag("--at", Some("<date>"), tr!("`check` と同じ。その日に効いている組織の版だけを読む", "as `check`: read only the version of the organization in effect on that day")),
                    flag("--through", Some("<date>"), tr!("`check` と同じ。その日まで使えなければ E107 か E404 で、何も書かない", "as `check`: E107 or E404, and nothing written, when the request cannot be used through that day")),
                    root_flag(),
                    made_lang_flag("RINGI_LANG"),
                ],
                exits: vec![
                    (0, tr!("書いた", "written")),
                    (1, tr!("ファイルが検査を通らない（JSON は書かず、`check` と同じ診断を出す）", "the file does not pass its check (no JSON is written; the diagnostics of `check` are printed)")),
                    (2, tr!("引数の誤り、読めないファイル、ほかの言語が要るファイルを、ほかの言語を持たないこの ringi で走らせた（E008）", "bad arguments, a file that cannot be read, or a file that needs another language run with this ringi, which holds none (E008)")),
                ],
                examples: vec!["ritsu ringi api expense.ringi", "ritsu ringi api shared/company.ringi"],
                codes: vec![],
            },
            doc_entry(),
            diff_entry(),
            Cmd {
                usage: Some("ringi extract <file.ringi> --out <dir> [--org <dir>] [--at <date>] [--through <date>]"),
                name: "extract",
                args: "<file.ringi>",
                purpose: tr!(
                    "下ろしたファイルを書き出す。検査で rulec と koyomi に確かめさせるのと同じ中身を、ディスクに書く（中に書いた物を、共有のファイルに移すとき）",
                    "write out the files a request is lowered to: the same files the check has rulec and koyomi check, on the disk (to move what is written inside into files of their own)"
                ),
                params: vec![("<file.ringi>", tr!("申請の種類か共有の経路の .ringi", "a request's or a shared route's .ringi"))],
                flags: vec![
                    flag("--out", Some("<dir>"), tr!("書き出すディレクトリ（無ければ作る。同じ名前のファイルは書き換える）", "the directory to write into (made when it is not there; a file of the same name is written over)")).needed(),
                    org_flag(),
                    flag("--at", Some("<date>"), tr!("`check` と同じ。その日に効いている組織の版だけを読む", "as `check`: read only the version of the organization in effect on that day")),
                    flag("--through", Some("<date>"), tr!("`check` と同じ。その日まで使えなければ E107 か E404 で、何も書かない", "as `check`: E107 or E404, and nothing written, when the request cannot be used through that day")),
                ],
                exits: vec![
                    (0, tr!("書き出した", "written")),
                    (1, tr!("ファイルが検査を通らない（何も書かない）。組織の版ごとの承認者の一覧（E502〜E504、W502、W506）は、下ろす物に効かないので、ここでは言わない", "the file does not pass its check (nothing is written); the approvers of each version of the organization (E502–E504, W502, W506) do not change what is lowered, and are not listed here")),
                    (2, tr!("引数の誤り、読めないファイル、ほかの言語が要るファイルを、ほかの言語を持たないこの ringi で走らせた（E008）", "bad arguments, a file that cannot be read, or a file that needs another language run with this ringi, which holds none (E008)")),
                ],
                examples: vec!["ritsu ringi extract expense.ringi --out shared/expense"],
                codes: vec![],
            },
            Cmd {
                usage: Some("ringi init [<dir>] --module <path> [--catalog <url>] [--mcp] [--temporal] [--lang ja|en]"),
                name: "init",
                args: "[<dir>]",
                purpose: tr!(
                    "申請と承認のサーバーを始める Go のプロジェクトの、最初のファイルを書く。申請の種類を一つと、それが読む組織、カレンダー、gate、そこから ritsu gen が作るコード、フレームワークのサーバー、試験、README。`--mcp` はエージェントのツールを、`--temporal` は flow を持つ申請の種類とワーカーを足す。書いたあとは使う人のファイルで、init は一度だけ書き、どのファイルも上書きしない。`--lang ja` なら日本語の名前で書き、日本語の README も書く。ritsu の中でだけ走る",
                    "write the first files of a Go project that serves requests and approvals: one request type with the organization, the calendar and the gate it reads, the code ritsu gen makes of them, a server of the framework, its tests and a README. `--mcp` adds the agents' tools, and `--temporal` a request type with a flow and a worker. They are the user's from then on: init writes them once and overwrites no file. `--lang ja` writes the project with Japanese names, and a Japanese README too. It runs inside ritsu only"
                ),
                params: vec![("[<dir>]", tr!("書くディレクトリ（無ければ作る）。無ければ今のディレクトリ", "the directory to write in, made when it is not there; else the current directory"))],
                flags: vec![
                    flag("--module", Some("<path>"), tr!("Go のモジュールのパス。`go mod init` が取る物（`example.com/acme/approvals`）。要る", "the Go module's path, as `go mod init` takes it (`example.com/acme/approvals`); required")).needed(),
                    flag(
                        "--catalog",
                        Some("<url>"),
                        tr!("画面のカタログの ID。無ければ `https://<モジュールのパス>/a2ui/catalog.json`", "the ID of the catalog of the screens; else `https://<the module's path>/a2ui/catalog.json`"),
                    ),
                    flag(
                        "--mcp",
                        None,
                        tr!(
                            "サーバーの `/mcp` で、エージェントのツールを MCP で出す（フレームワークのモジュール `mcp`）。`tools.go` と、ツールで申請を出して読む試験も書く",
                            "serve the agents' tools over MCP at `/mcp` of the server (the framework's module `mcp`): `tools.go` is written too, with a test that makes a request with the tools and reads it"
                        ),
                    ),
                    flag(
                        "--temporal",
                        None,
                        tr!(
                            "承認されると flow で払い戻す経費の精算を、申請の種類として足し、flow を Temporal の上で動かす（フレームワークのモジュール `temporal`）。flow を始める所、flow のタスク、ワーカーのコマンド `cmd/worker`、Temporal の開発用サーバーで flow を動かす試験も書く",
                            "add a request type whose approval starts a flow (an expense claim, paid back by the flow), and run the flows on Temporal (the framework's module `temporal`): what starts them, the flow's tasks, a worker's command `cmd/worker`, and a test that runs the flow on Temporal's dev server are written too"
                        ),
                    ),
                    made_lang_flag("RINGI_LANG"),
                ],
                exits: vec![
                    (0, tr!("書いた", "written")),
                    (
                        2,
                        tr!(
                            "引数の誤り、`--module` が無い、書くファイルがもうある（どれも書かない）、書けない所、ringi のバイナリで走らせた",
                            "bad arguments, no `--module`, files there already (nothing is written), a place that cannot be written, or run with ringi's binary"
                        ),
                    ),
                ],
                examples: vec![
                    "ritsu ringi init approvals --module example.com/acme/approvals",
                    "ritsu ringi init approvals --module example.com/acme/approvals --lang ja",
                    "ritsu ringi init approvals --module example.com/acme/approvals --mcp --temporal",
                ],
                codes: vec![],
            },
            Cmd {
                usage: Some("ringi explain <CODE>... | --all [--format markdown|json] [--lang ja|en]"),
                name: "explain",
                args: "<CODE>... | --all",
                purpose: tr!("診断のコードを説明する。いつ出るか、直し方、それを出す一番小さな例", "explain diagnostic codes: when each is printed, how to fix it, and the smallest file that prints it"),
                params: vec![("<CODE>...", tr!("`E101` のようなコード。いくつでも渡せて、渡した順に説明する。`--all` とは一緒に渡さない", "codes such as `E101`, any number, explained in the order given; not given with `--all`"))],
                flags: vec![
                    flag("--all", None, tr!("全部のコード", "every code")),
                    flag("--format", Some("markdown|json"), tr!("Markdown（docs/codes.md の形）か JSON", "Markdown (as docs/codes.md is) or JSON")).choices(&["markdown", "json"]),
                ],
                exits: vec![(0, tr!("説明した", "explained")), (2, tr!("知らないコード、引数の誤り", "an unknown code, or bad arguments"))],
                examples: vec!["ringi explain E101", "ringi explain E501 W502 --format json", "ringi explain --all --format markdown --lang ja"],
                codes: vec![],
            },
        ],
        footer: vec![
            tr!("どのコマンドも `--lang ja|en` と `--help` を取ります。", "Every command takes `--lang ja|en` and `--help`."),
            tr!(
                "exit code: 0 エラーなし / 1 エラーあり / 2 引数の誤りか読めないファイル、またはほかの言語が要るファイルを、ほかの言語を持たないこの ringi で走らせた（E008）",
                "Exit codes: 0 no errors / 1 errors found / 2 bad arguments or an unreadable file, or a file that needs another language run with this ringi, which holds none (E008)"
            ),
        ],
        reading: Reading::default(),
    }
}

/// Say what is wrong with the command line, or with a file it cannot read: one line on `w`, after
/// the head every command of ritsu writes ([`ritsu_base::cli::refuse`]), exit 2.
pub(crate) fn refuse(w: &mut dyn Write, msg: Text, lang: Lang) -> u8 {
    ritsu_base::cli::refuse(w, &msg, lang)
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

/// The language the command says in: `--lang` anywhere on the line, else `RINGI_LANG`, else English.
/// `ritsu ringi` reads it too, and has rulec read the files a request is lowered to in it, so that
/// each is checked once in the run (ritsu's DESIGN 4.15).
pub fn lang_of(args: &[String]) -> Lang {
    Lang::pick(lang_word(args).as_deref(), "RINGI_LANG")
}

/// The command line, read against the table and run: the exit code. A line that asks for JSON and
/// stops before its work gets `{"error": …}` on the standard output, whatever the command (ritsu's
/// DESIGN 6.2, item 10).
pub fn run(args: &[String], suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    ritsu_base::cli::answering(args, out, |out| command(args, suite, out, err))
}

/// What stops `name` given arguments it does not take, or none: what it takes, and its usage line,
/// the command named the way it was run (ritsu's DESIGN 6.2, item 10).
fn takes(name: &str) -> Text {
    table().takes_of(name)
}

fn command(args: &[String], suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let lang = lang_of(args);
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
            let _ = writeln!(out, "ringi {}", env!("CARGO_PKG_VERSION"));
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
                    None => refuse(err, ritsu_base::cli::no_command("ringi", n), lang),
                },
            };
        }
        _ => {}
    }
    let Some(cmd) = table.command(first) else {
        return refuse(err, ritsu_base::cli::no_command("ringi", first), lang);
    };
    let a = match table.parse(cmd, &args[1..]) {
        Ok(a) => a,
        Err(e) => return refuse(err, e, lang),
    };
    if a.has("--help") {
        let _ = write!(out, "{}", table.help_cmd(cmd, lang));
        return 0;
    }
    // a path that starts with `@`, in a module, is read as a path written in a file is (ritsu's DESIGN
    // 17.6)
    if let Err(t) = ritsu_base::modules::given_all(&a.pos) {
        return refuse(err, t, lang);
    }
    // the paths the run holds are written the way the first path given was (ritsu's DESIGN 4.15),
    // and so are the files and directories given: `./expense.ringi` and `x/../expense.ringi` are
    // `expense.ringi`, `./examples` and `examples/` are `examples` (6.2 item 8); and so are the
    // directories `--org` and `--out` take: `--out ./lowered` is `lowered`. The code `explain`
    // takes is no path
    let _way = ritsu_base::paths::run_from(a.pos.first().filter(|_| cmd.name != "explain").map_or(".", String::as_str));
    let mut a = a;
    if cmd.name != "explain" {
        for p in &mut a.pos {
            *p = ritsu_base::paths::show_given(p);
        }
    }
    a.fold_paths(&["--org", "--out"]);
    let a = a;
    match cmd.name {
        "check" => check_cmd(&a, lang, suite, out, err),
        "simulate" => simulate_cmd(&a, lang, suite, out, err),
        "run" => run_cmd(&a, lang, suite, out, err),
        "vectors" => vectors_cmd(&a, lang, suite, out, err),
        "api" => api_cmd(&a, lang, suite, out, err),
        "doc" => doc_cmd(&a, lang, suite, out, err),
        "diff" => diff_cmd(&a, lang, suite, out, err),
        "extract" => extract_cmd(&a, lang, suite, out, err),
        "explain" => explain_cmd(&a, lang, out, err),
        // `ritsu ringi` runs `init` before it comes here: it writes what `ritsu gen` makes
        "init" => refuse(
            err,
            tr!(
                "`init` は、ritsu gen で作るコードも書くので、ritsu の中でだけ走ります。`ritsu ringi init` を走らせてください",
                "`init` writes the code ritsu gen makes too, so it runs inside ritsu only: run `ritsu ringi init`"
            ),
            lang,
        ),
        _ => unreachable!("every command in the table is dispatched"),
    }
}

fn check_cmd(a: &Args, lang: Lang, suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    if a.pos.is_empty() {
        return refuse(err, takes("check"), lang);
    }
    let root = match a.get("--root") {
        Some(r) if !ritsu_base::fs::is_dir(std::path::Path::new(r)) => return refuse(err, tr!("`--root {r}` はディレクトリではありません", "`--root {r}` is not a directory"), lang),
        given => check::root_of(given.map(std::path::Path::new), &a.pos[0]),
    };
    let opts = match options(a) {
        Ok(o) => o,
        Err(t) => return refuse(err, t, lang),
    };
    let json = a.get("--format") == Some("json");
    // a directory stands for the .ringi files under it
    let mut files = Vec::new();
    for p in &a.pos {
        if ritsu_base::fs::is_dir(std::path::Path::new(p)) {
            let before = files.len();
            ringi_files(p, &mut files);
            if files.len() == before {
                return refuse(err, ritsu_base::cli::no_files_under(p, &[".ringi"]), lang);
            }
        } else {
            files.push(p.clone());
        }
    }
    // a file named twice (by itself and under a directory) is said once, where it is first named:
    // the file the system opens, however the path is written (ritsu's DESIGN 4.15)
    let mut seen = std::collections::BTreeSet::new();
    files.retain(|f| seen.insert(ritsu_base::paths::opened(std::path::Path::new(f))));
    // one run for every file given: an organization or masters several requests read is checked once
    let run = Run::with(suite.clone(), opts);
    run.set_lang(lang);
    let mut worst = 0u8;
    for f in &files {
        match run.file(f) {
            Ok(o) => {
                worst = worst.max(o.exit());
                if json {
                    let _ = writeln!(out, "{}", check::to_json(&o, &root, lang).pretty());
                } else {
                    let _ = write!(out, "{}", check::render(&o, lang));
                }
            }
            Err(e) => {
                worst = 2;
                refuse(err, ritsu_base::cli::cannot_read(f, &e), lang);
            }
        }
    }
    worst
}

/// `ringi api <file.ringi>`: what the file declares, as JSON, when it passes its check; its
/// diagnostics on the error stream, and no JSON, when it does not.
fn api_cmd(a: &Args, lang: Lang, suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let [f] = a.pos.as_slice() else {
        return refuse(err, takes("api"), lang);
    };
    let root = match a.get("--root") {
        Some(r) if !ritsu_base::fs::is_dir(std::path::Path::new(r)) => return refuse(err, tr!("`--root {r}` はディレクトリではありません", "`--root {r}` is not a directory"), lang),
        given => check::root_of(given.map(std::path::Path::new), f),
    };
    let opts = match options(a) {
        Ok(o) => o,
        Err(t) => return refuse(err, t, lang),
    };
    let run = Run::with(suite.clone(), opts);
    run.set_lang(lang);
    let o = match run.file(f) {
        Ok(o) => o,
        Err(e) => return refuse(err, ritsu_base::cli::cannot_read(f, &e), lang),
    };
    let (Some(file), false) = (&o.file, o.has_errors()) else {
        let _ = write!(err, "{}", check::render(&o, lang));
        return o.exit().max(1);
    };
    let src = match ritsu_base::fs::read_to_string(std::path::Path::new(f)) {
        Ok(s) => s,
        Err(e) => return refuse(err, ritsu_base::cli::cannot_read(f, &e), lang),
    };
    // the JSON's sentences are in `--lang`, else English, whatever the environment says (ritsu's
    // DESIGN 4.1)
    let _ = writeln!(out, "{}", crate::api::to_json(&o, file, &src, &o.names, &root, Lang::made(a.get("--lang"))).pretty());
    0
}

/// `ringi extract <file.ringi> --out <dir>`: the files the check lays, written to the disk when the
/// file passes its check; nothing written when it does not (its diagnostics are printed, exit 1).
fn extract_cmd(a: &Args, lang: Lang, suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let [f] = a.pos.as_slice() else {
        return refuse(err, takes("extract"), lang);
    };
    let Some(dir) = a.get("--out") else {
        return refuse(err, table().needs_of("extract", "--out <dir>"), lang);
    };
    let opts = match options(a) {
        Ok(o) => o,
        Err(t) => return refuse(err, t, lang),
    };
    // who answers each stage does not change what is lowered: `check` lists it
    let run = Run::with(suite.clone(), Options { unlisted: true, unrun: true, ..opts });
    run.set_lang(lang);
    let o = match run.file(f) {
        Ok(o) => o,
        Err(e) => return refuse(err, ritsu_base::cli::cannot_read(f, &e), lang),
    };
    if o.has_errors() {
        let _ = write!(out, "{}", check::render(&o, lang));
        return o.exit();
    }
    let files = o.lowered.as_ref().map(|l| crate::lower::extracted(l, std::path::Path::new(dir))).unwrap_or_default();
    if files.is_empty() {
        let _ = writeln!(out, "{}", tr!("`{f}` には、下ろす物がありません", "`{f}` lowers to nothing").get(lang));
        return 0;
    }
    if let Err(e) = ritsu_base::fs::create_dir_all(dir) {
        return refuse(err, ritsu_base::cli::cannot_create(dir, &e), lang);
    }
    for (name, text) in &files {
        let p = std::path::Path::new(dir).join(name);
        // under the directory `--out` takes, as it was given and folded (`Args::fold_paths`): absolute
        // only where it was given so (ritsu's DESIGN 4.15)
        let shown = p.display().to_string();
        if let Err(e) = ritsu_base::fs::write(&p, text) {
            return refuse(err, ritsu_base::cli::cannot_write(&shown, &e), lang);
        }
        let _ = writeln!(out, "{}", tr!("生成しました: {shown}", "wrote {shown}").get(lang));
    }
    0
}

/// `ringi simulate <file.ringi> (--requester <id> | --every-requester) [--form <values>] [--on
/// <date>] [--org <dir>]`: who answers each stage, as [`crate::simulate`] solves it.
fn simulate_cmd(a: &Args, lang: Lang, suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let [f] = a.pos.as_slice() else {
        return refuse(err, takes("simulate"), lang);
    };
    let opts = match options(a) {
        Ok(o) => o,
        Err(t) => return refuse(err, t, lang),
    };
    let on = match date_flag(a, "--on") {
        Ok(Some(d)) => crate::simulate::On::Day(d),
        Ok(None) => crate::simulate::On::Now(now()),
        Err(t) => return refuse(err, t, lang),
    };
    // one requester or every one, before the file is read: what it takes
    if a.has("--every-requester") == a.get("--requester").is_some() {
        return refuse(err, table().one_of("simulate", &["--requester", "--every-requester"]), lang);
    }
    // a stage with nobody is shown as it comes to the requester, not said as the check says it
    let run = Run::with(suite.clone(), Options { unlisted: true, unrun: true, ..opts });
    run.set_lang(lang);
    let o = match run.file(f) {
        Ok(o) => o,
        Err(e) => return refuse(err, ritsu_base::cli::cannot_read(f, &e), lang),
    };
    if o.has_errors() {
        let _ = write!(out, "{}", check::render(&o, lang));
        return o.exit();
    }
    let ask = crate::simulate::Ask { requester: a.get("--requester").map(str::to_string), every: a.has("--every-requester"), form: a.get("--form").map(str::to_string), on };
    let answer = match crate::simulate::simulate(&o, &run, &ask) {
        Ok(x) => x,
        Err(t) => return refuse(err, t, lang),
    };
    let Some(v) = answer.org_version() else { return refuse(err, tr!("組織のデータがありません", "the organization has no data"), lang) };
    if a.get("--format") == Some("json") {
        let _ = writeln!(out, "{}", crate::simulate::to_json(&from_root(f), &answer, lang).pretty());
    } else {
        let _ = write!(out, "{}", crate::simulate::render(f, v, &answer, lang));
    }
    0
}

/// `ringi run <file.ringi> [--scenario <name>] [--org <dir>]`: the scenarios of a request, each row
/// given to the ticket machine ([`crate::scenario`]). The file is checked once, without its
/// scenarios, and they are run on what the check came to.
fn run_cmd(a: &Args, lang: Lang, suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let [f] = a.pos.as_slice() else {
        return refuse(err, takes("run"), lang);
    };
    let opts = match options(a) {
        Ok(o) => o,
        Err(t) => return refuse(err, t, lang),
    };
    let run = Run::with(suite.clone(), Options { unlisted: true, unrun: true, ..opts });
    run.set_lang(lang);
    let o = match run.file(f) {
        Ok(o) => o,
        Err(e) => return refuse(err, ritsu_base::cli::cannot_read(f, &e), lang),
    };
    if o.has_errors() {
        let _ = write!(out, "{}", check::render(&o, lang));
        return o.exit();
    }
    let json = a.get("--format") == Some("json");
    let named = a.get("--scenario");
    let ran = crate::engine::with(&o, &run, |m, org| {
        let all = &m.s.file.scenarios;
        // nothing to run is said by the exit code, as `vectors` says it
        if all.is_empty() {
            return Err(tr!("`{f}` にシナリオがありません", "`{f}` has no scenario"));
        }
        if let Some(n) = named
            && !all.iter().any(|sc| sc.name.text == n)
        {
            let names: Vec<String> = all.iter().map(|sc| format!("`{}`", sc.name.text)).collect();
            let (ja, en) = (names.join("、"), names.join(", "));
            return Err(tr!("`{f}` に `{n}` というシナリオはありません。あるのは {ja} です", "`{f}` has no scenario `{n}`: it has {en}"));
        }
        let mut worst = 0u8;
        let mut text = String::new();
        let mut list = Vec::new();
        for sc in all.iter().filter(|sc| named.is_none_or(|n| sc.name.text == n)) {
            let r = crate::scenario::run(m, org, sc);
            if r.stop.is_some() {
                worst = 1;
            }
            if json {
                list.push(crate::scenario::to_json(m, &r, lang));
            } else {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(&crate::scenario::render(m, &r, lang));
                if let Some(stop) = &r.stop {
                    text.push_str(&crate::scenario::diag(m, stop).render(lang));
                }
            }
        }
        if json {
            text = format!("{}\n", ritsu_base::json::Json::obj([("ringi", ritsu_base::json::Json::str(env!("CARGO_PKG_VERSION"))), ("file", ritsu_base::json::Json::str(from_root(f))), ("scenarios", ritsu_base::json::Json::arr(list))]).pretty());
        }
        Ok((worst, text))
    });
    match ran {
        Ok(Ok((worst, text))) => {
            let _ = write!(out, "{text}");
            worst
        }
        Ok(Err(t)) | Err(t) => refuse(err, t, lang),
    }
}

fn vectors_cmd(a: &Args, lang: Lang, suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let [f] = a.pos.as_slice() else {
        return refuse(err, takes("vectors"), lang);
    };
    let opts = match options(a) {
        Ok(o) => o,
        Err(t) => return refuse(err, t, lang),
    };
    let run = Run::with(suite.clone(), Options { unlisted: true, unrun: true, ..opts });
    run.set_lang(lang);
    let o = match run.file(f) {
        Ok(o) => o,
        Err(e) => return refuse(err, ritsu_base::cli::cannot_read(f, &e), lang),
    };
    if o.has_errors() {
        let _ = write!(out, "{}", check::render(&o, lang));
        return o.exit();
    }
    let list = a.has("--list");
    let made = crate::engine::with(&o, &run, |m, org| {
        let all = &m.s.file.scenarios;
        if all.is_empty() {
            return Err(tr!("`{f}` にシナリオがありません。ベクタは、シナリオの申請者とフォームから作ります", "`{f}` has no scenario: the vectors are made from the requester and the form of a scenario"));
        }
        // the scenarios first, each as written: the vectors are made from where they come
        let rans: Vec<_> = all.iter().map(|sc| crate::scenario::run(m, org, sc)).collect();
        if let Some(stop) = rans.iter().find_map(|r| r.stop.as_ref()) {
            return Ok((1, crate::scenario::diag(m, stop).render(lang)));
        }
        let mut text = String::new();
        for v in crate::vectors::make(m, org, &rans) {
            text.push_str(&if list { v.listed(m) } else { v.to_json(m).compact() });
            text.push('\n');
        }
        Ok((0, text))
    });
    match made {
        Ok(Ok((code, text))) => {
            let _ = write!(out, "{text}");
            code
        }
        Ok(Err(t)) | Err(t) => refuse(err, t, lang),
    }
}

/// `ringi explain <code>... | --all` (ritsu's DESIGN 6.2, item 10): the codes in the order given,
/// one as it is written alone and several as `--all` writes every code; codes it does not have
/// are named together, and nothing is explained.
fn explain_cmd(a: &Args, lang: Lang, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let format = a.get("--format");
    let ledger = crate::codes::ledger();
    if a.has("--all") {
        // codes and `--all` are the two ways to call it, not one: what it takes
        if !a.pos.is_empty() {
            return refuse(err, takes("explain"), lang);
        }
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
    if a.pos.is_empty() {
        return refuse(err, takes("explain"), lang);
    }
    match ledger.look_up(&a.pos) {
        Ok(entries) => {
            let shape = match format {
                Some("markdown") => ritsu_base::ledger::Shape::Markdown,
                Some("json") => ritsu_base::ledger::Shape::Json,
                _ => ritsu_base::ledger::Shape::Text,
            };
            let _ = write!(out, "{}", ledger.explained(&entries, shape, lang));
            0
        }
        Err(unknown) => refuse(err, ritsu_base::cli::no_codes("ringi", &unknown), lang),
    }
}

// ---------------------------------------------------------------------------------------------
// The pages (DESIGN 11): `doc`

/// `doc`'s line of the table.
fn doc_entry() -> Cmd {
    Cmd {
        usage: Some("ringi doc <file.ringi> [--format markdown|html] [--longest [--on <date>]] [--org <dir>] [--at <date>] [--through <date>] [--root <dir>] [--lang ja|en]"),
        name: "doc",
        args: "<file.ringi>",
        purpose: tr!(
            "ファイルを一枚のページにする。承認する人と申請を管理する人が読む言葉で、申請の種類なら、使える期間、出せる人、フォーム、経路（区分ごとのステップ）、承認者の決め方、期限と催促（例の日付つき）、チケットの決まり、コメント、代理、帳簿と flow、主張、例、シナリオ。共有の経路、組織、マスタも、それぞれの一枚を書く。`--longest` は、チケットがいちばん長くかかる場合を日付つきで見せ、何を変えれば何日短くなるかを並べる",
            "write the file as one page, in the words of the people who approve and manage the requests: for a request, the days it can be used on, who may submit it, its form, its route (the stages of each case), how its approvers are chosen, its deadlines and reminders (with dates), the rules of a ticket, comments, proxies, books and flows, its claims, examples and scenarios; a shared route, an organization and masters each get a page of their own. `--longest` shows the longest a ticket can take, with dates, and what shortens it by how many days"
        ),
        params: vec![("<file.ringi>", tr!("申請の種類、共有の経路、組織、マスタの .ringi", "a .ringi: a request, a shared route, an organization or masters"))],
        flags: vec![
            flag("--format", Some("markdown|html"), tr!("Markdown（既定）か、外から何も読まない一つの HTML のファイル", "Markdown (the default), or one HTML file that reads nothing from outside")).choices(&["markdown", "html"]),
            flag("--longest", None, tr!("チケットがいちばん長くかかる場合のページ（申請の種類だけ）", "the page of the longest a ticket takes (a request only)")),
            flag(
                "--on",
                Some("<date>"),
                tr!("`--longest` のチケットを出す日（2027-04-01 の形）。無ければ、使える最初の営業日のうち、休みの前の日", "the day the ticket of `--longest` is submitted on (a date such as 2027-04-01); else the first business day the request can be used on that comes before a day off"),
            ),
            org_flag(),
            flag("--at", Some("<date>"), tr!("`check` と同じ。その日に効いている組織の版だけを読む", "as `check`: read only the version of the organization in effect on that day")),
            flag("--through", Some("<date>"), tr!("`check` と同じ。その日まで使えなければ E107 か E404 で、何も書かない", "as `check`: E107 or E404, and nothing written, when the request cannot be used through that day")),
            flag("--root", Some("<dir>"), tr!("ページの頭に書くパスを数えるルート。無ければ `check` と同じ", "the root the path in the head of the page counts from; else as `check`")),
            made_lang_flag("RINGI_LANG"),
        ],
        exits: vec![
            (0, tr!("書いた", "written")),
            (1, tr!("ファイルが検査を通らない（ページは書かず、`check` と同じ診断を出す）", "the file does not pass its check (no page is written; the diagnostics of `check` are printed)")),
            (
                2,
                tr!(
                    "引数の誤り、読めないファイル、申請の種類でないファイルに `--longest`、使えない日の `--on`、ほかの言語が要るファイルを、ほかの言語を持たないこの ringi で走らせた（E008）",
                    "bad arguments, a file that cannot be read, `--longest` of a file that is no request, `--on` a day the request cannot be used on, or a file that needs another language run with this ringi, which holds none (E008)"
                ),
            ),
        ],
        examples: vec!["ritsu ringi doc expense.ringi > expense.md", "ritsu ringi doc expense.ja.ringi --format html --lang ja > expense.ja.html", "ritsu ringi doc contract.ringi --longest --on 2026-12-25"],
        codes: vec![],
    }
}

/// `ringi doc <file.ringi>`: the page of the file, when it passes its check; its diagnostics on the
/// error stream, and no page, when it does not. The page is in `--lang`, else English, whatever the
/// environment says (ritsu's DESIGN 4.1); what is said on the terminal is in the language of the
/// command.
fn doc_cmd(a: &Args, lang: Lang, suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let [f] = a.pos.as_slice() else {
        return refuse(err, takes("doc"), lang);
    };
    let root = match a.get("--root") {
        Some(r) if !ritsu_base::fs::is_dir(std::path::Path::new(r)) => return refuse(err, tr!("`--root {r}` はディレクトリではありません", "`--root {r}` is not a directory"), lang),
        given => check::root_of(given.map(std::path::Path::new), f),
    };
    let opts = match options(a) {
        Ok(o) => o,
        Err(t) => return refuse(err, t, lang),
    };
    let on = match date_flag(a, "--on") {
        Ok(d) => d,
        Err(t) => return refuse(err, t, lang),
    };
    if on.is_some() && !a.has("--longest") {
        return refuse(err, tr!("`--on` は `--longest` のチケットを出す日です。`--longest` と一緒に渡してください", "`--on` is the day the ticket of `--longest` is submitted on: give it with `--longest`"), lang);
    }
    let run = Run::with(suite.clone(), opts);
    run.set_lang(lang);
    let o = match run.file(f) {
        Ok(o) => o,
        Err(e) => return refuse(err, ritsu_base::cli::cannot_read(f, &e), lang),
    };
    if o.file.is_none() || o.has_errors() {
        let _ = write!(err, "{}", check::render(&o, lang));
        return o.exit().max(1);
    }
    // the file as the page names it: from the root, however the path was given (ritsu's DESIGN 4.15)
    let shown = ritsu_base::paths::from_root(&root, std::path::Path::new(f)).unwrap_or_else(|| f.to_string());
    let project = crate::doc::project_of(&root, a.get("--root").is_some());
    let ask = crate::doc::Ask { longest: a.has("--longest"), on, project };
    match crate::doc::page(&o, &run, &shown, Lang::made(a.get("--lang")), &ask) {
        Ok(p) => {
            let text = if a.get("--format") == Some("html") { crate::doc::write::html(&p) } else { crate::doc::write::markdown(&p) };
            let _ = write!(out, "{text}");
            0
        }
        Err(t) => refuse(err, t, lang),
    }
}

/// `diff`'s usage line: the two ways to call it in one line, so that the line is named the way the
/// command was run as a whole (`ritsu ringi diff …`; ritsu's DESIGN 6.2, item 10). `--at` and
/// `--budget` are the two files' alone, as their help says.
const DIFF_USAGE: &str = "ringi diff (<old.ringi> <new.ringi> | --versions <date> <date> <file.ringi>) [--format text|markdown|html|json] [--org <dir>] [--at <date>] [--root <dir>] [--budget <n>] [--lang ja|en]";

fn diff_entry() -> Cmd {
    Cmd {
        usage: Some(DIFF_USAGE),
        name: "diff",
        args: "<old.ringi> <new.ringi>",
        purpose: tr!(
            "二つの版で、言うことと答えが違う所を、承認する人と申請を管理する人の言葉で言う。申請の種類なら、経路が変わる申請の区分（どのステップに、だれが答えるか）、組織のデータがあれば承認者が変わる申請者、フォーム、期限、チケットの決まり、出せる人、承認のあとにすること、使える期間の変更と、進行中のチケットへの効き方。`--versions` は、組織の二つの版で、承認者が変わる申請者とステップ、出せる人の変わり、答える人がいないステップの数を並べる",
            "say where two versions say or answer differently, in the words of the people who approve and manage the requests: for a request, the cases whose route changes (which stage, answered by whom), the requesters whose approvers change where the organization's data is read, and the changes of the form, the deadlines, the rules of a ticket, who may submit it, what is done after approval and the days it can be used on, with how each reaches the tickets in progress. `--versions` holds a request to two versions of the organization: the requesters and stages whose approvers change, who may submit it, and the stages nobody answers"
        ),
        params: vec![
            ("<old.ringi>", tr!("前の版（申請の種類、共有の経路、組織、マスタのどれか）", "the old version (a request, a shared route, an organization or masters)")),
            ("<new.ringi>", tr!("後の版。同じ種類と名前のファイル", "the new version: a file of the same kind and name")),
        ],
        flags: vec![
            flag("--versions", Some("<date>"), tr!("組織の版を比べる。その日と、次に渡す日に効いている二つの版で、渡した申請の種類を読み比べる", "compare two versions of the organization: the one in effect on this day and the one on the day given next, for the request given")),
            flag("--format", Some("text|markdown|html|json"), tr!("端末の文、Markdown のページ、外のファイルを読まない一枚の HTML、JSON", "text for the terminal, a Markdown page, one HTML file that reads nothing from outside, or JSON")).choices(&["text", "markdown", "html", "json"]),
            org_flag(),
            flag("--at", Some("<date>"), tr!("`check` と同じ。その日に効いている組織の版だけを読む（`--versions` とは渡せない）", "as `check`: read only the version of the organization in effect on that day (not with `--versions`)")),
            flag("--root", Some("<dir>"), tr!("ファイルの名前を数えるルート。無ければ `check` と同じ", "the root the files are named from; else as `check`")),
            flag("--budget", Some("<n>"), tr!("経路を比べるときに歩くセルの上限（rulec の `diff` の `--budget`。`--versions` とは渡せない）", "the cap on the cells walked to compare the routes (rulec's `diff --budget`; not with `--versions`)")),
            made_lang_flag("RINGI_LANG"),
        ],
        exits: vec![
            (0, tr!("違いが無い", "no difference")),
            (1, tr!("違いがある", "a difference")),
            (2, tr!("比べられない（引数の誤り、読めないファイル、検査を通らない版、種類か名前の違う二つ、組織のデータが無い `--versions`、ほかの言語が要るファイルを、ほかの言語を持たないこの ringi で走らせた（E008））", "cannot compare: bad arguments, a file that cannot be read, a version that does not pass its check, two files of other kinds or names, `--versions` with no data of the organization, or a file that needs another language run with this ringi, which holds none (E008)")),
        ],
        examples: vec!["ritsu ringi diff expense.ringi expense.v2.ringi", "ritsu ringi diff expense.ringi expense.v2.ringi --format markdown --lang ja", "ritsu ringi diff --versions 2026-04-01 2027-04-01 expense.ringi"],
        codes: vec![],
    }
}

/// `ringi diff <old> <new>` and `ringi diff --versions <date> <date> <file>`: the text follows the
/// language of the run; a page and the JSON, what `--lang` says alone (`Lang::made`).
fn diff_cmd(a: &Args, lang: Lang, suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    use crate::diff::{Refused, Side};
    let format = a.get("--format").unwrap_or("text");
    let made = if format == "text" { lang } else { Lang::made(a.get("--lang")) };
    let opts = match options(a) {
        Ok(o) => o,
        Err(t) => return refuse(err, t, lang),
    };
    let budget = match a.get("--budget") {
        None => None,
        Some(b) => match b.parse::<usize>() {
            Ok(n) if n > 0 => Some(n),
            _ => return refuse(err, ritsu_base::cli::not_positive("--budget", b), lang),
        },
    };
    let root = |first: &str| match a.get("--root") {
        Some(r) if !ritsu_base::fs::is_dir(std::path::Path::new(r)) => Err(tr!("`--root {r}` はディレクトリではありません", "`--root {r}` is not a directory")),
        given => Ok(check::root_of(given.map(std::path::Path::new), first)),
    };
    let side = |root: &std::path::Path, f: &str| Side { read: f.to_string(), files: None, shown: ritsu_base::paths::from_root(root, std::path::Path::new(f)).unwrap_or_else(|| f.to_string()) };
    let refused = |err: &mut dyn Write, r: Refused| match r {
        Refused::Said(t) => refuse(err, t, lang),
        Refused::Check { said, .. } => {
            let _ = write!(err, "{said}");
            2
        }
    };
    if let Some(first) = a.get("--versions") {
        if opts.at.is_some() {
            return refuse(err, tr!("`--at` は組織の版を一つだけ読みます。`--versions` と一緒には渡せません", "`--at` reads one version of the organization: it does not go with `--versions`"), lang);
        }
        // the versions of the organization are compared on the request as it is: no route of two
        // files is walked, and a cap on the walk would be read and do nothing
        if budget.is_some() {
            return refuse(err, tr!("`--budget` は、二つのファイルの経路を比べるときの上限です。`--versions` と一緒には渡せません", "`--budget` caps the walk that compares the routes of two files: it does not go with `--versions`"), lang);
        }
        let [second, f] = a.pos.as_slice() else {
            let command = format!("{} diff --versions", ritsu_base::cli::command_name("ringi"));
            return refuse(err, ritsu_base::cli::takes(&command, "<date> <date> <file.ringi>", DIFF_USAGE), lang);
        };
        let day = |d: &str| ritsu_ports::day_of(d).ok_or_else(|| tr!("`{d}` は 2027-04-01 の形の日付ではありません", "`{d}` is not a date such as 2027-04-01"));
        let days = match (day(first), day(second)) {
            (Ok(x), Ok(y)) => (x, y),
            (Err(t), _) | (_, Err(t)) => return refuse(err, t, lang),
        };
        let root = match root(f) {
            Ok(r) => r,
            Err(t) => return refuse(err, t, lang),
        };
        return match crate::diff::versions(&side(&root, f), days, suite, &opts, lang, made) {
            Ok(v) => {
                let text = match format {
                    "markdown" => crate::diff::versions_markdown(&v),
                    "html" => crate::diff::versions_html(&v),
                    "json" => format!("{}\n", crate::diff::versions_json(&v).pretty()),
                    _ => crate::diff::versions_text(&v),
                };
                let _ = write!(out, "{text}");
                u8::from(v.differs())
            }
            Err(r) => refused(err, r),
        };
    }
    let [old, new] = a.pos.as_slice() else {
        return refuse(err, takes("diff"), lang);
    };
    let root = match root(old) {
        Ok(r) => r,
        Err(t) => return refuse(err, t, lang),
    };
    match crate::diff::diff(&side(&root, old), &side(&root, new), suite, &opts, budget, lang, made) {
        Ok(d) => {
            let text = match format {
                "markdown" => crate::diff::markdown(&d),
                "html" => crate::diff::html(&d),
                "json" => format!("{}\n", crate::diff::json(&d).pretty()),
                _ => crate::diff::text(&d),
            };
            let _ = write!(out, "{text}");
            u8::from(d.differs())
        }
        Err(r) => refused(err, r),
    }
}

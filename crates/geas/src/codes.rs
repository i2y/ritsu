//! Every code geas reports, written once: a one-line summary, when it appears, what
//! usually fixes it, and a repro that gives it, in English and in Japanese.
//! `geas explain` prints from here, and `tests/explain.rs` runs every repro and
//! checks that it gives its own code, so the table cannot drift from the tool.

use crate::diag::Severity;
use ritsu_base::text::{Lang, Text};
use crate::json;

/// The smallest run that gives a code.
pub struct Repro {
    /// geas's arguments: `["check", "e002.geas"]`.
    pub args: &'static [&'static str],
    /// The files it needs, as (path, contents), written first.
    pub files: &'static [(&'static str, &'static str)],
    /// The exit status it ends with.
    pub exit: i32,
    /// Programs it needs besides geas.
    pub needs: &'static [&'static str],
    /// Environment variables it sets for geas.
    pub env: &'static [(&'static str, &'static str)],
}

pub struct Entry {
    pub code: &'static str,
    pub summary: Text,
    pub when: Text,
    pub fix: Text,
    pub repro: Repro,
}

impl Entry {
    pub fn severity(&self) -> Severity {
        Severity::of(self.code)
    }
}

// ---------- the repros: the smallest files that give each code ----------

const X_E001: &str = "target calc {
  run \"python3 calc.py\"
}

claim \"adds two integers {
  when calc.run(\"2\", \"+\", \"3\")
  then stdout is \"5\"
}
";

const X_E002: &str = "target calc {
  run \"python3 calc.py\"
}

claim \"adds two integers\" {
  when calc.run(\"2\", \"+\", \"3\")
  then output is \"5\"
}
";

const X_E003: &str = "target calc {
  run \"python3 calc.py\"
}

claim \"adds two integers\" {
  when calc.run(\"2\", \"+\", \"3\")
  then stdout is \"5\"
}

claim \"adds two integers\" {
  when calc.run(\"-2\", \"+\", \"3\")
  then stdout is \"1\"
}
";

const X_E004: &str = "target api {
  serve \"python3 server.py 8123\"
}

claim \"greets by name\" {
  when api.get(\"/greet?name=Ada\")
  then status is 200
}
";

const X_E005: &str = "target calc {
  run \"python3 calc.py\"
}

claim \"adds two integers\" {
  then stdout is \"5\"
  when calc.run(\"2\", \"+\", \"3\")
}
";

const X_E006: &str = "target calc {
  run \"python3 calc.py\"
}

claim \"adds two integers\" {
  when calc.get(\"/add?a=2&b=3\")
  then body is \"5\"
}
";

const X_E007: &str = "target calc {
  run \"python3 calc.py\"
}

claim \"adds two integers\" {
  when calc.run(\"2\", \"+\", \"3\")
  then status is 200
}
";

const X_E008: &str = "target calc {
  run \"python3 calc.py\"
}

claim \"adds two integers\" {
  when calc.run(\"2\", \"+\", \"3\")
  then exit is \"0\"
}
";

const X_E009: &str = "target calc {
  run \"python3 calc.py\"
}

claim \"adds two integers\" {
  when calc.run(\"2\", \"+\", \"3\")
  then stdout matches \"(5\"
}
";

const X_E010: &str = "target calc {
  run \"python3 'my calc.py\"
}

claim \"adds two integers\" {
  when calc.run(\"2\", \"+\", \"3\")
  then stdout is \"5\"
}
";

const X_E011: &str = "target calc {
  run \"python3 calc.py\"
  clock \"2026-08-29T09:00:00+09:00\"
}

claim \"adds two integers\" {
  when calc.run(\"2\", \"+\", \"3\")
  then stdout is \"5\"
}
";

const X_E012: &str = "target app {
  pixie \"build/greeter\"
}

target calc {
  run \"python3 calc.py\"
}

claim \"greets after adding\" {
  when app.input(\"Ada\")
  when calc.run(\"2\", \"+\", \"3\")
  when app.click(\"greet\")
  then screen contains text \"Hello, Ada!\"
}
";

const X_E013: &str = "target app {
  pixie \"build/greeter\"
}

claim \"greets the name typed in\" {
  when app.input(\"Ada\", into: \"type here\")
  when app.click(\"greet\")
  then screen contains text \"Hello, Ada!\"
}
";

const X_E030: &str = "target calc {
  run \"geas-example-no-such-program\"
}

claim \"adds two integers\" {
  when calc.run(\"2\", \"+\", \"3\")
  then stdout is \"5\"
}
";

const X_E031: &str = "target slow {
  run \"sleep 10\"
}

claim \"finishes\" {
  when slow.run()
  then exit is 0
}
";

const X_E032: &str = "target api {
  serve \"false\"
  port 8123
}

claim \"answers\" {
  when api.get(\"/\")
  then status is 200
}
";

const X_E033: &str = "target api {
  serve \"python3 hangup.py 8123\"
  port 8123
}

claim \"answers\" {
  when api.get(\"/\")
  then status is 200
}
";

const HANGUP_PY: &str = "# Accepts every connection and closes it without answering.
import socket
import sys

server = socket.create_server((\"127.0.0.1\", int(sys.argv[1])))
while True:
    connection, _ = server.accept()
    connection.close()
";

const X_E034: &str = "target web {
  serve \"python3 -m http.server {port} --bind 127.0.0.1\"
  port auto
}

claim \"shows the files\" {
  when web.open(\"/\")
  then screen contains heading containing \"Directory listing\"
}
";

const X_E035: &str = "target app {
  driver \"python3 driver.py\"
}

claim \"saves\" {
  when app.click(\"Save\")
  then screen contains text \"saved\"
}
";

const SAVE_DRIVER_PY: &str = "# A driver of a screen with one button, `save`, that refuses every other click.
import json
import sys

screen = {\"children\": [{\"role\": \"button\", \"name\": \"save\"}]}
for line in sys.stdin:
    msg = json.loads(line)
    if \"geas\" in msg:
        print(json.dumps({\"ok\": True}), flush=True)
    elif msg[\"do\"] == \"close\":
        break
    elif msg[\"do\"] == \"click\" and msg[\"name\"] != \"save\":
        print(json.dumps({\"error\": \"no button named \" + msg[\"name\"], \"screen\": screen}), flush=True)
    else:
        print(json.dumps({\"screen\": screen}), flush=True)
";

const X_E036: &str = "target web {
  serve \"python3 broken.py {port}\"
  port auto
}

claim \"shows the page\" {
  when web.open(\"/\")
  then screen contains heading \"Welcome\"
}
";

const BROKEN_PY: &str = "# Answers every request with two lengths for one body, which Chrome refuses to load.
import socket
import sys

server = socket.create_server((\"127.0.0.1\", int(sys.argv[1])))
while True:
    connection, _ = server.accept()
    try:
        connection.recv(65536)
        connection.sendall(b\"HTTP/1.1 200 OK\\r\\nContent-Length: 5\\r\\nContent-Length: 6\\r\\n\\r\\nhello!\")
    except OSError:
        pass
    connection.close()
";

const X_E037: &str = "target app {
  driver \"python3 driver.py\"
}

claim \"opens\" {
  when app.open()
  then screen contains button \"save\"
}
";

const CHATTY_DRIVER_PY: &str = "# A driver that takes the pins, then answers every action with a line that is not JSON.
import json
import sys

for line in sys.stdin:
    msg = json.loads(line)
    if \"geas\" in msg:
        print(json.dumps({\"ok\": True}), flush=True)
    elif msg[\"do\"] == \"close\":
        break
    else:
        print(\"ready\", flush=True)
";

const X_DRIFT: &str = "target calc {
  run \"python3 calc.py\"
}

claim \"adds two integers\" {
  when calc.run(\"2\", \"+\", \"3\")
  then stdout is \"5\"
}
";

const X_ADDS: &str = "target calc {
  run \"python3 calc.py\"
}

claim \"adds\" {
  when calc.run(\"2\", \"3\")
  then stdout is \"5\"
}
";

/// calc.py as the records of the repros knew it, and as the change leaves it.
// Only the unit test below reads CALC_BEFORE: it holds the records' blob to it.
#[cfg_attr(not(test), allow(dead_code))]
const CALC_BEFORE: &str = "import sys
print(int(sys.argv[1]) + int(sys.argv[2]))
";

const CALC_AFTER: &str = "import sys
a, b = int(sys.argv[1]), int(sys.argv[2])
print(a + b)
";

/// A record of `CALC_BEFORE`, for a spec named `<stem>.geas` in the root.
macro_rules! record_of_before {
    ($stem:literal) => {
        concat!(
            "{\"geas_map\":1,\"spec\":\"",
            $stem,
            ".geas\",\"root\":\".\",\"claims\":[{\"name\":\"adds\",\"status\":\"ok\",\"targets\":[\"calc\"]}]}\n",
            "{\"file\":\"calc.py\",\"blob\":\"f296577308110001ace80922309b2a55b5c8b854\",\"lang\":\"python\",\"code\":\"1-2\"}\n",
            "{\"claim\":\"adds\",\"target\":\"calc\",\"file\":\"calc.py\",\"ran\":\"1-2\"}\n"
        )
    };
}

/// The change from `CALC_BEFORE` to `CALC_AFTER`, as `git diff` writes it.
const CALC_CHANGE: &str = "diff --git a/calc.py b/calc.py
index f296577..178c189 100644
--- a/calc.py
+++ b/calc.py
@@ -1,2 +1,3 @@
 import sys
-print(int(sys.argv[1]) + int(sys.argv[2]))
+a, b = int(sys.argv[1]), int(sys.argv[2])
+print(a + b)
";

/// A change to the README alone.
const README_CHANGE: &str = "diff --git a/README.md b/README.md
index 734af0a..62c08b9 100644
--- a/README.md
+++ b/README.md
@@ -1 +1,2 @@
 A calculator.
+It adds.
";

/// A plain diff of calc.py that fits neither the file on disk nor that file before
/// the change.
const ELSEWHERE_CHANGE: &str = "--- calc.py.orig
+++ calc.py
@@ -1,2 +1,2 @@
 import sys
-print(x)
+print(y)
";

const X_E065: &str = "target fake {
  run \"python3 fake.py\"
}

claim \"runs\" {
  when fake.run()
  then exit is 0
}
";

const FAKE_PROFILE_PY: &str = "# Writes a file where a Rust program built with coverage writes its profile.
import os

name = os.environ[\"LLVM_PROFILE_FILE\"].replace(\"%p\", str(os.getpid())).replace(\"%m\", \"1_0\")
with open(name, \"w\") as f:
    f.write(\"not a profile\")
";

const X_E066: &str = "target api {
  serve \"python3 stubborn.py 8123\"
  port 8123
}

claim \"answers\" {
  when api.get(\"/\")
  then status is 200
}
";

const STUBBORN_PY: &str = "# A service that ignores SIGTERM, so it never stops by itself.
import signal
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

signal.signal(signal.SIGTERM, signal.SIG_IGN)


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.send_header(\"Content-Length\", \"0\")
        self.end_headers()

    def log_message(self, *args):
        pass


HTTPServer((\"127.0.0.1\", int(sys.argv[1])), Handler).serve_forever()
";

const X_W060: &str = "target five {
  run \"echo 5\"
}

claim \"prints five\" {
  when five.run()
  then stdout is \"5\"
}
";

const X_W061: &str = "target tally {
  run \"sh tally.sh\"
}

claim \"adds\" {
  when tally.run(\"2\", \"3\")
  then stdout is \"5\"
}
";

const TALLY_SH: &str = "# Builds the program with coverage the first time, then runs it: the program is
# started by sh, not by geas.
[ -x tally ] || rustc --edition 2021 -C instrument-coverage -o tally tally.rs
./tally \"$@\"
";

const TALLY_RS: &str = "fn main() {
    let sum: i64 = std::env::args().skip(1).map(|a| a.parse::<i64>().unwrap_or(0)).sum();
    println!(\"{sum}\");
}
";

/// A claims file that parses, and a file that is no OpenSpec spec (E090).
const X_E090: &str = "target calc {
  run \"python3 calc.py\"
}

claim \"adds two integers\" {
  when calc.run(\"2\", \"+\", \"3\")
  then stdout is \"5\"
}
";
const NOTES_MD: &str = "# Notes\n\nNothing here is a spec.\n";

/// The one fake key the repro of W901 holds (ritsu's DESIGN 16.10): a Google API key that reads
/// as a fake. It is put together from two pieces, so that no file of the source holds a key in one
/// run; what `explain` prints holds it whole.
macro_rules! fake_key {
    () => {
        concat!("AIzaSyD-ritsu-fake-", "key-for-tests-000000")
    };
}

const X_W901: &str = concat!(
    "# echo stands in for the address lookup\n# the Google Maps API key it reads against the real service: ",
    fake_key!(),
    "\n\ntarget lookup {\n  run \"echo\"\n}\n\nclaim \"passes the address on\" {\n  when lookup.run(\"1600 Amphitheatre Parkway\")\n  then stdout is \"1600 Amphitheatre Parkway\"\n}\n"
);

fn spec(args: &'static [&'static str], files: &'static [(&'static str, &'static str)], exit: i32) -> Repro {
    Repro { args, files, exit, needs: &[], env: &[] }
}

/// Every code, in code order.
pub fn table() -> Vec<Entry> {
    vec![
        Entry {
            code: "E001",
            summary: tr!(
                "文法に合わない文字、文字列、語",
                "a character, a string or a token the grammar does not allow there",
            ),
            when: tr!(
                "主張のファイルに、文字列とコメントの外には書けない文字（`@` など）、行のうちに閉じていない文字列、`\\n`・`\\t`・`\\\"`・`\\\\` 以外のエスケープ、文法と合わない語（`\"` で囲んだ主張の名前を書くところにある名前、足りない `{{`・`(`・`)`・`:`、文字列を書くところにある数など）があるとき。そこから先は正しく読めるとは限らないので、geas は最初の一つで読むのをやめます。",
                "The spec holds a character geas does not read outside a string or a comment (such as `@`), a string not closed on its line, an escape other than `\\n`, `\\t`, `\\\"` and `\\\\`, or a token where the grammar wants another: a name where the claim's name in quotes goes, a missing `{{`, `(`, `)` or `:`, a number where a string goes. geas stops at the first one, since what follows it cannot be read reliably.",
            ),
            fix: tr!(
                "メッセージが示すものを、その場所に書いてください。文字列は一行のうちに `\"` で開いて閉じ、中の改行は `\\n` と書いてください。",
                "Write there what the message says the grammar expects. A string opens and closes with `\"` on one line; a line break inside it is written `\\n`.",
            ),
            repro: spec(&["check", "e001.geas"], &[("e001.geas", X_E001)], 2),
        },
        Entry {
            code: "E002",
            summary: tr!("geas が知らない名前", "a name geas does not know"),
            when: tr!(
                "名前を書くところに、geas の知らない語があるとき。ファイルに宣言していないターゲット（`when` の後ろ）、`run`・`get`・`post`・`open`・`click`・`input`・`submit`・`press`・`advance` 以外の呼び出し、呼び出しが受け付けない引数（`into:` のない `nth:` も）、`stdout`・`stderr`・`exit`・`status`・`header`・`body`・`body json`・`screen` 以外のチェックするもの、`is`・`is not`・`is above`・`is below`・`is at least`・`is at most`・`is between`・`contains`・`does not contain`・`matches`・`does not match`・`exists`・`does not exist` 以外の比べ方、WAI-ARIA のロールと `text` 以外のノードのロール、`disabled`・`enabled`・`checked`・`unchecked` 以外の状態、`run`・`serve`・`pixie`・`driver`・`port`・`serial` と固定（`env`・`tz`・`locale`・`clock`・`seed`）以外のターゲットの行、`header`・`body json`・`screen` 以外の `mask` が、これに当たります。",
                "A word that has to name something geas knows names nothing it knows: a target in a `when` that the file does not declare, a call other than `run`, `get`, `post`, `open`, `click`, `input`, `submit`, `press` and `advance`, an argument a call does not take (`nth:` without `into:` among them), something to check other than `stdout`, `stderr`, `exit`, `status`, `header`, `body`, `body json` and `screen`, a way to compare other than `is`, `is not`, `is above`, `is below`, `is at least`, `is at most`, `is between`, `contains`, `does not contain`, `matches`, `does not match`, `exists` and `does not exist`, a role in a node other than WAI-ARIA's and `text`, a state other than `disabled`, `enabled`, `checked` and `unchecked`, a line in a target other than `run`, `serve`, `pixie`, `driver`, `port`, `serial` and the pins (`env`, `tz`, `locale`, `clock`, `seed`), or a `mask` of something other than `header`, `body json` and `screen`.",
            ),
            fix: tr!(
                "メッセージに並ぶ語のどれかに直してください。`when` で使うターゲットは、同じファイルに `target <名前> {{ … }}` で宣言してください。",
                "Correct the word: the message lists the words that fit there. A target a `when` names is declared in the same file, with `target <name> {{ … }}`.",
            ),
            repro: spec(&["check", "e002.geas"], &[("e002.geas", X_E002)], 2),
        },
        Entry {
            code: "E003",
            summary: tr!(
                "同じ名前のターゲットか主張が二つある。同じものを二回固定している",
                "two targets, or two claims, with one name; one thing pinned twice",
            ),
            when: tr!(
                "一つのファイルの中で、二つのターゲット、または二つの主張が同じ名前を持つとき。`when` はターゲットを名前で探し、ベースラインは主張ごとの結果を主張の名前で持つので、二つ目が一つ目と取り違えられます。また、一つの場所で同じものを二回固定したとき（ターゲットの中の二つの `tz`、ターゲットの外の同じ環境変数）や、二つの固定が一つのターゲットの同じ環境変数を設定するとき（`tz` と `env \"TZ\"`）にも出ます。どちらが効くのかが、読む人の目に入るところに書かれないからです。",
                "Two targets, or two claims, in one file have the same name. A `when` finds its target by name, and the baseline keeps each claim's observations under the claim's name, so a second one would be taken for the first. Or one place pins one thing twice (`tz` twice in a target, the same variable twice outside any target), or two pins set one variable of a target (`tz` and `env \"TZ\"`): which one wins would not be written anywhere a reader looks.",
            ),
            fix: tr!(
                "どちらかの名前を変えるか、二つの固定のどちらかを消してください。ターゲットの中の固定がターゲットの外の同じ固定を置き換えるのは、誤りではありません。",
                "Rename one of them, or keep one of the two pins. A pin inside a target replaces the same pin outside it, which is not an error.",
            ),
            repro: spec(&["check", "e003.geas"], &[("e003.geas", X_E003)], 2),
        },
        Entry {
            code: "E004",
            summary: tr!("ターゲットの行の組み合わせが合わない", "a target whose lines do not fit together"),
            when: tr!(
                "ターゲットに `run`・`serve`・`pixie`・`driver` のどれもないとき、そのうち二つがあるとき、`serve` に `port` がないとき、`serve` がないのに `port` があるとき、`port auto` なのにコマンドにも `env` の値にも `{{port}}` がないとき（geas が渡すポート番号を、サービスが知る方法がありません）、同じ行が二つあるとき。`run` のターゲットは `when` のたびに geas が起動するコマンド、`serve` のターゲットは主張ごとに一度起動し、`port` にリクエストを送るかページとして開くサービス、`pixie` のターゲットは pixie のアプリ、`driver` のターゲットはドライバーのプログラムで動かす GUI です。",
                "A target has none of `run`, `serve`, `pixie` and `driver`, or two of them; `serve` without `port`; `port` without `serve`; `port auto` with no `{{port}}` in its command or its `env` values, so that the service has no way to learn the port geas gives it; or one of its lines twice. A `run` target is a command geas starts for each `when`; a `serve` target is a service geas starts once per claim and sends requests to on its `port`, or opens as a page; a `pixie` target is a pixie app, and a `driver` target a GUI reached through a driver program.",
            ),
            fix: tr!(
                "コマンドのターゲットには `run` を一行、サービスのターゲットには `serve` と `port` を一行ずつ、pixie のアプリには `pixie` を一行、ドライバーで動かす GUI には `driver` を一行書いてください。`port auto` なら、サービスがポート番号を読むところに `{{port}}` を書いてください（`serve \"python3 server.py {{port}}\"`）。",
                "Give a command target one `run` line, a service target one `serve` line and one `port` line, a pixie app one `pixie` line, and a GUI behind a driver one `driver` line. With `port auto`, write `{{port}}` where the service reads its port: `serve \"python3 server.py {{port}}\"`.",
            ),
            repro: spec(&["check", "e004.geas"], &[("e004.geas", X_E004)], 2),
        },
        Entry {
            code: "E005",
            summary: tr!("実行できない主張", "a claim that cannot run"),
            when: tr!(
                "主張にステップが一つもないか、`when` より前に `then`（または `and`）があるとき。主張は `when` を上から順に実行し、チェックはそれぞれ直前の `when` の結果を見ます。",
                "A claim has no steps, or a `then` (or an `and`) comes before any `when`. A claim runs its `when`s in order, and each check reads what the `when` before it observed.",
            ),
            fix: tr!(
                "チェックが見る結果を返す `when` を、主張の最初に書いてください。",
                "Start the claim with the `when` whose observation the check reads.",
            ),
            repro: spec(&["check", "e005.geas"], &[("e005.geas", X_E005)], 2),
        },
        Entry {
            code: "E006",
            summary: tr!("ターゲットが受け付けない呼び出し", "a call its target does not take"),
            when: tr!(
                "`when` の呼び出しを、そのターゲットが受け付けないとき。`run` のターゲットに `get`・`post`・操作（`open`・`click`・`input`・`submit`・`press`・`advance`）を使ったとき、`serve` のターゲットに `run` を使ったとき、`pixie` や `driver` のターゲットに操作以外を使ったときです。",
                "A `when` makes a call its target does not take: `get`, `post` or an action (`open`, `click`, `input`, `submit`, `press`, `advance`) on a `run` target; `run` on a `serve` target; anything but an action on a `pixie` or a `driver` target.",
            ),
            fix: tr!(
                "コマンドは `run(…)` で、サービスは `get(…)`・`post(…)`・ページへの操作で、pixie のアプリとドライバーで動かす GUI は操作で呼んでください。別のターゲットのつもりだったなら、`when` のターゲットを直してください。",
                "Call a command with `run(…)`, a service with `get(…)`, `post(…)` or the actions on its page, and a pixie app or a GUI behind a driver with the actions; or point the `when` at the target meant.",
            ),
            repro: spec(&["check", "e006.geas"], &[("e006.geas", X_E006)], 2),
        },
        Entry {
            code: "E007",
            summary: tr!(
                "直前の `when` の結果にないもののチェック",
                "a check of something its `when` does not observe",
            ),
            when: tr!(
                "チェックが、直前の `when` の結果にないものを見ているとき。`run` のあとの `status`・`header`・`body`・`screen`、`get` や `post` のあとの `stdout`・`stderr`・`exit`・`screen`、操作のあとの `screen` 以外です。geas はこれを、何かを実行する前に見つけます。",
                "A check names something the `when` before it does not observe: `status`, `header`, `body` or `screen` after a `run`; `stdout`, `stderr`, `exit` or `screen` after a `get` or a `post`; anything but `screen` after an action. geas finds this before it runs anything.",
            ),
            fix: tr!(
                "その呼び出しの結果にあるもの（`run` なら stdout・stderr・exit、`get` と `post` なら status・header・body・body json、操作なら screen）をチェックするか、それを返す `when` のあとにチェックを移してください。",
                "Check what that call gives (a `run`: stdout, stderr, exit; a `get` or a `post`: status, header, body, body json; an action: screen), or move the check after the `when` that gives it.",
            ),
            repro: spec(&["check", "e007.geas"], &[("e007.geas", X_E007)], 2),
        },
        Entry {
            code: "E008",
            summary: tr!(
                "比べ方が、チェックするものか値に合わない",
                "a matcher that does not fit its subject or its value",
            ),
            when: tr!(
                "比べ方が、チェックするものか、その後ろの値に合わないときです。数である `exit` や `status` に `contains`・`matches`・`exists`（とその `does not` の形）を使ったとき、ヘッダーと JSON のパス以外に `exists` を使ったとき（ないことがあるのはこの二つだけです）、`exit` や `status` を文字列・`true`・`false`・`null` と比べたとき、文字列を JSON の値である `true`・`false`・`null` と比べたとき、`is above`・`is below`・`is at least`・`is at most`・`is between` に数以外を渡したときや `is between` の大きいほうを先に書いたとき、`contains` に文字列以外を渡したとき、`matches` に `\"` で囲んだパターン以外を渡したとき、`screen` を `contains` や `does not contain` とノード以外でチェックしたとき、文字列や数の `contains` にノードを渡したときです。",
                "The matcher does not fit what it checks, or the value after it: `contains`, `matches` or `exists` (or their `does not` forms) on `exit` or `status`, which are numbers; `exists` on anything but a header or a JSON path, the two subjects that can be absent; `exit` or `status` compared with a string, `true`, `false` or `null`; a text compared with `true`, `false` or `null`, which are JSON values; `is above`, `is below`, `is at least`, `is at most` or `is between` given anything but a number, or `is between` with its larger end first; `contains` given anything but a string; `matches` given anything but a pattern in quotes; `screen` checked with anything but `contains` or `does not contain` and a node; a node given to `contains` on a text or a number.",
            ),
            fix: tr!(
                "数は `is`・`is not`・数の比べ方で、`\"` で囲まない数と比べてください（`exit is 0`、`status is between 200 and 299`）。`contains` には文字列を、`matches` にはパターンを、どちらも `\"` で囲んで渡してください。`exists` で確かめるのは、ヘッダーか JSON のパスです。画面は `screen contains button \"greet\"` のように、ノードでチェックしてください。",
                "Compare a number with `is`, `is not` or a number matcher, and a number written without quotes (`exit is 0`, `status is between 200 and 299`); give `contains` a string and `matches` a pattern, both in quotes; ask `exists` of a header or a JSON path; check the screen for a node, `screen contains button \"greet\"`.",
            ),
            repro: spec(&["check", "e008.geas"], &[("e008.geas", X_E008)], 2),
        },
        Entry {
            code: "E009",
            summary: tr!("読めないパターン、または読めない JSON のパス", "a pattern, or a JSON path, that does not parse"),
            when: tr!(
                "`matches` や `does not match` の後ろのパターンが、geas の読める形になっていないときです。開いたままの `(` や `[`、前に何もない繰り返しや別の繰り返しの直後の繰り返し、`{{n}}`・`{{n,}}`・`{{n,m}}` の形でない回数や 1000 を超える回数、`^` や `$`（パターンはいつも値の全体と照らし合わせます）、それに `\\d`・`\\w`・`\\s` とその大文字、`\\n`・`\\t`・`\\r`、文字でも数字でもない文字の前のバックスラッシュ以外のエスケープが、これに当たります。`body json` や `mask body json` の後ろの JSON のパスが、`.key` と `[0]` をつないだ形になっていないときにも出ます。列は、読めなくなった文字の位置を指します。",
                "A pattern after `matches` or `does not match` is not one geas reads: a `(` or a `[` left open, a repetition with nothing before it or right after another, a count that is not `{{n}}`, `{{n,}}` or `{{n,m}}`, or is above 1000, `^` or `$` (a pattern always matches the whole value), or an escape other than `\\d`, `\\w`, `\\s` and their capitals, `\\n`, `\\t`, `\\r`, and a backslash before a character that is not a letter or a digit. Or a JSON path after `body json` or `mask body json` is not made of `.key` and `[0]` steps. The column points at the character where reading stops.",
            ),
            fix: tr!(
                "パターンは、文字そのもの、`.`、`[a-z]` や `[^0-9]` のような文字クラス、`\\d`・`\\w`・`\\s` とその大文字、グループ、`|`、`*`・`+`・`?`、回数で書いてください。主張のファイルでは、パターンのバックスラッシュはそのまま書けます（`matches \"\\d+\"`）。パターンで意味を持つ文字そのものは、前にバックスラッシュを置いて書いてください（`\\.`、`\\(`）。JSON のパスは `.key`、`.a.b`、`.items[0].name` のように書いてください。",
                "Write the pattern with literals, `.`, classes such as `[a-z]` and `[^0-9]`, `\\d`, `\\w`, `\\s` and their capitals, groups, `|`, `*`, `+`, `?` and counts. In a claims file a pattern's backslash is written as it is, `matches \"\\d+\"`, and a character the pattern language uses is kept with one: `\\.`, `\\(`. A JSON path is written `.key`, `.a.b` or `.items[0].name`.",
            ),
            repro: spec(&["check", "e009.geas"], &[("e009.geas", X_E009)], 2),
        },
        Entry {
            code: "E010",
            summary: tr!(
                "語に分けられない、または語のないコマンド。ポートのないターゲットの `{{port}}`",
                "a command that cannot be split into words, or has none; `{{port}}` on a target without a port",
            ),
            when: tr!(
                "geas は `run` と `serve` の文字列を、シェルを通さずに自分で語に分けます。空白で区切り、`'…'` は中をそのまま残し、`\"…\"` は空白を残して中の `\\\"` と `\\\\` をエスケープとして扱い、クォートの外のバックスラッシュは次の文字をそのまま残します。その文字列で開いたクォートが閉じていないとき、バックスラッシュで終わっているとき、語が一つもないときに出ます。ポートを持たない `run` のターゲットが、コマンドや `env` の値で、ポート番号に置き換わる `{{port}}` を使ったときにも出ます。",
                "geas splits a `run` or `serve` string into words itself, never through a shell: blanks separate words, `'…'` keeps what is in it as it is, `\"…\"` keeps blanks and reads `\\\"` and `\\\\`, and outside quotes a backslash keeps the next character. The string opens a quote that nothing closes, ends in a backslash, or holds no word at all. Or a `run` target, which has no port, says `{{port}}`, which stands for the target's port, in its command or in an `env` value.",
            ),
            fix: tr!(
                "クォートを閉じるか、残したい文字の前にバックスラッシュを置いてください。主張のファイルでは文字列の中のダブルクォートを `\\\"` と書くので、シングルクォートのほうが書きやすく、`run \"python3 'my calc.py'\"` のように書けます。シェルを使いたいときは、`run \"sh -c 'cd tools && ./gen'\"` のように、シェルをコマンドとして書いてください。`{{port}}` を使えるのは、`serve` と `port` のあるサービスのターゲットだけです。",
                "Close the quote, or keep the character with a backslash. In a claims file a double quote inside a string is written `\\\"`, so single quotes are the easy form: `run \"python3 'my calc.py'\"`. A shell, when one is wanted, is named: `run \"sh -c 'cd tools && ./gen'\"`. `{{port}}` belongs to a service: a target with `serve` and `port`.",
            ),
            repro: spec(&["check", "e010.geas"], &[("e010.geas", X_E010)], 2),
        },
        Entry {
            code: "E011",
            summary: tr!(
                "そのターゲットに使えない固定、または geas の読めない時刻・ロケール・乱数のシード・環境変数の名前",
                "a pin geas cannot keep for this target, or a time, a locale, a seed or a variable's name it cannot read",
            ),
            when: tr!(
                "geas がプロセスを起動するターゲット（pixie のアプリも含みます）で、`clock` や `seed` に `env \"NAME\"` がないとき。プログラムの時刻や乱数を外から設定する方法はないので、geas はプログラムが読む環境変数で渡します。固定には、その環境変数の名前が要ります。主張がページとして開くサービスでは、ページの時刻と乱数を geas が自分で固定するので、環境変数がなくてもかまいません。すべての固定を受け取るドライバーも同じです。また、geas の読めない値を固定したとき（RFC 3339 の形でない `clock`、言語と地域の形（`ja-JP`）でない `locale`、空の `tz`、0 以上の整数でない `seed`、空か `=` を含む環境変数の名前）にも出ます。主張の実行中に、ドライバーが固定にエラーを返したときや、Chrome が知らない `tz` や `locale` を受け付けなかったときにも出ます。",
                "`clock` or `seed` without `env \"NAME\"` on a target whose processes geas starts, a pixie app among them: no switch sets the time or the random numbers of a program from outside, so geas passes them in a variable the program reads, and the pin has to name it. A service a claim opens as a page takes them without one, since geas keeps the page's clock and random numbers itself, and so does a driver, which is handed every pin. Or a pin's value geas cannot read: a `clock` that is not an RFC 3339 time, a `locale` not written as a language and a region (`ja-JP`), an empty `tz`, a `seed` that is not a whole number from 0, a variable's name that is empty or holds `=`. When a claim runs, a driver that answers its pins with an error, and Chrome refusing a `tz` or a `locale` it does not know, give E011 too.",
            ),
            fix: tr!(
                "プログラムが読む環境変数の名前を、同じ行に書いてください（`clock \"2026-08-29T09:00:00+09:00\" env \"NOW\"`、`seed 7 env \"SEED\"`）。プログラムは時刻やシードを、その環境変数から受け取るようにしてください。時刻は `2026-08-29T09:00:00+09:00` のように、ロケールは `ja-JP` のように書いてください。",
                "Name the variable the program reads, on the same line, `clock \"2026-08-29T09:00:00+09:00\" env \"NOW\"` or `seed 7 env \"SEED\"`, and have the program take the time or the seed from there. Write a time as `2026-08-29T09:00:00+09:00` and a locale as `ja-JP`.",
            ),
            repro: spec(&["check", "e011.geas"], &[("e011.geas", X_E011)], 2),
        },
        Entry {
            code: "E012",
            summary: tr!(
                "pixie のアプリへの二つの操作のあいだにある、ほかのターゲットの `when`",
                "another target's `when` between two actions on a pixie app",
            ),
            when: tr!(
                "geas は、主張の中の pixie のアプリへの操作を、まとめて一つのスクリプトとして実行します。pixie はステップを、ウインドウを開かずに一つのプロセスで再生し、geas はその出力から操作ごとの画面を読みます。そのため、アプリへの最初の操作と最後の操作のあいだでは、ほかのターゲットの `when` を実行できません。そのあいだ、アプリは動いていないからです。最初の操作の前と最後の操作のあとなら書けます。",
                "A claim's actions on a pixie app run as one script: pixie replays the steps headless in one process, and geas reads the screen after each from what it prints. So between the first and the last action on a pixie app, no `when` on another target can run: the app is not running between them. Before the first and after the last is fine.",
            ),
            fix: tr!(
                "ほかのターゲットの `when` を、アプリへの最初の操作の前か最後の操作のあとに移すか、主張を二つに分けてください。",
                "Move the other target's `when` before the first action on the app or after the last, or split the claim in two.",
            ),
            repro: spec(&["check", "e012.geas"], &[("e012.geas", X_E012)], 2),
        },
        Entry {
            code: "E013",
            summary: tr!("その場所の操作が受け付けない引数", "an argument an action cannot take there"),
            when: tr!(
                "操作に、そのターゲットでは受け付けない引数を渡したときです。テキストフィールドを何番目かでしか選べない pixie のアプリへの `into:`、1 から数える `nth:` と `field:` に渡した 0、`advance(0)`、パスのない pixie のアプリへの `open(\"/path\")`、アプリが主張の中で一度だけ起動するのにほかの操作のあとに書いた `open()`、`/` で始まらないページのパス、主張の中でまだ開いていないサービスのページへの操作、geas がページに送れないキーが、これに当たります。",
                "An action is given what it cannot take on its target: `into:` on a pixie app, which reaches a text field by its place only; `nth: 0` or `field: 0`, which count from 1; `advance(0)`; `open(\"/path\")` on a pixie app, which has no paths, or `open()` after another action on it, since the app starts once in a claim; a page's path without its leading `/`; an action on a service's page before the claim opens it; a key geas cannot send to a page.",
            ),
            fix: tr!(
                "メッセージが示す形に直してください。pixie では `field: n` を使い、数は 1 から数え、パスは `/` から書き、ページへのほかの操作の前に `when <ターゲット>.open(\"/\")` を書いてください。キーは `enter`、`tab`、`a`、`cmd-s` のように書いてください。",
                "Write what the message says the action takes there: `field: n` on pixie, counts from 1, a path with its `/`, `when <target>.open(\"/\")` before the other actions on a page, a key such as `enter`, `tab`, `a` or `cmd-s`.",
            ),
            repro: spec(&["check", "e013.geas"], &[("e013.geas", X_E013)], 2),
        },
        Entry {
            code: "E030",
            summary: tr!("起動できないプログラム", "a program that could not be started"),
            when: tr!(
                "`run` や `serve` のコマンドの最初の語が、geas の起動できるプログラムではないとき。PATH に見つからないか、実行できるファイルではないかです。主張はエラーで終わり、残りのステップは実行しません。",
                "The first word of a `run` or `serve` command is not a program geas can start: it is not on `PATH`, or it is not a file the system can run. The claim ends as an error, and its remaining steps do not run.",
            ),
            fix: tr!(
                "プログラムを入れるか PATH に置くか、コマンドを直してください。`コマンド:` の行に、geas が起動しようとした語が出ます。",
                "Install the program or put it on `PATH`, or correct the command. The `command:` note shows the words geas tried to start.",
            ),
            repro: spec(&["check", "e030.geas"], &[("e030.geas", X_E030)], 1),
        },
        Entry {
            code: "E031",
            summary: tr!("5 秒のうちに終わらなかった `run`", "a `run` that did not finish within 5 s"),
            when: tr!(
                "`run` のコマンドが起動から 5 秒たっても終わらなかったので、geas が止め、主張がエラーで終わったとき。`serve` で起動するはずのサービスを `run` で起動したときにも起きます。",
                "A `run` command was still running 5 s after it started, so geas stopped it and the claim ended as an error. A service started with `run` instead of `serve` does this.",
            ),
            fix: tr!(
                "動き続けるプログラムは `serve` のターゲットにするか、コマンドがもっと早く終わるようにしてください。",
                "Declare a program that keeps running as a `serve` target, or make the command finish sooner.",
            ),
            repro: spec(&["check", "e031.geas"], &[("e031.geas", X_E031)], 1),
        },
        Entry {
            code: "E032",
            summary: tr!(
                "ポートを開く前に終了したか、5 秒のうちにポートを開かなかったサービス",
                "a service that exited before opening its port, or did not open it within 5 s",
            ),
            when: tr!(
                "geas が `serve` のターゲットを起動し、127.0.0.1 のポートが開くのを待つあいだに、サービスが終了したか、5 秒たってもポートが開かなかったとき。`=` の行に、コマンドと stderr の最後の数行が出ます。",
                "geas started a `serve` target and waited for its port to open on 127.0.0.1: the service exited first, or the port was still closed after 5 s. The notes give the command and the last lines of its stderr.",
            ),
            fix: tr!(
                "サービスが 127.0.0.1 の、`port` に書いた番号のポートで待ち受けるようにしてください。止まった理由は、たいてい stderr に出ています。",
                "Make the service listen on 127.0.0.1 at the target's `port`; its stderr usually says why it stopped.",
            ),
            repro: spec(&["check", "e032.geas"], &[("e032.geas", X_E032)], 1),
        },
        Entry {
            code: "E033",
            summary: tr!(
                "失敗した HTTP のやりとり（接続の拒否、途中で閉じられた接続、HTTP でないレスポンス、5 秒のうちにレスポンスがない）",
                "an HTTP exchange that failed: refused, cut off, malformed, or no answer within 5 s",
            ),
            when: tr!(
                "`get` や `post` がサービスのポートに届いたのに、HTTP のレスポンスを得られなかったとき。接続を拒否されたとき、サービスがレスポンスを返さずに接続を閉じたとき、返ってきたものが HTTP ではなかったとき、5 秒のうちに何も返ってこなかったときです。`=` の行に、サービスの stderr の最後の数行が出ます。",
                "A `get` or a `post` reached the service's port and got no HTTP answer: the connection was refused, the service closed it without answering, what came back was not HTTP, or nothing came back within 5 s. The notes give the last lines of the service's stderr.",
            ),
            fix: tr!(
                "サービスがどのリクエストにも HTTP のレスポンスを返すようにしてください。リクエストの処理で失敗したサービスは、接続を閉じることがよくあります。理由は stderr に出ています。",
                "Make the service answer every request with an HTTP response. A service that fails on a request often closes the connection; its stderr says why.",
            ),
            repro: Repro {
                args: &["check", "e033.geas"],
                files: &[("e033.geas", X_E033), ("hangup.py", HANGUP_PY)],
                exit: 1,
                needs: &["python3"],
                env: &[],
            },
        },
        Entry {
            code: "E034",
            summary: tr!(
                "Chrome が見つからない、起動しない、またはポートで応答しない",
                "Chrome not found, not started, or not answering on its port",
            ),
            when: tr!(
                "主張がサービスのページを操作しようとしたのに、Chrome を使えないときです（geas はページをヘッドレスの Chrome で開きます）。GEAS_CHROME がファイルでないものを指しているとき、Chrome が見つからないとき（macOS のアプリケーション、PATH の google-chrome・chromium・chromium-browser の順に探します）、Chrome が終了したか 15 秒のうちにデバッグ用のポートを開かなかったとき、応答しなくなったときに出ます。Chrome が要る主張は、どれもエラーで終わります。",
                "A claim acts on a service's page, which geas opens in headless Chrome, and Chrome cannot be had: `GEAS_CHROME` names something that is not a file; no Chrome is found (the macOS application, then `google-chrome`, `chromium` or `chromium-browser` on PATH); Chrome exits, or does not open its debugging port within 15 s; or it stops answering. Each claim that needs Chrome ends as an error.",
            ),
            fix: tr!(
                "Chrome か Chromium を入れるか、起動するプログラムを GEAS_CHROME に設定してください（macOS では `/Applications/Google Chrome.app/Contents/MacOS/Google Chrome`）。",
                "Install Chrome or Chromium, or set GEAS_CHROME to the program to start (on macOS, `/Applications/Google Chrome.app/Contents/MacOS/Google Chrome`).",
            ),
            repro: Repro {
                args: &["check", "e034.geas"],
                files: &[("e034.geas", X_E034)],
                exit: 1,
                needs: &["python3"],
                env: &[("GEAS_CHROME", "no-such-chrome")],
            },
        },
        Entry {
            code: "E035",
            summary: tr!("アプリが拒否した操作", "an action the app refused"),
            when: tr!(
                "`when` が求めた操作を、アプリができなかったときです。その名前のクリックできるものがない、そのテキストフィールドがない、どこも受け取らないキーを押した、といった場合です。pixie はそのステップで終了コード 101 で終わり、ドライバーはエラーを返し、Chrome のページにはそのロールと名前のノードがありません。`=` の行に、そのときの画面と、その画面で操作できるものが出ます。",
                "The app could not do what a `when` asked: nothing of that name to click, no such text field, a key nothing takes. pixie exits with 101 at such a step, a driver answers with an error, and a page in Chrome has no node of that role and name. The notes give the screen the app was on, and what on it the action could have reached.",
            ),
            fix: tr!(
                "`=` の行にあるとおり、画面にあるものをロールと名前で書いてください。名前は、スクリーンリーダーが読み上げるものです。名前で指せないコントロールは、スクリーンリーダーを使う人にも見つけられません。アプリの側で名前を付けてください（ラベル、`aria-label`）。",
                "Name what the screen has, as the notes list it, by its role and its name, which is what a screen reader reads out. A control a claim cannot reach by its name is one a person using a screen reader cannot reach either: give it a name in the app (a label, `aria-label`).",
            ),
            repro: Repro {
                args: &["check", "e035.geas"],
                files: &[("e035.geas", X_E035), ("driver.py", SAVE_DRIVER_PY)],
                exit: 1,
                needs: &["python3"],
                env: &[],
            },
        },
        Entry {
            code: "E036",
            summary: tr!("読み込まれなかったか、10 秒のうちに落ち着かなかったページ", "a page that did not load, or did not settle within 10 s"),
            when: tr!(
                "geas はページを読み込み、操作のたびにページの時刻を 1 秒進めます。この時刻は、ページのリクエストが終わるまで止まります。ページが読み込まれなかったとき（Chrome がエラーを返したか、サービスが 10 秒のうちにレスポンスを返さなかったとき）や、その 1 秒が実際の 10 秒のうちに過ぎなかったとき（ページが、レスポンスの返らないリクエストを待っているときなど）に出ます。",
                "geas loads a page and, after each action, lets the page's clock run one second of the page's own time; the clock waits while a request of the page is pending. The page did not load (Chrome reported an error, or the service did not answer the request within 10 s), or that second did not run out within 10 s of real time, as when the page waits on a request the service never answers.",
            ),
            fix: tr!(
                "サービスが、ページのどのリクエストにもすぐレスポンスを返すようにしてください。レスポンスの返らないリクエストがあると、ページの時刻は進みません。",
                "Make the service answer every request of the page, and soon; a request it never answers holds the page's clock.",
            ),
            repro: Repro {
                args: &["check", "e036.geas"],
                files: &[("e036.geas", X_E036), ("broken.py", BROKEN_PY)],
                exit: 1,
                needs: &["python3", "chrome"],
                env: &[],
            },
        },
        Entry {
            code: "E037",
            summary: tr!("geas が読めない、GUI のドライバーの出力", "output of a GUI driver geas cannot read"),
            when: tr!(
                "ドライバーが、JSON でない行やプロトコルの形でない行を返したとき、5 秒のうちに応答しなかったとき、終了したときです。また、pixie の出力に読めないアクセシビリティツリーがあったとき、操作ごとに一つのツリーがなかったとき、アプリがスクリプトを終えなかったとき、終了コード 0 と 101 以外で終えたときにも出ます。",
                "A driver answered with a line that is not JSON, or not of the protocol's shape; did not answer within 5 s; or exited. Or pixie's output held an accessibility tree that reads no way, or not one tree per action; or the app did not finish its script, or ended it with an exit code other than 0 and 101.",
            ),
            fix: tr!(
                "ドライバーが、固定を渡す最初の行には `{{\"ok\":true}}` を、操作ごとに一行の `{{\"screen\":…}}` か `{{\"error\":\"…\",\"screen\":…}}` を、5 秒のうちに返すようにしてください。pixie なら、`=` の行のスクリプトを `PIXIE_SCRIPT` に入れて、アプリを手で走らせてみてください。",
                "A driver answers the first line, which hands it the pins, with `{{\"ok\":true}}`, and each action with one line, `{{\"screen\":…}}` or `{{\"error\":\"…\",\"screen\":…}}`, within 5 s. For pixie, run the app by hand with the script the notes give, as `PIXIE_SCRIPT`.",
            ),
            repro: Repro {
                args: &["check", "e037.geas"],
                files: &[("e037.geas", X_E037), ("driver.py", CHATTY_DRIVER_PY)],
                exit: 1,
                needs: &["python3"],
                env: &[],
            },
        },
        Entry {
            code: "E050",
            summary: tr!("ベースラインがない（先に `geas snap` を）", "no baseline: run `geas snap` first"),
            when: tr!(
                "`geas drift` は、`geas snap` が主張のファイルの隣に残したベースライン（`.geas/<名前>.baseline.jsonl`。`<名前>` は主張のファイルの名前から `.geas` を除いたもの）と今回の実行を比べます。それがないときです。",
                "`geas drift` compares a run with the baseline `geas snap` kept beside the spec, in `.geas/<name>.baseline.jsonl` (`<name>` being the spec's file name without `.geas`), and there is none.",
            ),
            fix: tr!(
                "受け入れてよい振る舞いになったときに `geas snap <主張のファイル>` を走らせ、変更のたびに `geas drift <主張のファイル>` を走らせてください。",
                "Run `geas snap <spec>` when the behavior is what you accept, then `geas drift <spec>` after each change.",
            ),
            repro: spec(&["drift", "e050.geas"], &[("e050.geas", X_DRIFT)], 2),
        },
        Entry {
            code: "E051",
            summary: tr!("読めないベースライン", "a baseline that cannot be read"),
            when: tr!(
                "主張のファイルのベースライン（`.geas/<名前>.baseline.jsonl`）のある行が JSON ではないか、`geas snap` が書くもの（`claim`、`idx`、`call`、`obs`）が欠けているとき。",
                "A line of the spec's baseline (`.geas/<name>.baseline.jsonl`) is not JSON, or lacks what `geas snap` writes there: `claim`, `idx`, `call` and `obs`.",
            ),
            fix: tr!(
                "ベースラインは手で書かず、`geas snap` に書かせてください。もう一度走らせて書き直してください。",
                "The baseline is written by `geas snap`, not by hand: run it again to write a new one.",
            ),
            repro: spec(
                &["drift", "e051.geas"],
                &[("e051.geas", X_DRIFT), (".geas/e051.baseline.jsonl", "this is not a baseline\n")],
                2,
            ),
        },
        Entry {
            code: "E060",
            summary: tr!("記録がない（先に `geas map` を）", "no record: run `geas map` first"),
            when: tr!(
                "`geas affected` は、`geas map` が主張のファイルの隣に書いた記録（`.geas/<名前>.map.jsonl`）か、`--map` で渡した記録を読みます。それがないときです。",
                "`geas affected` reads the record `geas map` wrote beside the spec, in `.geas/<name>.map.jsonl`, or the records `--map` names, and there is none.",
            ),
            fix: tr!(
                "変更前か変更後のコードで `geas map <主張のファイル>` を走らせてから、もう一度 `geas affected` を走らせてください。",
                "Run `geas map <spec>` on the code before the change or after it, then `geas affected` again.",
            ),
            repro: spec(
                &["affected", "e060.geas", "change.diff", "--root", "."],
                &[("e060.geas", X_ADDS), ("calc.py", CALC_AFTER), ("change.diff", CALC_CHANGE)],
                2,
            ),
        },
        Entry {
            code: "E061",
            summary: tr!(
                "読めない記録、形式の違う記録、別の主張のファイルの記録",
                "a record that cannot be read, of another format version, or of another spec",
            ),
            when: tr!(
                "記録のある行が `geas map` の書くものになっていないとき、この geas の読めない形式の記録のとき、別の主張のファイルの記録のとき（記録の `spec` が、ルートから見た別のファイルを指している。`map` に別の `--root` を渡したときに起きます）です。",
                "A line of the record is not what `geas map` writes; or the record is of a format this geas does not read; or it was made for another spec: its `spec` names another file relative to the root, as when `map` was given another `--root`.",
            ),
            fix: tr!(
                "記録は手で書かず、`geas map` に書かせてください。`affected` と同じ `--root` で、この主張のファイルについて走らせ直してください。",
                "A record is written by `geas map`, not by hand: run it again on this spec, with the `--root` that `affected` is given.",
            ),
            repro: spec(
                &["affected", "e061.geas", "change.diff", "--root", "."],
                &[
                    ("e061.geas", X_ADDS),
                    ("calc.py", CALC_AFTER),
                    ("change.diff", CALC_CHANGE),
                    (".geas/e061.map.jsonl", record_of_before!("other")),
                ],
                2,
            ),
        },
        Entry {
            code: "E062",
            summary: tr!(
                "古い記録（ディスクか差分にあるファイルのハッシュが、記録と違う）",
                "a stale record: a file's hash on disk or in the diff is not the recorded one",
            ),
            when: tr!(
                "geas は記録を、それを取ったときのコードとファイルごとに突き合わせます。使うのは git の blob ハッシュで、geas が自分で計算します。差分が触れていないソースファイルは、ディスクでも記録と同じハッシュでなければならず、差分が触れているファイルは、記録のハッシュが変更前か変更後のどちらかでなければなりません。記録のあとに足したのに差分にないソースファイルも、これに当たります。`=` の行に、違っているファイルが出ます。",
                "A record is held to the code it was made on, file by file, by git's blob hashes, which geas computes itself: a source file the diff does not touch has to have its recorded hash on disk, and one it touches has to have, in the record, the hash of the code before the change or after it. A source file added since the record and missing from the diff counts too. The notes name the files that differ.",
            ),
            fix: tr!(
                "変更前か変更後のコードで、`geas map <主張のファイル>` を走らせて記録を取り直してください。新しいファイルは差分に入れておいてください。`git add -N <ファイル>` とすると、`git diff` に出るようになります。",
                "Record again with `geas map <spec>`, on the code before the change or after it. A new file has to be in the diff: `git add -N <file>` makes `git diff` show it.",
            ),
            repro: spec(
                &["affected", "e062.geas", "change.diff", "--root", "."],
                &[
                    ("e062.geas", X_ADDS),
                    ("calc.py", CALC_AFTER),
                    ("README.md", "A calculator.\nIt adds.\n"),
                    ("change.diff", README_CHANGE),
                    (".geas/e062.map.jsonl", record_of_before!("e062")),
                ],
                2,
            ),
        },
        Entry {
            code: "E063",
            summary: tr!(
                "変更前のコードの記録しかないのに、差分が行を足している",
                "added lines with only a record of the code before the change",
            ),
            when: tr!(
                "geas は、足した行や書き換えた行を、変更後のコードの記録で探します。行番号が変更後のコードのものだからです。渡した記録が変更前のコードのものしかないときに出ます。",
                "An added or rewritten line is looked up in a record of the code after the change, since its number is a line of that code, and the only record given is of the code before it.",
            ),
            fix: tr!(
                "変更後のコードで `geas map <主張のファイル> --out <after.jsonl>` を走らせ、`--map <変更前の記録> --map <after.jsonl>` の形で両方の記録を渡してください。",
                "Run `geas map <spec> --out <after.jsonl>` on the changed code, and give both records: `--map <before> --map <after.jsonl>`.",
            ),
            repro: spec(
                &["affected", "e063.geas", "change.diff", "--root", "."],
                &[
                    ("e063.geas", X_ADDS),
                    ("calc.py", CALC_AFTER),
                    ("change.diff", CALC_CHANGE),
                    (".geas/e063.map.jsonl", record_of_before!("e063")),
                ],
                2,
            ),
        },
        Entry {
            code: "E064",
            summary: tr!(
                "読めない差分、またはディスクのファイルの前にも後にも合わない差分",
                "a diff that cannot be read, or that fits neither side of the file on disk",
            ),
            when: tr!(
                "差分が unified 形式（`git diff`、`diff -u`）になっていないときや、ハンクの行がヘッダーの数と合わないとき。または `index` の行のない差分（`diff -u` が書くもの）がディスクのファイルに合わないときです。そうした差分を geas はディスクのファイルと突き合わせるので、ディスクのファイルは変更後か変更前のコードでなければなりません。",
                "The diff is not a unified diff (`git diff`, `diff -u`), or a hunk does not hold the lines its header counts. Or a diff without `index` lines, as `diff -u` writes it, does not fit the file on disk: geas holds such a diff to the disk, which has to be the code after the change or before it.",
            ),
            fix: tr!(
                "ディスクにあるコードの差分を、`git diff` か `diff -u` の形で渡してください。`git diff` なら両側の blob が書かれているので、ディスクのファイルは要りません。",
                "Give the diff of the code on disk, as `git diff` or `diff -u` writes it; `git diff` names both sides' blobs and needs no file on disk.",
            ),
            repro: spec(
                &["affected", "e064.geas", "change.diff", "--root", "."],
                &[
                    ("e064.geas", X_ADDS),
                    ("calc.py", CALC_AFTER),
                    ("change.diff", ELSEWHERE_CHANGE),
                    (".geas/e064.map.jsonl", record_of_before!("e064")),
                ],
                2,
            ),
        },
        Entry {
            code: "E065",
            summary: tr!(
                "カバレッジを変換するツールがないか、失敗した（`go tool covdata`、`llvm-profdata`、`llvm-cov`）",
                "a coverage converter missing or failing (`go tool covdata`, `llvm-profdata`, `llvm-cov`)",
            ),
            when: tr!(
                "`geas map` で、`-cover` を付けてビルドした Go のプログラムや `-C instrument-coverage` を付けてビルドした Rust のプログラムがカバレッジを書いたのに、それを行に直すツールがないか、失敗したときです。Go なら `go`、Rust なら `llvm-profdata` と `llvm-cov` で、`GEAS_LLVM_BIN` が設定されていればそこから、なければ Rust のツールチェーンの sysroot から、それもなければ PATH から探します。読めないカバレッジのファイルも、このコードで知らせます。このとき `map` は記録を書きません。",
                "In `geas map`, a Go program built with `-cover` or a Rust program built with `-C instrument-coverage` wrote its coverage, and the tool that turns it into lines is missing or failed: `go` for Go; `llvm-profdata` and `llvm-cov` for Rust, from `GEAS_LLVM_BIN` when it is set, else from the Rust toolchain's sysroot, else from PATH. A coverage file that does not read is reported the same way. `map` writes no record then.",
            ),
            fix: tr!(
                "ツールを入れてください。Go なら Go のツールチェーン、Rust なら rustup の llvm-tools コンポーネント（`rustup component add llvm-tools`）です。llvm-profdata と llvm-cov がほかの場所にあるなら、そのディレクトリを GEAS_LLVM_BIN に設定してください。",
                "Install the tool: Go's own toolchain for Go; rustup's llvm-tools component for Rust (`rustup component add llvm-tools`), or set GEAS_LLVM_BIN to a directory that holds llvm-profdata and llvm-cov.",
            ),
            repro: Repro {
                args: &["map", "e065.geas", "--root", "."],
                files: &[("e065.geas", X_E065), ("fake.py", FAKE_PROFILE_PY), ("empty/.keep", "")],
                exit: 2,
                needs: &["python3"],
                env: &[("GEAS_LLVM_BIN", "empty")],
            },
        },
        Entry {
            code: "E066",
            summary: tr!("記録を書かずに止まったサービス", "a service that stopped without writing its record"),
            when: tr!(
                "`geas map` は主張が終わるたびにサービスへ SIGTERM を送り、5 秒のうちに終了するのを待ちます。ランタイムが実行した行を書くのは、プログラムが終了するときだからです。5 秒たっても終わらず強制終了したサービスや、カバレッジのメタデータだけ書いてカウンターを書かなかった Go のプログラムは、行を書かずに止まったかもしれず、記録を信用できません。このとき `map` は記録を書きません。",
                "In `geas map`, a service is asked to stop with SIGTERM at the end of each claim and given 5 s to exit, since the runtimes write what ran when a program exits. One that had to be killed after the 5 s, or a Go program that wrote its coverage metadata and no counters, may have stopped without writing its lines, so the record cannot be trusted. `map` writes no record then.",
            ),
            fix: tr!(
                "サービスが SIGTERM で終了するようにしてください。Python と Node は、プログラムがシグナルを無視していなければ geas のフックで終了します。Go のサービスは main から戻るようにし（signal.NotifyContext と Server.Shutdown）、Rust のサービスには SIGTERM を処理するコードを足してください。",
                "Let the service exit on SIGTERM: Python and Node do, through geas's hooks, unless the program ignores the signal; a Go service returns from main (signal.NotifyContext and Server.Shutdown); a Rust service needs a handler of its own.",
            ),
            repro: Repro {
                args: &["map", "e066.geas", "--root", "."],
                files: &[("e066.geas", X_E066), ("stubborn.py", STUBBORN_PY)],
                exit: 2,
                needs: &["python3"],
                env: &[],
            },
        },
        Entry {
            code: "E080",
            summary: tr!("コマンドが受け付けない引数", "arguments the command does not take"),
            when: tr!(
                "geas をコマンドなしか、geas にないコマンドで走らせたとき、コマンドが受け付けないオプションや値のないオプションを渡したとき、`--jobs` に 1 以上の整数以外を渡したとき、コマンドに要る主張のファイル（`explain` ならコード）を渡さなかったとき、`skill` にファイルを渡したとき、`--install` なしで `--force` を渡したときです。",
                "geas was run with no command or an unknown one, with an option the command does not take or an option without its value, with `--jobs` given anything but a whole number from 1, without the specs (or, for `explain`, the codes) the command needs, or with a file given to `skill`, or `--force` without `--install`.",
            ),
            fix: tr!(
                "`geas --help` に、コマンドとオプションの一覧があります。",
                "`geas --help` lists the commands and their options.",
            ),
            repro: spec(&["check"], &[], 2),
        },
        Entry {
            code: "E081",
            summary: tr!("読めないか、書けないファイル", "a file that cannot be read or written"),
            when: tr!(
                "geas が主張のファイルを読めなかったか、その隣の `.geas/` にジャーナルやベースラインを書けなかったときです。`geas scenarios` の `--openspec` に渡したものが無いときや読めないとき、`geas skill --install <dir>` がスキルのフォルダーを書けなかったときと、`--force` を付けずに、すでにある `<dir>/geas` に書こうとしたときにも出ます。メッセージに OS が返した理由が出ます。",
                "geas could not read a spec, or could not write the journal or the baseline in the `.geas/` directory beside it; or what `geas scenarios` was given with `--openspec` is not there or cannot be read; or `geas skill --install <dir>` could not write the skill folder, or found `<dir>/geas` already there and was not given `--force`. The message carries the system's reason.",
            ),
            fix: tr!(
                "パスと、主張のファイルのディレクトリに書き込めるかを確かめてください。`geas skill --install` では、`--force` を付けると、そこにあるフォルダーにスキルを上書きします。",
                "Check the path, and that the spec's directory can be written to. For `geas skill --install`, add `--force` to write the skill over the folder that is there.",
            ),
            repro: spec(&["check", "missing.geas"], &[], 2),
        },
        Entry {
            code: "E090",
            summary: tr!("OpenSpec の仕様として読めないファイル", "a file that does not read as an OpenSpec spec"),
            when: tr!(
                "`geas scenarios` の `--openspec` に渡したファイルが、仕様（`## Requirements` の節）でも変更の提案の差分（`## ADDED Requirements` などの節）でもないとき、同じ名前の要件を二つ持つとき、UTF-8 でないときです。渡したディレクトリの下に `spec.md` が一つも無いときにも出ます。",
                "In `geas scenarios`, a file given to `--openspec` is neither a spec (a `## Requirements` section) nor a change's delta spec (a section such as `## ADDED Requirements`), holds two requirements of one name, or is not UTF-8; or a directory given holds no `spec.md`.",
            ),
            fix: tr!(
                "仕様（`openspec/specs/<capability>/spec.md`）、変更の提案の差分（`openspec/changes/<id>/specs/<capability>/spec.md`）、それらを持つディレクトリを渡してください。形の誤りは `openspec validate` が言います。",
                "Give a spec (`openspec/specs/<capability>/spec.md`), a change's delta spec (`openspec/changes/<id>/specs/<capability>/spec.md`), or a directory that holds them; `openspec validate` says what is wrong with a form.",
            ),
            repro: spec(&["scenarios", "e090.geas", "--openspec", "notes.md"], &[("e090.geas", X_E090), ("notes.md", NOTES_MD)], 2),
        },
        Entry {
            code: "W060",
            summary: tr!("`map` で記録が何も取れなかったターゲット", "a target that gave no record at all in `map`"),
            when: tr!(
                "`geas map` で、ターゲットが起動したどのプロセスからも、ルートの下のファイルの行が報告されなかったときです。geas が記録できるのは、Python 3.12 以降、Node、`-cover` を付けてビルドした Go のプログラム、`-C instrument-coverage` を付けてビルドした Rust のプログラムです。シェルスクリプト、古い Python、カバレッジなしでビルドしたプログラム、ルートの外のプログラムからは何も取れません。geas は、そのターゲットの行が無いまま記録を書きます。",
                "In `geas map`, no process a target started reported a line of a file under the root. geas records Python 3.12 and later, Node, a Go program built with `-cover` and a Rust program built with `-C instrument-coverage`; a shell script, an older Python, a program built without coverage, or a program outside the root gives nothing. The record is still written, without that target's lines.",
            ),
            fix: tr!(
                "`map` のためにカバレッジ付きでビルドするか（`go build -cover`、`rustc -C instrument-coverage`）、Python 3.12 以降で走らせてください。プロジェクトのコードではないターゲットなら、そのままでかまいません。この警告が言っているのは、そのターゲットの変更を `affected` がどの主張も通らないコードとして扱う、ということだけです。",
                "Build the program with coverage for `map` (`go build -cover`, `rustc -C instrument-coverage`), run Python 3.12 or later, or leave the target as it is when its code is not the project's: the warning says only that `affected` will call its changes unclaimed.",
            ),
            repro: spec(&["map", "w060.geas", "--root", "."], &[("w060.geas", X_W060)], 0),
        },
        Entry {
            code: "W061",
            summary: tr!(
                "geas が直接起動していないプログラムが書いた Rust のプロファイル",
                "a Rust profile from a program geas did not start itself",
            ),
            when: tr!(
                "`geas map` で、`-C instrument-coverage` を付けてビルドした Rust のプログラムがプロファイルを書いたのに、それが geas の起動したプログラムではなかったときです（スクリプトや `cargo run` が起動した）。`llvm-cov` がプロファイルを行に直すには、それを書いたプログラムが要ります。geas が知っているのは自分で起動したプログラムだけなので、そのプロファイルは記録に入れません。",
                "In `geas map`, a Rust program built with `-C instrument-coverage` wrote a profile, and it is not a program geas started: a script or `cargo run` started it. `llvm-cov` turns a profile into lines only with the program that wrote it, and geas knows only the programs it starts, so that profile is left out of the record.",
            ),
            fix: tr!(
                "ターゲットのコマンドで、ビルドしたプログラムを直接起動してください。`run \"cargo run\"` やスクリプトではなく、`run \"./tally\"` のように書いてください。",
                "Make the target's command start the built program itself: `run \"./tally\"`, not `run \"cargo run\"` or a script.",
            ),
            repro: Repro {
                args: &["map", "w061.geas", "--root", "."],
                files: &[("w061.geas", X_W061), ("tally.sh", TALLY_SH), ("tally.rs", TALLY_RS)],
                exit: 0,
                needs: &["rustc", "llvm-tools"],
                env: &[],
            },
        },
        Entry {
            code: "W901",
            summary: tr!("ファイルに書かれた鍵", "a key written in the file"),
            when: tr!(
                "`geas check` で、仕様のどこか（文字列でもコメントでも）に鍵の形の値があったときです。調べる鍵は、AWS のアクセスキー ID、GitHub・Slack・Stripe・OpenAI・Anthropic・Google の鍵やトークン、Slack の Incoming Webhook の URL、PEM の秘密鍵で、どれもプロバイダーが接頭辞や形を決めているものです。ritsu のどの言語も同じ決まりで調べます。診断には鍵の種類と、接頭辞と、長さだけを出し、鍵そのものも、その行も出しません。主張はいつもどおり走ります。",
                "In `geas check`, somewhere in the spec, in a string or a comment alike, there is a value in the shape of a key: an AWS access key ID, a key or token of GitHub, Slack, Stripe, OpenAI, Anthropic or Google, a Slack incoming webhook URL, or a PEM private key, each a shape its provider fixes. Every language of ritsu looks for them the same way. The diagnostic gives the kind of key, its prefix and its length, and never the key nor its line. The claims run as they always do.",
            ),
            fix: tr!(
                "鍵はコードが動くところ（環境変数、プラットフォームの接続やシークレットの置き場）に置き、ターゲットのプログラムがそこから読むようにしてください。本物の鍵なら、まずプロバイダーで無効にしてください。ファイルから消しても、リポジトリの履歴には残ります。テスト用の値なら、同じ行のコメントに `ritsu: test secret` と書いてください。",
                "Keep the key where the code runs (an environment variable, the platform's connection or secret store), and have the target's program read it from there. If it is real, revoke it with its provider first: taking it out of the file leaves it in the history of the repository. If it is a value for tests, write `ritsu: test secret` in a comment on the same line.",
            ),
            repro: spec(&["check", "w901.geas"], &[("w901.geas", X_W901)], 0),
        },
    ]
}

/// The entry of a code, written in either case.
pub fn find(code: &str) -> Option<Entry> {
    let up = code.to_ascii_uppercase();
    table().into_iter().find(|e| e.code == up)
}

fn indent(s: &str, by: &str) -> String {
    crate::diag::indent(s, by)
}

/// `geas explain <code>`: what a terminal shows.
pub fn render_text(e: &Entry, lang: Lang) -> String {
    let kind = e.severity().word(lang);
    let mut o = format!("{kind}[{}]: {}\n", e.code, e.summary.get(lang));
    o.push_str(tr!("\nいつ出るか\n", "\nWhen it appears\n").get(lang));
    o.push_str(&indent(e.when.get(lang), "  "));
    o.push_str(tr!("\n直し方\n", "\nWhat usually fixes it\n").get(lang));
    o.push_str(&indent(e.fix.get(lang), "  "));
    let needs = e.repro.needs.join(", ");
    o.push_str(&match (lang, needs.is_empty()) {
        (Lang::En, true) => "\nRepro\n".to_string(),
        (Lang::En, false) => format!("\nRepro (needs {needs})\n"),
        (Lang::Ja, true) => "\n再現\n".to_string(),
        (Lang::Ja, false) => format!("\n再現（{needs} が要ります）\n"),
    });
    let env: String = e.repro.env.iter().map(|(k, v)| format!("{k}={v} ")).collect();
    o.push_str(&format!("  $ {env}geas {}\n", e.repro.args.join(" ")));
    for (name, text) in e.repro.files {
        o.push_str(&format!("  {name}:\n"));
        o.push_str(&indent(text, "    "));
    }
    o
}

/// `geas explain <code> --json`: the same, as data; the repro as files and arguments
/// a program can run.
pub fn render_json(e: &Entry, lang: Lang) -> String {
    let args: Vec<String> = e.repro.args.iter().map(|a| json::quote(a)).collect();
    let files: Vec<String> = e
        .repro
        .files
        .iter()
        .map(|(n, text)| format!("{{\"name\":{},\"text\":{}}}", json::quote(n), json::quote(text)))
        .collect();
    let needs: Vec<String> = e.repro.needs.iter().map(|n| json::quote(n)).collect();
    let env: Vec<String> = e.repro.env.iter().map(|(k, v)| format!("{}:{}", json::quote(k), json::quote(v))).collect();
    format!(
        "{{\"geas\":1,\"code\":\"{}\",\"severity\":\"{}\",\"summary\":{},\"when\":{},\"fix\":{},\"repro\":{{\"args\":[{}],\"files\":[{}],\"exit\":{},\"needs\":[{}],\"env\":{{{}}}}}}}",
        e.code,
        e.severity().key(),
        json::quote(e.summary.get(lang)),
        json::quote(e.when.get(lang)),
        json::quote(e.fix.get(lang)),
        args.join(","),
        files.join(","),
        e.repro.exit,
        needs.join(","),
        env.join(",")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_in_order_and_unique() {
        let codes: Vec<&str> = table().iter().map(|e| e.code).collect();
        let mut sorted = codes.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(codes, sorted);
    }

    #[test]
    fn every_entry_is_written_in_both_languages() {
        for e in table() {
            for text in [&e.summary, &e.when, &e.fix] {
                assert!(!text.en.is_empty() && !text.ja.is_empty(), "{}", e.code);
                assert_ne!(text.en, text.ja, "{}", e.code);
            }
            assert!(!e.repro.args.is_empty(), "{}", e.code);
        }
    }

    #[test]
    fn the_repros_records_and_diffs_hold_their_blobs() {
        use crate::hash::blob;
        assert!(record_of_before!("x").contains(&blob(CALC_BEFORE.as_bytes())));
        let index = format!("index {}..{}", &blob(CALC_BEFORE.as_bytes())[..7], &blob(CALC_AFTER.as_bytes())[..7]);
        assert!(CALC_CHANGE.contains(&index), "{index}");
        let readme = format!("index {}..{}", &blob(b"A calculator.\n")[..7], &blob(b"A calculator.\nIt adds.\n")[..7]);
        assert!(README_CHANGE.contains(&readme), "{readme}");
    }

    #[test]
    fn found_in_either_case() {
        assert_eq!(find("e002").map(|e| e.code), Some("E002"));
        assert!(find("E999").is_none());
    }
}

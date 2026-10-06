//! The ledger of every diagnostic code (DESIGN 5.2). `sakai explain` reads it, and the tests run
//! every entry's reproduction and require its code to come out, so the reproduction cannot go
//! stale while the prose around it still reads well.
//!
//! A reproduction is a directory of files: the files of [`BASE`] (a map of two contexts, each
//! with a `.proto`) with the entry's own files laid over them. `sakai check` on that directory
//! gives the code (`sakai build …` for the codes of the settings). E107's is a map of Rust code
//! of its own ([`RUST`]), which the base has none of. The codes that read another
//! language (E105, E207, E208, E209, E211, E405) come out of `ritsu sakai check`, with every language
//! joined; E104 comes out of the binary of sakai's own crate, which joins none. N101 is retired:
//! its entry stays, and its number is given to nothing else.

use ritsu_base::ledger::{Entry, Ledger, Repro};
use ritsu_base::text::Text;

/// The map every reproduction starts from: 甲 and 乙, 乙 publishing `b.v1`, and no
/// relationship. It passes check.
pub const BASE: &[(&str, &str)] = &[
    ("地図.ctx", "map 地図(m) v1\nuse context \"甲.ctx\"\nuse context \"乙.ctx\"\ncovers \".\"\n"),
    ("甲.ctx", "context 甲(a) v1\nowns\n  dir \"a\"\n"),
    (
        "乙.ctx",
        "context 乙(b) v1\nowns\n  dir \"b\"\n\npublished language b.v1\n  proto \"b/v1/b.proto\"\n  open host service BService\n\nterms\n  種類 \"乙が扱うものの種類\"\n    means enum Kind\n",
    ),
    ("a/a.proto", "syntax = \"proto3\";\npackage a;\nmessage A {}\n"),
    (
        "b/v1/b.proto",
        "syntax = \"proto3\";\npackage b.v1;\nenum Kind {\n  KIND_UNSPECIFIED = 0;\n  KIND_ONE = 1;\n  KIND_TWO = 2;\n}\nmessage B { Kind kind = 1; }\nmessage Plain { string id = 1; }\nservice BService { rpc Get(B) returns (B); }\n",
    ),
];

/// The same map with its names in English: Alpha and Beta, Beta publishing `b.v1`, and no
/// relationship. It passes check too.
pub const BASE_EN: &[(&str, &str)] = &[
    ("map.ctx", "map Map(m) v1\nuse context \"alpha.ctx\"\nuse context \"beta.ctx\"\ncovers \".\"\n"),
    ("alpha.ctx", "context Alpha(a) v1\nowns\n  dir \"a\"\n"),
    (
        "beta.ctx",
        "context Beta(b) v1\nowns\n  dir \"b\"\n\npublished language b.v1\n  proto \"b/v1/b.proto\"\n  open host service BService\n\nterms\n  kind \"The kind of thing Beta deals with\"\n    means enum Kind\n",
    ),
    ("a/a.proto", "syntax = \"proto3\";\npackage a;\nmessage A {}\n"),
    (
        "b/v1/b.proto",
        "syntax = \"proto3\";\npackage b.v1;\nenum Kind {\n  KIND_UNSPECIFIED = 0;\n  KIND_ONE = 1;\n  KIND_TWO = 2;\n}\nmessage B { Kind kind = 1; }\nmessage Plain { string id = 1; }\nservice BService { rpc Get(B) returns (B); }\n",
    ),
];

/// A map of one context whose Rust code has no workspace manifest at its `code rust` place
/// (E107): the reproduction of the one code that reads Rust's crates.
const RUST: &[(&str, &str)] = &[
    ("shop.ctx", "map Shop(shop) v1\nuse context \"orders.ctx\"\ncovers \".\"\ncode rust \".\"\n"),
    ("orders.ctx", "context Orders(orders) v1\nowns\n  dir \"orders\"\n"),
    ("orders/Cargo.toml", "[package]\nname = \"orders\"\nversion = \"0.1.0\"\nedition = \"2021\"\n"),
    ("orders/src/lib.rs", "pub fn reserve() {}\n"),
];

const A_USES_B: (&str, &str) = ("a/a.proto", "syntax = \"proto3\";\npackage a;\nimport \"b/v1/b.proto\";\nmessage A { b.v1.B b = 1; }\n");
const A_USES_PLAIN: (&str, &str) = ("a/a.proto", "syntax = \"proto3\";\npackage a;\nimport \"b/v1/b.proto\";\nmessage A { b.v1.Plain p = 1; }\n");
const A_CONFORMS: (&str, &str) = ("甲.ctx", "context 甲(a) v1\nowns\n  dir \"a\"\n\nupstream 乙 conformist\n  through b.v1\n");
/// The same, with its names in English.
const A_CONFORMS_EN: (&str, &str) = ("alpha.ctx", "context Alpha(a) v1\nowns\n  dir \"a\"\n\nupstream Beta conformist\n  through b.v1\n");
const A_PUBLISHES: (&str, &str) = ("a/v1/a1.proto", "syntax = \"proto3\";\npackage a.v1;\nenum AKind {\n  A_KIND_UNSPECIFIED = 0;\n  A_KIND_X = 1;\n}\nmessage A1 {}\n");
/// A rule of 甲 whose table leaves an amount out: rulec does not answer for it (E105).
const A_RULE_WITH_A_GAP: (&str, &str) = (
    "a/fee.rule",
    "rule fee v1\n\ninputs\n  amount : money[JPY]  range >=0JPY <=10000JPY\n\noutputs\n  fee : money[JPY]  round down(1JPY)\n\ntable fees\n| amount       | -> fee  |\n| <5000JPY     | 500JPY  |\n| >5000JPY     | 0JPY    |\n",
);
/// A rule of 甲 that takes 乙's `Kind` in (`import proto`): `KIND_ONE` as 一, `KIND_TWO` as 二.
const A_RULE_TAKES_KIND: (&str, &str) = (
    "a/x.rule",
    "rule x(x) v1\n\nimport proto \"../b/v1/b.proto\" Kind -> 種類\nenum 種類(kind) = 一(one) | 二(two)\n\ninputs\n  k : 種類\n\noutputs\n  n : 種類\n\ntable t\npolicy unique\n| k  | -> n |\n| 一 | 一   |\n| 二 | 二   |\n",
);
/// The same, with its names in English.
const A_RULE_TAKES_KIND_EN: (&str, &str) = (
    "a/x.rule",
    "rule x v1\n\nimport proto \"../b/v1/b.proto\" Kind -> kind\nenum kind = one | two\n\ninputs\n  k : kind\n\noutputs\n  n : kind\n\ntable t\npolicy unique\n| k   | -> n |\n| one | one  |\n| two | two  |\n",
);
/// A workflow of 甲 that calls 乙's `BService` by Connect.
const A_CALLS_B: (&str, &str) = (
    "a/w.flow",
    "workflow w v1\n\nuse proto b from \"../b/v1/b.proto\"\n  url \"https://b.example.com\"\n\ninputs\n  kind : b.Kind\n\ntask get(kind: b.Kind) -> b.B\n  connect b \"BService/Get\"\n\nflow\n  let r = get(kind: kind)\n  succeed\n",
);

/// An OpenAPI document of 乙's (DESIGN 15) with a tab in its indentation: it does not read (E108).
const B_API_WITH_A_TAB: (&str, &str) = ("b/api.yaml", "openapi: 3.1.0\ninfo:\n\ttitle: B\n  version: 1.0.0\n");
/// An OpenAPI document of 乙's whose schema is a `$ref` to a URL (W104).
const B_API_WITH_A_URL: (&str, &str) = ("b/api.yaml", "openapi: 3.1.0\ninfo:\n  title: B\n  version: 1.0.0\ncomponents:\n  schemas:\n    Price:\n      $ref: 'https://example.com/schemas/money.yaml#/Money'\n");
/// 乙's events, a channel `done` it sends on (AsyncAPI 3.0).
const B_EVENTS: (&str, &str) = (
    "b/events.yaml",
    "asyncapi: 3.0.0\ninfo:\n  title: B events\n  version: 1.0.0\nchannels:\n  done:\n    address: b.done\n    messages:\n      done:\n        payload:\n          type: string\noperations:\n  sendDone:\n    action: send\n    channel:\n      $ref: '#/channels/done'\n",
);
/// 甲's events: it receives on 乙's channel `done`.
const A_RECEIVES_DONE: (&str, &str) = (
    "a/events.yaml",
    "asyncapi: 3.0.0\ninfo:\n  title: A, from B\n  version: 1.0.0\nchannels:\n  done:\n    $ref: '../b/events.yaml#/channels/done'\noperations:\n  receiveDone:\n    action: receive\n    channel:\n      $ref: '#/channels/done'\n",
);

/// 甲's context file with a key in a comment (W901): the one fake key the fixtures of ritsu hold
/// (ritsu's DESIGN 16.10), written in two pieces here so that the source holds no key whole.
const A_WITH_A_KEY: (&str, &str) = ("甲.ctx", concat!("context 甲(a) v1\n# 検証用の地図の API キー: AIzaSyD-ritsu-fake-", "key-for-tests-000000\nowns\n  dir \"a\"\n"));
/// The same, with its names in English.
const A_WITH_A_KEY_EN: (&str, &str) = ("alpha.ctx", concat!("context Alpha(a) v1\n# the maps API key of the staging site: AIzaSyD-ritsu-fake-", "key-for-tests-000000\nowns\n  dir \"a\"\n"));
/// 乙's events, on a Kafka broker reached without TLS (W902).
const B_EVENTS_ON_KAFKA: (&str, &str) = ("b/events.yaml", "asyncapi: 3.0.0\ninfo:\n  title: B events\n  version: 1.0.0\nservers:\n  production:\n    host: broker.example.com:9092\n    protocol: kafka\n");
/// 乙 publishes an HTTP API whose operation says no authentication (W903).
const B_PUBLISHES_AN_API: (&str, &str) = (
    "乙.ctx",
    "context 乙(b) v1\nowns\n  dir \"b\"\n\npublished language b.v1\n  proto \"b/v1/b.proto\"\n  open host service BService\n\npublished language b.api\n  openapi \"b/api.yaml\"\n\nterms\n  種類 \"乙が扱うものの種類\"\n    means enum Kind\n",
);
/// The same, with its names in English.
const B_PUBLISHES_AN_API_EN: (&str, &str) = (
    "beta.ctx",
    "context Beta(b) v1\nowns\n  dir \"b\"\n\npublished language b.v1\n  proto \"b/v1/b.proto\"\n  open host service BService\n\npublished language b.api\n  openapi \"b/api.yaml\"\n\nterms\n  kind \"The kind of thing Beta deals with\"\n    means enum Kind\n",
);
/// 乙's HTTP API: one operation, and no `security` anywhere.
const B_API_WITHOUT_SECURITY: (&str, &str) = ("b/api.yaml", "openapi: 3.1.0\ninfo:\n  title: B\n  version: 1.0.0\npaths:\n  /things/{id}:\n    get:\n      operationId: getThing\n      responses:\n        '200':\n          description: The thing\n");

/// A gate of 甲 whose action guards a method of 乙's `BService` (E211).
const A_GATE_GUARDS_B: (&str, &str) = (
    "a/関所.gate",
    "gate 甲の関所(a_gate) v1\n\nuse proto 乙 from \"../b/v1/b.proto\"\n\nrole 係(clerk)\n\nprincipal 職員(User)\n  roles 係\n\nresource もの(Thing)\n\naction 見る(get)\n  guards 乙 \"BService/Get\"\n  principal 職員\n  resource もの\n\npermit 係は見る(clerks_get)\n  principal in 係\n  action 見る\n",
);
/// The same, with its names in English.
const A_GATE_GUARDS_B_EN: (&str, &str) = (
    "a/gate.gate",
    "gate a_gate v1\n\nuse proto b from \"../b/v1/b.proto\"\n\nrole clerk\n\nprincipal User\n  roles clerk\n\nresource Thing\n\naction get\n  guards b \"BService/Get\"\n  principal User\n  resource Thing\n\npermit clerks_get\n  principal in clerk\n  action get\n",
);

/// The files of a reproduction: [`BASE`] in its order, each replaced by the entry's file of the
/// same path, then the entry's other files.
fn laid(files: &'static [(&'static str, &'static str)]) -> Vec<(&'static str, &'static str)> {
    let mut out: Vec<(&str, &str)> = BASE.iter().map(|(p, b)| files.iter().find(|(q, _)| q == p).copied().unwrap_or((*p, *b))).collect();
    out.extend(files.iter().filter(|(q, _)| !BASE.iter().any(|(p, _)| p == q)).copied());
    out
}

/// The files of a reproduction with English names: [`BASE_EN`] in its order, each replaced by the
/// entry's file of the same path, then the entry's other files.
fn laid_en(files: &'static [(&'static str, &'static str)]) -> Vec<(&'static str, &'static str)> {
    let mut out: Vec<(&str, &str)> = BASE_EN.iter().map(|(p, b)| files.iter().find(|(q, _)| q == p).copied().unwrap_or((*p, *b))).collect();
    out.extend(files.iter().filter(|(q, _)| !BASE_EN.iter().any(|(p, _)| p == q)).copied());
    out
}

/// An entry that shows a reproduction with English names where it had one with Japanese names:
/// `explain` shows the English one in English and the Japanese one in Japanese, and the tests run both.
trait English {
    fn en(self, files: &'static [(&'static str, &'static str)]) -> Entry;
}

impl English for Entry {
    fn en(self, files: &'static [(&'static str, &'static str)]) -> Entry {
        let command = match &self.repro {
            Repro::Dir { command, .. } => command.iter().map(|w| if *w == "地図.ctx" { "map.ctx" } else { *w }).collect(),
            _ => vec!["check", "."],
        };
        self.english(Repro::Dir { files: laid_en(files), command })
    }
}

/// An entry whose reproduction is `files` laid over [`BASE`], checked with `sakai check .`, or
/// with `ritsu sakai check .` when the files hold an artifact of another language (but for E104,
/// which is what the binary of sakai's own crate says of one); one with no files is a code sakai
/// does not print yet, and has no reproduction.
fn e(code: &'static str, title: Text, when: Text, fix: Text, files: &'static [(&'static str, &'static str)], related: &'static [&'static str]) -> Entry {
    let other = files.iter().any(|(p, _)| [".rule", ".cal", ".flow", ".gate"].iter().any(|x| p.ends_with(x)));
    let command = if other && code != "E104" { vec!["ritsu", "sakai", "check", "."] } else { vec!["check", "."] };
    let repro = if files.is_empty() { Repro::Later } else { Repro::Dir { files: laid(files), command } };
    Entry::new(code, title, when, fix, repro, related)
}

/// An entry run with another command than `check .` (`build` gives E501 and E502).
trait Running {
    fn running(self, command: &'static [&'static str]) -> Entry;
}

impl Running for Entry {
    fn running(mut self, command: &'static [&'static str]) -> Entry {
        let files = match self.repro {
            Repro::Dir { files, .. } => files,
            _ => laid(&[]),
        };
        self.repro = Repro::Dir { files, command: command.to_vec() };
        self
    }
}

/// Whether sakai prints a code: a code with a reproduction (not one retired).
pub fn implemented(e: &Entry) -> bool {
    matches!(e.repro, Repro::Dir { .. })
}

/// The command a reproduction is run with (the words after `sakai`, or the whole command when it
/// is `ritsu sakai …`), and its files.
pub fn reproduction(e: &Entry) -> (Vec<&'static str>, Vec<(&'static str, &'static str)>) {
    match &e.repro {
        Repro::Dir { files, command } => (command.clone(), files.clone()),
        _ => (vec![], vec![]),
    }
}

pub fn ledger() -> Ledger {
    let entries = vec![
        // ── Words, sections, names, paths ──
        e(
            "E001",
            tr!("読めない字句があります", "Something cannot be read as a word of the language"),
            tr!(
                "閉じていない文字列、文字列の中の知らないエスケープ、全角の空白、名前に使えない文字があるとき。名前は文字、数字、`_` で書き、数字では始められません。",
                "A string not closed, an escape a string does not take, a full-width space, or a character a name cannot have. A name is letters, digits and `_`, and does not start with a digit."
            ),
            tr!("示された位置を直してください。文字列は同じ行の `\"` で閉じ、エスケープは `\\\"` と `\\\\` だけを使ってください。", "Correct it where it points: close the string with `\"` on the same line, and use no escape but `\\\"` and `\\\\`."),
            &[("甲.ctx", "context 甲(a) v1\ndescription \"閉じていない\nowns\n  dir \"a\"\n")],
            &["E002"],
        )
        .en(&[("alpha.ctx", "context Alpha(a) v1\ndescription \"not closed\nowns\n  dir \"a\"\n")]),
        e(
            "E002",
            tr!("この位置に書けない語があります", "A word is written where it does not belong"),
            tr!(
                "その位置に、構文が受け付けない語があるとき。役割の無い `upstream`、名前にした予約語、見出しに無いバージョン、行の終わりの余分な語などです。",
                "The syntax does not take the word there: an `upstream` with no role, a keyword as a name, a heading without its version, a word left over at the end of a line, and the like."
            ),
            tr!("注に挙がる書き方のどれかにしてください。", "Use one of the forms the note gives."),
            &[("甲.ctx", "context 甲(a) v1\nowns\n  dir \"a\"\n\nupstream 乙\n  through b.v1\n")],
            &["E004"],
        )
        .en(&[("alpha.ctx", "context Alpha(a) v1\nowns\n  dir \"a\"\n\nupstream Beta\n  through b.v1\n")]),
        e(
            "E003",
            tr!("ファイルが `map` か `context` の行で始まっていません", "The file does not start with a `map` or a `context` line"),
            tr!("コメントと空行を除いた最初の行が、`map …` でも `context …` でもないとき。", "The first line that is not a comment or blank is neither `map …` nor `context …`."),
            tr!(
                "コンテキストマップなら `map 通販(shop) v1`、一つのコンテキストなら `context 在庫(inventory) v1` のように書き始めてください。",
                "Start a context map like `map Shop(shop) v1`, and one context like `context Inventory(inventory) v1`."
            ),
            &[("甲.ctx", "owns\n  dir \"a\"\n")],
            &["E004"],
        )
        .en(&[("alpha.ctx", "owns\n  dir \"a\"\n")]),
        e(
            "E004",
            tr!("節の順序か数が違います", "A section is out of order, repeated, missing, or in the wrong kind of file"),
            tr!(
                "節が決まった順序にないとき、一度だけの節が二度あるとき、省けない節（地図の `use context` と `covers`、コンテキストの `owns`、`upstream` の下の `through`）が無いとき、map のファイルに `owns` があるなど種類の違うファイルの節があるとき。語が定義の文と `as` のどちらか一つを持たないときにも出ます。",
                "Sections are out of their order, a section that comes once comes twice, one that cannot be left out is missing (a map's `use context` and `covers`, a context's `owns`, the `through` under `upstream`), or a section belongs to the other kind of file (`owns` in a map); also a term with both a definition and `as`, or neither."
            ),
            tr!(
                "map のファイルは見出し、`description`、`use context`、`covers`、`except`、`proto root`、`code` の順に、context のファイルは見出し、`description`、`owner`、`also`、`owns`、`published language`、`terms`、関係の順に書いてください。",
                "A map goes: heading, `description`, `use context`, `covers`, `except`, `proto root`, `code`. A context goes: heading, `description`, `owner`, `also`, `owns`, `published language`, `terms`, then the relationships."
            ),
            &[("甲.ctx", "context 甲(a) v1\nowns\n  dir \"a\"\nowner \"甲のチーム\"\n")],
            &["E002", "E003"],
        )
        .en(&[("alpha.ctx", "context Alpha(a) v1\nowns\n  dir \"a\"\nowner \"Alpha's team\"\n")]),
        e(
            "E005",
            tr!("字下げが合いません", "The indentation does not line up"),
            tr!(
                "字下げにタブがあるとき、同じ節の行の字下げがそろっていないとき、受ける行の無いところに字下げした行があるとき。",
                "The indentation has a tab, the lines of one block are not indented alike, or an indented line has no line above to take it."
            ),
            tr!("字下げにはスペースを使い、同じ節の行は同じ幅だけ字下げしてください。", "Indent with spaces, every line of a block by the same amount."),
            &[("甲.ctx", "context 甲(a) v1\nowns\n\tdir \"a\"\n")],
            &[],
        )
        .en(&[("alpha.ctx", "context Alpha(a) v1\nowns\n\tdir \"a\"\n")]),
        e(
            "E006",
            tr!("同じ名前を二度宣言しています", "A name is declared twice"),
            tr!(
                "コンテキストの名前や別名、一つのコンテキストの語の名前と `also`、公表された言語の package、同じ相手への同じ関係、`owns` の項、対応の左辺のどれかが二度出てくるとき。",
                "A context's name or alias, a term's name or `also` in one context, a published package, the same relationship toward the same context, an entry of `owns`, or the left side of a mapping comes twice."
            ),
            tr!("どちらかの名前を変えるか、二つめを消してください。", "Rename one of them, or delete the second."),
            &[(
                "乙.ctx",
                "context 甲(b) v1\nowns\n  dir \"b\"\n\npublished language b.v1\n  proto \"b/v1/b.proto\"\n  open host service BService\n\nterms\n  種類 \"乙が扱うものの種類\"\n    means enum Kind\n",
            )],
            &["E007"],
        )
        .en(&[(
                    "beta.ctx",
                    "context Alpha(b) v1\nowns\n  dir \"b\"\n\npublished language b.v1\n  proto \"b/v1/b.proto\"\n  open host service BService\n\nterms\n  kind \"The kind of thing Beta deals with\"\n    means enum Kind\n",
                )],
        ),
        e(
            "E007",
            tr!("宣言されていない名前です", "A name is not declared"),
            tr!(
                "関係の相手や `as` のコンテキストが地図に無いとき、`means` や対応の要素が proto に無いとき。短い書き方の名前が二つの package に当たるときにも出ます。",
                "A relationship or an `as` names a context the map does not read, or `means` or a mapping names an element the proto does not have; also a short name that two packages have."
            ),
            tr!("名前の書き違いを直すか、`use context` を足してください。二つの package に当たる名前は package から書いてください（`message warehouse.v1.ReserveResponse`）。", "Correct the spelling, or add the `use context`. Write a name two packages have with its package (`message warehouse.v1.ReserveResponse`)."),
            &[("甲.ctx", "context 甲(a) v1\nowns\n  dir \"a\"\n\nupstream 丙 conformist\n  through b.v1\n")],
            &["E006", "E410"],
        )
        .en(&[("alpha.ctx", "context Alpha(a) v1\nowns\n  dir \"a\"\n\nupstream Gamma conformist\n  through b.v1\n")]),
        e(
            "E008",
            tr!("地図かコンテキストに ASCII の別名がありません", "A map or a context has no ASCII alias"),
            tr!(
                "見出しの名前のすぐあとに `(alias)` が無いとき、または別名が `[A-Za-z_][A-Za-z0-9_]*` の形でないとき。別名は CML の名前と、import の検査の設定の名前になります。",
                "The heading's name has no `(alias)` right after it, or the alias is not of the form `[A-Za-z_][A-Za-z0-9_]*`. The alias names it in CML and in the settings of the import linters."
            ),
            tr!("`受注(ordering)` のように、名前のすぐあとに丸括弧で書いてください。", "Write it in parentheses right after the name, like `Ordering(ordering)`."),
            &[("甲.ctx", "context 甲 v1\nowns\n  dir \"a\"\n")],
            &[],
        )
        .en(&[("alpha.ctx", "context Alpha v1\nowns\n  dir \"a\"\n")]),
        e(
            "E009",
            tr!("書いたパスがありません", "A path written is not there"),
            tr!(
                "`use context`、`covers`、`except`、`proto root`、`code`、`owns`、公表された言語の `proto` と `crate` と `generated dir`、`layer`、共有カーネル、`means` のパスが、ディスクに無いとき。パスは、書いたファイルのディレクトリからの相対パスです。",
                "A path of `use context`, `covers`, `except`, `proto root`, `code`, `owns`, a published language's `proto`, `crate` and `generated dir`, `layer`, a shared kernel or `means` is not on the disk. A path counts from the directory of the file it is in."
            ),
            tr!("パスを直してください。", "Correct the path."),
            &[("甲.ctx", "context 甲(a) v1\nowns\n  dir \"a\", \"nowhere\"\n")],
            &["E012"],
        )
        .en(&[("alpha.ctx", "context Alpha(a) v1\nowns\n  dir \"a\", \"nowhere\"\n")]),
        e(
            "E010",
            tr!("`use context` の先が使えません", "What `use context` names cannot be used"),
            tr!("`use context` の先が map のファイルのとき、同じファイルを二度読むとき。", "What `use context` names is a map file, or the same file is named twice."),
            tr!("`use context` の先には context のファイルを書き、同じファイルは一度だけ読んでください。", "Name each context file once."),
            &[("地図.ctx", "map 地図(m) v1\nuse context \"甲.ctx\"\nuse context \"乙.ctx\"\nuse context \"甲.ctx\"\ncovers \".\"\n")],
            &[],
        )
        .en(&[("map.ctx", "map Map(m) v1\nuse context \"alpha.ctx\"\nuse context \"beta.ctx\"\nuse context \"alpha.ctx\"\ncovers \".\"\n")]),
        e(
            "E011",
            tr!("成果物の参照の形が違います", "The name of an artifact is not of the right form"),
            tr!(
                "ツール名と拡張子が合わないとき、`dir` や公表された言語の `crate` の先がファイルのとき、ツール名で書いた先がディレクトリのとき、知らないツール名や、そのツールに無い種類の語を書いたとき、子の種類（`value`、`field`、`method`）が親のすぐあとにないとき、`file` に種類を書いたとき。対応の先が列挙でないときにも出ます。",
                "The tool and the extension do not agree, a `dir` or a published language's `crate` is a file or a tool's file a directory, a tool or a kind is not one the tool has, a child kind (`value`, `field`, `method`) does not come right after its parent, or a `file` is given a kind; also a mapping's target that is not an enum."
            ),
            tr!("`<ツール> \"<パス>\" [<種類> <名前>]…` の形で書いてください。", "Write it in the form `<tool> \"<path>\" [<kind> <name>]...`."),
            &[("甲.ctx", "context 甲(a) v1\nowns\n  rulec \"a/a.proto\"\n")],
            &["E012"],
        )
        .en(&[("alpha.ctx", "context Alpha(a) v1\nowns\n  rulec \"a/a.proto\"\n")]),
        e(
            "E012",
            tr!("絶対パスか、ルートの外に出るパスか、空のパスです", "A path is absolute, goes outside the root, or is empty"),
            tr!(
                "パスを `/` から書いたとき、`..` でルートの外に出るとき、`\"\"` と空で書いたとき。ルートは、sakai に渡したパスの上で .git を持つ一番近いディレクトリです（無ければ渡したディレクトリ。`--root` で替えられます）。",
                "A path starts with `/`, goes outside the root with `..`, or is written empty, `\"\"`. The root is the nearest directory above the path given to sakai that holds .git (else the directory given; `--root` changes it)."
            ),
            tr!(
                "ルートの中のパスを、書いたファイルのディレクトリからの相対パスで書いてください。そのディレクトリ自身は `\".\"` と書いてください。",
                "Write a path inside the root, from the directory of the file; that directory itself is written `\".\"`."
            ),
            &[("甲.ctx", "context 甲(a) v1\nowns\n  dir \"a\", \"../outside\"\n")],
            &["E009"],
        )
        .en(&[("alpha.ctx", "context Alpha(a) v1\nowns\n  dir \"a\", \"../outside\"\n")]),
        // ── Who owns what, and reading the artifacts ──
        e(
            "E101",
            tr!("どのコンテキストにも属さない成果物があります", "An artifact belongs to no context"),
            tr!(
                "範囲の成果物が、どのコンテキストの `owns` の項にも含まれないとき。属さないファイルしか含まないディレクトリは、いちばん上のディレクトリにまとめて一つの診断になります。",
                "An artifact of the scope is held by no entry of any context's `owns`. A directory of such files only is told once, at its top."
            ),
            tr!("どれかのコンテキストの `owns` に、そのディレクトリかファイルを書いてください。", "Write the directory or the file under the `owns` of a context."),
            &[("c/c.proto", "syntax = \"proto3\";\npackage c;\nmessage C {}\n")],
            &["E102"],
        )
        .en(&[("c/c.proto", "syntax = \"proto3\";\npackage c;\nmessage C {}\n")]),
        e(
            "E102",
            tr!("二つのコンテキストが同じ深さで持つ成果物があります", "Two contexts own an artifact at the same depth"),
            tr!("二つのコンテキストの `owns` が、同じディレクトリかファイルを書いているとき。成果物はそれを含むいちばん深い項のコンテキストに属するので、同じ深さの項が二つあると、どちらのものか決められません。", "The `owns` of two contexts write the same directory or file; an artifact belongs to the context of the deepest entry that holds it, and that cannot be told."),
            tr!("一方を消すか、どちらかにもっと深いディレクトリを書いてください。", "Delete one, or let one of them write a deeper directory."),
            &[(
                "乙.ctx",
                "context 乙(b) v1\nowns\n  dir \"b\", \"a\"\n\npublished language b.v1\n  proto \"b/v1/b.proto\"\n  open host service BService\n\nterms\n  種類 \"乙が扱うものの種類\"\n    means enum Kind\n",
            )],
            &["E101"],
        )
        .en(&[(
                    "beta.ctx",
                    "context Beta(b) v1\nowns\n  dir \"b\", \"a\"\n\npublished language b.v1\n  proto \"b/v1/b.proto\"\n  open host service BService\n\nterms\n  kind \"The kind of thing Beta deals with\"\n    means enum Kind\n",
                )],
        ),
        e(
            "E103",
            tr!("範囲の外のものを、持っているか参照しています", "Something outside the scope is owned or referred to"),
            tr!(
                "`owns` の項や、proto の import の先、`means` の proto が、地図の範囲の外にあるとき。範囲は `covers` で決まり、`except` と、パスに . で始まる名前、node_modules、site-packages、__pycache__、target を含むものは外れます。",
                "An entry of `owns`, what a proto imports, or the proto of a `means` is outside the map's scope: the `covers`, without `except` and without any path with a name that starts with ., node_modules, site-packages, __pycache__ or target in it."
            ),
            tr!("範囲を広げるか、項や参照を消してください。", "Widen the scope, or delete the entry or the reference."),
            &[
                ("地図.ctx", "map 地図(m) v1\nuse context \"甲.ctx\"\nuse context \"乙.ctx\"\ncovers \".\"\nexcept \"c\"\n"),
                ("甲.ctx", "context 甲(a) v1\nowns\n  dir \"a\", \"c\"\n"),
                ("c/c.proto", "syntax = \"proto3\";\npackage c;\nmessage C {}\n"),
            ],
            &[],
        )
        .en(&[
                    ("map.ctx", "map Map(m) v1\nuse context \"alpha.ctx\"\nuse context \"beta.ctx\"\ncovers \".\"\nexcept \"c\"\n"),
                    ("alpha.ctx", "context Alpha(a) v1\nowns\n  dir \"a\", \"c\"\n"),
                    ("c/c.proto", "syntax = \"proto3\";\npackage c;\nmessage C {}\n"),
                ],
        ),
        e(
            "E104",
            tr!("ほかの言語の成果物を読めません（言語がつながっていません）", "Another language's artifacts cannot be read: the language is not joined"),
            tr!(
                "地図が rulec、koyomi、dandori の成果物を含むのに、その言語を読む部分がつながっていないとき（sakai 単独のバイナリで走らせたとき）。sakai は、言語ごとに一度、その最初の成果物を持つ `owns` の行でこのエラーを出します。確かめていないことを、黙って通すことはしません。exit code は 2 です（地図の誤りではなく、走らせ方の問題なので）。",
                "The map holds rulec, koyomi or dandori artifacts, and the ports of ritsu that read that language are not joined (the binary of sakai's own crate). Told once a language, at the line of `owns` that holds its first artifact. What is not checked is not passed in silence. The exit code is 2: it is how the command is run, not what the map says."
            ),
            tr!("すべての言語をつないだ `ritsu sakai` で走らせてください。", "Run it as `ritsu sakai`, which joins every language."),
            &[A_RULE_TAKES_KIND],
            &["E105"],
        )
        .en(&[A_RULE_TAKES_KIND_EN]),
        e(
            "E105",
            tr!("成果物が、その言語の検査を通らないか、読めません", "An artifact does not pass its language's check, or cannot be read"),
            tr!(
                "rulec が規則の情報を返さないとき（規則が rulec の検査を通らないとき）や、koyomi と dandori がファイルを読めないとき。注には、その言語の診断が並びます。sakai は、その成果物の参照を確かめられません。",
                "rulec does not answer for a rule (it does not pass rulec's check), or koyomi or dandori cannot read the file. The notes give what the language says; the references of the artifact cannot be checked."
            ),
            tr!("その成果物を、その言語の検査が通るように直してください。", "Fix the artifact until its language's check passes."),
            &[A_RULE_WITH_A_GAP],
            &["E104"],
        )
        .en(&[A_RULE_WITH_A_GAP]),
        e(
            "E106",
            tr!("proto が読めません", "A proto cannot be read"),
            tr!("範囲の `.proto` を、sakai が読めないとき（構文の誤り、proto2 の `group`）。", "sakai's reader cannot read a `.proto` of the scope: a syntax error, or a proto2 `group`."),
            tr!("示された位置を直してください。", "Correct it where it points."),
            &[("b/v1/b.proto", "syntax = \"proto3\";\npackage b.v1;\nenum Kind {\n  KIND_UNSPECIFIED = 0;\n  KIND_ONE = 1;\n  KIND_TWO = 2;\n}\nmessage B { Kind kind = ; }\nmessage Plain { string id = 1; }\nservice BService { rpc Get(B) returns (B); }\n")],
            &[],
        )
        .en(&[("b/v1/b.proto", "syntax = \"proto3\";\npackage b.v1;\nenum Kind {\n  KIND_UNSPECIFIED = 0;\n  KIND_ONE = 1;\n  KIND_TWO = 2;\n}\nmessage B { Kind kind = ; }\nmessage Plain { string id = 1; }\nservice BService { rpc Get(B) returns (B); }\n")]),
        Entry::new(
            "E107",
            tr!("Cargo から Rust のクレートを読めません", "Cargo cannot say the crates of the Rust code"),
            tr!(
                "地図が `code rust` を書いていて、その場所に `Cargo.toml` が無いか、`cargo metadata` が失敗したとき（cargo が無い、マニフェストが読めない、など）。sakai は、Rust のクレートとその依存を Cargo から読みます。読めなければ、Rust のクレートの依存は確かめられません。",
                "The map writes `code rust`, and there is no `Cargo.toml` at its place, or `cargo metadata` fails (cargo is not there, a manifest does not read, and the like). sakai reads Rust's crates and their dependencies from Cargo; when it cannot, the dependencies of the crates are not checked."
            ),
            tr!(
                "`code rust` には、ワークスペースの（クレートが一つなら、そのクレートの）`Cargo.toml` のあるディレクトリを書いてください。そのディレクトリで `cargo metadata --no-deps --offline` が通ることも確かめてください。",
                "Write under `code rust` the directory of the workspace's `Cargo.toml` (for one crate, of its own), and see that `cargo metadata --no-deps --offline` passes there."
            ),
            Repro::Dir { files: RUST.to_vec(), command: vec!["check", "."] },
            &[],
        ),
        e(
            "E108",
            tr!("OpenAPI か AsyncAPI の文書を読めません", "An OpenAPI or AsyncAPI document cannot be read"),
            tr!(
                "範囲の OpenAPI か AsyncAPI の文書（一番上に `openapi`、`asyncapi`、`swagger` のある `.yaml`、`.yml`、`.json`）か、文書が `$ref` で指すファイルを読めないとき。YAML か JSON として読めないとき、JSON と行き来できない YAML の書き方（タグ、`?` のキー、二つ目の文書など）をしているとき、sakai が読まない版（OpenAPI 2.0、AsyncAPI 2.x）のとき、`$ref` の先のファイルが無いか、ポインタが何も指さないときに出ます。",
                "An OpenAPI or AsyncAPI document of the scope (a `.yaml`, `.yml` or `.json` whose top holds `openapi`, `asyncapi` or `swagger`), or a file a document points at by `$ref`, cannot be read: it does not read as YAML or JSON, it writes YAML that does not go to JSON and back (a tag, a key with `?`, a second document), it is of a version sakai does not read (OpenAPI 2.0, AsyncAPI 2.x), or a `$ref` points at no file or at nothing in one."
            ),
            tr!(
                "示された位置を直してください。OpenAPI 2.0 は OpenAPI 3 に、AsyncAPI 2.x は `asyncapi convert` で AsyncAPI 3 に変換してください。",
                "Correct it where it points. Convert OpenAPI 2.0 to OpenAPI 3, and AsyncAPI 2.x to AsyncAPI 3 with `asyncapi convert`."
            ),
            &[B_API_WITH_A_TAB],
            &["W104"],
        )
        .en(&[B_API_WITH_A_TAB]),
        e(
            "W101",
            tr!("`owns` の項が成果物を一つも含みません", "An entry of `owns` holds no artifact"),
            tr!("`owns` のディレクトリかファイルに、成果物が一つも無いとき。たいていはパスの書き誤りです。", "A directory or file under `owns` holds no artifact; the path is often mistyped."),
            tr!("パスを直すか、項を消してください。", "Correct the path, or delete the entry."),
            &[("甲.ctx", "context 甲(a) v1\nowns\n  dir \"a\", \"empty\"\n"), ("empty/README.txt", "nothing here is an artifact\n")],
            &[],
        )
        .en(&[("alpha.ctx", "context Alpha(a) v1\nowns\n  dir \"a\", \"empty\"\n"), ("empty/README.txt", "nothing here is an artifact\n")]),
        e(
            "W102",
            tr!("proto の import が見つかりません", "A proto's import is not found"),
            tr!(
                "import の先が、地図の `proto root`、その proto の package の形から決まるディレクトリ、その proto のディレクトリのどこにも無いとき。Google の well-known types、`buf/validate`、dandori の `options.proto` は、ファイルが無くても sakai が中身を知っています。",
                "What a proto imports is in none of the map's `proto root`s, the directory its package places it in, or its own directory. Google's well-known types, `buf/validate` and dandori's `options.proto` are known without their files."
            ),
            tr!("地図に `proto root` を書くか、import を直してください。範囲の外の契約なら、そのままでかまいません（その型は参照の検査から外れます）。", "Write a `proto root` in the map, or correct the import; a contract outside the scope may stay so (its types are left out of the check of the references)."),
            &[("a/a.proto", "syntax = \"proto3\";\npackage a;\nimport \"elsewhere/v1/x.proto\";\nmessage A {}\n")],
            &[],
        )
        .en(&[("a/a.proto", "syntax = \"proto3\";\npackage a;\nimport \"elsewhere/v1/x.proto\";\nmessage A {}\n")]),
        e(
            "W103",
            tr!("どの地図にも読まれない context のファイルがあります", "A context file no map reads"),
            tr!("`check` にディレクトリを渡したとき、その下の context のファイルを、どの地図も `use context` で読んでいないとき。", "`check` was given a directory, and no map under it names a context file under it with `use context`."),
            tr!("地図の `use context` に足すか、ファイルを消してください。", "Add it to a map's `use context`, or delete the file."),
            &[("丙.ctx", "context 丙(c) v1\nowns\n  dir \"a\"\n")],
            &[],
        )
        .en(&[("gamma.ctx", "context Gamma(c) v1\nowns\n  dir \"a\"\n")]),
        e(
            "W104",
            tr!("`$ref` が URL を指しています", "A `$ref` points at a URL"),
            tr!(
                "OpenAPI か AsyncAPI の文書の `$ref` が、`https://…` のような URL を指すとき。sakai はネットワークに出ないので、その先を読まず、範囲の外のものとして扱います（その先の型は、境界の検査から外れます）。",
                "A `$ref` of an OpenAPI or AsyncAPI document points at a URL, like `https://…`. sakai does not go to the network: what it points at is not read, and is taken as outside the scope (its types are left out of the checks of the boundaries)."
            ),
            tr!(
                "ほかのコンテキストの文書なら、リポジトリの中のファイルを相対パスで指してください。外のシステムの契約なら、そのままでかまいません。",
                "For another context's document, point at its file in the repository by a relative path; a contract of a system outside may stay so."
            ),
            &[B_API_WITH_A_URL],
            &["E108"],
        )
        .en(&[B_API_WITH_A_URL]),
        e(
            "N101",
            tr!("dandori の参照を確かめていません", "The references of dandori are not checked"),
            tr!("いまは出ません。dandori が参照を JSON で出さなかったころに、地図が `.flow` を含むとき、属し方だけを確かめたことを伝えるためのコードでした。", "Never: it was for a map that holds `.flow` files, when sakai checked only who owns them, dandori printing no references as JSON."),
            tr!("直すものはありません。", "Nothing to fix."),
            &[],
            &["E202", "E207", "E208", "E209"],
        )
        .retired(tr!(
            "ritsu 0.23.0 で使われなくなりました。いまの sakai は、dandori の参照を ritsu の中で（`References` から）読み、境界を越えるものを確かめます（E202、E207〜E209）。",
            "Retired in ritsu 0.23.0: dandori's references are read through ritsu's ports (`References`), and those that cross a boundary are checked (E202, E207 to E209)."
        )),
        // ── The references that cross a boundary ──
        e(
            "E201",
            tr!("関係の無いコンテキストへの参照です", "A reference to a context there is no relationship with"),
            tr!("境界を越える参照の相手と、上流と下流（下流が書く）、パートナーシップ、共有カーネルのどの関係も無いとき。関係が逆向き（参照の先が下流）のときにも出ます。", "A reference crosses to a context with no relationship that allows it: upstream and downstream (written by the downstream), a partnership, or a shared kernel; also when the relationship goes the other way."),
            tr!("参照する側に `upstream <相手> <役割>` と `through <package>` を書くか、参照を消してください。", "Write `upstream <context> <role>` and `through <package>` in the referring context, or delete the reference."),
            &[A_USES_B],
            &["E202", "E206"],
        )
        .en(&[A_USES_B]),
        e(
            "E202",
            tr!("相手の内側への参照です", "A reference to the inside of another context"),
            tr!("参照の先が、相手の公表された言語にも、二つの共有カーネルにも入っていないとき（公表された言語でない proto や OpenAPI と AsyncAPI の文書、規則やカレンダーそのもの）。", "What a reference crosses to is in no published language of the other context and in no shared kernel of the two: a proto or an OpenAPI or AsyncAPI document that is not published, a rule or a calendar itself."),
            tr!("相手の公表された言語を通して参照するか、二つの共有カーネルに並べてください。", "Refer through the other context's published language, or list it in a shared kernel of the two."),
            &[
                A_CONFORMS,
                ("a/a.proto", "syntax = \"proto3\";\npackage a;\nimport \"b/internal/v1/x.proto\";\nmessage A { b.internal.v1.X x = 1; }\n"),
                ("b/internal/v1/x.proto", "syntax = \"proto3\";\npackage b.internal.v1;\nmessage X {}\n"),
            ],
            &["E201"],
        )
        .en(&[
                    A_CONFORMS_EN,
                    ("a/a.proto", "syntax = \"proto3\";\npackage a;\nimport \"b/internal/v1/x.proto\";\nmessage A { b.internal.v1.X x = 1; }\n"),
                    ("b/internal/v1/x.proto", "syntax = \"proto3\";\npackage b.internal.v1;\nmessage X {}\n"),
                ],
        ),
        e(
            "E203",
            tr!("`through` に無い package を通る参照です", "A reference goes through a package `through` does not list"),
            tr!("上流の公表された言語のうち、関係の `through` に並べていない package を参照しているとき。", "A reference reaches a published package of the upstream that the relationship's `through` does not list."),
            tr!("`through` に package を足すか、参照を直してください。", "Add the package to `through`, or change the reference."),
            &[
                A_CONFORMS,
                (
                    "乙.ctx",
                    "context 乙(b) v1\nowns\n  dir \"b\"\n\npublished language b.v1\n  proto \"b/v1/b.proto\"\n  open host service BService\n\npublished language b.v2\n  proto \"b/v2/b2.proto\"\n\nterms\n  種類 \"乙が扱うものの種類\"\n    means enum Kind\n",
                ),
                ("b/v2/b2.proto", "syntax = \"proto3\";\npackage b.v2;\nmessage B2 {}\n"),
                ("a/a.proto", "syntax = \"proto3\";\npackage a;\nimport \"b/v2/b2.proto\";\nmessage A { b.v2.B2 b = 1; }\n"),
            ],
            &["E312"],
        )
        .en(&[
                    A_CONFORMS_EN,
                    (
                        "beta.ctx",
                        "context Beta(b) v1\nowns\n  dir \"b\"\n\npublished language b.v1\n  proto \"b/v1/b.proto\"\n  open host service BService\n\npublished language b.v2\n  proto \"b/v2/b2.proto\"\n\nterms\n  kind \"The kind of thing Beta deals with\"\n    means enum Kind\n",
                    ),
                    ("b/v2/b2.proto", "syntax = \"proto3\";\npackage b.v2;\nmessage B2 {}\n"),
                    ("a/a.proto", "syntax = \"proto3\";\npackage a;\nimport \"b/v2/b2.proto\";\nmessage A { b.v2.B2 b = 1; }\n"),
                ],
        ),
        e(
            "E204",
            tr!("腐敗防止層の外からの、上流の公表された言語への参照です", "A reference to the upstream's published language from outside the anticorruption layer"),
            tr!("腐敗防止層の関係に `layer` があるのに、層の外の成果物が上流の公表された言語を参照しているとき。", "An anticorruption layer has a `layer`, and an artifact outside it refers to the upstream's published language."),
            tr!("参照を層の中に移すか、`layer` に足してください。", "Move the reference into the layer, or add it to `layer`."),
            &[
                ("甲.ctx", "context 甲(a) v1\nowns\n  dir \"a\"\n\nupstream 乙 anticorruption layer\n  through b.v1\n  layer dir \"a/acl\"\n"),
                ("a/acl/v1/acl.proto", "syntax = \"proto3\";\npackage a.acl.v1;\nmessage Seen {}\n"),
                A_USES_PLAIN,
            ],
            &["E205"],
        )
        .en(&[
                    ("alpha.ctx", "context Alpha(a) v1\nowns\n  dir \"a\"\n\nupstream Beta anticorruption layer\n  through b.v1\n  layer dir \"a/acl\"\n"),
                    ("a/acl/v1/acl.proto", "syntax = \"proto3\";\npackage a.acl.v1;\nmessage Seen {}\n"),
                    A_USES_PLAIN,
                ],
        ),
        e(
            "E205",
            tr!("腐敗防止層の下流の公表された言語に、上流の型が出ています", "The published language of a layer's downstream shows the upstream's types"),
            tr!("腐敗防止層の下流が、自分の公表された言語の proto で、上流の公表された言語を import しているとき（OpenAPI と AsyncAPI の文書なら、上流の文書を `$ref` で指しているとき）。上流のモデルが、層を通らずに下流の外へ出ていきます。", "Downstream of an anticorruption layer, a proto of the downstream's own published language imports the upstream's published language (or a document of it points at the upstream's documents by `$ref`): the upstream's model goes out past the layer."),
            tr!("層の中で自分の型に読み替え、公表された言語には自分の型だけを出してください。", "Map it to the downstream's own types in the layer, and publish those only."),
            &[
                (
                    "甲.ctx",
                    "context 甲(a) v1\nowns\n  dir \"a\"\n\npublished language a.v1\n  proto \"a/v1/a1.proto\"\n\nupstream 乙 anticorruption layer\n  through b.v1\n",
                ),
                ("a/v1/a1.proto", "syntax = \"proto3\";\npackage a.v1;\nimport \"b/v1/b.proto\";\nmessage A1 { b.v1.Plain p = 1; }\n"),
            ],
            &["E204"],
        )
        .en(&[
                    (
                        "alpha.ctx",
                        "context Alpha(a) v1\nowns\n  dir \"a\"\n\npublished language a.v1\n  proto \"a/v1/a1.proto\"\n\nupstream Beta anticorruption layer\n  through b.v1\n",
                    ),
                    ("a/v1/a1.proto", "syntax = \"proto3\";\npackage a.v1;\nimport \"b/v1/b.proto\";\nmessage A1 { b.v1.Plain p = 1; }\n"),
                ],
        ),
        e(
            "E206",
            tr!("別々の道の相手への参照です", "A reference to a context it goes separate ways from"),
            tr!("`separate ways from` を書いた二つのあいだに、境界を越える参照があるとき。", "There is a reference across the boundary between two contexts that go separate ways."),
            tr!("参照を消すか、別々の道をやめて関係を書いてください。", "Delete the reference, or replace separate ways with a relationship."),
            &[("甲.ctx", "context 甲(a) v1\nowns\n  dir \"a\"\n\nseparate ways from 乙\n"), A_USES_PLAIN],
            &["E310"],
        )
        .en(&[("alpha.ctx", "context Alpha(a) v1\nowns\n  dir \"a\"\n\nseparate ways from Beta\n"), A_USES_PLAIN]),
        e(
            "E207",
            tr!("ワークフローが、相手の公開ホストサービスでないサービスを呼んでいます", "A workflow calls a service that is no open host service of the other side"),
            tr!(
                "ワークフローが境界の向こうのサービスを `connect` で呼ぶか、規則を `use rule … connect` で呼ぶのに、そのサービスが、相手の公表された言語の `open host service` に無いとき。",
                "A workflow calls a service across a boundary with `connect`, or a rule with `use rule … connect`, and the service is not under `open host service` of the other side's published language."
            ),
            tr!("相手にそのサービスを `open host service` に並べてもらうか、相手が並べたサービスを呼んでください。", "The other side lists the service under `open host service`, or the workflow calls one it lists."),
            &[A_CONFORMS, ("乙.ctx", "context 乙(b) v1\nowns\n  dir \"b\"\n\npublished language b.v1\n  proto \"b/v1/b.proto\"\n\nterms\n  種類 \"乙が扱うものの種類\"\n    means enum Kind\n"), A_CALLS_B],
            &["E202", "E301"],
        )
        .en(&[A_CONFORMS_EN, ("beta.ctx", "context Beta(b) v1\nowns\n  dir \"b\"\n\npublished language b.v1\n  proto \"b/v1/b.proto\"\n\nterms\n  kind \"The kind of thing Beta deals with\"\n    means enum Kind\n"), A_CALLS_B]),
        e(
            "E208",
            tr!("ワークフローが実装するサービスが、自分の公表された言語にありません", "The service a workflow implements is not in its own published language"),
            tr!(
                "ワークフローが `implements` で実装するサービスが、そのワークフローを持つコンテキストの公表された言語の `open host service` に無いとき（proto が公表された言語に無いときも）。",
                "The service a workflow `implements` is not under `open host service` of a published language of the workflow's context (nor when the .proto is in no published language)."
            ),
            tr!("自分の公表された言語に proto を並べ、`open host service` にサービスを書いてください。", "List the .proto in the context's published language, and the service under `open host service`."),
            &[
                ("甲.ctx", "context 甲(a) v1\nowns\n  dir \"a\"\n\npublished language a.v1\n  proto \"a/v1/a.proto\"\n"),
                ("a/v1/a.proto", "syntax = \"proto3\";\npackage a.v1;\nimport \"dandori/v1/options.proto\";\nservice AService {\n  option (dandori.v1.workflow) = {name: \"w\", version: 1};\n  rpc Start(StartRequest) returns (StartResponse) {\n    option (dandori.v1.start) = {};\n  }\n}\nmessage StartRequest { string id = 1; }\nmessage StartResponse { string id = 1; }\n"),
                ("a/w.flow", "workflow w v1 implements a1.AService\n\nuse proto a1 from \"v1/a.proto\"\n\ninputs\n  id : string\n\noutputs\n  id : string\n\nflow\n  succeed id = id\n"),
            ],
            &["E301"],
        )
        .en(&[
                    ("alpha.ctx", "context Alpha(a) v1\nowns\n  dir \"a\"\n\npublished language a.v1\n  proto \"a/v1/a.proto\"\n"),
                    ("a/v1/a.proto", "syntax = \"proto3\";\npackage a.v1;\nimport \"dandori/v1/options.proto\";\nservice AService {\n  option (dandori.v1.workflow) = {name: \"w\", version: 1};\n  rpc Start(StartRequest) returns (StartResponse) {\n    option (dandori.v1.start) = {};\n  }\n}\nmessage StartRequest { string id = 1; }\nmessage StartResponse { string id = 1; }\n"),
                    ("a/w.flow", "workflow w v1 implements a1.AService\n\nuse proto a1 from \"v1/a.proto\"\n\ninputs\n  id : string\n\noutputs\n  id : string\n\nflow\n  succeed id = id\n"),
                ],
        ),
        e(
            "E209",
            tr!("ワークフローが、境界の向こうのワークフローを子として走らせています", "A workflow runs another context's workflow as its child"),
            tr!(
                "ワークフローが `flow` で走らせる子のフローが別のコンテキストのもので、二つがパートナーシップでなく、子のフローが共有カーネルになく、子のフローが相手の公開ホストサービスを実装していないとき。",
                "A child workflow a workflow runs with `flow` belongs to another context, the two are not partners, the child is in no shared kernel of theirs, and it implements no open host service of the other side."
            ),
            tr!("相手が公表したサービスを `connect` で呼ぶか、二つをパートナーシップにするか、子のフローを共有カーネルに並べるか、子のフローに相手の公開ホストサービスを実装させてください。", "Call a service the other side publishes with `connect`; or make the two partners; or list the child in their shared kernel; or have the child implement an open host service of the other side."),
            &[
                A_CONFORMS,
                ("b/c.flow", "workflow c v1\n\ninputs\n  id : string\n\noutputs\n  id : string\n\nflow\n  succeed id = id\n"),
                ("a/p.flow", "workflow p v1\n\nrecord 答え\n  id : string\n\ninputs\n  id : string\n\ntask run_child(id: string) -> 答え\n  flow \"../b/c.flow\"\n\nflow\n  let r = run_child(id: id)\n  succeed\n"),
            ],
            &["E202", "E207"],
        )
        .en(&[
                    A_CONFORMS_EN,
                    ("b/c.flow", "workflow c v1\n\ninputs\n  id : string\n\noutputs\n  id : string\n\nflow\n  succeed id = id\n"),
                    ("a/p.flow", "workflow p v1\n\nrecord Answer\n  id : string\n\ninputs\n  id : string\n\ntask run_child(id: string) -> Answer\n  flow \"../b/c.flow\"\n\nflow\n  let r = run_child(id: id)\n  succeed\n"),
                ],
        ),
        e(
            "E210",
            tr!("文書が、相手の公開ホストサービスでないチャネルか HTTP の操作を使っています", "A document uses a channel or an HTTP operation that is no open host service of the other side"),
            tr!(
                "AsyncAPI の文書が境界の向こうのチャネルに送るか、そこから受ける（ルートの `channels` の項が、ほかのコンテキストの文書のチャネルを `$ref` で指す）のに、そのチャネルが相手の公表された言語の `open host service` に無いとき。OpenAPI の文書が、境界の向こうの `paths` の項を `$ref` で指すとき、その HTTP の操作についても同じです。",
                "An AsyncAPI document sends to or receives from a channel across a boundary (an entry of its `channels` is a `$ref` to a channel of another context's document), and the channel is not under `open host service` of the other side's published language; and the same for the HTTP operations of an entry of `paths` an OpenAPI document points at across a boundary."
            ),
            tr!(
                "相手にそのチャネルか操作を `open host service` に並べてもらうか、相手が並べたものを使ってください。",
                "The other side lists the channel or the operation under `open host service`, or the document uses one it lists."
            ),
            &[
                ("甲.ctx", "context 甲(a) v1\nowns\n  dir \"a\"\n\nupstream 乙 conformist\n  through b.events.v1\n"),
                (
                    "乙.ctx",
                    "context 乙(b) v1\nowns\n  dir \"b\"\n\npublished language b.v1\n  proto \"b/v1/b.proto\"\n  open host service BService\n\npublished language b.events.v1\n  asyncapi \"b/events.yaml\"\n\nterms\n  種類 \"乙が扱うものの種類\"\n    means enum Kind\n",
                ),
                B_EVENTS,
                A_RECEIVES_DONE,
            ],
            &["E207", "E301"],
        )
        .en(&[
            ("alpha.ctx", "context Alpha(a) v1\nowns\n  dir \"a\"\n\nupstream Beta conformist\n  through b.events.v1\n"),
            (
                "beta.ctx",
                "context Beta(b) v1\nowns\n  dir \"b\"\n\npublished language b.v1\n  proto \"b/v1/b.proto\"\n  open host service BService\n\npublished language b.events.v1\n  asyncapi \"b/events.yaml\"\n\nterms\n  kind \"The kind of thing Beta deals with\"\n    means enum Kind\n",
            ),
            B_EVENTS,
            A_RECEIVES_DONE,
        ]),
        e(
            "E211",
            tr!("ほかのコンテキストの操作を守る action です", "An action guards an operation of another context"),
            tr!(
                "`.gate` の action が `guards` で守る操作（か、`cedar \"…\"` と書いた Cedar のスキーマが `@guards` に書いた操作）の契約（OpenAPI と AsyncAPI の文書、`.proto`、帳簿）が、その `.gate` と違うコンテキストに属するとき。二つのあいだに関係があっても出ます。",
                "An operation an action of a `.gate` guards (or one the `@guards` of a schema of Cedar written as `cedar \"…\"` names) is of a contract (an OpenAPI or AsyncAPI document, a `.proto`, a book) that belongs to another context than the `.gate`; whatever the two are to each other."
            ),
            tr!(
                "action を、操作の契約を持つコンテキストの `.gate` に移すか、`guards` の行を消してください。認可の決まりは、操作を持つサービスが自分で書きます。",
                "Move the action to a `.gate` of the context that holds the operation's contract, or delete the `guards` line: the service that holds an operation writes who may call it."
            ),
            &[A_GATE_GUARDS_B],
            &["E202"],
        )
        .en(&[A_GATE_GUARDS_B_EN]),
        // ── The patterns ──
        e(
            "E301",
            tr!("公開ホストサービスが、公表された言語に無いサービスです", "An open host service is not in the published language"),
            tr!("`open host service` に並べたサービスが、その公表された言語の proto に無いとき。OpenAPI と AsyncAPI の文書の公表された言語では、並べた名前が、文書の HTTP の操作（operationId）にもチャネルにも無いとき。Rust のクレートの公表された言語に `open host service` を書いたときにも出ます（クレートはサービスを持ちません）。", "A service under `open host service` is not in the proto files of the published language; for a published language of OpenAPI and AsyncAPI documents, a name that is no HTTP operation (operationId) and no channel of its documents; also any under the published language of a Rust crate, which has no service."),
            tr!("サービスの名前を直すか、proto にサービスを足してください。", "Correct the service's name, or add the service to the proto."),
            &[(
                "乙.ctx",
                "context 乙(b) v1\nowns\n  dir \"b\"\n\npublished language b.v1\n  proto \"b/v1/b.proto\"\n  open host service BService, NoService\n\nterms\n  種類 \"乙が扱うものの種類\"\n    means enum Kind\n",
            )],
            &["E302"],
        )
        .en(&[(
                    "beta.ctx",
                    "context Beta(b) v1\nowns\n  dir \"b\"\n\npublished language b.v1\n  proto \"b/v1/b.proto\"\n  open host service BService, NoService\n\nterms\n  kind \"The kind of thing Beta deals with\"\n    means enum Kind\n",
                )],
        ),
        e(
            "E302",
            tr!("公表された言語のファイルが、そのコンテキストのものでないか、package が違います", "A file of a published language is not the context's, or has another package"),
            tr!(
                "公表された言語の proto、規則、Rust のクレート、OpenAPI と AsyncAPI の文書、生成したコードの置き場所が、そのコンテキストに属さないとき。proto の package やクレートの名前（`-` を `_` にしたもの）が見出しと違うとき、`crate` の先が地図の `code rust` のワークスペースのクレートでないとき、地図に `code rust` が無いのにクレートを公表したときにも出ます。",
                "A proto, a rule, a Rust crate or the place of the generated code of a published language does not belong to the context; or a proto's package, or a crate's name (with `-` written `_`), is not what the heading names; or a `crate` is no crate of the workspace at the map's `code rust` place, or the map has no `code rust`."
            ),
            tr!("見出しの package を直すか、そのコンテキストのファイルを並べてください。", "Correct the heading's package, or list the context's own files."),
            &[(
                "乙.ctx",
                "context 乙(b) v1\nowns\n  dir \"b\"\n\npublished language b.v2\n  proto \"b/v1/b.proto\"\n  open host service BService\n",
            )],
            &["E301"],
        )
        .en(&[(
                    "beta.ctx",
                    "context Beta(b) v1\nowns\n  dir \"b\"\n\npublished language b.v2\n  proto \"b/v1/b.proto\"\n  open host service BService\n",
                )],
        ),
        e(
            "E303",
            tr!("顧客／供給者が片側だけです", "Customer and supplier is written on one side only"),
            tr!("顧客の側の `upstream <供給者> customer` と、供給者の側の `downstream <顧客> supplier` の、どちらかが無いとき。", "One of the customer's `upstream <supplier> customer` and the supplier's `downstream <customer> supplier` is missing."),
            tr!("もう片方のファイルにも書いてください。", "Write it in the other file too."),
            &[("甲.ctx", "context 甲(a) v1\nowns\n  dir \"a\"\n\nupstream 乙 customer\n  through b.v1\n")],
            &["E306"],
        )
        .en(&[("alpha.ctx", "context Alpha(a) v1\nowns\n  dir \"a\"\n\nupstream Beta customer\n  through b.v1\n")]),
        e(
            "E304",
            tr!("順応者に対応か `layer` があります", "A conformist has a mapping or a `layer`"),
            tr!("`conformist` の関係に、`layer`、`enum` の対応、`term` の対応のどれかがあるとき。順応者は上流のモデルをそのまま使います。", "A `conformist` relationship has a `layer`, an `enum` mapping or a `term` mapping; a conformist uses the upstream's model as it is."),
            tr!("読み替えるなら役割を anticorruption layer にし、そうでなければ対応と層を消してください。", "To map the model, make the role anticorruption layer; otherwise delete the mappings and the layer."),
            &[("甲.ctx", "context 甲(a) v1\nowns\n  dir \"a\"\n\nupstream 乙 conformist\n  through b.v1\n  layer dir \"a\"\n")],
            &["E305"],
        )
        .en(&[("alpha.ctx", "context Alpha(a) v1\nowns\n  dir \"a\"\n\nupstream Beta conformist\n  through b.v1\n  layer dir \"a\"\n")]),
        e(
            "E305",
            tr!("腐敗防止層でないのに、対応か `layer` があります", "A relationship that is not an anticorruption layer has a mapping or a `layer`"),
            tr!("役割に anticorruption layer の無い関係（customer だけなど）に、対応か `layer` があるとき。", "A relationship with no anticorruption layer among its roles (customer alone, say) has a mapping or a `layer`."),
            tr!("役割に `, anticorruption layer` を足すか、対応と層を消してください。", "Add `, anticorruption layer` to the roles, or delete the mappings and the layer."),
            &[
                ("甲.ctx", "context 甲(a) v1\nowns\n  dir \"a\"\n\nupstream 乙 customer\n  through b.v1\n  layer dir \"a\"\n"),
                (
                    "乙.ctx",
                    "context 乙(b) v1\nowns\n  dir \"b\"\n\npublished language b.v1\n  proto \"b/v1/b.proto\"\n  open host service BService\n\nterms\n  種類 \"乙が扱うものの種類\"\n    means enum Kind\n\ndownstream 甲 supplier\n",
                ),
            ],
            &["E304"],
        )
        .en(&[
                    ("alpha.ctx", "context Alpha(a) v1\nowns\n  dir \"a\"\n\nupstream Beta customer\n  through b.v1\n  layer dir \"a\"\n"),
                    (
                        "beta.ctx",
                        "context Beta(b) v1\nowns\n  dir \"b\"\n\npublished language b.v1\n  proto \"b/v1/b.proto\"\n  open host service BService\n\nterms\n  kind \"The kind of thing Beta deals with\"\n    means enum Kind\n\ndownstream Alpha supplier\n",
                    ),
                ],
        ),
        e(
            "E306",
            tr!("一緒に書けない役割です", "Roles that cannot go together"),
            tr!("conformist と customer、conformist と anticorruption layer を一緒に書いたとき。", "conformist is written with customer, or with anticorruption layer."),
            tr!("どちらか一つにしてください。", "Keep one of them."),
            &[
                ("甲.ctx", "context 甲(a) v1\nowns\n  dir \"a\"\n\nupstream 乙 conformist, customer\n  through b.v1\n"),
                (
                    "乙.ctx",
                    "context 乙(b) v1\nowns\n  dir \"b\"\n\npublished language b.v1\n  proto \"b/v1/b.proto\"\n  open host service BService\n\nterms\n  種類 \"乙が扱うものの種類\"\n    means enum Kind\n\ndownstream 甲 supplier\n",
                ),
            ],
            &["E303"],
        )
        .en(&[
                    ("alpha.ctx", "context Alpha(a) v1\nowns\n  dir \"a\"\n\nupstream Beta conformist, customer\n  through b.v1\n"),
                    (
                        "beta.ctx",
                        "context Beta(b) v1\nowns\n  dir \"b\"\n\npublished language b.v1\n  proto \"b/v1/b.proto\"\n  open host service BService\n\nterms\n  kind \"The kind of thing Beta deals with\"\n    means enum Kind\n\ndownstream Alpha supplier\n",
                    ),
                ],
        ),
        e(
            "E307",
            tr!("共有カーネルが片側だけか、両側の並びが違います", "A shared kernel is written on one side only, or lists different things"),
            tr!(
                "共有カーネルを片側にしか書いていないとき、両側の並びが違うとき（コピーを両側に置くときを除く）、並べたものが二つのどちらのものでもないとき。",
                "A shared kernel is written on one side only, the two sides list different things (other than each keeping a copy), or what it lists belongs to neither of the two."
            ),
            tr!("両方のファイルに `shared kernel with <相手>` を書き、同じものを並べてください。", "Write `shared kernel with <context>` in both files, with the same entries."),
            &[("甲.ctx", "context 甲(a) v1\nowns\n  dir \"a\"\n\nshared kernel with 乙\n  proto \"a/a.proto\"\n")],
            &["E308", "E202"],
        )
        .en(&[("alpha.ctx", "context Alpha(a) v1\nowns\n  dir \"a\"\n\nshared kernel with Beta\n  proto \"a/a.proto\"\n")]),
        e(
            "E308",
            tr!("共有カーネルのコピーの中身が違います", "The copies of a shared kernel differ"),
            tr!("両側がそれぞれのコピーを共有カーネルに並べているのに、コピーのバイト列が違うとき。", "Each side lists its own copy in the shared kernel, and the copies' bytes differ."),
            tr!("両側のコピーの中身をそろえてください。", "Make the copies the same."),
            &[
                ("甲.ctx", "context 甲(a) v1\nowns\n  dir \"a\"\n\nshared kernel with 乙\n  proto \"a/k/units.proto\"\n"),
                ("a/k/units.proto", "syntax = \"proto3\";\npackage k;\nmessage Yen { int64 amount = 1; }\n"),
                (
                    "乙.ctx",
                    "context 乙(b) v1\nowns\n  dir \"b\"\n\npublished language b.v1\n  proto \"b/v1/b.proto\"\n  open host service BService\n\nterms\n  種類 \"乙が扱うものの種類\"\n    means enum Kind\n\nshared kernel with 甲\n  proto \"b/k/units.proto\"\n",
                ),
                ("b/k/units.proto", "syntax = \"proto3\";\npackage k;\nmessage Yen { int32 amount = 1; }\n"),
            ],
            &["E307"],
        )
        .en(&[
                    ("alpha.ctx", "context Alpha(a) v1\nowns\n  dir \"a\"\n\nshared kernel with Beta\n  proto \"a/k/units.proto\"\n"),
                    ("a/k/units.proto", "syntax = \"proto3\";\npackage k;\nmessage Yen { int64 amount = 1; }\n"),
                    (
                        "beta.ctx",
                        "context Beta(b) v1\nowns\n  dir \"b\"\n\npublished language b.v1\n  proto \"b/v1/b.proto\"\n  open host service BService\n\nterms\n  kind \"The kind of thing Beta deals with\"\n    means enum Kind\n\nshared kernel with Alpha\n  proto \"b/k/units.proto\"\n",
                    ),
                    ("b/k/units.proto", "syntax = \"proto3\";\npackage k;\nmessage Yen { int32 amount = 1; }\n"),
                ],
        ),
        e(
            "E309",
            tr!("パートナーシップが片側だけです", "A partnership is written on one side only"),
            tr!("`partnership with` を片側にしか書いていないとき。", "`partnership with` is written on one side only."),
            tr!("もう片方のファイルにも書いてください。", "Write it in the other file too."),
            &[("甲.ctx", "context 甲(a) v1\nowns\n  dir \"a\"\n\npartnership with 乙\n")],
            &["E201"],
        )
        .en(&[("alpha.ctx", "context Alpha(a) v1\nowns\n  dir \"a\"\n\npartnership with Beta\n")]),
        e(
            "E310",
            tr!("別々の道とほかの関係が両立しません", "Separate ways and another relationship cannot go together"),
            tr!("`separate ways from` を書いた二つのあいだに、ほかの関係もあるとき。", "Two contexts that go separate ways have another relationship too."),
            tr!("どちらかを消してください。", "Delete one of them."),
            &[("甲.ctx", "context 甲(a) v1\nowns\n  dir \"a\"\n\nupstream 乙 conformist\n  through b.v1\n\nseparate ways from 乙\n")],
            &["E206"],
        )
        .en(&[("alpha.ctx", "context Alpha(a) v1\nowns\n  dir \"a\"\n\nupstream Beta conformist\n  through b.v1\n\nseparate ways from Beta\n")]),
        e(
            "E311",
            tr!("自分自身との関係です", "A relationship with itself"),
            tr!("関係の相手が、そのコンテキスト自身のとき。", "A relationship names the context it is written in."),
            tr!("相手の名前を直してください。", "Correct the other context's name."),
            &[("甲.ctx", "context 甲(a) v1\nowns\n  dir \"a\"\n\npartnership with 甲\n")],
            &[],
        )
        .en(&[("alpha.ctx", "context Alpha(a) v1\nowns\n  dir \"a\"\n\npartnership with Alpha\n")]),
        e(
            "E312",
            tr!("上流が `through` の package を公表していません", "The upstream does not publish a package of `through`"),
            tr!("`through` に並べた package が、上流の `published language` に無いとき。", "A package under `through` is not among the upstream's `published language`s."),
            tr!("上流が公表している package を書いてください。", "Write a package the upstream publishes."),
            &[("甲.ctx", "context 甲(a) v1\nowns\n  dir \"a\"\n\nupstream 乙 conformist\n  through b.v9\n")],
            &["E203"],
        )
        .en(&[("alpha.ctx", "context Alpha(a) v1\nowns\n  dir \"a\"\n\nupstream Beta conformist\n  through b.v9\n")]),
        e(
            "E313",
            tr!("腐敗防止層の `layer` が下流のものではありません", "The `layer` of an anticorruption layer is not the downstream's"),
            tr!("`layer` に並べたディレクトリかファイルが、その関係を書いた下流のコンテキストに属さないとき。腐敗防止層は、下流の側に置くものです。", "A directory or file under `layer` does not belong to the downstream that writes the relationship; the layer is on the downstream's side."),
            tr!("下流に属するディレクトリを書いてください。", "Write a directory of the downstream."),
            &[("甲.ctx", "context 甲(a) v1\nowns\n  dir \"a\"\n\nupstream 乙 anticorruption layer\n  through b.v1\n  layer dir \"b\"\n")],
            &[],
        )
        .en(&[("alpha.ctx", "context Alpha(a) v1\nowns\n  dir \"a\"\n\nupstream Beta anticorruption layer\n  through b.v1\n  layer dir \"b\"\n")]),
        e(
            "W301",
            tr!("上流をたどると、元のコンテキストに戻ります", "Following the upstreams comes back where it started"),
            tr!("二つ以上のコンテキストが、たどると互いに上流になっているとき。", "Two or more contexts are, one after another, upstream of each other."),
            tr!("向きを一つにそろえるか、パートナーシップにすることを考えてください。", "Consider one direction, or a partnership."),
            &[
                (
                    "甲.ctx",
                    "context 甲(a) v1\nowns\n  dir \"a\"\n\npublished language a.v1\n  proto \"a/v1/a1.proto\"\n\nupstream 乙 conformist\n  through b.v1\n",
                ),
                A_PUBLISHES,
                (
                    "乙.ctx",
                    "context 乙(b) v1\nowns\n  dir \"b\"\n\npublished language b.v1\n  proto \"b/v1/b.proto\"\n  open host service BService\n\nterms\n  種類 \"乙が扱うものの種類\"\n    means enum Kind\n\nupstream 甲 conformist\n  through a.v1\n",
                ),
            ],
            &[],
        )
        .en(&[
                    (
                        "alpha.ctx",
                        "context Alpha(a) v1\nowns\n  dir \"a\"\n\npublished language a.v1\n  proto \"a/v1/a1.proto\"\n\nupstream Beta conformist\n  through b.v1\n",
                    ),
                    A_PUBLISHES,
                    (
                        "beta.ctx",
                        "context Beta(b) v1\nowns\n  dir \"b\"\n\npublished language b.v1\n  proto \"b/v1/b.proto\"\n  open host service BService\n\nterms\n  kind \"The kind of thing Beta deals with\"\n    means enum Kind\n\nupstream Alpha conformist\n  through a.v1\n",
                    ),
                ],
        ),
        // ── The mappings and the words ──
        e(
            "E401",
            tr!("対応に、上流の列挙の値が抜けています", "A mapping leaves values of the upstream enum out"),
            tr!("腐敗防止層の `enum` の対応に、上流の列挙（proto の列挙か、OpenAPI と AsyncAPI の文書の `enum` のスキーマ）の値で、下流の値も refuse も書いていないものがあるとき。上流が値を足すと、これが出ます。値が無いことを表す値（proto の 0 番の `…_UNSPECIFIED`、文書の `null`）には、対応は要りません。", "An anticorruption layer's `enum` mapping gives neither a value nor refuse for some value of the upstream enum (a proto's enum, or a schema with `enum` of OpenAPI and AsyncAPI documents): what comes when the upstream adds a value. The value that says nothing is set (a proto's value 0 `…_UNSPECIFIED`, a document's `null`) needs none."),
            tr!("値ごとに `<上流の値> -> <下流の値>` か `<上流の値> -> refuse \"<理由>\"` を書いてください。", "Write `<upstream value> -> <value>` or `<upstream value> -> refuse \"<why>\"` for each."),
            &[("甲.ctx", "context 甲(a) v1\nowns\n  dir \"a\"\n\nupstream 乙 anticorruption layer\n  through b.v1\n  enum Kind -> 甲の種類\n    KIND_ONE -> 一つめ\n")],
            &["E402", "W402"],
        )
        .en(&[("alpha.ctx", "context Alpha(a) v1\nowns\n  dir \"a\"\n\nupstream Beta anticorruption layer\n  through b.v1\n  enum Kind -> alpha_kind\n    KIND_ONE -> first\n")]),
        e(
            "E402",
            tr!("対応に、上流の列挙に無い値があります", "A mapping has a value the upstream enum lacks"),
            tr!("`enum` の対応の左辺に、上流の列挙の値でない名前を書いたとき。", "The left side of an `enum` mapping is not a value of the upstream enum."),
            tr!("値の名前を直すか、その行を消してください。", "Correct the value's name, or delete the line."),
            &[(
                "甲.ctx",
                "context 甲(a) v1\nowns\n  dir \"a\"\n\nupstream 乙 anticorruption layer\n  through b.v1\n  enum Kind -> 甲の種類\n    KIND_ONE -> 一つめ\n    KIND_TWO -> 二つめ\n    KIND_THREE -> 三つめ\n",
            )],
            &["E401"],
        )
        .en(&[(
                    "alpha.ctx",
                    "context Alpha(a) v1\nowns\n  dir \"a\"\n\nupstream Beta anticorruption layer\n  through b.v1\n  enum Kind -> alpha_kind\n    KIND_ONE -> first\n    KIND_TWO -> second\n    KIND_THREE -> third\n",
                )],
        ),
        e(
            "E403",
            tr!("対応の先の値が、下流の列挙に無い値です", "A mapping's value is not one of the target enum"),
            tr!("対応の先が proto の列挙なのに、右辺がその列挙の値でないとき。", "The target is a proto enum, and a right side is not one of its values."),
            tr!("右辺を、先の列挙の値にしてください。", "Make the right side a value of the target enum."),
            &[
                (
                    "甲.ctx",
                    "context 甲(a) v1\nowns\n  dir \"a\"\n\npublished language a.v1\n  proto \"a/v1/a1.proto\"\n\nupstream 乙 anticorruption layer\n  through b.v1\n  enum Kind -> enum AKind\n    KIND_ONE -> A_KIND_X\n    KIND_TWO -> A_KIND_Y\n",
                ),
                A_PUBLISHES,
            ],
            &["E401"],
        )
        .en(&[
                    (
                        "alpha.ctx",
                        "context Alpha(a) v1\nowns\n  dir \"a\"\n\npublished language a.v1\n  proto \"a/v1/a1.proto\"\n\nupstream Beta anticorruption layer\n  through b.v1\n  enum Kind -> enum AKind\n    KIND_ONE -> A_KIND_X\n    KIND_TWO -> A_KIND_Y\n",
                    ),
                    A_PUBLISHES,
                ],
        ),
        e(
            "E404",
            tr!("参照している上流の列挙に、腐敗防止層の対応がありません", "An upstream enum referred to has no mapping in the anticorruption layer"),
            tr!("腐敗防止層の下流の成果物が上流の列挙を参照している（使うメッセージからたどれるものも含む）のに、その列挙の `enum` の対応が無いとき。", "Downstream of an anticorruption layer, an artifact refers to an upstream enum (the messages it uses reaching it count), and there is no `enum` mapping for it."),
            tr!("`enum <上流の列挙> -> <先>` と値の行を書いてください。", "Write `enum <upstream enum> -> <target>` and its value lines."),
            &[("甲.ctx", "context 甲(a) v1\nowns\n  dir \"a\"\n\nupstream 乙 anticorruption layer\n  through b.v1\n"), A_USES_B],
            &["E401"],
        )
        .en(&[("alpha.ctx", "context Alpha(a) v1\nowns\n  dir \"a\"\n\nupstream Beta anticorruption layer\n  through b.v1\n"), A_USES_B]),
        e(
            "E405",
            tr!("rulec の規則の取り込みと、`.ctx` に書いた対応が食い違います", "A rule's import and the mapping of the `.ctx` disagree"),
            tr!(
                "対応の先が、上流の列挙を `import proto` で取り込む規則の列挙なのに、`.ctx` の値の行が、規則の取り込み（rulec が渡す、値ごとの proto での名前）と違うとき。",
                "The target is the enum of a rule that takes the upstream enum in with `import proto`, and a value line of the `.ctx` differs from the rule's import (each value's name on the wire, as rulec says it)."
            ),
            tr!("値の行を消して規則の取り込みに任せるか、規則と同じにしてください。", "Delete the value lines and leave the mapping to the rule, or make them the rule's."),
            &[
                ("甲.ctx", "context 甲(a) v1\nowns\n  dir \"a\"\n\nupstream 乙 anticorruption layer\n  through b.v1\n  layer rulec \"a/x.rule\"\n  enum Kind -> rulec \"a/x.rule\" enum 種類\n    KIND_ONE -> 二\n    KIND_TWO -> 二\n"),
                A_RULE_TAKES_KIND,
            ],
            &["E401"],
        )
        .en(&[
                    ("alpha.ctx", "context Alpha(a) v1\nowns\n  dir \"a\"\n\nupstream Beta anticorruption layer\n  through b.v1\n  layer rulec \"a/x.rule\"\n  enum Kind -> rulec \"a/x.rule\" enum kind\n    KIND_ONE -> two\n    KIND_TWO -> two\n"),
                    A_RULE_TAKES_KIND_EN,
                ],
        ),
        e(
            "E406",
            tr!("同じ名前で違う意味の語が、対応の無いまま境界を越えます", "A word of the same name and another meaning crosses unmapped"),
            tr!(
                "上流の語が、その `means` の要素とともに下流に越えてくるのに、下流に同じ名前（`also` を含む）で `as` で取り入れていない語があり、腐敗防止層の対応がそれを読み替えていないとき。",
                "An upstream term crosses into the downstream with the element it `means`, the downstream has a term of the same name (`also` included) not taken with `as`, and no anticorruption layer maps it."
            ),
            tr!("下流の語の名前を変えるか、同じ意味なら `as` で取り入れるか、腐敗防止層にして読み替えてください。", "Rename the downstream's term, take it with `as` if it means the same, or make the relationship an anticorruption layer and map it."),
            &[
                ("甲.ctx", "context 甲(a) v1\nowns\n  dir \"a\"\n\nterms\n  種類 \"甲が客に出す区分\"\n\nupstream 乙 conformist\n  through b.v1\n"),
                A_USES_B,
            ],
            &["E407"],
        )
        .en(&[
                    ("alpha.ctx", "context Alpha(a) v1\nowns\n  dir \"a\"\n\nterms\n  kind \"The kind Alpha gives to customers\"\n\nupstream Beta conformist\n  through b.v1\n"),
                    A_USES_B,
                ],
        ),
        e(
            "E407",
            tr!("違う意味の語を、同じ名前に読み替えています", "A word is mapped to a word of the same name and another meaning"),
            tr!("腐敗防止層で上流の語を読み替えた先の値や語の名前が、下流にある、意味の違う語の名前と同じとき。", "What an anticorruption layer maps an upstream word to has the name of a downstream word of another meaning."),
            tr!("読み替えた先に、違う名前を付けてください。", "Give what it is mapped to another name."),
            &[
                (
                    "甲.ctx",
                    "context 甲(a) v1\nowns\n  dir \"a\"\n\nterms\n  種類 \"甲が客に出す区分\"\n\nupstream 乙 anticorruption layer\n  through b.v1\n  enum Kind -> 種類\n    KIND_ONE -> 一つめ\n    KIND_TWO -> 二つめ\n",
                ),
                A_USES_B,
            ],
            &["E406"],
        )
        .en(&[
                    (
                        "alpha.ctx",
                        "context Alpha(a) v1\nowns\n  dir \"a\"\n\nterms\n  kind \"The kind Alpha gives to customers\"\n\nupstream Beta anticorruption layer\n  through b.v1\n  enum Kind -> kind\n    KIND_ONE -> first\n    KIND_TWO -> second\n",
                    ),
                    A_USES_B,
                ],
        ),
        e(
            "E408",
            tr!("`means` の要素が、そのコンテキストの公表された言語にありません", "What a term `means` is not in the context's published language"),
            tr!("語の `means` が、自分の公表された言語のファイル以外を指すとき。", "A term's `means` points outside the files of the context's own published languages."),
            tr!("自分の公表された言語の要素を指すか、`means` を消してください。", "Point at an element of the context's own published language, or delete the `means`."),
            &[("甲.ctx", "context 甲(a) v1\nowns\n  dir \"a\"\n\nterms\n  品 \"甲が売るもの\"\n    means proto \"b/v1/b.proto\" message Plain\n")],
            &["E007"],
        )
        .en(&[("alpha.ctx", "context Alpha(a) v1\nowns\n  dir \"a\"\n\nterms\n  goods \"What Alpha sells\"\n    means proto \"b/v1/b.proto\" message Plain\n")]),
        e(
            "E409",
            tr!("`term` の対応の語が、用語集にありません", "A word of a `term` mapping is not in the glossary"),
            tr!("`term <上流の語> -> <下流の語>` の左が上流の用語集に、右が下流の用語集に無いとき。", "The left of `term <upstream term> -> <term>` is not in the upstream's glossary, or the right is not in the downstream's."),
            tr!("語の名前を直すか、用語集に語を足してください。", "Correct the word, or add it to the glossary."),
            &[("甲.ctx", "context 甲(a) v1\nowns\n  dir \"a\"\n\nupstream 乙 anticorruption layer\n  through b.v1\n  term 無い語 -> 何か\n")],
            &["E410"],
        )
        .en(&[("alpha.ctx", "context Alpha(a) v1\nowns\n  dir \"a\"\n\nupstream Beta anticorruption layer\n  through b.v1\n  term no_such_word -> something\n")]),
        e(
            "E410",
            tr!("`as` で取り入れる語が使えません", "What `as` takes cannot be taken"),
            tr!("`as` の語が相手の用語集に無いとき、相手と関係が無いとき。", "The term of `as` is not in the other context's glossary, or there is no relationship with that context."),
            tr!("語の名前を直すか、相手との関係を書いてください。", "Correct the term, or write a relationship with the other context."),
            &[("甲.ctx", "context 甲(a) v1\nowns\n  dir \"a\"\n\nterms\n  種類 as 乙.種類\n")],
            &["E007"],
        )
        .en(&[("alpha.ctx", "context Alpha(a) v1\nowns\n  dir \"a\"\n\nterms\n  kind as Beta.kind\n")]),
        e(
            "W401",
            tr!("境界を越えない語です", "A term crosses no boundary"),
            tr!("語が、`means` で自分の公表された言語の要素を指さず、`as` で取り入れたものでもなく、腐敗防止層の対応の先でもなく、上流から越えてくる同じ名前の語ともぶつからないとき。用語集に載せるのは、境界を越える語だけです。", "A term `means` nothing of the context's published language, is not taken with `as`, is not what a layer maps to, and meets no word of the same name crossing from upstream. A glossary holds only the words that cross a boundary."),
            tr!("境界を越える語にするか、用語集から消してください。", "Make it a word that crosses, or delete it from the glossary."),
            &[("甲.ctx", "context 甲(a) v1\nowns\n  dir \"a\"\n\nterms\n  孤立 \"どこにも出ていかない語\"\n")],
            &[],
        )
        .en(&[("alpha.ctx", "context Alpha(a) v1\nowns\n  dir \"a\"\n\nterms\n  stranded \"A word that goes out nowhere\"\n")]),
        e(
            "W402",
            tr!("値が無いことを表す 0 番の値に対応を書いています", "The value 0 that says nothing is set is mapped"),
            tr!("0 番の値で、名前から列挙の接頭辞を外すと unspecified になるものに、対応を書いたとき。その値は、値が設定されていないことを表す値なので、対応は要りません。", "The value 0 whose name, without the enum's prefix, is unspecified is mapped; it marks that nothing is set, and needs no mapping."),
            tr!("その行を消してください。", "Delete the line."),
            &[(
                "甲.ctx",
                "context 甲(a) v1\nowns\n  dir \"a\"\n\nupstream 乙 anticorruption layer\n  through b.v1\n  enum Kind -> 甲の種類\n    KIND_UNSPECIFIED -> 無し\n    KIND_ONE -> 一つめ\n    KIND_TWO -> 二つめ\n",
            )],
            &["E401"],
        )
        .en(&[(
                    "alpha.ctx",
                    "context Alpha(a) v1\nowns\n  dir \"a\"\n\nupstream Beta anticorruption layer\n  through b.v1\n  enum Kind -> alpha_kind\n    KIND_UNSPECIFIED -> nothing\n    KIND_ONE -> first\n    KIND_TWO -> second\n",
                )],
        ),
        // ── Building the settings ──
        e(
            "E501",
            tr!("ツールの設定に書けません", "The settings of a tool cannot be written"),
            tr!(
                "`sakai build` で、地図にその言語の `code` の行が無いとき、その言語のコードを持つコンテキストが無いとき、ディレクトリの名前がその言語のモジュールの名前にならないとき（Python と Java）、コードのファイルを一つだけ指す項があるとき、Python の置き場所の直下にモジュールがあるとき（import-linter は読めない）、Java にデフォルトパッケージのクラスがあるとき、ArchUnit に `test` の置き場所が無いとき、Go の置き場所に go.mod が無いとき。",
                "`sakai build` finds that the map has no `code` line for the language, no context has code in it, a directory's name is not a module's (Python and Java), an entry names one file of code, a Python module sits right in the place of the code (import-linter cannot read it), a Java class is in the default package, ArchUnit has no place for the tests, or the place of the Go code has no go.mod."
            ),
            tr!("注のとおりに、地図の `code` と `owns` か、コードの置き場所を直してください。", "Correct the map's `code` and `owns`, or where the code is, as the note says."),
            &[],
            &["E502"],
        )
        .running(&["build", "地図.ctx", "--target", "import-linter"])
        .en(&[]),
        e(
            "E502",
            tr!("書いてある設定が、いまの地図から書くものと違います", "The settings on the disk differ from what the map writes now"),
            tr!(
                "`build --check` で、ディスクの設定のファイルが、いまの地図から書くものと一字でも違うとき（無いときも）。注には、最初に違う行と、地図から書くとその行がどうなるかが出ます。",
                "`build --check` finds a settings file on the disk that differs from what the map writes now, or none. The note gives the first line that differs and what the map writes there."
            ),
            tr!(
                "`sakai build` で書き直してください。書いたときと違う `--lang` で確かめると、説明の文が違うので、同じ `--lang` を渡してください。",
                "Write it again with `sakai build`. Checked with another `--lang` than it was written with, the words of its descriptions differ: give the same one."
            ),
            &[
                ("地図.ctx", "map 地図(m) v1\nuse context \"甲.ctx\"\nuse context \"乙.ctx\"\ncovers \".\"\ncode python \"py\"\n"),
                ("甲.ctx", "context 甲(a) v1\nowns\n  dir \"a\", \"py\"\n"),
                ("py/k/x.py", "X = 1\n"),
                ("py/.importlinter", "# written by hand\n"),
            ],
            &["E501"],
        )
        .running(&["build", "地図.ctx", "--target", "import-linter", "--check"])
        .en(&[
                    ("map.ctx", "map Map(m) v1\nuse context \"alpha.ctx\"\nuse context \"beta.ctx\"\ncovers \".\"\ncode python \"py\"\n"),
                    ("alpha.ctx", "context Alpha(a) v1\nowns\n  dir \"a\", \"py\"\n"),
                    ("py/k/x.py", "X = 1\n"),
                    ("py/.importlinter", "# written by hand\n"),
                ],
        ),
        // ── Security (ritsu's DESIGN 16) ──
        e(
            "W901",
            tr!("鍵の形の値が `.ctx` に書いてあります", "A value of a key's shape is written in a `.ctx`"),
            tr!(
                "地図か、地図が読む context のファイルに、プロバイダーが形を決めている鍵（AWS のアクセスキー ID、GitHub・Slack・Stripe・OpenAI・Anthropic・Google の鍵やトークン、PEM の秘密鍵）が、文字列でもコメントでも書いてあるとき。sakai は鍵の種類と接頭辞と長さだけを示し、鍵そのものは出しません。OpenAPI と AsyncAPI の文書と `.proto` の鍵は、`ritsu check` が一度だけ言います。",
                "A key whose provider fixes its shape (an AWS access key ID, a key or token of GitHub, Slack, Stripe, OpenAI, Anthropic or Google, a PEM private key) is written in the map or a context file it reads, in a string or a comment. sakai gives the key's kind, prefix and length, never the key. The keys of the OpenAPI and AsyncAPI documents and the `.proto` files are said once, by `ritsu check`."
            ),
            tr!(
                "鍵をファイルから消し、コードが動くところ（環境変数、シークレットの置き場）から読んでください。本物の鍵なら、まずプロバイダーで無効にしてください（リポジトリの履歴に残ります）。テスト用の値なら、同じ行のコメントに `ritsu: test secret` と書いてください。",
                "Take the key out of the file, and read it where the code runs (an environment variable, a secret store). If it is real, revoke it with its provider first: the history of the repository keeps it. For a value for tests, write `ritsu: test secret` in a comment on the same line."
            ),
            &[A_WITH_A_KEY],
            &[],
        )
        .en(&[A_WITH_A_KEY_EN]),
        e(
            "W902",
            tr!("文書のサーバーが、通信を暗号化しません", "A server of a document does not encrypt the connection"),
            tr!(
                "地図の OpenAPI か AsyncAPI の文書のサーバーが、ループバック（localhost、127.0.0.0/8、::1）の外へ、暗号化しない通信をするとき。OpenAPI では `url` が `http://` か `ws://` のサーバー（文書、パスの項、操作のもの。サーバー変数には既定の値と `enum` の値を一つずつ入れて見ます。相対の `url` は見ません）、AsyncAPI では `protocol` が `http`・`ws`・`amqp`・`mqtt`（`mqtt5`）・`stomp`・`kafka` のサーバーです。",
                "A server of an OpenAPI or AsyncAPI document of the map talks to a host off this machine (not localhost, 127.0.0.0/8 or ::1) without encryption: in OpenAPI, a `url` of `http://` or `ws://` (of the document, a path item or an operation; a server variable is given its default and each value of its `enum`; a relative `url` is passed over); in AsyncAPI, a `protocol` of `http`, `ws`, `amqp`, `mqtt` (`mqtt5`), `stomp` or `kafka`."
            ),
            tr!(
                "暗号化して通信するプロトコル（https、wss、amqps、secure-mqtt、stomps、kafka-secure）にしてください。ほかの仕組み（サービスメッシュ、プライベートな接続など）で守っているなら、サーバーに `x-ritsu-plaintext: \"<理由>\"` と書いてください。",
                "Use the encrypted form (https, wss, amqps, secure-mqtt, stomps, kafka-secure). If the connection is protected another way (a service mesh, a private link), write `x-ritsu-plaintext: \"<why>\"` in the server."
            ),
            &[B_EVENTS_ON_KAFKA],
            &["W903"],
        )
        .en(&[B_EVENTS_ON_KAFKA]),
        e(
            "W903",
            tr!("公表された言語の操作に、認証の指定がありません", "An operation of a published language says no authentication"),
            tr!(
                "公表された言語の OpenAPI の文書の操作（`webhooks` のものを除く）に、操作にも文書にも `security` が無いとき。AsyncAPI の文書では、チャネルが使うサーバーに `security` が無く、そのチャネルの操作にも無いとき（文書に `servers` が無ければ見ません）。どの操作をだれに許すか（認可）は、ここでは見ません。",
                "An operation of a published language's OpenAPI document (not a webhook) has no `security`, and neither has the document; or, in an AsyncAPI document, a server a channel is on has no `security`, and no operation on the channel has it either (a document with no `servers` is not looked at). Which operation is allowed to whom is not looked at here."
            ),
            tr!(
                "操作か文書（AsyncAPI ではサーバーか操作）に `security` を書いてください。だれでも呼べるようにわざとしているなら、`security: []` と書いてください。",
                "Write `security` on the operation or the document (in AsyncAPI, on the server or an operation). If it is open to anyone on purpose, write `security: []`."
            ),
            &[B_PUBLISHES_AN_API, B_API_WITHOUT_SECURITY],
            &["W902"],
        )
        .en(&[B_PUBLISHES_AN_API_EN, B_API_WITHOUT_SECURITY]),
    ];
    Ledger {
        tool: "sakai",
        example_file: "",
        fence: "ctx",
        repro_heading: tr!("再現", "Reproduction"),
        later_text: tr!("(sakai はこのコードをまだ出さないので、再現はありません)", "(sakai does not print this code yet; it has no reproduction)"),
        later_markdown: tr!("sakai はこのコードをまだ出さないので、再現はありません", "sakai does not print this code yet; it has no reproduction"),
        entries,
    }
}

/// The entry of a code, written in either case.
pub fn find(code: &str) -> Option<Entry> {
    ledger().find(code).cloned()
}

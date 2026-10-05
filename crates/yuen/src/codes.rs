//! The ledger of every diagnostic code (DESIGN 6.2). `yuen explain` reads it, and the tests
//! run every entry's example and require its code to come out, so the example cannot go stale
//! while the prose around it still reads well.

use ritsu_base::ledger::{Entry, Ledger, Repro};
use ritsu_base::text::Text;

/// An entry whose example is the smallest `.req` that gets the code, with what has to be beside
/// it. Some need another language to come out (a rule, a calendar): those are checked with every
/// language joined, as `ritsu yuen` runs them. A retired code keeps its entry, and its number is
/// given to nothing else ([`Entry::retired`], ritsu's DESIGN 7.10).
fn e(code: &'static str, title: Text, when: Text, fix: Text, example: &'static str, related: &'static [&'static str]) -> Entry {
    Entry::new(code, title, when, fix, Repro::File { body: example, beside: &[] }, related)
}

// ── What the examples need beside them ───────────────────────────────────

const COPY_142: (&str, &[u8]) = (
    "sources/law/129AC0000000089@2026-10-01/MainProvision-Article_142.xml",
    include_bytes!("../tests/fixtures/period/sources/law/129AC0000000089@2026-10-01/MainProvision-Article_142.xml"),
);
const NOT_XML: (&str, &[u8]) = ("sources/law/129AC0000000089@2026-10-01/MainProvision-Article_142.xml", b"not xml\n");
const A_TXT: (&str, &[u8]) = ("a.txt", b"a\n");
const B_TXT: (&str, &[u8]) = ("b.txt", b"b\n");
/// What `r1` said when the link was looked at: `text the old x`.
const OLD_X: (&str, &[u8]) = ("reviewed/f1e653e8ce72c16f", b"text the old x\n");
/// What `a.txt` was when the link was looked at.
const OLD_A: (&str, &[u8]) = ("reviewed/0263829989b6fd95", b"b\n");

// ── What the examples of the codes that read another language need beside them ──

const E106_CAL: (&str, &[u8]) = ("a.cal", include_bytes!("../tests/mutants/E106_宣言されていない出典を借りる/a.cal"));
const CHANGED_142: (&str, &[u8]) = (
    "sources/law/129AC0000000089@2026-10-01/MainProvision-Article_142.xml",
    include_bytes!("../tests/mutants/E107_同じ条の違う本文/sources/law/129AC0000000089@2026-10-01/MainProvision-Article_142.xml"),
);
const E107_CAL: (&str, &[u8]) = ("cal/a.cal", include_bytes!("../tests/mutants/E107_同じ条の違う本文/cal/a.cal"));
const E107_CAL_142: (&str, &[u8]) = (
    "cal/sources/law/129AC0000000089@2026-10-01/MainProvision-Article_142.xml",
    include_bytes!("../tests/mutants/E107_同じ条の違う本文/cal/sources/law/129AC0000000089@2026-10-01/MainProvision-Article_142.xml"),
);
const E202_PROTO: (&str, &[u8]) = ("a.proto", include_bytes!("../tests/mutants/E202_無いメッセージ/a.proto"));
const E203_RULE: (&str, &[u8]) = ("a.rule", include_bytes!("../tests/mutants/E203_検査を通らない規則/a.rule"));
const E205_PROTO: (&str, &[u8]) = ("a.proto", include_bytes!("../tests/mutants/E205_読めないproto/a.proto"));
const E206_RULE: (&str, &[u8]) = ("fee.rule", include_bytes!("../tests/mutants/E206_rulec-not-joined/fee.rule"));

// ── The same, for the examples in English (the eCFR for the law, English names) ──

const COPY_1_7: (&str, &[u8]) = (
    "sources/law/37-CFR-1@2026-01-01/1.7.xml",
    include_bytes!("../tests/fixtures/period_of_months/sources/law/37-CFR-1@2026-01-01/1.7.xml"),
);
const NOT_XML_EN: (&str, &[u8]) = ("sources/law/37-CFR-1@2026-01-01/1.7.xml", b"not xml\n");
const E106_CAL_EN: (&str, &[u8]) = ("a.cal", include_bytes!("../tests/mutants/E106_borrows_an_undeclared_source/a.cal"));
const E107_RULE_EN: (&str, &[u8]) = ("rules/osha_extinguisher.rule", include_bytes!("../tests/mutants/E107_same_article_different_text/rules/osha_extinguisher.rule"));
const E107_RULE_COPY_EN: (&str, &[u8]) = (
    "rules/sources/law/29-CFR-1910@2026-01-01/1910.157.xml",
    include_bytes!("../tests/mutants/E107_same_article_different_text/rules/sources/law/29-CFR-1910@2026-01-01/1910.157.xml"),
);
const E107_COPY_EN: (&str, &[u8]) = (
    "sources/law/29-CFR-1910@2026-01-01/1910.157.xml",
    include_bytes!("../tests/mutants/E107_same_article_different_text/sources/law/29-CFR-1910@2026-01-01/1910.157.xml"),
);
const E202_PROTO_EN: (&str, &[u8]) = ("a.proto", include_bytes!("../tests/mutants/E202_no_such_message/a.proto"));
const E203_RULE_EN: (&str, &[u8]) = ("a.rule", include_bytes!("../tests/mutants/E203_rule_that_does_not_pass/a.rule"));
const E205_PROTO_EN: (&str, &[u8]) = ("a.proto", include_bytes!("../tests/mutants/E205_unreadable_proto/a.proto"));

/// The example of an entry in English: the same kind of `.req` as the Japanese one, in names of
/// English and, for a law, in the eCFR. `explain` shows it with `--lang en`; `--lang ja` shows the
/// Japanese one, as it did.
trait English {
    fn en(self, body: &'static str, beside: &'static [(&'static str, &'static [u8])]) -> Entry;
}

impl English for Entry {
    fn en(self, body: &'static str, beside: &'static [(&'static str, &'static [u8])]) -> Entry {
        self.english(Repro::File { body, beside })
    }
}

pub fn ledger() -> Ledger {
    let entries = vec![
        // ── Words and lines ──
        e(
            "E001",
            tr!("読めない字句があります", "Something cannot be read as a word of the language"),
            tr!(
                "yuen で使えない文字、閉じていない文字列、`\\\"` と `\\\\` のほかのエスケープ、形の崩れた日付・ハッシュ・版・別名、ASCII の数字で始まる ASCII の名前、全角の空白があるとき。",
                "A character the language does not have, a string not closed, an escape other than `\\\"` and `\\\\`, a date, hash, version or alias of the wrong shape, an ASCII name that starts with a digit, or a full-width space."
            ),
            tr!(
                "示された位置を直してください。文字列は `\"…\"` で閉じ、日付は `2026-10-03`、ハッシュは `sha256:` と 16 桁の小文字の 16 進数、別名は `(payment_day)` の形で書いてください。",
                "Correct it where it points: close the string with `\"`; write a date as `2026-10-03`, a hash as `sha256:` and 16 lowercase hex digits, an alias as `(payment_day)`."
            ),
            "requirements 例 v1\ndescription \"閉じていない\n",
            &["E002"],
        )
        .en("requirements example v1\ndescription \"not closed\n", &[]),
        e(
            "E002",
            tr!("この位置に書けない語があります", "A word is written where it does not belong"),
            tr!(
                "yuen に無い行、行の中で構文が受け付けない語、足りない語、名前に使った yuen の語（`text` など）があるとき。",
                "A line that does not exist, a word the syntax does not take there, a word missing, or a word of the language (such as `text`) used as a name."
            ),
            tr!("注に挙がる書き方のどれかにしてください。", "Use one of the forms the note gives."),
            "requirements 例 v1\nrole 法務\n\nrequirement r1\n  text \"x\"\n  owner 法務\n  because \"y\"\n",
            &["E004"],
        )
        .en("requirements example v1\nrole legal\n\nrequirement r1\n  text \"x\"\n  owner legal\n  because \"y\"\n", &[]),
        e(
            "E003",
            tr!("ファイルが `requirements` の行で始まっていません", "The file does not start with a `requirements` line"),
            tr!("コメントと空行を除いた最初の行が `requirements …` でないとき、ファイルが空のとき。", "The first line that is not a comment or blank is not `requirements …`, or the file is empty."),
            tr!("`requirements 民法の期間 v1` のように、要件の集まりの名前と版で書き始めてください。", "Start with the name and version of the set of requirements, like `requirements payment_terms v1`."),
            "role 法務\n",
            &["E004"],
        )
        .en("role legal\n", &[]),
        e(
            "E004",
            tr!("節や行の順序か数が違います", "A section or a line is out of order, or there are too many"),
            tr!(
                "ファイルの節が `requirements`、`description`、`role`、`source`、`scope`、`requirement` の順にないとき、要件の中の行が `text`、`in force`、`owner`、`replaces`、`from`、`decided`、`satisfied by`・`not satisfied`、`verified by`・`not verified` の順にないとき、一つだけの行（`description`、`text`、`in force`、`owner`、確かめた記録）が二つあるとき。",
                "The sections are not in the order `requirements`, `description`, `role`, `source`, `scope`, `requirement`; the lines of a requirement are not in the order `text`, `in force`, `owner`, `replaces`, `from`, `decided`, `satisfied by` and `not satisfied`, `verified by` and `not verified`; or a line that comes once (`description`, `text`, `in force`, `owner`, a record) comes twice."
            ),
            tr!("決まった順序に並べ替え、二つめを消してください。", "Put them in their order, and delete the second one."),
            "requirements 例 v1\nrole 法務\ndescription \"役割より後ろ\"\n",
            &["E002"],
        )
        .en("requirements example v1\nrole legal\ndescription \"after the role\"\n", &[]),
        e(
            "E005",
            tr!("字下げが合いません", "The indentation does not line up"),
            tr!(
                "字下げにタブがあるとき、同じブロックの行の字下げがそろっていないとき、字下げした行を続けられないところに字下げした行があるとき、確かめた記録の行がリンクか見送りの行のすぐ下に一段深く書かれていないとき。",
                "The indentation has a tab, the lines of a block are not indented alike, an indented line follows nothing that takes indented lines, or a record is not right under its link or waiver, indented deeper."
            ),
            tr!("字下げにはスペースを使い、同じブロックの行はそろえ、記録はリンクの行より深く字下げしてください。", "Indent with spaces, every line of a block alike, and a record deeper than its link."),
            "requirements 例 v1\nrole 法務\n\nrequirement r1\n\ttext \"x\"\n",
            &[],
        )
        .en("requirements example v1\nrole legal\n\nrequirement r1\n\ttext \"x\"\n", &[]),
        e(
            "E006",
            tr!("無い日付か、終わりが始まりより前の期間が書かれています", "A date that does not exist, or a period that ends before it starts"),
            tr!("`2026-02-30` のように暦に無い日付を書いたとき、期間の終わりが始まりより前のとき。書ける日付は 0001-01-01〜9999-12-31 です。", "A date no calendar has, like `2026-02-30`, or a period whose end is before its start. Dates run from 0001-01-01 to 9999-12-31."),
            tr!("暦にある日付に直し、期間は始まりを先に書いてください。", "Write a date that exists, and a period start first."),
            "requirements 例 v1\nrole 法務\n\nrequirement r1\n  text \"x\"\n  owner 法務\n  decided 2026-02-30 by 法務 \"例\"\n",
            &["E001"],
        )
        .en("requirements example v1\nrole legal\n\nrequirement r1\n  text \"x\"\n  owner legal\n  decided 2026-02-30 by legal \"example\"\n", &[]),
        // ── Names ──
        e(
            "E007",
            tr!("同じ名前を二度宣言しています", "A name is declared twice"),
            tr!(
                "要件（版を書かずに二つ）、要件の別名、役割、同じファイルの出典、ファイルの見出しの名前のどれかが二度出てくるとき、一つの要件に別名が二つあるとき。",
                "A requirement (twice, without versions), an alias, a role, a source of the same file or the heading of a file comes twice, or a requirement has two aliases."
            ),
            tr!("どちらかの名前を変えてください。同じ要件の版なら `v1`、`v2` と書き分けてください。", "Rename one of them; if they are versions of one requirement, write `v1` and `v2`."),
            "requirements 例 v1\nrole 法務\nrole 法務\n",
            &["E009"],
        )
        .en("requirements example v1\nrole legal\nrole legal\n", &[]),
        e(
            "E008",
            tr!("宣言されていない名前です", "A name that is not declared"),
            tr!(
                "`owner`、`decided … by`、確かめた記録の `by`、`review --by` の役割が宣言されていないとき、`from` と `replaces` の要件がプロジェクトに無いとき。",
                "The role of an `owner`, a `decided … by`, the `by` of a record or `review --by` is not declared, or the requirement of a `from` or `replaces` is not in the project."
            ),
            tr!("`role 法務 \"…\"` で役割を宣言するか、名前の書き間違いを直してください。", "Declare the role with `role legal \"…\"`, or correct the misspelling."),
            "requirements 例 v1\nrole 法務\n\nrequirement r1\n  text \"x\"\n  owner 法務部\n  decided 2026-10-03 by 法務 \"例\"\n",
            &["E007"],
        )
        .en("requirements example v1\nrole legal\n\nrequirement r1\n  text \"x\"\n  owner legal_dept\n  decided 2026-10-03 by legal \"example\"\n", &[]),
        e(
            "E009",
            tr!("版の書き方が違います", "A version is wrong"),
            tr!(
                "`v0` と書いたとき、同じ版が二つあるとき、版が二つ以上ある要件の版で版を書かなかったとき、版が二つ以上ある要件を版を書かずに指したとき、無い版を指したとき。",
                "A `v0`; the same version twice; a version of a requirement with more than one that does not write its version; pointing at a requirement with more than one version without saying which; or pointing at a version that does not exist."
            ),
            tr!("版の番号は `v1` から始まります。版が二つ以上あるなら、どの版にも版を書き、その要件を指すときにも版を書いてください。", "Versions count from `v1`; when there are two or more, every version writes its own, and so does whatever points at one."),
            "requirements 例 v1\nrole 法務\n\nrequirement r1 v0\n  text \"x\"\n  owner 法務\n  decided 2026-10-03 by 法務 \"例\"\n",
            &["E007", "E408"],
        )
        .en("requirements example v1\nrole legal\n\nrequirement r1 v0\n  text \"x\"\n  owner legal\n  decided 2026-10-03 by legal \"example\"\n", &[]),
        e(
            "E010",
            tr!("要件に要るものがありません", "A requirement lacks something it needs"),
            tr!(
                "要件に `text` か `owner` が無いとき、出どころ（`from` も `decided` も）が無いとき、名前が ASCII の小文字・数字・`_` でない要件に別名が無いとき。",
                "A requirement has no `text` or no `owner`, no origin (neither `from` nor `decided`), or no alias although its name is not lowercase ASCII, digits and `_`."
            ),
            tr!("足りない行を書いてください。別名は `支払日(payment_day)` のように、名前のすぐあとに付けてください。", "Write the line missing; an alias goes right after the name, as in `PaymentDay(payment_day)`."),
            "requirements 例 v1\nrole 法務\n\nrequirement r1\n  owner 法務\n  decided 2026-10-03 by 法務 \"例\"\n",
            &[],
        )
        .en("requirements example v1\nrole legal\n\nrequirement r1\n  owner legal\n  decided 2026-10-03 by legal \"example\"\n", &[]),
        e(
            "E011",
            tr!("名指しの最初の語が、ツールの語ではありません", "A tool that does not exist"),
            tr!(
                "名指しの最初の語が、rulec、dandori、koyomi、chobo、geas、proto、file、yuen、sakai のどれでもないとき（`dir` も、名指しの語ではありません）。",
                "The first word of a naming is none of rulec, dandori, koyomi, chobo, geas, proto, file, yuen and sakai (`dir` is not a word of a naming either)."
            ),
            tr!("九つのどれかを書いてください。ほかのファイルは `file \"…\"` で名指してください。", "Write one of the nine; name any other file with `file \"…\"`."),
            "requirements 例 v1\nrole 法務\n\nrequirement r1\n  text \"x\"\n  owner 法務\n  decided 2026-10-03 by 法務 \"例\"\n  satisfied by excel \"a.xlsx\"\n",
            &["E012", "E013"],
        )
        .en("requirements example v1\nrole legal\n\nrequirement r1\n  text \"x\"\n  owner legal\n  decided 2026-10-03 by legal \"example\"\n  satisfied by excel \"a.xlsx\"\n", &[]),
        e(
            "E012",
            tr!("そこに書けない種類か組があります", "A kind or pair that cannot be written there"),
            tr!(
                "ツールに無い種類を書いたとき、子の種類（`method`、`field`、`value`）が親のすぐあとにないとき、子の組や、入れ子の無いツールの組が二つあるとき、file に種類を書いたとき、種類のあとに名前が無いとき。`source` や `yuen` の名指しをリンクや範囲に書いたとき、借りた出典が rulec か koyomi の `source` でないときにも出ます。",
                "A kind the tool does not have, a child kind (`method`, `field`, `value`) not right after its parent, two child pairs, two pairs for a tool without nesting, a kind for file, or a kind without its name; also `source` or a `yuen` naming in a link or a scope, and a borrowed source that is not the `source` of a rulec or koyomi file."
            ),
            tr!("診断の文に挙がる種類のどれかを書いてください。file はファイルを丸ごと名指します（`file \"src/app.py\"`）。", "Write one of the kinds the message gives; file names a whole file (`file \"src/app.py\"`)."),
            "requirements 例 v1\nrole 法務\n\nrequirement r1\n  text \"x\"\n  owner 法務\n  decided 2026-10-03 by 法務 \"例\"\n  satisfied by dandori \"order.flow\" table reserve\n",
            &["E011", "E013"],
        )
        .en("requirements example v1\nrole legal\n\nrequirement r1\n  text \"x\"\n  owner legal\n  decided 2026-10-03 by legal \"example\"\n  satisfied by dandori \"order.flow\" table reserve\n", &[]),
        e(
            "E013",
            tr!("パスの書き方が違います", "A path that cannot be read"),
            tr!("パスに引用符が無いとき、空のとき、絶対パスのとき、`..` を解いたあとでルートの外に出るとき。", "A path without quotes, empty, absolute, or outside the root once collapsed."),
            tr!("名指しを書いたファイルのディレクトリからの相対パスを、`\"…\"` で囲んで書いてください。", "Write it in quotes, from the directory of the file the naming is in."),
            "requirements 例 v1\nrole 法務\n\nrequirement r1\n  text \"x\"\n  owner 法務\n  decided 2026-10-03 by 法務 \"例\"\n  satisfied by file \"/etc/hosts\"\n",
            &["E011", "E012"],
        )
        .en("requirements example v1\nrole legal\n\nrequirement r1\n  text \"x\"\n  owner legal\n  decided 2026-10-03 by legal \"example\"\n  satisfied by file \"/etc/hosts\"\n", &[]),
        // ── Sources ──
        e(
            "E101",
            tr!("出典のコピーがありません", "The copy of a source is not there"),
            tr!("固定した条のコピー（`sources/law/<ID>@<日付>/<要素>.xml`）か、`file` の出典のファイルが無いとき。check は通信しません。", "The copy of a pinned article (`sources/law/<id>@<date>/<element>.xml`), or the file of a `file` source, is not there. check never reads the network."),
            tr!("`yuen source fetch` でコピーを取ってくるか、パスを直してください。", "Bring the copy with `yuen source fetch`, or correct the path."),
            "requirements 例 v1\nrole 法務\n\nsource 民法 = law \"129AC0000000089\" asof 2026-10-01\n  第142条 sha256:fc8c35a0769d3b35\n\nrequirement r1\n  text \"x\"\n  owner 法務\n  from @民法 第142条\n  not satisfied \"例なので置かない\"\n  not verified \"例なので置かない\"\n",
            &["E102", "E103"],
        )
        .en("requirements example v1\nrole legal\n\nsource cfr = law ecfr \"37 CFR 1\" asof 2026-01-01\n  \"§1.7\" sha256:01de176ebe4740d7\n\nrequirement r1\n  text \"x\"\n  owner legal\n  from @cfr \"§1.7\"\n  not satisfied \"left out in this example\"\n  not verified \"left out in this example\"\n", &[]),
        e(
            "E102",
            tr!("出典が固定されていません", "A source is not pinned"),
            tr!("引いている条に固定の行が無いとき、固定の行や `file` の出典に `sha256:` が無いとき。", "An article cited has no pin line, or a pin line or a `file` source has no `sha256:`."),
            tr!("コピーの SHA-256 の先頭 16 桁を書いてください（直した行が注に出ます。`yuen source pin` でも書けます）。", "Write the first 16 digits of the SHA-256 of the copy (the fixed line is shown; `yuen source pin` writes it too)."),
            "requirements 例 v1\nrole 法務\n\nsource 民法 = law \"129AC0000000089\" asof 2026-10-01\n  第142条\n\nrequirement r1\n  text \"x\"\n  owner 法務\n  from @民法 第142条\n  not satisfied \"例なので置かない\"\n  not verified \"例なので置かない\"\n",
            &["E101", "E103"],
        )
        .beside(&[COPY_142])
        .en("requirements example v1\nrole legal\n\nsource cfr = law ecfr \"37 CFR 1\" asof 2026-01-01\n  \"§1.7\"\n\nrequirement r1\n  text \"x\"\n  owner legal\n  from @cfr \"§1.7\"\n  not satisfied \"left out in this example\"\n  not verified \"left out in this example\"\n", &[COPY_1_7]),
        e(
            "E103",
            tr!("コピーが固定と違います", "A copy does not match its pin"),
            tr!("コピーのハッシュが、固定の行の `sha256:` と違うとき。固定したあとでコピーが変わっています。", "The hash of the copy differs from the `sha256:` of its pin: the copy changed after it was pinned."),
            tr!("何が変わったかを読んでから（`yuen source outdated`）、固定を書き換えてください。", "Read what changed (`yuen source outdated`), then pin it again."),
            "requirements 例 v1\nrole 法務\n\nsource 民法 = law \"129AC0000000089\" asof 2026-10-01\n  第142条 sha256:0000000000000000\n\nrequirement r1\n  text \"x\"\n  owner 法務\n  from @民法 第142条\n  not satisfied \"例なので置かない\"\n  not verified \"例なので置かない\"\n",
            &["E102", "E302"],
        )
        .beside(&[COPY_142])
        .en("requirements example v1\nrole legal\n\nsource cfr = law ecfr \"37 CFR 1\" asof 2026-01-01\n  \"§1.7\" sha256:0000000000000000\n\nrequirement r1\n  text \"x\"\n  owner legal\n  from @cfr \"§1.7\"\n  not satisfied \"left out in this example\"\n  not verified \"left out in this example\"\n", &[COPY_1_7]),
        e(
            "E104",
            tr!("コピーが読めません", "A copy cannot be read"),
            tr!("法令のコピーが UTF-8 の XML でないか、e-Gov や eCFR が配る形（条なら `<Article>`、eCFR の section なら `<DIV8>` で始まる）でないとき。", "The copy of a law is not UTF-8 XML, or not what e-Gov or the eCFR serves (an article starts with `<Article>`, an eCFR section with `<DIV8>`)."),
            tr!("コピーは手で直さず、`yuen source fetch` で取り直してください。", "Fetch it again with `yuen source fetch` rather than editing it."),
            "requirements 例 v1\nrole 法務\n\nsource 民法 = law \"129AC0000000089\" asof 2026-10-01\n  第142条 sha256:6210aedce8fd1601\n\nrequirement r1\n  text \"x\"\n  owner 法務\n  from @民法 第142条\n  not satisfied \"例なので置かない\"\n  not verified \"例なので置かない\"\n",
            &["E101"],
        )
        .beside(&[NOT_XML])
        .en("requirements example v1\nrole legal\n\nsource cfr = law ecfr \"37 CFR 1\" asof 2026-01-01\n  \"§1.7\" sha256:6210aedce8fd1601\n\nrequirement r1\n  text \"x\"\n  owner legal\n  from @cfr \"§1.7\"\n  not satisfied \"left out in this example\"\n  not verified \"left out in this example\"\n", &[NOT_XML_EN]),
        e(
            "E105",
            tr!("引用が使えません", "A citation cannot be used"),
            tr!(
                "引用の条が読めない形のとき、同じファイルで宣言されていない出典を引いたとき、法令を条なしで引いたとき、`file` の出典に条を書いたとき。",
                "The article of a citation is not in a form read, the source is not declared in the same file, a law is cited without an article, or a `file` source is cited with one."
            ),
            tr!("出典を同じファイルで宣言し、法令は `@民法 第142条` のように条で、`file` の出典は `@約款` と丸ごと引いてください。", "Declare the source in the same file; cite a law by article or section (`@cfr \"§1.7\"`) and a `file` source whole (`@terms`)."),
            "requirements 例 v1\nrole 法務\n\nrequirement r1\n  text \"x\"\n  owner 法務\n  from @商法 第1条\n  not satisfied \"例なので置かない\"\n  not verified \"例なので置かない\"\n",
            &["E102"],
        )
        .en("requirements example v1\nrole legal\n\nrequirement r1\n  text \"x\"\n  owner legal\n  from @usc \"§1.1\"\n  not satisfied \"left out in this example\"\n  not verified \"left out in this example\"\n", &[]),
        e(
            "E106",
            tr!("借りた出典が使えません", "A borrowed source cannot be used"),
            tr!(
                "借りた出典を、名指したファイルが宣言していないか、引いた条を固定していないとき。ファイルが無いとき、コピーが読めないか固定と違うときにも出ます。",
                "The file named declares no such source, or does not pin the article cited; also when the file is not there, or a copy cannot be read or does not match its pin."
            ),
            tr!("そのファイルが宣言して固定している出典と条を書いてください。ほかの条を引くなら、そのファイルに固定の行を足してください。", "Write a source and an article the file declares and pins; to cite another article, add its pin to that file."),
            include_str!("../tests/mutants/E106_宣言されていない出典を借りる/例.req"),
            &["E105", "E203"],
        )
        .beside(&[E106_CAL])
        .en(include_str!("../tests/mutants/E106_borrows_an_undeclared_source/example.req"), &[E106_CAL_EN]),
        e(
            "E107",
            tr!("要件と成果物が、同じ条の違う本文を読んでいます", "A requirement and what meets it read different texts of one article"),
            tr!(
                "要件が引く条を、それを満たす規則かカレンダーのファイルも固定していて、どのコピーも要件のコピーと本文が違うとき。どちらかが古いコピーです。",
                "A rule or a calendar that meets a requirement pins an article the requirement cites, and none of its copies has the text of the requirement's copy: one of them is old."
            ),
            tr!("本文の差分を読み、古いほうのコピーを取り直して固定し直してください。", "Read the diff of the texts, then fetch and pin the older copy again."),
            include_str!("../tests/mutants/E107_同じ条の違う本文/例.req"),
            &["E103"],
        )
        .beside(&[CHANGED_142, E107_CAL, E107_CAL_142])
        .en(include_str!("../tests/mutants/E107_same_article_different_text/fire_extinguishers.req"), &[E107_RULE_EN, E107_RULE_COPY_EN, E107_COPY_EN]),
        e(
            "W101",
            tr!("固定した条が、どの要件からも引かれていません", "A pinned article is cited by no requirement"),
            tr!("出典の下に固定の行があるのに、同じファイルのどの要件の `from` もその条を引いていないとき。", "A pin line under a source, and no `from` of a requirement of the same file cites the article."),
            tr!("引用を消したあとの残りなら、固定の行を消してください。", "If it is what is left after a citation was removed, delete the pin line."),
            "requirements 例 v1\nrole 法務\n\nsource 民法 = law \"129AC0000000089\" asof 2026-10-01\n  第142条 sha256:fc8c35a0769d3b35\n\nrequirement r1\n  text \"x\"\n  owner 法務\n  decided 2026-10-03 by 法務 \"例\"\n  not satisfied \"例なので置かない\"\n    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f\n  not verified \"例なので置かない\"\n    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f\n",
            &["E102"],
        )
        .beside(&[COPY_142])
        .en("requirements example v1\nrole legal\n\nsource cfr = law ecfr \"37 CFR 1\" asof 2026-01-01\n  \"§1.7\" sha256:01de176ebe4740d7\n\nrequirement r1\n  text \"x\"\n  owner legal\n  decided 2026-10-03 by legal \"example\"\n  not satisfied \"left out in this example\"\n    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f\n  not verified \"left out in this example\"\n    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f\n", &[COPY_1_7]),
        // ── Artifacts ──
        e(
            "E201",
            tr!("成果物のファイルがありません", "The file of an artifact is not there"),
            tr!("リンクが名指すファイルか、範囲のパスが無いとき。リンクにディレクトリを書いたときにも出ます。", "The file a link names, or the path of a scope, is not there; also a directory written in a link."),
            tr!("パスを直してください。ファイルの名前を変えたのなら、リンクも直してください。", "Correct the path; if the file was renamed, correct the link."),
            "requirements 例 v1\nrole 法務\n\nrequirement r1\n  text \"x\"\n  owner 法務\n  decided 2026-10-03 by 法務 \"例\"\n  satisfied by file \"missing.txt\"\n  not verified \"例なので置かない\"\n    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f\n",
            &["E013"],
        )
        .en("requirements example v1\nrole legal\n\nrequirement r1\n  text \"x\"\n  owner legal\n  decided 2026-10-03 by legal \"example\"\n  satisfied by file \"missing.txt\"\n  not verified \"left out in this example\"\n    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f\n", &[]),
        e(
            "E202",
            tr!("成果物の名前が、そのファイルにありません", "The name of an artifact is not in its file"),
            tr!(
                "名指した名前が、そのファイルに無いとき（その言語が渡す名前にも、`.proto` の中にも無いとき）。別名で書いたときや、名前が変わったときにも出ます（注に候補が出ます）。",
                "The name is not in the file (among the things its language gives, or in the `.proto`): written by its alias, or renamed (the candidates are given)."
            ),
            tr!("別名ではなく、その言語の名前で書いてください。名前が変わったのなら、リンクも直してください。", "Write the name the language gives (not its alias); if it was renamed, correct the link."),
            include_str!("../tests/mutants/E202_無いメッセージ/例.req"),
            &["E201"],
        )
        .beside(&[E202_PROTO])
        .en(include_str!("../tests/mutants/E202_no_such_message/example.req"), &[E202_PROTO_EN]),
        e(
            "E203",
            tr!("名指したものの言語から、そのファイルの情報を得られません", "The language of what is named cannot answer for its file"),
            tr!(
                "名指したファイルが、その言語の検査を通らないか、読めないとき。そのようなファイルからは成果物の定義を読み取れないので、yuen はハッシュを取れません。注には、その言語の診断が並びます。",
                "The file named does not pass its language's check, or does not read; no end is made from a file that does not pass. The notes give what the language says."
            ),
            tr!("そのファイルを、その言語の検査を通るように直してください。", "Make the file pass its language's check."),
            include_str!("../tests/mutants/E203_検査を通らない規則/例.req"),
            &["E106", "E202"],
        )
        .beside(&[E203_RULE])
        .en(include_str!("../tests/mutants/E203_rule_that_does_not_pass/example.req"), &[E203_RULE_EN]),
        e(
            "E204",
            tr!("ツールの JSON が、yuen の知らない形です", "The tool's JSON is not of a known shape"),
            tr!("いまは出ません。ツールの JSON に、yuen が読むキーが無いときのためのコードでした。", "Never: it was for a tool's JSON lacking a key yuen reads."),
            tr!("直すものはありません。", "Nothing to fix."),
            "",
            &["E203"],
        )
        .retired(tr!(
            "ritsu 0.23.0 で使われなくなりました。ほかの言語を子プロセスの JSON で読む予定だったころに決めたコードです。いまの yuen は、ほかの言語を ritsu の中で型のまま読み、JSON を読みません。",
            "Retired in ritsu 0.23.0: it was set aside for reading the other languages from the JSON of child processes, and yuen reads them through ritsu's ports, as types, and reads no JSON."
        )),
        e(
            "E205",
            tr!("proto が読めません", "A .proto cannot be read"),
            tr!("`.proto` を、ritsu の `.proto` のパーサーが読めないとき（proto3 として読めないとき、import の先が読めないとき）。", "The `.proto` cannot be read by ritsu's reader of `.proto` files (not proto3, or an import that does not read)."),
            tr!("`.proto` を直してください。", "Correct the `.proto`."),
            include_str!("../tests/mutants/E205_読めないproto/例.req"),
            &["E201"],
        )
        .beside(&[E205_PROTO])
        .en(include_str!("../tests/mutants/E205_unreadable_proto/example.req"), &[E205_PROTO_EN]),
        e(
            "E206",
            tr!("名指したものの言語がつながっていません", "The language of what a line names is not joined"),
            tr!(
                "yuen 単独のバイナリ（`yuen`）に、ほかの言語のもの（規則、カレンダー、帳簿、主張、ワークフロー、コンテキスト）を名指すプロジェクトを渡したとき。このバイナリにはほかの言語が入っておらず、それを読めません。yuen は、言語ごとに一度、最初に名指したところでこのエラーを出し、そこで止まります。exit code は 2 です（プロジェクトの誤りではなく、走らせ方の問題なので）。",
                "The binary of yuen's own crate (`yuen`), given a project that names a thing of another language (a rule, a calendar, a book, a claim, a workflow, a context), which it holds none of and cannot read. It is said once for each language, where the first thing of it is named, and the check stops there. The exit code is 2: it is how the command is run, not what the project says."
            ),
            tr!(
                "同じコマンドを `ritsu yuen` で走らせてください（`ritsu yuen check .`）。ritsu はすべての言語をつなぎ、同じプロセスの中で読みます。",
                "Run the same command as `ritsu yuen` (`ritsu yuen check .`), which joins every language and reads them in the same process."
            ),
            include_str!("../tests/mutants/E206_rulec-not-joined/shipping.req"),
            &["E203"],
        )
        .beside(&[E206_RULE]),
        e(
            "W201",
            tr!("geas の記録が無いので、主張があるかを確かめていません", "No geas record, so whether the claim exists is not checked"),
            tr!("いまは出ません。`geas map` の記録が無く、spec のファイルがあることしか確かめられないときのためのコードでした。", "Never: it was for a spec with no `geas map` record, where only the file could be checked."),
            tr!("直すものはありません。", "Nothing to fix."),
            "",
            &["E202", "E404"],
        )
        .retired(tr!(
            "ritsu 0.23.0 で使われなくなりました。いまは geas が spec から主張を読むので、yuen は記録が無くても主張があるかを確かめます。記録が無いために辿れない範囲のファイルは、E404 の注に出ます。",
            "Retired in ritsu 0.23.0: geas's port reads the claims from the spec, so whether a claim exists is checked with no record; a file of a scope that cannot be traced for want of a record is said in a note of E404."
        )),
        // ── Links and hashes ──
        e(
            "E301",
            tr!("まだ確かめていないリンクです", "A link no one has looked at yet"),
            tr!("リンクの下に、確かめた記録（`reviewed …`）が無いとき。", "The link has no record (`reviewed …`) under it."),
            tr!("両端を読んで確かめたら、`yuen review … --by <役割>` で記録を書いてください。", "Once a person has read both ends, write the record with `yuen review … --by <role>`."),
            "requirements 例 v1\nrole 法務\n\nrequirement r1\n  text \"x\"\n  owner 法務\n  decided 2026-10-03 by 法務 \"例\"\n  satisfied by file \"a.txt\"\n  not verified \"例なので置かない\"\n    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f\n",
            &["E302", "E303"],
        )
        .beside(&[A_TXT])
        .en("requirements example v1\nrole legal\n\nrequirement r1\n  text \"x\"\n  owner legal\n  decided 2026-10-03 by legal \"example\"\n  satisfied by file \"a.txt\"\n  not verified \"left out in this example\"\n    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f\n", &[A_TXT]),
        e(
            "E302",
            tr!("確かめたあとで、リンク元が変わりました", "The upper end changed after the link was looked at"),
            tr!(
                "記録のリンク元のハッシュが、いまのハッシュと違うとき。リンク元は、`from` なら出典の条か元の要件、`satisfied by` と `verified by` なら要件です。要件のハッシュは出典のハッシュも含めて取るので、条が変われば、その先のリンクにも印が付きます。",
                "The record's hash of the upper end differs from its hash now. The upper end of a `from` is the article or the requirement it reads from, and of `satisfied by` and `verified by` the requirement; a requirement's end holds its sources' hashes, so a changed article marks the links below it too."
            ),
            tr!("差分を読み、要件がまだ正しく読めているか、成果物がまだ満たしているかを確かめてから、`yuen review` で記録を書き直してください。", "Read the diff; once a person has made sure the requirement still reads right and what meets it still does, write the record again with `yuen review`."),
            "requirements 例 v1\nrole 法務\n\nrequirement r1\n  text \"x\"\n  owner 法務\n  decided 2026-10-03 by 法務 \"例\"\n  satisfied by file \"a.txt\"\n    reviewed 2026-10-03 by 法務 sha256:f1e653e8ce72c16f -> sha256:87428fc522803d31\n  not verified \"例なので置かない\"\n    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f\n",
            &["E303", "E304", "W301"],
        )
        .beside(&[A_TXT, OLD_X])
        .en("requirements example v1\nrole legal\n\nrequirement r1\n  text \"x\"\n  owner legal\n  decided 2026-10-03 by legal \"example\"\n  satisfied by file \"a.txt\"\n    reviewed 2026-10-03 by legal sha256:f1e653e8ce72c16f -> sha256:87428fc522803d31\n  not verified \"left out in this example\"\n    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f\n", &[A_TXT, OLD_X]),
        e(
            "E303",
            tr!("確かめたあとで、リンク先が変わりました", "The lower end changed after the link was looked at"),
            tr!("記録のリンク先のハッシュが、いまのハッシュと違うとき。リンク先は、`from` なら要件、`satisfied by` と `verified by` なら成果物です。", "The record's hash of the lower end differs from its hash now: the requirement, for a `from`, and the artifact, for `satisfied by` and `verified by`."),
            tr!("差分を読み、成果物がまだ要件を満たしているかを確かめてから、`yuen review` で記録を書き直してください。", "Read the diff; once a person has made sure it still meets the requirement, write the record again with `yuen review`."),
            "requirements 例 v1\nrole 法務\n\nrequirement r1\n  text \"x\"\n  owner 法務\n  decided 2026-10-03 by 法務 \"例\"\n  satisfied by file \"a.txt\"\n    reviewed 2026-10-03 by 法務 sha256:fbdfb71af500ce5f -> sha256:0263829989b6fd95\n  not verified \"例なので置かない\"\n    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f\n",
            &["E302", "W301"],
        )
        .beside(&[A_TXT, OLD_A])
        .en("requirements example v1\nrole legal\n\nrequirement r1\n  text \"x\"\n  owner legal\n  decided 2026-10-03 by legal \"example\"\n  satisfied by file \"a.txt\"\n    reviewed 2026-10-03 by legal sha256:fbdfb71af500ce5f -> sha256:0263829989b6fd95\n  not verified \"left out in this example\"\n    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f\n", &[A_TXT, OLD_A]),
        e(
            "E304",
            tr!("見送りが承認されていないか、承認のあとで要件が変わりました", "A waiver is not approved, or the requirement changed after it was"),
            tr!("`not satisfied` か `not verified` の下に承認の記録（`approved …`）が無いとき、記録の要件のハッシュがいまと違うとき。", "A `not satisfied` or `not verified` has no approval (`approved …`) under it, or the approval's hash of the requirement differs from its hash now."),
            tr!("持ち主が理由を読んで承認したら、`yuen review … --by <役割>` で承認を書いてください。", "Once the owner has read the reason and approves it, write the approval with `yuen review … --by <role>`."),
            "requirements 例 v1\nrole 法務\n\nrequirement r1\n  text \"x\"\n  owner 法務\n  decided 2026-10-03 by 法務 \"例\"\n  not satisfied \"例なので置かない\"\n  not verified \"例なので置かない\"\n    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f\n",
            &["E301"],
        )
        .en("requirements example v1\nrole legal\n\nrequirement r1\n  text \"x\"\n  owner legal\n  decided 2026-10-03 by legal \"example\"\n  not satisfied \"left out in this example\"\n  not verified \"left out in this example\"\n    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f\n", &[]),
        e(
            "E305",
            tr!("確かめた記録の形が崩れています", "A record is not written right"),
            tr!("記録の行の形が崩れているとき（`->` や `by` が無いとき、見送りの下に `reviewed` があるとき）、ハッシュの数がリンク元の数と合わないとき。", "A record is not of its form (no `->` or `by`, a `reviewed` under a waiver), or holds a number of hashes other than the link's upper ends."),
            tr!("記録は `yuen review` が書くものです。確かめ直してから、`yuen review` で書き直してください。", "A record is what `yuen review` writes: look again, and let it write the record anew."),
            "requirements 例 v1\nrole 法務\n\nrequirement r1\n  text \"x\"\n  owner 法務\n  decided 2026-10-03 by 法務 \"例\"\n  satisfied by file \"a.txt\"\n    reviewed 2026-10-03 by 法務 sha256:fbdfb71af500ce5f\n  not verified \"例なので置かない\"\n    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f\n",
            &["E301"],
        )
        .beside(&[A_TXT])
        .en("requirements example v1\nrole legal\n\nrequirement r1\n  text \"x\"\n  owner legal\n  decided 2026-10-03 by legal \"example\"\n  satisfied by file \"a.txt\"\n    reviewed 2026-10-03 by legal sha256:fbdfb71af500ce5f\n  not verified \"left out in this example\"\n    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f\n", &[A_TXT]),
        e(
            "W301",
            tr!("確かめたときの中身が reviewed/ に無いので、差分を見せられません", "What was looked at is not in reviewed/, so no diff can be shown"),
            tr!("印の付いたリンクの、確かめたときの中身（`reviewed/<ハッシュ>`）が無いとき。印は、中身が無くてもハッシュで付きます。", "The content a marked link was looked at with (`reviewed/<hash>`) is not there; the mark is still made by the hashes."),
            tr!("`reviewed/` を git に入れておいてください。無くした中身は、git の履歴から戻せることがあります。", "Keep `reviewed/` in git; a lost content can often be brought back from the history."),
            "requirements 例 v1\nrole 法務\n\nrequirement r1\n  text \"x\"\n  owner 法務\n  decided 2026-10-03 by 法務 \"例\"\n  satisfied by file \"a.txt\"\n    reviewed 2026-10-03 by 法務 sha256:fbdfb71af500ce5f -> sha256:0263829989b6fd95\n  not verified \"例なので置かない\"\n    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f\n",
            &["E302", "E303"],
        )
        .beside(&[A_TXT])
        .en("requirements example v1\nrole legal\n\nrequirement r1\n  text \"x\"\n  owner legal\n  decided 2026-10-03 by legal \"example\"\n  satisfied by file \"a.txt\"\n    reviewed 2026-10-03 by legal sha256:fbdfb71af500ce5f -> sha256:0263829989b6fd95\n  not verified \"left out in this example\"\n    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f\n", &[A_TXT]),
        // ── Structure and coverage ──
        e(
            "E401",
            tr!("満たす成果物も、その見送りもありません", "Nothing meets the requirement, and no waiver says so"),
            tr!("要件の版に、`satisfied by` も `not satisfied` も無いとき。", "A version of a requirement has neither `satisfied by` nor `not satisfied`."),
            tr!("`satisfied by <成果物>` を書くか、`not satisfied \"<理由>\"` を書いて承認してもらってください。", "Write `satisfied by <artifact>`, or `not satisfied \"<why>\"` and have it approved."),
            "requirements 例 v1\nrole 法務\n\nrequirement r1\n  text \"x\"\n  owner 法務\n  decided 2026-10-03 by 法務 \"例\"\n  not verified \"例なので置かない\"\n    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f\n",
            &["E402"],
        )
        .en("requirements example v1\nrole legal\n\nrequirement r1\n  text \"x\"\n  owner legal\n  decided 2026-10-03 by legal \"example\"\n  not verified \"left out in this example\"\n    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f\n", &[]),
        e(
            "E402",
            tr!("確かめる主張も、その見送りもありません", "Nothing checks the requirement, and no waiver says so"),
            tr!("要件の版に、`verified by` も `not verified` も無いとき。", "A version of a requirement has neither `verified by` nor `not verified`."),
            tr!("`verified by <主張>` を書くか、`not verified \"<理由>\"` を書いて承認してもらってください。", "Write `verified by <claim>`, or `not verified \"<why>\"` and have it approved."),
            "requirements 例 v1\nrole 法務\n\nrequirement r1\n  text \"x\"\n  owner 法務\n  decided 2026-10-03 by 法務 \"例\"\n  not satisfied \"例なので置かない\"\n    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f\n",
            &["E401"],
        )
        .en("requirements example v1\nrole legal\n\nrequirement r1\n  text \"x\"\n  owner legal\n  decided 2026-10-03 by legal \"example\"\n  not satisfied \"left out in this example\"\n    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f\n", &[]),
        e(
            "E403",
            tr!("確かめる側に、何も確かめないものが書かれています", "Something that checks nothing is on the side that verifies"),
            tr!("`verified by` に、geas と koyomi の主張、検査するツールのファイル全体、テストのファイルのほかを書いたとき（rulec の出力、chobo の振替、proto など）。", "`verified by` names something other than a geas or koyomi claim, the whole file of a tool that checks it, or a test file (a rulec output, a chobo transfer, a proto)."),
            tr!("満たすものなら `satisfied by` に書いてください。確かめる側には、落ちることのあるものを書いてください。", "If it meets the requirement, write it after `satisfied by`; the side that verifies takes what can fail."),
            "requirements 例 v1\nrole 法務\n\nrequirement r1\n  text \"x\"\n  owner 法務\n  decided 2026-10-03 by 法務 \"例\"\n  verified by koyomi \"支払条件.cal\" date 支払日\n",
            &["E012"],
        )
        .en("requirements example v1\nrole legal\n\nrequirement r1\n  text \"x\"\n  owner legal\n  decided 2026-10-03 by legal \"example\"\n  verified by koyomi \"payment_terms.cal\" date payment_day\n", &[]),
        e(
            "E404",
            tr!("範囲の成果物を、どの要件からも辿れません", "An artifact in scope traces to no requirement"),
            tr!("`scope` が集めた成果物を、どのリンクも名指していないとき（それを含むものも、それに含まれるものも名指していないとき）。", "No link names an artifact a `scope` gathers, nor anything containing it or in it."),
            tr!("`satisfied by` か `verified by` でそれを名指す要件を足すか、範囲を狭めてください。", "Add a requirement whose `satisfied by` or `verified by` names it, or narrow the scope."),
            "requirements 例 v1\nrole 法務\n\nscope file \"b.txt\"\n\nrequirement r1\n  text \"x\"\n  owner 法務\n  decided 2026-10-03 by 法務 \"例\"\n  not satisfied \"例なので置かない\"\n    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f\n  not verified \"例なので置かない\"\n    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f\n",
            &["E401"],
        )
        .beside(&[B_TXT])
        .en("requirements example v1\nrole legal\n\nscope file \"b.txt\"\n\nrequirement r1\n  text \"x\"\n  owner legal\n  decided 2026-10-03 by legal \"example\"\n  not satisfied \"left out in this example\"\n    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f\n  not verified \"left out in this example\"\n    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f\n", &[B_TXT]),
        e(
            "E405",
            tr!("要件のあいだに循環があります", "The requirements make a cycle"),
            tr!("`from <要件>` と `replaces <要件>` をたどると、元の要件に戻ってくるとき。", "Following `from <requirement>` and `replaces <requirement>` comes back to where it started."),
            tr!("どちらが元かを決め、もう一方の `from` か `replaces` を消してください。", "Decide which one comes first, and delete the other's `from` or `replaces`."),
            "requirements 例 v1\nrole 法務\n\nrequirement r1\n  text \"x\"\n  owner 法務\n  from r2\n  not satisfied \"例なので置かない\"\n  not verified \"例なので置かない\"\n\nrequirement r2\n  text \"y\"\n  owner 法務\n  from r1\n  not satisfied \"例なので置かない\"\n  not verified \"例なので置かない\"\n",
            &["E409"],
        )
        .en("requirements example v1\nrole legal\n\nrequirement r1\n  text \"x\"\n  owner legal\n  from r2\n  not satisfied \"left out in this example\"\n  not verified \"left out in this example\"\n\nrequirement r2\n  text \"y\"\n  owner legal\n  from r1\n  not satisfied \"left out in this example\"\n  not verified \"left out in this example\"\n", &[]),
        e(
            "E406",
            tr!("版の期間に隙間があります", "The periods of the versions leave a gap"),
            tr!("前の版の終わりの翌日に、次の版が始まらないとき。注に、どの版にも入らない日が出ます。", "A version does not start on the day after the one before ends; the days in no version are given."),
            tr!("次の版の始まりを、前の版の終わりの翌日にしてください（直した行が注に出ます）。", "Start the next version on the day after the one before ends (the fixed line is shown)."),
            "requirements 例 v1\nrole 法務\n\nrequirement x v1\n  text \"x\"\n  in force 2026-01-01..2026-12-31\n  owner 法務\n  decided 2026-10-03 by 法務 \"例\"\n  not satisfied \"例なので置かない\"\n    approved 2026-10-03 by 法務 sha256:5ca1701c0312a54b\n  not verified \"例なので置かない\"\n    approved 2026-10-03 by 法務 sha256:5ca1701c0312a54b\n\nrequirement x v2\n  text \"x\"\n  in force 2027-01-02..\n  owner 法務\n  decided 2026-10-03 by 法務 \"例\"\n  not satisfied \"例なので置かない\"\n    approved 2026-10-03 by 法務 sha256:4e7392102a031a5b\n  not verified \"例なので置かない\"\n    approved 2026-10-03 by 法務 sha256:4e7392102a031a5b\n",
            &["E407", "E408"],
        )
        .en("requirements example v1\nrole legal\n\nrequirement x v1\n  text \"x\"\n  in force 2026-01-01..2026-12-31\n  owner legal\n  decided 2026-10-03 by legal \"example\"\n  not satisfied \"left out in this example\"\n    approved 2026-10-03 by legal sha256:5ca1701c0312a54b\n  not verified \"left out in this example\"\n    approved 2026-10-03 by legal sha256:5ca1701c0312a54b\n\nrequirement x v2\n  text \"x\"\n  in force 2027-01-02..\n  owner legal\n  decided 2026-10-03 by legal \"example\"\n  not satisfied \"left out in this example\"\n    approved 2026-10-03 by legal sha256:4e7392102a031a5b\n  not verified \"left out in this example\"\n    approved 2026-10-03 by legal sha256:4e7392102a031a5b\n", &[]),
        e(
            "E407",
            tr!("版の期間が重なります", "The periods of the versions overlap"),
            tr!("次の版が、前の版の終わり以前に始まるとき。注に、重なる日と二つの版が出ます。", "A version starts on or before the day the one before ends; the days and the two versions are given."),
            tr!("一つの日に効く版は一つです。前の版の終わりか、次の版の始まりを直してください。", "One version holds on a day: correct the end of the one or the start of the other."),
            "requirements 例 v1\nrole 法務\n\nrequirement x v1\n  text \"x\"\n  in force 2026-01-01..2026-12-31\n  owner 法務\n  decided 2026-10-03 by 法務 \"例\"\n  not satisfied \"例なので置かない\"\n    approved 2026-10-03 by 法務 sha256:5ca1701c0312a54b\n  not verified \"例なので置かない\"\n    approved 2026-10-03 by 法務 sha256:5ca1701c0312a54b\n\nrequirement x v2\n  text \"x\"\n  in force 2026-12-30..\n  owner 法務\n  decided 2026-10-03 by 法務 \"例\"\n  not satisfied \"例なので置かない\"\n    approved 2026-10-03 by 法務 sha256:7523d312c91d81c0\n  not verified \"例なので置かない\"\n    approved 2026-10-03 by 法務 sha256:7523d312c91d81c0\n",
            &["E406", "E408"],
        )
        .en("requirements example v1\nrole legal\n\nrequirement x v1\n  text \"x\"\n  in force 2026-01-01..2026-12-31\n  owner legal\n  decided 2026-10-03 by legal \"example\"\n  not satisfied \"left out in this example\"\n    approved 2026-10-03 by legal sha256:5ca1701c0312a54b\n  not verified \"left out in this example\"\n    approved 2026-10-03 by legal sha256:5ca1701c0312a54b\n\nrequirement x v2\n  text \"x\"\n  in force 2026-12-30..\n  owner legal\n  decided 2026-10-03 by legal \"example\"\n  not satisfied \"left out in this example\"\n    approved 2026-10-03 by legal sha256:7523d312c91d81c0\n  not verified \"left out in this example\"\n    approved 2026-10-03 by legal sha256:7523d312c91d81c0\n", &[]),
        e(
            "E408",
            tr!("版の期間が、そろって書かれていません", "The periods of the versions are not all written"),
            tr!("版が二つ以上あるのに期間の無い版があるとき、終わりを開けた版が最後でないとき、始まりを開けた版が最初でないとき、版の番号が期間の順に増えていないとき。", "A requirement with more than one version has a version without a period, a version that leaves its end open is not the last, one that leaves its start open is not the first, or the versions do not number in the order of their periods."),
            tr!("どの版にも `in force` を書き、版の番号を期間の順にそろえてください。", "Give every version an `in force`, and number them in the order of their periods."),
            "requirements 例 v1\nrole 法務\n\nrequirement x v1\n  text \"x\"\n  owner 法務\n  decided 2026-10-03 by 法務 \"例\"\n  not satisfied \"例なので置かない\"\n    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f\n  not verified \"例なので置かない\"\n    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f\n\nrequirement x v2\n  text \"x\"\n  in force 2027-01-01..\n  owner 法務\n  decided 2026-10-03 by 法務 \"例\"\n  not satisfied \"例なので置かない\"\n    approved 2026-10-03 by 法務 sha256:fa2942b05a851b79\n  not verified \"例なので置かない\"\n    approved 2026-10-03 by 法務 sha256:fa2942b05a851b79\n",
            &["E406", "E407"],
        )
        .en("requirements example v1\nrole legal\n\nrequirement x v1\n  text \"x\"\n  owner legal\n  decided 2026-10-03 by legal \"example\"\n  not satisfied \"left out in this example\"\n    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f\n  not verified \"left out in this example\"\n    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f\n\nrequirement x v2\n  text \"x\"\n  in force 2027-01-01..\n  owner legal\n  decided 2026-10-03 by legal \"example\"\n  not satisfied \"left out in this example\"\n    approved 2026-10-03 by legal sha256:fa2942b05a851b79\n  not verified \"left out in this example\"\n    approved 2026-10-03 by legal sha256:fa2942b05a851b79\n", &[]),
        e(
            "E409",
            tr!("置き換える要件の期間が、置き換えられる要件の終わりの翌日から始まりません", "What replaces a requirement does not start the day after it ends"),
            tr!("`replaces` で置き換えられる要件に終わりの日が無いとき、`replaces` を書いた版の始まりが、その翌日でないとき。", "What `replaces` names has no end, or the version that writes `replaces` does not start on the day after it."),
            tr!("置き換えられる要件に終わりの日を書き、置き換える版をその翌日から始めてください（直した行が注に出ます）。", "End what is replaced, and start what replaces it on the day after (the fixed line is shown)."),
            "requirements 例 v1\nrole 法務\n\nrequirement r1\n  text \"x\"\n  in force 2026-01-01..2026-12-31\n  owner 法務\n  decided 2026-10-03 by 法務 \"例\"\n  not satisfied \"例なので置かない\"\n    approved 2026-10-03 by 法務 sha256:5ca1701c0312a54b\n  not verified \"例なので置かない\"\n    approved 2026-10-03 by 法務 sha256:5ca1701c0312a54b\n\nrequirement r2\n  text \"y\"\n  in force 2027-01-02..\n  owner 法務\n  replaces r1\n  decided 2026-10-03 by 法務 \"例\"\n  not satisfied \"例なので置かない\"\n    approved 2026-10-03 by 法務 sha256:cb98f1b81b3a40fd\n  not verified \"例なので置かない\"\n    approved 2026-10-03 by 法務 sha256:cb98f1b81b3a40fd\n",
            &["E406", "E405"],
        )
        .en("requirements example v1\nrole legal\n\nrequirement r1\n  text \"x\"\n  in force 2026-01-01..2026-12-31\n  owner legal\n  decided 2026-10-03 by legal \"example\"\n  not satisfied \"left out in this example\"\n    approved 2026-10-03 by legal sha256:5ca1701c0312a54b\n  not verified \"left out in this example\"\n    approved 2026-10-03 by legal sha256:5ca1701c0312a54b\n\nrequirement r2\n  text \"y\"\n  in force 2027-01-02..\n  owner legal\n  replaces r1\n  decided 2026-10-03 by legal \"example\"\n  not satisfied \"left out in this example\"\n    approved 2026-10-03 by legal sha256:cb98f1b81b3a40fd\n  not verified \"left out in this example\"\n    approved 2026-10-03 by legal sha256:cb98f1b81b3a40fd\n", &[]),
        e(
            "W401",
            tr!("見送りと、同じ側のリンクの両方があります", "A waiver and a link on the same side"),
            tr!("`satisfied by` と `not satisfied`（か `verified by` と `not verified`）が同じ要件の版にあるとき。見送りは要りません。", "A version has both a `satisfied by` and a `not satisfied` (or a `verified by` and a `not verified`): the waiver is not needed."),
            tr!("リンクを置いたのなら、見送りを消してください。", "Now that there is a link, delete the waiver."),
            "requirements 例 v1\nrole 法務\n\nrequirement r1\n  text \"x\"\n  owner 法務\n  decided 2026-10-03 by 法務 \"例\"\n  satisfied by file \"a.txt\"\n    reviewed 2026-10-03 by 法務 sha256:fbdfb71af500ce5f -> sha256:87428fc522803d31\n  not satisfied \"例なので置かない\"\n    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f\n  not verified \"例なので置かない\"\n    approved 2026-10-03 by 法務 sha256:fbdfb71af500ce5f\n",
            &["E401"],
        )
        .beside(&[A_TXT])
        .en("requirements example v1\nrole legal\n\nrequirement r1\n  text \"x\"\n  owner legal\n  decided 2026-10-03 by legal \"example\"\n  satisfied by file \"a.txt\"\n    reviewed 2026-10-03 by legal sha256:fbdfb71af500ce5f -> sha256:87428fc522803d31\n  not satisfied \"left out in this example\"\n    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f\n  not verified \"left out in this example\"\n    approved 2026-10-03 by legal sha256:fbdfb71af500ce5f\n", &[A_TXT]),
    ];
    // every code that is printed has its example; the words are for one that has none yet
    let later = tr!("（再現はまだありません。）", "(No example yet.)");
    Ledger {
        tool: "yuen",
        example_file: "example.req",
        fence: "req",
        repro_heading: tr!("再現", "Example"),
        later_text: later.clone(),
        later_markdown: later,
        entries,
    }
}

/// The entry of a code, written in either case.
pub fn find(code: &str) -> Option<Entry> {
    ledger().find(code).cloned()
}

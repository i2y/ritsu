//! The ledger of every diagnostic code (DESIGN 4.2). `koyomi explain` reads it, and the
//! tests run every entry's example and require its code to come out, so the example cannot
//! go stale while the prose around it still reads well. The entries are koyomi's; how they are
//! written out, and how every example is run, is ritsu-base's ([`ritsu_base::ledger`]).

use ritsu_base::ledger::{Entry, Ledger, Repro};
use ritsu_base::text::Text;

/// An entry whose example is the smallest `.cal` that gets the code.
fn e(code: &'static str, title: Text, when: Text, fix: Text, example: &'static str, related: &'static [&'static str]) -> Entry {
    Entry::new(code, title, when, fix, Repro::File { body: example, beside: &[] }, related)
}

// ── What the examples need beside them ───────────────────────────────────

const WEEKENDS: (&str, &[u8]) = ("weekends.cal", "calendar 土日(weekends) v1\noffset +09:00\n\nclosed weekly sat, sun\n".as_bytes());
const WEEKENDS_NO_OFFSET: (&str, &[u8]) = ("weekends.cal", "calendar 土日(weekends) v1\n\nclosed weekly sat, sun\n".as_bytes());
const HOLIDAYS: (&str, &[u8]) = ("holidays.csv", "2026-01-01,元日\n2026-05-04,みどりの日\n".as_bytes());
const HOLIDAYS_BAD: (&str, &[u8]) = ("holidays.csv", "2026-01-01,元日\n2026-13-01,休み\n".as_bytes());
const HOLIDAYS_GAP: (&str, &[u8]) = ("holidays.csv", "2025-01-01,元日\n2027-01-01,元日\n".as_bytes());
const TABLE_2026: (&str, &[u8]) = (
    "closed_days.cal",
    "calendar 休み(closed_days) v1\n\nsource 休み = file \"holidays.csv\" sha256:56ebcd2f1e91e0a1\n  format csv\n  covers 2026-01-01..2026-12-31\n\nclosed weekly sat, sun\nclosed 休み\n".as_bytes(),
);
const LAW_142: (&str, &[u8]) = (
    "sources/law/129AC0000000089@2026-10-01/MainProvision-Article_142.xml",
    include_bytes!("../examples/sources/law/129AC0000000089@2026-10-01/MainProvision-Article_142.xml"),
);

// ── The same, with English names (what `explain` shows in English) ──

const HOLIDAYS_EN: (&str, &[u8]) = ("holidays.csv", "2026-01-01,New Year's Day\n2026-05-04,Greenery Day\n".as_bytes());
const HOLIDAYS_BAD_EN: (&str, &[u8]) = ("holidays.csv", "2026-01-01,New Year's Day\n2026-13-01,Day off\n".as_bytes());
const HOLIDAYS_GAP_EN: (&str, &[u8]) = ("holidays.csv", "2025-01-01,New Year's Day\n2027-01-01,New Year's Day\n".as_bytes());
const WEEKENDS_EN: (&str, &[u8]) = ("weekends.cal", "calendar weekends v1\noffset +09:00\n\nclosed weekly sat, sun\n".as_bytes());
const WEEKENDS_NO_OFFSET_EN: (&str, &[u8]) = ("weekends.cal", "calendar weekends v1\n\nclosed weekly sat, sun\n".as_bytes());
const TABLE_2026_EN: (&str, &[u8]) = ("closed_days.cal", "calendar closed_days v1\n\nsource holidays = file \"holidays.csv\" sha256:899aee90fcd554a9\n  format csv\n  covers 2026-01-01..2026-12-31\n\nclosed weekly sat, sun\nclosed holidays\n".as_bytes());

const RANGE_JAN: &str = "range >=2026-01-01 <=2026-01-31";

pub fn ledger() -> Ledger {
    let _ = RANGE_JAN;
    let entries = vec![
        // ── Words and lines ──
        e(
            "E001",
            tr!("読めない字句があります", "Something cannot be read as a word of the language"),
            tr!(
                "知らない文字、閉じていない文字列、形の崩れた日付や固定（`sha256:` のあとが 16 桁の小文字の 16 進数でない）、数で始まる英字の名前があるとき。",
                "A character the language does not have, a string not closed, a date or pin of the wrong shape (a pin is 16 lowercase hex digits), or an ASCII name that starts with a digit."
            ),
            tr!(
                "示された位置を直します。文字列は `\"…\"` で閉じ、日付は `2026-01-01` の形で書きます。",
                "Correct it where it points: close the string with `\"`, write a date as `2026-01-01`."
            ),
            "dates t v1\ndescription \"閉じていない\n",
            &["E006"],
        )
        .english(Repro::File { body: "dates t v1\ndescription \"not closed\n", beside: &[] }),
        e(
            "E002",
            tr!("この位置に書けない語があります", "A word is written where it does not belong"),
            tr!(
                "行の中で、構文がその語を受け付けないとき。知らない操作、`else` のあとの知らない扱い、`roll` のあとの知らない慣行など。",
                "The syntax does not take the word there: an operation that does not exist, an unknown way after `else`, an unknown convention after `roll`, and the like."
            ),
            tr!("注に挙がる書き方のどれかにします。", "Use one of the forms the note lists."),
            "dates t v1\n\ninputs\n  d : date  range >=2026-01-01 <=2026-01-31\n\ndate x = d\n  next month\n",
            &["E004"],
        ),
        e(
            "E003",
            tr!("ファイルが `calendar` か `dates` の行で始まっていません", "The file does not start with a `calendar` or a `dates` line"),
            tr!("コメントと空行を除いた最初の行が、`calendar …` でも `dates …` でもないとき。", "The first line that is not a comment or blank is neither `calendar …` nor `dates …`."),
            tr!(
                "休みの決まりなら `calendar 東京の営業日(tokyo) v1`、日付の関数なら `dates 支払条件(payment_terms) v1` のように書き始めます。",
                "Start a calendar like `calendar tokyo v1` and date functions like `dates payment_terms v1`."
            ),
            "inputs\n  d : date  range >=2026-01-01 <=2026-01-31\n",
            &["E004"],
        ),
        e(
            "E004",
            tr!("節の順序か数が違います", "A section is out of order, repeated, or in the wrong kind of file"),
            tr!(
                "節が決まった順序にないとき、一度だけの節が二度あるとき、calendar のファイルに `inputs` があるなど種類の違うファイルの節があるとき、表の出典に `format` か `covers` の行が無いとき。",
                "Sections are out of their order, a section that comes once comes twice, a section belongs to the other kind of file (`inputs` in a calendar), or a table has no `format` or `covers` line."
            ),
            tr!(
                "calendar は 見出し、`description`、`offset`、`use calendar`、`source`、`closed` と `open` の順。dates は 見出し、`description`、`use calendar`、`source`、`inputs`、`date`、`claims`、`examples` の順に書きます。",
                "A calendar goes: heading, `description`, `offset`, `use calendar`, `source`, `closed` and `open`. A dates file goes: heading, `description`, `use calendar`, `source`, `inputs`, `date`, `claims`, `examples`."
            ),
            "calendar t v1\n\ninputs\n  d : date  range >=2026-01-01 <=2026-01-31\n",
            &["E002", "E003"],
        ),
        e(
            "E005",
            tr!("字下げが合いません", "The indentation does not line up"),
            tr!(
                "字下げにタブがあるとき、同じ節の行の字下げがそろっていないとき、字下げで続く行の無いところに字下げした行があるとき。",
                "The indentation has a tab, the lines of one block are not indented alike, or an indented line follows nothing that takes indented lines."
            ),
            tr!("スペースで、同じ節の行は同じ幅に字下げします。", "Indent with spaces, every line of a block by the same amount."),
            "dates t v1\n\ninputs\n\td : date  range >=2026-01-01 <=2026-01-31\n",
            &[],
        ),
        e(
            "E006",
            tr!("無い日付・月日・時刻が書かれています", "A date, a month and day, or a time that does not exist"),
            tr!(
                "`2026-02-30`、`13-01`、`25:00` のように、暦や時計に無い値を書いたとき。日付は 0001-01-01〜9999-12-31 です。",
                "A value no calendar or clock has, like `2026-02-30`, `13-01` or `25:00`. Dates run from 0001-01-01 to 9999-12-31."
            ),
            tr!("暦にある日付、時計にある時刻に直します。", "Write one that exists."),
            "dates t v1\n\ninputs\n  d : date  range >=2026-02-01 <=2026-02-30\n",
            &["E001"],
        ),
        // ── Names and types ──
        e(
            "E007",
            tr!("同じ名前を二度宣言しています", "A name is declared twice"),
            tr!(
                "入力と日付の名前、別名、条件の名前、出典の名前、例の列のどれかが二度出てくるとき。入力と日付には、互いに同じ名前を付けられません。",
                "An input or date name, an alias, a claim name, a source name or an example column comes twice. An input and a date cannot have the same name."
            ),
            tr!("どちらかの名前を変えます。", "Rename one of them."),
            "dates t v1\n\ninputs\n  d : date  range >=2026-01-01 <=2026-01-31\n\ndate x = d\n  + 1 day\ndate x = d\n  + 2 days\n",
            &["E008"],
        ),
        e(
            "E008",
            tr!("宣言されていない名前です", "A name is not declared"),
            tr!(
                "日付の始まり、操作の数、条件、例の列、`closed` の表に、宣言の無い名前を書いたとき。",
                "The start of a date, a number in an operation, a claim, an example column or a `closed` table names something not declared."
            ),
            tr!("名前の書き違いを直すか、宣言を足します。", "Correct the spelling, or declare it."),
            "dates t v1\n\ninputs\n  d : date  range >=2026-01-01 <=2026-01-31\n\ndate x = e\n  + 1 day\n",
            &["E007", "E011"],
        ),
        e(
            "E009",
            tr!("名前か別名が予約語です", "A name or an alias is a reserved word"),
            tr!(
                "名前か別名が koyomi のキーワードのとき、別名が出力先の言語（TypeScript、Python、Go、Rust、PostgreSQL）の予約語のとき、生成したコードが使う名前（`is_open`、`date`、`<別名>_at` など）とぶつかるとき。",
                "A name or an alias is a keyword of koyomi, an alias is a reserved word of a target (TypeScript, Python, Go, Rust, PostgreSQL), or it collides with a name the generated code defines (`is_open`, `date`, `<alias>_at`, …)."
            ),
            tr!("別の名前か別名にします。", "Choose another name or alias."),
            "dates t v1\n\ninputs\n  d : date  range >=2026-01-01 <=2026-01-31\n\ndate 期日(select) = d\n  + 1 day\n",
            &["E010"],
        )
        .english(Repro::File { body: "dates t v1\n\ninputs\n  d : date  range >=2026-01-01 <=2026-01-31\n\ndate select = d\n  + 1 day\n", beside: &[] }),
        e(
            "E010",
            tr!("公開する名前に ASCII の別名がありません", "A public name has no ASCII alias"),
            tr!(
                "ファイルの見出し、入力、日付の名前が `[a-z][a-z0-9_]*` の形でなく、丸括弧の別名も無いとき。別名の形が違うときも。",
                "The name of the heading, an input or a date is not of the form `[a-z][a-z0-9_]*` and has no alias in parentheses, or its alias is not of that form."
            ),
            tr!("`受領日(received)` のように、生成したコードの識別子になる別名を付けます。", "Add the alias the generated code will use, like `Received(received)`."),
            "dates t v1\n\ninputs\n  受領日 : date  range >=2026-01-01 <=2026-01-31\n",
            &["E009"],
        )
        .english(Repro::File { body: "dates t v1\n\ninputs\n  Received : date  range >=2026-01-01 <=2026-01-31\n", beside: &[] }),
        e(
            "E011",
            tr!("型が合いません", "A type does not fit"),
            tr!(
                "日付のところに整数の入力を、数のところに日付を書いたとき。条件で整数を比べたとき、例の列に型の違う値を書いたときも。",
                "An integer input where a date goes, a date where a number goes, a claim that compares integers, or an example value of the wrong type."
            ),
            tr!("日付は日付の入力かほかの日付から、数は数か整数の入力から取ります。", "Take a date from the date input or another date, and a number from a number or an integer input."),
            "dates t v1\n\ninputs\n  d : date  range >=2026-01-01 <=2026-01-31\n  n : int   range >=1 <=3\n\ndate x = n\n  + 1 day\n",
            &["E008"],
        ),
        e(
            "E012",
            tr!("`date` の入力がちょうど一つではありません", "There is not exactly one `date` input"),
            tr!(
                "dates のファイルの `inputs` に、日付の入力が無いか、二つ以上あるとき。総当たりで確かめられる大きさに抑えるためです。",
                "The `inputs` of a dates file have no date or more than one. One date keeps the check exhaustive."
            ),
            tr!(
                "日付を一つにします。二つめの日付が要る計算は、あいだの日数を整数の入力として受け取ります。",
                "Keep one date; for a computation that needs a second one, take the days between as an integer input."
            ),
            "dates t v1\n\ninputs\n  a : date  range >=2026-01-01 <=2026-01-31\n  b : date  range >=2026-01-01 <=2026-01-31\n",
            &[],
        ),
        e(
            "E013",
            tr!("範囲が違います", "A range is wrong"),
            tr!(
                "入力の範囲の端が無いとき、空のとき、`>` や `<` で書いたとき。足す数に使う整数の入力が負の数を取りうるとき、日に使う数が 1〜31 の外になりうるときも。",
                "An input's range has an end missing, is empty, or is written with `>` or `<`; an integer input added can be negative; a day of the month can fall outside 1..31."
            ),
            tr!("`range >=2026-01-01 <=2026-12-31` のように、両端を `>=` と `<=` で書きます。", "Write both ends with `>=` and `<=`, like `range >=2026-01-01 <=2026-12-31`."),
            "dates t v1\n\ninputs\n  d : date  range >=2026-01-01\n",
            &[],
        ),
        e(
            "E014",
            tr!("日付が循環しています", "Dates start from each other in a circle"),
            tr!("日付の始まりをたどると、自分に戻ってくるとき。", "Following where each date starts from comes back to it."),
            tr!("どれかの日付を、入力の日付から始めます。", "Start one of them from the date input."),
            "dates t v1\n\ninputs\n  d : date  range >=2026-01-01 <=2026-01-31\n\ndate x = y\n  + 1 day\ndate y = x\n  + 1 day\n",
            &[],
        ),
        e(
            "E015",
            tr!("`use calendar` の先が使えません", "The calendar of `use calendar` cannot be used"),
            tr!(
                "読めないとき、calendar のファイルでないとき、読んでいくと元のファイルに戻るとき、オフセットが食い違うとき。",
                "It cannot be read, it is not a calendar file, reading it leads back to the file, or its offset differs."
            ),
            tr!("パス（この .cal のあるディレクトリから数える）か、読む先のファイルを直します。", "Correct the path (from this .cal's directory) or the file it names."),
            "dates t v1\nuse calendar \"nowhere.cal\"\n\ninputs\n  d : date  range >=2026-01-01 <=2026-01-31\n",
            &[],
        ),
        // ── The calendar and the sources ──
        e(
            "E101",
            tr!("出典の写しがありません", "The copy of a source is not there"),
            tr!(
                "表の出典の `file`、または固定した法令の条の写し（`sources/law/<法令ID>@<日付>/`）が無いとき。check は通信しません。",
                "The `file` of a table, or the copy of a pinned article of a law (under `sources/law/<law id>@<date>/`), is missing. check never reads the network."
            ),
            tr!("`koyomi source fetch` で写しを取ってくるか、パスを直します。", "Take the copy with `koyomi source fetch`, or correct the path."),
            "calendar t v1\n\nsource 休み = file \"holidays.csv\" sha256:56ebcd2f1e91e0a1\n  format csv\n  covers 2026-01-01..2026-12-31\n\nclosed 休み\n",
            &["E102", "E103"],
        )
        .english(Repro::File { body: "calendar t v1\n\nsource holidays = file \"holidays.csv\" sha256:899aee90fcd554a9\n  format csv\n  covers 2026-01-01..2026-12-31\n\nclosed holidays\n", beside: &[] }),
        e(
            "E102",
            tr!("出典が固定されていません", "A source is not pinned"),
            tr!("表の出典の行か、法令の条の固定の行に `sha256:` が無いとき。", "A table's line, or the pin line of a law's article, has no `sha256:`."),
            tr!("写しの SHA-256 の先頭 16 桁を `sha256:` で書きます（直し方に、いまの写しの値を書いた行が出ます）。", "Pin it with the first 16 digits of the copy's SHA-256; the fix gives the line with the copy's own."),
            "calendar t v1\n\nsource 休み = file \"holidays.csv\"\n  format csv\n  covers 2026-01-01..2026-12-31\n\nclosed 休み\n",
            &["E101", "E103"],
        )
        .beside(&[HOLIDAYS])
        .english(Repro::File { body: "calendar t v1\n\nsource holidays = file \"holidays.csv\"\n  format csv\n  covers 2026-01-01..2026-12-31\n\nclosed holidays\n", beside: &[HOLIDAYS_EN] }),
        e(
            "E103",
            tr!("写しが固定と違います", "A copy does not match its pin"),
            tr!("写しのバイト列の SHA-256 の先頭 16 桁が、固定した値と違うとき。固定したあとで写しが変わっています。", "The first 16 digits of the copy's SHA-256 differ from the pin: the copy changed after it was pinned."),
            tr!("何が変わったかを読んでから（`koyomi source outdated`）、固定を書き換えます。", "Read what changed (`koyomi source outdated`), then pin it again."),
            "calendar t v1\n\nsource 休み = file \"holidays.csv\" sha256:0123456789abcdef\n  format csv\n  covers 2026-01-01..2026-12-31\n\nclosed 休み\n",
            &["E102"],
        )
        .beside(&[HOLIDAYS])
        .english(Repro::File { body: "calendar t v1\n\nsource holidays = file \"holidays.csv\" sha256:0123456789abcdef\n  format csv\n  covers 2026-01-01..2026-12-31\n\nclosed holidays\n", beside: &[HOLIDAYS_EN] }),
        e(
            "E104",
            tr!("写しが読めません", "A copy cannot be read"),
            tr!(
                "写しの文字コードが違うとき（Shift_JIS として読めないバイト、UTF-8 でないバイト）、行の最初の値が日付でないとき、値が多すぎるとき、同じ日付が二度あるとき、JSON として読めないとき、地域が無いとき、行が一つも無いとき。",
                "The copy is not in its encoding (bytes Shift_JIS or UTF-8 does not have), a line does not start with a date, has too many values or repeats a date, the JSON cannot be read or lacks the division, or there are no rows."
            ),
            tr!("示された行を直すか、`format` の文字コードを直します。", "Correct the line it names, or the encoding under `format`."),
            "calendar t v1\n\nsource 休み = file \"holidays.csv\" sha256:72fa28860be08f0a\n  format csv\n  covers 2026-01-01..2026-12-31\n\nclosed 休み\n",
            &[],
        )
        .beside(&[HOLIDAYS_BAD])
        .english(Repro::File { body: "calendar t v1\n\nsource holidays = file \"holidays.csv\" sha256:6467b916662fb97a\n  format csv\n  covers 2026-01-01..2026-12-31\n\nclosed holidays\n", beside: &[HOLIDAYS_BAD_EN] }),
        e(
            "E105",
            tr!("表の行が `covers` の外にあります", "A row of a table is outside `covers`"),
            tr!("`covers` は表が休みを全部載せている範囲で、その外の日の行があるとき。", "`covers` is the span the table lists every closed day of, and a row lies outside it."),
            tr!("表が延びたのなら `covers` も延ばします（直し方に、行のある年を含む範囲が出ます）。", "When the table grew, widen `covers` too; the fix gives a span that holds the rows."),
            "calendar t v1\n\nsource 休み = file \"holidays.csv\" sha256:56ebcd2f1e91e0a1\n  format csv\n  covers 2026-01-01..2026-03-31\n\nclosed 休み\n",
            &["E106"],
        )
        .beside(&[HOLIDAYS])
        .english(Repro::File { body: "calendar t v1\n\nsource holidays = file \"holidays.csv\" sha256:899aee90fcd554a9\n  format csv\n  covers 2026-01-01..2026-03-31\n\nclosed holidays\n", beside: &[HOLIDAYS_EN] }),
        e(
            "E106",
            tr!("`covers listed years` で、行の無い年があります", "`covers listed years`, and a year has no rows"),
            tr!("行のある最初の年と最後の年のあいだに、行の無い年があるとき。休みの無い年か、表から落ちた年かを決められません。", "A year between the first and the last with rows has none: it cannot be told whether it had no closed days or fell out of the table."),
            tr!("範囲を日付で書きます（`covers 2025-01-01..2027-12-31` のように）。", "Write the span as dates, like `covers 2025-01-01..2027-12-31`."),
            "calendar t v1\n\nsource 休み = file \"holidays.csv\" sha256:bec0e9a9279b388d\n  format csv\n  covers listed years\n\nclosed 休み\n",
            &["E105"],
        )
        .beside(&[HOLIDAYS_GAP])
        .english(Repro::File { body: "calendar t v1\n\nsource holidays = file \"holidays.csv\" sha256:cf519bbe72d28ea7\n  format csv\n  covers listed years\n\nclosed holidays\n", beside: &[HOLIDAYS_GAP_EN] }),
        e(
            "E107",
            tr!("オフセットが `±HH:MM` の形ではありません", "The offset is not of the form `±HH:MM`"),
            tr!(
                "`offset` に `Asia/Tokyo` のようなタイムゾーンの名前や、`UTC` のような語を書いたとき。夏時間のあるタイムゾーンは、固定のオフセットでは一年の半分で一時間ずれるので扱いません。",
                "`offset` names a time zone, like `Asia/Tokyo`, or is a word like `UTC`. A zone with daylight saving time is an hour off a fixed offset for half the year, so it is not taken."
            ),
            tr!("`offset +09:00` のように数で書きます。夏時間のある地域なら、時刻は出さず日付だけにします。", "Write the number, like `offset +09:00`. For a place with daylight saving time, give dates only."),
            "calendar t v1\noffset Asia/Tokyo\n\nclosed weekly sat, sun\n",
            &["E110"],
        ),
        e(
            "E108",
            tr!("営業日が一日も無いカレンダーです", "The calendar has no business day at all"),
            tr!("データの範囲のどの日も休みのとき。表の `covers` が重ならないときも。翌営業日を探す計算が終わりません。", "Every day of the data range is closed, or the tables' `covers` do not overlap. Looking for a business day would never end."),
            tr!("休みの決まりを見直します。", "Look at the closing lines again."),
            "calendar t v1\n\nclosed weekly mon, tue, wed, thu, fri, sat, sun\n",
            &[],
        ),
        e(
            "E109",
            tr!("休みかどうかを調べる行があるのに、カレンダーがありません", "A line asks which days are closed, and there is no calendar"),
            tr!("`business days`、`roll`、`if closed`、条件の `is open` や `business days` があるのに、`use calendar` が無いとき。", "There is `business days`, `roll`, `if closed`, or a claim with `is open` or `business days`, and no `use calendar`."),
            tr!("見出しのあとに `use calendar \"<ファイル>\"` を書きます。", "Write `use calendar \"<file>\"` after the heading."),
            "dates t v1\n\ninputs\n  d : date  range >=2026-01-01 <=2026-01-31\n\ndate x = d\n  roll following\n",
            &["E110"],
        ),
        e(
            "E110",
            tr!("`at` があるのに、カレンダーにオフセットがありません", "`at` is written, and the calendar has no offset"),
            tr!("日付に `at 09:00` か `at end of day` を書いたのに、カレンダー（読んだ先を含む）に `offset` が無いとき。", "A date has `at 09:00` or `at end of day`, and its calendar (with the ones it reads) has no `offset`."),
            tr!("カレンダーに `offset +09:00` のように書くか、`at` の行を消します。", "Give the calendar an offset, like `offset +09:00`, or delete the `at` line."),
            "dates t v1\nuse calendar \"weekends.cal\"\n\ninputs\n  d : date  range >=2026-01-01 <=2026-01-31\n\ndate x = d\n  roll following\n  at 09:00\n",
            &["E107"],
        )
        .beside(&[WEEKENDS_NO_OFFSET])
        .english(Repro::File { body: "dates t v1\nuse calendar \"weekends.cal\"\n\ninputs\n  d : date  range >=2026-01-01 <=2026-01-31\n\ndate x = d\n  roll following\n  at 09:00\n", beside: &[WEEKENDS_NO_OFFSET_EN] }),
        e(
            "E111",
            tr!("法令の引用が使えません", "A citation of a law cannot be used"),
            tr!(
                "`@` で引いた出典が宣言されていないとき、法令でないとき、条を書いていないとき、引ける形でないとき（本則の条・項・号だけ）、固定されていないとき。",
                "A citation names a source not declared, a source that is not a law, no article, a fragment of a form that cannot be cited (only articles, paragraphs and items of the main provisions), or one that is not pinned."
            ),
            tr!(
                "`source 民法 = law \"<法令ID>\" asof <日付>` を宣言し、その下に引く条を `第143条 sha256:…` のように固定します。",
                "Declare `source 民法 = law \"<law id>\" asof <date>` and pin every article cited under it, like `第143条 sha256:…`."
            ),
            "dates t v1\n\ninputs\n  d : date  range >=2026-01-01 <=2026-01-31\n\ndate x = d    @民法 第143条\n  + 1 day\n",
            &["W102", "E101"],
        ),
        e(
            "W101",
            tr!("`open` に書いた日が、もともと営業日です", "A day under `open` is a business day anyway"),
            tr!("`open` の日が、どの `closed` にも当たらないとき。", "No `closed` line names the day written under `open`."),
            tr!("日付の書き違いでなければ、その行を消します。", "Unless the date is a slip, delete the line."),
            "calendar t v1\n\nclosed weekly sat, sun\nopen 2026-12-28 \"臨時営業\"\n",
            &[],
        )
        .english(Repro::File { body: "calendar t v1\n\nclosed weekly sat, sun\nopen 2026-12-28 \"Special opening\"\n", beside: &[] }),
        e(
            "W102",
            tr!("固定した条が、どこからも引かれていません", "A pinned article is cited nowhere"),
            tr!("法令の出典の下に固定した条を、どの行も `@` で引いていないとき。引用を消したあとの残りです。", "No line cites, with `@`, an article pinned under a law: what is left after a citation was removed."),
            tr!("固定の行を消すか、その条に当たる行に引用を書きます。", "Delete the pin line, or cite it on the line it belongs to."),
            "dates t v1\nsource 民法 = law \"129AC0000000089\" asof 2026-10-01\n  第142条 sha256:fc8c35a0769d3b35\n\ninputs\n  d : date  range >=2026-01-01 <=2026-01-31\n",
            &["E111"],
        )
        .beside(&[LAW_142]),
        // ── Computing dates ──
        e(
            "E201",
            tr!("無い日に当たりうる操作に、無い日の扱いが書かれていません", "An operation that can land on a missing day does not say what to do there"),
            tr!(
                "月を足す操作、または日が 29 以上になりうる `day N of month` と `close day N` に、`else` が無いとき。書いたものだけから決めます（範囲の中で一度も当たらなくても出ます）。",
                "Adding months, or `day N of month` and `close day N` with N that can be 29 or more, has no `else`. It is decided from what is written, so it comes even when the range never lands on one."
            ),
            tr!(
                "`else end_of_month`（その月の末日へ）、`else start_of_next_month`（次の月の 1 日へ）、`else reject`（起きないことを検査が確かめる）のどれかを書きます。",
                "Write `else end_of_month` (the end of that month), `else start_of_next_month` (the first of the next) or `else reject` (the check makes sure it never happens)."
            ),
            "dates t v1\n\ninputs\n  d : date  range >=2026-01-01 <=2026-12-31\n\ndate x = d\n  + 1 month\n",
            &["E202", "W201"],
        ),
        e(
            "E202",
            tr!("`else reject` の操作が、範囲の中で無い日に当たります", "An `else reject` operation lands on a missing day in the range"),
            tr!("`else reject` は範囲の中で一度も当たらないことを言う書き方で、当たる入力があるとき。", "`else reject` says it never happens in the range, and an input makes it happen."),
            tr!("ほかの扱いを書くか、範囲を狭めます。", "Say what to do instead, or narrow the range."),
            "dates t v1\n\ninputs\n  d : date  range >=2026-01-01 <=2026-12-31\n\ndate x = d\n  + 1 month else reject\n",
            &["E201"],
        ),
        e(
            "E203",
            tr!("データの範囲の外の日が営業日かを問う計算があります", "A computation asks whether a day outside the data range is a business day"),
            tr!(
                "入力が範囲の中でも、計算の先がカレンダーの表の外に出て、その日が営業日かを問うとき。表はその日を知りません。",
                "For an input in the range, the computation goes past what the calendar's tables know and asks whether that day is a business day."
            ),
            tr!("入力の範囲を、直し方に出る範囲まで狭めるか、新しい表が出てから写しを取り直します。", "Narrow the input's range to the one the fix gives, or take the copy again when a newer table is out."),
            "dates t v1\nuse calendar \"closed_days.cal\"\n\ninputs\n  d : date  range >=2026-12-01 <=2026-12-31\n\ndate x = d\n  + 5 business days\n",
            &["E108"],
        )
        .beside(&[TABLE_2026, HOLIDAYS])
        .english(Repro::File { body: "dates t v1\nuse calendar \"closed_days.cal\"\n\ninputs\n  d : date  range >=2026-12-01 <=2026-12-31\n\ndate x = d\n  + 5 business days\n", beside: &[TABLE_2026_EN, HOLIDAYS_EN] }),
        e(
            "E204",
            tr!("計算した日付が 0001-01-01〜9999-12-31 の外に出ます", "A computed date falls outside 0001-01-01..9999-12-31"),
            tr!("範囲の中の入力で、計算の途中か結果が、扱える日付の外に出るとき。", "For an input in the range, the computation goes outside the dates there are."),
            tr!("範囲を狭めます。", "Narrow the range."),
            "dates t v1\n\ninputs\n  d : date  range >=9999-12-01 <=9999-12-31\n\ndate x = d\n  + 1 day\n",
            &[],
        ),
        e(
            "W201",
            tr!("無い日に当たらない操作に `else` が書かれています", "An operation that never lands on a missing day has an `else`"),
            tr!("`day N of month` や `close day N` の N がどれも 28 以下のとき、または `else` を取らない操作に書いたとき。", "N of `day N of month` or `close day N` is 28 or less, or the operation takes no `else` at all."),
            tr!("`else …` を消します。", "Delete the `else …`."),
            "dates t v1\n\ninputs\n  d : date  range >=2026-01-01 <=2026-01-31\n\ndate x = d\n  day 10 of month +1 else end_of_month\n",
            &["E201"],
        ),
        e(
            "W202",
            tr!("同じ意味の短い書き方があります", "There is a shorter way to write the same thing"),
            tr!(
                "`day 31 of month ±k else end_of_month`（`end of month ±k` と同じ）、`day 1 of month ±k`（`start of month ±k` と同じ）、`close day 31 else end_of_month`（`close end of month` と同じ）と書いたとき。",
                "`day 31 of month ±k else end_of_month` (the same as `end of month ±k`), `day 1 of month ±k` (the same as `start of month ±k`), or `close day 31 else end_of_month` (the same as `close end of month`)."
            ),
            tr!("直し方の短い形で書きます。", "Write the short form the fix gives."),
            "dates t v1\n\ninputs\n  d : date  range >=2026-01-01 <=2026-01-31\n\ndate x = d\n  close day 31 else end_of_month\n",
            &[],
        ),
        // ── Claims and examples ──
        e(
            "E301",
            tr!("条件が成り立たない入力があります", "A claim fails on some inputs"),
            tr!("範囲のすべての入力を計算して、条件が成り立たない入力が一つでもあるとき。", "Computed on every input of the range, the claim fails on at least one."),
            tr!(
                "成り立たない入力を `koyomi eval` で計算して確かめ、決まりか条件か範囲を直します。条件が正しく決まりが違うのか、その逆かは、人が決めます。",
                "Compute an input it fails on with `koyomi eval`, then correct the rule, the claim or the range; which of them is wrong is for a person to decide."
            ),
            "dates t v1\nuse calendar \"weekends.cal\"\n\ninputs\n  d : date  range >=2026-01-01 <=2026-01-31\n\ndate x = d\n  + 1 day\n\nclaims\n  営業日 : x is open\n",
            &["E302"],
        )
        .beside(&[WEEKENDS])
        .english(Repro::File { body: "dates t v1\nuse calendar \"weekends.cal\"\n\ninputs\n  d : date  range >=2026-01-01 <=2026-01-31\n\ndate x = d\n  + 1 day\n\nclaims\n  business_day : x is open\n", beside: &[WEEKENDS_EN] }),
        e(
            "E302",
            tr!("日付が単調ではありません", "A date is not monotonic"),
            tr!("隣り合う二日のうち、後の日のほうが早い結果になる組があるとき。`if closed` を使った日付で起きます。", "On a pair of adjacent days, the later day gives an earlier date. It happens with `if closed`."),
            tr!("`roll` で書けないかを考えます。単調でなくてよいなら、条件を消します。", "See whether `roll` says it; if the date need not be monotonic, delete the claim."),
            "dates t v1\nuse calendar \"weekends.cal\"\n\ninputs\n  d : date  range >=2026-01-01 <=2026-01-31\n\ndate x = d\n  if closed + 3 days\n\nclaims\n  遅いほど遅い : x is monotonic\n",
            &["E301"],
        )
        .beside(&[WEEKENDS])
        .english(Repro::File { body: "dates t v1\nuse calendar \"weekends.cal\"\n\ninputs\n  d : date  range >=2026-01-01 <=2026-01-31\n\ndate x = d\n  if closed + 3 days\n\nclaims\n  later_is_later : x is monotonic\n", beside: &[WEEKENDS_EN] }),
        e(
            "E303",
            tr!("例の値が違います", "An example has a different value"),
            tr!("例の行に書いた日付が、参照インタプリタの計算と違うとき。例の入力が範囲の外のときも。", "A date in an example row differs from what the reference interpreter computes, or the row's input is outside its range."),
            tr!("計算を読んで、例か決まりを直します（直し方に、計算どおりの行が出ます）。", "Read the computation, then correct the example or the rule; the fix gives the row as computed."),
            "dates t v1\n\ninputs\n  d : date  range >=2026-01-01 <=2026-01-31\n\ndate x = d\n  + 1 day\n\nexamples\n| d          | -> x       |\n| 2026-01-01 | 2026-01-03 |\n",
            &["E304"],
        ),
        e(
            "E304",
            tr!("例に入力か日付の列が足りません", "The examples lack a column for an input or a date"),
            tr!("`examples` の表に、入力か日付の列が無いとき。人が書いた値だけが、参照インタプリタと生成したコードに共通の誤りを見つけられます。", "The `examples` table has no column for an input or a date. Only values a person wrote can catch a mistake the reference interpreter and the generated code share."),
            tr!("足りない列を足します（日付の列は `-> <名前>`）。", "Add the columns missing (a date's is `-> <name>`)."),
            "dates t v1\n\ninputs\n  d : date  range >=2026-01-01 <=2026-01-31\n\ndate x = d\n  + 1 day\n\nexamples\n| d          |\n| 2026-01-01 |\n",
            &["E303"],
        ),
        e(
            "E305",
            tr!("検査の予算を超えました（確かめていません）", "The check is over its budget (nothing was checked)"),
            tr!("入力の組み合わせの数（日付の範囲の日数と、整数の入力の範囲の大きさの積）が予算を超えるとき。一部だけ試して通すことはしません。", "The input combinations (the days of the date's range times the size of every integer input's range) are more than the budget. It never tries some and passes."),
            tr!("範囲を狭めるか、ファイルを分けるか、`--budget` で予算を上げます。", "Narrow a range, split the file, or raise the budget with `--budget`."),
            "dates t v1\n\ninputs\n  d : date  range >=0001-01-01 <=9999-12-31\n  n : int   range >=1 <=100\n\ndate x = d\n  + n days\n",
            &[],
        ),
    ];
    Ledger {
        tool: "koyomi",
        example_file: "example.cal",
        fence: "cal",
        repro_heading: tr!("再現", "Example"),
        later_text: Text::default(),
        later_markdown: Text::default(),
        entries,
    }
}

/// The entry of a code, written in either case.
pub fn find(code: &str) -> Option<Entry> {
    ledger().find(code).cloned()
}

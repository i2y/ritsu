//! The ledger of ritsu's own codes (DESIGN 4.3, 7.1): what `ritsu check` says itself, apart from
//! what each language says in its own codes. `ritsu explain` reads it, `docs/codes.md` and
//! `docs/codes.ja.md` are its Markdown, and a test lays out every reproduction — the files of a
//! small project — runs `ritsu check .` on it, and requires the code to come out, so a
//! reproduction cannot go stale while the prose around it still reads well. The entries are
//! ritsu's; how they are written out is ritsu-base's ([`ritsu_base::ledger`]).
//!
//! The numbers go in bands: E1xx for the files of a project as ritsu reads them, E2xx for the
//! checks of the borders between the languages (DESIGN 7.2). A code whose check no project reaches
//! yet (X3 (a), X4, X6 wait for dandori to call koyomi and chobo) has no reproduction yet. A code
//! that is retired keeps its entry, and its number is given to nothing else (DESIGN 7.10).

use ritsu_base::ledger::{Entry, Ledger, Repro};
use ritsu_base::tr;

/// The command a reproduction is run with: in the directory the files are laid out in.
const CHECK: [&str; 3] = ["ritsu", "check", "."];

/// The rule of the reproductions of E201 and W201: a refund never asks for more than was paid.
const REFUND_RULE: &str = "rule refund_check v1\ndescription \"Whether a refund is paid at once or reviewed. A refund never asks for more than was paid, which the rule takes for granted\"\n\nenum path = at_once | review\n\ninputs\n  paid  : number  range >=0 <=10000\n  asked : number  range >=0 <=10000\n\nconstraint asked <= paid\n\noutputs\n  route : path\n\ntable pick\npolicy unique\n| asked | -> route : path |\n| <=100 | at_once         |\n| >100  | review          |\n";

/// The flow of E201: what the customer asks for can be more than was paid.
const REFUND_FLOW: &str = "workflow refund v1\ndescription \"Pays a refund back at once or sends it to review, as the rule decides\"\n\nuse rule check from \"refund_check.rule\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:refund-check\"\n\ninputs\n  order : string\n  paid  : int  range >=0 <=10000\n\ntask ask_amount(order: string) -> int range >=0 <=10000\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:ask-amount\"\n  idempotent\n\ntask pay_back(order: string, amount: int range >=0 <=10000)\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:pay-back\"\n  key\n\nflow\n  let asked = ask_amount(order: order)\n  let decision = check(paid: paid, asked: asked)\n  match decision.route\n    at_once => pay_back(order: order, amount: asked)\n    review => pass\n";

/// The flow of W201: nothing says what range the amount asked for is in.
const REFUND_FLOW_OPEN: &str = "workflow refund v1\ndescription \"Pays a refund back at once or sends it to review, as the rule decides\"\n\nuse rule check from \"refund_check.rule\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:refund-check\"\n\ninputs\n  order : string\n  paid  : int  range >=0 <=10000\n\ntask ask_amount(order: string) -> int\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:ask-amount\"\n  idempotent\n\ntask pay_back(order: string, amount: int range >=0 <=10000)\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:pay-back\"\n  key\n\nflow\n  let asked = ask_amount(order: order)\n  let decision = check(paid: paid, asked: asked)\n  match decision.route\n    at_once => pay_back(order: order, amount: asked)\n    review => pass\n";

pub fn ledger() -> Ledger {
    let entries = vec![
        // ── The files of a project ──
        Entry::new(
            "E101",
            tr!(".proto として読めないファイル", "A file that does not read as a .proto"),
            tr!(
                "プロジェクトの `.proto` を、ritsu の一つの読み手（ritsu-proto）が読めないとき。閉じていない `{{`、`;` の無い文、知らない `syntax`、proto2 の `group`、UTF-8 でないファイル。どの言語もこの読み手で `.proto` を読むので、読めないファイルは、どの言語からも読めません。それを読む言語は、読むところで自分のコードでも言います（rulec の E013、dandori の E016、sakai の E106、yuen の E205）。",
                "A `.proto` of the project that ritsu's one reader of `.proto` files (ritsu-proto) cannot read: a `{{` that is never closed, a statement without its `;`, a `syntax` it does not know, a proto2 `group`, a file that is not UTF-8. Every language reads a `.proto` with that reader, so none of them can read the file; a language that reads it says so where it does, in its own code too (rulec's E013, dandori's E016, sakai's E106, yuen's E205)."
            ),
            tr!(
                "示された位置を直し、proto3 の `.proto` にします。`buf build` が組めるファイルなら、ritsu の読み手も読みます。",
                "Correct it where it points, as a proto3 `.proto`; a file `buf build` builds, ritsu's reader reads."
            ),
            Repro::Dir { files: vec![("shop.proto", "syntax = \"proto3\";\n\npackage shop.v1;\n\nmessage Order {\n  string id = 1;\n")], command: CHECK.to_vec() },
            &[],
        ),
        // ── The borders between the languages (DESIGN 7.2) ──
        Entry::new(
            "E201",
            tr!("規則を呼ぶところで、前提を破る値を渡すことがあります", "A call can give a rule values that break its precondition"),
            tr!(
                "ワークフローが規則を呼ぶところで、dandori が知っている値の範囲の中に、規則の前提（入力どうしの関係 `constraint`）を破る組み合わせがあるとき（X2、DESIGN 7.4）。範囲は dandori が値を入れるすべての場所から集めたもので、dandori の E014 と同じ読み方です。前提を破る呼び出しは、規則の生成したコードが入口で断るので、ワークフローを走らせたときに初めて落ちます。注に、二つの値の範囲と、破る組み合わせを書きます。",
                "Where a workflow calls a rule, the ranges dandori knows for the values it gives hold a combination that breaks one of the rule's preconditions, a relation between two inputs (`constraint`) (X2, DESIGN 7.4). The ranges are dandori's, gathered from every place a value comes from, read as dandori's E014 reads them. The rule's generated code refuses such a call at its door, so it would fail only when the workflow runs. The notes give the two ranges and the combination that breaks it."
            ),
            tr!(
                "呼ぶ前に前提が保たれるよう分岐するか、値の範囲を狭めます（入力やタスクの結果の `range`）。前提のほうが業務に合っていないなら、規則の `constraint` を直します。",
                "Branch so that the precondition holds before the call, or narrow the ranges (the `range` of an input or a task's result). If the precondition is the part that is wrong, correct the rule's `constraint`."
            ),
            Repro::Dir { files: vec![("refund_check.rule", REFUND_RULE), ("refund.flow", REFUND_FLOW)], command: CHECK.to_vec() },
            &["W201"],
        ),
        Entry::new(
            "W201",
            tr!("規則を呼ぶところで、前提が保たれるかを決められません", "Whether a call keeps a rule's precondition cannot be decided"),
            tr!(
                "ワークフローが規則を呼ぶところで、前提が保たれるかを決められないとき（X2）。渡す値に範囲の無いところから来るものがある（タスクの結果に `range` が無い、など）、前提が並びの合計や長さの上限である（dandori は並びの長さを知りません）、前提が koyomi の日付がとる日である（dandori は日付の範囲を運びません）、のどれかです。決められない前提は、規則の生成したコードが、ワークフローを走らせたときに入口で確かめます。",
                "Where a workflow calls a rule, whether a precondition holds cannot be decided (X2): a value comes from a place with no range (a task's result without `range`, say), the precondition bounds the total or the length of a list (dandori knows no list's length), or it is the days of a koyomi date (dandori carries no range of dates). The rule's generated code checks such a precondition at its door when the workflow runs."
            ),
            tr!(
                "値の来るところに範囲を書きます（タスクの結果やワークフローの入力の `range`）。範囲を書けないなら、このままで構いません。規則が実行時に断ります。",
                "Give the place the value comes from a range (the `range` of a task's result or of the workflow's input). Where none can be given, leave it: the rule refuses at run time."
            ),
            Repro::Dir { files: vec![("refund_check.rule", REFUND_RULE), ("refund.flow", REFUND_FLOW_OPEN)], command: CHECK.to_vec() },
            &["E201"],
        ),
        Entry::new(
            "E202",
            tr!("koyomi の日付がとる日が、規則の入力の範囲を外れます", "The days a koyomi date comes to fall outside a rule input's range"),
            tr!(
                "ワークフローが koyomi の日付の結果を規則の日付の入力に渡すとき、koyomi がとりうる日として数えた日のどれかが、規則が宣言した入力の範囲の外にあるとき（X3 の (a)、DESIGN 7.5）。koyomi は範囲のすべての入力で日付を計算するので、外れる日は例として一つに決まります。",
                "Where a workflow gives the result of a koyomi date to a rule's date input, a day koyomi counts that date coming to lies outside the range the rule declares for the input (X3 (a), DESIGN 7.5). koyomi computes the date on every input of its range, so the day outside is an exact example."
            ),
            tr!(
                "規則の入力の範囲を広げるか、規則の範囲を `range from koyomi` にして、koyomi の日をそのまま範囲にします（rulec の §15.174）。",
                "Widen the rule input's range, or make it `range from koyomi`, so that koyomi's days are the range (rulec's §15.174)."
            ),
            Repro::Later,
            &["W202", "E205"],
        ),
        Entry::new(
            "W202",
            tr!("koyomi の日付がとる日が、規則の入力の範囲に収まるかを決められません", "Whether the days of a koyomi date stay inside a rule input's range cannot be decided"),
            tr!(
                "koyomi が日付のとる日を数えられないとき（入力の組み合わせが確かめる数を超える、途中で計算が止まる入力がある）か、規則が入力の範囲を答えないとき（X3 の (a)）。",
                "koyomi does not count the days the date comes to (more input combinations than it checks, or an input where the computation stops), or the rule gives no range for the input (X3 (a))."
            ),
            tr!(
                "koyomi のファイルの入力の範囲を狭めて、koyomi が数えられるようにします。",
                "Narrow the inputs of the koyomi file so that koyomi can count them."
            ),
            Repro::Later,
            &["E202"],
        ),
        Entry::new(
            "E203",
            tr!("規則の出力が、chobo の受け取らない額になることがあります", "A rule's output can be an amount chobo does not take"),
            tr!(
                "ワークフローが規則の数の出力を chobo の振替の額に渡すとき、その出力が 1 未満か 2⁶³ − 1 を超えることがあるとき（X4、DESIGN 7.6）。chobo は範囲の外の額を、断る（業務の結果）のではなく呼び出しの失敗にします。注に、その額になる規則の入力の例（規則のベクタから）を書きます。",
                "Where a workflow gives a rule's numeric output to a chobo transfer as its amount, the output can be below 1 or above 2⁶³ − 1 (X4, DESIGN 7.6). chobo fails such a call rather than refusing it as a business outcome. The notes give an input of the rule that comes to that amount, from the rule's vectors."
            ),
            tr!(
                "規則が返す額を 1 以上にするか（返金などの負の額は、向きの違う振替に分けます）、振替に渡す前に分岐します。",
                "Make the rule's amounts 1 or more (a negative one, such as a refund, is a transfer the other way), or branch before the transfer."
            ),
            Repro::Later,
            &["E204", "W204"],
        ),
        Entry::new(
            "E204",
            tr!("振替が断られうる理由を、タスクが処理していません", "The task does not handle a refusal the transfer can come to"),
            tr!(
                "chobo の探索が、規則の出力の範囲の額で振替の操作が断られる例を見つけた理由を、その振替を呼ぶタスクが宣言したエラーとして処理していないとき（X4）。探索は chobo の検査と同じ深さまでたどり、見つけた理由には、そこへ至る操作の例があります。",
                "chobo's search finds a run in which an operation of the transfer is refused for a reason, with its amounts in the range of the rule's output, and the task that calls the transfer does not handle that reason as an error it declares (X4). The search goes as deep as chobo's check goes, and each reason it finds comes with the operations that lead to it."
            ),
            tr!(
                "その理由をタスクのエラーとして宣言し、処理します。",
                "Declare the reason as an error of the task, and handle it."
            ),
            Repro::Later,
            &["E203", "W204"],
        ),
        Entry::new(
            "W204",
            tr!("タスクが処理する断る理由を、chobo の探索が見つけません", "chobo's search finds no run for a refusal the task handles"),
            tr!(
                "タスクが振替の断る理由として処理しているのに、chobo の探索が、規則の出力の範囲の額でその理由になる例を見つけないとき（X4）。探索は chobo の検査と同じ深さまでしかたどらないので、「起きない」と言えるのはその深さまでです。額や帳簿が決まらないときも、この警告で決められないと言います。",
                "The task handles a reason as a refusal of the transfer, and chobo's search finds no run that comes to it with the amounts in the range of the rule's output (X4). The search goes only as deep as chobo's check does, so it does not happen as far as that depth. A case that cannot be decided at all (no range for the amount, a book that does not pass) is this warning too."
            ),
            tr!(
                "起きない理由なら、タスクのエラーから外します。深い手順でしか起きないなら、そのままで構いません。",
                "If the reason cannot happen, drop it from the task's errors; if it happens only after a longer run, leave it."
            ),
            Repro::Later,
            &["E204"],
        ),
        Entry::new(
            "E205",
            tr!("koyomi の日付に渡す日が、入力かカレンダーのデータの範囲を外れます", "A date given to a koyomi date is outside its input's range or its calendar's data"),
            tr!(
                "ワークフローが koyomi の日付を呼ぶとき、渡す日付の範囲が、koyomi の入力の範囲か、カレンダーがデータを持つ日の範囲を外れるとき（X6、DESIGN 7.8）。注に、外れる日と、何の範囲を外れるかを書きます。",
                "Where a workflow calls a koyomi date, the range of the date it gives falls outside the range of the koyomi input or of the days its calendar has data for (X6, DESIGN 7.8). The notes give the day outside and what it is outside of."
            ),
            tr!(
                "koyomi の入力の範囲を広げるか（カレンダーのデータも足します）、渡す日付の範囲を狭めます。",
                "Widen the koyomi input's range (and the calendar's data), or narrow the range of the date given."
            ),
            Repro::Later,
            &["W205", "E202"],
        ),
        Entry::new(
            "W205",
            tr!("koyomi の日付に渡す日が、範囲に収まるかを決められません", "Whether a date given to a koyomi date stays inside its range cannot be decided"),
            tr!(
                "ワークフローが koyomi の日付に渡す日付の範囲を、dandori が知らないとき（X6）。koyomi が実行時に範囲の外の日付を断ります。",
                "dandori does not know the range of the date a workflow gives a koyomi date (X6). koyomi refuses a date outside its range when the workflow runs."
            ),
            tr!(
                "渡す日付の来るところに範囲を書きます。",
                "Give the place the date comes from a range."
            ),
            Repro::Later,
            &["E205"],
        ),
    ];
    Ledger {
        tool: "ritsu",
        example_file: "",
        fence: "",
        repro_heading: tr!("再現", "Reproduction"),
        later_text: tr!("（ritsu はこのコードをまだ出さないので、再現はありません）", "(ritsu does not print this code yet; it has no reproduction)"),
        later_markdown: tr!("ritsu はこのコードをまだ出さないので、再現はありません", "ritsu does not print this code yet; it has no reproduction"),
        entries,
    }
}

/// The codes `ritsu check` can print, in the order of the ledger.
pub fn codes() -> Vec<&'static str> {
    ledger().entries.iter().filter(|e| !e.is_retired()).map(|e| e.code).collect()
}

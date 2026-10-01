//! A workflow that implements a service of a `.proto` (`workflow … implements <api>.<Service>`,
//! DESIGN 1.14) is held to it, E017. The service names the workflow, and each of its methods says
//! by one of dandori's options how it reaches a run. The method that starts a run takes the
//! workflow's inputs and answers its outputs, by their names in protobuf's JSON, and lists every
//! name a run can fail with. A method that sends an event or answers a callback names the task that
//! waits for it, and sends a value the task reads. The method that asks a run where it is answers
//! what the query `dandori.status` does. Each difference is a diagnostic of its own, where
//! `implements` names the service.

use crate::apis::{self, Api, ApiDoc, MessageView};
use crate::diag::Diag;
use crate::model::*;
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// dandori's options, from the file dandori has in itself: `dandori.v1.Status` is what a method
/// that asks a run where it is answers.
fn options() -> &'static Api {
    static OPTIONS: OnceLock<Api> = OnceLock::new();
    OPTIONS.get_or_init(|| {
        let pf = crate::proto::load_text(crate::proto::OPTIONS_IMPORT, crate::proto::DANDORI_OPTIONS).expect("dandori's options are a .proto dandori reads");
        Api { name: "dandori".into(), doc: ApiDoc::Proto(std::sync::Arc::new(pf)), url: None }
    })
}

/// The status message, by its full name.
const STATUS: &str = "dandori.v1.Status";

/// Names in backquotes, as a sentence lists them: `` `a` and `b` ``, `` `a`, `b` and `c` ``; in
/// Japanese `` `a` と `b` ``, `` `a`・`b`・`c` ``.
fn and_list(names: &[&str]) -> (String, String) {
    let q: Vec<String> = names.iter().map(|n| format!("`{n}`")).collect();
    match q.split_last() {
        Some((last, rest)) if rest.len() == 1 => (format!("{} and {last}", rest[0]), format!("{} と {last}", rest[0])),
        Some((last, rest)) if !rest.is_empty() => (format!("{} and {last}", rest.join(", ")), q.join("・")),
        _ => (q.join(""), q.join("")),
    }
}

/// The fields of a message, as a parenthesis of a diagnostic says them.
fn fields_are(fields: &[(String, bool)]) -> (String, String) {
    if fields.is_empty() {
        return ("it has no fields".into(), "フィールドはありません".into());
    }
    let names: Vec<&str> = fields.iter().map(|(f, _)| f.as_str()).collect();
    (format!("its fields are {}", names.join(", ")), format!("フィールドは {}", names.join("・")))
}

/// The diagnostics of one service, each at the place `implements` names it.
struct Said<'a> {
    s: &'a ServiceUse,
    out: Vec<Diag>,
}

impl Said<'_> {
    fn say(&mut self, en: impl Into<String>, ja: impl Into<String>) {
        self.out.push(Diag::error("E017", self.s.line, self.s.col, en, ja));
    }

    /// Whether the message is one nothing read has, and if so, say so: what takes or answers it
    /// cannot be held to the flow.
    fn unknown(&mut self, view: &MessageView, label: &str) -> bool {
        if view.known() {
            return false;
        }
        let mut d = Diag::error(
            "E017",
            self.s.line,
            self.s.col,
            format!("the message `{}` of `{label}` is not known: a file it may be in was not read", view.name),
            format!("`{label}` のメッセージ `{}` が分かりません。それがあるはずのファイルを読めませんでした", view.name),
        );
        if let ApiDoc::Proto(pf) = &self.s.doc.doc {
            if let Some((en, ja)) = apis::unread_note(pf) {
                d = d.note(en, ja);
            }
        }
        self.out.push(d);
        true
    }
}

pub fn check(m: &Model) -> Vec<Diag> {
    let Some(s) = &m.service else { return vec![] };
    let ApiDoc::Proto(pf) = &s.doc.doc else { return vec![] };
    let Some(svc) = pf.services.iter().find(|x| x.name == s.name) else { return vec![] };
    let full = &s.name;
    let mut said = Said { s, out: Vec::new() };

    // the file that uses dandori's options imports them, as protoc and buf want
    let marked = svc.options.keys().chain(svc.methods.iter().flat_map(|x| x.options.keys())).any(|k| k.starts_with("dandori.v1."));
    if marked && !svc.imports_options {
        let imp = crate::proto::OPTIONS_IMPORT;
        said.say(
            format!("`{}` uses dandori's options without `import \"{imp}\";`, which protoc and buf need", s.file),
            format!("`{}` は `import \"{imp}\";` を書かずに dandori のオプションを使っています。protoc と buf にはこの import が要ります", s.file),
        );
    }

    // the workflow that implements it
    match svc.options.get("dandori.v1.workflow") {
        None => {
            let opt = format!("option (dandori.v1.workflow) = {{name: {}, version: {}}};", serde_json::to_string(&m.name).unwrap(), m.version);
            said.say(format!("`{full}` does not say which workflow implements it; give it `{opt}`"), format!("`{full}` は、どのワークフローが実装するかを言いません。`{opt}` を書いてください"));
        }
        Some(v) => {
            let name = crate::proto::strings_of(&v["name"]).into_iter().next().unwrap_or_default();
            let version = v["version"].as_u64().unwrap_or(0);
            if name != m.name || version != m.version as u64 {
                said.say(
                    format!("`{full}` is implemented by `{name}` v{version}, and this workflow is `{}` v{}", m.name, m.version),
                    format!("`{full}` を実装するのは `{name}` v{version} ですが、このワークフローは `{}` v{} です", m.name, m.version),
                );
            }
        }
    }

    // each method: one message each way, and one mark
    for mt in &s.methods {
        let label = s.label(mt);
        if mt.streams {
            said.say(format!("`{label}` streams; a workflow takes one message and answers one"), format!("`{label}` はストリームです。ワークフローが受け取るのも返すのも、メッセージ一つです"));
        }
        match mt.marks.len() {
            0 => said.say(
                format!("`{label}` does not say how it reaches the workflow; mark it with one of `(dandori.v1.start)`, `(dandori.v1.event)`, `(dandori.v1.answer)` and `(dandori.v1.status)`"),
                format!("`{label}` がワークフローにどう届くかが書かれていません。`(dandori.v1.start)`・`(dandori.v1.event)`・`(dandori.v1.answer)`・`(dandori.v1.status)` のどれか一つを付けてください"),
            ),
            1 => {}
            n => {
                let (en, ja) = and_list(&mt.marks.iter().map(|k| k.option()).collect::<Vec<_>>());
                if n == 2 {
                    said.say(format!("`{label}` has both {en}; a method has one of them"), format!("`{label}` には {ja} の両方があります。一つにしてください"));
                } else {
                    said.say(format!("`{label}` has {en}; a method has one of them"), format!("`{label}` には {ja} があります。一つにしてください"));
                }
            }
        }
    }
    // what is held to the flow: a method with one mark, that takes one message and answers one
    let plain = |mt: &MethodUse| !mt.streams && mt.marks.len() == 1;

    // the method that starts a run
    let starts: Vec<&MethodUse> = s.methods.iter().filter(|x| x.starts()).collect();
    match starts.as_slice() {
        [] => said.say(format!("`{full}` has no method with `(dandori.v1.start)`, which starts a run"), format!("`{full}` に、実行を始めるメソッド（`(dandori.v1.start)`）がありません")),
        [_] => {}
        many => {
            let (en, ja) = and_list(&many.iter().map(|x| x.name.as_str()).collect::<Vec<_>>());
            if many.len() == 2 {
                said.say(format!("`{full}` starts a run by both {en}; one method does"), format!("`{full}` では、{ja} の両方が実行を始めます。一つにしてください"));
            } else {
                said.say(format!("`{full}` starts a run by {en}; one method does"), format!("`{full}` では、{ja} が実行を始めます。一つにしてください"));
            }
        }
    }
    if let [start] = starts.as_slice() {
        let label = s.label(start);
        if plain(start) {
            // what a run starts with: the request, field by field, read into the inputs
            let req = apis::message(&s.doc, &start.request);
            if !said.unknown(&req, &label) {
                let r = req.simple().to_string();
                let fields = req.fields();
                for (f, _) in &fields {
                    if !m.inputs.iter().any(|(i, _)| i == f) {
                        said.say(format!("`{r}` has `{f}`, which is not an input of the workflow"), format!("`{r}` の `{f}` は、ワークフローの入力にありません"));
                    }
                }
                for (i, t) in &m.inputs {
                    if !fields.iter().any(|(f, _)| f == i) {
                        let (en, ja) = fields_are(&fields);
                        said.say(format!("the input `{i}` is not a field of `{r}` ({en})"), format!("入力 `{i}` は `{r}` のフィールドにありません（{ja}）"));
                    } else if let Err((en, ja)) = req.field_reads(i, m, t, m.input_ranges.get(i).copied(), true) {
                        said.say(format!("the input `{i}` does not read `{i}` of `{r}`: {en}"), format!("入力 `{i}` では、`{r}` の `{i}` を読めません。{ja}"));
                    }
                }
            }
            // what a run ends with: the outputs, each written into a field of the response
            let res = apis::message(&s.doc, &start.response);
            if !said.unknown(&res, &label) {
                let r = res.simple().to_string();
                let fields = res.fields();
                for (f, _) in &fields {
                    if !m.outputs.iter().any(|(o, _)| o == f) {
                        said.say(
                            format!("`{r}` has `{f}`, which is not an output of the workflow; the client would read its zero value"),
                            format!("`{r}` の `{f}` は、ワークフローの出力にありません。クライアントはいつもゼロ値を読むことになります"),
                        );
                    }
                }
                for (o, t) in &m.outputs {
                    match fields.iter().find(|(f, _)| f == o) {
                        None => {
                            let (en, ja) = fields_are(&fields);
                            said.say(
                                format!("the output `{o}` is not a field of `{r}` ({en}); protobuf's JSON reader refuses a field it does not know"),
                                format!("出力 `{o}` は `{r}` のフィールドにありません（{ja}）。protobuf の JSON を読む側は、知らないフィールドを拒否します"),
                            );
                        }
                        // an output that may be absent goes to a field that says whether it is set
                        Some((_, false)) if matches!(t, Ty::Opt(_)) => {
                            let inner = m.ty_name(t.inner());
                            let facts = res.field(o).expect("a field of the message");
                            if facts.required {
                                said.say(
                                    format!("the output `{o}` may be absent, and `{o}` of `{r}` is `required`; make the output `{inner}`"),
                                    format!("出力 `{o}` は無いことがありますが、`{r}` の `{o}` は `required` です。出力を `{inner}` にしてください"),
                                );
                            } else if facts.list {
                                said.say(
                                    format!("the output `{o}` may be absent, and `{o}` of `{r}`, a `repeated` field or a `map`, cannot say so; make the output `{inner}`"),
                                    format!("出力 `{o}` は無いことがありますが、`{r}` の `{o}` は `repeated` か `map` のフィールドなので、無いことを表せません。出力を `{inner}` にしてください"),
                                );
                            } else {
                                said.say(
                                    format!("the output `{o}` may be absent, and `{o}` of `{r}`, which is not `optional`, cannot say so; make the field `optional`, or the output `{inner}`"),
                                    format!("出力 `{o}` は無いことがありますが、`{r}` の `{o}` は `optional` でないので、無いことを表せません。フィールドを `optional` にするか、出力を `{inner}` にしてください"),
                                );
                            }
                        }
                        Some(_) => {
                            if let Err((en, ja)) = res.field_sends(o, m, t, m.output_ranges.get(o).copied()) {
                                said.say(format!("the output `{o}` is not what `{o}` of `{r}` takes: {en}"), format!("出力 `{o}` は、`{r}` の `{o}` が受け取るものと合いません。{ja}"));
                            }
                        }
                    }
                }
            }
        }
        // the names a run fails with, both ways: the client learns them from here
        if let Some(Mark::Start { fails }) = start.marks.iter().find(|k| matches!(k, Mark::Start { .. })) {
            let mut flow: Vec<String> = Vec::new();
            for n in crate::contract::fail_names(m) {
                if !flow.contains(&n) {
                    flow.push(n);
                }
            }
            for n in flow.iter().filter(|n| !fails.contains(n)) {
                said.say(
                    format!("the workflow fails with `{n}`, which `fails` of `{}` does not list", start.name),
                    format!("ワークフローは `{n}` で失敗することがありますが、`{}` の `fails` にありません", start.name),
                );
            }
            let mut listed: Vec<&String> = Vec::new();
            for n in fails {
                if !flow.contains(n) && !listed.contains(&n) {
                    said.say(
                        format!("`fails` of `{}` lists `{n}`, and the workflow never fails with it", start.name),
                        format!("`{}` の `fails` に `{n}` がありますが、ワークフローがその名前で失敗することはありません", start.name),
                    );
                }
                listed.push(n);
            }
        }
    }

    // the methods that send an event or answer a callback: the task that waits for it, and the value
    let mut by_task: BTreeMap<(bool, String), Vec<&str>> = BTreeMap::new();
    for mt in s.methods.iter().filter(|x| plain(x)) {
        let (task, event) = match &mt.marks[0] {
            Mark::Event { task } => (task, true),
            Mark::Answer { task } => (task, false),
            _ => continue,
        };
        by_task.entry((event, task.clone())).or_default().push(&mt.name);
        let label = s.label(mt);
        let (what_en, what_ja) = if event {
            (format!("sends the event of `{task}`"), format!("`{task}` のイベントを送りますが"))
        } else {
            (format!("answers the callback of `{task}`"), format!("`{task}` のコールバックに応答しますが"))
        };
        match m.tasks.iter().find(|t| t.name == *task) {
            None => said.say(format!("`{label}` {what_en}, and the workflow has no task of that name"), format!("`{label}` は {what_ja}、その名前のタスクはありません")),
            Some(t) if event && !t.event => said.say(format!("`{label}` {what_en}, and `{task}` is not an `event` task"), format!("`{label}` は {what_ja}、`{task}` は `event` のタスクではありません")),
            Some(t) if !event && !t.callback => said.say(format!("`{label}` {what_en}, and `{task}` is not a `callback` task"), format!("`{label}` は {what_ja}、`{task}` は `callback` のタスクではありません")),
            Some(t) => {
                let req = apis::message(&s.doc, &mt.request);
                if !said.unknown(&req, &label) {
                    if let Some(rt) = &t.result {
                        let r = req.simple().to_string();
                        for (en, ja) in req.reads(m, rt, t.result_range, true) {
                            said.say(format!("`{task}` does not read `{r}` of `{label}`: {en}"), format!("`{task}` では、`{label}` の `{r}` を読めません。{ja}"));
                        }
                    }
                }
            }
        }
        // the workflow answers nothing to what it takes
        let res = apis::message(&s.doc, &mt.response);
        if !said.unknown(&res, &label) && !res.fields().is_empty() {
            let r = res.simple();
            let (en, ja) = if event { ("takes an event", "イベントを受け取っても") } else { ("takes the answer of a callback", "応答を受け取っても") };
            said.say(format!("`{label}` answers `{r}`, which has fields; the workflow {en} and answers nothing"), format!("`{label}` のレスポンス `{r}` にはフィールドがありますが、ワークフローは{ja}何も返しません"));
        }
    }
    for ((event, task), methods) in &by_task {
        if methods.len() < 2 {
            continue;
        }
        let (en, ja) = and_list(methods);
        let (what_en, what_ja) = if *event { (format!("send the event of `{task}`"), format!("`{task}` のイベントを送ります")) } else { (format!("answer the callback of `{task}`"), format!("`{task}` のコールバックに応答します")) };
        let (all_en, all_ja) = if methods.len() == 2 { ("both", "どちらも") } else { ("all", "どれも") };
        said.say(format!("{en} {all_en} {what_en}; one method does"), format!("{ja} が{all_ja}{what_ja}。一つにしてください"));
    }

    // the method that asks a run where it is
    let statuses: Vec<&MethodUse> = s.methods.iter().filter(|x| x.is_status()).collect();
    match statuses.as_slice() {
        [st] if plain(st) => {
            let label = s.label(st);
            let req = apis::message(&s.doc, &st.request);
            if !said.unknown(&req, &label) && !req.fields().is_empty() {
                let r = req.simple();
                said.say(
                    format!("`{label}` takes `{r}`, which has fields; a run is asked where it is by its workflow id alone"),
                    format!("`{label}` のリクエスト `{r}` にはフィールドがありますが、実行がいまどこにいるかは、ワークフロー ID だけで聞きます"),
                );
            }
            let res = apis::message(&s.doc, &st.response);
            if !said.unknown(&res, &label) {
                let r = res.simple().to_string();
                for (en, ja) in res.same_fields(&apis::message(options(), STATUS)) {
                    said.say(format!("`{label}` answers `{r}`, which is not the shape of `{STATUS}`: {en}"), format!("`{label}` のレスポンス `{r}` は、`{STATUS}` の形ではありません。{ja}"));
                }
            }
        }
        [] | [_] => {}
        many => {
            let (en, ja) = and_list(&many.iter().map(|x| x.name.as_str()).collect::<Vec<_>>());
            if many.len() == 2 {
                said.say(format!("`{full}` asks where a run is by both {en}; one method does"), format!("`{full}` では、{ja} の両方が、実行がいまどこにいるかを聞きます。一つにしてください"));
            } else {
                said.say(format!("`{full}` asks where a run is by {en}; one method does"), format!("`{full}` では、{ja} が、実行がいまどこにいるかを聞きます。一つにしてください"));
            }
        }
    }
    said.out
}

//! The checks of security that are sakai's (DESIGN 16; ritsu's DESIGN 16): a key of a known shape
//! written in a `.ctx` (W901), a server of an OpenAPI or AsyncAPI document of the map that does
//! not encrypt the connection (W902), and an operation or a channel of a published language that
//! says nothing of how a client proves who it is (W903). Each is a warning: a key can be a value
//! for tests, a connection can be protected by something the document does not show, and
//! authentication can be put on at a gateway. Each goes away when the file says it is meant:
//! `ritsu: test secret` on the line of the key, `x-ritsu-plaintext: "<why>"` in the server, and
//! `security: []` on the operation, the server or the AsyncAPI operation.
//!
//! The keys of the documents themselves are said once in a project, by `ritsu check` (ritsu-cross),
//! since a `.proto` or a document is read by more than one language. Which operation a policy
//! allows to whom (Cedar, sekisho) is not looked at here.

use crate::contracts::{self, Contracts, Kind};
use crate::diag::{self, Diag, DiagExt, Ref};
use crate::model::Model;
use crate::paths::{self, shown};
use ritsu_base::secrets;
use ritsu_base::text::Text;
use ritsu_base::urls;
use ritsu_base::yaml::Node;
use std::path::Path;

/// W901: the keys written in the `.ctx` files `files` (paths from the root): the map, and the
/// context files it reads. A file that cannot be read is told by the check's other stages.
pub fn keys(root: &Path, files: &[String]) -> Vec<Diag> {
    let mut out = Vec::new();
    let mut seen: Vec<&String> = Vec::new();
    for f in files {
        if seen.contains(&f) {
            continue;
        }
        seen.push(f);
        let Ok(src) = ritsu_base::fs::read_to_string(paths::on_disk(root, f)) else { continue };
        out.extend(secrets::scan(&src).iter().filter(|k| !k.test).map(|k| key(f, k)));
    }
    out
}

/// One key, said by its kind, its prefix and its length. The line is not shown: it holds the key,
/// which would then be in the log of every run.
pub fn key(file: &str, k: &secrets::Found) -> Diag {
    let (name, prefix, n) = (&k.kind.name, &k.shown, k.len);
    let revoke = match k.kind.provider {
        "" => tr!(
            "本物の鍵なら、まず新しい鍵に替えて、古い鍵を使えないようにしてください。ファイルから消しても、リポジトリの履歴には残ります。",
            "If this key is real, replace it with a new one first, and see that the old one is no longer accepted: taking it out of the file leaves it in the history of the repository."
        ),
        p => tr!(
            "本物の鍵なら、まず {p} で無効にしてください。ファイルから消しても、リポジトリの履歴には残ります。",
            "If this key is real, revoke it with {p} first: taking it out of the file leaves it in the history of the repository."
        ),
    };
    diag::at("W901", file, k.line, k.col, tr!("{}がここに書かれています（{prefix}、{n} 文字）", "{} is written here ({prefix}, {n} characters)", name.ja; name.en))
        .note(tr!(
            "ファイルに書いた鍵は、リポジトリとその履歴とビルドを読めるすべての人に渡ります。鍵はコードが動くところ（環境変数、プラットフォームの接続やシークレットの置き場）に置き、そこから読んでください。",
            "A key in a file reaches everyone who can read the repository, its history and its builds. Keep it where the code runs (an environment variable, the platform's connection or secret store) and read it from there."
        ))
        .note(revoke)
        .note(tr!(
            "テスト用の値なら、同じ行のコメントに `{}` と書いてください。",
            "If it is a value for tests, write `{}` in a comment on the same line.",
            secrets::TEST_MARK
        ))
}

/// The methods of a path item.
const METHODS: [&str; 9] = ["get", "put", "post", "delete", "options", "head", "patch", "trace", "query"];

fn text(m: &Model, file: &str) -> String {
    ritsu_base::fs::read_to_string(paths::on_disk(&m.root, file)).unwrap_or_default()
}

/// The name of the context a file belongs to, for the lines of what is involved.
fn owner(m: &Model, file: &str) -> Option<String> {
    crate::owners::context_of(m, file).map(|c| m.contexts[c].name.clone())
}

/// The value of `x-ritsu-plaintext` in a server: Some(true) when it gives a reason (a string that
/// is not empty), Some(false) when it is there with none, None when it is not there.
fn reason(server: &Node) -> Option<bool> {
    server.get("x-ritsu-plaintext").map(|v| v.as_str().is_some_and(|s| !s.trim().is_empty()))
}

/// What a text with server variables in it (`{scheme}://{host}/v1`) comes to: with every variable
/// at its `default`, then with each variable at each value of its `enum`, the others at their
/// defaults. Each with the variable and the value that made it, when one did; a variable with no
/// default is left as written.
fn expansions(written: &str, variables: Option<&Node>) -> Vec<(String, Option<(String, String)>)> {
    let vars = variables.and_then(Node::as_map).unwrap_or_default();
    let default = |v: &Node| v.get("default").and_then(Node::as_str).map(str::to_string);
    let with = |name: Option<(&str, &str)>| -> String {
        let mut out = written.to_string();
        for (k, v) in vars {
            let value = match name {
                Some((n, val)) if n == k.name => Some(val.to_string()),
                _ => default(v),
            };
            if let Some(val) = value {
                out = out.replace(&format!("{{{}}}", k.name), &val);
            }
        }
        out
    };
    let mut out = vec![(with(None), None)];
    for (k, v) in vars {
        for e in v.get("enum").and_then(Node::as_seq).unwrap_or_default() {
            if let Some(val) = e.as_str()
                && Some(val.to_string()) != default(v)
            {
                out.push((with(Some((&k.name, val))), Some((k.name.clone(), val.to_string()))));
            }
        }
    }
    out
}

/// The host of an AsyncAPI server's `host`, without its port: `broker.example.com:9092` is
/// `broker.example.com`, `[::1]:5672` is `[::1]`.
fn host_part(host: &str) -> &str {
    if host.starts_with('[') {
        &host[..host.find(']').map(|e| e + 1).unwrap_or(host.len())]
    } else {
        host.split([':', '/']).next().unwrap_or("")
    }
}

/// The notes every W902 ends with: the encrypted form of the protocol, and how to say the
/// connection is protected another way (or that the reason is missing).
fn plaintext_notes(d: Diag, encrypted: &str, reason: Option<bool>) -> Diag {
    let d = d.note(tr!("暗号化して通信するプロトコルは {encrypted} です。", "The encrypted form of the protocol is {encrypted}."));
    match reason {
        Some(false) => d.note(tr!(
            "`x-ritsu-plaintext` には、ほかの仕組みで守っている理由を、空でない文字列で書いてください。",
            "`x-ritsu-plaintext` takes the reason the connection is safe another way, as a string that is not empty."
        )),
        _ => d.note(tr!(
            "ほかの仕組み（サービスメッシュ、プライベートな接続など）で守っているなら、サーバーに `x-ritsu-plaintext: \"<理由>\"` と書いてください。",
            "If the connection is protected another way (a service mesh, a private link), write `x-ritsu-plaintext: \"<why>\"` in the server."
        )),
    }
}

/// W902: the servers of every document of the map (published or not) that do not encrypt the
/// connection to a host that is not this machine. OpenAPI's servers of the document, of each path
/// item and of each operation (`additionalOperations` too): a `url` of `http://` or `ws://` (a
/// relative `url` says no scheme, and is passed over; a server variable is given its default and
/// each value of its `enum`).
/// AsyncAPI's servers: a `protocol` with an encrypted form of its own (`http`, `ws`, `amqp`,
/// `mqtt`, `stomp`, `kafka`). A server with `x-ritsu-plaintext: "<why>"` is meant so.
pub fn plaintext(m: &Model, cs: &Contracts) -> Vec<Diag> {
    let mut out = Vec::new();
    for (file, d) in &cs.docs {
        if d.part {
            continue;
        }
        let src = text(m, file);
        let sf = shown(file);
        let ctx = owner(m, file);
        match d.kind {
            Kind::OpenApi => {
                // where servers can be written: the document, a path item, an operation
                let mut lists: Vec<(String, &Node)> = Vec::new();
                if let Some(s) = d.root.get("servers") {
                    lists.push(("/servers".to_string(), s));
                }
                for (pk, item) in d.root.get("paths").and_then(Node::as_map).unwrap_or_default() {
                    let base = format!("/paths/{}", contracts::escape(&pk.name));
                    if let Some(s) = item.get("servers") {
                        lists.push((format!("{base}/servers"), s));
                    }
                    for (mk, op) in item.as_map().unwrap_or_default() {
                        if METHODS.contains(&mk.name.as_str())
                            && let Some(s) = op.get("servers")
                        {
                            lists.push((format!("{base}/{}/servers", mk.name), s));
                        }
                    }
                    // OpenAPI 3.2's other methods
                    for (mk, op) in item.get("additionalOperations").and_then(Node::as_map).unwrap_or_default() {
                        if let Some(s) = op.get("servers") {
                            lists.push((format!("{base}/additionalOperations/{}/servers", contracts::escape(&mk.name)), s));
                        }
                    }
                }
                for (at, list) in lists {
                    for (i, server) in list.as_seq().unwrap_or_default().iter().enumerate() {
                        let Some(url) = server.get("url") else { continue };
                        let Some(written) = url.as_str() else { continue };
                        let why = reason(server);
                        if why == Some(true) {
                            continue;
                        }
                        let found = expansions(written, server.get("variables")).into_iter().find_map(|(u, var)| urls::plaintext(&u).map(|(scheme, _)| (u, var, scheme)));
                        let Some((u, var, scheme)) = found else { continue };
                        let encrypted = urls::encrypted_form(&scheme).unwrap_or("https");
                        let mut dg = diag::at("W902", file, url.line, url.col, tr!("{sf} のサーバー {written} は、通信を暗号化しません（{scheme}）", "The server {written} of {sf} does not encrypt the connection ({scheme})"))
                            .source(&src)
                            .note(tr!(
                                "途中のネットワークにいる人は、リクエストとレスポンスと、ヘッダーの鍵を読んだり書き換えたりできます。",
                                "Whoever is on the network between can read and change the requests, the answers, and any key in the headers."
                            ));
                        if u != written {
                            // the variables written in the URL that their defaults filled in
                            let defaults: Vec<(String, String)> = server
                                .get("variables")
                                .and_then(Node::as_map)
                                .unwrap_or_default()
                                .iter()
                                .filter(|(k, _)| written.contains(&format!("{{{}}}", k.name)))
                                .filter_map(|(k, v)| Some((k.name.clone(), v.get("default")?.as_str()?.to_string())))
                                .collect();
                            dg = dg.note(match (var, defaults.as_slice()) {
                                (Some((v, val)), _) => tr!("サーバー変数 `{v}` が `{val}` のとき、URL は {u} です。", "With the server variable `{v}` at `{val}`, the URL is {u}."),
                                (None, [(v, val)]) => tr!("サーバー変数 `{v}` が既定の `{val}` のとき、URL は {u} です。", "With the server variable `{v}` at its default `{val}`, the URL is {u}."),
                                (None, _) => tr!("サーバー変数を既定の値にすると、URL は {u} です。", "With the server variables at their defaults, the URL is {u}."),
                            });
                        }
                        let dg = plaintext_notes(dg, encrypted, why).refer(Ref::name_at(ctx.as_deref(), cs.naming(file, &format!("{at}/{i}")), file, server.line, Text::default()));
                        out.push(dg);
                    }
                }
            }
            Kind::AsyncApi => {
                for (name, node) in d.root.get("servers").and_then(Node::as_map).unwrap_or_default() {
                    let at = format!("/servers/{}", contracts::escape(&name.name));
                    let (sfile, _, server) = cs.definition(file, &at, node);
                    let Some(protocol) = server.get("protocol") else { continue };
                    let Some(p) = protocol.as_str() else { continue };
                    let Some(encrypted) = urls::encrypted_form(p) else { continue };
                    let why = reason(server);
                    if why == Some(true) {
                        continue;
                    }
                    let Some(host) = server.get("host").and_then(Node::as_str) else { continue };
                    if expansions(host, server.get("variables")).iter().all(|(h, _)| urls::is_loopback(host_part(h))) {
                        continue;
                    }
                    let (n, src) = (&name.name, if sfile == *file { src.clone() } else { text(m, &sfile) });
                    let dg = diag::at("W902", &sfile, protocol.line, protocol.col, tr!("{sf} のサーバー {n} は、通信を暗号化しません（{p}）", "The server {n} of {sf} does not encrypt the connection ({p})"))
                        .source(&src)
                        .note(tr!(
                            "途中のネットワークにいる人は、メッセージと、一緒に送る鍵を読んだり書き換えたりできます。",
                            "Whoever is on the network between can read and change the messages, and any key sent with them."
                        ));
                    out.push(plaintext_notes(dg, encrypted, why).refer(Ref::name_at(ctx.as_deref(), cs.naming(file, &at), file, name.line, Text::default())));
                }
            }
        }
    }
    out
}

/// Whether a node has `security` written as a list (an empty one too: it says no authentication
/// is asked, on purpose).
fn has_security(node: &Node) -> bool {
    node.get("security").is_some_and(|s| s.as_seq().is_some())
}

/// W903: the operations of the published languages' OpenAPI documents (under `paths`; a webhook
/// is the API calling a client, and is not looked at) whose `security` (their own, or the
/// document's when they have none) is not written; and the channels of the published languages'
/// AsyncAPI documents that a server without `security` carries, when no operation on the channel
/// has `security` either. A document with no `servers` says nothing of a connection, and a
/// channel that is another document's (a `$ref`) is that document's to say.
pub fn authentication(m: &Model, cs: &Contracts) -> Vec<Diag> {
    let mut out = Vec::new();
    for c in &m.contexts {
        for p in &c.published {
            let pkg = &p.package;
            for (kind, file, _) in &p.contracts {
                let Some(d) = cs.docs.get(file) else { continue };
                match kind {
                    Kind::OpenApi => {
                        if has_security(&d.root) {
                            continue;
                        }
                        for op in cs.operations(file).into_iter().filter(|o| !o.webhook) {
                            let Some((of, _, node)) = cs.locate(file, &op.pointer) else { continue };
                            if has_security(node) {
                                continue;
                            }
                            let name = op.name();
                            let (method, path) = (&op.method, &op.path);
                            out.push(
                                diag::at("W903", &of, node.line, node.col, tr!("公表された言語 {pkg} の操作 {name} に、認証の指定がありません", "The operation {name} of the published language {pkg} says no authentication"))
                                    .source(&text(m, &of))
                                    .note(tr!(
                                        "操作にも文書にも `security` が無いので、契約を読む人には、クライアントがどう認証すればよいかが分かりません。",
                                        "Neither the operation nor the document has `security`, so a reader of the contract cannot tell how a client proves who it is."
                                    ))
                                    .note(tr!(
                                        "操作か文書全体に `security` を書いてください。だれでも呼べるようにわざとしている操作なら、その操作に `security: []` と書いてください。",
                                        "Add `security` to the operation, or to the whole document. If the operation is open to anyone on purpose, write `security: []` on it."
                                    ))
                                    .refer(Ref::name_at(Some(c.name.as_str()), cs.naming(file, &op.pointer), &of, node.line, Text::same(format!("{method} {path}")))),
                            );
                        }
                    }
                    Kind::AsyncApi => {
                        let Some(servers) = d.root.get("servers").and_then(Node::as_map).filter(|s| !s.is_empty()) else { continue };
                        let secured = |name: &str, node: &Node| {
                            let (_, _, def) = cs.definition(file, &format!("/servers/{}", contracts::escape(name)), node);
                            has_security(def)
                        };
                        let actions = cs.actions(file);
                        for ch in cs.channels(file).into_iter().filter(|ch| ch.elsewhere.is_none()) {
                            let at = format!("/channels/{}", contracts::escape(&ch.id));
                            let Some((_, _, def)) = cs.locate(file, &at) else { continue };
                            // the servers the channel says it is on, or all of them
                            let on: Vec<String> = match def.get("servers").and_then(Node::as_seq) {
                                Some(rs) => rs
                                    .iter()
                                    .filter_map(|r| r.get("$ref").and_then(Node::as_str))
                                    .filter_map(|w| contracts::target(file, w).ok())
                                    .filter(|(f, _)| f == file)
                                    .filter_map(|(_, ptr)| ptr.strip_prefix("/servers/").filter(|s| !s.contains('/')).map(|s| s.replace("~1", "/").replace("~0", "~")))
                                    .collect(),
                                None => servers.iter().map(|(k, _)| k.name.clone()).collect(),
                            };
                            let open: Vec<&(ritsu_base::yaml::Key, Node)> = servers.iter().filter(|(k, v)| on.contains(&k.name) && !secured(&k.name, v)).collect();
                            if open.is_empty() {
                                continue;
                            }
                            let operated = actions.iter().filter(|a| a.channel.as_deref() == Some(ch.id.as_str())).any(|a| {
                                let ptr = format!("/operations/{}", contracts::escape(&a.id));
                                cs.locate(file, &ptr).is_some_and(|(of, op, node)| has_security(cs.definition(&of, &op, node).2))
                            });
                            if operated {
                                continue;
                            }
                            let id = &ch.id;
                            let names: Vec<Text> = open.iter().map(|(k, _)| Text::same(k.name.clone())).collect();
                            let list = Text::list(&names);
                            let col = d.root.get("channels").and_then(|c| c.entry(id)).map(|(k, _)| k.col).unwrap_or(1);
                            let mut dg = diag::at(
                                "W903",
                                file,
                                ch.line,
                                col,
                                if open.len() == 1 {
                                    tr!("公表された言語 {pkg} のチャネル {id} に、サーバー {} での認証の指定がありません", "The channel {id} of the published language {pkg} says no authentication on the server {}", list.ja; list.en)
                                } else {
                                    tr!("公表された言語 {pkg} のチャネル {id} に、サーバー {} での認証の指定がありません", "The channel {id} of the published language {pkg} says no authentication on the servers {}", list.ja; list.en)
                                },
                            )
                            .source(&text(m, file))
                            .note(tr!(
                                "サーバーにも、このチャネルの操作にも `security` が無いので、契約を読む人には、クライアントがどう認証すればよいかが分かりません。",
                                "Neither the server nor an operation on the channel has `security`, so a reader of the contract cannot tell how a client proves who it is."
                            ))
                            .note(tr!(
                                "サーバーか、このチャネルの操作に `security` を書いてください。だれでも使えるようにわざとしているなら、サーバーか操作に `security: []` と書いてください。",
                                "Add `security` to the server, or to the operations on the channel. If it is open to anyone on purpose, write `security: []` on the server or on an operation."
                            ))
                            .refer(Ref::name_at(Some(c.name.as_str()), cs.naming(file, &at), file, ch.line, Text::default()));
                            for (k, _) in &open {
                                let at = format!("/servers/{}", contracts::escape(&k.name));
                                dg = dg.refer(Ref::name_at(Some(c.name.as_str()), cs.naming(file, &at), file, k.line, tr!("`security` の無いサーバー", "a server without `security`")));
                            }
                            out.push(dg);
                        }
                    }
                }
            }
        }
    }
    out
}

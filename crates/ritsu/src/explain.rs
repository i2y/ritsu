//! `ritsu explain` (DESIGN 8.1): ritsu's own codes, from its ledger (`ritsu_cross::codes`). The
//! codes of the languages overlap one another, so a language's code is looked up with `ritsu
//! <language> explain`.

use crate::cli;
use ritsu_base::json::Json;
use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;

fn refuse(msg: Text, lang: Lang) -> u8 {
    let head = if lang == Lang::Ja { "エラー" } else { "error" };
    eprintln!("{head}: {}", msg.get(lang));
    2
}

/// `ritsu explain <CODE> | --all [--format markdown|json]`: the exit code.
pub fn command(args: &[String], lang: Lang) -> u8 {
    let table = cli::table();
    let cmd = table.command("explain").expect("explain is in the table");
    let asked = args.iter().enumerate().find_map(|(i, x)| if x == "--lang" { args.get(i + 1).cloned() } else { x.strip_prefix("--lang=").map(str::to_string) });
    let lang = if asked.is_some() { Lang::pick(asked.as_deref(), "RITSU_LANG") } else { lang };
    let a = match table.parse(cmd, args) {
        Ok(a) => a,
        Err(e) => return refuse(e, lang),
    };
    if a.has("--help") {
        print!("{}", table.help_cmd(cmd, lang));
        return 0;
    }
    let ledger = ritsu_cross::codes::ledger();
    let format = a.get("--format");
    if a.has("--all") {
        if !a.pos.is_empty() {
            return refuse(tr!("`ritsu explain` に渡すのは、コードか `--all` のどちらかです", "give `ritsu explain` a code or `--all`, not both"), lang);
        }
        match format {
            Some("markdown") => print!("{}", ledger.render_markdown(lang)),
            Some("json") => println!("{}", Json::arr(ledger.entries.iter().map(|e| ledger.to_json(e, lang))).pretty()),
            _ => {
                for (i, e) in ledger.entries.iter().enumerate() {
                    if i > 0 {
                        println!();
                    }
                    print!("{}", ledger.render_text(e, lang));
                }
            }
        }
        return 0;
    }
    let Some(code) = a.pos.first() else {
        return refuse(tr!("`ritsu explain` には `E101` のようなコードか `--all` が要ります", "`ritsu explain` needs a code such as `E101`, or `--all`"), lang);
    };
    match ledger.find(code) {
        Some(e) => {
            match format {
                Some("markdown") => print!("{}", ledger.render_markdown_one(e, lang)),
                Some("json") => println!("{}", ledger.to_json(e, lang).pretty()),
                _ => print!("{}", ledger.render_text(e, lang)),
            }
            0
        }
        None => refuse(
            tr!(
                "ritsu に `{code}` というコードはありません。言語のコードは `ritsu <言語> explain {code}` で引きます（ritsu のコードは `ritsu explain --all`）",
                "ritsu has no code `{code}`; a language's code is looked up with `ritsu <language> explain {code}` (`ritsu explain --all` lists ritsu's)"
            ),
            lang,
        ),
    }
}

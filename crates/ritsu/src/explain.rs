//! `ritsu explain` (DESIGN 8.1): ritsu's own codes, from its ledger (`ritsu_cross::codes`). The
//! codes of the languages overlap one another, so a language's code is looked up with `ritsu
//! <language> explain`.

use crate::cli;
use ritsu_base::json::Json;
use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;

/// What stops the command, as every command of the toolchain says it (DESIGN 6.2, item 10), and 2.
fn refuse(msg: Text, lang: Lang) -> u8 {
    ritsu_base::cli::refuse(&mut std::io::stderr(), &msg, lang)
}

/// `ritsu explain <CODE>... | --all [--format markdown|json]`: the exit code.
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
        // codes and `--all` are the two ways to call it, not one: what it takes
        if !a.pos.is_empty() {
            return refuse(table.takes(cmd), lang);
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
    if a.pos.is_empty() {
        return refuse(table.takes(cmd), lang);
    }
    match ledger.look_up(&a.pos) {
        Ok(entries) => {
            let shape = match format {
                Some("markdown") => ritsu_base::ledger::Shape::Markdown,
                Some("json") => ritsu_base::ledger::Shape::Json,
                _ => ritsu_base::ledger::Shape::Text,
            };
            print!("{}", ledger.explained(&entries, shape, lang));
            0
        }
        // the words of every command of the toolchain, and where a language's code is looked up
        Err(unknown) => {
            let mut once: Vec<&str> = Vec::new();
            for c in &unknown {
                if !once.contains(&c.as_str()) {
                    once.push(c);
                }
            }
            let codes = once.join(" ");
            let where_ = if once.len() == 1 {
                tr!("（言語のコードは `ritsu <言語> explain {codes}` で引けます）", " (a language's code is looked up with `ritsu <language> explain {codes}`)")
            } else {
                tr!("（言語のコードは `ritsu <言語> explain {codes}` で引けます）", " (a language's codes are looked up with `ritsu <language> explain {codes}`)")
            };
            refuse(ritsu_base::cli::no_codes("ritsu", &unknown).then(&where_), lang)
        }
    }
}

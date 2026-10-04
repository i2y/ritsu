//! The ledger of ritsu's own codes (DESIGN 4.3, 7.1): what `ritsu check` says itself, apart from
//! what each language says in its own codes. `ritsu explain` reads it, `docs/codes.md` and
//! `docs/codes.ja.md` are its Markdown, and a test lays out every reproduction — the files of a
//! small project — runs `ritsu check .` on it, and requires the code to come out, so a
//! reproduction cannot go stale while the prose around it still reads well. The entries are
//! ritsu's; how they are written out is ritsu-base's ([`ritsu_base::ledger`]).
//!
//! The numbers go in bands: E1xx for the files of a project as ritsu reads them, E2xx for the
//! checks of the borders between the languages (DESIGN 7.2; they come in the second part of
//! stage E). A code that is retired keeps its entry, and its number is given to nothing else
//! (DESIGN 7.10).

use ritsu_base::ledger::{Entry, Ledger, Repro};
use ritsu_base::tr;

/// The command a reproduction is run with: in the directory the files are laid out in.
const CHECK: [&str; 3] = ["ritsu", "check", "."];

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

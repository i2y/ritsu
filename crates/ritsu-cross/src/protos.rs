//! The `.proto` files of a project (E101): read with ritsu's one reader of `.proto` files
//! (ritsu-proto, DESIGN 4.10), which every language reads them with. A `.proto` is a file of the
//! project (DESIGN 6.1) that no language checks on its own; each language that reads one says so
//! where it reads it (rulec's E013, dandori's E016, sakai's E106, yuen's E205), and a file that
//! no language reads would otherwise go unsaid.

use ritsu_base::diag::Diag;
use ritsu_base::text::Lang;
use ritsu_base::tr;
use ritsu_ports::Finding;
use ritsu_project::Project;

/// E101 for each `.proto` of the project that does not read: where the reader stopped, and why.
pub fn unread(project: &Project, lang: Lang) -> Vec<Finding> {
    let mut out = Vec::new();
    for f in project.files.iter().filter(|f| f.tool == ritsu_base::naming::Tool::Proto) {
        let disk = ritsu_base::paths::on_disk(&project.root, &f.rel);
        let shown = f.shown.clone();
        let d: Diag = match std::fs::read(&disk).map(String::from_utf8) {
            Ok(Ok(src)) => match ritsu_proto::read(&f.rel, &src) {
                Ok(_) => continue,
                Err(e) => {
                    let why = e.message("ritsu");
                    Diag::at("E101", &shown, e.line, e.col, tr!("{shown} を .proto として読めません: {}", "{shown} does not read as a .proto: {}", why.ja; why.en)).source(&src)
                }
            },
            Ok(Err(_)) => Diag::whole("E101", &shown, tr!("{shown} を .proto として読めません: UTF-8 ではありません", "{shown} does not read as a .proto: it is not UTF-8")),
            Err(e) => Diag::whole("E101", &shown, tr!("{shown} を .proto として読めません: {e}", "{shown} does not read as a .proto: {e}")),
        };
        let d = d.rel(&f.rel).note(tr!(
            "どの言語も、`.proto` を ritsu の一つの読み手で読みます。このファイルは、どの言語からも読めません。",
            "Every language reads a `.proto` with ritsu's one reader; none of them can read this file."
        ));
        out.push(Finding::of(&d, Some(f.rel.clone()), lang));
    }
    out
}

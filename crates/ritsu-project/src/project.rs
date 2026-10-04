//! The files of a project (DESIGN 6.1): every file under the paths given, passing over what every
//! language of ritsu passes over (DESIGN 4.7), each with its language by its extension, and the
//! root its paths are counted from (DESIGN 6.2, item 3).

use ritsu_base::naming::Tool;
use ritsu_base::paths::{self, Shown};
use ritsu_base::text::Text;
use ritsu_base::tr;
use std::path::{Path, PathBuf};

/// The languages of a project's files, in the order they are checked (DESIGN 6.1, step 3): the
/// ones that give facts through the ports first (rulec, koyomi, chobo, geas, and the `.proto`
/// files every language reads with ritsu-proto), then dandori, which receives rules and gives its
/// tasks, then yuen and sakai, which receive from all of them.
pub const ORDER: [Tool; 8] = [Tool::Rulec, Tool::Koyomi, Tool::Chobo, Tool::Geas, Tool::Proto, Tool::Dandori, Tool::Yuen, Tool::Sakai];

/// The language of a file, by its extension: `.rule`, `.cal`, `.book`, `.geas`, `.proto`, `.flow`
/// (`.ja.flow` too), `.req` and `.ctx`. None for any other file, which a project reads only when
/// something in it names the file.
pub fn kind_of(path: &str) -> Option<Tool> {
    let ext = Path::new(path).extension()?.to_str()?;
    ORDER.into_iter().find(|t| t.extension() == Some(ext))
}

/// One file of a project.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct File {
    pub tool: Tool,
    /// From the root.
    pub rel: String,
    /// As this run writes it for a person: from where it runs, the way the paths were given.
    pub shown: String,
}

/// The files under the paths a person gives (`ritsu check <path>...`).
#[derive(Clone, Debug)]
pub struct Project {
    /// Absolute.
    pub root: PathBuf,
    /// How the run writes a path from the root (DESIGN 6.2, item 8).
    pub shown: Shown,
    /// The paths as given.
    pub given: Vec<String>,
    /// `--root`, as given.
    pub root_flag: Option<String>,
    /// In the order of [`ORDER`], then of their paths.
    pub files: Vec<File>,
}

impl Project {
    /// The project of the paths given (`.` when none is), under `--root` when it is given, else
    /// under the nearest directory above the first path that holds a `.git`, else the first path
    /// itself (DESIGN 6.2, item 3). A path that is not there, that is outside the root, or a file
    /// of no language given by name, is what the person has to correct: the error says so. So does
    /// a project with no file of any language, which checks nothing.
    pub fn load(paths: &[String], root_flag: Option<&str>) -> Result<Project, Text> {
        let given: Vec<String> = if paths.is_empty() { vec![".".to_string()] } else { paths.to_vec() };
        for g in &given {
            if !ritsu_base::fs::exists(g) {
                return Err(tr!("`{g}` がありません", "`{g}` is not there"));
            }
        }
        let root = match root_flag {
            Some(r) => {
                let p = paths::absolute(Path::new(r));
                if !ritsu_base::fs::is_dir(&p) {
                    return Err(tr!("`--root {r}` はディレクトリではありません", "`--root {r}` is not a directory"));
                }
                p
            }
            None => paths::find_root(Path::new(&given[0])),
        };
        let shown = Shown::new(&root, &given[0]);
        let mut rels: Vec<(Tool, String)> = Vec::new();
        for g in &given {
            let Some(rel) = paths::from_root(&root, Path::new(g)) else {
                let r = shown.root();
                return Err(tr!("`{g}` はルート {r} の外にあります", "`{g}` is outside the root {r}"));
            };
            if ritsu_base::fs::is_dir(g) {
                let mut under = Vec::new();
                paths::walk(&root, &rel, &[], &mut under);
                rels.extend(under.into_iter().filter_map(|f| kind_of(&f).map(|t| (t, f))));
            } else {
                match kind_of(g) {
                    Some(t) => rels.push((t, rel)),
                    None => {
                        return Err(tr!(
                            "`{g}` は ritsu のどの言語のファイルでもありません（.rule、.flow、.cal、.book、.geas、.req、.ctx、.proto のどれか）",
                            "`{g}` is a file of none of ritsu's languages (.rule, .flow, .cal, .book, .geas, .req, .ctx or .proto)"
                        ));
                    }
                }
            }
        }
        rels.sort_by(|(ta, a), (tb, b)| (rank(*ta), a).cmp(&(rank(*tb), b)));
        rels.dedup();
        if rels.is_empty() {
            let gs = given.join(" ");
            return Err(tr!(
                "{gs} には、ritsu の言語のファイル（.rule、.flow、.cal、.book、.geas、.req、.ctx、.proto）がありません",
                "there is no file of ritsu's languages (.rule, .flow, .cal, .book, .geas, .req, .ctx, .proto) in {gs}"
            ));
        }
        let files = rels.into_iter().map(|(tool, rel)| File { tool, shown: shown.path(&rel), rel }).collect();
        Ok(Project { root, shown, given, root_flag: root_flag.map(str::to_string), files })
    }

    /// The root as the run writes it: `.` when it runs there.
    pub fn root_shown(&self) -> String {
        self.shown.root()
    }

    /// The files of one language, in path order.
    pub fn of(&self, tool: Tool) -> Vec<&File> {
        self.files.iter().filter(|f| f.tool == tool).collect()
    }

    /// The file of the project at `rel` (from the root), if the project holds it.
    pub fn holds(&self, rel: &str) -> Option<&File> {
        self.files.iter().find(|f| f.rel == rel)
    }

    /// How many files of each language, in the order of [`ORDER`], leaving out the languages it
    /// has none of.
    pub fn counts(&self) -> Vec<(Tool, usize)> {
        ORDER.into_iter().map(|t| (t, self.files.iter().filter(|f| f.tool == t).count())).filter(|(_, n)| *n > 0).collect()
    }

    /// The paths given that hold files of `tool`: a file of it, or a directory with one under it.
    /// What a language that checks a whole project at once (yuen) or reads its files through
    /// others (sakai, from its maps) is given, as the person gave it.
    pub fn given_for(&self, tool: Tool) -> Vec<String> {
        self.given
            .iter()
            .filter(|g| match paths::from_root(&self.root, Path::new(g.as_str())) {
                Some(rel) => self.files.iter().any(|f| f.tool == tool && (f.rel == rel || paths::contains(&rel, &f.rel))),
                None => false,
            })
            .cloned()
            .collect()
    }
}

fn rank(t: Tool) -> usize {
    ORDER.iter().position(|o| *o == t).unwrap_or(ORDER.len())
}

//! The files of a project (DESIGN 6.1): every file under the paths given, passing over what every
//! language of ritsu passes over (DESIGN 4.7), each with its language by its extension, and the
//! root its paths are counted from (DESIGN 6.2, item 3).
//!
//! In a module (a root with a `ritsu.mod`, DESIGN 17), the project holds the tree of its
//! dependencies, and a check of the whole module reads every file of every module of the tree
//! ([`Project::with_dependencies`], DESIGN 17.8), each written as it is from the root
//! (`@pricing/rules/fee.rule`, `@pricing/@tax/…`). A walk does not go into a module below the root;
//! when the root is in no module, the modules it passed over are kept, to be said (DESIGN 17.6).

use ritsu_base::modules::{Problem, Tree};
use ritsu_base::naming::Tool;
use ritsu_base::paths::{self, Shown};
use ritsu_base::text::Text;
use ritsu_base::tr;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// The languages of a project's files, in the order they are checked (DESIGN 6.1, step 3): the
/// ones that give facts through the ports first (rulec, koyomi, chobo, geas, and the `.proto`
/// files every language reads with ritsu-proto), then kaname, which receives rules, dates and books
/// and gives nothing yet, then dandori, which receives rules and gives its tasks, then sekisho,
/// which receives rules, dates and workflows and gives its gates, then yuen and sakai, which receive
/// from all of them. kaname comes before dandori, for when a flow calls a function of kaname's
/// (kaname's DESIGN 11.2).
pub const ORDER: [Tool; 10] = [Tool::Rulec, Tool::Koyomi, Tool::Chobo, Tool::Geas, Tool::Proto, Tool::Kaname, Tool::Dandori, Tool::Sekisho, Tool::Yuen, Tool::Sakai];

/// The language of a file, by its extension: `.rule`, `.cal`, `.book`, `.geas`, `.proto`, `.kn`,
/// `.flow` (`.ja.flow` too), `.gate`, `.req` and `.ctx`. None for any other file, which a project
/// reads only when something in it names the file.
pub fn kind_of(path: &str) -> Option<Tool> {
    let ext = Path::new(path).extension()?.to_str()?;
    ORDER.into_iter().find(|t| t.extension() == Some(ext))
}

thread_local! {
    /// Whether the projects this thread loads check the `.geas` files of the module at hand.
    static OWN_CLAIMS: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

/// `f`, run with the `.geas` files of the module at hand left out of the files a project checks,
/// and not counted: the check `ritsu mod push` makes of the copy of what it sends runs no claim, as
/// it runs none of a dependency's (DESIGN 17.12, step 4). A yuen or sakai file that names one still
/// reads it, on the disk.
pub fn without_own_claims<R>(f: impl FnOnce() -> R) -> R {
    struct Back;
    impl Drop for Back {
        fn drop(&mut self) {
            OWN_CLAIMS.with(|c| c.set(true));
        }
    }
    OWN_CLAIMS.with(|c| c.set(false));
    let _back = Back;
    f()
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
    /// The modules the walk passed over (directories below the root with a `ritsu.mod` of their
    /// own), by their paths from the root, when the root has none: each is said in one line
    /// (DESIGN 17.6). Below a root with a `ritsu.mod`, they are passed over without a word.
    pub skipped: Vec<String>,
    /// The tree of the module at hand, when the root has a `ritsu.mod` (DESIGN 17.6, 17.7): read
    /// once, as every language reads it.
    pub tree: Option<Rc<Result<Tree, Problem>>>,
    /// Whether a path given is the root: a check of the whole module (DESIGN 17.8).
    pub whole: bool,
    /// The `.geas` files of the dependencies, whose claims are not run (DESIGN 17.8): left out of
    /// [`Project::files`], and counted.
    pub not_run: Vec<File>,
}

impl Project {
    /// The project of the paths given (`.` when none is), under `--root` when it is given, else
    /// under the nearest directory above the first path that holds a `.git`, else the first path
    /// itself (DESIGN 6.2, item 3). A path that is not there, that is outside the root, or a file
    /// of no language given by name, is what the person has to correct: the error says so. So does
    /// a project with no file of any language, which checks nothing, and one with only the `.geas`
    /// files of dependencies, whose claims are not run (DESIGN 17.8).
    pub fn load(paths: &[String], root_flag: Option<&str>) -> Result<Project, Text> {
        let given: Vec<String> = if paths.is_empty() { vec![".".to_string()] } else { paths.to_vec() };
        // a path that starts with `@`, in a module, is read as a path written in a file is (DESIGN 17.6)
        ritsu_base::modules::given_all(&given)?;
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
        let module = ritsu_base::fs::is_file(root.join(ritsu_base::modules::manifest::FILE));
        let mut rels: Vec<(Tool, String)> = Vec::new();
        let mut skipped: Vec<String> = Vec::new();
        let mut whole = false;
        for g in &given {
            let Some(rel) = paths::from_root(&root, Path::new(g)) else {
                let r = shown.root();
                return Err(tr!("`{g}` はルート {r} の外にあります", "`{g}` is outside the root {r}"));
            };
            whole |= rel == ".";
            if ritsu_base::fs::is_dir(g) {
                let (mut under, mut modules) = (Vec::new(), Vec::new());
                paths::walk_noting(&root, &rel, &[], &mut under, &mut modules);
                rels.extend(under.into_iter().filter_map(|f| kind_of(&f).map(|t| (t, f))));
                if !module {
                    skipped.extend(modules);
                }
            } else {
                match kind_of(g) {
                    Some(t) => rels.push((t, rel)),
                    None => {
                        return Err(tr!(
                            "`{g}` は ritsu のどの言語のファイルでもありません（.rule、.flow、.cal、.book、.geas、.req、.ctx、.gate、.kn、.proto のどれか）",
                            "`{g}` is a file of none of ritsu's languages (.rule, .flow, .cal, .book, .geas, .req, .ctx, .gate, .kn or .proto)"
                        ));
                    }
                }
            }
        }
        skipped.sort();
        skipped.dedup();
        rels.sort_by(|(ta, a), (tb, b)| (rank(*ta), a).cmp(&(rank(*tb), b)));
        rels.dedup();
        if rels.is_empty() {
            let gs = given.join(" ");
            let mut e = tr!(
                "{gs} には、ritsu の言語のファイル（.rule、.flow、.cal、.book、.geas、.req、.ctx、.gate、.kn、.proto）がありません",
                "there is no file of ritsu's languages (.rule, .flow, .cal, .book, .geas, .req, .ctx, .gate, .kn, .proto) in {gs}"
            );
            for m in &skipped {
                let note = paths::other_module_note(&shown.path(m), "ritsu check");
                e = Text::new(format!("{}\n{}", e.ja, note.ja), format!("{}\n{}", e.en, note.en));
            }
            return Err(e);
        }
        let files = rels.into_iter().map(|(tool, rel)| File { tool, shown: shown.path(&rel), rel }).collect();
        let tree = module.then(|| ritsu_base::modules::tree(&root));
        let mut p = Project { root, shown, given, root_flag: root_flag.map(str::to_string), files, skipped, tree, whole, not_run: Vec::new() };
        p.leave_out_claims();
        if p.files.is_empty() {
            let gs = p.given.join(" ");
            return Err(tr!(
                "{gs} には、依存の .geas のファイルしかありません。依存の主張は走らせないので、検査するものがありません",
                "{gs} holds only .geas files of dependencies, whose claims are not run: there is nothing to check"
            ));
        }
        Ok(p)
    }

    /// The project with every file of every dependency of the tree in it, when it reads the whole
    /// module (DESIGN 17.8): each dependency's files as its prefix walks them (`@pricing`,
    /// `@pricing/@tax`), passing over what the dependency does not carry (its `exclude` lines, as
    /// [`ritsu_base::modules::digest::module_files`] leaves them out, for a dependency read from its
    /// path), with its `.geas` files left out to be counted ([`Project::not_run`]). A partial check,
    /// a root with no `ritsu.mod` and a tree that cannot be read come back as they are. What `ritsu
    /// check` reads; `ritsu gen` reads what the files given reach, and no more ([`Project::reached`]).
    pub fn with_dependencies(mut self) -> Project {
        let Some(tree) = self.tree.clone() else { return self };
        let Ok(tree) = tree.as_ref() else { return self };
        if !self.whole {
            return self;
        }
        let mut rels: Vec<(Tool, String)> = self.files.iter().map(|f| (f.tool, f.rel.clone())).collect();
        rels.extend(self.dependency_files(tree));
        rels.sort_by(|(ta, a), (tb, b)| (rank(*ta), a).cmp(&(rank(*tb), b)));
        rels.dedup();
        self.files = rels.into_iter().map(|(tool, rel)| File { tool, shown: self.shown.path(&rel), rel }).collect();
        self.leave_out_claims();
        self
    }

    /// Every file of a language of every dependency of the tree, by its path from the root, as its
    /// prefix walks it, passing over what the dependency does not carry (its `exclude` lines), and
    /// over a dependency a path given names (its files are the ones given).
    pub(crate) fn dependency_files(&self, tree: &Tree) -> Vec<(Tool, String)> {
        let mut out = Vec::new();
        for node in &tree.nodes[1..] {
            if self.given.iter().any(|g| paths::from_root(&self.root, Path::new(g)).is_some_and(|r| ritsu_base::modules::prefix_of(&r).as_deref() == Some(node.prefix.as_str()))) {
                continue;
            }
            let except: Vec<String> = node.manifest.excludes.iter().filter_map(|e| paths::join(&node.prefix, &e.value).ok()).collect();
            let (mut under, mut modules) = (Vec::new(), Vec::new());
            paths::walk_noting(&self.root, &node.prefix, &except, &mut under, &mut modules);
            out.extend(under.into_iter().filter_map(|f| kind_of(&f).map(|t| (t, f))));
        }
        out
    }

    /// Moves the `.geas` files of the dependencies out of the files checked, into [`Project::not_run`]:
    /// their claims are not run (DESIGN 17.8), whether the whole module is read or a path names one.
    pub(crate) fn leave_out_claims(&mut self) {
        let root = self.root.clone();
        let (theirs, mut ours): (Vec<File>, Vec<File>) = std::mem::take(&mut self.files)
            .into_iter()
            .partition(|f| f.tool == Tool::Geas && ritsu_base::modules::under_dependency(&paths::on_disk(&root, &f.rel)));
        if !OWN_CLAIMS.with(std::cell::Cell::get) {
            ours.retain(|f| f.tool != Tool::Geas);
        }
        self.files = ours;
        self.not_run.extend(theirs);
        self.not_run.sort_by(|a, b| a.rel.cmp(&b.rel));
        self.not_run.dedup();
    }

    /// The dependency a file of the project is in, by its path from the root (`@pricing`,
    /// `@pricing/@tax`); None for a file of the module at hand, and for every file when the root has
    /// no `ritsu.mod`.
    pub fn dependency_of(&self, rel: &str) -> Option<String> {
        self.tree.as_ref()?;
        ritsu_base::modules::prefix_of(rel)
    }

    /// The dependencies whose files of `tool` the paths given do not reach, each as the run writes
    /// it (`@lib`), in the order of the tree: what a language that checks a whole project at once
    /// (yuen) or reads its files through others (sakai) is given for them, a module at a time, in a
    /// check of the whole module.
    pub fn dependency_dirs(&self, tool: Tool) -> Vec<String> {
        let Some(Ok(tree)) = self.tree.as_deref() else { return Vec::new() };
        tree.nodes[1..]
            .iter()
            .filter(|n| self.files.iter().any(|f| f.tool == tool && self.dependency_of(&f.rel).as_deref() == Some(n.prefix.as_str())))
            .filter(|n| !self.given.iter().any(|g| paths::from_root(&self.root, Path::new(g)).is_some_and(|r| ritsu_base::modules::prefix_of(&r).as_deref() == Some(n.prefix.as_str()))))
            .map(|n| self.shown.path(&n.prefix))
            .collect()
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
    /// others (sakai, from its maps) is given, as the person gave it. In a module, a directory holds
    /// the files of its own module: a walk of it does not go into a dependency
    /// ([`Project::dependency_dirs`] gives those).
    pub fn given_for(&self, tool: Tool) -> Vec<String> {
        self.given
            .iter()
            .filter(|g| match paths::from_root(&self.root, Path::new(g.as_str())) {
                Some(rel) => self.files.iter().any(|f| f.tool == tool && (f.rel == rel || paths::contains(&rel, &f.rel)) && self.dependency_of(&rel) == self.dependency_of(&f.rel)),
                None => false,
            })
            .cloned()
            .collect()
    }
}

pub(crate) fn rank(t: Tool) -> usize {
    ORDER.iter().position(|o| *o == t).unwrap_or(ORDER.len())
}

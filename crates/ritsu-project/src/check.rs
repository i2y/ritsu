//! Each language's own `check`, run on the files of a project (DESIGN 6.1, step 3; 8.3): in the
//! order of [`crate::ORDER`], so the languages that give facts are checked before the ones that
//! receive them, each with the languages it reads joined (`Joined`). What each language says comes
//! back as its command prints it, a unit at a time (`ritsu_ports::Checked`); `ritsu check` prints
//! it again with the tool's word in each headline.

use crate::joined::Joined;
use crate::project::{ORDER, Project};
use ritsu_base::diag::Severity;
use ritsu_base::json::Json;
use ritsu_base::naming::Tool;
use ritsu_base::text::Lang;
use ritsu_base::tr;
use ritsu_ports::{Checked, Finding, Part, Verdict, Verdicts};
use std::path::Path;
use std::rc::Rc;

impl Project {
    /// What each language's `check` says of the project's files, in the order the languages are
    /// checked: rulec, koyomi, chobo and geas a file at a time; kaname a file at a time, with the
    /// rules, dates files and books it calls read through rulec, koyomi and chobo; dandori a file at
    /// a time, with the rules, dates files and books it reads through the same ports; sekisho a file
    /// at a time, with the rules, dates files, calendars, books and flows it reads
    /// (`Joined::sekisho`); yuen the project's `.req` files as one; sakai each map. The `.proto`
    /// files have no language of their own to check them (ritsu-cross reads them).
    ///
    /// rulec, koyomi, chobo, geas, kaname, dandori and sekisho are given the project's files, as the
    /// person would write them; yuen and sakai, which find their files themselves, the paths given
    /// that hold theirs, with the project's root as `--root`. In a check of a whole module, each
    /// dependency with files of theirs is given too (`@lib`, DESIGN 17.8), as a module is given when
    /// it is checked by itself: yuen checks each module's `.req` files as one project (its names are
    /// a project's, and two modules may each have a requirement of the same name), and sakai checks
    /// the maps of each module with the contexts beside them.
    ///
    /// kaname is checked with no verdicts of the checks across the borders here: a place they decide
    /// waits for them ([`Project::check_with`] hands them over).
    pub fn check(&self, joined: &Joined, lang: Lang) -> Vec<(Tool, Checked)> {
        self.check_with(joined, None, lang)
    }

    /// [`Project::check`], with kaname handed what the checks across the borders came to at the
    /// calls of its modules (`verdicts`, ritsu-cross's `ProgramVerdicts`): the effects of its
    /// functions are decided with them (kaname's DESIGN 4.3, 14.5). ritsu-project does not hold
    /// ritsu-cross, which reads the project through it, so the verdicts are handed in.
    pub fn check_with(&self, joined: &Joined, verdicts: Option<Rc<dyn Verdicts>>, lang: Lang) -> Vec<(Tool, Checked)> {
        let root_arg = self.root_arg();
        let mut out = Vec::new();
        for tool in ORDER {
            let files: Vec<String> = self.of(tool).iter().map(|f| f.shown.clone()).collect();
            if files.is_empty() {
                continue;
            }
            let units = match tool {
                Tool::Rulec => joined.rulec.checked(&self.root, &files, lang),
                Tool::Koyomi => joined.koyomi.checked(&self.root, &files, lang),
                Tool::Chobo => joined.chobo.checked(&self.root, &files, lang),
                Tool::Geas => joined.geas.checked(&self.root, &files, lang),
                Tool::Kaname => joined.kaname.checked_with(&self.root, &files, &joined.ports(), verdicts.clone(), lang),
                Tool::Dandori => joined.dandori.checked_with(&self.root, &files, &joined.ports(), lang),
                Tool::Sekisho => joined.sekisho.checked(&self.root, &files, lang),
                Tool::Yuen => {
                    let engine = yuen::ports::Engine::with(joined.yuen());
                    let ours = self.given_for(Tool::Yuen);
                    let mut units = if ours.is_empty() { Vec::new() } else { engine.checked(&ours, Some(&root_arg), lang) };
                    for d in self.dependency_dirs(Tool::Yuen) {
                        units.extend(engine.checked(std::slice::from_ref(&d), Some(&root_arg), lang));
                    }
                    units
                }
                Tool::Sakai => {
                    let mut args = self.given_for(Tool::Sakai);
                    args.extend(self.dependency_dirs(Tool::Sakai));
                    sakai::run::checked(&args, Some(&root_arg), &joined.sakai(), lang)
                }
                // the standard formats are read by the languages that name them
                Tool::Proto | Tool::Openapi | Tool::Asyncapi | Tool::Cedar | Tool::File => continue,
            };
            out.extend(units.into_iter().map(|u| (tool, u)));
        }
        out
    }

    /// Files what rulec says of a line of a contribution, when it checks the rule the contribution
    /// adds to, under the contribution (rulec files each finding under the file it checks, the rule,
    /// and says where the line is in the JSON's `where`; §15.231), when the project holds the
    /// contribution. In a module, a contribution of the module at hand to a rule of a dependency is
    /// the module's own, and so is what is said of its lines (DESIGN 17.8: what is said at a border
    /// between the module and a dependency is the module's). `ritsu check` and `ritsu gen` count a
    /// check's findings this way, so that the two say the same of a tree.
    ///
    /// An error so filed under a file of the module at hand, from the check of a rule of a
    /// dependency, is given a note that names the rule, when its text does not: the person reads it
    /// at the line of the contribution, and the rule it adds to is in another module.
    pub fn place_findings(&self, units: &mut [(Tool, Checked)], lang: Lang) {
        for (_, u) in units.iter_mut().filter(|(t, _)| *t == Tool::Rulec) {
            let rule = ritsu_base::paths::from_root(&self.root, Path::new(&u.label));
            let of = rule.as_deref().and_then(|r| self.dependency_of(r));
            for p in &mut u.parts {
                let Part::Finding(f) = p else { continue };
                let at = f.json.get("where").and_then(|w| w.get("file")).and_then(|x| x.as_str());
                let Some(rel) = at.and_then(|s| ritsu_base::paths::from_root(&self.root, Path::new(s))).filter(|r| self.holds(r).is_some()) else { continue };
                if let Some(prefix) = &of
                    && rule.as_deref() != Some(rel.as_str())
                    && self.dependency_of(&rel).is_none()
                    && f.severity == Severity::Error
                    && !f.text.contains(&u.label)
                {
                    let label = &u.label;
                    let note = tr!(
                        "この行は、依存 `{prefix}` の規則 `{label}` に足すものです。その規則の検査で見つかりました。",
                        "This line adds to `{label}`, a rule of the dependency `{prefix}`, and the check of that rule found it."
                    );
                    add_note(f, note.get(lang));
                }
                f.file = Some(rel);
            }
        }
    }

    /// Whether the check of a file of a dependency, `u`, failed only for errors
    /// [`Project::place_findings`] filed under other files: each is of a line a contribution of
    /// another file adds to it (DESIGN 17.8). Such a file passes on the line of the dependencies, and
    /// the contribution, whose error it is, fails in its place: `ritsu check @pricing` passes it, and
    /// the error is counted once, with the file it is filed under.
    pub fn failed_for_others(&self, rel: &str, u: &Checked) -> bool {
        if u.verdict != Verdict::Fails || self.dependency_of(rel).is_none() {
            return false;
        }
        let mut errors = u.findings().filter(|f| f.severity == Severity::Error).peekable();
        errors.peek().is_some() && errors.all(|f| f.file.as_deref().is_some_and(|x| x != rel))
    }

    /// The `--root` the languages that take one are given: the one the person gave, else the root
    /// found, written from where the run is.
    pub fn root_arg(&self) -> String {
        self.root_flag.clone().unwrap_or_else(|| self.root_shown())
    }
}

/// A note put at the end of a finding of rulec, as rulec writes its notes: a line under the source
/// in the text, before the blank line that ends it, and one more of the JSON's `notes`.
fn add_note(f: &mut Finding, note: &str) {
    match f.text.strip_suffix("\n\n") {
        Some(head) => f.text = format!("{head}\n {note}\n\n"),
        None => f.text.push_str(&format!(" {note}\n")),
    }
    if let Json::Obj(kv) = &mut f.json {
        match kv.iter_mut().find(|(k, _)| k == "notes") {
            Some((_, Json::Arr(notes))) => notes.push(Json::str(note)),
            Some(_) => {}
            None => kv.push(("notes".to_string(), Json::arr([Json::str(note)]))),
        }
    }
}

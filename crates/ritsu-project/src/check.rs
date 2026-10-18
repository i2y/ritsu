//! Each language's own `check`, run on the files of a project (DESIGN 6.1, step 3; 8.3): in the
//! order of [`crate::ORDER`], so the languages that give facts are checked before the ones that
//! receive them, each with the languages it reads joined (`Joined`). What each language says comes
//! back as its command prints it, a unit at a time (`ritsu_ports::Checked`); `ritsu check` prints
//! it again with the tool's word in each headline.

use crate::joined::Joined;
use crate::project::{ORDER, Project};
use ritsu_base::naming::Tool;
use ritsu_base::text::Lang;
use ritsu_ports::{Checked, Verdicts};
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
    /// that hold theirs, with the project's root as `--root`.
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
                Tool::Yuen => yuen::ports::Engine::with(joined.yuen()).checked(&self.given_for(Tool::Yuen), Some(&root_arg), lang),
                Tool::Sakai => sakai::run::checked(&self.given_for(Tool::Sakai), Some(&root_arg), &joined.sakai(), lang),
                // the standard formats are read by the languages that name them
                Tool::Proto | Tool::Openapi | Tool::Asyncapi | Tool::Cedar | Tool::File => continue,
            };
            out.extend(units.into_iter().map(|u| (tool, u)));
        }
        out
    }

    /// The `--root` the languages that take one are given: the one the person gave, else the root
    /// found, written from where the run is.
    pub fn root_arg(&self) -> String {
        self.root_flag.clone().unwrap_or_else(|| self.root_shown())
    }
}

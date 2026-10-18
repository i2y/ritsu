//! The nine languages, each made once (DESIGN 3.2, 6.4): the engine every language answers the
//! ports with, the index of what the files hold and name, and the ports each language that reads
//! others is handed. `ritsu dandori`, `ritsu yuen`, `ritsu sakai` and `ritsu check` take them from
//! here, so the three readers share one rulec (which keeps each rule it has checked) and one index
//! (which keeps what each file holds and names), and a file is asked once in a run.

use ritsu_base::naming::Tool;
use ritsu_ports::{Index, Rules};
use std::rc::Rc;

/// The engines of the languages that answer the ports, and the index over them.
pub struct Joined {
    pub rulec: Rc<rulec::ports::Engine>,
    pub koyomi: Rc<koyomi::ports::Engine>,
    pub chobo: Rc<chobo::ports::Engine>,
    pub geas: Rc<geas::ports::Engine>,
    pub dandori: Rc<dandori::ports::Engine>,
    /// kaname, which calls the rules, the dates files and the books of `ports` (kaname's DESIGN
    /// 11.2); one check of the project checks each module once, the ones other modules read too.
    pub kaname: Rc<kaname::ports::Engine>,
    /// sekisho, with the languages a gate reads joined: it keeps each gate it checks for the
    /// questions the checks across the borders ask of it after the check (`Gates`).
    pub sekisho: Rc<sekisho::ports::Engine>,
    pub sakai: Rc<sakai::ports::Engine>,
    /// What the files of rulec, koyomi, chobo, geas, dandori, kaname, sekisho and sakai hold, and what
    /// the files of rulec, koyomi, dandori, kaname, sekisho and sakai name outside themselves (and the
    /// files of Cedar written by hand, the operations their schemas guard). yuen's own things are named by no
    /// language yet: yuen's engine reads what it borrows through the other languages' ports, so it
    /// would have to hold this index itself, and the index is left without it until a reader of
    /// requirements needs it (the LSP, DESIGN 15).
    pub index: Rc<Index>,
}

impl Default for Joined {
    fn default() -> Joined {
        Joined::new()
    }
}

impl Joined {
    /// Every language, made once, and the index over them.
    pub fn new() -> Joined {
        Joined::with_budget(None)
    }

    /// The same, with the checks that walk a space held to `budget` (`ritsu check --budget`): the
    /// nodes rulec's check of a rule visits, its tables and its machine alike, and the combinations
    /// sekisho's check of one action walks. `None` leaves each language its own default.
    pub fn with_budget(budget: Option<u64>) -> Joined {
        // rulec reads the days of a koyomi date through koyomi's port (DESIGN 7.5 (b)).
        let rulec = rulec::ports::Engine::with_dates(Joined::dates());
        let rulec = Rc::new(match budget {
            Some(n) => rulec.with_budget(i64::try_from(n).unwrap_or(i64::MAX)),
            None => rulec,
        });
        let koyomi = Rc::new(koyomi::ports::Engine);
        let chobo = Rc::new(chobo::ports::Engine);
        let geas = Rc::new(geas::ports::Engine);
        let dandori = Rc::new(dandori::ports::Engine);
        let kaname = Rc::new(kaname::ports::Engine);
        let sekisho = sekisho::ports::Engine::new(
            ritsu_ports::GatePorts { rules: rulec.clone(), dates: koyomi.clone(), books: chobo.clone(), flows: dandori.clone(), items: dandori.clone() }.into(),
        );
        let sekisho = Rc::new(match budget {
            Some(n) => sekisho.with_budget(n),
            None => sekisho,
        });
        let sakai = Rc::new(sakai::ports::Engine);
        let index = Index::new()
            .with_items(Tool::Rulec, rulec.clone())
            .with_references(Tool::Rulec, rulec.clone())
            .with_items(Tool::Koyomi, koyomi.clone())
            .with_references(Tool::Koyomi, koyomi.clone())
            .with_items(Tool::Chobo, chobo.clone())
            .with_items(Tool::Geas, geas.clone())
            .with_items(Tool::Dandori, dandori.clone())
            .with_references(Tool::Dandori, dandori.clone())
            .with_items(Tool::Kaname, kaname.clone())
            .with_references(Tool::Kaname, kaname.clone())
            .with_items(Tool::Sekisho, sekisho.clone())
            .with_references(Tool::Sekisho, sekisho.clone())
            .with_references(Tool::Cedar, sekisho.clone())
            .with_items(Tool::Sakai, sakai.clone())
            .with_references(Tool::Sakai, sakai.clone());
        Joined { rulec, koyomi, chobo, geas, dandori, kaname, sekisho, sakai, index: Rc::new(index) }
    }

    /// The port of dates, as rulec reads the days of `range from koyomi` with it (DESIGN 7.5
    /// (b)): koyomi's engine holds nothing, so one for each reader is the same one.
    pub fn dates() -> rulec::days::Port {
        std::sync::Arc::new(koyomi::ports::Engine)
    }

    /// The port of rules, as dandori reads a flow's rules with it.
    pub fn rules(&self) -> Rc<dyn Rules> {
        self.rulec.clone()
    }

    /// The port of maps, as ritsu-cross reads sakai's maps with it: their contexts and
    /// relationships, and the context a file belongs to (X14, DESIGN 16.8).
    pub fn maps(&self) -> Rc<dyn ritsu_ports::Maps> {
        self.sakai.clone()
    }

    /// What the project's requirements name, as ritsu-cross reads it (the Cedar written by hand a
    /// requirement points at, for X15): yuen's `References`, with every language yuen reads.
    pub fn requirements(&self) -> Rc<dyn ritsu_ports::References> {
        Rc::new(yuen::ports::Engine::with(self.yuen()))
    }

    /// The port of gates, as ritsu-cross reads sekisho's gates and the Cedar written by hand with
    /// it: the operations an action guards, the workflows a gate names, and how far one is allowed
    /// an action (X15, X16; sekisho's DESIGN 4.6).
    pub fn gates(&self) -> Rc<dyn ritsu_ports::Gates> {
        self.sekisho.clone()
    }

    /// The ports of rules, dates and books, as dandori reads a flow's rules, dates files and books
    /// with them (`ritsu check`, and the checks across the borders a flow crosses), and as kaname
    /// calls a module's rules, dates and transfers (`ritsu kaname`, `ritsu check`).
    pub fn ports(&self) -> ritsu_ports::Ports {
        ritsu_ports::Ports { rules: self.rulec.clone(), dates: self.koyomi.clone(), books: self.chobo.clone() }
    }

    /// Every language yuen reads (yuen's DESIGN 3.1): what a file holds, through the index; the
    /// sources a rule or a calendar pins (`Sources`); the aliases of a rule's and a dates file's
    /// names (`Rules`, `Dates`); and the claims of a spec with their records (`Claims`).
    pub fn yuen(&self) -> yuen::suite::Suite {
        let mut s = yuen::suite::Suite::default();
        s.index = self.index.clone();
        s.sources.insert("rulec".into(), self.rulec.clone());
        s.sources.insert("koyomi".into(), self.koyomi.clone());
        s.rules = Some(self.rulec.clone());
        s.dates = Some(self.koyomi.clone());
        s.claims = Some(self.geas.clone());
        s
    }

    /// Every language sakai reads (sakai's DESIGN 4.1): a rule's enums, its Connect service and
    /// its names (`Rules`), what a rule, a calendar and a workflow name outside themselves and
    /// what a rule holds, through the index, a book's accounts and transfers (`Books`), and a dates
    /// file's dates and its calendar's data (`Dates`).
    pub fn sakai(&self) -> sakai::suite::Suite {
        let mut s = sakai::suite::Suite::default();
        s.index = self.index.clone();
        s.rules = Some(self.rulec.clone());
        s.books = Some(self.chobo.clone());
        s.dates = Some(self.koyomi.clone());
        s
    }

    /// Every language sekisho reads (sekisho's DESIGN 8.1): a rule a computed value calls, what
    /// its outputs come to over the intervals a policy cuts, and its page (`Rules`); a dates file's
    /// dates, a calendar's closed days, and their pages (`Dates`); a book whose transfers an action
    /// guards (`Books`); and dandori for the workflows a gate names — whether a `.flow` passes its
    /// check (`Flows`, read with the rules, dates files and books here) and what it holds (`Items`).
    /// The contracts an action guards need no port (`ritsu_base::openapi`, ritsu-proto).
    pub fn sekisho(&self) -> ritsu_ports::GatePorts {
        ritsu_ports::GatePorts { rules: self.rulec.clone(), dates: self.koyomi.clone(), books: self.chobo.clone(), flows: self.dandori.clone(), items: self.dandori.clone() }
    }
}

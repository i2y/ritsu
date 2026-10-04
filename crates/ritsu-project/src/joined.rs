//! The seven languages, each made once (DESIGN 3.2, 6.4): the engine every language answers the
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
    pub sakai: Rc<sakai::ports::Engine>,
    /// What the files of rulec, koyomi, chobo, geas, dandori and sakai hold, and what the files of
    /// rulec, koyomi, dandori and sakai name outside themselves. yuen's own things are named by no
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
        // rulec reads the days of a koyomi date through koyomi's port (DESIGN 7.5 (b)).
        let rulec = Rc::new(rulec::ports::Engine::with_dates(Joined::dates()));
        let koyomi = Rc::new(koyomi::ports::Engine);
        let chobo = Rc::new(chobo::ports::Engine);
        let geas = Rc::new(geas::ports::Engine);
        let dandori = Rc::new(dandori::ports::Engine);
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
            .with_items(Tool::Sakai, sakai.clone())
            .with_references(Tool::Sakai, sakai.clone());
        Joined { rulec, koyomi, chobo, geas, dandori, sakai, index: Rc::new(index) }
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
    /// what a rule holds, through the index, and a book's accounts and transfers (`Books`).
    pub fn sakai(&self) -> sakai::suite::Suite {
        let mut s = sakai::suite::Suite::default();
        s.index = self.index.clone();
        s.rules = Some(self.rulec.clone());
        s.books = Some(self.chobo.clone());
        s
    }
}

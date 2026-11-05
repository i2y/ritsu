//! The table of what two versions of a rule answer (`rulec::vdiff::map_of`, DESIGN 17.17): the page of
//! a module's version shows it for a rule small enough to have one.
//!
//! - Two inputs: the input of words makes the rows, the other the columns; numeric coordinates side
//!   by side that answer alike are one band, and a value of an enum only one version takes is a row
//!   of its own, with what that version answers.
//! - One input: its only row, with a value only one version takes as one more column.
//! - No table: an answer that moves inside a coordinate (it could not be one cell), three inputs,
//!   and a table too wide to read.

use ritsu_testkit::TempDir;
use rulec::vdiff::{Map, MapCoord, MapSide};

/// The map of `rulec diff` over two versions.
fn map_of(what: &str, old: &str, new: &str) -> Option<Map> {
    let t = TempDir::new(what);
    t.write("old.rule", old);
    t.write("new.rule", new);
    let (o, n) = (t.path().join("old.rule"), t.path().join("new.rule"));
    rulec::cli::diff_whole(o.to_str().unwrap(), n.to_str().unwrap(), ("old", "new"), None).unwrap_or_else(|e| panic!("{what}: {e}")).map
}

fn coord(c: &MapCoord) -> String {
    match c {
        MapCoord::Word(w, MapSide::Both) => w.clone(),
        MapCoord::Word(w, MapSide::OldOnly) => format!("{w} (old)"),
        MapCoord::Word(w, MapSide::NewOnly) => format!("{w} (new)"),
        MapCoord::Band(a, b) => format!("{}..{}", a.as_deref().unwrap_or(""), b.as_deref().unwrap_or("")),
    }
}

/// A map as lines: the columns, then a row each, `old>new` where they differ.
fn lines(m: &Map) -> Vec<String> {
    let mut out = vec![m.cols.coords.iter().map(coord).collect::<Vec<_>>().join(" | ")];
    for (i, row) in m.cells.iter().enumerate() {
        let head = m.rows.as_ref().map(|r| coord(&r.coords[i])).unwrap_or_default();
        let answer = |side: &Option<Vec<(String, String)>>| side.as_ref().map(|v| v.iter().map(|(_, x)| x.clone()).collect::<Vec<_>>().join(",")).unwrap_or_else(|| "-".into());
        let cells: Vec<String> = row
            .iter()
            .map(|c| match (&c.old, &c.new) {
                (Some(_), Some(_)) if c.moved => format!("{}>{}", answer(&c.old), answer(&c.new)),
                (Some(_), Some(_)) | (None, Some(_)) => answer(&c.new),
                _ => answer(&c.old),
            })
            .collect();
        out.push(format!("{head}: {}", cells.join(" | ")));
    }
    out
}

const FEE_OLD: &str = "rule fee v1\ndescription \"A fee\"\n\nenum region = domestic | world\n\ninputs\n  weight : mass[g]  range >=1g <=30_000g\n  region : region\n\noutputs\n  fee : money[GBPc]  round up(1GBPc)\n\ntable by_weight\npolicy unique\n| region   | weight  | -> fee : money[GBPc] |\n| domestic | <=2000g | 395GBPc              |\n| domestic | >2000g  | 695GBPc              |\n| world    | <=2000g | 1850GBPc             |\n| world    | >2000g  | 2950GBPc             |\n";

const FEE_NEW: &str = "rule fee v1\ndescription \"A fee\"\n\nenum region = domestic | world | eu\n\ninputs\n  weight : mass[g]  range >=1g <=30_000g\n  region : region\n\noutputs\n  fee : money[GBPc]  round up(1GBPc)\n\ntable by_weight\npolicy unique\n| region   | weight  | -> fee : money[GBPc] |\n| domestic | <=2500g | 395GBPc              |\n| domestic | >2500g  | 695GBPc              |\n| eu       | <=2500g | 1250GBPc             |\n| eu       | >2500g  | 1950GBPc             |\n| world    | <=2500g | 1850GBPc             |\n| world    | >2500g  | 2950GBPc             |\n";

#[test]
fn two_inputs_make_rows_and_columns() {
    let m = map_of("map-two", FEE_OLD, FEE_NEW).expect("a rule of two inputs has a table");
    let (rows, cols) = (m.rows.as_ref().expect("rows"), &m.cols);
    assert_eq!((rows.col.as_str(), cols.col.as_str()), ("region", "weight"));
    assert_eq!(cols.unit, "g");
    assert_eq!(
        lines(&m),
        [
            "1..2000 | 2001..2500 | 2501..30000",
            "domestic: 395 | 695>395 | 695",
            "world: 1850 | 2950>1850 | 2950",
            "eu (new): 1250 | 1250 | 1950",
        ]
    );
}

#[test]
fn a_value_only_the_old_version_takes_is_a_row_too() {
    let m = map_of("map-gone", FEE_NEW, FEE_OLD).expect("the other way round");
    assert_eq!(
        lines(&m),
        [
            "1..2000 | 2001..2500 | 2501..30000",
            "domestic: 395 | 395>695 | 695",
            "world: 1850 | 1850>2950 | 2950",
            "eu (old): 1250 | 1250 | 1950",
        ]
    );
}

const RATE: &str = "rule rate v1\ndescription \"A rate\"\n\nenum kind = standard | reduced | zero\n\ninputs\n  kind : kind\n\noutputs\n  rate : number  round up(1)\n\ntable by_kind\npolicy unique\n| kind     | -> rate : number |\n| standard | 20               |\n| reduced  | 5                |\n| zero     | 0                |\n";

#[test]
fn one_input_is_one_row() {
    let new = RATE.replace("standard | reduced | zero", "standard | reduced | zero | luxury").replace("| reduced  | 5  ", "| reduced  | 6  ").replace("| zero     | 0                |", "| zero     | 0                |\n| luxury   | 25               |");
    let m = map_of("map-one", RATE, &new).expect("a rule of one input has a table");
    assert!(m.rows.is_none());
    assert_eq!(lines(&m), ["standard | reduced | zero | luxury (new)", ": 20 | 5>6 | 0 | 25"]);
}

#[test]
fn numeric_coordinates_that_answer_alike_are_one_band() {
    let old = "rule band v1\ndescription \"A band\"\n\ninputs\n  age : number  range >=0 <=100\n\noutputs\n  price : money[GBPc]  round up(1GBPc)\n\ntable by_age\npolicy unique\n| age  | -> price : money[GBPc] |\n| <18  | 100GBPc                |\n| >=18 | 200GBPc                |\n";
    let new = old.replace("| <18  |", "| <21  |").replace("| >=18 |", "| >=21 |");
    let m = map_of("map-band", old, &new).expect("one numeric input has a table");
    assert_eq!(lines(&m), ["0..17 | 18..20 | 21..100", ": 100 | 200>100 | 200"]);
    // the same answers on both sides of a step of one version are one band
    let same = old.replace("| >=18 | 200GBPc", "| >=18 | 100GBPc");
    let m = map_of("map-merged", &same, &old.replace("| <18  | 100GBPc", "| <18  | 300GBPc").replace("| >=18 | 200GBPc", "| >=18 | 300GBPc")).expect("a table");
    assert_eq!(lines(&m), ["0..100", ": 100>300"]);
}

#[test]
fn an_answer_that_moves_inside_a_coordinate_has_no_table() {
    let old = "rule per_unit v1\ndescription \"A price per unit\"\n\ninputs\n  qty : number  range >=1 <=100\n\noutputs\n  total : money[GBPc]  round up(1GBPc)\n\ntable unit_price\npolicy unique\n| qty  | -> unit : money[GBPc] |\n| <=10 | 100GBPc               |\n| >10  | 90GBPc                |\n\nresult total = unit * qty\n";
    let new = old.replace("| >10  | 90GBPc  ", "| >10  | 80GBPc  ");
    assert!(map_of("map-moves", old, &new).is_none(), "the total moves with the quantity inside each coordinate");
}

#[test]
fn three_inputs_have_no_table() {
    let old = "rule three v1\ndescription \"Three inputs\"\n\nenum yes = yes | no\n\ninputs\n  a : yes\n  b : yes\n  c : yes\n\noutputs\n  out : number  round up(1)\n\ntable t\npolicy unique\n| a   | b   | c   | -> out : number |\n| yes | yes | yes | 1               |\n| yes | yes | no  | 2               |\n| yes | no  | -   | 2               |\n| no  | -   | -   | 2               |\n";
    let new = old.replace("| yes | 1 ", "| yes | 3 ");
    assert!(map_of("map-three", old, &new).is_none());
}

#[test]
fn a_table_too_wide_to_read_is_none() {
    let values: Vec<String> = (0..10).map(|i| format!("v{i}")).collect();
    let rows = |base: usize| values.iter().enumerate().map(|(i, v)| format!("| {v} | {} |\n", base + i)).collect::<String>();
    let rule = |base: usize| format!("rule wide v1\ndescription \"Ten values\"\n\nenum e = {}\n\ninputs\n  e : e\n\noutputs\n  out : number  round up(1)\n\ntable t\npolicy unique\n| e | -> out : number |\n{}", values.join(" | "), rows(base));
    assert!(map_of("map-wide", &rule(1), &rule(2)).is_none(), "ten columns do not fit a page");
}

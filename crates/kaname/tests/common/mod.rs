//! What the tests share: the languages joined as `ritsu kaname` joins them, and a file checked with
//! them.

#![allow(dead_code)]

pub mod targets;

use kaname::check::{Outcome, Run};
use kaname::suite::Suite;
use ritsu_base::text::Lang;
use std::rc::Rc;
use std::sync::Arc;

/// The ports as ritsu-project's `Joined::ports` makes them: rulec, koyomi and chobo, each through
/// its own engine (rulec reads the days of a koyomi date through koyomi's).
pub fn ports() -> ritsu_ports::Ports {
    ritsu_ports::Ports { rules: Rc::new(rulec::ports::Engine::with_dates(Arc::new(koyomi::ports::Engine))), dates: Rc::new(koyomi::ports::Engine), books: Rc::new(chobo::ports::Engine) }
}

/// Every language joined, as `ritsu kaname` hands them over.
pub fn joined() -> Suite {
    Suite::from(ports())
}

/// The file at `path`, checked with every language joined.
pub fn check(path: &str) -> Rc<Outcome> {
    Run::new(joined()).file(path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// The text `src`, checked as the file `path` with every language joined.
pub fn check_text(path: &str, src: &str) -> Outcome {
    Run::new(joined()).text(path, src)
}

/// The codes it says, in order.
pub fn codes(o: &Outcome) -> Vec<&'static str> {
    o.diags.iter().map(|d| d.code).collect()
}

/// Whether a code is one of the analysis of ranges, the effects or the decisions written in code
/// (E301–W303, E401, W501): what a test of the words, the names or the types leaves to the tests of
/// the analysis.
pub fn analysis_code(code: &str) -> bool {
    matches!(code.get(1..2), Some("3" | "4" | "5"))
}

/// The codes it says, but those of the analysis, in order.
pub fn codes_before_analysis(o: &Outcome) -> Vec<&'static str> {
    o.diags.iter().map(|d| d.code).filter(|c| !analysis_code(c)).collect()
}

/// What `kaname check` prints for it.
pub fn shown(o: &Outcome, lang: Lang) -> String {
    kaname::check::render(o, lang)
}

/// The examples, English first (DESIGN 17): the four that join no other language, then the four that
/// call rules, dates and books.
pub const EXAMPLES: [&str; 16] = [
    "examples/order_flow/order_flow.kn",
    "examples/order_flow/order_flow.ja.kn",
    "examples/stock_diff/stock_diff.kn",
    "examples/stock_diff/stock_diff.ja.kn",
    "examples/handles/handles.kn",
    "examples/handles/handles.ja.kn",
    "examples/catalog/catalog.kn",
    "examples/catalog/catalog.ja.kn",
    "examples/payout/payout.kn",
    "examples/payout/payout.ja.kn",
    "examples/late_payment/late_payment.kn",
    "examples/late_payment/late_payment.ja.kn",
    "examples/media_mail/media_mail.kn",
    "examples/media_mail/media_mail.ja.kn",
    "examples/discount_split/discount_split.kn",
    "examples/discount_split/discount_split.ja.kn",
];

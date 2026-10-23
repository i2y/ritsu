//! The runner with no module of a book: what the tests build puts a main.rs of its own here, with
//! the generated modules beside it (tests/common/runners.rs). This one builds the harness alone.

#[allow(dead_code)]
mod harness;

fn main() {
    harness::main(Vec::new());
}

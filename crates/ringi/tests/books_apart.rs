//! The holds of the examples keyed by the try (DESIGN 12.4), on the SQL chobo writes for the
//! example budget (`examples/shared/budget.book`), in PostgreSQL: whether the server keeps the
//! book in the tickets' database or apart from it, a try sent again gets the same answer and holds
//! nothing twice, a new try after a refusal is checked anew, a hold voided does not stop a later
//! one, and the server voids the hold of a try whose ticket it did not save. Apart, the book's
//! calls commit on their own and the ticket's side is rolled back; in the same database, both are
//! one transaction. The same book keyed by the request and its round, as it was, fails the second
//! and the third once the book is apart (the mutation).

use ritsu_testkit::pg::Postgres;

const TENANT: &str = "'acme'";

/// The SQL chobo writes for the budget `src`, under the schema `schema`.
fn sql_of(src: &str, schema: &str) -> String {
    let src = src.replacen("book budget v1", &format!("book {schema} v1"), 1);
    let (book, d) = chobo::model::load(&src);
    assert!(d.is_empty(), "the budget does not load: {}", d[0].message.en);
    let origin = chobo::target::Origin::named("budget.book", src.as_bytes());
    let files = chobo::target::build(&book.unwrap(), "budget", chobo::target::Target::Postgres, &origin).unwrap_or_else(|d| panic!("{}", d[0].message.en));
    files.into_iter().map(|(_, t)| t).collect::<Vec<_>>().join("\n")
}

/// Where the book is kept, as a server keeps it.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Kept {
    /// In the tickets' database: a call of the book and the ticket's save are one transaction.
    Together,
    /// Apart: a call of the book commits on its own, whatever comes of the ticket.
    Apart,
}

struct Book<'p> {
    pg: &'p Postgres,
    schema: String,
    kept: Kept,
}

impl Book<'_> {
    fn sql(&self, sql: &str) -> String {
        self.pg.sql(sql).unwrap_or_else(|e| panic!("{sql}: {e}")).trim().to_string()
    }

    fn select(&self, call: &str) -> String {
        self.sql(&format!("select result, reason from \"{}\".{call}", self.schema))
    }

    fn hold_call(request: &str, key: &str, amount: i64) -> String {
        format!("\"reserve_hold\"({TENANT}, '{request}', '{key}', 'sales', {amount})")
    }

    /// A try whose ticket is saved: its hold.
    fn saved(&self, request: &str, key: &str, amount: i64) -> String {
        self.select(&Self::hold_call(request, key, amount))
    }

    /// A try whose ticket is not saved (the machine refused it, or the save failed and the try
    /// done again came to another answer): its hold, as the server leaves the book. Together, the
    /// transaction is rolled back with the ticket; apart, the book's call stays, and the server,
    /// once it has kept the try's answer with its idempotency key, voids the hold it made.
    fn unsaved(&self, request: &str, key: &str, amount: i64) -> String {
        let call = Self::hold_call(request, key, amount);
        match self.kept {
            Kept::Together => {
                let out = self.sql(&format!("begin; select result, reason from \"{}\".{call}; rollback;", self.schema));
                out.lines().find(|l| l.contains('|')).unwrap_or_default().to_string()
            }
            Kept::Apart => {
                let held = self.select(&call);
                if held.starts_with("done|") {
                    let void = self.select(&format!("\"reserve_void\"({TENANT}, '{request}', '{key}')"));
                    assert!(void.starts_with("done|"), "{}: the server voids the hold of a try it did not save: {void}", self.schema);
                }
                held
            }
        }
    }

    /// What the department has posted, and what is held of it.
    fn left(&self) -> String {
        self.sql(&format!("select posted, held_out from \"{}\".\"balance_dept_budget\"({TENANT}, 'sales')", self.schema))
    }
}

/// The ways of DESIGN 12.4, on a book whose holds are given `key(try, round)` for the key's second
/// parameter: what each answered, in order, for the test to compare.
fn four_ways(b: &Book, key: &dyn Fn(&str, u32) -> String) -> Vec<String> {
    let mut said = Vec::new();
    b.sql(&format!("select * from \"{}\".\"allocate_do\"({TENANT}, 'sales', '2026', 200000)", b.schema));
    // 1. a try sent again: the same answer, held once
    said.push(b.saved("T-1", &key("a1", 1), 1_000));
    said.push(b.saved("T-1", &key("a1", 1), 1_000));
    said.push(b.left());
    // 2. refused for the budget, then a new try of the same round with less
    said.push(b.unsaved("T-2", &key("a2", 1), 300_000));
    said.push(b.saved("T-2", &key("a3", 1), 64_000));
    said.push(b.left());
    // 3 and 4. a try whose ticket was not saved, its hold voided by the server, then a new try of
    //    the same round
    said.push(b.unsaved("T-3", &key("a4", 1), 5_000));
    said.push(b.left());
    said.push(b.saved("T-3", &key("a5", 1), 1_000));
    said.push(b.left());
    said
}

#[test]
fn holds_keyed_by_the_try_are_right_with_the_book_anywhere() {
    if !ritsu_testkit::level::need(ritsu_testkit::level::Need::Postgres) {
        return;
    }
    let pg = match Postgres::start() {
        Ok(pg) => pg,
        Err(why) => {
            ritsu_testkit::skip::skip(&format!("{why}; the holds keyed by the try are not tried on PostgreSQL"));
            return;
        }
    };
    let src = std::fs::read_to_string("examples/shared/budget.book").unwrap();
    assert!(src.contains("key request, attempt"), "the example budget is keyed by the try");
    let by_round = src.replace("reserve(request: string, attempt: string,", "reserve(request: string, round: string,").replace("key request, attempt", "key request, round");
    let try_key = |t: &str, _: u32| t.to_string();
    let round_key = |_: &str, r: u32| r.to_string();
    let mut answers = Vec::new();
    for (schema, kept, text, key) in [
        ("together", Kept::Together, &src, &try_key as &dyn Fn(&str, u32) -> String),
        ("apart", Kept::Apart, &src, &try_key),
        ("round_together", Kept::Together, &by_round, &round_key),
        ("round_apart", Kept::Apart, &by_round, &round_key),
    ] {
        let sql = sql_of(text, schema);
        let file = std::env::temp_dir().join(format!("ringi-books-apart-{}-{schema}.sql", std::process::id()));
        std::fs::write(&file, &sql).unwrap();
        let out = ritsu_testkit::run::run(pg.psql("postgres").arg("-f").arg(&file), std::time::Duration::from_secs(300));
        let _ = std::fs::remove_file(&file);
        assert!(out.ok, "{schema}: {}", out.stderr);
        let b = Book { pg: &pg, schema: schema.to_string(), kept };
        answers.push((schema, four_ways(&b, key)));
    }
    let want = [
        // 1
        "done|",
        "done_before|",
        "200000|1000",
        // 2
        "refused|over_budget",
        "done|",
        "200000|65000",
        // 3 and 4
        "done|",
        "200000|65000",
        "done|",
        "200000|66000",
    ];
    for (schema, said) in &answers[..2] {
        assert_eq!(said, &want, "{schema}: keyed by the try, the four ways");
    }
    // keyed by the round, together still holds; apart, the second and the third fail
    assert_eq!(answers[2].1, want, "round_together");
    let apart = &answers[3].1;
    assert_ne!(apart[4], "done|", "keyed by the round and apart, a new try after a refusal is refused again: {apart:?}");
    assert_ne!(apart[8], "done|", "keyed by the round and apart, a voided key stops the later hold: {apart:?}");
    eprintln!("keyed by the try, the book in the tickets' database and apart: {} answers each, the same; keyed by the round and apart: `{}` and `{}`", want.len(), apart[4], apart[8]);
}

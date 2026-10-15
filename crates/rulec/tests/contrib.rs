//! The merging of files (§15.227–§15.231): open values and the contributions to them, open enums
//! and the values a contribution adds, open definitions and the clauses that take precedence over
//! them, the pins of what a contribution relies on, and what the merged rule is for everything
//! downstream — the evaluator, the generated code, the certificate, the page.

use ritsu_testkit::{Need, TempDir, ready, skip};
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn have(cmd: &str) -> bool {
    Command::new(cmd).arg("--version").output().is_ok_and(|o| o.status.success())
}

fn run_in(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").current_dir(dir).args(args).output().expect("cannot start rulec");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

fn corpus() -> PathBuf {
    root().join("tests/corpus")
}

/// The rule at `rule` from `dir`, merged with the contributions among `set`, prepared as `rulec gen`
/// prepares it.
fn merged(dir: &Path, rule: &str, set: &[&str]) -> (rulec::ast::RuleFile, rulec::types::Checked) {
    let files: Vec<String> = set.iter().map(|s| dir.join(s).to_string_lossy().into_owned()).collect();
    let path = dir.join(rule).to_string_lossy().into_owned();
    let src = std::fs::read_to_string(&path).unwrap();
    rulec::link::with_set(&files, || rulec::prepare(&src, &path)).unwrap_or_else(|ds| panic!("{rule}: {:?}", ds.iter().map(|d| (d.code, d.title.clone())).collect::<Vec<_>>()))
}

/// The answers of a merged rule are the answers of the list file that writes the same thing the
/// way it had to be written before — the rule applied, and what each contribution adds put together
/// by hand — on every vector of the merged rule:
/// a sum (`sum`), the sums of two kinds multiplied (`kinds`), the largest of a walk and a pairing
/// with a team size added (`largest`), a clause over an open table and a second over the first
/// (`override`), and a cost of each vehicle a walk adds up with a surcharge in it (`walk`).
#[test]
fn the_merged_rule_answers_as_the_list_of_what_was_loaded() {
    for (case, rule, contribs) in [
        ("sum", "site_capacity.rule", &["contrib/mezzanine.rule"][..]),
        ("kinds", "reward_points.rule", &["contrib/holiday.rule"][..]),
        ("largest", "mentor_bonus.rule", &["contrib/pairing.rule", "contrib/team_size.rule"][..]),
        ("override", "standing.rule", &["contrib/strict_policy.rule", "contrib/grace_policy.rule"][..]),
        ("walk", "fleet_upkeep.rule", &["contrib/aging.rule", "contrib/green.rule"][..]),
    ] {
        let dir = root().join("tests/merging").join(case);
        let mut args = vec!["check", rule];
        args.extend_from_slice(contribs);
        let (c, out, _) = run_in(&dir, &args);
        assert_eq!(c, 0, "{case}: {out}");
        let n = contribs.len();
        assert!(out.contains(&format!("({n} contribution")), "{case}: {out}");
        let (f, t) = merged(&dir, rule, contribs);
        assert_eq!(f.contributions.len(), n, "{case}");
        let list = dir.join("list.rule").to_string_lossy().into_owned();
        let lsrc = std::fs::read_to_string(&list).unwrap();
        let (lf, lt) = rulec::prepare(&lsrc, &list).unwrap_or_else(|ds| panic!("{case}/list.rule: {:?}", ds.iter().map(|d| d.code).collect::<Vec<_>>()));
        let vs = rulec::vectors::generate(&f, &t);
        assert!(vs.len() >= 4, "{case}: {} vectors", vs.len());
        for v in &vs {
            let (want, _, _, _) = rulec::eval::run_all_traced(&f, &t, v.input.clone().into_iter().collect());
            let (got, _, _, _) = rulec::eval::run_all_traced(&lf, &lt, v.input.clone().into_iter().collect());
            assert_eq!(got, want, "{case}: the list file and the merged rule part at {:?}", v.input);
        }
    }
}

/// The product line: a kind of product added from a file of its own reaches every rule that reads
/// the kinds. Without the clauses that give it a markup and a tax rate, the two open tables have no
/// row for it (E101, naming the contribution that added it), and without `default` the shipping
/// table that is not open lets it fall to its `-` row in silence (E033) — in English and in
/// Japanese.
#[test]
fn a_kind_a_product_line_adds_asks_every_table_that_reads_kinds_for_a_row() {
    for (kinds, price, tax, ship, kind_c, price_c, tax_c, value) in [
        ("shared/product_kinds.rule", "product_price.rule", "product_tax.rule", "product_shipping.rule", "contrib/product_line/garden_kind.rule", "contrib/product_line/garden_price.rule", "contrib/product_line/garden_tax.rule", "garden"),
        ("shared/商品の種類.rule", "商品の料金.rule", "商品の税.rule", "商品の配送.rule", "contrib/商品ライン/園芸の種類.rule", "contrib/商品ライン/園芸の料金.rule", "contrib/商品ライン/園芸の税.rule", "園芸"),
    ] {
        let d = corpus();
        let (c, out, _) = run_in(&d, &["check", kinds, price, tax, ship, kind_c, price_c, tax_c]);
        assert_eq!(c, 0, "{out}");
        // Without the price and the tax of the new kind: E101 in both, saying who added it.
        let (c, out, _) = run_in(&d, &["check", price, tax, kind_c]);
        assert_eq!(c, 1, "{out}");
        assert_eq!(out.matches("error[E101]").count(), 2, "{out}");
        assert!(out.contains(&format!("{value} is a value")), "{out}");
        // Without `default`: E033 in the shipping table, at its `-` row.
        let t = TempDir::new("contrib-e033");
        let src = std::fs::read_to_string(d.join(kind_c)).unwrap().replace(" default\n", "\n");
        let here = t.path().join("kind.rule");
        let to = std::fs::read_to_string(d.join(kind_c)).unwrap();
        let target = to.lines().find(|l| l.starts_with("to ")).unwrap().split('"').nth(1).unwrap().to_string();
        let abs = d.join(Path::new(kind_c).parent().unwrap()).join(&target).canonicalize().unwrap();
        std::fs::write(&here, src.replace(&format!("\"{target}\""), &format!("\"{}\"", abs.display()))).unwrap();
        let (c, out, _) = run_in(&d, &["check", ship, here.to_str().unwrap()]);
        assert_eq!(c, 1, "{out}");
        assert!(out.contains("error[E033]") && out.contains(&format!("{value} falls to this row in silence")), "{out}");
    }
}

/// An open value is read over its declared range, not over what its terms reach: the rule alone has
/// no term past 10%, and a row for a bonus of 50% or more is still a row some contribution reaches,
/// not E102. A contribution whose term would take the value past its range is E131, with the term
/// and an input that does it; one that adds a value to an enum that is not open, or a definition to
/// an output that is not, is E067; an `open` line with no range is E068.
#[test]
fn an_open_value_is_its_declared_range_and_contributions_keep_inside_it() {
    let t = TempDir::new("contrib-open");
    let d = t.path();
    std::fs::write(
        d.join("bonus.rule"),
        "rule bonus_tier(bonus_tier) v1\n\nenum tier(tier) = low(low) | mid(mid) | high(high)\n\ninputs\n  member(member) : bool\n\noutputs\n  tier(tier) : tier\n\n\
         table member_rate\npolicy unique\n| member | -> member_bonus : rate[step 1%] |\n| true   | 10%                             |\n| false  | 0%                              |\n\n\
         open bonus : rate[step 1%]  combine sum  range >=0% <=60%\n  member_bonus\n\n\
         table tier_of\npolicy unique\n| bonus      | -> tier |\n| <20%       | low     |\n| >=20% <50% | mid     |\n| >=50%      | high    |\n",
    )
    .unwrap();
    let (c, out, _) = run_in(d, &["check", "bonus.rule"]);
    assert_eq!(c, 0, "the rule alone is checked over 0% to 60%, and every row is some value's: {out}");
    std::fs::write(
        d.join("promo.rule"),
        "contribution promo(promo) v1\nto \"bonus.rule\"\n\ninputs\n  promo(promo) : bool\n\n\
         table promo_rate\npolicy unique\n| promo | -> promo_bonus : rate[step 1%] |\n| true  | 55%                            |\n| false | 0%                             |\n\n\
         into bonus\n  promo_bonus\n",
    )
    .unwrap();
    let (c, _, _) = run_in(d, &["source", "pin", "promo.rule"]);
    assert_eq!(c, 0);
    let (c, out, _) = run_in(d, &["check", "bonus.rule", "promo.rule"]);
    assert_eq!(c, 1, "{out}");
    assert!(out.contains("error[E131]") && out.contains("promo:promo_bonus: 0% .. 55% (contribution promo (promo.rule))") && out.contains("member = true, promo = true"), "{out}");
    // E067: an `extend` of an enum that is not open
    std::fs::write(d.join("tiers.rule"), "contribution tiers(tiers) v1\nto \"bonus.rule\"\n\nextend tier with top\n").unwrap();
    let (_, out, _) = run_in(d, &["check", "tiers.rule"]);
    assert!(out.contains("error[E067]: The enum tier is not open"), "{out}");
    // E067: a clause for an output no definition opens
    std::fs::write(d.join("clause.rule"), "contribution clause_c(clause_c) v1\nto \"bonus.rule\"\n\nclause always_high -> tier\n  when member true\n  then high\n").unwrap();
    let (_, out, _) = run_in(d, &["check", "clause.rule"]);
    assert!(out.contains("error[E067]") && out.contains("none of the tables and clauses that decide tier is marked `open`"), "{out}");
    // E068: an `open` line without its range
    let src = std::fs::read_to_string(d.join("bonus.rule")).unwrap().replace("  range >=0% <=60%", "");
    std::fs::write(d.join("bonus.rule"), src).unwrap();
    let (_, out, _) = run_in(d, &["check", "bonus.rule"]);
    assert!(out.contains("error[E068]: The open value bonus declares no range"), "{out}");
}

/// The pins (§15.230). A contribution's `signature:` is the shape of what it relies on: a row of the
/// rule moves no signature, a range of an input it reads does, and E040 says what moved, read from
/// the version in the repository's history it was pinned at. An `apply`'s `meaning:` is the
/// callee's lines without comments: a comment moves nothing, a row does.
#[test]
fn a_pin_moves_with_what_is_relied_on_and_says_what_moved() {
    if !have("git") {
        skip("no git, so the version a pin was taken at cannot be read back");
        return;
    }
    let t = TempDir::new("contrib-pins");
    let d = t.path();
    let git = |args: &[&str]| {
        let o = Command::new("git").current_dir(d).args(args).env("GIT_AUTHOR_NAME", "t").env("GIT_AUTHOR_EMAIL", "t@example.com").env("GIT_COMMITTER_NAME", "t").env("GIT_COMMITTER_EMAIL", "t@example.com").output().unwrap();
        assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    };
    let base = "rule fee(fee) v1\n\ninputs\n  size(size) : number  range >=1 <=10\n\noutputs\n  total(total) : number  round down(1)\n\n\
                table unit_of\npolicy unique\n| size | -> unit : number |\n| <=5  | 2               |\n| >5   | 3               |\n\n\
                open extra : number  combine sum  range >=0 <=50\n\ndefine total : number = size * unit + extra\n";
    std::fs::write(d.join("fee.rule"), base).unwrap();
    std::fs::write(d.join("handling.rule"), "contribution handling(handling) v1\nto \"fee.rule\"\n\ntable handling_of\npolicy unique\n| size | -> handling : number |\n| <=3  | 1                   |\n| >3   | 4                   |\n\ninto extra\n  handling\n").unwrap();
    std::fs::write(d.join("caller.rule"), "rule caller(caller) v1\n\ninputs\n  n(n) : number  range >=1 <=10\n\noutputs\n  y(y) : number  round down(1)\n\napply f = \"fee.rule\"\n  size = n\n  total -> y\n").unwrap();
    for p in ["handling.rule", "caller.rule"] {
        let (c, _, e) = run_in(d, &["source", "pin", p]);
        assert_eq!(c, 0, "{e}");
    }
    assert!(std::fs::read_to_string(d.join("caller.rule")).unwrap().contains("meaning:"), "a new pin is a meaning");
    git(&["init", "-q"]);
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "pinned"]);
    let (c, out, _) = run_in(d, &["check", "fee.rule", "handling.rule", "caller.rule"]);
    assert_eq!(c, 0, "{out}");
    // A comment and a row of the table: the contribution's signature does not move; the apply's
    // meaning does for the row, not for the comment.
    std::fs::write(d.join("fee.rule"), base.replace("table unit_of\n", "# the unit, by size\ntable unit_of\n")).unwrap();
    let (c, out, _) = run_in(d, &["check", "fee.rule", "handling.rule", "caller.rule"]);
    assert_eq!(c, 0, "a comment moves no pin: {out}");
    std::fs::write(d.join("fee.rule"), base.replace("| >5   | 3               |", "| >5   | 4               |")).unwrap();
    let (c, out, _) = run_in(d, &["check", "handling.rule"]);
    assert_eq!(c, 0, "a row moves no signature: {out}");
    let (c, out, _) = run_in(d, &["check", "caller.rule"]);
    assert_eq!(c, 1, "{out}");
    assert!(out.contains("error[E040]: The callee `fee.rule` has changed") && out.contains("`| >5 | 3 |` is now `| >5 | 4 |`"), "the meaning says the row that moved: {out}");
    // The range of an input the contribution reads moves its signature, and E040 says which.
    std::fs::write(d.join("fee.rule"), base.replace("range >=1 <=10", "range >=1 <=20")).unwrap();
    let (c, out, _) = run_in(d, &["check", "handling.rule"]);
    assert_eq!(c, 1, "{out}");
    assert!(out.contains("error[E040]: The shape of `fee.rule` this contribution relies on has changed"), "{out}");
    assert!(out.contains("`input size : number range >=1 <=10` is now `input size : number range >=1 <=20`"), "{out}");
    let (c, _, _) = run_in(d, &["source", "pin", "handling.rule"]);
    assert_eq!(c, 0);
    let (c, out, _) = run_in(d, &["check", "handling.rule"]);
    assert_eq!(c, 0, "{out}");
}

/// What the merged rule is to everything downstream (§15.231): the generated header names each
/// contribution with the shape it pinned and the SHA-256 of its file, `rulec api` lists them with the
/// inputs each adds and every open value's terms, the page has a section for what contributes to each
/// open value and one for each contribution with its tables from its file, and the value a
/// contribution adds to an enum says who added it.
#[test]
fn the_header_the_api_and_the_page_say_what_was_merged() {
    let d = corpus();
    let (c, api, _) = run_in(&d, &["api", "order_discount.rule", "contrib/order_discount/regional_campaign.rule"]);
    assert_eq!(c, 0, "{api}");
    let j = rulec::json::parse(&api).unwrap();
    let cs = j.get("contributions").and_then(|x| x.as_arr()).expect("contributions");
    assert_eq!(cs.len(), 1);
    assert_eq!(cs[0].get("name").and_then(|x| x.as_str()), Some("regional_campaign"));
    assert_eq!(cs[0].get("inputs").and_then(|x| x.as_arr()).map(|a| a.len()), Some(1));
    let opens = j.get("opens").and_then(|x| x.as_arr()).expect("opens");
    assert_eq!(opens.iter().map(|o| o.get("combine").and_then(|x| x.as_str()).unwrap_or("")).collect::<Vec<_>>(), ["sum", "max", "min"]);
    // the rule alone says nothing of contributions, and the same rule merged with none no more
    let (_, alone, _) = run_in(&d, &["api", "order_discount.rule"]);
    assert!(!alone.contains("\"contributions\""), "{alone}");
    let t = TempDir::new("contrib-gen");
    let out = t.path().join("gen");
    let (c, _, e) = run_in(&d, &["gen", "order_discount.rule", "contrib/order_discount/regional_campaign.rule", "--out", out.to_str().unwrap()]);
    assert_eq!(c, 0, "{e}");
    let py = std::fs::read_to_string(out.join("python/order_discount.py")).unwrap();
    assert!(py.lines().any(|l| l.starts_with("# Contributions: regional_campaign = contrib/order_discount/regional_campaign.rule (signature:")), "{}", &py[..400.min(py.len())]);
    assert!(py.contains("def order_discount(subtotal: USD, member: bool, coupon: bool, via: Channel, north: bool) -> USD:"), "the rule's inputs, then the contribution's");
    let (c, page, _) = run_in(&d, &["doc", "order_discount.rule", "contrib/order_discount/regional_campaign.rule"]);
    assert_eq!(c, 0);
    for want in ["## What contributes to stackable", "| regional_campaign:campaign_off | contribution regional_campaign (`contrib/order_discount/regional_campaign.rule`) |", "## Contribution regional_campaign: contrib/order_discount/regional_campaign.rule", "### Table regional_campaign:campaign_parts (policy unique)"] {
        assert!(page.contains(want), "{want}:\n{page}");
    }
    let (_, page, _) = run_in(&d, &["doc", "shared/product_kinds.rule", "contrib/product_line/garden_kind.rule"]);
    assert!(page.contains("garden \"Garden supplies\" (added by contribution garden_kind) (default)"), "{page}");
}

/// The certificate of a merged rule reads the rows a contribution adds back out of the contribution's
/// file, held to its SHA-256: both re-checkers pass it as written, and fail it when the file is
/// another text. The open values are stated in `opens`, the axis of one is of the kind `open`, and both
/// hold the range each declares to what its terms come to together — the Lean one by
/// `openValue_mem_stated` — and every contribution merged into the rule to the digest of its file.
#[test]
fn the_certificate_reads_a_contributed_row_out_of_its_own_file() {
    if !ready(Need::Python, || have("python3"), "no python3, so tools/recheck.py was not run") {
        return;
    }
    let t = TempDir::new("contrib-cert");
    let d = t.path();
    for f in ["order_discount.rule", "contrib/order_discount/regional_campaign.rule"] {
        let to = d.join(f);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(corpus().join(f), &to).unwrap();
    }
    let (c, cert, e) = run_in(d, &["certificate", "order_discount.rule", "contrib/order_discount/regional_campaign.rule"]);
    assert_eq!(c, 0, "{e}");
    let j = rulec::json::parse(&cert).unwrap();
    assert_eq!(j.get("opens").and_then(|x| x.as_arr()).map(|a| a.len()), Some(3));
    assert!(cert.contains("\"file\":{\"path\":\"contrib/order_discount/regional_campaign.rule\""), "{cert}");
    let recheck = |rule: &str| -> (bool, String) {
        let p = d.join("c.json");
        std::fs::write(&p, &cert).unwrap();
        let o = Command::new("python3").current_dir(d).arg(root().join("tools/recheck.py")).arg(&p).args(["--rule", rule]).output().unwrap();
        (o.status.success(), String::from_utf8_lossy(&o.stdout).into_owned())
    };
    let (ok, out) = recheck("order_discount.rule");
    assert!(ok, "{out}");
    assert!(out.contains("open values: 3 hold what their 6 terms come to together inside the range they declare, 3 of the terms from contributions"), "{out}");
    let lean = root().join("../../proofs/.lake/build/bin/rulec-recheck");
    let lean_says = |d: &Path| -> (bool, String) {
        use std::io::Write;
        let mut p = Command::new(&lean)
            .current_dir(d)
            .args(["--rule", "order_discount.rule"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("cannot start the Lean program");
        p.stdin.as_mut().unwrap().write_all(cert.as_bytes()).unwrap();
        let o = p.wait_with_output().unwrap();
        (o.status.success(), String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr))
    };
    let with_lean = ready(Need::Lean, || lean.exists(), "ritsu's proofs/ is not built (lake build there)");
    if with_lean {
        let (ok, out) = lean_says(d);
        assert!(ok, "{out}");
        assert!(out.contains("open values: 3 hold what their 6 terms come to together inside the range they declare, 3 of the terms from contributions"), "{out}");
        assert!(out.contains("OK: every claim this program states was proved"), "{out}");
    }
    // The contribution's file changed under the certificate: both refuse it.
    let p = d.join("contrib/order_discount/regional_campaign.rule");
    let src = std::fs::read_to_string(&p).unwrap();
    std::fs::write(&p, src.replace("| true  | 7USD ", "| true  | 8USD ")).unwrap();
    let (ok, out) = recheck("order_discount.rule");
    assert!(!ok && out.contains("is written in contrib/order_discount/regional_campaign.rule, and that file is another text"), "{out}");
    if with_lean {
        let (ok, out) = lean_says(d);
        assert!(!ok && out.contains("is written in contrib/order_discount/regional_campaign.rule, and that file is another text"), "{out}");
    }
}

/// Every generated language answers the merged rule as the reference evaluator does: the order
/// discount with its campaign, and the price of the product line with its new kind.
#[test]
fn the_merged_rules_run_alike_in_every_language() {
    if !ready(Need::Python, || have("python3"), "no python3") {
        return;
    }
    let d = corpus();
    let t = TempDir::new("contrib-test");
    let out = t.path().join("gen");
    let (c, _, e) = run_in(
        &d,
        &[
            "gen",
            "order_discount.rule",
            "contrib/order_discount/regional_campaign.rule",
            "product_price.rule",
            "shared/product_kinds.rule",
            "contrib/product_line/garden_kind.rule",
            "contrib/product_line/garden_price.rule",
            "--out",
            out.to_str().unwrap(),
        ],
    );
    assert_eq!(c, 0, "{e}");
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").args(["test", out.to_str().unwrap()]).output().expect("cannot start rulec test");
    let said = String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr);
    assert!(o.status.success(), "{said}");
    for rule in ["order_discount", "product_price"] {
        assert!(said.lines().any(|l| l.starts_with(&format!("ok    {rule} (Python)"))), "{rule}:\n{said}");
    }
    for l in said.lines().filter(|l| l.starts_with("warning") || l.starts_with("skip")) {
        skip(l);
    }
}

/// A contribution may open a value of its own, and another contribution add to it, naming the first
/// with `to` (§15.227): the two are merged into the rule, the second after the first, and the second's
/// example runs on the rule merged with both.
#[test]
fn a_contribution_may_open_a_value_another_adds_to() {
    let t = TempDir::new("contrib-chain");
    let d = t.path();
    std::fs::create_dir_all(d.join("contrib")).unwrap();
    std::fs::copy(corpus().join("order_discount.rule"), d.join("order_discount.rule")).unwrap();
    std::fs::write(
        d.join("contrib/campaign.rule"),
        "contribution campaign(campaign) v1\ndescription \"A campaign whose own discount other files may add to\"\nto \"../order_discount.rule\"\n\ninputs\n  north(north) : bool\n\n\
         table base_part\npolicy unique\n| north | -> base_off : money[USD] |\n| true  | 5USD                     |\n| false | 0USD                     |\n\n\
         open campaign_off : money[USD]  combine sum  range >=0USD <=20USD\n  base_off\n\ninto stackable\n  campaign_off\n",
    )
    .unwrap();
    std::fs::write(
        d.join("contrib/weekend.rule"),
        "contribution weekend(weekend) v1\ndescription \"A weekend top-up of the campaign\"\nto \"campaign.rule\"\n\ninputs\n  weekend(weekend) : bool\n\n\
         table weekend_part\npolicy unique\n| weekend | -> weekend_off : money[USD] |\n| true    | 2USD                        |\n| false   | 0USD                        |\n\n\
         into campaign_off\n  weekend_off\n\nexamples\n| subtotal | member | coupon | via | north | weekend | -> discount |\n| 200USD   | true   | false  | web | true  | true    | 15USD       |\n",
    )
    .unwrap();
    for f in ["contrib/campaign.rule", "contrib/weekend.rule"] {
        let (c, _, e) = run_in(d, &["source", "pin", f]);
        assert_eq!(c, 0, "{e}");
    }
    let (c, out, _) = run_in(d, &["check", "order_discount.rule", "contrib"]);
    assert_eq!(c, 0, "{out}");
    assert!(out.contains("ok order_discount.rule (2 contributions: contrib/campaign.rule, contrib/weekend.rule)") && out.contains("ok contrib/weekend.rule (contributes to contrib/campaign.rule)"), "{out}");
    let (_, api, _) = run_in(d, &["api", "order_discount.rule", "contrib"]);
    let j = rulec::json::parse(&api).unwrap();
    let opened = j.get("opens").and_then(|x| x.as_arr()).unwrap().iter().find(|o| o.get("name").and_then(|x| x.as_str()) == Some("campaign:campaign_off")).cloned().expect("the campaign's open value");
    let terms: Vec<&str> = opened.get("terms").and_then(|x| x.as_arr()).unwrap().iter().filter_map(|t| t.get("name").and_then(|x| x.as_str())).collect();
    assert_eq!(terms, ["campaign:base_off", "weekend:weekend_off"]);
}

/// A value added to an ordered enum goes where its `after` puts it, and a comparing cell of a table
/// that is not open and now takes the value is E033, said at the cell (§15.228); `default` on the
/// `extend` line says that answer is meant.
#[test]
fn a_comparing_cell_that_comes_to_take_an_added_value_is_said() {
    let t = TempDir::new("contrib-ordered");
    let d = t.path();
    std::fs::write(
        d.join("fee.rule"),
        "rule member_fee(member_fee) v1\n\nenum tier(tier) = basic(basic) < silver(silver) < gold(gold)  open\n\ninputs\n  level(level) : tier\n\noutputs\n  fee(fee) : number  round down(1)\n\n\
         table fee_of\npolicy unique\n| level    | -> fee : number |\n| basic    | 10              |\n| >=silver | 5               |\n",
    )
    .unwrap();
    std::fs::write(d.join("top.rule"), "contribution top_tier(top_tier) v1\nto \"fee.rule\"\n\nextend tier with platinum after gold\n").unwrap();
    let (c, _, e) = run_in(d, &["source", "pin", "top.rule"]);
    assert_eq!(c, 0, "{e}");
    let (c, out, _) = run_in(d, &["check", "fee.rule", "top.rule"]);
    assert_eq!(c, 1, "{out}");
    assert!(out.contains("error[E033]: The added value platinum has neither a row of table fee_of nor `default`") && out.contains("this comparing cell now takes platinum too"), "{out}");
    let src = std::fs::read_to_string(d.join("top.rule")).unwrap().replace("after gold\n", "after gold default\n");
    std::fs::write(d.join("top.rule"), src).unwrap();
    let (c, out, _) = run_in(d, &["check", "fee.rule", "top.rule"]);
    assert_eq!(c, 0, "{out}");
    // The value is placed after gold: a comparing cell `>=gold` would take it.
    let (f, _) = merged(d, "fee.rule", &["top.rule"]);
    let tier = f.enums.iter().find(|e| e.name.text == "tier").unwrap();
    assert_eq!(tier.values.iter().map(|v| v.text.as_str()).collect::<Vec<_>>(), ["basic", "silver", "gold", "platinum"]);
}

/// Contributions and chances (§15.224, §15.231). A rule's claims of chances hold whatever is added:
/// the merged rule is held to them, an input a contribution adds is a `-` column of every claim,
/// and a contribution that moves the chance of a claimed answer is E133, which names the rows of the
/// contribution that take part — here a holiday that makes more numbers of a lottery win, and an
/// audit that takes a reviewer out of a pick by weight, whose own examples still hold. A
/// contribution draws no input and claims no chance (E067).
#[test]
fn a_rules_claims_of_chances_hold_whatever_is_added() {
    let t = TempDir::new("contrib-chances");
    let d = t.path();
    let open = |file: &str, table: &str| std::fs::read_to_string(corpus().join(file)).unwrap().replace(&format!("\ntable {table}\n"), &format!("\ntable {table} open\n"));
    std::fs::write(d.join("lottery.rule"), open("prize_lottery.rule", "numbers")).unwrap();
    std::fs::write(
        d.join("holiday.rule"),
        "contribution holiday_draw(holiday_draw) v1\ndescription \"On a holiday, the numbers from 60 to 99 win the third prize out of a campaign too\"\nto \"lottery.rule\"\n\n\
         inputs\n  holiday(holiday) : bool\n\nclause holiday_third -> prize\n  when holiday true and campaign false and ticket >=60 <=99\n  then third\n  overrides numbers\n",
    )
    .unwrap();
    std::fs::write(d.join("routing.rule"), open("case_routing.rule", "in_today")).unwrap();
    std::fs::write(
        d.join("audit.rule"),
        "contribution audit_week(audit_week) v1\ndescription \"In an audit week, reviewer 1 audits and takes no case\"\nto \"routing.rule\"\n\n\
         inputs\n  audit(audit) : bool\n\nclause auditing -> presence\n  when audit true and reviewer_id 1\n  then away\n  overrides in_today\n\n\
         sequence team_of_three\n| reviewer_id | present | capacity |\n| 1           | true    | 50       |\n| 2           | true    | 30       |\n| 3           | true    | 20       |\n\n\
         examples\n| roll | reviewers     | audit | -> reviewer |\n| 0    | team_of_three | true  | 2           |\n| 29   | team_of_three | true  | 2           |\n| 30   | team_of_three | true  | 3           |\n| 0    | team_of_three | false | 1           |\n",
    )
    .unwrap();
    for f in ["holiday.rule", "audit.rule"] {
        let (c, _, e) = run_in(d, &["source", "pin", f]);
        assert_eq!(c, 0, "{e}");
    }
    for (rule, contribution, claims, says, by) in [
        ("lottery.rule", "holiday.rule", 2, "With campaign = false, holiday = true, the chance of prize = third is 9/100 (9.0%).", "holiday_draw:holiday_third, a definition contribution holiday_draw adds"),
        ("routing.rule", "audit.rule", 3, "With audit = true, reviewers = team_of_three, the chance of reviewer = 2 is 3/5 (60.0%).", "audit_week:auditing, a definition contribution audit_week adds"),
    ] {
        let (c, out, _) = run_in(d, &["check", rule]);
        assert_eq!(c, 0, "the rule alone holds its claims: {out}");
        let (c, out, _) = run_in(d, &["check", rule, contribution]);
        assert_eq!(c, 1, "{out}");
        assert_eq!(out.matches("error[E133]").count(), claims, "{out}");
        assert!(out.contains(says) && out.contains(by), "{out}");
        assert!(!out.contains("E107") && out.contains(&format!("note {contribution}: its errors are said above")), "the contribution's examples hold, and its claims are the rule's: {out}");
    }
    // The pick by weight, merged: in an audit week, reviewer 1 is never picked.
    let (f, c) = merged(d, "routing.rule", &["audit.rule"]);
    let reviewer = |roll: i64| {
        let mut inputs: std::collections::HashMap<String, rulec::eval::Val> = std::collections::HashMap::new();
        inputs.insert("roll".into(), rulec::eval::Val::Num(rulec::num::Rat::int(roll as i128)));
        inputs.insert("audit".into(), rulec::eval::Val::Bool(true));
        let team: Vec<std::collections::BTreeMap<String, rulec::eval::Val>> = [(1, 50), (2, 30), (3, 20)]
            .into_iter()
            .map(|(id, cap)| {
                let mut m = std::collections::BTreeMap::new();
                m.insert("reviewer_id".to_string(), rulec::eval::Val::Num(rulec::num::Rat::int(id)));
                m.insert("present".to_string(), rulec::eval::Val::Bool(true));
                m.insert("capacity".to_string(), rulec::eval::Val::Num(rulec::num::Rat::int(cap)));
                m
            })
            .collect();
        inputs.insert("reviewers".into(), rulec::eval::Val::Seq(team));
        rulec::eval::run_all(&f, &c, inputs).0[0].1.clone()
    };
    assert_eq!(reviewer(0), Some(rulec::eval::Val::Num(rulec::num::Rat::int(2))));
    assert_eq!(reviewer(30), Some(rulec::eval::Val::Num(rulec::num::Rat::int(3))));
    // A holiday that swaps which numbers win keeps every claim: the merged rule passes, and the
    // function that counts the chances takes the input the holiday adds, in every language.
    std::fs::write(
        d.join("swap.rule"),
        "contribution holiday_swap(holiday_swap) v1\ndescription \"On a holiday out of a campaign, the numbers from 60 to 99 win the third prize in place of those from 10 to 49\"\nto \"lottery.rule\"\n\n\
         inputs\n  holiday(holiday) : bool\n\nclause holiday_third -> prize\n  when holiday true and campaign false and ticket >=60 <=99\n  then third\n  overrides numbers\n\n\
         clause holiday_none -> prize\n  when holiday true and campaign false and ticket >=10 <=49\n  then no_prize\n  overrides numbers\n\n\
         examples\n| ticket | campaign | holiday | -> prize |\n| 70     | false    | true    | third    |\n| 20     | false    | true    | no_prize |\n| 20     | false    | false   | third    |\n",
    )
    .unwrap();
    let (c, _, e) = run_in(d, &["source", "pin", "swap.rule"]);
    assert_eq!(c, 0, "{e}");
    let (c, out, _) = run_in(d, &["check", "lottery.rule", "swap.rule"]);
    assert_eq!(c, 0, "{out}");
    let out_dir = d.join("gen");
    let (c, _, e) = run_in(d, &["gen", "lottery.rule", "swap.rule", "--out", out_dir.to_str().unwrap()]);
    assert_eq!(c, 0, "{e}");
    let py = std::fs::read_to_string(out_dir.join("python/prize_lottery.py")).unwrap();
    assert!(py.contains("def prize_lottery_chances(campaign: bool, holiday: bool) -> Chances:"), "the counts take the added input");
    if ready(Need::Python, || have("python3"), "no python3, so the merged rule's chances were not run") {
        let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").args(["test", out_dir.to_str().unwrap()]).output().unwrap();
        let said = String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr);
        assert!(o.status.success() && said.lines().any(|l| l.starts_with("ok    prize_lottery (Python)")), "{said}");
    }
    // A contribution draws no input, and claims no chance.
    std::fs::write(d.join("drawn.rule"), "contribution drawn(drawn) v1\nto \"lottery.rule\"\n\ninputs\n  roll(roll) : number  range >=0 <=9  random\n").unwrap();
    let (_, out, _) = run_in(d, &["check", "drawn.rule"]);
    assert!(out.contains("error[E067]: The contribution's input roll cannot be drawn at random"), "{out}");
    std::fs::write(d.join("claims.rule"), "contribution claims(claims) v1\nto \"lottery.rule\"\n\nchances\n| campaign | -> prize | chance |\n| -        | first    | 0.1%   |\n").unwrap();
    let (_, out, _) = run_in(d, &["check", "claims.rule"]);
    assert!(out.contains("error[E067]: A contribution file cannot hold `chances`") && out.contains("The claims of chances are written in the rule a contribution adds to"), "{out}");
}

/// Contributions and the caller's object (§15.223, §15.231). A contribution's input may be read out
/// of a contract the rule it adds to declares (`from`): the projection function of the merged rule
/// reads it from the object, and takes an input it adds without `from` as a parameter after the
/// object. A path the contract does not have is said in the contribution's file.
#[test]
fn a_contributions_input_may_be_read_out_of_the_rules_contract() {
    let t = TempDir::new("contrib-from");
    let d = t.path();
    std::fs::create_dir_all(d.join("contracts")).unwrap();
    std::fs::copy(corpus().join("contracts/order_lines.schema.json"), d.join("contracts/order_lines.schema.json")).unwrap();
    std::fs::write(
        d.join("handling.rule"),
        "rule handling_fee(handling_fee) v1\n\nshape order(order) = jsonschema \"contracts/order_lines.schema.json\" \"#/$defs/Order\"\n\n\
         inputs\n  total(total) : money[USD]  range >=0USD <=100000USD  from sum order.lines of amount_usd\n\noutputs\n  fee(fee) : money[USD]  round up(1USD)\n\n\
         table base_of\npolicy unique\n| total      | -> base : money[USD] |\n| >=10000USD | 0USD                 |\n| <10000USD  | 5USD                 |\n\n\
         open extra : money[USD]  combine sum  range >=0USD <=100USD\n\ndefine fee : money[USD] = base + extra\n",
    )
    .unwrap();
    let pieces = "contribution pieces(pieces) v1\nto \"handling.rule\"\n\ninputs\n  pieces(pieces) : number  range >=0 <=1980  from sum order.lines of quantity\n  rush(rush)     : bool\n\n\
                  table pieces_extra\npolicy unique\n| pieces | rush  | -> piece_fee : money[USD] |\n| >=100  | -     | 10USD                     |\n| <100   | true  | 3USD                      |\n| <100   | false | 0USD                      |\n\n\
                  into extra\n  piece_fee\n";
    std::fs::write(d.join("pieces.rule"), pieces).unwrap();
    let (c, _, e) = run_in(d, &["source", "pin", "pieces.rule"]);
    assert_eq!(c, 0, "{e}");
    let (c, out, _) = run_in(d, &["check", "handling.rule", "pieces.rule"]);
    assert_eq!(c, 0, "{out}");
    let (_, api, _) = run_in(d, &["api", "handling.rule", "pieces.rule"]);
    let j = rulec::json::parse(&api).unwrap();
    let p = j.get("projection").expect("projection");
    let froms: Vec<&str> = p.get("inputs").and_then(|x| x.as_arr()).unwrap().iter().filter_map(|i| i.get("from").and_then(|x| x.as_str())).collect();
    assert_eq!(froms, ["sum order.lines of amount_usd", "sum order.lines of quantity"]);
    assert_eq!(p.get("python").and_then(|x| x.get("signature")).and_then(|x| x.as_str()), Some("def handling_fee_from(order: dict[str, Any], rush: bool) -> USD:"));
    std::fs::write(d.join("pieces.rule"), pieces.replace("of quantity", "of quantty")).unwrap();
    let (c, out, _) = run_in(d, &["check", "handling.rule", "pieces.rule"]);
    assert_eq!(c, 1, "{out}");
    assert!(out.contains("error[E121]: An element has no field `quantty`\n  --> pieces.rule:5"), "{out}");
}

/// A contribution's check is about the rule it adds to, and runs under the contribution's path
/// (§15.231); what the rule cites is still followed from the rule's directory (§15.235): its
/// contract (`shape`), the enum it reads from a `.proto` (`import proto`), and the koyomi file of a
/// date it takes its days from. Before, each was looked for from the contribution's directory, and
/// the contribution's check stopped with E013 or E130 while the rule merged with it passed. A date
/// the contribution adds is the other way round: its koyomi file is named from the contribution's
/// directory, whichever file the merged rule is checked as.
#[test]
fn what_a_rule_cites_is_found_from_its_own_directory_in_a_contributions_check() {
    let t = TempDir::new("contrib-cites");
    let d = t.path();
    for sub in ["rules", "mods/m", "views", "api", "cal"] {
        std::fs::create_dir_all(d.join(sub)).unwrap();
    }
    std::fs::write(
        d.join("views/thing.schema.json"),
        r#"{"$defs":{"Thing":{"type":"object","properties":{"size":{"type":"integer","minimum":0,"maximum":9}},"required":["size"]}}}"#,
    )
    .unwrap();
    std::fs::write(d.join("api/kinds.proto"), "syntax = \"proto3\";\npackage shop.v1;\nenum Kind {\n  KIND_UNSPECIFIED = 0;\n  KIND_SMALL = 1;\n  KIND_LARGE = 2;\n}\n").unwrap();
    std::fs::write(
        d.join("rules/fee.rule"),
        "rule fee v1\ndescription \"A fee by the size and the kind of a thing read out of the caller's object\"\n\n\
         enum kind = small | large\nimport proto \"../api/kinds.proto\" Kind -> kind\n\n\
         shape thing = jsonschema \"../views/thing.schema.json\" \"#/$defs/Thing\"\n\n\
         inputs\n  size : number  range >=0 <=9  from thing.size\n  k    : kind\n\n\
         outputs\n  fee : number  round down(1)\n\n\
         table base\npolicy unique\n| size | k     | -> base_fee : number |\n| <=4  | -     | 1                    |\n| >=5  | small | 2                    |\n| >=5  | large | 3                    |\n\n\
         open extra : number  combine sum  range >=0 <=10\n\ndefine fee : number = base_fee + extra\n",
    )
    .unwrap();
    std::fs::write(
        d.join("mods/m/extra.rule"),
        "contribution extra_fee v1\ndescription \"One more for every thing\"\nto \"../../rules/fee.rule\"\n\n\
         table one_more\npolicy unique\n| size | -> more : number |\n| -    | 1                |\n\ninto extra\n  more\n",
    )
    .unwrap();
    let (c, _, e) = run_in(d, &["source", "pin", "mods/m/extra.rule"]);
    assert_eq!(c, 0, "{e}");
    for args in [&["check", "rules/fee.rule", "mods/m"][..], &["check", "mods/m"][..]] {
        let (c, out, err) = run_in(d, args);
        assert_eq!(c, 0, "{args:?}:\n{out}{err}");
        assert!(out.contains("ok mods/m/extra.rule (contributes to rules/fee.rule)\n"), "{args:?}:\n{out}");
    }
    // The koyomi file of the rule's date, and of a date the contribution adds, each from its own
    // file's directory: checked with koyomi joined, as `ritsu rulec` joins it.
    std::fs::copy(root().join("tests/days/payment_terms.cal"), d.join("cal/payment_terms.cal")).unwrap();
    std::fs::write(
        d.join("rules/settlement.rule"),
        "rule settlement v1\ndescription \"A fee by the half of the year a payment day falls in\"\n\n\
         inputs\n  pay_day : date  range from koyomi \"../cal/payment_terms.cal\" date payment\n\n\
         outputs\n  fee : number  round down(1)\n\n\
         table base_of\npolicy unique\n| pay_day      | -> base : number |\n| <=2026-06-30 | 1                |\n| >=2026-07-01 | 2                |\n\n\
         open extra : number  combine sum  range >=0 <=10\n\ndefine fee : number = base + extra\n",
    )
    .unwrap();
    std::fs::write(
        d.join("mods/m/rush.rule"),
        "contribution rush_fee v1\ndescription \"One more for a rush, by the day it is due\"\nto \"../../rules/settlement.rule\"\n\n\
         inputs\n  due : date  range from koyomi \"../../cal/payment_terms.cal\" date payment\n\n\
         table more_of\npolicy unique\n| due          | -> more : number |\n| <=2026-06-30 | 1                |\n| >=2026-07-01 | 2                |\n\ninto extra\n  more\n",
    )
    .unwrap();
    let (c, _, e) = run_in(d, &["source", "pin", "mods/m/rush.rule"]);
    assert_eq!(c, 0, "{e}");
    let rule = d.join("rules/settlement.rule").to_string_lossy().into_owned();
    let rush = d.join("mods/m/rush.rule").to_string_lossy().into_owned();
    let port: rulec::days::Port = std::sync::Arc::new(koyomi::ports::Engine);
    for (what, path) in [("the rule merged with the contribution", &rule), ("the contribution", &rush)] {
        let src = std::fs::read_to_string(path).unwrap();
        let ds = rulec::days::with(Some(port.clone()), || rulec::link::with_set(&[rule.clone(), rush.clone()], || rulec::report(&src, path).diags));
        let errors: Vec<(&str, &str)> = ds.iter().filter(|x| x.severity == rulec::diag::Severity::Error).map(|x| (x.code, x.title.as_str())).collect();
        assert!(errors.is_empty(), "{what}: {errors:?}");
    }
}

/// A contribution merged into a rule another rule applies is part of the function the applying rule
/// is generated as (§15.237): the applied rule's rows come in with the contribution's among them.
/// The applying rule's `ok` line names its file, the head of every file generated from it names it
/// under the apply's name (`elig:m_elig`, as the names the apply brings in are written), and `rulec
/// api` lists it with no input of its own, since the apply binds what the applied rule takes. Before,
/// none of the three said it, while the generated code took the contribution's clause.
#[test]
fn a_contribution_to_an_applied_rule_is_named_by_the_rule_that_applies_it() {
    let t = TempDir::new("contrib-applied");
    let d = t.path();
    for sub in ["rules", "mods/m"] {
        std::fs::create_dir_all(d.join(sub)).unwrap();
    }
    std::fs::write(d.join("rules/content.rule"), "enums content v1\ndescription \"Events\"\n\nenum event = harvest \"Harvest\" | quiet \"Quiet\"  open\n").unwrap();
    std::fs::write(
        d.join("rules/elig.rule"),
        "rule elig v1\ndescription \"Whether an event can happen, and its weight\"\n\nimport enum event from \"content.rule\"\n\n\
         inputs\n  event  : event\n  autumn : bool\n\noutputs\n  weight : number  round down(1)\n\n\
         table weight_of open\npolicy unique\n| event   | autumn | -> weight : number |\n| harvest | true   | 30                 |\n| harvest | false  | 0                  |\n| quiet   | -      | 40                 |\n",
    )
    .unwrap();
    std::fs::write(
        d.join("rules/pick.rule"),
        "rule pick v1\ndescription \"Pick one event by weight\"\n\nimport enum event from \"content.rule\"\n\nenum pick_verdict = weighed | skipped\n\n\
         elements candidates\n  candidate : event\n\ninputs\n  roll   : number  range >=0 <=999  random\n  autumn : bool\n\noutputs\n  picked : event\n\n\
         apply elig = \"elig.rule\"\n  event = candidate\n  autumn = autumn\n\n\
         table verdict_of\npolicy unique\n| elig:weight | -> verdict : pick_verdict |\n| >=1         | weighed                   |\n| <=0         | skipped                   |\n\n\
         fold verdict over candidates -> picked\n  weighed   -> take_weighted candidate by elig:weight\n  skipped   -> next\n  draw      -> roll\n  empty     -> quiet\n  exhausted -> held\n",
    )
    .unwrap();
    std::fs::write(d.join("mods/m/content.rule"), "contribution m_content v1\ndescription \"A storm\"\nto \"../../rules/content.rule\"\n\nextend event with storm \"Storm\"\n").unwrap();
    std::fs::write(
        d.join("mods/m/elig.rule"),
        "contribution m_elig v1\ndescription \"The storm's weight\"\nto \"../../rules/elig.rule\"\nafter \"content.rule\"\n\nclause storm_weight -> weight\n  when event storm\n  then 12\n",
    )
    .unwrap();
    for f in ["mods/m/content.rule", "mods/m/elig.rule", "rules/pick.rule"] {
        let (c, out, e) = run_in(d, &["source", "pin", f]);
        assert_eq!(c, 0, "{f}: {out}{e}");
    }
    let (c, out, e) = run_in(d, &["check", "rules", "mods"]);
    assert_eq!(c, 0, "{out}{e}");
    assert!(out.contains("ok rules/pick.rule (2 contributions: mods/m/elig.rule, mods/m/content.rule)\n"), "{out}");
    let (c, out, e) = run_in(d, &["gen", "rules", "mods", "--out", "gen"]);
    assert_eq!(c, 0, "{out}{e}");
    let line = "Contributions: elig:m_elig = ../mods/m/elig.rule (signature:";
    for (file, comment) in [("gen/python/pick.py", "#"), ("gen/typescript/pick.ts", "//"), ("gen/go/pick/pick.go", "//"), ("gen/java/Pick.java", "//")] {
        let text = std::fs::read_to_string(d.join(file)).unwrap();
        let head: Vec<&str> = text.lines().take(8).collect();
        assert!(head.iter().any(|l| l.starts_with(&format!("{comment} {line}"))), "{file}:\n{}", head.join("\n"));
    }
    let (_, api, _) = run_in(d, &["api", "rules/pick.rule", "mods"]);
    let j = rulec::json::parse(api.lines().next().unwrap()).unwrap();
    let cs = j.get("contributions").and_then(|x| x.as_arr()).expect("no `contributions`");
    assert_eq!(cs.len(), 1, "{api}");
    assert_eq!(cs[0].get("name").and_then(|x| x.as_str()), Some("elig:m_elig"));
    assert_eq!(cs[0].get("file").and_then(|x| x.as_str()), Some("../mods/m/elig.rule"));
    assert_eq!(cs[0].get("inputs").and_then(|x| x.as_arr()).map(|x| x.len()), Some(0));
    // The rule alone, with nothing merged into what it applies, names none.
    let (c, out, _) = run_in(d, &["check", "rules/pick.rule"]);
    assert_eq!(c, 0, "{out}");
    assert!(out.contains("ok rules/pick.rule\n"), "{out}");
}

//! `ritsu yuen affected` follows what a request reads (yuen's DESIGN 8), through ringi's port of
//! references and the ports of the languages below it, every language joined as `ritsu yuen`
//! joins them: the organization, the masters, the shared route, the calendar and the rule a
//! request uses, and the rule that rule applies; and a contribution, read by the rule it is merged
//! into. The project is a copy of ringi's examples with requirements of the test beside them
//! (`reads.req`, its Japanese version `reads.ja.req`), and a rule the table of authority applies.
//! A file only a request no requirement names reads reaches nothing. The data an organization and
//! its masters are made of reaches the requests too: ringi's port answers the `data` lines among
//! what a file reads, a folder for every file under it.

use ritsu_project::Joined;
use ritsu_testkit::TempDir;
use std::path::{Path, PathBuf};

const REQ: &str = "requirements ringi_reads v1
description \"Two requests of the examples, for the tests of what affected follows: each request reads the organization and its calendar, and the masters, the routes and the rules it uses\"

role general_affairs \"Owns the requests\"

requirement leave_request
  text \"Leave is approved along the route the requests share\"
  owner general_affairs
  decided 2026-11-02 by general_affairs \"Written for this test\"
  satisfied by ringi \"leave.ringi\"
  not verified \"The tests of affected read no claim\"

requirement purchase_request
  text \"A purchase is decided by the table of authority, from a vendor of the masters\"
  owner general_affairs
  decided 2026-11-02 by general_affairs \"Written for this test\"
  satisfied by ringi \"purchase.ringi\"
  not verified \"The tests of affected read no claim\"
";

const REQ_JA: &str = "requirements 稟議の読むもの v1
description \"例の申請のうち二つ。affected が辿るものの試験のために書いた。どの申請も組織とカレンダーを読み、使うマスタ・経路・規則を読む\"

role 総務 \"申請を持つ\"

requirement 休暇の申請(leave_request)
  text \"休暇は、申請が共有する経路で承認する\"
  owner 総務
  decided 2026-11-02 by 総務 \"この試験のために書いた\"
  satisfied by ringi \"leave.ja.ringi\"
  not verified \"affected の試験は主張を読まない\"

requirement 購買の稟議(purchase_request)
  text \"購買は、権限表が決める人が、マスタの取引先から決める\"
  owner 総務
  decided 2026-11-02 by 総務 \"この試験のために書いた\"
  satisfied by ringi \"purchase.ja.ringi\"
  not verified \"affected の試験は主張を読まない\"
";

/// The rule the table of authority applies, in the copy: whether a purchase is above what a head
/// of department decides alone.
const LIMITS: &str = "rule purchase_limits v1
description \"Whether a purchase is above what a head of department decides alone: written for the tests of yuen affected\"

inputs
  amount : money[JPY]  range >=1JPY <=100_000_000JPY

outputs
  above : bool

table above
policy unique
| amount         | -> above : bool |
| <=1_000_000JPY | false           |
| >1_000_000JPY  | true            |
";

/// The `apply` the copy of the table of authority gets, before its table, pinned at the meaning
/// of [`LIMITS`].
const APPLY: &str = "apply limit_of = \"limits.rule\" meaning:a54e9973a8e72adb
  amount = amount
  above -> over_limit

";

/// ringi's examples copied, as `<tmp>/ringi`, with the requirements of the test and the applied
/// rule.
fn project() -> (TempDir, PathBuf) {
    let t = TempDir::new("reads-ringi");
    let d = t.path().join("ringi");
    ritsu_testkit::tmp::copy_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../ringi/examples"), &d);
    std::fs::write(d.join("reads.req"), REQ).unwrap();
    std::fs::write(d.join("reads.ja.req"), REQ_JA).unwrap();
    std::fs::write(d.join("shared/limits.rule"), LIMITS).unwrap();
    let rule = std::fs::read_to_string(d.join("shared/authority.rule")).unwrap();
    assert!(rule.contains("\ntable authority"), "{rule}");
    std::fs::write(d.join("shared/authority.rule"), rule.replacen("\ntable authority", &format!("\n{APPLY}table authority"), 1)).unwrap();
    (t, d)
}

/// The line of `file` (from `d`) that holds `text`, from 1.
fn line_of(d: &Path, file: &str, text: &str) -> usize {
    let s = std::fs::read_to_string(d.join(file)).unwrap();
    s.lines().position(|l| l.contains(text)).unwrap_or_else(|| panic!("{file} has no {text:?}")) + 1
}

/// `affected` of `req` over a diff that adds a line after the last line of `file`, as `ritsu yuen`
/// runs it with `suite`: the exit and what it prints.
fn affected(d: &Path, req: &str, file: &str, lang: &str, suite: yuen::suite::Suite) -> (i32, String) {
    let text = std::fs::read_to_string(d.join(file)).unwrap();
    let n = text.lines().count();
    let last = text.lines().last().unwrap_or("");
    let diff = d.parent().unwrap().join("change.diff");
    std::fs::write(&diff, format!("--- a/{file}\n+++ b/{file}\n@@ -{n} +{n},2 @@\n {last}\n+# one more line\n")).unwrap();
    let dir = d.to_string_lossy().to_string();
    let args: Vec<String> = ["affected", &format!("{dir}/{req}"), "--root", &dir, "--diff", &diff.to_string_lossy(), "--lang", lang].iter().map(|a| a.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = yuen::run::run(&args, suite, &mut out, &mut err);
    (code as i32, format!("{}{}", String::from_utf8_lossy(&out), String::from_utf8_lossy(&err)))
}

fn joined() -> yuen::suite::Suite {
    Joined::new().yuen()
}

/// The organization, the masters, the shared route, the calendar and the rule each reach the
/// requests that use them, by the line that uses them; the rule the table of authority applies
/// reaches the purchase a level further down.
#[test]
fn what_a_request_uses_reaches_the_request() {
    let (_t, d) = project();
    let dir = d.to_string_lossy().to_string();
    let (code, out) = affected(&d, "reads.req", "shared/company.ringi", "en", joined());
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains(&format!("  {dir}/shared/company.ringi\n    ringi \"leave.ringi\" line 5 (use organization): met by leave_request\n    ringi \"purchase.ringi\" line 5 (use organization): met by purchase_request\n")),
        "{out}"
    );
    assert!(out.ends_with("2 requirements touched; ask general_affairs\n"), "{out}");
    let (_, out) = affected(&d, "reads.req", "shared/masters.ringi", "en", joined());
    assert!(out.contains(&format!("  {dir}/shared/masters.ringi\n    ringi \"purchase.ringi\" line 6 (use masters): met by purchase_request\n")), "{out}");
    assert!(out.ends_with("1 requirement touched; ask general_affairs\n"), "{out}");
    let (_, out) = affected(&d, "reads.req", "shared/manager_approval.ringi", "en", joined());
    assert!(out.contains(&format!("  {dir}/shared/manager_approval.ringi\n    ringi \"leave.ringi\" line 9 (use route): met by leave_request\n")), "{out}");
    assert!(out.ends_with("1 requirement touched; ask general_affairs\n"), "{out}");
    let (_, out) = affected(&d, "reads.req", "shared/office.cal", "en", joined());
    assert!(
        out.contains(&format!("  {dir}/shared/office.cal\n    ringi \"leave.ringi\" line 6 (use calendar): met by leave_request\n    ringi \"purchase.ringi\" line 7 (use calendar): met by purchase_request\n")),
        "{out}"
    );
    let (_, out) = affected(&d, "reads.req", "shared/authority.rule", "en", joined());
    assert!(out.contains(&format!("  {dir}/shared/authority.rule\n    ringi \"purchase.ringi\" line 8 (use rule): met by purchase_request\n")), "{out}");
    let apply = line_of(&d, "shared/authority.rule", "apply limit_of");
    let (code, out) = affected(&d, "reads.req", "shared/limits.rule", "en", joined());
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains(&format!("  {dir}/shared/limits.rule\n    ringi \"purchase.ringi\" line 8 (use rule) -> rulec \"shared/authority.rule\" line {apply} (apply): met by purchase_request\n")),
        "{out}"
    );
    assert!(out.ends_with("1 requirement touched; ask general_affairs\n"), "{out}");
}

/// The same in Japanese, from the Japanese versions of the requests.
#[test]
fn what_a_request_uses_reaches_the_request_in_japanese() {
    let (_t, d) = project();
    let dir = d.to_string_lossy().to_string();
    let (code, out) = affected(&d, "reads.ja.req", "shared/company.ringi", "ja", joined());
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains(&format!("  {dir}/shared/company.ringi\n    ringi \"leave.ja.ringi\" の 5 行目（use organization）: 満たす要件 休暇の申請\n    ringi \"purchase.ja.ringi\" の 5 行目（use organization）: 満たす要件 購買の稟議\n")),
        "{out}"
    );
    assert!(out.ends_with("触る要件は 2 件です。総務 に見てもらってください\n"), "{out}");
    let apply = line_of(&d, "shared/authority.rule", "apply limit_of");
    let (_, out) = affected(&d, "reads.ja.req", "shared/limits.rule", "ja", joined());
    assert!(out.contains(&format!("    ringi \"purchase.ja.ringi\" の 8 行目（use rule） -> rulec \"shared/authority.rule\" の {apply} 行目（apply）: 満たす要件 購買の稟議\n")), "{out}");
    let (_, out) = affected(&d, "reads.ja.req", "shared/manager_approval.ringi", "ja", joined());
    assert!(out.contains("    ringi \"leave.ja.ringi\" の 9 行目（use route）: 満たす要件 休暇の申請\n"), "{out}");
}

/// A flow only a request no requirement names uses (the account request's), and a file no
/// request reads, reach no requirement.
#[test]
fn what_only_another_request_uses_reaches_nothing() {
    let (_t, d) = project();
    let dir = d.to_string_lossy().to_string();
    for file in ["shared/provision.flow", "shared/regulations/article-05.md"] {
        let (code, out) = affected(&d, "reads.req", file, "en", joined());
        assert_eq!(code, 0, "{out}");
        assert!(out.contains(&format!("other files the diff touches: {dir}/{file}\nno requirement is touched\n")), "{file}: {out}");
    }
}

/// A contribution merged into a rule is read by the rule (rulec's §15.227): `ritsu yuen` gives
/// rulec the rule files of the root's project, as `ritsu check` does, so the change to the
/// contribution reaches the requirement of the rule it adds to. The project is a copy of yuen's
/// fixture `contributions`.
#[test]
fn a_change_to_a_contribution_reaches_the_rule_it_adds_to() {
    let t = TempDir::new("reads-contributions");
    let d = t.path().join("contributions");
    ritsu_testkit::tmp::copy_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../yuen/tests/fixtures/contributions"), &d);
    let dir = d.to_string_lossy().to_string();
    for (req, file, lang, want) in [
        ("fees.req", "rules/extra/courier.rule", "en", "    rulec \"rules/fee.rule\" (contribution): met by fee_by_way\n"),
        ("fees.ja.req", "rules/extra/courier.ja.rule", "ja", "    rulec \"rules/fee.ja.rule\"（contribution）: 満たす要件 戻し方ごとの手数料\n"),
    ] {
        let (code, out) = affected(&d, req, file, lang, joined());
        assert_eq!(code, 0, "{out}");
        assert!(out.contains(&format!("  {dir}/{file}\n{want}")), "{out}");
    }
}

/// A version of the organization's data and a master's table: ringi's port answers the `data`
/// lines among what a file reads, a folder standing for every file in it, and the change reaches
/// each request through the file that names the data.
#[test]
fn the_data_of_the_organization_and_the_masters_reaches_the_requests() {
    let (_t, d) = project();
    let dir = d.to_string_lossy().to_string();
    let org = line_of(&d, "shared/company.ringi", "data \"../org\"");
    let (code, out) = affected(&d, "reads.req", "org/2026-04-01/members.csv", "en", joined());
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains(&format!(
            "  {dir}/org/2026-04-01/members.csv\n    ringi \"leave.ringi\" line 5 (use organization) -> ringi \"shared/company.ringi\" line {org} (data): met by leave_request\n    ringi \"purchase.ringi\" line 5 (use organization) -> ringi \"shared/company.ringi\" line {org} (data): met by purchase_request\n"
        )),
        "{out}"
    );
    let vendors = line_of(&d, "shared/masters.ringi", "data \"../masters/vendors.csv\"");
    let (_, out) = affected(&d, "reads.req", "masters/vendors.csv", "en", joined());
    assert!(out.contains(&format!("  {dir}/masters/vendors.csv\n    ringi \"purchase.ringi\" line 6 (use masters) -> ringi \"shared/masters.ringi\" line {vendors} (data): met by purchase_request\n")), "{out}");
    assert!(out.ends_with("1 requirement touched; ask general_affairs\n"), "{out}");
}

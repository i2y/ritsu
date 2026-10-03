//! The ends of a link (DESIGN 3.2, 4.1, PLAN B.5): the requirements of the `period` fixture
//! have the hashes the throwaway prototype of stage A computed, and the end of `起算日` is
//! the three lines DESIGN 4.1 shows, to the letter.

use yuen::check::check;

fn period() -> yuen::check::Checked {
    let p = "tests/fixtures/period".to_string();
    check(std::slice::from_ref(&p), Some(&p)).unwrap()
}

#[test]
fn the_ends_of_the_period_fixture() {
    let c = period();
    let p = c.project.as_ref().unwrap();
    let m = c.model.as_ref().unwrap();
    let hash = |name: &str| {
        let r = p.find_req(name).unwrap()[0];
        m.req_ends[r].as_ref().unwrap().hash.clone()
    };
    assert_eq!(hash("起算日"), "a9ebc73907faddc8");
    assert_eq!(hash("満了日"), "465b83ed8c251406");
    assert_eq!(hash("満了日_142条"), "d4f2d2a67322df17");
    let cal = m.artifacts.iter().find(|(n, _)| n.path == "民法の期間.cal").unwrap().1.as_ref().unwrap();
    assert_eq!(cal.hash, "c9b94eecde23e6b5");
}

#[test]
fn the_end_of_a_requirement_is_what_design_4_1_shows() {
    let c = period();
    let p = c.project.as_ref().unwrap();
    let m = c.model.as_ref().unwrap();
    let r = p.find_req("first_day").unwrap()[0];
    let end = String::from_utf8(m.req_ends[r].as_ref().unwrap().bytes.clone()).unwrap();
    let design = std::fs::read_to_string("DESIGN.md").unwrap();
    let a = design.find("1.1 の `起算日` の端の中身は次の 3 行").unwrap();
    let block = &design[a..];
    let start = block.find("```\n").unwrap() + 4;
    let stop = start + block[start..].find("```").unwrap();
    assert_eq!(end, &block[start..stop]);
    assert_eq!(end.lines().count(), 3);
}

#[test]
fn the_end_carries_a_requirement_it_is_read_from() {
    // `from <requirement>` writes the end of that requirement, with its version.
    let t = std::env::temp_dir().join(format!("yuen-ends-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&t);
    std::fs::create_dir_all(&t).unwrap();
    std::fs::write(
        t.join("a.req"),
        "requirements a v1\nrole 経理\n\nrequirement 方針(policy)\n  text \"x\"\n  owner 経理\n  decided 2026-10-03 by 経理 \"例\"\n\nrequirement 支払日(pay)\n  text \"y\"\n  in force 2026-10-01..\n  owner 経理\n  from 方針\n",
    )
    .unwrap();
    let path = t.to_string_lossy().to_string();
    let c = check(std::slice::from_ref(&path), Some(&path)).unwrap();
    let p = c.project.as_ref().unwrap();
    let m = c.model.as_ref().unwrap();
    let policy = m.req_ends[p.find_req("方針").unwrap()[0]].as_ref().unwrap();
    assert_eq!(String::from_utf8(policy.bytes.clone()).unwrap(), "text x\n");
    let pay = m.req_ends[p.find_req("pay").unwrap()[0]].as_ref().unwrap();
    assert_eq!(String::from_utf8(pay.bytes.clone()).unwrap(), format!("text y\nfrom requirement 方針 v1 sha256:{}\nin force 2026-10-01..\n", policy.hash));
    let _ = std::fs::remove_dir_all(&t);
}

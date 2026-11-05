//! A module of one rule and its requirements, in two versions that follow an amendment of the section
//! they read, for what `ritsu mod diff` and `ritsu mod site` say of a `.req`. The records are the
//! test's own, in the test's directory: a person's records are made by a person.

use ritsu_testkit::TempDir;
use std::path::Path;
use std::process::Command;

/// What a version follows.
pub struct Version {
    pub version: &'static str,
    pub asof: &'static str,
    pub after: &'static str,
    pub from: &'static str,
    pub amount: &'static str,
    pub cites: &'static str,
    pub because: &'static str,
}

pub const OLD: Version = Version {
    version: "1.0.0",
    asof: "2024-02-01",
    after: "January 15, 2024",
    from: "2024-01-16",
    amount: "100",
    cites: "89 FR 1817, Jan. 11, 2024",
    because: "Follows the amendment at 89 FR 1817 (January 11, 2024).",
};

pub const NEW: Version = Version {
    version: "1.1.0",
    asof: "2025-02-01",
    after: "January 15, 2025",
    from: "2025-01-16",
    amount: "200",
    cites: "89 FR 1817, Jan. 11, 2024; 90 FR 1861, Jan. 10, 2025",
    because: "Follows the amendment at 90 FR 1861 (January 10, 2025).",
};

/// The module `name` at `dir` of `t`, at the version `v`, its links not looked at.
pub fn write(t: &TempDir, dir: &str, name: &str, v: &Version) {
    let (version, asof, after, from, amount, cites, because) = (v.version, v.asof, v.after, v.from, v.amount, v.cites, v.because);
    t.write(&format!("{dir}/ritsu.mod"), format!("module {name}\nversion {version}\nritsu >=0.36.0\n"));
    let copy = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<DIV8 N=\"1903.15\" TYPE=\"SECTION\">\n<HEAD>§ 1903.15 Proposed penalties.</HEAD>\n<P>(c) Kept from one version to the next.</P>\n<P>(d) For penalties proposed after {after}, a penalty shall not exceed ${amount}.</P>\n<CITA>[36 FR 17850, as amended at {cites}]</CITA>\n</DIV8>\n"
    );
    let pin = ritsu_base::sha256::short(copy.as_bytes());
    t.write(&format!("{dir}/sources/law/29-CFR-1903@{asof}/1903.15.xml"), copy);
    t.write(
        &format!("{dir}/fine.rule"),
        format!(
            "rule fine v1\ndescription \"The most a penalty may be\"\n\nsource osha = law ecfr \"29 CFR 1903\" asof {asof}\n  \"§1903.15\" sha256:{pin}\n\nenum kind = serious | other\n\ninputs\n  violation : kind\n\noutputs\n  most : money[USD]  round down(1USD)\n\ntable amounts  @osha \"§1903.15\"\npolicy unique\n| violation | -> most : money[USD] |\n| serious   | {amount}USD |\n| other     | {amount}USD |\n\nexamples\n| violation | -> most |\n| serious   | {amount}USD |\n| other     | {amount}USD |\n"
        ),
    );
    t.write(
        &format!("{dir}/fines.req"),
        format!(
            "requirements fines v1\ndescription \"Where the amounts of fine.rule come from\"\n\nrole maintainer \"Reads the section against the rule\"\n\nsource osha = rulec \"fine.rule\" source osha\n\nscope rulec \"fine.rule\" table\n\nrequirement amounts\n  text \"For penalties proposed after {after}, a penalty is at most ${amount}.\"\n  in force {from}..\n  owner maintainer\n  from @osha \"§1903.15\"\n  decided 2026-01-01 by maintainer \"{because}\"\n  satisfied by rulec \"fine.rule\" table amounts\n  verified by rulec \"fine.rule\"\n"
        ),
    );
}

/// The records of the test, on every link of the `.req` at `dir` of `root`.
pub fn review(root: &Path, dir: &str) {
    let out = Command::new(env!("CARGO_BIN_EXE_ritsu"))
        .current_dir(root.join(dir))
        .args(["yuen", "review", "fines.req", "--root", ".", "--all", "--by", "maintainer", "--date", "2026-01-02"])
        .env_remove("RITSU_LANG")
        .env_remove("YUEN_LANG")
        .output()
        .expect("could not run ritsu");
    assert!(out.status.success(), "{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
}

/// A copy of the version at `from`, at `to`.
#[allow(dead_code)]
pub fn copy(t: &TempDir, from: &str, to: &str) {
    ritsu_testkit::tmp::copy_dir(&t.path().join(from), &t.path().join(to));
}

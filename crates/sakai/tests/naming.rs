//! The table both yuen and sakai hold (DESIGN 2.8): every line of `tests/fixtures/naming.tsv`
//! is a name written in a file at the root, a tab, and the JSON it gives or `ERROR: <why>`.
//! sakai gives that JSON, byte for byte, and refuses every line the table refuses, for the
//! reason the table gives.

/// What sakai's English message says for each reason of the table: a line refused for another
/// reason than the table's is refused by accident (a full-width space outside a string was once
/// refused only because the word after it had no name).
const REASONS: &[(&str, &str)] = &[
    ("file has no kinds", "`file` has no kinds"),
    ("value only right after enum", "`value` comes only right after `enum`"),
    ("nothing under task", "`case` cannot come under `task`"),
    ("only field under record", "`task` cannot come under `record`"),
    ("method only right after service", "`method` comes only right after `service`"),
    ("one child at most", "one child at most"),
    ("chobo has no nested kinds", "the tool chobo has no nested kinds"),
    ("unknown kind for koyomi", "the tool koyomi has no kind `alias`"),
    ("unknown tool", "is not a tool"),
    ("absolute path", "the absolute path"),
    ("outside the root", "goes outside the root"),
    ("a full-width space outside a string", "full-width space outside a string"),
    ("only \\\" and \\\\ are escapes", "only `\\\"` and `\\\\` are"),
    ("a kind written as a string", "a kind goes here"),
    ("a tool written as a string", "a name starts with its tool"),
    ("a kind without a name", "has no name after it"),
    ("an empty path", "the path is empty"),
];

#[test]
fn every_line_of_the_table_gives_its_json_or_is_refused() {
    let table = std::fs::read_to_string("../ritsu-base/tests/fixtures/naming.tsv").unwrap();
    let mut failures = Vec::new();
    let (mut ok, mut refused) = (0, 0);
    for (i, line) in table.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let (name, want) = line.split_once('\t').unwrap_or_else(|| panic!("line {} has no tab", i + 1));
        let got = sakai::naming::parse(name, ".");
        if let Some(why) = want.strip_prefix("ERROR:") {
            let why = why.trim();
            let Some((_, says)) = REASONS.iter().find(|(r, _)| *r == why) else {
                failures.push(format!("line {}: the reason `{why}` is new to this test; add what sakai says for it to REASONS", i + 1));
                continue;
            };
            match got {
                Ok(n) => failures.push(format!("line {}: `{name}` should be refused ({why}), and gave {}", i + 1, n.to_json())),
                Err(e) if e.en.contains(says) => refused += 1,
                Err(e) => failures.push(format!("line {}: `{name}` should be refused because {why}, and was refused because {}", i + 1, e.en)),
            }
            continue;
        }
        match got {
            Ok(n) => {
                let json = n.to_json().compact();
                if json != want {
                    failures.push(format!("line {}: `{name}`\n  want {want}\n  got  {json}", i + 1));
                } else {
                    ok += 1;
                }
                // The text reads back as the same name.
                let again = sakai::naming::parse(&n.text(), ".").unwrap();
                assert_eq!(again, n, "line {}: the text does not read back", i + 1);
            }
            Err(e) => failures.push(format!("line {}: `{name}` was refused: {}", i + 1, e.en)),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert_eq!((ok, refused), (24, 18), "the table has 24 names and 18 refusals");
}

/// The table is ritsu-base's, and yuen is held to it too; this test only says what it holds, so
/// that a line dropped by accident shows.
#[test]
fn the_table_names_every_tool_but_dir() {
    let table = std::fs::read_to_string("../ritsu-base/tests/fixtures/naming.tsv").unwrap();
    for t in sakai::naming::Tool::ALL {
        assert!(table.lines().any(|l| l.starts_with(&format!("{} ", t.word()))), "{} is in the table", t.word());
    }
    assert!(!table.lines().any(|l| l.starts_with("dir ")), "dir is not a tool");
}

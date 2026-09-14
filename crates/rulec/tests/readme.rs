//! The README must not rot: its rule example has to pass check, and the excerpts of
//! generated code pasted under it have to be lines the generator really writes.
//!
//! Its own binary because the excerpts are of the **default** output, which is English
//! (§11 principle 7), while `.cargo/config.toml` pins `RULEC_LANG=ja` for the suite. The
//! output language is a process-wide setting, so a binary must not mix languages — the
//! same reason `golden_en.rs` is separate from `golden.rs`.

fn readme() -> String {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md");
    std::fs::read_to_string(&p).expect("README.md が読めない")
}

/// The `.rule` block under "## Write a table (.rule)".
fn example(md: &str) -> &str {
    let head = md.find("## Write a table (.rule)").expect("## Write a table (.rule) の節が無い");
    let fence = md[head..].find("```").expect("コードブロックが無い") + head;
    // Step over the fence and the tag on it (```rule), to the first line of the source.
    let open = fence + md[fence..].find('\n').expect("コードブロックが閉じていない") + 1;
    let close = md[open..].find("```").expect("コードブロックが閉じていない") + open;
    &md[open..close]
}

#[test]
fn readmeの例は通る() {
    // Check the example in the README every time so it does not rot.
    // Putting an example that does not pass into the README goes against the point of this tool.
    let md = readme();
    let src = example(&md);
    let ds = rulec::check_source(src, "README.md");
    assert!(
        !rulec::has_error(&ds),
        "README の例が通らない: {:?}",
        ds.iter().map(|d| format!("{}: {}", d.code, d.title)).collect::<Vec<_>>()
    );
}

#[test]
fn readmeの生成コード抜粋は実物と一致する() {
    // The README shows what a reader gets without passing `--lang`, so render in the
    // default language.
    rulec::i18n::set(rulec::i18n::Lang::En);
    let md = readme();
    let src = example(&md);
    // The excerpts pasted under "generated code" must not diverge from the actual output.
    // The excerpts are abridged with `...`, so the whole cannot be compared, but it can be verified
    // that every line shown is in the output. The generated code in the README was copied by hand,
    // and hand-copied text rots.
    let (f, c) = rulec::prepare(src, "README.md").expect("README の例は検査を通る");
    let g = rulec::codegen::Gen::new(&f, &c, src);
    let py = g.python();
    let go = g.go();
    let sec = md.find("## The generated code").expect("## The generated code の節が無い");
    let end = md[sec..].find("## Using it").expect("節の終わりが無い") + sec;
    for (lang, body) in [("python", &py), ("go", &go)] {
        let fence = format!("```{lang}\n");
        let open = md[sec..end].find(&fence).unwrap_or_else(|| panic!("{lang} の抜粋が無い")) + sec + fence.len();
        let close = md[open..end].find("```").expect("抜粋が閉じていない") + open;
        for line in md[open..close].lines() {
            if line.trim() == "..." || line.trim().is_empty() {
                continue;
            }
            assert!(body.contains(line), "README の {lang} 抜粋が生成物に無い:\n{line}");
        }
    }
}

/// The first diagnostic the README shows is the one a reader gets with no flags at all.
#[test]
fn readmeの診断抜粋は既定の言語で出る() {
    rulec::i18n::set(rulec::i18n::Lang::En);
    let md = readme();
    let src = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/mutants/m_e101.rule"),
    )
    .unwrap();
    let lines: Vec<String> = src.lines().map(|s| s.to_string()).collect();
    let ds = rulec::check_source(&src, "rules/ゆうパック運賃.rule");
    let d = ds.iter().find(|d| d.code == "E101").expect("E101 が出ない");
    let got = rulec::diag::render(d, &lines);
    // The README quotes it with a `rules/…` path, so compare line by line and skip the
    // `-->` line, which names the file.
    for line in got.lines().filter(|l| !l.trim_start().starts_with("-->")) {
        assert!(md.contains(line), "README の診断抜粋が実物と食い違う:\n{line}");
    }
}

/// The README states two counts about the tool. Both rot silently: a corpus rule or a
/// diagnostic gets added and the sentence keeps claiming the old number, which is exactly
/// the kind of quietly-false documentation the rest of this suite exists to prevent.
#[test]
fn readmeが言う件数は実物と合っている() {
    let md = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md"),
    )
    .unwrap();
    let corpus = std::fs::read_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "rule"))
        .count();
    let codes = rulec::codes::ledger().len();
    let want = format!(
        "{} rules taken from real published terms are checked, generated and run on every commit, and all {codes} diagnostics are implemented.",
        match corpus {
            12 => "Twelve".to_string(),
            n => n.to_string(),
        }
    );
    assert!(md.contains(&want), "README の件数が実物と違う。正しくは: {want}");
}

//! The README must not rot: its rule example has to pass check, the excerpts of generated
//! code pasted under it have to be lines the generator really writes, and the counts it
//! states have to be what is on disk.
//!
//! Its own binary because the excerpts are of the **default** output, which is English
//! (§11 principle 7), while `.cargo/config.toml` pins `RULEC_LANG=ja` for the suite. The
//! output language is a process-wide setting, so a binary must not mix languages — the
//! same reason `golden_en.rs` is separate from `golden.rs`.

fn readme() -> String {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md");
    std::fs::read_to_string(&p).expect("README.md が読めない")
}

fn root(rel: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)
}

/// How many files with that extension the directory holds.
fn count(dir: &str, ext: &str) -> usize {
    std::fs::read_dir(root(dir))
        .unwrap_or_else(|_| panic!("読めない: {dir}"))
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == ext))
        .count()
}

/// The README's worked example: the first ```rule block that holds a whole rule.
///
/// Found by the fence rather than by the heading above it. Keyed to a heading, reorganising
/// the README turns this suite into a test that passes while checking nothing — which is the
/// failure mode the rest of the file exists to prevent.
fn example(md: &str) -> &str {
    let mut rest = md;
    while let Some(k) = rest.find("```rule\n") {
        let open = k + "```rule\n".len();
        let close = rest[open..].find("```").expect("コードブロックが閉じていない") + open;
        if rest[open..close].starts_with("rule ") {
            return &rest[open..close];
        }
        rest = &rest[close..];
    }
    panic!("README に規則ひとつぶんの ```rule の塊が無い");
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
    let (f, c) = rulec::prepare(src, "README.md").expect("README の例は検査を通る");
    let g = rulec::codegen::Gen::new(&f, &c, src);
    // Whichever languages the README happens to show, held to what the generator writes for
    // that same example. The excerpts are abridged with `...`, so the whole cannot be
    // compared, but every line shown must be a line the generator really writes: they were
    // copied by hand, and hand-copied text rots.
    let mut shown = 0usize;
    for (lang, body) in [("python", g.python()), ("go", g.go())] {
        let fence = format!("```{lang}\n");
        let mut rest = md.as_str();
        while let Some(k) = rest.find(&fence) {
            let open = k + fence.len();
            let close = rest[open..].find("```").expect("抜粋が閉じていない") + open;
            for line in rest[open..close].lines() {
                if line.trim() == "..." || line.trim().is_empty() {
                    continue;
                }
                assert!(body.contains(line), "README の {lang} 抜粋が生成物に無い:\n{line}");
            }
            shown += 1;
            rest = &rest[close..];
        }
    }
    assert!(shown > 0, "README に生成コードの抜粋が無い");
}

/// The first diagnostic the README shows is the one a reader gets with no flags at all.
#[test]
fn readmeの診断抜粋は既定の言語で出る() {
    rulec::i18n::set(rulec::i18n::Lang::En);
    let md = readme();
    let src = std::fs::read_to_string(root("tests/mutants/m_e101en.rule")).unwrap();
    let lines: Vec<String> = src.lines().map(|s| s.to_string()).collect();
    let ds = rulec::check_source(&src, "rules/parcel.rule");
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
    let md = readme();
    let corpus = count("tests/corpus", "rule");
    let codes = rulec::codes::ledger().len();
    // A rule counts as transcribed when its description names the published source it was
    // copied from. The rest are sketches and examples — some modelled on a real rule but not
    // copied from one — written to reach the words a transcription never does, so the
    // sentence counts them apart rather than calling them something they are not (§15.138).
    const WRITTEN: usize = 27;
    let want = format!(
        "{corpus} rules — {} transcribed from a published source, {WRITTEN} written to reach the rest of the language — are checked, generated and run on every commit, and all {codes} diagnostics are implemented.",
        corpus - WRITTEN
    );
    assert!(md.contains(&want), "README の件数が実物と違う。正しくは: {want}");
}

/// The tree under "What is in this repository" counts files, and every one of those counts
/// rots the moment a file is added — by the time this test went in, four of the five were
/// wrong, one of them by a factor of two and a half. Held to the directories themselves, the
/// way `tests/website.rs` holds the assurance page's counts.
#[test]
fn readmeのツリーが言う件数は実物と合っている() {
    let md = readme();
    for want in [
        format!(
            "{} modules, and {} more under codegen/",
            count("src", "rs"),
            count("src/codegen", "rs")
        ),
        format!("{} rules, and the copies of the documents they cite", count("tests/corpus", "rule")),
        format!("{} files, each with one mistake planted in it", count("tests/mutants", "rule")),
        format!(
            "{} in Japanese, {} in English",
            count("tests/golden", "txt"),
            count("tests/golden/en", "txt")
        ),
    ] {
        assert!(md.contains(&want), "README のツリーの件数が実物と違う。「{want}」が要る");
    }
}

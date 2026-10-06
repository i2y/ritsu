//! The prose a flow writes stays text in the Markdown `dandori doc` writes (ritsu's DESIGN 9.2), as
//! in the page rulec's `doc` writes for a rule: a `<` that would open HTML there — a tag, a comment,
//! an autolink — is written `&lt;`, outside the code spans, in the workflow's description and in the
//! reason of a `fail`. `a > b`, `R&D` and a code span are as written. The HTML page has escaped them
//! all along.

use dandori::diag::Lang;
use dandori::doc::{self, Input};
use ritsu_testkit::TempDir;

const FLOW: &str = "workflow notice v1
description \"Tells the customer, a > b and R&D. <img src=x onerror=alert(1)> <!-- c --> and `<b>` stay as they read\"

inputs
  order : string

task send(order: string)
  lambda \"arn:aws:lambda:us-east-1:123456789012:function:send\"
  key
  errors bounced

flow
  send(order: order)
    on bounced => fail Bounced \"<script>alert(1)</script>\"
";

/// What `doc` writes for the flow, in `lang`, as Markdown and as HTML.
fn written(lang: Lang) -> (String, String) {
    let t = TempDir::new("doc-text");
    let f = t.write("notice.flow", FLOW);
    let (src, d) = dandori::check::drawable(&f).unwrap();
    let m = d.model.as_ref().expect("the flow lowers");
    let i = Input { m, src: &src, file: "notice.flow", facts: &d.facts, diags: &d.diags, lang };
    (doc::markdown(&i), doc::html(&i))
}

#[test]
fn the_prose_of_a_flow_stays_text_in_the_markdown() {
    for lang in [Lang::En, Lang::Ja] {
        let (md, html) = written(lang);
        assert!(md.contains("\nTells the customer, a > b and R&D. &lt;img src=x onerror=alert(1)> &lt;!-- c --> and `<b>` stay as they read\n"), "{md}");
        assert!(md.contains("&lt;script>alert(1)&lt;/script>"), "{md}");
        assert!(!md.contains("<img") && !md.contains("<script") && !md.contains("<!-- c"), "{md}");
        // the page for a browser escapes every one of them
        assert!(!html.contains("<img src=x") && !html.contains("<script>alert"), "the HTML page writes the prose as it is");
    }
}

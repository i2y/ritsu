//! Keys written into a file (DESIGN 16.3): each kind found where it is, its prefix and length
//! shown and the key never, and what is not one — too short, of the wrong characters, run on
//! into a letter or a digit, an example (`EXAMPLE` at the end of an AWS key ID, one character over
//! and over), or a value for tests (`ritsu: test secret` on its line).
//!
//! No key is written here in one piece: each is joined from parts as the test runs, so that the
//! repository holds no value of a key's shape (GitHub's secret scanning reads it). The one fake
//! key the fixtures of the languages hold is the Google API key of DESIGN 16.10.

use ritsu_base::secrets::{TEST_MARK, kinds, mask, scan};

/// What the scan finds in `text`: the kind's id, where, what is shown, how long, whether a test.
fn found(text: &str) -> Vec<(&'static str, usize, usize, String, usize, bool)> {
    scan(text).into_iter().map(|f| (f.kind.id, f.line, f.col, f.shown, f.len, f.test)).collect()
}

fn ids(text: &str) -> Vec<&'static str> {
    scan(text).into_iter().map(|f| f.kind.id).collect()
}

/// `n` characters of letters and digits, from a run that is not one character over and over.
fn chars(n: usize) -> String {
    "Q7tF2mZ9wK5vNb3XrL8pJ4hD6sG1cY0e".chars().cycle().take(n).collect()
}

/// The fake Google API key of DESIGN 16.10: 39 characters that read as fake.
fn fake_google() -> String {
    ["AIzaSyD-ritsu-fake-key-for-tests-", &"0".repeat(6)].concat()
}

fn aws() -> String {
    ["AKIA", "Q7TF", "MZ3W", "K5VN", "B2XR"].concat()
}

#[test]
fn the_kinds_are_the_nine_of_the_table() {
    let ks: Vec<(&str, &str, &str)> = kinds().iter().map(|k| (k.id, k.name.en.as_str(), k.provider)).collect();
    assert_eq!(
        ks,
        [
            ("aws-access-key-id", "an AWS access key ID", "AWS"),
            ("github-token", "a GitHub token", "GitHub"),
            ("slack-token", "a Slack token", "Slack"),
            ("slack-webhook-url", "a Slack incoming webhook URL", "Slack"),
            ("stripe-key", "a Stripe secret key", "Stripe"),
            ("openai-api-key", "an OpenAI API key", "OpenAI"),
            ("anthropic-api-key", "an Anthropic API key", "Anthropic"),
            ("google-api-key", "a Google API key", "Google"),
            ("private-key", "a private key", ""),
        ]
    );
    assert_eq!(kinds()[7].name.ja, "Google の API キー");
    assert_eq!(kinds()[0].name.ja, "AWS のアクセスキー ID");
    assert_eq!(TEST_MARK, "ritsu: test secret");
}

#[test]
fn a_key_is_found_where_it_starts_and_shown_by_its_prefix() {
    let key = fake_google();
    assert_eq!(key.chars().count(), 39);
    let text = format!("task find(q: string) -> Place\n  http GET \"https://maps.example.com/v1/find?key={key}\"\n");
    assert_eq!(found(&text), [("google-api-key", 2, 50, "AIza…".to_string(), 39, false)]);
    // the column counts characters, not bytes
    let ja = format!("# 鍵: {key}\n");
    assert_eq!(found(&ja), [("google-api-key", 1, 6, "AIza…".to_string(), 39, false)]);
    // a key in a comment is found as one in a string is, and each place is told
    let twice = format!("// {key}\nkey: \"{key}\"\n");
    assert_eq!(found(&twice).iter().map(|f| (f.1, f.2)).collect::<Vec<_>>(), [(1, 4), (2, 7)]);
    // the key itself is never given back
    assert!(scan(&text).iter().all(|f| !f.shown.contains(&key[4..])));
}

#[test]
fn a_value_for_tests_says_so_on_its_line() {
    let key = fake_google();
    let text = format!("  http GET \"https://maps.example.com/v1/find?key={key}\"   # {TEST_MARK}\nother: \"{key}\"\n");
    let f = found(&text);
    assert_eq!((f[0].5, f[1].5), (true, false));
}

#[test]
fn aws_access_key_ids() {
    let k = aws();
    assert_eq!(found(&format!("id = \"{k}\"")), [("aws-access-key-id", 1, 7, "AKIA…".to_string(), 20, false)]);
    for p in ["ASIA", "ABIA", "ACCA"] {
        assert_eq!(ids(&format!("{p}{}", &k[4..])), ["aws-access-key-id"], "{p}");
    }
    let a3t = ["A3TX", &k[4..]].concat();
    assert_eq!(found(&a3t)[0].3, "A3T…");
    // too short; a character outside A–Z and 2–7; run on into a letter or a digit on either side
    assert!(ids(&k[..19]).is_empty());
    assert!(ids(&format!("{}1", &k[..19])).is_empty());
    assert!(ids(&format!("x{k}")).is_empty());
    assert!(ids(&format!("{k}9")).is_empty());
    assert!(ids(&format!("{k}_")).is_empty(), "`_` is of a word, as `\\b` takes it");
    assert_eq!(ids(&format!("({k})")), ["aws-access-key-id"]);
    // AWS's own example, and one character over and over
    assert!(ids(&["AKIA", "IOSFODNN7", "EXAMPLE"].concat()).is_empty());
    assert!(ids(&["AKIA", &"A".repeat(16)].concat()).is_empty());
}

#[test]
fn github_tokens() {
    for p in ["ghp_", "gho_", "ghu_", "ghs_", "ghr_"] {
        let t = [p, &chars(36)].concat();
        assert_eq!(found(&t), [("github-token", 1, 1, format!("{p}…"), 40, false)], "{p}");
    }
    let fine = ["github_pat_", &chars(22), "_", &chars(59)].concat();
    assert_eq!(found(&fine), [("github-token", 1, 1, "github_pat_…".to_string(), 93, false)]);
    assert!(ids(&["ghp_", &chars(35)].concat()).is_empty());
    assert!(ids(&["ghp_", &chars(37)].concat()).is_empty());
    assert!(ids(&["ghx_", &chars(36)].concat()).is_empty());
    assert!(ids(&["xghp_", &chars(36)].concat()).is_empty());
    assert!(ids(&["ghp_", &"x".repeat(36)].concat()).is_empty());
}

#[test]
fn slack_tokens_and_webhooks() {
    let bot = ["xoxb-", "1234567890", "-", "9876543210", "-", &chars(24)].concat();
    assert_eq!(found(&bot), [("slack-token", 1, 1, "xoxb-…".to_string(), bot.len(), false)]);
    let legacy = ["xoxb-", "12345678", "-", &chars(20)].concat();
    assert_eq!(ids(&legacy), ["slack-token"]);
    let user = ["xoxp-", "1234567890-", "1234567890-", "1234567890-", &chars(32)].concat();
    assert_eq!(found(&user)[0].3, "xoxp-…");
    let app = ["xapp-", "1-", "A0123456789-", "1234567890123-", &chars(16).to_lowercase()].concat();
    assert_eq!(found(&app)[0].3, "xapp-…");
    let refresh = ["xoxe-", "1-", &chars(146)].concat();
    assert_eq!(ids(&refresh), ["slack-token"]);
    let config = ["xoxe.xoxp-", "1-", &chars(164)].concat();
    assert_eq!(found(&config)[0].3, "xoxe.xoxp-…");
    assert!(ids(&["xoxb-", "123", "-", "9876543210"].concat()).is_empty());
    assert!(ids(&["xoxe-", "1-", &chars(145)].concat()).is_empty());
    let hook = ["https://hooks.slack.com/services/", "T01234567/B01234567/", &chars(24)].concat();
    assert_eq!(found(&format!("url: \"{hook}\"")), [("slack-webhook-url", 1, 7, "https://hooks.slack.com/services/…".to_string(), hook.len(), false)]);
    let bare = ["hooks.slack.com/workflows/", "T01234567/B01234567/", &chars(24)].concat();
    assert_eq!(found(&bare)[0].3, "hooks.slack.com/workflows/…");
    assert!(ids(&["https://hooks.slack.com/services/", "T0/B0/abc"].concat()).is_empty());
}

#[test]
fn stripe_keys() {
    for p in ["sk_live_", "sk_test_", "sk_prod_", "rk_live_"] {
        let k = [p, &chars(24)].concat();
        assert_eq!(found(&format!("\"{k}\"")), [("stripe-key", 1, 2, format!("{p}…"), 32, false)], "{p}");
    }
    // what follows must end a value: a quote, a space, `;`, a line's end, `\n` written in a string
    let k = ["sk_live_", &chars(24)].concat();
    for after in ["'", " ", ";", "\n", "\\n", "`", ""] {
        assert_eq!(ids(&format!("{k}{after}")), ["stripe-key"], "{after:?}");
    }
    assert!(ids(&format!("{k}-")).is_empty());
    assert!(ids(&["sk_live_", &chars(9)].concat()).is_empty());
    assert!(ids(&["sk_live_", &chars(100)].concat()).is_empty());
    assert!(ids(&["sk_dev_", &chars(24)].concat()).is_empty());
    assert!(ids(&["sk_live_", &"0".repeat(24)].concat()).is_empty());
}

#[test]
fn openai_api_keys() {
    for (p, a) in [("sk-proj-", 74), ("sk-svcacct-", 58), ("sk-admin-", 74)] {
        let k = [p, &chars(a), "T3BlbkFJ", &chars(a)].concat();
        assert_eq!(found(&format!("{k}\n")), [("openai-api-key", 1, 1, format!("{p}…"), k.len(), false)], "{p}");
    }
    let old = ["sk-", &chars(20), "T3BlbkFJ", &chars(20)].concat();
    assert_eq!(found(&old), [("openai-api-key", 1, 1, "sk-…".to_string(), 51, false)]);
    assert!(ids(&["sk-proj-", &chars(70), "T3BlbkFJ", &chars(74)].concat()).is_empty());
    assert!(ids(&["sk-", &chars(20), "T3Blbk", &chars(22)].concat()).is_empty());
}

#[test]
fn anthropic_api_keys() {
    for p in ["sk-ant-api03-", "sk-ant-admin01-"] {
        let k = [p, &chars(93), "AA"].concat();
        assert_eq!(found(&format!("\"{k}\"")), [("anthropic-api-key", 1, 2, format!("{p}…"), k.len(), false)], "{p}");
        assert!(ids(&[p, &chars(92), "AA"].concat()).is_empty());
        assert!(ids(&[p, &chars(93), "AB"].concat()).is_empty());
    }
}

#[test]
fn google_api_keys() {
    let k = fake_google();
    assert_eq!(ids(&k), ["google-api-key"]);
    assert!(ids(&k[..38]).is_empty());
    assert!(ids(&format!("{k}0")).is_empty());
    assert!(ids(&format!("{k}-")).is_empty());
    assert!(ids(&format!("_{k}")).is_empty());
    assert!(ids(&["AIza", &"x".repeat(35)].concat()).is_empty());
}

#[test]
fn private_keys() {
    let begin = ["-----BEGIN ", "RSA PRIVATE KEY", "-----"].concat();
    let line = chars(64);
    let pem = format!("{begin}\n{line}\n{line}\n-----END RSA PRIVATE KEY-----\n");
    assert_eq!(found(&pem), [("private-key", 1, 1, begin.clone(), begin.len(), false)]);
    // in a string, with `\n` written; in YAML, indented; in comments
    assert_eq!(ids(&format!("key: \"{begin}\\n{line}\\n\"")), ["private-key"]);
    assert_eq!(ids(&format!("key: |\n  {begin}\n  {line}\n")), ["private-key"]);
    assert_eq!(ids(&format!("# {begin}\n# {line}\n")), ["private-key"]);
    assert_eq!(ids(&format!("// {begin}\n// {line}\n")), ["private-key"]);
    // the base64 may run over lines shorter than 64
    assert_eq!(ids(&format!("{begin}\n{}\n{}\n", &line[..32], &line[..32])), ["private-key"]);
    // other headers
    for h in ["-----BEGIN PRIVATE KEY-----", "-----BEGIN OPENSSH PRIVATE KEY-----", "-----BEGIN PGP PRIVATE KEY BLOCK-----", "-----begin ec private key-----"] {
        assert_eq!(found(&format!("{h}\n{line}\n"))[0].3, h, "{h}");
    }
    // a header alone, a header and words, a public key, a body of one character
    assert!(ids(&format!("{begin}\n-----END RSA PRIVATE KEY-----\n")).is_empty());
    assert!(ids(&format!("{begin} marks where a key starts, which the tool reads from the environment and never from a file at all\n")).is_empty());
    assert!(ids(&format!("-----BEGIN PUBLIC KEY-----\n{line}\n")).is_empty());
    assert!(ids(&format!("{begin}\n{}\n", "A".repeat(64))).is_empty());
    // the test mark goes on the header's line
    assert!(found(&format!("{begin}  # {TEST_MARK}\n{line}\n"))[0].5);
}

#[test]
fn keys_are_found_in_the_order_written_and_text_without_keys_has_none() {
    let text = format!("a: \"{}\"\nb: \"{}\" c: \"{}\"\n", fake_google(), aws(), ["ghp_", &chars(36)].concat());
    assert_eq!(found(&text).iter().map(|f| (f.0, f.1)).collect::<Vec<_>>(), [("google-api-key", 1), ("aws-access-key-id", 2), ("github-token", 2)]);
    // hashes, IDs and base64 of no known shape are not keys
    let plain = "source fetch \"https://law.example/x\" sha256:9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08\nid 550e8400-e29b-41d4-a716-446655440000\nsk-not-a-key\nAKIA\n";
    assert!(scan(plain).is_empty());
    assert!(scan("").is_empty());
}

#[test]
fn a_text_with_its_keys_masked() {
    let key = fake_google();
    assert_eq!(mask(&format!("  http GET \"https://maps.example.com/v1/find?key={key}\"   # {TEST_MARK}")), format!("  http GET \"https://maps.example.com/v1/find?key=AIza…\"   # {TEST_MARK}"));
    assert_eq!(mask(&format!("a: {} b: {}", aws(), ["ghp_", &chars(36)].concat())), "a: AKIA… b: ghp_…");
    let begin = ["-----BEGIN ", "PRIVATE KEY", "-----"].concat();
    assert_eq!(mask(&format!("{begin}\n{}\n{}\n-----END PRIVATE KEY-----\n", chars(64), chars(64))), format!("{begin}…\n-----END PRIVATE KEY-----\n"));
    // an example and a text with no key are as they were
    let example = ["AKIA", "IOSFODNN7", "EXAMPLE"].concat();
    assert_eq!(mask(&example), example);
    assert_eq!(mask("no key here"), "no key here");
}

//! The registry inside a test (DESIGN 17.25), asked with curl as `ritsu mod` asks one: a blob and an
//! OCI manifest there and back, `tags/list` a page at a time, the referrers filtered, the three
//! ways of credentials, a blob's `GET` sent on to the second port, and the broken answers.

use ritsu_testkit::registry::{Auth, Json, OCI_MANIFEST, Registry, TokenAuth, base64_encode, digest};
use ritsu_testkit::{Level, Need, TempDir};
use std::process::Command;

/// An answer: its status, its headers (the last response's, after a `100 Continue`), its body.
struct Answer {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Answer {
    fn header(&self, k: &str) -> Option<&str> {
        self.headers.iter().find(|(x, _)| x.eq_ignore_ascii_case(k)).map(|(_, v)| v.as_str())
    }

    fn json(&self) -> Json {
        Json::parse(std::str::from_utf8(&self.body).unwrap()).unwrap()
    }
}

/// `curl` on `url` with `method`, the headers, and `body` sent from a file.
fn curl(t: &TempDir, method: &str, url: &str, headers: &[&str], body: Option<&[u8]>) -> Answer {
    let out = t.path().join("body");
    let dump = t.path().join("headers");
    let _ = std::fs::remove_file(&out);
    let mut c = Command::new("curl");
    c.args(["-q", "--silent", "--show-error", "--max-time", "30", "--dump-header"]).arg(&dump).arg("--output").arg(&out);
    if method == "HEAD" {
        c.arg("--head");
    } else {
        c.args(["--request", method]);
    }
    for h in headers {
        c.args(["--header", h]);
    }
    if let Some(b) = body {
        let f = t.write("sent", b);
        c.arg("--data-binary").arg(format!("@{}", f.display()));
    }
    c.arg(url);
    let o = c.output().expect("curl runs");
    assert!(o.status.success(), "curl {method} {url}: {}", String::from_utf8_lossy(&o.stderr));
    let text = std::fs::read_to_string(&dump).unwrap();
    // the last block of headers: a `100 Continue` comes before it
    let block = text.split("\r\n\r\n").filter(|b| b.starts_with("HTTP/")).last().unwrap_or("");
    let mut lines = block.lines();
    let status = lines.next().and_then(|l| l.split_whitespace().nth(1)).and_then(|s| s.parse().ok()).unwrap_or(0);
    let headers = lines.filter_map(|l| l.split_once(':').map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))).collect();
    let body = std::fs::read(&out).unwrap_or_default();
    Answer { status, headers, body }
}

/// A manifest of ritsu's shape, with its config and one layer.
fn manifest(config: &str, layer: (&str, usize), subject: Option<(&str, usize)>, artifact: &str) -> Vec<u8> {
    let mut m = format!(
        "{{\"schemaVersion\":2,\"mediaType\":\"{OCI_MANIFEST}\",\"artifactType\":\"{artifact}\",\"config\":{{\"mediaType\":\"application/vnd.oci.empty.v1+json\",\"digest\":\"{config}\",\"size\":2}},\"layers\":[{{\"mediaType\":\"application/octet-stream\",\"digest\":\"{}\",\"size\":{}}}]",
        layer.0, layer.1
    );
    if let Some((d, n)) = subject {
        m.push_str(&format!(",\"subject\":{{\"mediaType\":\"{OCI_MANIFEST}\",\"digest\":\"{d}\",\"size\":{n}}}"));
    }
    m.push('}');
    m.into_bytes()
}

#[test]
fn the_needs_of_the_modules_have_their_levels() {
    assert_eq!(Need::Cosign.level(), Level::Tools);
    assert_eq!(Need::Registry.level(), Level::Platforms);
}

#[test]
fn the_digests_are_sha256() {
    assert_eq!(digest(b""), "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    assert_eq!(digest(b"{}"), "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a");
    // a message across two blocks
    assert_eq!(digest(&[b'a'; 100]), "sha256:2816597888e4a0d3a36b82b83316ab32680eb8f00f8cd3b904d681246d285a0e");
    assert_eq!(base64_encode(b"user:pass"), "dXNlcjpwYXNz");
}

#[test]
fn a_blob_and_a_manifest_go_there_and_back() {
    let t = TempDir::new("registry-roundtrip");
    let r = Registry::start();
    let base = &r.url;
    assert_eq!(curl(&t, "GET", &format!("{base}/v2/"), &[], None).status, 200);
    // the empty config, in one request
    let config = digest(b"{}");
    let a = curl(&t, "POST", &format!("{base}/v2/acme/pricing/blobs/uploads/?digest={config}"), &["Content-Type: application/octet-stream"], Some(b"{}"));
    assert_eq!(a.status, 201);
    // a layer in parts, as cosign sends one: POST, PATCH, PATCH, PUT
    let layer = b"the files of a module".to_vec();
    let d = digest(&layer);
    let a = curl(&t, "POST", &format!("{base}/v2/acme/pricing/blobs/uploads/"), &[], None);
    assert_eq!(a.status, 202);
    let at = a.header("Location").unwrap().to_string();
    let a = curl(&t, "PATCH", &format!("{base}{at}"), &["Content-Type: application/octet-stream"], Some(&layer[..10]));
    assert_eq!((a.status, a.header("Range")), (202, Some("0-9")));
    let at = a.header("Location").unwrap().to_string();
    let a = curl(&t, "PATCH", &format!("{base}{at}"), &["Content-Type: application/octet-stream"], Some(&layer[10..]));
    let at = a.header("Location").unwrap().to_string();
    let a = curl(&t, "PUT", &format!("{base}{at}?digest={d}"), &[], Some(b""));
    assert_eq!((a.status, a.header("Docker-Content-Digest")), (201, Some(d.as_str())));
    // a digest that is not the bytes'
    let a = curl(&t, "POST", &format!("{base}/v2/acme/pricing/blobs/uploads/?digest={config}"), &[], Some(b"{ }"));
    assert_eq!((a.status, a.json().get("errors").unwrap().as_arr().unwrap()[0].get("code").unwrap().as_str()), (400, Some("DIGEST_INVALID")));
    // the blob
    let a = curl(&t, "HEAD", &format!("{base}/v2/acme/pricing/blobs/{d}"), &[], None);
    assert_eq!((a.status, a.header("Content-Length")), (200, Some("21")));
    assert_eq!(curl(&t, "GET", &format!("{base}/v2/acme/pricing/blobs/{d}"), &[], None).body, layer);
    assert_eq!(curl(&t, "GET", &format!("{base}/v2/acme/tax/blobs/{d}"), &[], None).status, 404);
    // a manifest whose blobs are not there is refused; one whose blobs are is taken
    let m = manifest(&config, (&d, layer.len()), None, "application/vnd.ritsu.module.v1");
    let missing = manifest(&config, (&digest(b"x"), 1), None, "application/vnd.ritsu.module.v1");
    let a = curl(&t, "PUT", &format!("{base}/v2/acme/pricing/manifests/1.0.0"), &[&format!("Content-Type: {OCI_MANIFEST}")], Some(&missing));
    assert_eq!(a.status, 400);
    let a = curl(&t, "PUT", &format!("{base}/v2/acme/pricing/manifests/1.0.0"), &[&format!("Content-Type: {OCI_MANIFEST}")], Some(&m));
    let md = digest(&m);
    assert_eq!((a.status, a.header("Docker-Content-Digest")), (201, Some(md.as_str())));
    assert_eq!(r.tag("acme/pricing", "1.0.0").as_deref(), Some(md.as_str()));
    // by its tag and by its digest
    let a = curl(&t, "GET", &format!("{base}/v2/acme/pricing/manifests/1.0.0"), &[&format!("Accept: {OCI_MANIFEST}")], None);
    assert_eq!((a.status, a.header("Content-Type"), a.body.clone()), (200, Some(OCI_MANIFEST), m.clone()));
    let a = curl(&t, "HEAD", &format!("{base}/v2/acme/pricing/manifests/{md}"), &[], None);
    assert_eq!((a.status, a.header("Docker-Content-Digest")), (200, Some(md.as_str())));
    assert_eq!(curl(&t, "HEAD", &format!("{base}/v2/acme/pricing/manifests/2.0.0"), &[], None).status, 404);
    // the log has each request
    let log = r.log();
    assert!(log.iter().any(|s| s.method == "PATCH" && s.status == 202), "{log:?}");
}

#[test]
fn the_tags_come_a_page_at_a_time() {
    let t = TempDir::new("registry-tags");
    let r = Registry::start();
    for v in ["1.0.0", "1.1.0", "2.0.0", "sha256-abc.sig", "0.9.0"] {
        r.put_manifest("acme/tax", Some(v), OCI_MANIFEST, format!("{{\"v\":\"{v}\"}}").as_bytes());
    }
    let a = curl(&t, "GET", &format!("{}/v2/acme/tax/tags/list?n=2", r.url), &[], None);
    let tags = |a: &Answer| -> Vec<String> { a.json().get("tags").unwrap().as_arr().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect() };
    assert_eq!(tags(&a), ["0.9.0", "1.0.0"]);
    let link = a.header("Link").unwrap().to_string();
    assert_eq!(link, "</v2/acme/tax/tags/list?n=2&last=1.0.0>; rel=\"next\"");
    let next = link.trim_start_matches('<').split('>').next().unwrap().to_string();
    let a = curl(&t, "GET", &format!("{}{next}", r.url), &[], None);
    assert_eq!(tags(&a), ["1.1.0", "2.0.0"]);
    let a = curl(&t, "GET", &format!("{}/v2/acme/tax/tags/list?n=2&last=2.0.0", r.url), &[], None);
    assert_eq!((tags(&a), a.header("Link")), (vec!["sha256-abc.sig".to_string()], None));
    assert_eq!(curl(&t, "GET", &format!("{}/v2/acme/none/tags/list", r.url), &[], None).status, 404);
}

#[test]
fn the_referrers_are_filtered_by_their_type() {
    let t = TempDir::new("registry-referrers");
    let r = Registry::start();
    let config = r.put_blob("acme/pricing", b"{}");
    let layer = r.put_blob("acme/pricing", b"layer");
    let subject = manifest(&config, (&layer, 5), None, "application/vnd.ritsu.module.v1");
    let sd = r.put_manifest("acme/pricing", Some("1.0.0"), OCI_MANIFEST, &subject);
    let sig = r.put_blob("acme/pricing", b"a signature");
    let signed = manifest(&config, (&sig, 11), Some((&sd, subject.len())), "application/vnd.dev.sigstore.bundle.v0.3+json");
    let a = curl(&t, "PUT", &format!("{}/v2/acme/pricing/manifests/{}", r.url, digest(&signed)), &[&format!("Content-Type: {OCI_MANIFEST}")], Some(&signed));
    assert_eq!((a.status, a.header("OCI-Subject")), (201, Some(sd.as_str())));
    let other = manifest(&config, (&layer, 5), Some((&sd, subject.len())), "application/example");
    r.put_manifest("acme/pricing", None, OCI_MANIFEST, &other);
    let types = |a: &Answer| -> Vec<String> {
        a.json().get("manifests").unwrap().as_arr().unwrap().iter().map(|m| m.get("artifactType").unwrap().as_str().unwrap().to_string()).collect()
    };
    let a = curl(&t, "GET", &format!("{}/v2/acme/pricing/referrers/{sd}", r.url), &[], None);
    assert_eq!((a.status, a.header("Content-Type")), (200, Some("application/vnd.oci.image.index.v1+json")));
    let mut all = types(&a);
    all.sort();
    assert_eq!(all, ["application/example", "application/vnd.dev.sigstore.bundle.v0.3+json"]);
    let a = curl(&t, "GET", &format!("{}/v2/acme/pricing/referrers/{sd}?artifactType=application/example", r.url), &[], None);
    assert_eq!((types(&a), a.header("OCI-Filters-Applied")), (vec!["application/example".to_string()], Some("artifactType")));
}

#[test]
fn basic_and_the_token_steps_ask_for_credentials() {
    let t = TempDir::new("registry-auth");
    // Basic
    let r = Registry::with_auth(Auth::Basic { user: "ritsu-test-user".into(), password: "ritsu-fake-password".into() });
    let a = curl(&t, "GET", &format!("{}/v2/", r.url), &[], None);
    assert_eq!((a.status, a.header("WWW-Authenticate")), (401, Some("Basic realm=\"ritsu-test\"")));
    let basic = format!("Authorization: Basic {}", base64_encode(b"ritsu-test-user:ritsu-fake-password"));
    assert_eq!(curl(&t, "GET", &format!("{}/v2/", r.url), &[&basic], None).status, 200);
    let wrong = format!("Authorization: Basic {}", base64_encode(b"ritsu-test-user:no"));
    assert_eq!(curl(&t, "GET", &format!("{}/v2/", r.url), &[&wrong], None).status, 401);
    drop(r);
    // the token's steps: the challenge names the realm on the same server and the scope
    let r = Registry::with_auth(Auth::Token(TokenAuth {
        credentials: Some(("ritsu-test-user".into(), "ritsu-fake-password".into())),
        refresh_token: Some("ritsu-fake-identity-token".into()),
        anonymous: true,
    }));
    let a = curl(&t, "GET", &format!("{}/v2/acme/pricing/tags/list", r.url), &[], None);
    let challenge = a.header("WWW-Authenticate").unwrap().to_string();
    assert_eq!(challenge, format!("Bearer realm=\"{}/token\",service=\"ritsu-test\",scope=\"repository:acme/pricing:pull\"", r.url));
    // with the user and the password: a token to pull and push
    let a = curl(&t, "GET", &format!("{}/token?service=ritsu-test&scope=repository:acme/pricing:pull,push", r.url), &[&basic], None);
    let token = a.json().get("token").unwrap().as_str().unwrap().to_string();
    assert!(token.starts_with("ritsu-fake-token-"), "{token}");
    let bearer = format!("Authorization: Bearer {token}");
    assert_eq!(curl(&t, "GET", &format!("{}/v2/acme/pricing/tags/list", r.url), &[&bearer], None).status, 404);
    assert_eq!(curl(&t, "POST", &format!("{}/v2/acme/pricing/blobs/uploads/", r.url), &[&bearer], None).status, 202);
    // a token for one repository does not reach another
    assert_eq!(curl(&t, "GET", &format!("{}/v2/acme/tax/tags/list", r.url), &[&bearer], None).status, 401);
    // with none: a token to pull only
    let a = curl(&t, "GET", &format!("{}/token?service=ritsu-test&scope=repository:acme/pricing:pull,push", r.url), &[], None);
    let anonymous = format!("Authorization: Bearer {}", a.json().get("token").unwrap().as_str().unwrap());
    assert_eq!(curl(&t, "GET", &format!("{}/v2/acme/pricing/tags/list", r.url), &[&anonymous], None).status, 404);
    assert_eq!(curl(&t, "POST", &format!("{}/v2/acme/pricing/blobs/uploads/", r.url), &[&anonymous], None).status, 401);
    // with an identity token: `POST` of the form
    let form = b"grant_type=refresh_token&service=ritsu-test&scope=repository%3Aacme%2Fpricing%3Apull&client_id=ritsu&refresh_token=ritsu-fake-identity-token";
    let a = curl(&t, "POST", &format!("{}/token", r.url), &["Content-Type: application/x-www-form-urlencoded"], Some(form));
    assert_eq!(a.status, 200);
    let refreshed = format!("Authorization: Bearer {}", a.json().get("access_token").unwrap().as_str().unwrap());
    assert_eq!(curl(&t, "GET", &format!("{}/v2/acme/pricing/tags/list", r.url), &[&refreshed], None).status, 404);
    let a = curl(&t, "POST", &format!("{}/token", r.url), &[], Some(b"grant_type=refresh_token&refresh_token=no"));
    assert_eq!(a.status, 401);
    // what reached the server
    assert!(r.log().iter().any(|s| s.target.starts_with("/token") && s.authorization.as_deref() == Some(&basic["Authorization: ".len()..])));
}

#[test]
fn a_blob_is_sent_on_to_the_second_port() {
    let t = TempDir::new("registry-redirect");
    let r = Registry::start();
    let d = r.put_blob("acme/pricing", b"files");
    r.redirect_blobs();
    let a = curl(&t, "GET", &format!("{}/v2/acme/pricing/blobs/{d}", r.url), &["Authorization: Bearer ritsu-fake-token"], None);
    assert_eq!(a.status, 307);
    let to = a.header("Location").unwrap().to_string();
    assert!(to.starts_with(&format!("http://127.0.0.1:{}/blobs/acme/pricing/{d}?", r.second)), "{to}");
    // what follows it without the header
    let a = curl(&t, "GET", &to, &[], None);
    assert_eq!((a.status, a.body), (200, b"files".to_vec()));
    let second: Vec<_> = r.log().into_iter().filter(|s| s.port == r.second).collect();
    assert_eq!((second.len(), second[0].authorization.as_deref()), (1, None));
}

#[test]
fn the_broken_answers() {
    let t = TempDir::new("registry-broken");
    let r = Registry::start();
    let d = r.put_blob("acme/pricing", b"files");
    // other bytes for a digest
    r.serve_instead(&d, b"other");
    assert_eq!(curl(&t, "GET", &format!("{}/v2/acme/pricing/blobs/{d}", r.url), &[], None).body, b"other");
    // a status, then the answer
    r.fail(Some("GET"), "/blobs/", 429, 1, Some(1));
    let a = curl(&t, "GET", &format!("{}/v2/acme/pricing/blobs/{d}", r.url), &[], None);
    assert_eq!((a.status, a.header("Retry-After")), (429, Some("1")));
    r.fail(None, "/manifests/", 500, 2, None);
    assert_eq!(curl(&t, "GET", &format!("{}/v2/acme/pricing/manifests/1.0.0", r.url), &[], None).status, 500);
    assert_eq!(curl(&t, "HEAD", &format!("{}/v2/acme/pricing/manifests/1.0.0", r.url), &[], None).status, 500);
    assert_eq!(curl(&t, "GET", &format!("{}/v2/acme/pricing/manifests/1.0.0", r.url), &[], None).status, 404);
    // a tag moved
    r.put_manifest("acme/pricing", Some("1.0.0"), OCI_MANIFEST, b"{\"a\":1}");
    let moved = r.put_manifest("acme/pricing", Some("1.0.0"), OCI_MANIFEST, b"{\"a\":2}");
    assert_eq!(r.tag("acme/pricing", "1.0.0"), Some(moved));
    // a tar that holds a link
    let tar = ritsu_testkit::registry::tar(&[("rules/a.rule", b'0', b"x", ""), ("rules/b.rule", b'2', b"", "/etc/passwd")]);
    assert_eq!(tar.len(), 512 * 2 + 512 + 1024);
    assert_eq!(&tar[512 + 512 + 156..512 + 512 + 157], b"2");
}

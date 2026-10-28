//! The signatures of the modules (DESIGN 17.14): cosign, run as a child process to check a module
//! against the `signature` lines that cover it, and the commands `ritsu mod push` prints to sign one.
//!
//! ritsu signs nothing and has no cryptography of its own. For each line that applies to a module
//! it runs `cosign verify <the line's arguments> [--allow-http-registry] <name>@sha256:<digest>`:
//!
//! | line | arguments |
//! |---|---|
//! | `key "<path>"` | `--key <absolute path>` |
//! | `key "<path>" tlog off` | `--key <absolute path> --insecure-ignore-tlog=true` |
//! | `by "<identity>" issuer "<issuer>"` | `--certificate-identity <identity> --certificate-oidc-issuer <issuer>` |
//! | `by matching "<pattern>" issuer "<issuer>"` | `--certificate-identity-regexp <pattern> --certificate-oidc-issuer <issuer>` |
//!
//! `--allow-http-registry` is for a registry of the loopback alone. cosign reads the registry's
//! credentials from the Docker config file itself: ritsu hands it no secret. cosign is found on the
//! PATH; when a module has a line to check and there is none, the command stops (exit 2).

use ritsu_base::modules::manifest::Policy;
use ritsu_base::text::Text;
use ritsu_base::tr;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// A line to check a module against.
#[derive(Clone, Debug)]
pub struct Rule {
    /// What the record of its check says ([`ritsu_base::modules::manifest::Signature::checked`]).
    pub checked: String,
    /// The line as `ritsu.mod` writes it.
    pub line: String,
    pub policy: Policy,
    /// For a key, the key file, absolute.
    pub key: Option<PathBuf>,
}

/// Why a check did not pass.
pub enum Failed {
    /// cosign cannot be run: not on the PATH, or under WASI (exit 2).
    Missing(Text),
    /// cosign said no: the first 20 lines of what it said on its standard error (exit 1).
    Refused(String),
}

/// The arguments of `cosign verify` for `rule` and the module at `reference` (`<name>@sha256:…`).
pub fn verify_args(rule: &Rule, reference: &str, loopback: bool) -> Vec<String> {
    let mut args = vec!["verify".to_string()];
    match &rule.policy {
        Policy::Key { tlog, .. } => {
            args.push("--key".to_string());
            args.push(rule.key.as_ref().map(|k| k.display().to_string()).unwrap_or_default());
            if !tlog {
                args.push("--insecure-ignore-tlog=true".to_string());
            }
        }
        Policy::By { identity, issuer } => {
            args.extend(["--certificate-identity".to_string(), identity.clone(), "--certificate-oidc-issuer".to_string(), issuer.clone()]);
        }
        Policy::Matching { pattern, issuer } => {
            args.extend(["--certificate-identity-regexp".to_string(), pattern.clone(), "--certificate-oidc-issuer".to_string(), issuer.clone()]);
        }
    }
    if loopback {
        args.push("--allow-http-registry".to_string());
    }
    args.push(reference.to_string());
    args
}

/// `cosign verify` of the module at `reference` against `rule`.
pub fn verify(rule: &Rule, reference: &str, loopback: bool) -> Result<(), Failed> {
    if cfg!(target_os = "wasi") {
        return Err(Failed::Missing(ritsu_base::wasi::cannot_start("cosign")));
    }
    let out = Command::new("cosign").args(verify_args(rule, reference, loopback)).stdin(Stdio::null()).output();
    let o = match out {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(Failed::Missing(tr!(
                "cosign がありません。署名を確かめるモジュールがあるので、cosign を入れて PATH に置いてください",
                "cosign is not there, and a module has a signature to check; install cosign and put it on the PATH"
            )));
        }
        Err(e) => return Err(Failed::Missing(ritsu_base::wasi::starting("cosign", &e))),
    };
    if o.status.success() {
        return Ok(());
    }
    let said: Vec<String> = String::from_utf8_lossy(&o.stderr).lines().take(20).map(str::to_string).collect();
    Err(Failed::Refused(said.join("\n")))
}

/// The commands that sign a module pushed to `reference`: with no key (in CI, by its OIDC), and
/// with a key.
pub fn sign_commands(reference: &str, loopback: bool) -> (String, String) {
    let http = if loopback { " --allow-http-registry" } else { "" };
    (format!("cosign sign --yes{http} {reference}"), format!("cosign sign --key cosign.key{http} {reference}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(policy: Policy) -> Rule {
        Rule { checked: String::new(), line: String::new(), key: Some(PathBuf::from("/k/acme.pub")), policy }
    }

    #[test]
    fn the_arguments_of_each_line() {
        let r = "127.0.0.1:5000/acme/tax@sha256:00";
        assert_eq!(verify_args(&rule(Policy::Key { path: "keys/acme.pub".into(), tlog: true }), r, false), ["verify", "--key", "/k/acme.pub", r]);
        assert_eq!(
            verify_args(&rule(Policy::Key { path: "keys/acme.pub".into(), tlog: false }), r, true),
            ["verify", "--key", "/k/acme.pub", "--insecure-ignore-tlog=true", "--allow-http-registry", r]
        );
        assert_eq!(
            verify_args(&rule(Policy::By { identity: "me@example.com".into(), issuer: "https://issuer.example.com".into() }), r, false),
            ["verify", "--certificate-identity", "me@example.com", "--certificate-oidc-issuer", "https://issuer.example.com", r]
        );
        assert_eq!(
            verify_args(&rule(Policy::Matching { pattern: "^https://github.com/acme/".into(), issuer: "https://token.actions.githubusercontent.com".into() }), r, false),
            ["verify", "--certificate-identity-regexp", "^https://github.com/acme/", "--certificate-oidc-issuer", "https://token.actions.githubusercontent.com", r]
        );
        assert_eq!(sign_commands(r, true).0, format!("cosign sign --yes --allow-http-registry {r}"));
    }
}

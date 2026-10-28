//! The tar of a module's files and the OCI manifest of a version (DESIGN 17.11): fixed bytes (a
//! golden of a small module's tar and digests, in hex), read back to the same files (sets of files
//! made at random from a fixed seed), every form ritsu does not write refused, a pax header before a
//! path longer than 100 bytes, two entries a file system takes as one file stopped when unpacked,
//! and the files a module carries.

use ritsu_base::modules::artifact;
use ritsu_base::modules::digest;
use ritsu_base::modules::manifest;
use ritsu_base::modules::tar::{self, Error, PathWhy};
use ritsu_testkit::TempDir;
use std::path::Path;

const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/modules");

/// SplitMix64: numbers from a seed, the same on every machine.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// The small module of the golden: a `ritsu.mod`, a rule, a Japanese name, and a path longer than
/// 100 bytes.
fn small() -> Vec<(String, Vec<u8>)> {
    let long = format!("rules/{}/fee.rule", "deep/".repeat(20).trim_end_matches('/'));
    vec![
        ("ritsu.mod".into(), b"module example.com/acme/pricing\nversion 1.1.0\nlicense \"MIT OR Apache-2.0\"\n".to_vec()),
        ("rules/shipping_fee.rule".into(), b"# a rule\n".to_vec()),
        ("規則/送料.rule".into(), "# 規則\n".as_bytes().to_vec()),
        (long, Vec::new()),
    ]
}

fn hex_lines(bytes: &[u8]) -> String {
    let mut out = String::new();
    for chunk in bytes.chunks(32) {
        out.push_str(&chunk.iter().map(|b| format!("{b:02x}")).collect::<String>());
        out.push('\n');
    }
    out
}

#[test]
fn a_small_module_has_fixed_bytes_and_digests() {
    let files = small();
    let t = tar::write(&files).unwrap();
    // the order of the files does not change the bytes
    let mut reversed = files.clone();
    reversed.reverse();
    assert_eq!(tar::write(&reversed).unwrap(), t);
    ritsu_testkit::golden::golden(Path::new(GOLDEN).join("small.tar.hex"), &hex_lines(&t));
    let m = manifest::parse(&files[0].1, "ritsu.mod").unwrap();
    let built = artifact::build(&m, &files[0].1, &files).unwrap();
    assert_eq!(built.tar, t);
    assert_eq!(built.files_digest, digest::of(&t));
    let text = format!(
        "files {}\nritsu.mod {}\nmanifest {}\n{}\n",
        built.files_digest,
        built.module_digest,
        built.digest,
        String::from_utf8(built.manifest.clone()).unwrap()
    );
    ritsu_testkit::golden::golden(Path::new(GOLDEN).join("small.digests"), &text);
    // the manifest checks, and says the layers it was built with
    let layers = artifact::check(&built.manifest).unwrap();
    assert_eq!((layers.module_digest.as_str(), layers.files_digest.as_str()), (built.module_digest.as_str(), built.files_digest.as_str()));
    assert_eq!((layers.title.as_deref(), layers.version.as_deref()), (Some("example.com/acme/pricing"), Some("1.1.0")));
    assert_eq!(digest::of(artifact::EMPTY), artifact::EMPTY_DIGEST);
}

#[test]
fn a_path_longer_than_100_bytes_has_one_pax_header() {
    let long = format!("{}/{}.rule", "あ".repeat(30), "b".repeat(20));
    assert!(long.len() > 100);
    // the long path's bytes come after `short.rule`'s
    let t = tar::write(&[(long.clone(), b"x".to_vec()), ("short.rule".into(), b"y".to_vec())]).unwrap();
    let flags: Vec<u8> = (0..t.len() / 512).map(|k| &t[k * 512..k * 512 + 512]).filter(|h| h[257..263] == *b"ustar\0").map(|h| h[156]).collect();
    assert_eq!(flags, [b'0', b'x', b'0'], "the short file, the pax header, the long file");
    assert_eq!(tar::read(&t).unwrap(), vec![("short.rule".into(), b"y".to_vec()), (long.clone(), b"x".to_vec())]);
    // the pax header's name, its record, and the file's name cut at a boundary of the characters
    let t = tar::write(&[(long.clone(), b"x".to_vec())]).unwrap();
    assert_eq!(&t[..15], b"././@PaxHeader\0");
    let record = format!("{} path={long}\n", long.len() + 10);
    assert_eq!(&t[512..512 + record.len()], record.as_bytes());
    let name = &t[1024..1124];
    let end = name.iter().position(|b| *b == 0).unwrap_or(100);
    let cut = std::str::from_utf8(&name[..end]).expect("cut at a boundary");
    assert!(long.starts_with(cut) && cut.len() <= 100 && cut.len() > 96, "{cut}");
    // a name of more than 100 bytes whose 100th byte is in a character
    let jp = "あ".repeat(40);
    let t = tar::write(&[(jp.clone(), vec![])]).unwrap();
    let name = &t[1024..1124];
    let end = name.iter().position(|b| *b == 0).unwrap_or(100);
    assert_eq!(std::str::from_utf8(&name[..end]).unwrap(), "あ".repeat(33));
    assert_eq!(tar::read(&t).unwrap(), vec![(jp, vec![])]);
}

#[test]
fn what_is_written_reads_back_the_same() {
    let seed = 0x5eed_0001_u64;
    let mut rng = Rng(seed);
    let parts = ["rules", "規則", "a", "b-c", "d_e", "f.g", "deep"];
    for round in 0..200 {
        let n = rng.below(12) as usize;
        let mut files: Vec<(String, Vec<u8>)> = Vec::new();
        for _ in 0..n {
            let deep = rng.below(10) == 0;
            let depth = 1 + rng.below(if deep { 30 } else { 4 }) as usize;
            let mut p: Vec<String> = (0..depth - 1).map(|_| parts[rng.below(parts.len() as u64) as usize].to_string()).collect();
            p.push(format!("f{}.rule", rng.below(1000)));
            let path = p.join("/");
            // a path that is a file and a directory at once is not a module's
            if files.iter().any(|(q, _)| q == &path || q.starts_with(&format!("{path}/")) || path.starts_with(&format!("{q}/"))) {
                continue;
            }
            let large = rng.below(5) == 0;
            let len = rng.below(if large { 3000 } else { 40 }) as usize;
            files.push((path, (0..len).map(|_| rng.below(256) as u8).collect()));
        }
        let t = tar::write(&files).unwrap_or_else(|e| panic!("seed {seed:#x}, round {round}: {:?}\n{:?}", e, files.iter().map(|f| &f.0).collect::<Vec<_>>()));
        let mut want = files.clone();
        want.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
        let got = tar::read(&t).unwrap_or_else(|e| panic!("seed {seed:#x}, round {round}: {}\n{:?}", e.text().en, want.iter().map(|f| &f.0).collect::<Vec<_>>()));
        assert_eq!(got, want, "seed {seed:#x}, round {round}");
        assert_eq!(t.len() % 512, 0);
    }
}

/// A header as a tar of another writer may have it, every field given.
fn header(name: &[u8], size: u64, flag: u8, magic: &[u8; 8], prefix: &[u8]) -> Vec<u8> {
    let mut h = vec![0u8; 512];
    h[..name.len()].copy_from_slice(name);
    h[100..108].copy_from_slice(b"0000644\0");
    h[108..116].copy_from_slice(b"0000000\0");
    h[116..124].copy_from_slice(b"0000000\0");
    h[124..136].copy_from_slice(format!("{size:011o}\0").as_bytes());
    h[136..148].copy_from_slice(b"00000000000\0");
    h[156] = flag;
    h[257..265].copy_from_slice(magic);
    h[345..345 + prefix.len()].copy_from_slice(prefix);
    h[148..156].copy_from_slice(b"        ");
    let sum: u32 = h.iter().map(|b| u32::from(*b)).sum();
    h[148..156].copy_from_slice(format!("{sum:06o}\0 ").as_bytes());
    h
}

/// A tar of the entries given (a name, a typeflag, a body), with its end.
fn tar_of(entries: &[(&[u8], u8, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    for (name, flag, body) in entries {
        out.extend(header(name, body.len() as u64, *flag, b"ustar\x0000", b""));
        out.extend(*body);
        while out.len() % 512 != 0 {
            out.push(0);
        }
    }
    out.extend([0u8; 1024]);
    out
}

fn pax(record: &str) -> Vec<u8> {
    let body = format!(" {record}\n");
    let mut n = body.len() + 1;
    while n.to_string().len() + body.len() != n {
        n = n.to_string().len() + body.len();
    }
    format!("{n}{body}").into_bytes()
}

#[test]
fn every_form_ritsu_does_not_write_is_refused() {
    let path = |p: &str, why: PathWhy| Err::<(), Error>(Error::Path(p.into(), why));
    let check = |t: Vec<u8>, want: Result<(), Error>| {
        let got = tar::read(&t).map(|_| ());
        assert_eq!(got, want);
        if let Err(e) = got {
            assert!(!e.text().ja.is_empty() && !e.text().en.is_empty());
        }
    };
    // the types: a hard link, a symbolic link, a directory, a device, a global header
    for flag in [b'1', b'2', b'5', b'3', b'4', b'6', b'g', b'L'] {
        check(tar_of(&[(b"a", flag, b"")]), Err(Error::Type("a".into(), flag)));
    }
    // the paths
    check(tar_of(&[(b"../x", b'0', b"")]), path("../x", PathWhy::DotDot));
    check(tar_of(&[(b"/etc/x", b'0', b"")]), path("/etc/x", PathWhy::Absolute));
    check(tar_of(&[(b"a//b", b'0', b"")]), path("a//b", PathWhy::EmptyPart));
    check(tar_of(&[(b"./a", b'0', b"")]), path("./a", PathWhy::Dot));
    check(tar_of(&[(b"a\\b", b'0', b"")]), path("a\\b", PathWhy::Backslash));
    check(tar_of(&[(b"a/.hidden", b'0', b"")]), path("a/.hidden", PathWhy::DotStart));
    check(tar_of(&[(b"@pricing/x.rule", b'0', b"")]), path("@pricing/x.rule", PathWhy::AtStart));
    check(tar_of(&[(b"a/", b'0', b"")]), path("a/", PathWhy::EmptyPart));
    check(tar_of(&[(b"\xffx", b'0', b"")]), path("\u{fffd}x", PathWhy::NotUtf8));
    check(tar_of(&[(b"x", b'x', &pax("path=a\0b")), (b"x", b'0', b"")]), path("a\0b", PathWhy::Nul));
    let deep = vec!["d"; tar::MAX_PARTS + 1].join("/");
    check(tar_of(&[(b"x", b'x', &pax(&format!("path={deep}"))), (b"x", b'0', b"")]), path(&deep, PathWhy::TooDeep));
    let long = "a".repeat(tar::MAX_PATH + 1);
    check(tar_of(&[(b"x", b'x', &pax(&format!("path={long}"))), (b"x", b'0', b"")]), path(&long, PathWhy::TooLong));
    // the same path twice; a file under a file
    check(tar_of(&[(b"a", b'0', b"1"), (b"a", b'0', b"2")]), Err(Error::Twice("a".into())));
    check(tar_of(&[(b"a", b'0', b""), (b"a/b", b'0', b"")]), Err(Error::UnderFile("a".into(), "a/b".into())));
    // a pax header with another record, two records, two headers in a row, or no file after it
    check(tar_of(&[(b"x", b'x', &pax("linkpath=y")), (b"x", b'0', b"")]), Err(Error::Pax("x".into())));
    let two = [pax("path=a"), pax("mtime=1")].concat();
    check(tar_of(&[(b"x", b'x', &two), (b"x", b'0', b"")]), Err(Error::Pax("x".into())));
    check(tar_of(&[(b"x", b'x', &pax("path=a")), (b"y", b'x', &pax("path=b")), (b"z", b'0', b"")]), Err(Error::Pax("y".into())));
    let t = tar_of(&[(b"x", b'x', &pax("path=a"))]);
    assert!(matches!(tar::read(&t), Err(Error::Shape(_, tar::Shape::PaxLast))));
    // the header itself: its checksum, its magic, its prefix
    let mut bad = tar_of(&[(b"a", b'0', b"")]);
    bad[0] = b'b';
    assert!(matches!(tar::read(&bad), Err(Error::Shape(148, tar::Shape::Checksum))));
    let mut gnu = header(b"a", 0, b'0', b"ustar  \0", b"");
    gnu.extend([0u8; 1024]);
    assert!(matches!(tar::read(&gnu), Err(Error::Shape(257, tar::Shape::Magic))));
    let mut prefixed = header(b"a", 0, b'0', b"ustar\x0000", b"dir");
    prefixed.extend([0u8; 1024]);
    assert!(matches!(tar::read(&prefixed), Err(Error::Shape(345, tar::Shape::Prefix))));
    // the end: one block of zeros, something after the two, no end at all
    let mut t = tar_of(&[(b"a", b'0', b"")]);
    t.truncate(t.len() - 512);
    assert!(matches!(tar::read(&t), Err(Error::Shape(_, tar::Shape::Short | tar::Shape::End))));
    let mut t = tar_of(&[(b"a", b'0', b"")]);
    t.extend([0u8; 512]);
    assert!(matches!(tar::read(&t), Err(Error::Shape(_, tar::Shape::End))));
    let t = header(b"a", 10, b'0', b"ustar\x0000", b"");
    assert!(matches!(tar::read(&t), Err(Error::Shape(_, tar::Shape::PastEnd))));
    // the limits: a file too large, too many files
    let big = vec![0u8; tar::MAX_FILE as usize + 1];
    check(tar_of(&[(b"big", b'0', &big)]), Err(Error::FileTooLarge("big".into(), tar::MAX_FILE + 1)));
    assert_eq!(tar::write(&[("big".into(), big)]), Err(Error::FileTooLarge("big".into(), tar::MAX_FILE + 1)));
    let names: Vec<String> = (0..=tar::MAX_FILES).map(|i| format!("f{i}")).collect();
    let entries: Vec<(&[u8], u8, &[u8])> = names.iter().map(|n| (n.as_bytes(), b'0', &b""[..])).collect();
    check(tar_of(&entries), Err(Error::TooManyFiles(tar::MAX_FILES + 1)));
    // and what ritsu writes refuses the same paths
    assert_eq!(tar::write(&[("../x".into(), vec![])]), Err(Error::Path("../x".into(), PathWhy::DotDot)));
    assert_eq!(tar::write(&[("a".into(), vec![]), ("a/b".into(), vec![])]), Err(Error::UnderFile("a".into(), "a/b".into())));
}

/// Whether the file system of the temporary directory takes `A` and `a` as one name.
fn folds_case(dir: &Path) -> bool {
    std::fs::write(dir.join("Probe"), b"").unwrap();
    let folds = dir.join("probe").exists();
    std::fs::remove_file(dir.join("Probe")).unwrap();
    folds
}

#[test]
fn two_entries_one_file_system_takes_as_one_file_stop_the_unpacking() {
    let t = TempDir::new("tar-case");
    let folds = folds_case(t.path());
    for (k, files) in [
        vec![("A.rule".to_string(), b"1".to_vec()), ("a.rule".to_string(), b"2".to_vec())],
        vec![("A/x.rule".to_string(), b"1".to_vec()), ("a/y.rule".to_string(), b"2".to_vec())],
        vec![("A/x.rule".to_string(), b"1".to_vec()), ("a".to_string(), b"2".to_vec())],
    ]
    .into_iter()
    .enumerate()
    {
        let dir = t.path().join(format!("u{k}"));
        std::fs::create_dir(&dir).unwrap();
        let bytes = tar::write(&files).unwrap();
        let read = tar::read(&bytes).unwrap();
        let got = tar::unpack(&read, &dir);
        if folds {
            assert!(matches!(got, Err(Error::Clash(_))), "{k}: {got:?}");
        } else {
            assert_eq!(got, Ok(()), "{k}");
        }
    }
    // a file already there is not written over
    let dir = t.path().join("there");
    std::fs::create_dir(&dir).unwrap();
    std::fs::write(dir.join("x.rule"), b"mine").unwrap();
    assert_eq!(tar::unpack(&[("x.rule".into(), b"theirs".to_vec())], &dir), Err(Error::Clash("x.rule".into())));
    assert_eq!(std::fs::read(dir.join("x.rule")).unwrap(), b"mine");
}

#[test]
fn the_digest_of_a_directory_is_the_digest_of_its_tar() {
    let t = TempDir::new("tar-digest");
    for (p, b) in small() {
        t.write(&format!("m/{p}"), b);
    }
    let files = digest::files_in(&t.path().join("m")).unwrap();
    assert_eq!(files, {
        let mut s = small();
        s.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
        s
    });
    assert_eq!(digest::files(&t.path().join("m")).unwrap(), digest::of(&tar::write(&small()).unwrap()));
    assert!(digest::is_digest(&digest::of(b"")) && !digest::is_digest("sha256:ABC") && !digest::is_digest("sha512:00"));
}

#[test]
fn a_module_carries_its_files_and_not_what_a_walk_passes_over() {
    let t = TempDir::new("tar-carry");
    let root = t.path().join("pricing");
    for (p, b) in [
        ("ritsu.mod", "module example.com/acme/pricing\nversion 1.0.0\nexclude \"generated\"\nexclude \"notes/draft.md\"\n"),
        ("ritsu.lock", "lock 1\nritsu 0.37.0\n"),
        ("rules/fee.rule", "# fee\n"),
        ("notes/draft.md", "draft\n"),
        ("notes/kept.md", "kept\n"),
        ("generated/ts/fee.ts", "x\n"),
        (".git/HEAD", "ref\n"),
        (".ritsu/modules/x", "x\n"),
        ("node_modules/x/index.js", "x\n"),
        ("target/debug/x", "x\n"),
        ("inner/ritsu.mod", "module inner\n"),
        ("inner/rules/x.rule", "# x\n"),
    ] {
        t.write(&format!("pricing/{p}"), b);
    }
    let m = manifest::parse(&std::fs::read(root.join("ritsu.mod")).unwrap(), "ritsu.mod").unwrap();
    let carried: Vec<String> = digest::module_files(&root, &m).unwrap().into_iter().map(|(p, _)| p).collect();
    assert_eq!(carried, ["notes/kept.md", "ritsu.lock", "ritsu.mod", "rules/fee.rule"]);
    // a name that starts with `@` at the root
    t.write("pricing/@tax/x.rule", "x");
    let e = digest::module_files(&root, &m).unwrap_err();
    assert!(e.en.starts_with("the root of the module has `@tax` in it"), "{}", e.en);
    std::fs::remove_dir_all(root.join("@tax")).unwrap();
    // a symbolic link
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("rules/fee.rule", root.join("link.rule")).unwrap();
        let e = digest::module_files(&root, &m).unwrap_err();
        assert!(e.en.starts_with("`link.rule` is neither a file nor a directory"), "{}", e.en);
    }
}

#[test]
fn a_fetched_manifest_that_is_not_a_modules_is_refused() {
    let files = small();
    let m = manifest::parse(&files[0].1, "ritsu.mod").unwrap();
    let built = artifact::build(&m, &files[0].1, &files).unwrap();
    let good = String::from_utf8(built.manifest.clone()).unwrap();
    let cases = [
        (good.replace("\"schemaVersion\":2", "\"schemaVersion\":1"), "`schemaVersion` is not 2"),
        (good.replace("vnd.ritsu.module.v1", "vnd.other.v1"), "`artifactType` is not `application/vnd.ritsu.module.v1`"),
        (good.replace("\"size\":2,", "\"size\":3,"), "its `config` is not the empty descriptor"),
        (good.replace("application/vnd.ritsu.mod.v1", "application/vnd.oci.image.layer.v1.tar"), "a layer's type is not `application/vnd.ritsu.mod.v1`"),
        (good.replacen("\"digest\":\"sha256:", "\"digest\":\"sha512:", 2), "its `config` is not the empty descriptor"),
    ];
    for (text, en) in cases {
        let e = artifact::check(text.as_bytes()).unwrap_err();
        assert_eq!(e.en, format!("the OCI manifest fetched is not one of a ritsu module: {en}"));
    }
    let huge = good.replace("\"files.tar\"", &format!("\"{}\"", "x".repeat(70_000)));
    assert!(artifact::check(huge.as_bytes()).unwrap_err().en.starts_with("the OCI manifest is "));
    let over = good.replace(&format!("\"size\":{},", built.tar.len()), &format!("\"size\":{},", tar::MAX_TAR + 1));
    assert!(artifact::check(over.as_bytes()).unwrap_err().en.contains(" bytes, and it is 67108864 bytes at most"));
    // a module with a short name, or no version, is no artifact
    let short = manifest::parse(b"module pricing\nversion 1.0.0\n", "ritsu.mod").unwrap();
    assert!(artifact::build(&short, b"module pricing\nversion 1.0.0\n", &[]).unwrap_err().en.contains("has a name that is not published"));
    let unversioned = manifest::parse(b"module example.com/a\n", "ritsu.mod").unwrap();
    assert_eq!(artifact::build(&unversioned, b"module example.com/a\n", &[]).unwrap_err().en, "the `ritsu.mod` of the module `example.com/a` has no `version`");
}

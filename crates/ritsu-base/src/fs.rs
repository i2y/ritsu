//! Where the languages read and write a project's files: the disk, or files held in memory
//! (DESIGN 4.15). A page in the browser has no disk; ritsu-wasm hands the languages the files of
//! the project the reader is editing, and every language reads them through here as it would read
//! them from a directory, so that what `ritsu check`, `gen` and `doc` answer in the page is what
//! they answer on the command line.
//!
//! The functions have the names and the answers of `std::fs` (and of `Path::exists`, `is_dir`,
//! `is_file`, `std::env::current_dir`). Without [`with`] they are `std::fs`, unchanged. Inside
//! [`with`], they read and write what it was given — [`Memory`], or any other [`Files`] — on this
//! thread only. Where the disk has no answer (a symbolic link, permissions), memory has none
//! either: it holds files and the directories they are in. [`watched`] notes what a run of code
//! reads through here, which a language keeps a check by while every file it read reads the same.
//! [`steady`] asks the disk each question once while a run of code asks it many times (the walk of
//! `dandori scenarios`, whose every call of a rule asks again what the rule's check read).
//!
//! Built for WASI (the npm package, DESIGN 8.8), they are `std::fs` with two differences that make
//! the answers the native binary's: an error of the disk says the system's number rather than
//! WASI's ([`as_native`]), and [`canonicalize`], which WASI's std does not have, is made by hand
//! ([`realpath`]).
//!
//! A module's dependency `<name>` is seen at `R/@<name>`, `R` the module's root (DESIGN 17.6): a
//! path with a part that starts with `@` right under a directory with a `ritsu.mod` is read where
//! [`crate::modules::refer::map`] says, the dependency's directory or its entry in the store, and
//! nothing is made on the disk. A path with no such part is read as it is, so a tree with no
//! `ritsu.mod` reads as it did (M14). What is written under a dependency is refused: its files are
//! read only. A listing of `R` does not show `@<name>`. [`raw`] reads without the view.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ffi::OsString;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Weak};

/// What is at a path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    File,
    Dir,
    /// A symbolic link (from `symlink_metadata`), a socket, a device.
    Other,
}

/// What [`metadata`] answers: what is at the path, and how long it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Meta {
    pub kind: Kind,
    pub len: u64,
}

impl Meta {
    pub fn is_file(&self) -> bool {
        self.kind == Kind::File
    }

    pub fn is_dir(&self) -> bool {
        self.kind == Kind::Dir
    }

    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn of(m: &std::fs::Metadata) -> Meta {
        let t = m.file_type();
        let kind = if t.is_file() {
            Kind::File
        } else if t.is_dir() {
            Kind::Dir
        } else {
            Kind::Other
        };
        Meta { kind, len: m.len() }
    }
}

/// One entry of a directory, as [`read_dir`] lists it.
#[derive(Clone, Debug)]
pub struct Entry {
    dir: PathBuf,
    name: OsString,
    kind: Kind,
}

impl Entry {
    pub fn path(&self) -> PathBuf {
        self.dir.join(&self.name)
    }

    pub fn file_name(&self) -> OsString {
        self.name.clone()
    }

    /// What the entry is, a symbolic link not followed.
    pub fn file_type(&self) -> io::Result<Meta> {
        Ok(Meta { kind: self.kind, len: 0 })
    }

    /// What the entry is, with its length (a symbolic link not followed, as `DirEntry::metadata`).
    pub fn metadata(&self) -> io::Result<Meta> {
        symlink_metadata(self.path())
    }
}

/// Files somewhere other than the disk. Paths come as the caller wrote them; a relative one is
/// read from [`Files::current_dir`].
pub trait Files {
    fn read(&self, p: &Path) -> io::Result<Vec<u8>>;
    fn metadata(&self, p: &Path) -> io::Result<Meta>;
    /// The names in a directory, each with what it is.
    fn read_dir(&self, p: &Path) -> io::Result<Vec<(OsString, Kind)>>;
    fn write(&self, p: &Path, bytes: &[u8]) -> io::Result<()>;
    fn create_dir_all(&self, p: &Path) -> io::Result<()>;
    /// The working directory, named with no symbolic link in it, as the system names it
    /// (`std::env::current_dir`): a `..` that climbs above it folds by its letters
    /// ([`crate::paths::folds_by_letters`]).
    fn current_dir(&self) -> io::Result<PathBuf>;
    /// The path made absolute, `.` and `..` folded; an error when nothing is there.
    fn canonicalize(&self, p: &Path) -> io::Result<PathBuf>;
    /// Where the symbolic link at `p` leads, as the link is written; None where `p` is no link or
    /// nothing is there. Files of their own hold no link, the default (memory, a git revision).
    fn link(&self, _p: &Path) -> Option<PathBuf> {
        None
    }
    /// The git revision the files are read at ([`GitRevision`]), for a sentence that says what to
    /// run (`ritsu mod fetch --rev <revision>`): None for the disk and memory. A [`Files`] that hands
    /// its calls on hands this on too.
    fn revision(&self) -> Option<String> {
        None
    }
    /// The files a [`Files`] that only watches what is read stands for, as [`id`] numbers them:
    /// it hands every call on to them unchanged, so what is kept for them (the tree of a module,
    /// [`crate::modules::refer`]) is kept for it too. None, the default, for files of their own.
    fn stands_for(&self) -> Option<usize> {
        None
    }
}

thread_local! {
    static FILES: RefCell<Option<Rc<dyn Files>>> = const { RefCell::new(None) };
}

/// Run `f` with every function of this module reading and writing `files` on this thread, then
/// read what was read before (the disk, unless an outer `with` said otherwise).
pub fn with<R>(files: Rc<dyn Files>, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<Rc<dyn Files>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let before = self.0.take();
            FILES.with(|c| *c.borrow_mut() = before);
        }
    }
    let _restore = Restore(FILES.with(|c| c.borrow_mut().replace(files)));
    f()
}

fn held() -> Option<Rc<dyn Files>> {
    FILES.with(|c| c.borrow().clone())
}

/// The git revision this thread reads at ([`Files::revision`]): None for the disk and memory.
pub fn revision() -> Option<String> {
    held().and_then(|f| f.revision())
}

/// What this module reads and writes on this thread when it is not the disk: the files of the
/// innermost [`with`], or None for the disk. A [`Files`] that watches what is read (the ports of
/// koyomi and rulec keep a check until a file it read reads differently) hands every call on to it.
pub fn current() -> Option<Rc<dyn Files>> {
    held()
}

/// Which files this thread reads, as a number to keep what is read from them by: 0 for the disk,
/// else the files of the innermost [`with`], or the files they stand for ([`Files::stands_for`]).
pub fn id() -> usize {
    match held() {
        None => 0,
        Some(f) => f.stands_for().unwrap_or(Rc::as_ptr(&f) as *const () as usize),
    }
}

/// What a run of code read through this module, each question once with its answer ([`watched`]):
/// what a language keeps a check by, while every file the check read reads the same (DESIGN 4.15).
/// [`Seen::still`] asks every question again through the files of now, so a check kept and handed
/// out while another is being watched is read again by the one watched: what the kept check rests
/// on, the other rests on too. A file read is held by its bytes, in one copy that every check of the
/// thread that read the same bytes at the same path shares ([`copies_held`]): a check is kept for
/// the rest of the run, and a file many checks read (an `enums` file every rule imports) is held
/// once, not once for each of them.
#[derive(Clone, Default, Debug)]
pub struct Seen(Vec<Look>);

/// A question asked of the files, and the answer (None where there was nothing); a file's bytes in
/// the copy the checks of the thread share.
#[derive(Clone, Debug, PartialEq)]
enum Look {
    Read(PathBuf, Option<Arc<Vec<u8>>>),
    Meta(PathBuf, Option<Meta>),
    Dir(PathBuf, Option<Vec<(OsString, Kind)>>),
    Canonical(PathBuf, Option<PathBuf>),
    Link(PathBuf, Option<PathBuf>),
}

/// The kinds of question, as [`Look::question`] tells them apart.
const READ: u8 = 0;
const META: u8 = 1;
const DIR: u8 = 2;
const CANONICAL: u8 = 3;
const LINK: u8 = 4;

impl Look {
    /// The question, to ask it once.
    fn question(&self) -> (u8, &Path) {
        match self {
            Look::Read(p, _) => (READ, p),
            Look::Meta(p, _) => (META, p),
            Look::Dir(p, _) => (DIR, p),
            Look::Canonical(p, _) => (CANONICAL, p),
            Look::Link(p, _) => (LINK, p),
        }
    }

    /// Whether the files the thread reads now answer the question the same (the paths are where
    /// the view of a module's dependencies led, so they are asked as they are).
    fn holds(&self) -> bool {
        match self {
            Look::Read(p, was) => match (raw::read(p), was) {
                (Ok(now), Some(was)) => now == **was,
                (Err(_), None) => true,
                _ => false,
            },
            Look::Meta(p, was) => raw::metadata(p).ok() == *was,
            Look::Dir(p, was) => raw::read_dir(p).ok() == *was,
            Look::Canonical(p, was) => raw::canonicalize(p).ok() == *was,
            Look::Link(p, was) => raw::link(p) == *was,
        }
    }
}

thread_local! {
    /// The copies of the files read that the checks of this thread hold ([`Seen`]).
    static COPIES: RefCell<Copies> = RefCell::new(Copies::default());
}

#[derive(Default)]
struct Copies {
    /// For each path read from the files [`id`] numbers, the copies of what was read there that a
    /// check may still hold, pointed at without being held. The bytes are a `Vec` of their own, so
    /// that what points at a copy no check holds keeps the counts of the `Arc`, not the bytes.
    held: HashMap<(usize, PathBuf), Vec<Weak<Vec<u8>>>>,
    /// How many paths may be listed before those no check holds a copy of are let go.
    sweep_at: usize,
}

/// The copy of `bytes`, read at `p` from the files [`id`] numbers `files`: one a check of the thread
/// holds of the same bytes there, else a new one. A copy goes with the last check that held it.
fn copy(files: usize, p: &Path, bytes: &[u8]) -> Arc<Vec<u8>> {
    COPIES.with(|c| {
        let mut c = c.borrow_mut();
        let copies = c.held.entry((files, p.to_path_buf())).or_default();
        copies.retain(|w| w.strong_count() > 0);
        if let Some(held) = copies.iter().filter_map(Weak::upgrade).find(|b| b.as_slice() == bytes) {
            return held;
        }
        let new = Arc::new(bytes.to_vec());
        copies.push(Arc::downgrade(&new));
        if c.held.len() >= c.sweep_at {
            c.held.retain(|_, copies| {
                copies.retain(|w| w.strong_count() > 0);
                !copies.is_empty()
            });
            c.sweep_at = (c.held.len() * 2).max(64);
        }
        new
    })
}

/// How many copies of files read the checks of this thread hold now, and their bytes in all: one
/// for each file and what it held when read, however many checks read it.
pub fn copies_held() -> (usize, usize) {
    COPIES.with(|c| c.borrow().held.values().flatten().filter_map(Weak::upgrade).fold((0, 0), |(n, len), b| (n + 1, len + b.len())))
}

impl Seen {
    /// Whether every question asked reads the same now.
    pub fn still(&self) -> bool {
        self.0.iter().all(Look::holds)
    }

    /// Every file read, with what it held (None where nothing could be read), in the order read.
    pub fn reads(&self) -> impl Iterator<Item = (&Path, Option<&[u8]>)> {
        self.0.iter().filter_map(|l| if let Look::Read(p, b) = l { Some((p.as_path(), b.as_deref().map(Vec::as_slice))) } else { None })
    }

    /// What `other` asked, asked here too: an answer read off another check rests on what that
    /// check read.
    pub fn extend(&mut self, other: &Seen) {
        for l in &other.0 {
            if !self.0.iter().any(|m| m.question() == l.question()) {
                self.0.push(l.clone());
            }
        }
    }
}

/// The files a run of code reads through ([`watched`]): every call handed on to what the thread read
/// before (the disk, or the files of an outer [`with`]), each question noted once with its answer.
/// It stands for the files it hands on to ([`Files::stands_for`]), so what is kept for those (the
/// tree of a module) is read here too.
struct Recorder {
    under: Option<Rc<dyn Files>>,
    stands: usize,
    seen: RefCell<(Vec<Look>, BTreeSet<(u8, PathBuf)>)>,
}

impl Recorder {
    fn files(&self) -> &dyn Files {
        match &self.under {
            Some(f) => f.as_ref(),
            None => &Disk,
        }
    }

    /// Notes the question of the kind `kind` about `p` with its answer, the first time it is asked.
    fn note(&self, kind: u8, p: &Path, look: impl FnOnce() -> Look) {
        let mut s = self.seen.borrow_mut();
        if s.1.insert((kind, p.to_path_buf())) {
            s.0.push(look());
        }
    }
}

impl Files for Recorder {
    fn read(&self, p: &Path) -> io::Result<Vec<u8>> {
        let got = self.files().read(p);
        self.note(READ, p, || Look::Read(p.to_path_buf(), got.as_ref().ok().map(|b| copy(self.stands, p, b))));
        got
    }

    fn metadata(&self, p: &Path) -> io::Result<Meta> {
        let got = self.files().metadata(p);
        self.note(META, p, || Look::Meta(p.to_path_buf(), got.as_ref().ok().copied()));
        got
    }

    fn read_dir(&self, p: &Path) -> io::Result<Vec<(OsString, Kind)>> {
        let got = self.files().read_dir(p);
        self.note(DIR, p, || Look::Dir(p.to_path_buf(), got.as_ref().ok().cloned()));
        got
    }

    fn write(&self, p: &Path, bytes: &[u8]) -> io::Result<()> {
        self.files().write(p, bytes)
    }

    fn create_dir_all(&self, p: &Path) -> io::Result<()> {
        self.files().create_dir_all(p)
    }

    fn current_dir(&self) -> io::Result<PathBuf> {
        self.files().current_dir()
    }

    fn canonicalize(&self, p: &Path) -> io::Result<PathBuf> {
        let got = self.files().canonicalize(p);
        self.note(CANONICAL, p, || Look::Canonical(p.to_path_buf(), got.as_ref().ok().cloned()));
        got
    }

    fn link(&self, p: &Path) -> Option<PathBuf> {
        let got = self.files().link(p);
        self.note(LINK, p, || Look::Link(p.to_path_buf(), got.clone()));
        got
    }

    fn revision(&self) -> Option<String> {
        self.files().revision()
    }

    fn stands_for(&self) -> Option<usize> {
        Some(self.stands)
    }
}

thread_local! {
    /// What the disk answered within the [`steady`] this thread runs in: each question once.
    static STEADY: RefCell<Option<Steady>> = const { RefCell::new(None) };
}

/// The answers of the disk within a [`steady`], each as the disk gave it the first time it was asked.
#[derive(Default)]
struct Steady {
    read: HashMap<PathBuf, Result<Rc<Vec<u8>>, Said>>,
    meta: HashMap<PathBuf, Result<Meta, Said>>,
    link_meta: HashMap<PathBuf, Result<Meta, Said>>,
    dir: HashMap<PathBuf, Result<Vec<(OsString, Kind)>, Said>>,
    canonical: HashMap<PathBuf, Result<PathBuf, Said>>,
    link: HashMap<PathBuf, Option<PathBuf>>,
    cwd: Option<Result<PathBuf, Said>>,
}

/// An error of the disk, kept to be said again in the same words: an error of the system by its
/// number, any other by its kind and its text.
#[derive(Clone)]
struct Said {
    kind: io::ErrorKind,
    number: Option<i32>,
    text: String,
}

impl Said {
    fn of(e: &io::Error) -> Said {
        Said { kind: e.kind(), number: e.raw_os_error(), text: e.to_string() }
    }

    fn again(&self) -> io::Error {
        match self.number {
            Some(n) => io::Error::from_raw_os_error(n),
            None => io::Error::new(self.kind, self.text.clone()),
        }
    }
}

/// `f` run with the disk asked each question once (DESIGN 4.15): within it, the bytes of a file,
/// what is at a path (with a symbolic link followed and not), the names in a directory, a path made
/// canonical, where a link leads and the working directory are asked of the disk the first time,
/// and the same question again is answered as the disk answered then. A file or a directory this
/// module writes ([`write`], [`create_dir_all`]) has every question asked anew after it. What a
/// check kept rests on ([`Seen::still`]) is asked through it as well, so a run of code that asks
/// the ports many times (the walk of `dandori scenarios`) reads each file once, however many times
/// a port asks whether its check still holds; once `f` returns, the disk is asked again, and a check
/// whose files have changed is made anew. A `steady` inside another takes the outer one's answers.
/// Files of their own (memory, a git revision) are not the disk and are read as they are. The
/// working directory is asked once too, and a relative path is kept as it was given: code that
/// moves the working directory (`std::env::set_current_dir`), or writes files other than through
/// this module, runs outside a `steady`.
pub fn steady<R>(f: impl FnOnce() -> R) -> R {
    if STEADY.with(|s| s.borrow().is_some()) {
        return f();
    }
    struct End;
    impl Drop for End {
        fn drop(&mut self) {
            STEADY.with(|s| *s.borrow_mut() = None);
        }
    }
    STEADY.with(|s| *s.borrow_mut() = Some(Steady::default()));
    let _end = End;
    f()
}

/// The answer to a question of the disk: within a [`steady`], the one kept in `slot` for `p`, else
/// `ask`'s, kept there; outside, `ask`'s.
fn asked_once<T: Clone>(slot: fn(&mut Steady) -> &mut HashMap<PathBuf, Result<T, Said>>, p: &Path, ask: impl FnOnce() -> io::Result<T>) -> io::Result<T> {
    let kept = STEADY.with(|s| s.borrow_mut().as_mut().map(|s| slot(s).get(p).cloned()));
    match kept {
        None => ask(),
        Some(Some(answer)) => answer.map_err(|e| e.again()),
        Some(None) => {
            let got = ask();
            let keep = match &got {
                Ok(v) => Ok(v.clone()),
                Err(e) => Err(Said::of(e)),
            };
            STEADY.with(|s| {
                if let Some(s) = s.borrow_mut().as_mut() {
                    slot(s).insert(p.to_path_buf(), keep);
                }
            });
            got
        }
    }
}

/// The bytes of a file on the disk, asked once within a [`steady`].
fn disk_read(p: &Path) -> io::Result<Vec<u8>> {
    asked_once(|s| &mut s.read, p, || std::fs::read(p).map(Rc::new).map_err(as_native)).map(|b| b.as_ref().clone())
}

/// A file on the disk as text: within a [`steady`], its bytes as asked once; bytes that are not
/// UTF-8 are read again for `std`'s own error.
fn disk_read_to_string(p: &Path) -> io::Result<String> {
    if STEADY.with(|s| s.borrow().is_none()) {
        return std::fs::read_to_string(p).map_err(as_native);
    }
    String::from_utf8(disk_read(p)?).or_else(|_| std::fs::read_to_string(p).map_err(as_native))
}

/// What is at a path on the disk with a symbolic link not followed, asked once within a [`steady`].
fn disk_symlink_metadata(p: &Path) -> io::Result<Meta> {
    asked_once(|s| &mut s.link_meta, p, || std::fs::symlink_metadata(p).map(|m| Meta::of(&m)).map_err(as_native))
}

/// The names in a directory on the disk, each with what it is: asked once within a [`steady`] when
/// every entry could be read, else asked as it is each time.
fn entries(dir: &Path) -> io::Result<Vec<io::Result<(OsString, Kind)>>> {
    let kept = STEADY.with(|s| s.borrow_mut().as_mut().map(|s| s.dir.get(dir).cloned()));
    match kept {
        None => disk_entries(dir),
        Some(Some(answer)) => answer.map(|v| v.into_iter().map(Ok).collect()).map_err(|e| e.again()),
        Some(None) => {
            let got = disk_entries(dir);
            let keep = match &got {
                Ok(v) => v.iter().map(|e| e.as_ref().ok().cloned()).collect::<Option<Vec<_>>>().map(Ok),
                Err(e) => Some(Err(Said::of(e))),
            };
            if let Some(keep) = keep {
                STEADY.with(|s| {
                    if let Some(s) = s.borrow_mut().as_mut() {
                        s.dir.insert(dir.to_path_buf(), keep);
                    }
                });
            }
            got
        }
    }
}

/// What the disk answered before a write: asked anew after it.
fn forget_steady() {
    STEADY.with(|s| {
        if let Some(s) = s.borrow_mut().as_mut() {
            *s = Steady::default();
        }
    });
}

/// `f` run with every question it asks of this module noted: what it came to, and what it read.
pub fn watched<R>(f: impl FnOnce() -> R) -> (R, Seen) {
    let rec = Rc::new(Recorder { under: current(), stands: id(), seen: RefCell::default() });
    let got = with(rec.clone(), f);
    let (looks, _) = rec.seen.take();
    (got, Seen(looks))
}

/// What the layer reads without the view of a module's dependencies: the paths as they are. What
/// reads a module's own files to make the view (its `ritsu.mod`, its lock, the store) reads here.
pub mod raw {
    use super::{Disk, Files, Kind, Meta, disk_read_to_string, disk_symlink_metadata, entries, held};
    use std::ffi::OsString;
    use std::io;
    use std::path::{Path, PathBuf};

    /// `std::fs::read`.
    pub fn read(p: &Path) -> io::Result<Vec<u8>> {
        match held() {
            Some(f) => f.read(p),
            None => Disk.read(p),
        }
    }

    /// `std::fs::read_to_string`.
    pub fn read_to_string(p: &Path) -> io::Result<String> {
        match held() {
            Some(f) => String::from_utf8(f.read(p)?).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "stream did not contain valid UTF-8")),
            None => disk_read_to_string(p),
        }
    }

    /// `std::fs::metadata`: a symbolic link followed.
    pub fn metadata(p: &Path) -> io::Result<Meta> {
        match held() {
            Some(f) => f.metadata(p),
            None => Disk.metadata(p),
        }
    }

    /// `std::fs::symlink_metadata`: a symbolic link is [`Kind::Other`]. Memory holds none.
    pub fn symlink_metadata(p: &Path) -> io::Result<Meta> {
        match held() {
            Some(f) => f.metadata(p),
            None => disk_symlink_metadata(p),
        }
    }

    /// The names in a directory, each with what it is (a symbolic link not followed); the first
    /// error of an entry is the answer.
    pub fn read_dir(p: &Path) -> io::Result<Vec<(OsString, Kind)>> {
        match held() {
            Some(f) => f.read_dir(p),
            None => entries(p)?.into_iter().collect(),
        }
    }

    /// `std::fs::canonicalize`.
    pub fn canonicalize(p: &Path) -> io::Result<PathBuf> {
        match held() {
            Some(f) => f.canonicalize(p),
            None => Disk.canonicalize(p),
        }
    }

    /// [`super::link`].
    pub fn link(p: &Path) -> Option<PathBuf> {
        match held() {
            Some(f) => f.link(p),
            None => Disk.link(p),
        }
    }
}

/// Where a path is read: where the view of a module's dependencies says, when the path goes through
/// one; else the path.
fn seen(p: &Path) -> std::borrow::Cow<'_, Path> {
    match crate::modules::refer::map(p) {
        Some(r) => std::borrow::Cow::Owned(r),
        None => std::borrow::Cow::Borrowed(p),
    }
}

/// What writing under a dependency answers: its files are read only. A command says it in its own
/// words, in the language it prints in ([`crate::cli::cannot_write`]).
#[derive(Debug)]
pub struct ReadOnly(pub PathBuf);

impl std::fmt::Display for ReadOnly {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} is in a dependency, whose files are read only", self.0.display())
    }
}

impl std::error::Error for ReadOnly {}

fn read_only(p: &Path) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, ReadOnly(p.to_path_buf()))
}

/// `std::fs::read`.
pub fn read(p: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    raw::read(&seen(p.as_ref()))
}

/// `std::fs::read_to_string`.
pub fn read_to_string(p: impl AsRef<Path>) -> io::Result<String> {
    raw::read_to_string(&seen(p.as_ref()))
}

/// `std::fs::write`. Under a dependency, refused.
pub fn write(p: impl AsRef<Path>, bytes: impl AsRef<[u8]>) -> io::Result<()> {
    let p = p.as_ref();
    if crate::modules::refer::under_dependency(p) {
        return Err(read_only(p));
    }
    match held() {
        Some(f) => f.write(p, bytes.as_ref()),
        None => Disk.write(p, bytes.as_ref()),
    }
}

/// `std::fs::create_dir_all`. Under a dependency, refused.
pub fn create_dir_all(p: impl AsRef<Path>) -> io::Result<()> {
    let p = p.as_ref();
    if crate::modules::refer::under_dependency(p) {
        return Err(read_only(p));
    }
    match held() {
        Some(f) => f.create_dir_all(p),
        None => Disk.create_dir_all(p),
    }
}

/// `std::fs::metadata`: a symbolic link followed.
pub fn metadata(p: impl AsRef<Path>) -> io::Result<Meta> {
    raw::metadata(&seen(p.as_ref()))
}

/// `std::fs::symlink_metadata`: a symbolic link is [`Kind::Other`]. Memory holds none.
pub fn symlink_metadata(p: impl AsRef<Path>) -> io::Result<Meta> {
    raw::symlink_metadata(&seen(p.as_ref()))
}

/// `std::fs::read_dir`, in the order the directory gives (memory gives its names in order). The
/// entries of a dependency's directory have their paths through the view, as the directory was named.
pub fn read_dir(p: impl AsRef<Path>) -> io::Result<std::vec::IntoIter<io::Result<Entry>>> {
    let dir = p.as_ref().to_path_buf();
    let real = seen(&dir).into_owned();
    let listed: Vec<io::Result<Entry>> = match held() {
        Some(f) => f.read_dir(&real)?.into_iter().map(|(name, kind)| Ok(Entry { dir: dir.clone(), name, kind })).collect(),
        None => entries(&real)?.into_iter().map(|e| e.map(|(name, kind)| Entry { dir: dir.clone(), name, kind })).collect(),
    };
    Ok(listed.into_iter())
}

/// `std::fs::canonicalize`. In memory, the path made absolute with `.` and `..` folded. Built
/// for WASI, whose std has no `canonicalize` (it answers that it is not supported), [`realpath`].
/// Through a dependency, the path as the view writes it: the module's root made canonical, then
/// `@<name>` and the rest, once the file is there.
pub fn canonicalize(p: impl AsRef<Path>) -> io::Result<PathBuf> {
    let p = p.as_ref();
    match crate::modules::refer::view_parts(p) {
        Some((real, head, after)) => {
            raw::canonicalize(&real)?;
            Ok(raw::canonicalize(&head)?.join(after))
        }
        None => raw::canonicalize(p),
    }
}

/// Where the symbolic link at `p` leads, as the link is written (`std::fs::read_link`); None where
/// `p` is no link or nothing is there. Memory holds none, and a path through the view of a module's
/// dependencies is none either: the view is read by its letters.
pub fn link(p: impl AsRef<Path>) -> Option<PathBuf> {
    raw::link(p.as_ref())
}

/// `std::env::current_dir`.
pub fn current_dir() -> io::Result<PathBuf> {
    match held() {
        Some(f) => f.current_dir(),
        None => Disk.current_dir(),
    }
}

/// The disk, as the native binary reads it: what the functions of this module read when no
/// [`with`] holds other files, and what a [`Files`] that hands its calls on (koyomi's ports) hands
/// them to then. `std::fs`, with an error of the disk in the system's words ([`as_native`]);
/// built for WASI, a directory read whole ([`disk_entries`]) and `canonicalize` made by hand
/// ([`realpath`]).
#[derive(Clone, Copy, Debug, Default)]
pub struct Disk;

/// Within a [`steady`], each question is asked of the disk once.
impl Files for Disk {
    fn read(&self, p: &Path) -> io::Result<Vec<u8>> {
        disk_read(p)
    }

    fn metadata(&self, p: &Path) -> io::Result<Meta> {
        asked_once(|s| &mut s.meta, p, || std::fs::metadata(p).map(|m| Meta::of(&m)).map_err(as_native))
    }

    fn read_dir(&self, p: &Path) -> io::Result<Vec<(OsString, Kind)>> {
        entries(p)?.into_iter().collect()
    }

    fn write(&self, p: &Path, bytes: &[u8]) -> io::Result<()> {
        forget_steady();
        std::fs::write(p, bytes).map_err(as_native)
    }

    fn create_dir_all(&self, p: &Path) -> io::Result<()> {
        forget_steady();
        std::fs::create_dir_all(p).map_err(as_native)
    }

    fn current_dir(&self) -> io::Result<PathBuf> {
        let kept = STEADY.with(|s| s.borrow().as_ref().map(|s| s.cwd.clone()));
        match kept {
            None => std::env::current_dir().map_err(as_native),
            Some(Some(answer)) => answer.map_err(|e| e.again()),
            Some(None) => {
                let got = std::env::current_dir().map_err(as_native);
                let keep = got.as_ref().map(Clone::clone).map_err(Said::of);
                STEADY.with(|s| {
                    if let Some(s) = s.borrow_mut().as_mut() {
                        s.cwd = Some(keep);
                    }
                });
                got
            }
        }
    }

    fn canonicalize(&self, p: &Path) -> io::Result<PathBuf> {
        asked_once(|s| &mut s.canonical, p, || {
            #[cfg(target_os = "wasi")]
            return realpath(p);
            #[cfg(not(target_os = "wasi"))]
            std::fs::canonicalize(p)
        })
    }

    fn link(&self, p: &Path) -> Option<PathBuf> {
        let kept = STEADY.with(|s| s.borrow_mut().as_mut().map(|s| s.link.get(p).cloned()));
        match kept {
            None => std::fs::read_link(p).ok(),
            Some(Some(answer)) => answer,
            Some(None) => {
                let got = std::fs::read_link(p).ok();
                STEADY.with(|s| {
                    if let Some(s) = s.borrow_mut().as_mut() {
                        s.link.insert(p.to_path_buf(), got.clone());
                    }
                });
                got
            }
        }
    }
}

/// The entries of a directory on the disk, each with what it is (a symbolic link not followed),
/// in the order the directory gives.
#[cfg(not(target_os = "wasi"))]
fn disk_entries(dir: &Path) -> io::Result<Vec<io::Result<(OsString, Kind)>>> {
    Ok(std::fs::read_dir(dir)
        .map_err(as_native)?
        .map(|e| {
            let e = e.map_err(as_native)?;
            let t = e.file_type().map_err(as_native)?;
            Ok((e.file_name(), kind_of(t.is_file(), t.is_dir())))
        })
        .collect())
}

/// Built for WASI: every entry of the directory, read with one call of WASI's `fd_readdir` from
/// its start, into a buffer made larger until the whole directory fits. std reads a directory a
/// few entries at a time, each call going on from the cookie the entry before gave; Node's WASI
/// before uvwasi 0.0.23 (Node 23.11.0; older Node 22 too) gives macOS's `telldir` as that cookie
/// and opens the directory again for every call, where the cookie means nothing, and std reads
/// the first entries over and over: a directory of more than about 96 entries never ended, and
/// what was read missed some. A read from the start never seeks.
#[cfg(target_os = "wasi")]
fn disk_entries(dir: &Path) -> io::Result<Vec<io::Result<(OsString, Kind)>>> {
    use std::os::fd::AsRawFd;
    use std::os::wasi::ffi::OsStrExt;
    #[link(wasm_import_module = "wasi_snapshot_preview1")]
    unsafe extern "C" {
        fn fd_readdir(fd: i32, buf: *mut u8, buf_len: usize, cookie: u64, bufused: *mut usize) -> u16;
    }
    let f = std::fs::File::open(dir).map_err(as_native)?;
    let mut len = 64 * 1024;
    loop {
        let mut buf = vec![0u8; len];
        let mut used = 0usize;
        // SAFETY: the buffer is `len` bytes the call writes at most, and `used` is where it says how many
        let errno = unsafe { fd_readdir(f.as_raw_fd(), buf.as_mut_ptr(), len, 0, &mut used) };
        if errno != 0 {
            return Err(as_native(io::Error::from_raw_os_error(i32::from(errno))));
        }
        // a full buffer may hold the directory cut short: read it again into a larger one
        if used >= len {
            len *= 4;
            continue;
        }
        // each entry: d_next (u64), d_ino (u64), d_namlen (u32), d_type (u8), three bytes, the name
        let mut out = Vec::new();
        let mut at = 0;
        while at + 24 <= used {
            let namlen = u32::from_le_bytes([buf[at + 16], buf[at + 17], buf[at + 18], buf[at + 19]]) as usize;
            let kind = buf[at + 20];
            let (from, to) = (at + 24, at + 24 + namlen);
            if to > used {
                break;
            }
            at = to;
            let name = std::ffi::OsStr::from_bytes(&buf[from..to]);
            if name == "." || name == ".." {
                continue;
            }
            // 3 is a directory, 4 a regular file, 7 a symbolic link; 0 is a host that did not say
            let kind = match kind {
                3 => Kind::Dir,
                4 => Kind::File,
                0 => match std::fs::symlink_metadata(dir.join(name)) {
                    Ok(m) => kind_of(m.file_type().is_file(), m.file_type().is_dir()),
                    Err(e) => {
                        out.push(Err(as_native(e)));
                        continue;
                    }
                },
                _ => Kind::Other,
            };
            out.push(Ok((name.to_os_string(), kind)));
        }
        return Ok(out);
    }
}

fn kind_of(file: bool, dir: bool) -> Kind {
    if file {
        Kind::File
    } else if dir {
        Kind::Dir
    } else {
        Kind::Other
    }
}

/// The path `realpath(3)` answers, made by hand from what the disk says of each part: absolute
/// from the working directory; each part that is a symbolic link replaced by where it points (a
/// relative link read from the directory it is in), however many links in a row; `..` taken
/// after the part before it is followed, so that it leaves where the link led; an error when a
/// part is not there (`NotFound`), when a part with more after it is not a directory, or after
/// 40 links (a loop). What `canonicalize` is built for WASI, and the same answer as
/// `std::fs::canonicalize` everywhere (`tests/fs.rs` holds the two to each other natively).
///
/// Built for WASI, the walk starts from the deepest directory the host opened for the module that
/// holds the path (its preopens), taken as it is, and starts there again for a link that points
/// by an absolute path: a loader that opens a project's directory alone (the npm package's
/// `dirs`) leaves the directories above it out of the module's sight, and a walk from the root
/// would stop at the first of them. The npm package's loader opens each directory at its real
/// path, so nothing above it needs following, and at the path it was given as well when that goes
/// through a link; a path under the second is walked from the first, which is the same directory
/// (the same device and inode), so that it ends where `realpath(3)` ends.
pub fn realpath(p: &Path) -> io::Result<PathBuf> {
    let start = if crate::paths::rooted(p) { p.to_path_buf() } else { std::env::current_dir().map_err(as_native)?.join(p) };
    // what is left to walk, the next part last
    let mut todo: Vec<OsString> = Vec::new();
    let mut out = walked_from(&start, &mut todo);
    let mut links = 0;
    while let Some(part) = todo.pop() {
        if part == ".." {
            out.pop();
            continue;
        }
        let next = out.join(&part);
        let meta = std::fs::symlink_metadata(&next).map_err(as_native)?;
        if meta.file_type().is_symlink() {
            links += 1;
            if links > 40 {
                return Err(io::Error::other(format!("too many symbolic links in a row at {}", next.display())));
            }
            let to = std::fs::read_link(&next).map_err(as_native)?;
            if crate::paths::rooted(&to) {
                out = walked_from(&to, &mut todo);
            } else {
                push_parts(&mut todo, &to);
            }
        } else if !meta.is_dir() && !todo.is_empty() {
            return Err(not_a_directory());
        } else {
            out = next;
        }
    }
    Ok(out)
}

/// Where the walk of the absolute path `p` starts, with the parts of `p` after it pushed onto
/// `todo`: the deepest directory the host opened for the module that `p` lies under, by its parts
/// (the first opened of that directory, when it is opened twice); else, and natively, the root.
fn walked_from(p: &Path, todo: &mut Vec<OsString>) -> PathBuf {
    let above = opened().iter().filter(|(d, _)| d.as_os_str() != "/" && p.starts_with(d)).max_by_key(|(d, _)| d.components().count());
    match above {
        Some((d, first)) => {
            push_parts(todo, p.strip_prefix(d).unwrap_or(Path::new("")));
            first.clone()
        }
        None => {
            push_parts(todo, p);
            PathBuf::from("/")
        }
    }
}

/// The directories the host opened for the module (WASI's preopens), as `fd_prestat_get` and
/// `fd_prestat_dir_name` say them, from descriptor 3 on: what wasi-libc reads at the start too.
/// Each comes with the first of them that is the same directory (the same device and inode, from
/// `fd_filestat_get`), itself when no other is.
#[cfg(target_os = "wasi")]
fn opened() -> &'static [(PathBuf, PathBuf)] {
    use std::os::wasi::ffi::OsStrExt;
    static OPENED: std::sync::OnceLock<Vec<(PathBuf, PathBuf)>> = std::sync::OnceLock::new();
    #[link(wasm_import_module = "wasi_snapshot_preview1")]
    unsafe extern "C" {
        fn fd_prestat_get(fd: i32, prestat: *mut u32) -> u16;
        fn fd_prestat_dir_name(fd: i32, path: *mut u8, len: usize) -> u16;
        fn fd_filestat_get(fd: i32, filestat: *mut u64) -> u16;
    }
    OPENED.get_or_init(|| {
        let mut out: Vec<(PathBuf, Option<[u64; 2]>)> = Vec::new();
        for fd in 3..1024 {
            // a prestat is its tag (0, a directory) and the length of the directory's name
            let mut prestat = [0u32; 2];
            // SAFETY: the two words are the eight bytes a prestat takes
            if unsafe { fd_prestat_get(fd, prestat.as_mut_ptr()) } != 0 {
                break;
            }
            if prestat[0] & 0xff != 0 {
                continue;
            }
            let mut name = vec![0u8; prestat[1] as usize];
            // SAFETY: the buffer is the length the prestat gave
            if unsafe { fd_prestat_dir_name(fd, name.as_mut_ptr(), name.len()) } != 0 {
                continue;
            }
            while name.len() > 1 && name.last() == Some(&b'/') {
                name.pop();
            }
            let dir = PathBuf::from(std::ffi::OsStr::from_bytes(&name));
            if crate::paths::rooted(&dir) {
                // a filestat is eight words: the device and the inode first
                let mut stat = [0u64; 8];
                // SAFETY: the eight words are the 64 bytes a filestat takes
                let id = (unsafe { fd_filestat_get(fd, stat.as_mut_ptr()) } == 0).then_some([stat[0], stat[1]]);
                out.push((dir, id));
            }
        }
        out.iter()
            .map(|(dir, id)| {
                let first = id.and_then(|id| out.iter().find(|(_, other)| *other == Some(id))).map_or(dir, |(d, _)| d);
                (dir.clone(), first.clone())
            })
            .collect()
    })
}

#[cfg(not(target_os = "wasi"))]
fn opened() -> &'static [(PathBuf, PathBuf)] {
    &[]
}

/// The parts of `p` that name something (its root and its `.` left out), pushed onto `todo` so
/// that the first part is the last pushed.
fn push_parts(todo: &mut Vec<OsString>, p: &Path) {
    for c in p.components().rev() {
        match c {
            Component::ParentDir => todo.push(OsString::from("..")),
            Component::Normal(n) => todo.push(n.to_os_string()),
            Component::RootDir | Component::CurDir | Component::Prefix(_) => {}
        }
    }
}

/// An error of the disk as the native binary words it. Built for WASI, an error of the disk
/// carries WASI's number (`No such file or directory (os error 44)`) rather than the system's,
/// and a message that quotes one would read otherwise than the native binary's; the errors whose
/// words and number are one on Linux (musl and glibc) and on macOS take that number. Any other
/// error, and every error on any other target, is as it was.
pub fn as_native(e: io::Error) -> io::Error {
    #[cfg(target_os = "wasi")]
    if let Some(text) = e.raw_os_error().and_then(|n| renumbered(&e.to_string(), n)) {
        return io::Error::new(e.kind(), text);
    }
    e
}

/// `<words> (os error <n>)`, WASI's number `n` (wasi_snapshot_preview1's errno) given as the
/// system's, for the errors the systems number and word alike: EPERM, ENOENT, EBADF, EACCES,
/// EEXIST, ENOTDIR, EISDIR, EINVAL, ENOSPC and EROFS. None for any other.
pub fn renumbered(text: &str, wasi: i32) -> Option<String> {
    let native = match wasi {
        63 => 1,
        44 => 2,
        8 => 9,
        2 => 13,
        20 => 17,
        54 => 20,
        31 => 21,
        28 => 22,
        51 => 28,
        69 => 30,
        _ => return None,
    };
    let words = text.strip_suffix(&format!(" (os error {wasi})"))?;
    Some(format!("{words} (os error {native})"))
}



/// `Path::exists`.
pub fn exists(p: impl AsRef<Path>) -> bool {
    metadata(p).is_ok()
}

/// `Path::is_file`.
pub fn is_file(p: impl AsRef<Path>) -> bool {
    metadata(p).is_ok_and(|m| m.is_file())
}

/// `Path::is_dir`.
pub fn is_dir(p: impl AsRef<Path>) -> bool {
    metadata(p).is_ok_and(|m| m.is_dir())
}

/// The errors the disk gives, with its words, so that a message that quotes one reads the same
/// from memory.
fn not_found() -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, "No such file or directory (os error 2)")
}

fn is_a_directory() -> io::Error {
    io::Error::new(io::ErrorKind::IsADirectory, "Is a directory (os error 21)")
}

fn not_a_directory() -> io::Error {
    io::Error::new(io::ErrorKind::NotADirectory, "Not a directory (os error 20)")
}

/// Files held in memory, under a working directory: the files given, and the directories they
/// are in. What is written is held too, so a command that writes files (`gen --out`) leaves them
/// to be read back.
#[derive(Debug, Default)]
pub struct Memory {
    cwd: PathBuf,
    files: RefCell<BTreeMap<PathBuf, Vec<u8>>>,
    dirs: RefCell<BTreeSet<PathBuf>>,
    /// What was written through [`Files::write`], in the order it first was.
    written: RefCell<Vec<PathBuf>>,
}

impl Memory {
    /// No files, run in `cwd` (absolute).
    pub fn new(cwd: impl Into<PathBuf>) -> Memory {
        let cwd = fold(&cwd.into());
        let m = Memory { cwd: cwd.clone(), ..Memory::default() };
        m.mkdirs(&cwd);
        m
    }

    /// A file, at a path from the working directory (or absolute).
    pub fn add(&self, path: impl AsRef<Path>, bytes: impl Into<Vec<u8>>) {
        let p = self.abs(path.as_ref());
        if let Some(d) = p.parent() {
            self.mkdirs(d);
        }
        self.files.borrow_mut().insert(p, bytes.into());
    }

    /// Every file under a directory (from the working directory, or absolute), by its path from
    /// that directory with `/` between the parts, in path order.
    pub fn files_under(&self, dir: impl AsRef<Path>) -> Vec<(String, Vec<u8>)> {
        let d = self.abs(dir.as_ref());
        self.files
            .borrow()
            .iter()
            .filter_map(|(p, b)| {
                let rel = p.strip_prefix(&d).ok()?;
                let parts: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
                Some((parts.join("/"), b.clone()))
            })
            .collect()
    }

    /// What was written since the files were given, each once, in the order it first was, by its
    /// path from the working directory (`/` between the parts) with what it holds now.
    pub fn written(&self) -> Vec<(String, Vec<u8>)> {
        let files = self.files.borrow();
        self.written
            .borrow()
            .iter()
            .filter_map(|p| {
                let body = files.get(p)?.clone();
                let shown = match p.strip_prefix(&self.cwd) {
                    Ok(rel) => rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/"),
                    Err(_) => p.to_string_lossy().into_owned(),
                };
                Some((shown, body))
            })
            .collect()
    }

    fn abs(&self, p: &Path) -> PathBuf {
        if crate::paths::rooted(p) { fold(p) } else { fold(&self.cwd.join(p)) }
    }

    fn mkdirs(&self, d: &Path) {
        let mut dirs = self.dirs.borrow_mut();
        for a in d.ancestors() {
            dirs.insert(a.to_path_buf());
        }
    }
}

/// `.` and `..` folded by their letters (memory holds no symbolic link to follow).
fn fold(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

impl Files for Memory {
    fn read(&self, p: &Path) -> io::Result<Vec<u8>> {
        let a = self.abs(p);
        if let Some(b) = self.files.borrow().get(&a) {
            return Ok(b.clone());
        }
        Err(if self.dirs.borrow().contains(&a) { is_a_directory() } else { not_found() })
    }

    fn metadata(&self, p: &Path) -> io::Result<Meta> {
        let a = self.abs(p);
        if let Some(b) = self.files.borrow().get(&a) {
            return Ok(Meta { kind: Kind::File, len: b.len() as u64 });
        }
        if self.dirs.borrow().contains(&a) {
            return Ok(Meta { kind: Kind::Dir, len: 0 });
        }
        Err(not_found())
    }

    fn read_dir(&self, p: &Path) -> io::Result<Vec<(OsString, Kind)>> {
        let a = self.abs(p);
        if self.files.borrow().contains_key(&a) {
            return Err(not_a_directory());
        }
        if !self.dirs.borrow().contains(&a) {
            return Err(not_found());
        }
        let mut names: BTreeMap<OsString, Kind> = BTreeMap::new();
        for d in self.dirs.borrow().iter() {
            if d.parent() == Some(a.as_path())
                && let Some(n) = d.file_name()
            {
                names.insert(n.to_os_string(), Kind::Dir);
            }
        }
        for f in self.files.borrow().keys() {
            if f.parent() == Some(a.as_path())
                && let Some(n) = f.file_name()
            {
                names.insert(n.to_os_string(), Kind::File);
            }
        }
        Ok(names.into_iter().collect())
    }

    fn write(&self, p: &Path, bytes: &[u8]) -> io::Result<()> {
        let a = self.abs(p);
        if self.dirs.borrow().contains(&a) {
            return Err(is_a_directory());
        }
        match a.parent() {
            Some(d) if self.dirs.borrow().contains(d) => {}
            _ => return Err(not_found()),
        }
        if !self.written.borrow().contains(&a) {
            self.written.borrow_mut().push(a.clone());
        }
        self.files.borrow_mut().insert(a, bytes.to_vec());
        Ok(())
    }

    fn create_dir_all(&self, p: &Path) -> io::Result<()> {
        let a = self.abs(p);
        if a.ancestors().any(|x| self.files.borrow().contains_key(x)) {
            return Err(not_a_directory());
        }
        self.mkdirs(&a);
        Ok(())
    }

    fn current_dir(&self) -> io::Result<PathBuf> {
        Ok(self.cwd.clone())
    }

    fn canonicalize(&self, p: &Path) -> io::Result<PathBuf> {
        self.metadata(p)?;
        Ok(self.abs(p))
    }
}

// ── A git revision, and files laid over others ───────────────────────────

/// What is at a path of a git revision.
#[derive(Clone, Debug)]
enum AtRevision {
    /// A file: its object, and its length.
    File(String, u64),
    Dir,
    /// A symbolic link or a submodule, which a revision is not read through.
    Other,
}

/// The files of a git revision (DESIGN 17.9.1): a path in the repository is read as `git show
/// <revision>:<path>` reads it, and a path outside it (the store, a file of another repository) from
/// the disk. The repository is the one the directory it was opened in is in; its `.git` is read from
/// the disk, so that the search for a module's root stops where it does in the working tree. Nothing
/// is written. What is in the revision is listed once, when it is opened (`git ls-tree`), and a file
/// is read once (`git cat-file`).
#[derive(Debug)]
pub struct GitRevision {
    /// The top of the working tree, as git writes it, and as the directory it was opened in writes
    /// it when that differs (a path through a symbolic link).
    tops: Vec<PathBuf>,
    /// The revision as it was written, which the sentences say.
    rev: String,
    commit: String,
    /// Every path of the revision, from the top with `/` between the parts.
    entries: BTreeMap<String, AtRevision>,
    read: RefCell<BTreeMap<String, Vec<u8>>>,
}

/// The oldest git ritsu runs (DESIGN 17.12.1): `rev-parse --end-of-options`, with which a revision
/// is read here, came in 2.30, and `fetch --stdin` and `--no-write-fetch-head`, with which a
/// dependency is fetched from git, in 2.29.
pub const GIT_OLDEST: (u32, u32) = (2, 30);

/// The major and minor version in what `git version` printed (`git version 2.50.1 (Apple Git-155)`).
pub fn git_version(said: &str) -> Option<(u32, u32)> {
    let v = said.strip_prefix("git version ")?.split_whitespace().next()?;
    let mut parts = v.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok()?;
    Some((major, minor))
}

/// Whether what `git version` printed is a git as new as [`GIT_OLDEST`]; what does not read as a
/// version is not.
pub fn git_new_enough(said: &str) -> bool {
    git_version(said).is_some_and(|v| v >= GIT_OLDEST)
}

/// Err, with the sentence to say, when git cannot start or is older than [`GIT_OLDEST`].
fn git_checked() -> Result<(), crate::text::Text> {
    let o = match std::process::Command::new("git").arg("version").output() {
        Ok(o) => o,
        Err(e) => return Err(crate::cli::cannot_start("git", &e)),
    };
    let said = String::from_utf8_lossy(&o.stdout).trim().to_string();
    if git_new_enough(&said) {
        return Ok(());
    }
    let (a, b) = GIT_OLDEST;
    Err(crate::tr!("git が古すぎます（`{said}`）。git の版を読むには git {a}.{b} 以上が要ります", "git is too old (`{said}`): reading a git revision needs git {a}.{b} or later"))
}

/// Run git in `dir`: what it printed, or None when it ended with an error. Err when it cannot start.
fn git_in(dir: &Path, args: &[&str]) -> Result<Option<Vec<u8>>, crate::text::Text> {
    match std::process::Command::new("git").arg("-C").arg(dir).args(args).output() {
        Ok(o) => Ok(o.status.success().then_some(o.stdout)),
        Err(e) => Err(crate::cli::cannot_start("git", &e)),
    }
}

impl GitRevision {
    /// The revision `rev` of the repository the directory `dir` is in. Err, with the sentence to say,
    /// when git cannot start or is too old, `dir` is in no repository, or the revision is not one.
    pub fn open(dir: &Path, rev: &str) -> Result<GitRevision, crate::text::Text> {
        git_checked()?;
        // the directory as the run writes a place it looked at (DESIGN 6.2 item 8): from where it runs,
        // the shortest way (`.` for that directory itself)
        let shown = crate::paths::between(&crate::paths::absolute(Path::new(".")), &crate::paths::absolute(dir));
        let top = git_in(dir, &["rev-parse", "--show-toplevel"])?
            .ok_or_else(|| crate::tr!("`{shown}` は git のリポジトリの中にありません", "`{shown}` is in no git repository"))?;
        let top = PathBuf::from(String::from_utf8_lossy(&top).trim_end_matches(['\n', '\r']));
        let commit = git_in(&top, &["rev-parse", "--verify", "--quiet", "--end-of-options", &format!("{rev}^{{commit}}")])?
            .ok_or_else(|| crate::tr!("git の版 `{rev}` が見つかりません", "the git revision `{rev}` cannot be resolved"))?;
        let commit = String::from_utf8_lossy(&commit).trim().to_string();
        let listing = git_in(&top, &["ls-tree", "-r", "-t", "-z", "--long", &commit])?
            .ok_or_else(|| crate::tr!("git の版 `{rev}` のファイルを一覧できません", "cannot list the files of the git revision `{rev}`"))?;
        let mut entries = BTreeMap::new();
        for rec in listing.split(|b| *b == 0).filter(|r| !r.is_empty()) {
            // `<mode> <type> <object> <size>\t<path>`, the size `-` for a tree
            let Some(tab) = rec.iter().position(|b| *b == b'\t') else { continue };
            let head = String::from_utf8_lossy(&rec[..tab]).into_owned();
            let path = String::from_utf8_lossy(&rec[tab + 1..]).into_owned();
            let words: Vec<&str> = head.split_whitespace().collect();
            let at = match (words.first(), words.get(1), words.get(2), words.get(3)) {
                (Some(&"120000"), ..) => AtRevision::Other,
                (_, Some(&"blob"), Some(object), Some(len)) => AtRevision::File(object.to_string(), len.parse().unwrap_or(0)),
                (_, Some(&"tree"), ..) => AtRevision::Dir,
                _ => AtRevision::Other,
            };
            entries.insert(path, at);
        }
        let mut tops = vec![top.clone()];
        for other in [std::fs::canonicalize(&top).ok(), spelled_from(dir, &top)].into_iter().flatten() {
            if !tops.contains(&other) {
                tops.push(other);
            }
        }
        Ok(GitRevision { tops, rev: rev.to_string(), commit, entries, read: RefCell::new(BTreeMap::new()) })
    }

    /// The commit the revision is.
    pub fn commit(&self) -> &str {
        &self.commit
    }

    /// The path from the top of the repository, `/` between the parts, of a path in it; None for a
    /// path outside it, and for one in its `.git`.
    fn in_repo(&self, p: &Path) -> Option<String> {
        let abs = if crate::paths::rooted(p) { fold(p) } else { fold(&Disk.current_dir().unwrap_or_default().join(p)) };
        let rel = self.tops.iter().find_map(|t| abs.strip_prefix(t).ok().map(Path::to_path_buf)).or_else(|| {
            // a spelling of the top through a symbolic link: the deepest part of the path that is on the disk, made canonical
            let real = abs.ancestors().find_map(|a| std::fs::canonicalize(a).ok().map(|c| c.join(abs.strip_prefix(a).unwrap_or(Path::new("")))))?;
            self.tops.iter().find_map(|t| real.strip_prefix(t).ok().map(Path::to_path_buf))
        })?;
        let parts: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
        if parts.first().is_some_and(|f| f == ".git") {
            return None;
        }
        Some(parts.join("/"))
    }

    fn at(&self, rel: &str) -> Option<&AtRevision> {
        if rel.is_empty() {
            return Some(&AtRevision::Dir);
        }
        self.entries.get(rel)
    }

    fn read_only(&self, p: &Path) -> io::Error {
        io::Error::new(io::ErrorKind::PermissionDenied, format!("{} is read at the git revision {}, which is not written to", p.display(), self.rev))
    }
}

/// The top of the repository as the directory it was opened in writes it, when `dir` reaches the top
/// through a symbolic link (`/var` and `/private/var`): the part of `dir` above as many parts as the
/// canonical `dir` has below the top.
fn spelled_from(dir: &Path, top: &Path) -> Option<PathBuf> {
    let abs = if crate::paths::rooted(dir) { fold(dir) } else { fold(&std::env::current_dir().ok()?.join(dir)) };
    let below = std::fs::canonicalize(&abs).ok()?.strip_prefix(std::fs::canonicalize(top).ok()?).ok()?.components().count();
    let mut t = abs;
    for _ in 0..below {
        t.pop();
    }
    Some(t)
}

impl Files for GitRevision {
    fn read(&self, p: &Path) -> io::Result<Vec<u8>> {
        let Some(rel) = self.in_repo(p) else { return Disk.read(p) };
        match self.at(&rel) {
            Some(AtRevision::File(object, _)) => {
                if let Some(b) = self.read.borrow().get(&rel) {
                    return Ok(b.clone());
                }
                let top = &self.tops[0];
                let o = std::process::Command::new("git").arg("-C").arg(top).args(["cat-file", "blob", object]).output()?;
                if !o.status.success() {
                    return Err(not_found());
                }
                self.read.borrow_mut().insert(rel, o.stdout.clone());
                Ok(o.stdout)
            }
            Some(AtRevision::Dir) => Err(is_a_directory()),
            _ => Err(not_found()),
        }
    }

    fn metadata(&self, p: &Path) -> io::Result<Meta> {
        let Some(rel) = self.in_repo(p) else { return Disk.metadata(p) };
        match self.at(&rel) {
            Some(AtRevision::File(_, len)) => Ok(Meta { kind: Kind::File, len: *len }),
            Some(AtRevision::Dir) => Ok(Meta { kind: Kind::Dir, len: 0 }),
            Some(AtRevision::Other) => Ok(Meta { kind: Kind::Other, len: 0 }),
            None => Err(not_found()),
        }
    }

    fn read_dir(&self, p: &Path) -> io::Result<Vec<(OsString, Kind)>> {
        let Some(rel) = self.in_repo(p) else { return Disk.read_dir(p) };
        match self.at(&rel) {
            Some(AtRevision::Dir) => {}
            Some(_) => return Err(not_a_directory()),
            None => return Err(not_found()),
        }
        let start = if rel.is_empty() { String::new() } else { format!("{rel}/") };
        let mut out = Vec::new();
        for (path, at) in self.entries.range(start.clone()..) {
            let Some(name) = path.strip_prefix(&start) else { break };
            if name.contains('/') {
                continue;
            }
            let kind = match at {
                AtRevision::File(..) => Kind::File,
                AtRevision::Dir => Kind::Dir,
                AtRevision::Other => Kind::Other,
            };
            out.push((OsString::from(name), kind));
        }
        if rel.is_empty() {
            // the repository's own directory is there at every revision
            out.push((OsString::from(".git"), Kind::Dir));
            out.sort_by(|a, b| a.0.cmp(&b.0));
        }
        Ok(out)
    }

    fn write(&self, p: &Path, _bytes: &[u8]) -> io::Result<()> {
        Err(self.read_only(p))
    }

    fn create_dir_all(&self, p: &Path) -> io::Result<()> {
        Err(self.read_only(p))
    }

    fn current_dir(&self) -> io::Result<PathBuf> {
        Disk.current_dir()
    }

    fn canonicalize(&self, p: &Path) -> io::Result<PathBuf> {
        let Some(rel) = self.in_repo(p) else { return Disk.canonicalize(p) };
        self.at(&rel).ok_or_else(not_found)?;
        let top = std::fs::canonicalize(&self.tops[0]).unwrap_or_else(|_| self.tops[0].clone());
        Ok(if rel.is_empty() { top } else { top.join(rel) })
    }

    fn link(&self, p: &Path) -> Option<PathBuf> {
        match self.in_repo(p) {
            Some(_) => None,
            None => Disk.link(p),
        }
    }

    fn revision(&self) -> Option<String> {
        Some(self.rev.clone())
    }
}

/// Files laid over others: each file given is read at its path (absolute) in place of what is there,
/// and every other call is handed to the files under (the disk when none). What `@<name>@<version>/…`
/// reads through (DESIGN 17.9.1): the module at hand's `ritsu.mod` and `ritsu.lock` as they would be
/// if it required that version, and its other files as they are. Nothing is written.
///
/// A file may be laid in a directory the files under do not have (what ringi lowers a request to,
/// `expense.lowered/route.rule`): the directory is there while the overlay is, holding the files laid
/// in it, and a path that goes up out of it (`expense.lowered/../shared/office.cal`) is folded before
/// it is handed on, as the system would open it were the directory there (`paths::opened`, asking
/// the files under where a link leads): the `..` out of the directory by its letters, a `..` out of a
/// symbolic link from where the link leads (`in/expense.lowered/../../shared/office.cal`, with `in`
/// a link to `../common/in`, is `../common/shared/office.cal`). A path through directories the files
/// under have is handed on as it is.
pub struct Overlay {
    files: BTreeMap<PathBuf, Vec<u8>>,
    under: Option<Rc<dyn Files>>,
}

impl Overlay {
    /// No file laid over `under` (the disk when None).
    pub fn new(under: Option<Rc<dyn Files>>) -> Overlay {
        Overlay { files: BTreeMap::new(), under }
    }

    /// The file at `path` (absolute), read as `bytes`.
    pub fn put(&mut self, path: &Path, bytes: impl Into<Vec<u8>>) {
        self.files.insert(fold(path), bytes.into());
    }

    fn under(&self) -> &dyn Files {
        match &self.under {
            Some(f) => f.as_ref(),
            None => &Disk,
        }
    }

    fn laid(&self, p: &Path) -> Option<&Vec<u8>> {
        if self.files.is_empty() {
            return None;
        }
        self.files.get(&fold(&self.unfolded(p)?))
    }

    /// The path made absolute against the working directory of the files under, not folded.
    fn unfolded(&self, p: &Path) -> Option<PathBuf> {
        if crate::paths::rooted(p) { Some(p.to_path_buf()) } else { Some(self.under().current_dir().ok()?.join(p)) }
    }

    /// Whether `dir` (absolute, folded) is a directory only the files laid make: some file is laid
    /// under it, and the files under have nothing there.
    fn made(&self, dir: &Path) -> bool {
        self.files.keys().any(|f| f != dir && f.starts_with(dir)) && self.under().metadata(dir).is_err()
    }

    /// The path to hand to the files under: when it goes up out of a directory only the files laid
    /// make (which is no link), folded as the system would open it, asking the files under where a
    /// link leads; else as it is.
    fn handed(&self, p: &Path) -> PathBuf {
        if self.files.is_empty() {
            return p.to_path_buf();
        }
        let Some(abs) = self.unfolded(p) else { return p.to_path_buf() };
        let mut at = PathBuf::new();
        for c in abs.components() {
            if c == Component::ParentDir && self.made(&fold(&at)) {
                return crate::paths::opened_by(&abs, &|q| self.under().link(q));
            }
            at.push(c.as_os_str());
        }
        p.to_path_buf()
    }
}

impl Files for Overlay {
    fn read(&self, p: &Path) -> io::Result<Vec<u8>> {
        match self.laid(p) {
            Some(b) => Ok(b.clone()),
            None => self.under().read(&self.handed(p)),
        }
    }

    fn metadata(&self, p: &Path) -> io::Result<Meta> {
        if let Some(b) = self.laid(p) {
            return Ok(Meta { kind: Kind::File, len: b.len() as u64 });
        }
        let p = self.handed(p);
        match self.under().metadata(&p) {
            Err(e) => match self.unfolded(&p).map(|a| fold(&a)) {
                Some(abs) if self.made(&abs) => Ok(Meta { kind: Kind::Dir, len: 0 }),
                _ => Err(e),
            },
            ok => ok,
        }
    }

    fn read_dir(&self, p: &Path) -> io::Result<Vec<(OsString, Kind)>> {
        let p = self.handed(p);
        let abs = fold(&self.unfolded(&p).ok_or_else(not_found)?);
        let mut out = match self.under().read_dir(&p) {
            Ok(v) => v,
            Err(_) if self.made(&abs) => Vec::new(),
            Err(e) => return Err(e),
        };
        for f in self.files.keys() {
            // a file laid here, or the first directory on the way to one laid deeper
            let Ok(rest) = f.strip_prefix(&abs) else { continue };
            let mut parts = rest.components();
            let (Some(first), deeper) = (parts.next(), parts.next().is_some()) else { continue };
            let n = first.as_os_str();
            if !out.iter().any(|(x, _)| x == n) {
                out.push((n.to_os_string(), if deeper { Kind::Dir } else { Kind::File }));
            }
        }
        Ok(out)
    }

    fn write(&self, p: &Path, _bytes: &[u8]) -> io::Result<()> {
        Err(read_only(p))
    }

    fn create_dir_all(&self, p: &Path) -> io::Result<()> {
        Err(read_only(p))
    }

    fn current_dir(&self) -> io::Result<PathBuf> {
        self.under().current_dir()
    }

    fn canonicalize(&self, p: &Path) -> io::Result<PathBuf> {
        let p = self.handed(p);
        let abs = fold(&self.unfolded(&p).ok_or_else(not_found)?);
        if self.laid(&p).is_none() && !self.made(&abs) {
            return self.under().canonicalize(&p);
        }
        // the nearest directory above that the files under have, made canonical by them, and the
        // rest of the path as it is
        let mut rest = Vec::new();
        let mut at = abs.clone();
        loop {
            let Some(name) = at.file_name().map(|n| n.to_os_string()) else { return Ok(abs) };
            rest.push(name);
            if !at.pop() {
                return Ok(abs);
            }
            if !self.made(&at)
                && let Ok(top) = self.under().canonicalize(&at)
            {
                return Ok(rest.iter().rev().fold(top, |d, n| d.join(n)));
            }
        }
    }

    fn link(&self, p: &Path) -> Option<PathBuf> {
        match self.laid(p) {
            Some(_) => None,
            None => self.under().link(p),
        }
    }

    fn revision(&self) -> Option<String> {
        self.under().revision()
    }
}

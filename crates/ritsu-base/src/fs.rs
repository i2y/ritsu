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
//! either: it holds files and the directories they are in.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;

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
    fn current_dir(&self) -> io::Result<PathBuf>;
    /// The path made absolute, `.` and `..` folded; an error when nothing is there.
    fn canonicalize(&self, p: &Path) -> io::Result<PathBuf>;
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

/// What this module reads and writes on this thread when it is not the disk: the files of the
/// innermost [`with`], or None for the disk. A [`Files`] that watches what is read (koyomi's ports
/// keep a check until a file it read reads differently) hands every call on to it.
pub fn current() -> Option<Rc<dyn Files>> {
    held()
}

/// `std::fs::read`.
pub fn read(p: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    match held() {
        Some(f) => f.read(p.as_ref()),
        None => std::fs::read(p),
    }
}

/// `std::fs::read_to_string`.
pub fn read_to_string(p: impl AsRef<Path>) -> io::Result<String> {
    match held() {
        Some(f) => String::from_utf8(f.read(p.as_ref())?).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "stream did not contain valid UTF-8")),
        None => std::fs::read_to_string(p),
    }
}

/// `std::fs::write`.
pub fn write(p: impl AsRef<Path>, bytes: impl AsRef<[u8]>) -> io::Result<()> {
    match held() {
        Some(f) => f.write(p.as_ref(), bytes.as_ref()),
        None => std::fs::write(p, bytes),
    }
}

/// `std::fs::create_dir_all`.
pub fn create_dir_all(p: impl AsRef<Path>) -> io::Result<()> {
    match held() {
        Some(f) => f.create_dir_all(p.as_ref()),
        None => std::fs::create_dir_all(p),
    }
}

/// `std::fs::metadata`: a symbolic link followed.
pub fn metadata(p: impl AsRef<Path>) -> io::Result<Meta> {
    match held() {
        Some(f) => f.metadata(p.as_ref()),
        None => std::fs::metadata(p).map(|m| Meta::of(&m)),
    }
}

/// `std::fs::symlink_metadata`: a symbolic link is [`Kind::Other`]. Memory holds none.
pub fn symlink_metadata(p: impl AsRef<Path>) -> io::Result<Meta> {
    match held() {
        Some(f) => f.metadata(p.as_ref()),
        None => std::fs::symlink_metadata(p).map(|m| Meta::of(&m)),
    }
}

/// `std::fs::read_dir`, in the order the directory gives (memory gives its names in order).
pub fn read_dir(p: impl AsRef<Path>) -> io::Result<std::vec::IntoIter<io::Result<Entry>>> {
    let dir = p.as_ref().to_path_buf();
    let entries: Vec<io::Result<Entry>> = match held() {
        Some(f) => f.read_dir(&dir)?.into_iter().map(|(name, kind)| Ok(Entry { dir: dir.clone(), name, kind })).collect(),
        None => std::fs::read_dir(&dir)?
            .map(|e| {
                let e = e?;
                let t = e.file_type()?;
                let kind = if t.is_file() {
                    Kind::File
                } else if t.is_dir() {
                    Kind::Dir
                } else {
                    Kind::Other
                };
                Ok(Entry { dir: dir.clone(), name: e.file_name(), kind })
            })
            .collect(),
    };
    Ok(entries.into_iter())
}

/// `std::fs::canonicalize`. In memory, the path made absolute with `.` and `..` folded.
pub fn canonicalize(p: impl AsRef<Path>) -> io::Result<PathBuf> {
    match held() {
        Some(f) => f.canonicalize(p.as_ref()),
        None => std::fs::canonicalize(p),
    }
}

/// `std::env::current_dir`.
pub fn current_dir() -> io::Result<PathBuf> {
    match held() {
        Some(f) => f.current_dir(),
        None => std::env::current_dir(),
    }
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

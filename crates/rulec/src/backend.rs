//! The one place that knows which languages rulec generates (§15.20).
//!
//! Adding Ruby as the fifth target took 540 lines of generator and **22 hand-edited
//! files**, because the set of backends was written out again in seven separate lists —
//! `gen`'s output table, `runtest`'s five steps, `api`'s entries, and four test suites —
//! and then again in the diagrams and the prose. §15.13's "one `Lang` enum is enough" was
//! true only inside `guards()`.
//!
//! So the set lives here, once. Everything that has to walk it walks [`ALL`], and a sixth
//! language is a row in this file plus its own emitter. What cannot be expressed as data —
//! the emitter itself — stays in `codegen.rs`; what can is here.

use crate::codegen::{Gen, Lang};


/// The words each target will not take as an identifier, or will take only by hiding
/// something of its own (§15.103). Kept beside the registry that uses them.
mod words {
    // Two kinds of word, because the risk is not the same. A **keyword** cannot be an
    // identifier at all, so an alias that is one stops the compiler wherever the generated
    // code writes it. A **global** is a name the language already uses at the top of a
    // file, so only the aliases that become top-level identifiers — the rule's function and
    // an enum's type — can hide one.

    pub const PY_KW: &[&str] = &[
        "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class",
        "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global",
        "if", "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return",
        "try", "while", "with", "yield",
    ];
    pub const PY_GLOBAL: &[&str] = &[
        "abs", "all", "any", "bin", "bool", "bytes", "callable", "chr", "compile", "complex",
        "dict", "dir", "divmod", "enumerate", "eval", "exec", "filter", "float", "format",
        "frozenset", "getattr", "hash", "help", "hex", "id", "input", "int", "isinstance", "iter",
        "len", "list", "map", "max", "min", "next", "object", "oct", "open", "ord", "pow", "print",
        "property", "range", "repr", "reversed", "round", "set", "slice", "sorted", "str", "sum",
        "super", "tuple", "type", "vars", "zip",
    ];

    pub const JS_KW: &[&str] = &[
        "await", "break", "case", "catch", "class", "const", "continue", "debugger", "default",
        "delete", "do", "else", "enum", "export", "extends", "false", "finally", "for", "function",
        "if", "import", "in", "instanceof", "new", "null", "return", "super", "switch", "this",
        "throw", "true", "try", "typeof", "var", "void", "while", "with", "yield", "let", "static",
    ];
    pub const JS_GLOBAL: &[&str] = &[
        "Array", "BigInt", "Boolean", "Date", "Error", "JSON", "Map", "Math", "NaN", "Number",
        "Object", "Promise", "Set", "String", "Symbol", "console", "globalThis", "undefined",
        "interface", "namespace", "declare", "any", "unknown", "never", "readonly",
    ];

    pub const RS_KW: &[&str] = &[
        "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
        "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move",
        "mut", "pub", "ref", "return", "self", "static", "struct", "super", "trait", "true",
        "type", "unsafe", "use", "where", "while", "abstract", "become", "box", "do", "final",
        "gen", "macro", "override", "priv", "try", "typeof", "unsized", "virtual", "yield",
    ];
    /// The prelude types the generated Rust names itself. A variant is written qualified
    /// (`判定::Ok`), so the prelude's values are not here.
    pub const RS_GLOBAL: &[&str] = &["Option", "Result", "String", "Vec"];

    pub const RB_KW: &[&str] = &[
        "alias", "and", "begin", "break", "case", "class", "def", "do", "else", "elsif", "end",
        "ensure", "false", "for", "if", "in", "module", "next", "nil", "not", "or", "redo",
        "rescue", "retry", "return", "self", "super", "then", "true", "undef", "unless", "until",
        "when", "while", "yield",
    ];
    pub const RB_GLOBAL: &[&str] = &[
        "clone", "dup", "format", "freeze", "hash", "inspect", "lambda", "loop", "method",
        "object_id", "print", "proc", "puts", "raise", "require", "send", "tap", "to_s",
    ];
    /// The classes and modules Ruby defines at the top level, with the two libraries the
    /// generated code requires (`date` and `json`). The rule's module is its alias in
    /// PascalCase, and `module Time` stops with "Time is not a module".
    pub const RB_CORE: &[&str] = &[
        "ArgumentError", "Array", "BasicObject", "Binding", "Class", "ClosedQueueError",
        "Comparable", "Complex", "ConditionVariable", "Data", "Date", "DateTime", "DidYouMean",
        "Dir", "EOFError", "Encoding", "EncodingError", "Enumerable", "Enumerator", "Errno",
        "ErrorHighlight", "Exception", "FalseClass", "Fiber", "FiberError", "File", "FileTest",
        "Float", "FloatDomainError", "FrozenError", "GC", "Gem", "Hash", "IO", "IOError",
        "IndexError", "Integer", "Interrupt", "JSON", "Kernel", "KeyError", "LoadError",
        "LocalJumpError", "Marshal", "MatchData", "Math", "Method", "Module", "Monitor",
        "MonitorMixin", "Mutex", "NameError", "NilClass", "NoMatchingPatternError",
        "NoMatchingPatternKeyError", "NoMemoryError", "NoMethodError", "NotImplementedError",
        "Numeric", "Object", "ObjectSpace", "Pathname", "Proc", "Process", "Queue", "Ractor",
        "Random", "Range", "RangeError", "Rational", "RbConfig", "Refinement", "Regexp",
        "RegexpError", "Ruby", "RubyVM", "RuntimeError", "ScriptError", "SecurityError", "Set",
        "Signal", "SignalException", "SizedQueue", "StandardError", "StopIteration", "String",
        "Struct", "Symbol", "SyntaxError", "SyntaxSuggest", "SystemCallError", "SystemExit",
        "SystemStackError", "Thread", "ThreadError", "ThreadGroup", "Time", "TracePoint",
        "TrueClass", "TypeError", "UnboundMethod", "UncaughtThrowError", "UnicodeNormalize",
        "Warning", "ZeroDivisionError",
    ];

    /// PHP writes a variable with a `$`, so a keyword is only a problem where a bare name
    /// goes: the function this rule becomes.
    pub const PHP_KW: &[&str] = &[];
    pub const PHP_GLOBAL: &[&str] = &[
        "abstract", "and", "array", "as", "break", "callable", "case", "catch", "class", "clone",
        "const", "continue", "declare", "default", "die", "do", "echo", "else", "elseif", "empty",
        "enum", "exit", "extends", "final", "finally", "fn", "for", "foreach", "function",
        "global", "goto", "if", "implements", "include", "instanceof", "insteadof", "interface",
        "isset", "list", "match", "namespace", "new", "or", "print", "private", "protected",
        "public", "readonly", "require", "return", "static", "switch", "throw", "trait", "try",
        "unset", "use", "var", "while", "xor", "yield", "true", "false", "null",
        "count", "max", "min", "round", "sort",
    ];

    pub const GO_KW: &[&str] = &[
        "break", "case", "chan", "const", "continue", "default", "defer", "else", "fallthrough",
        "for", "func", "go", "goto", "if", "import", "interface", "map", "package", "range",
        "return", "select", "struct", "switch", "type", "var",
    ];
    pub const GO_GLOBAL: &[&str] = &[
        "any", "append", "bool", "byte", "cap", "clear", "close", "comparable", "complex", "copy",
        "delete", "error", "false", "float32", "float64", "imag", "int", "int16", "int32", "int64",
        "int8", "iota", "len", "make", "max", "min", "new", "nil", "panic", "print", "println",
        "real", "recover", "rune", "string", "true", "uint", "uint16", "uint32", "uint64", "uint8",
        "uintptr",
    ];

    pub const SWIFT_KW: &[&str] = &[
        "associatedtype", "class", "deinit", "enum", "extension", "fileprivate", "func", "import",
        "init", "inout", "internal", "let", "open", "operator", "private", "protocol", "public",
        "rethrows", "static", "struct", "subscript", "typealias", "var", "break", "case",
        "continue", "default", "defer", "do", "else", "fallthrough", "for", "guard", "if", "in",
        "repeat", "return", "switch", "where", "while", "as", "catch", "false", "is", "nil",
        "super", "self", "throw", "throws", "true", "try",
    ];
    pub const SWIFT_GLOBAL: &[&str] =
        &["Array", "Bool", "Double", "Error", "Int", "Int64", "Result", "Set", "String"];

    pub const JAVA_KW: &[&str] = &[
        "abstract", "assert", "boolean", "break", "byte", "case", "catch", "char", "class",
        "const", "continue", "default", "do", "double", "else", "enum", "extends", "final",
        "finally", "float", "for", "goto", "if", "implements", "import", "instanceof", "int",
        "interface", "long", "native", "new", "package", "private", "protected", "public",
        "return", "short", "static", "strictfp", "super", "switch", "synchronized", "this",
        "throw", "throws", "transient", "try", "void", "volatile", "while", "true", "false",
        "null",
    ];
    /// The classes of `java.lang` the generated code names, and the ones its files import: the
    /// rule's class is its alias in PascalCase, and a class `List` beside `import java.util.List`
    /// does not compile.
    pub const JAVA_GLOBAL: &[&str] = &[
        "ArrayList", "Boolean", "BufferedReader", "Character", "Double", "Error", "Exception",
        "IOException", "IllegalArgumentException", "InputStreamReader", "Integer", "LinkedHashMap",
        "List", "Long", "Map", "Math", "Number", "NumberFormatException", "Object", "PrintStream",
        "Record", "RuntimeException", "StandardCharsets", "String", "StringBuilder", "System",
        "Thread",
    ];

    /// Python's standard library, as `sys.stdlib_module_names` lists it (3.14, without the
    /// private modules), and the modules 3.12 and 3.13 removed. The rule's module is a file
    /// named after its alias, so `time.py` is what `import time` finds from beside it.
    pub const PY_STDLIB: &[&str] = &[
        "abc", "aifc", "annotationlib", "antigravity", "argparse", "array", "ast", "asynchat",
        "asyncio", "asyncore", "atexit", "audioop", "base64", "bdb", "binascii", "bisect",
        "builtins", "bz2", "calendar", "cgi", "cgitb", "chunk", "cmath", "cmd", "code",
        "codecs", "codeop", "collections", "colorsys", "compileall", "compression",
        "concurrent", "configparser", "contextlib", "contextvars", "copy", "copyreg",
        "cProfile", "crypt", "csv", "ctypes", "curses", "dataclasses", "datetime", "dbm",
        "decimal", "difflib", "dis", "distutils", "doctest", "email", "encodings", "ensurepip",
        "enum", "errno", "faulthandler", "fcntl", "filecmp", "fileinput", "fnmatch",
        "fractions", "ftplib", "functools", "gc", "genericpath", "getopt", "getpass",
        "gettext", "glob", "graphlib", "grp", "gzip", "hashlib", "heapq", "hmac", "html",
        "http", "idlelib", "imaplib", "imghdr", "imp", "importlib", "inspect", "io",
        "ipaddress", "itertools", "json", "keyword", "lib2to3", "linecache", "locale",
        "logging", "lzma", "mailbox", "mailcap", "marshal", "math", "mimetypes", "mmap",
        "modulefinder", "msilib", "msvcrt", "multiprocessing", "netrc", "nis", "nntplib", "nt",
        "ntpath", "nturl2path", "numbers", "opcode", "operator", "optparse", "os",
        "ossaudiodev", "pathlib", "pdb", "pickle", "pickletools", "pipes", "pkgutil",
        "platform", "plistlib", "poplib", "posix", "posixpath", "pprint", "profile", "pstats",
        "pty", "pwd", "py_compile", "pyclbr", "pydoc", "pydoc_data", "pyexpat", "queue",
        "quopri", "random", "re", "readline", "reprlib", "resource", "rlcompleter", "runpy",
        "sched", "secrets", "select", "selectors", "shelve", "shlex", "shutil", "signal",
        "site", "smtpd", "smtplib", "sndhdr", "socket", "socketserver", "spwd", "sqlite3",
        "sre_compile", "sre_constants", "sre_parse", "ssl", "stat", "statistics", "string",
        "stringprep", "struct", "subprocess", "sunau", "symtable", "sys", "sysconfig",
        "syslog", "tabnanny", "tarfile", "telnetlib", "tempfile", "termios", "textwrap",
        "this", "threading", "time", "timeit", "tkinter", "token", "tokenize", "tomllib",
        "trace", "traceback", "tracemalloc", "tty", "turtle", "turtledemo", "types", "typing",
        "unicodedata", "unittest", "urllib", "uu", "uuid", "venv", "warnings", "wave",
        "weakref", "webbrowser", "winreg", "winsound", "wsgiref", "xdrlib", "xml", "xmlrpc",
        "zipapp", "zipfile", "zipimport", "zlib", "zoneinfo",
    ];

    /// The first element of every standard package's import path (`go list std`). The rule's
    /// module is named after its alias, and a module `time` makes every `import "time"` —
    /// the standard library's own included — ambiguous.
    pub const GO_STD: &[&str] = &[
        "archive", "bufio", "builtin", "bytes", "cmp", "compress", "container", "context",
        "crypto", "database", "debug", "embed", "encoding", "errors", "expvar", "flag", "fmt",
        "go", "hash", "html", "image", "index", "internal", "io", "iter", "log", "maps",
        "math", "mime", "net", "os", "path", "plugin", "reflect", "regexp", "runtime",
        "slices", "sort", "strconv", "strings", "structs", "sync", "syscall", "testing",
        "text", "time", "unicode", "unique", "unsafe", "weak",
    ];

    /// SQL is the one target with nothing to list. The query quotes every identifier it
    /// writes — `"on"`, `"select"`, the function's own name and each argument it is called
    /// by name with — so a reserved word costs a reader of the relation a pair of quotes
    /// and costs the generated code nothing (§15.103).
    pub const NONE: &[&str] = &[];
}

/// What a backend has to say about itself for the shared machinery to drive it.
pub struct Backend {
    /// The stable identifier: the directory under the output, the key in `rulec api`, the
    /// name in `--format json`. Never translated, never renamed.
    pub id: &'static str,
    /// The name a person reads in a report.
    pub name: &'static str,
    /// The command that has to be on PATH to run what this backend writes. `rulec test`
    /// skips the language and says so when it is missing.
    pub tool: &'static str,
    /// Which arm of the generator emits it.
    pub lang: Lang,
    /// The files it writes, as (path under the output directory, contents).
    pub files: fn(&Gen, &str, &str) -> Vec<(String, String)>,
    /// What this backend names a rule's files, when that is not the alias. Java's public
    /// class has to be the name of the file it lives in, so its module is `ShippingFee.java`
    /// where the other ten write `shipping_fee.…`; `rulec test` finds what was written for a
    /// rule by this stem, and `None` means the alias itself.
    pub stem: Option<fn(&str) -> String>,
    /// How to run the generated runner over the vectors, relative to the output directory.
    pub run: fn(&str, &str) -> Plan,
    /// How to run the unit vectors of the rounding helpers.
    pub round: fn(&str) -> Plan,
    /// Whether this backend can write the walk a `fold` declares (§15.56). A rule that walks
    /// a sequence is generated only for the backends that say yes, and the others are
    /// refused by name — a generated file that cannot run is worse than a missing one.
    pub folds: bool,
    /// Whether this backend can write a table whose column is a `string` (§15.101). The
    /// test is a prefix, which every language but the columnar one does in one call; the
    /// backends that say no are refused by name for the same reason as a walk.
    pub texts: bool,
    /// How to run the same runner as a WASI module under wasmtime (§15.63), for the languages
    /// whose output compiles to `wasm32-wasip1` unchanged. `rulec test` runs it as a pass of its
    /// own when wasmtime and the target's standard library are installed.
    ///
    /// **The column is closed at one.** It exists to prove the *shape* — a rule reached as a
    /// command that reads stdin and writes stdout — and one language proving it is the whole
    /// claim. TinyGo, Javy or ruby.wasm here would add a toolchain to `rulec test` and widen
    /// nothing, so the `None`s elsewhere are a decision and not a list of things to do. Not to be
    /// confused with the `wasm` backend below, which is the other shape: a module that exports
    /// a function for a host to call (§15.64).
    pub wasi: Option<fn(&str, &str) -> Plan>,
    /// How to start the rule as an MCP server (§15.44), for the languages that get one.
    /// `rulec test` drives it over the vectors like the runner and holds its answers to the
    /// same expected records.
    pub mcp: Option<fn(&str) -> Plan>,
    /// How to reach the rule as a **Connect service** (§15.112), for the backends that get
    /// one: the stubs are built from the `.proto` and the vectors go through the generated
    /// client, so what is held to the reference evaluator is the answer that came back over
    /// a socket. `rulec test` runs it four times — the two applications the service is
    /// written as, ASGI and WSGI, each asked by POST and by GET. Both applications are
    /// generated, and a door nobody drove would be a claim nobody checked; both methods,
    /// because the rule declares itself free of side effects and may be called either way,
    /// and a carrying that changed an answer is the thing worth catching.
    ///
    /// The second argument is the `.proto`'s path under `proto/`, which carries the rule's
    /// version and so cannot be spelled from the alias alone.
    pub connect: Option<fn(&str, &str) -> Plan>,
    /// How to run the rule as a function **inside a database** (§15.80), for the backends
    /// that get one. The relation and the function are two doors on one query, and a door
    /// nobody drove would be a claim nobody checked, so `rulec test` runs this as a pass of
    /// its own whenever `psql` can reach a server. Without one it says so and skips, the way
    /// a missing toolchain does.
    pub pg: Option<fn(&str) -> Plan>,
    /// How to run the generated proof harnesses (§15.95), for the backends that have them.
    /// `rulec test --proofs` runs this as a pass of its own, and says it skipped when the
    /// tool is not on PATH, the way a missing toolchain is said. It is behind a flag because
    /// it is the one pass whose cost a person would notice — seconds where the vectors are
    /// milliseconds, and half a minute for a rule that walks fifty elements (§15.95).
    ///
    /// **The column is closed at one**, like `wasi`. What it proves — the artifact answers
    /// over the whole declared domain, no table falls through, no W114 guard fires, no i64
    /// overflows — is a property of the rule as lowered to *this* language, and one language
    /// carrying it is the whole claim. A second model checker here (JBMC for Java is the
    /// obvious one) would add a toolchain and widen nothing until a rule is found where the
    /// two disagree.
    pub proof: Option<fn(&str) -> Plan>,
    /// What else has to be there beyond `tool`, checked before the language is run; the Err is
    /// the note `rulec test` prints when it skips the language for that reason.
    pub ready: Option<fn() -> Result<(), String>>,
    /// The words this language will not let **any** identifier be — its keywords
    /// (§15.103). Every ASCII alias reaches at least one target verbatim: measured, a rule
    /// with an input aliased `type` generated Rust that reads `pub fn sum(type: i64, …)`
    /// and does not compile. Matching ignores case, because an alias arrives as
    /// `PascalCase` in some targets and `UPPER_CASE` in others.
    pub reserved: &'static [&'static str],
    /// The names that are already taken at the top of a file here — builtins, the standard
    /// library, the globals in scope everywhere. Only the two aliases that become top-level
    /// identifiers are held to this: the rule's, which is the function's name, and an
    /// enum's, which is a type's. A parameter or a local of the same name shadows nothing
    /// outside its own body, which is why `min` and `list` in the corpus are silent.
    pub globals: &'static [&'static str],
    /// The names the rule's own alias cannot take here, because the module or package named
    /// after it would collide with one of the standard library's (§15.149). Held to
    /// `module_of(alias)`, the name that module actually gets. Only the rule's alias names a
    /// module; an enum's becomes a type inside it.
    pub modules: &'static [&'static str],
    /// The name the rule's module or package gets from its alias here.
    pub module_of: fn(&str) -> String,
}

/// The name of the rule's Go package: its alias with the underscores taken out.
pub fn go_package(alias: &str) -> String {
    alias.replace('_', "").to_lowercase()
}

fn same_name(alias: &str) -> String {
    alias.to_string()
}

/// Whether a command answers `--version` or `version`.
pub fn have(cmd: &str) -> bool {
    ["--version", "version"]
        .iter()
        .any(|a| std::process::Command::new(cmd).arg(a).output().map(|o| o.status.success()).unwrap_or(false))
}

/// What the Connect side needs beyond `python3`: the compiler, the plugin that writes the
/// stubs, and the runtime the generated service imports. All three come from outside this
/// repository, so a machine without them skips the pass and is told which piece is missing —
/// the way a missing toolchain is said, not as a failure of the rule.
pub fn connect_ready() -> Result<(), String> {
    if !have("buf") {
        return Err(tr!("buf が無いので Connect 側を飛ばしました", "buf not found; skipped the Connect side"));
    }
    for plugin in ["protoc-gen-py", "protoc-gen-connectrpc"] {
        if !have(plugin) {
            return Err(tr!(
                "{plugin} が無いので Connect 側を飛ばしました（uv add --dev protoc-gen-py protoc-gen-connectrpc）",
                "{plugin} not found; skipped the Connect side (uv add --dev protoc-gen-py protoc-gen-connectrpc)"
            ));
        }
    }
    let rt = std::process::Command::new("python3")
        .args(["-c", "import connectrpc"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !rt {
        return Err(tr!(
            "connectrpc が入っていないので Connect 側を飛ばしました（uv add connectrpc）",
            "connectrpc is not installed; skipped the Connect side (uv add connectrpc)"
        ));
    }
    Ok(())
}

/// Whether an ASGI server is here to run the ASGI half on. connect-py's documentation names three;
/// uvicorn is the one the generated runner starts, because it can be handed a socket that is
/// already bound and so can say which port it took.
pub fn asgi_ready() -> Result<(), String> {
    let ok = std::process::Command::new("python3")
        .args(["-c", "import uvicorn"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if ok {
        Ok(())
    } else {
        Err(tr!(
            "uvicorn が無いので Connect の ASGI 側を飛ばしました（pip install uvicorn）",
            "uvicorn is not installed; skipped the ASGI half of the Connect side (pip install uvicorn)"
        ))
    }
}

/// Whether `psql` is here and answers from a server. Both halves matter: the client alone
/// cannot run a function, and libpq takes the connection from the environment (PGHOST,
/// PGDATABASE, PGUSER), so "is there a database" is a question only a query can answer.
pub fn psql_ready() -> bool {
    std::process::Command::new("psql")
        .args(["-X", "-q", "-A", "-t", "-c", "SELECT 1"])
        // libpq waits forever by default, and a `PGHOST` left pointing at something that is
        // not there would hang the whole run rather than skip one pass.
        .env("PGCONNECT_TIMEOUT", "5")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Whether the Rust toolchain can build for `target`: its standard library sits under the
/// sysroot once `rustup target add <target>` has been run.
pub fn rust_target(target: &str) -> bool {
    let Ok(o) = std::process::Command::new("rustc").args(["--print", "sysroot"]).output() else { return false };
    if !o.status.success() {
        return false;
    }
    let root = String::from_utf8_lossy(&o.stdout).trim().to_string();
    std::path::Path::new(&root).join("lib").join("rustlib").join(target).join("lib").is_dir()
}

/// One command to run, with an optional build that has to succeed first.
pub struct Plan {
    /// Directory under the output root.
    pub cwd: String,
    /// A compile step, for the languages that need one.
    pub build: Option<(String, Vec<String>)>,
    pub cmd: String,
    pub args: Vec<String>,
}

impl Plan {
    fn new(cwd: &str, cmd: &str, args: &[&str]) -> Self {
        Plan {
            cwd: cwd.into(),
            build: None,
            cmd: cmd.into(),
            args: args.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    fn built(mut self, cmd: &str, args: &[&str]) -> Self {
        self.build = Some((cmd.into(), args.iter().map(|s| (*s).to_string()).collect()));
        self
    }
}

/// How `rulec test` builds the Connect stubs: buf, with the plugins that `pip` put on the
/// PATH rather than the ones on the network (§15.112). `out` is relative to the output base,
/// which is the directory the command runs in — so `stubs` under `python/`, which is where
/// the generated service imports them from.
const LOCAL_STUBS: &str = r#"{"version":"v2","plugins":[{"local":"protoc-gen-py","out":"stubs","strategy":"all"},{"local":"protoc-gen-connectrpc","out":"stubs"}]}"#;

/// The `go` line of the generated `go.mod`.
///
/// Since Go 1.21 that line is not only a minimum version but a **toolchain switch**: with the
/// default `GOTOOLCHAIN=auto`, a `go` older than it downloads the named version and hands over.
/// In a sandbox with no network that fails, and `rulec test` reports the download error as the
/// generated code disagreeing with the reference evaluator.
///
/// So the line states what the output actually needs, and the output needs very little: `bufio`,
/// `encoding/json`, `fmt`, `os`, `time`, `testing`, no generics, no `slices` or `maps`, and
/// `min`/`max` written out by hand. 1.21 is the oldest release this was run against; asking for
/// more would only switch toolchains on somebody's machine for nothing.
const GO_MIN: &str = "1.21";

/// How the generated Java is compiled, in `rulec test` and in the `build` line of
/// `rulec api` alike, so that the two cannot part company (the same arrangement the two
/// Wasm shapes have).
pub const JAVAC_FLAGS: &[&str] = &["--release", "17", "-encoding", "UTF-8", "-d", "classes"];

/// Every language `rulec gen` writes, in the order they are reported.
pub const ALL: &[Backend] = &[
    Backend {
        id: "python",
        name: "Python",
        tool: "python3",
        lang: Lang::Py,
        files: |g, alias, _pkg| {
            vec![
                (format!("python/{alias}.py"), g.python()),
                (format!("python/{alias}_runner.py"), g.python_runner()),
                (format!("python/{alias}_mcp.py"), g.py_mcp()),
                // The rule behind a Connect endpoint, and the runner that holds it to the
                // same records as everything else (§15.112). The `.proto` they speak is one
                // file for every language and is written outside this registry.
                (format!("python/{alias}_service.py"), g.py_connect()),
                (format!("python/{alias}_connect_runner.py"), g.py_connect_runner()),
                ("python/_round_test.py".into(), crate::codegen::round_tests_python()),
                (format!("python/{alias}_page.html"), g.page()),
            ]
        },
        stem: None,
        // `-B` and a cleared cache: a `.pyc` counts as fresh when the source has the same
        // length and the same whole-second mtime, so a same-length edit within a second of
        // the last run would otherwise execute the old module and report it as ok.
        run: |alias, _| Plan::new("python", "python3", &["-B", &format!("{alias}_runner.py")]),
        round: |_| Plan::new("python", "python3", &["-B", "_round_test.py"]),
        folds: true,
        texts: true,
        wasi: None,
        mcp: Some(|alias| Plan::new("python", "python3", &["-B", &format!("{alias}_mcp.py")])),
        connect: Some(|alias, _proto| {
            // The stubs come from buf, which is the only way to reach protobuf-py's own
            // plugin. The template is given inline and names the **local** plugins, so a
            // machine with no network runs this pass like any other; the `buf.gen.yaml`
            // `gen` writes beside the `.proto` is the other one, the remote plugins that
            // connect-py's documentation recommends.
            Plan::new("python", "python3", &["-B", &format!("{alias}_connect_runner.py")]).built(
                "buf",
                &["generate", "../proto", "--template", LOCAL_STUBS],
            )
        }),
        pg: None,
        proof: None,
        ready: None,
        reserved: words::PY_KW,
        globals: words::PY_GLOBAL,
        modules: words::PY_STDLIB,
        module_of: same_name,
    },
    // The twelfth target is the one that is not a language: the rule travels as data and a
    // fixed evaluator reads it. It is here rather than in a package of its own because the
    // claim is the same claim — `rulec test` runs it over the vectors like the others, and a
    // plan that disagreed with the reference evaluator would be as red as generated code is.
    Backend {
        id: "numpy",
        name: "NumPy",
        tool: "python3",
        lang: Lang::Py,
        files: |g, alias, _pkg| {
            vec![
                (format!("numpy/{alias}.json"), g.np_plan()),
                ("numpy/rulec_np.py".into(), crate::codegen::np_runtime()),
                (format!("numpy/{alias}_runner.py"), g.np_runner()),
                ("numpy/_round_test.py".into(), crate::codegen::round_tests_numpy()),
            ]
        },
        stem: None,
        run: |alias, _| Plan::new("numpy", "python3", &["-B", &format!("{alias}_runner.py")]),
        round: |_| Plan::new("numpy", "python3", &["-B", "_round_test.py"]),
        // A walk carries state from element to element, which is not a column operation.
        folds: false,
        // A prefix over a column of strings is not one either: the runtime here reads
        // integer columns, and a text column would need a second representation.
        texts: false,
        wasi: None,
        mcp: None,
        connect: None,
        pg: None,
        proof: None,
        ready: Some(|| {
            let ok = std::process::Command::new("python3")
                .args(["-c", "import numpy"])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            if ok {
                Ok(())
            } else {
                Err(tr!("numpy が無いので NumPy 側を飛ばしました", "numpy not found; skipped the NumPy side"))
            }
        }),
        // The plan is JSON: a name here is data the runtime reads, never an identifier.
        reserved: words::NONE,
        globals: words::NONE,
        modules: words::NONE,
        module_of: same_name,
    },
    Backend {
        id: "typescript",
        name: "TypeScript",
        tool: "node",
        lang: Lang::Ts,
        files: |g, alias, _pkg| {
            vec![
                (format!("typescript/{alias}.ts"), g.typescript()),
                (format!("typescript/{alias}_runner.ts"), g.ts_runner()),
                (format!("typescript/{alias}_mcp.ts"), g.ts_mcp()),
                ("typescript/_round_test.ts".into(), crate::codegen::round_tests_typescript()),
                (format!("typescript/{alias}_page.html"), g.page()),
            ]
        },
        stem: None,
        run: |alias, _| {
            Plan::new("typescript", "node", &["--no-warnings", &format!("{alias}_runner.ts")])
        },
        round: |_| Plan::new("typescript", "node", &["--no-warnings", "_round_test.ts"]),
        folds: true,
        texts: true,
        wasi: None,
        mcp: Some(|alias| Plan::new("typescript", "node", &["--no-warnings", &format!("{alias}_mcp.ts")])),
        connect: None,
        pg: None,
        proof: None,
        ready: None,
        reserved: words::JS_KW,
        globals: words::JS_GLOBAL,
        modules: words::NONE,
        module_of: same_name,
    },
    Backend {
        id: "javascript",
        name: "JavaScript",
        tool: "node",
        lang: Lang::Ts,
        // The TypeScript with its types taken off (§15.36). `.mjs`, so that node reads it
        // as a module without a package.json, and a browser takes it as it stands.
        files: |g, alias, _pkg| {
            vec![
                (format!("javascript/{alias}.mjs"), g.javascript()),
                (format!("javascript/{alias}_runner.mjs"), g.js_runner()),
                (format!("javascript/{alias}_mcp.mjs"), g.js_mcp()),
                ("javascript/_round_test.mjs".into(), crate::codegen::round_tests_javascript()),
                (format!("javascript/{alias}_page.html"), g.page()),
            ]
        },
        stem: None,
        run: |alias, _| Plan::new("javascript", "node", &[&format!("{alias}_runner.mjs")]),
        round: |_| Plan::new("javascript", "node", &["_round_test.mjs"]),
        folds: true,
        texts: true,
        wasi: None,
        mcp: Some(|alias| Plan::new("javascript", "node", &[&format!("{alias}_mcp.mjs")])),
        connect: None,
        pg: None,
        proof: None,
        ready: None,
        reserved: words::JS_KW,
        globals: words::JS_GLOBAL,
        modules: words::NONE,
        module_of: same_name,
    },
    Backend {
        id: "rust",
        name: "Rust",
        tool: "rustc",
        lang: Lang::Rs,
        files: |g, alias, _pkg| {
            vec![
                (format!("rust/{alias}.rs"), g.rust()),
                (format!("rust/{alias}_runner.rs"), g.rs_runner()),
                (format!("rust/{alias}_proof.rs"), g.rs_proof()),
                ("rust/_round_test.rs".into(), crate::codegen::round_tests_rust()),
            ]
        },
        stem: None,
        // rustc takes no dependencies and needs no project file, so one invocation builds
        // the rule and its runner together through a `#[path] mod`.
        run: |alias, _| {
            Plan::new("rust", &format!("./{alias}"), &[]).built(
                "rustc",
                &["--edition", "2021", "-O", &format!("{alias}_runner.rs"), "-o", alias],
            )
        },
        round: |_| {
            Plan::new("rust", "./_round_test", &[]).built(
                "rustc",
                &["--edition", "2021", "-O", "_round_test.rs", "-o", "_round_test"],
            )
        },
        folds: true,
        texts: true,
        // The same runner, compiled for WASI and run under wasmtime. Nothing in the generated
        // Rust is platform-specific, so the source is the one above, unchanged.
        wasi: Some(|alias, _| {
            let args = wasi_rustc(&format!("{alias}_runner.rs"), &format!("{alias}_runner.wasm"));
            let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            Plan::new("rust", "wasmtime", &[&format!("{alias}_runner.wasm")]).built("rustc", &refs)
        }),
        mcp: None,
        connect: None,
        pg: None,
        // `kani <alias>_proof.rs`: the harnesses are a crate of their own whose only item is
        // the rule, included by path, so nothing has to be built first.
        proof: Some(|alias| Plan::new("rust", "kani", &[&format!("{alias}_proof.rs")])),
        ready: None,
        reserved: words::RS_KW,
        globals: words::RS_GLOBAL,
        modules: words::NONE,
        module_of: same_name,
    },
    Backend {
        id: "ruby",
        name: "Ruby",
        tool: "ruby",
        lang: Lang::Rb,
        files: |g, alias, _pkg| {
            vec![
                (format!("ruby/{alias}.rb"), g.ruby()),
                (format!("ruby/{alias}_runner.rb"), g.ruby_runner()),
                // The signature goes under sig/, where steep looks by default.
                (format!("ruby/sig/{alias}.rbs"), g.rbs()),
                ("ruby/_round_test.rb".into(), crate::codegen::round_tests_ruby()),
            ]
        },
        stem: None,
        run: |alias, _| Plan::new("ruby", "ruby", &[&format!("{alias}_runner.rb")]),
        round: |_| Plan::new("ruby", "ruby", &["_round_test.rb"]),
        folds: true,
        texts: true,
        wasi: None,
        mcp: None,
        connect: None,
        pg: None,
        proof: None,
        ready: None,
        reserved: words::RB_KW,
        globals: words::RB_GLOBAL,
        modules: words::RB_CORE,
        module_of: crate::codegen::ruby_module,
    },
    Backend {
        id: "php",
        name: "PHP",
        tool: "php",
        lang: Lang::Php,
        files: |g, alias, _pkg| {
            vec![
                (format!("php/{alias}.php"), g.php()),
                (format!("php/{alias}_runner.php"), g.php_runner()),
                ("php/_round_test.php".into(), crate::codegen::round_tests_php()),
            ]
        },
        stem: None,
        // `-n` ignores the machine's php.ini, the way `-B` keeps Python off a stale cache:
        // what runs here must not depend on somebody's local settings. `ext/json` is compiled
        // into every 8.x build and cannot be switched off, so nothing is lost by it.
        run: |alias, _| Plan::new("php", "php", &["-n", &format!("{alias}_runner.php")]),
        round: |_| Plan::new("php", "php", &["-n", "_round_test.php"]),
        folds: true,
        texts: true,
        wasi: None,
        mcp: None,
        connect: None,
        pg: None,
        proof: None,
        ready: None,
        reserved: words::PHP_KW,
        globals: words::PHP_GLOBAL,
        modules: words::NONE,
        module_of: same_name,
    },
    Backend {
        id: "go",
        name: "Go",
        tool: "go",
        lang: Lang::Go,
        files: |g, alias, pkg| {
            vec![
                (format!("go/{pkg}/{alias}.go"), g.go()),
                (format!("go/{pkg}/go.mod"), format!("module {pkg}\n\ngo {GO_MIN}\n")),
                (format!("go/{pkg}runner/main.go"), g.go_runner()),
                (
                    format!("go/{pkg}runner/go.mod"),
                    format!("module {pkg}runner\n\ngo {GO_MIN}\n\nrequire {pkg} v0.0.0\n\nreplace {pkg} => ../{pkg}\n"),
                ),
                (format!("go/{pkg}/round_test.go"), crate::codegen::round_tests_go(pkg)),
            ]
        },
        stem: None,
        run: |_, pkg| Plan::new(&format!("go/{pkg}runner"), "go", &["run", "."]),
        round: |pkg| Plan::new(&format!("go/{pkg}"), "go", &["test", "./..."]),
        folds: true,
        texts: true,
        wasi: None,
        mcp: None,
        connect: None,
        pg: None,
        proof: None,
        ready: None,
        reserved: words::GO_KW,
        globals: words::GO_GLOBAL,
        modules: words::GO_STD,
        module_of: go_package,
    },
    Backend {
        id: "swift",
        name: "Swift",
        tool: "swiftc",
        lang: Lang::Sw,
        files: |g, alias, _pkg| {
            vec![
                (format!("swift/{alias}.swift"), g.swift()),
                (format!("swift/{alias}_runner.swift"), g.swift_runner()),
                ("swift/_round_test.swift".into(), crate::codegen::round_tests_swift()),
            ]
        },
        stem: None,
        // Two files, one module: top-level code is only allowed in `main.swift`, so the
        // runner carries `@main` instead and the pair compiles as it stands. `-Onone` is
        // deliberate — nothing here is measured for speed, and the optimizer is the slowest
        // part of a build whose only job is to answer whether the vectors agree.
        run: |alias, _| {
            Plan::new("swift", &format!("./{alias}"), &[]).built(
                "swiftc",
                &["-Onone", &format!("{alias}.swift"), &format!("{alias}_runner.swift"), "-o", alias],
            )
        },
        round: |_| {
            Plan::new("swift", "./_round_test", &[])
                .built("swiftc", &["-Onone", "_round_test.swift", "-o", "_round_test"])
        },
        folds: true,
        texts: true,
        wasi: None,
        mcp: None,
        connect: None,
        pg: None,
        proof: None,
        ready: None,
        reserved: words::SWIFT_KW,
        globals: words::SWIFT_GLOBAL,
        modules: words::NONE,
        module_of: same_name,
    },
    Backend {
        id: "java",
        name: "Java",
        // `javac` builds and `java` runs; `ready` asks for the second one, because a JDK is
        // what has to be there and a JRE alone would compile nothing.
        tool: "javac",
        lang: Lang::Java,
        // The public class has to be the file's name, so the file is named after the class
        // and not after the alias the other backends use.
        files: |g, _alias, _pkg| {
            let cls = g.java_class();
            vec![
                (format!("java/{cls}.java"), g.java()),
                (format!("java/{cls}Runner.java"), g.java_runner()),
                ("java/_RoundTest.java".into(), crate::codegen::round_tests_java()),
            ]
        },
        stem: Some(crate::codegen::java_class),
        // `--release 17` is the floor the output claims (§15.78) rather than whatever JDK is
        // installed, `-encoding UTF-8` is what a JDK before 18 needs to read a Japanese
        // identifier, and `-d classes` keeps the build's output out of the source directory.
        run: |alias, _| {
            let cls = crate::codegen::java_class(alias);
            Plan::new("java", "java", &["-cp", "classes", &format!("{cls}Runner")]).built(
                "javac",
                &JAVAC_FLAGS
                    .iter()
                    .copied()
                    .chain([format!("{cls}.java"), format!("{cls}Runner.java")].iter().map(|s| s.as_str()))
                    .collect::<Vec<&str>>(),
            )
        },
        round: |_| {
            Plan::new("java", "java", &["-cp", "classes", "_RoundTest"]).built(
                "javac",
                &JAVAC_FLAGS.iter().copied().chain(["_RoundTest.java"]).collect::<Vec<&str>>(),
            )
        },
        folds: true,
        texts: true,
        wasi: None,
        mcp: None,
        connect: None,
        pg: None,
        proof: None,
        ready: Some(|| {
            if have("java") {
                Ok(())
            } else {
                Err(tr!("java が無いので Java 側を飛ばしました", "java not found; skipped the Java side"))
            }
        }),
        reserved: words::JAVA_KW,
        globals: words::JAVA_GLOBAL,
        modules: words::NONE,
        module_of: same_name,
    },
    Backend {
        id: "sql",
        name: "SQL",
        // The query runs on PostgreSQL; the agreement check runs it on the SQLite that ships
        // inside python3, so that is the tool it needs (§15.46).
        tool: "python3",
        lang: Lang::Sql,
        files: |g, alias, _pkg| {
            vec![
                (format!("sql/{alias}.sql"), g.sql()),
                (format!("sql/{alias}_function.sql"), g.sql_function()),
                (format!("sql/{alias}_runner.py"), g.sql_runner()),
                (format!("sql/{alias}_function_runner.py"), g.sql_function_runner()),
                ("sql/_round_test.py".into(), crate::codegen::round_tests_sql()),
            ]
        },
        stem: None,
        run: |alias, _| Plan::new("sql", "python3", &["-B", &format!("{alias}_runner.py")]),
        round: |_| Plan::new("sql", "python3", &["-B", "_round_test.py"]),
        folds: false,
        // `LIKE 'ABC%'` is one call here too, so a text column is generated.
        texts: true,
        wasi: None,
        mcp: None,
        connect: None,
        proof: None,
        pg: Some(|alias| Plan::new("sql", "python3", &["-B", &format!("{alias}_function_runner.py")])),
        ready: None,
        reserved: words::NONE,
        globals: words::NONE,
        modules: words::NONE,
        module_of: same_name,
    },
    Backend {
        id: "wasm",
        name: "Wasm",
        tool: "rustc",
        lang: Lang::Rs,
        // The module is the Rust one; what differs is the door. `<alias>_wasm.rs` is the crate
        // root that exports the canonical ABI, `<alias>.wit` names the world, and the Node
        // runner drives the built module through that ABI (§15.64).
        files: |g, alias, _pkg| {
            vec![
                (format!("wasm/{alias}.rs"), g.rust()),
                (format!("wasm/{alias}_wasm.rs"), g.rs_wasm()),
                (format!("wasm/{alias}.wit"), g.wit()),
                (format!("wasm/{alias}_runner.mjs"), g.wasm_runner()),
                ("wasm/_round.rs".into(), crate::codegen::round_wasm_rust()),
                ("wasm/_round_test.mjs".into(), crate::codegen::round_tests_wasm_js()),
            ]
        },
        stem: None,
        run: |alias, _| {
            let args = wasm_rustc(&format!("{alias}_wasm.rs"), &format!("{alias}.wasm"));
            let refs: Vec<&str> = args.iter().map(String::as_str).collect();
            Plan::new("wasm", "node", &[&format!("{alias}_runner.mjs")]).built("rustc", &refs)
        },
        round: |_| {
            let args = wasm_rustc("_round.rs", "_round.wasm");
            let refs: Vec<&str> = args.iter().map(String::as_str).collect();
            Plan::new("wasm", "node", &["_round_test.mjs"]).built("rustc", &refs)
        },
        folds: true,
        texts: true,
        wasi: None,
        mcp: None,
        connect: None,
        pg: None,
        proof: None,
        ready: Some(|| {
            if !have("node") {
                return Err(tr!("node が無いので Wasm 側を飛ばしました", "node not found; skipped the Wasm side"));
            }
            if !rust_target("wasm32-unknown-unknown") {
                return Err(tr!(
                    "wasm32-unknown-unknown の標準ライブラリが無いので Wasm 側を飛ばしました（rustup target add wasm32-unknown-unknown）",
                    "the wasm32-unknown-unknown standard library is not installed; skipped the Wasm side (rustup target add wasm32-unknown-unknown)"
                ));
            }
            Ok(())
        }),
        reserved: words::RS_KW,
        globals: words::RS_GLOBAL,
        modules: words::NONE,
        module_of: same_name,
    },
];

/// The rustc invocation for the runner as a WASI command: the flags of
/// `codegen::WASI_RUSTC_FLAGS`, so `rulec test` and the `wasi` entry of `rulec api` cannot
/// build it two different ways.
fn wasi_rustc(src: &str, out: &str) -> Vec<String> {
    let mut v: Vec<String> = vec!["--edition".into(), "2021".into()];
    v.extend(crate::codegen::WASI_RUSTC_FLAGS.split(' ').map(|s| s.to_string()));
    v.extend([src.to_string(), "-o".to_string(), out.to_string()]);
    v
}

/// The rustc invocation for a Wasm module: the flags of `codegen::WASM_RUSTC_FLAGS`, the
/// source, the output.
fn wasm_rustc(src: &str, out: &str) -> Vec<String> {
    let mut v: Vec<String> = vec!["--edition".into(), "2021".into()];
    v.extend(crate::codegen::WASM_RUSTC_FLAGS.split(' ').map(|s| s.to_string()));
    v.extend([src.to_string(), "-o".to_string(), out.to_string()]);
    v
}

/// What this backend calls the files it wrote for `alias`.
pub fn stem(b: &Backend, alias: &str) -> String {
    b.stem.map(|f| f(alias)).unwrap_or_else(|| alias.to_string())
}

/// The backend with this id, for the places that address one by name.
pub fn by_id(id: &str) -> Option<&'static Backend> {
    ALL.iter().find(|b| b.id == id)
}

/// The ids, in order. `rulec api` keys its entries with these, the output directory is
/// named with them, and the documentation lists them in this order.
pub fn ids() -> Vec<&'static str> {
    ALL.iter().map(|b| b.id).collect()
}

/// The names as a person reads them, in the output language. Every sentence that lists the
/// targets takes them from here, so that adding one is still a row in this file (§15.21).
pub fn names() -> String {
    if crate::i18n::ja() {
        names_ja()
    } else {
        names_en()
    }
}

/// The names as a person reads them, joined the way the prose does: `A, B and C`.
pub fn names_en() -> String {
    let ns: Vec<&str> = ALL.iter().map(|b| b.name).collect();
    match ns.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
        _ => ns.join(""),
    }
}

/// The same for Japanese, which separates with a middle dot and needs no conjunction.
pub fn names_ja() -> String {
    ALL.iter().map(|b| b.name).collect::<Vec<_>>().join("・")
}

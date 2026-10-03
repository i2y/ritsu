//! The tables rulec and dandori hold the names of their generated code to, copied from
//! rulec's `src/backend.rs` and dandori's `src/temporal_py.rs`, `src/temporal_go.rs` and
//! `src/temporal.rs` (PLAN C.10), made of the standards' lists ([`crate::words`]) where they
//! are those lists and of what each tool adds where they are not. `tests/copies.rs` holds them
//! to the tools' own; C.11 has the tools read them from here.

use crate::words::{Words, go, python, typescript};

/// rulec's: for each target, its keywords (no name may be one; rulec matches them in any case),
/// the names already taken at the top of a file (the rule's function and an enum's type may
/// not be one), and the modules the rule's module may not be named after.
pub mod rulec {
    use super::*;

    pub struct Backend {
        pub id: &'static str,
        pub reserved: Words,
        pub globals: Words,
        pub modules: Words,
    }

    const NONE: &[&str] = &[];

    /// The reserved words of ECMAScript, and `let` and `static` of strict mode's.
    const JS_KW_MORE: &[&str] = &["let", "static"];

    pub const PY_GLOBAL: &[&str] = &[
        "abs", "all", "any", "bin", "bool", "bytes", "callable", "chr", "compile", "complex",
        "dict", "dir", "divmod", "enumerate", "eval", "exec", "filter", "float", "format",
        "frozenset", "getattr", "hash", "help", "hex", "id", "input", "int", "isinstance", "iter",
        "len", "list", "map", "max", "min", "next", "object", "oct", "open", "ord", "pow", "print",
        "property", "range", "repr", "reversed", "round", "set", "slice", "sorted", "str", "sum",
        "super", "tuple", "type", "vars", "zip",
    ];

    pub const JS_GLOBAL: &[&str] = &[
        "Array", "BigInt", "Boolean", "Date", "Error", "JSON", "Map", "Math", "NaN", "Number",
        "Object", "Promise", "Set", "String", "Symbol", "console", "globalThis", "undefined",
        "interface", "namespace", "declare", "any", "unknown", "never", "readonly",
    ];

    /// Rust's keywords as rulec lists them: without `Self`, which `self` covers when the case is not looked at.
    pub const RS_KW: &[&str] = &[
        "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
        "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move",
        "mut", "pub", "ref", "return", "self", "static", "struct", "super", "trait", "true",
        "type", "unsafe", "use", "where", "while", "abstract", "become", "box", "do", "final",
        "gen", "macro", "override", "priv", "try", "typeof", "unsized", "virtual", "yield",
    ];

    pub const RS_GLOBAL: &[&str] = &[
        "Option", "Result", "String", "Vec",
    ];

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

    pub const PHP_GLOBAL: &[&str] = &[
        "abstract", "and", "array", "as", "break", "callable", "case", "catch", "class", "clone",
        "const", "continue", "declare", "default", "die", "do", "echo", "else", "elseif", "empty",
        "enum", "exit", "extends", "final", "finally", "fn", "for", "foreach", "function",
        "global", "goto", "if", "implements", "include", "instanceof", "insteadof", "interface",
        "isset", "list", "match", "namespace", "new", "or", "print", "private", "protected",
        "public", "readonly", "require", "return", "static", "switch", "throw", "trait", "try",
        "unset", "use", "var", "while", "xor", "yield", "true", "false", "null", "count", "max",
        "min", "round", "sort",
    ];

    /// Go's predeclared identifiers but `complex64` and `complex128`.
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

    pub const SWIFT_GLOBAL: &[&str] = &[
        "Array", "Bool", "Double", "Error", "Int", "Int64", "Result", "Set", "String",
    ];

    pub const JAVA_KW: &[&str] = &[
        "abstract", "assert", "boolean", "break", "byte", "case", "catch", "char", "class",
        "const", "continue", "default", "do", "double", "else", "enum", "extends", "final",
        "finally", "float", "for", "goto", "if", "implements", "import", "instanceof", "int",
        "interface", "long", "native", "new", "package", "private", "protected", "public",
        "return", "short", "static", "strictfp", "super", "switch", "synchronized", "this",
        "throw", "throws", "transient", "try", "void", "volatile", "while", "true", "false",
        "null",
    ];

    pub const JAVA_GLOBAL: &[&str] = &[
        "ArrayList", "Boolean", "BufferedReader", "Character", "Double", "Error", "Exception",
        "IOException", "IllegalArgumentException", "InputStreamReader", "Integer", "LinkedHashMap",
        "List", "Long", "Map", "Math", "Number", "NumberFormatException", "Object", "PrintStream",
        "Record", "RuntimeException", "StandardCharsets", "String", "StringBuilder", "System",
        "Thread",
    ];

    pub const PY_STDLIB: &[&str] = &[
        "abc", "aifc", "annotationlib", "antigravity", "argparse", "array", "ast", "asynchat",
        "asyncio", "asyncore", "atexit", "audioop", "base64", "bdb", "binascii", "bisect",
        "builtins", "bz2", "calendar", "cgi", "cgitb", "chunk", "cmath", "cmd", "code", "codecs",
        "codeop", "collections", "colorsys", "compileall", "compression", "concurrent",
        "configparser", "contextlib", "contextvars", "copy", "copyreg", "cProfile", "crypt", "csv",
        "ctypes", "curses", "dataclasses", "datetime", "dbm", "decimal", "difflib", "dis",
        "distutils", "doctest", "email", "encodings", "ensurepip", "enum", "errno", "faulthandler",
        "fcntl", "filecmp", "fileinput", "fnmatch", "fractions", "ftplib", "functools", "gc",
        "genericpath", "getopt", "getpass", "gettext", "glob", "graphlib", "grp", "gzip",
        "hashlib", "heapq", "hmac", "html", "http", "idlelib", "imaplib", "imghdr", "imp",
        "importlib", "inspect", "io", "ipaddress", "itertools", "json", "keyword", "lib2to3",
        "linecache", "locale", "logging", "lzma", "mailbox", "mailcap", "marshal", "math",
        "mimetypes", "mmap", "modulefinder", "msilib", "msvcrt", "multiprocessing", "netrc", "nis",
        "nntplib", "nt", "ntpath", "nturl2path", "numbers", "opcode", "operator", "optparse", "os",
        "ossaudiodev", "pathlib", "pdb", "pickle", "pickletools", "pipes", "pkgutil", "platform",
        "plistlib", "poplib", "posix", "posixpath", "pprint", "profile", "pstats", "pty", "pwd",
        "py_compile", "pyclbr", "pydoc", "pydoc_data", "pyexpat", "queue", "quopri", "random",
        "re", "readline", "reprlib", "resource", "rlcompleter", "runpy", "sched", "secrets",
        "select", "selectors", "shelve", "shlex", "shutil", "signal", "site", "smtpd", "smtplib",
        "sndhdr", "socket", "socketserver", "spwd", "sqlite3", "sre_compile", "sre_constants",
        "sre_parse", "ssl", "stat", "statistics", "string", "stringprep", "struct", "subprocess",
        "sunau", "symtable", "sys", "sysconfig", "syslog", "tabnanny", "tarfile", "telnetlib",
        "tempfile", "termios", "textwrap", "this", "threading", "time", "timeit", "tkinter",
        "token", "tokenize", "tomllib", "trace", "traceback", "tracemalloc", "tty", "turtle",
        "turtledemo", "types", "typing", "unicodedata", "unittest", "urllib", "uu", "uuid", "venv",
        "warnings", "wave", "weakref", "webbrowser", "winreg", "winsound", "wsgiref", "xdrlib",
        "xml", "xmlrpc", "zipapp", "zipfile", "zipimport", "zlib", "zoneinfo",
    ];

    pub const GO_STD: &[&str] = &[
        "archive", "bufio", "builtin", "bytes", "cmp", "compress", "container", "context",
        "crypto", "database", "debug", "embed", "encoding", "errors", "expvar", "flag", "fmt",
        "go", "hash", "html", "image", "index", "internal", "io", "iter", "log", "maps", "math",
        "mime", "net", "os", "path", "plugin", "reflect", "regexp", "runtime", "slices", "sort",
        "strconv", "strings", "structs", "sync", "syscall", "testing", "text", "time", "unicode",
        "unique", "unsafe", "weak",
    ];

    pub const PHP_KW: &[&str] = &[];

    pub const BACKENDS: &[Backend] = &[
        Backend { id: "python", reserved: Words(&[python::KEYWORDS]), globals: Words(&[PY_GLOBAL]), modules: Words(&[PY_STDLIB]) },
        Backend { id: "numpy", reserved: Words(&[NONE]), globals: Words(&[NONE]), modules: Words(&[NONE]) },
        Backend { id: "typescript", reserved: Words(&[typescript::RESERVED, JS_KW_MORE]), globals: Words(&[JS_GLOBAL]), modules: Words(&[NONE]) },
        Backend { id: "javascript", reserved: Words(&[typescript::RESERVED, JS_KW_MORE]), globals: Words(&[JS_GLOBAL]), modules: Words(&[NONE]) },
        Backend { id: "rust", reserved: Words(&[RS_KW]), globals: Words(&[RS_GLOBAL]), modules: Words(&[NONE]) },
        Backend { id: "ruby", reserved: Words(&[RB_KW]), globals: Words(&[RB_GLOBAL]), modules: Words(&[RB_CORE]) },
        Backend { id: "php", reserved: Words(&[PHP_KW]), globals: Words(&[PHP_GLOBAL]), modules: Words(&[NONE]) },
        Backend { id: "go", reserved: Words(&[go::KEYWORDS]), globals: Words(&[GO_GLOBAL]), modules: Words(&[GO_STD]) },
        Backend { id: "swift", reserved: Words(&[SWIFT_KW]), globals: Words(&[SWIFT_GLOBAL]), modules: Words(&[NONE]) },
        Backend { id: "java", reserved: Words(&[JAVA_KW]), globals: Words(&[JAVA_GLOBAL]), modules: Words(&[NONE]) },
        Backend { id: "sql", reserved: Words(&[NONE]), globals: Words(&[NONE]), modules: Words(&[NONE]) },
        Backend { id: "wasm", reserved: Words(&[RS_KW]), globals: Words(&[RS_GLOBAL]), modules: Words(&[NONE]) },
    ];
}

/// dandori's: the names a flow's name gives way to in the generated Python, Go and TypeScript,
/// with a `_` after it — the language's words and the names the generated code uses itself.
pub mod dandori {
    use super::*;

    /// The builtins of Python the generated code would hide, and the names it uses.
    const PY_MORE: &[&str] = &[
        "abs", "all", "any", "bool", "bytes", "callable", "dict", "enumerate", "Exception",
        "filter", "float", "format", "frozenset", "getattr", "hasattr", "hash", "id", "input",
        "int", "isinstance", "iter", "len", "list", "map", "max", "min", "next", "object", "open",
        "print", "range", "repr", "reversed", "round", "set", "setattr", "slice", "sorted", "str",
        "sum", "super", "tuple", "vars", "zip", "self", "T", "dd", "io", "asyncio", "timedelta",
        "workflow", "activity", "json", "re", "Any", "Literal", "NotRequired", "Protocol",
        "TypedDict", "ApplicationError", "RetryPolicy", "NO_RETRY", "TIMESTAMP", "is_int", "rules",
        "workflows", "fail", "own", "make_activities", "WorkflowInput", "WorkflowOutput",
        "OwnTasks",
    ];

    /// `PY_RESERVED`: Python's keywords and soft keywords, and [`PY_MORE`].
    pub const PY_RESERVED: Words = Words(&[python::KEYWORDS, python::SOFT_KEYWORDS, PY_MORE]);

    /// The names the generated Go uses.
    const GO_MORE: &[&str] = &[
        "ctx", "input", "resume", "workflow", "time", "errors", "err", "out", "WorkflowType",
        "TaskQueue", "Events", "Workflow",
    ];

    /// `GO_RESERVED`: Go's keywords and predeclared identifiers, and [`GO_MORE`].
    pub const GO_RESERVED: Words = Words(&[go::KEYWORDS, go::PREDECLARED, GO_MORE]);

    /// `GO_EXPORTED`: the exported names every generated package has, which a record's, an
    /// enum's or a method's name gives way to.
    pub const GO_EXPORTED: &[&str] = &[
        "WorkflowType", "TaskQueue", "Events", "Workflow", "RegisterWorkflow", "OwnTasks",
        "OwnTasksBy", "Activities", "RegisterActivities", "BuildID", "WorkerOptions", "NewWorker",
        "History", "ReplayFailure", "Replay", "Start", "CallbackAnswer", "Answer", "Send", "Where",
        "Status", "Histories", "Outcome", "HTTPRequest", "HTTPResponse", "AgentCall", "Transport",
        "TransportOptions", "NewTransport", "DefaultTransport", "AgentHTTPError", "AgentStopped",
        "JevURL", "ClaudeMaxTokens", "WorkflowInput", "WorkflowOutput", "IsWorkflowInput",
        "Decode", "TIMESTAMP", "Service",
    ];

    /// `TS_GLOBALS`: the globals of JavaScript a type's name would hide, and the generated code's.
    pub const TS_GLOBALS: &[&str] = &[
        "Array", "Boolean", "Date", "Error", "Function", "JSON", "Map", "Math", "Number", "Object",
        "Promise", "RegExp", "Set", "String", "Symbol", "WorkflowInput", "WorkflowOutput",
    ];
}

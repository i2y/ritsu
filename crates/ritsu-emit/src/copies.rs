//! The tables rulec and dandori hold the names of their generated code to, copied from
//! rulec's `src/backend.rs` and dandori's `src/temporal_py.rs`, `src/temporal_go.rs` and
//! `src/temporal.rs` (PLAN C.10), made of the standards' lists ([`crate::words`]) where they
//! are those lists and of what each tool adds where they are not. Since C.11 the two tools read
//! them from here; `tests/copies.rs` holds them to the golden file that was written while they
//! were still compared with the tools' own, word for word. They are not the standards' lists
//! made whole (rulec has no `Self` and no `complex64`), because a word added changes what the
//! tools generate and refuse; whether to make them so is a question for the one package of
//! stage E.

use crate::words::{Words, go, python, typescript};

/// rulec's: for each target, its keywords (no name may be one; rulec matches them in any case),
/// the names already taken at the top of a file (the rule's function and an enum's type may
/// not be one), and the modules the rule's module may not be named after (rulec's §15.103,
/// §15.149).
///
/// Two kinds of word, because the risk is not the same. A **keyword** cannot be an identifier at
/// all, so an alias that is one stops the compiler wherever the generated code writes it. A
/// **global** is a name the language already uses at the top of a file, so only the aliases that
/// become top-level identifiers — the rule's function and an enum's type — can hide one.
pub mod rulec {
    use super::*;

    #[derive(Clone, Copy, Debug)]
    pub struct Backend {
        pub id: &'static str,
        pub reserved: Words,
        pub globals: Words,
        pub modules: Words,
    }

    /// SQL is the one target with nothing to list. The query quotes every identifier it writes —
    /// `"on"`, `"select"`, the function's own name and each argument it is called by name with —
    /// so a reserved word costs a reader of the relation a pair of quotes and costs the generated
    /// code nothing (rulec's §15.103).
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

    /// Rust's keywords as rulec lists them: without `Self`, which `self` covers when the case is
    /// not looked at.
    pub const RS_KW: &[&str] = &[
        "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
        "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move",
        "mut", "pub", "ref", "return", "self", "static", "struct", "super", "trait", "true",
        "type", "unsafe", "use", "where", "while", "abstract", "become", "box", "do", "final",
        "gen", "macro", "override", "priv", "try", "typeof", "unsized", "virtual", "yield",
    ];

    /// The prelude types the generated Rust names itself. A variant is written qualified
    /// (`判定::Ok`), so the prelude's values are not here.
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

    /// The classes and modules Ruby defines at the top level, with the two libraries the
    /// generated code requires (`date` and `json`). The rule's module is its alias in PascalCase,
    /// and `module Time` stops with "Time is not a module".
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
    /// private modules), and the modules 3.12 and 3.13 removed. The rule's module is a file named
    /// after its alias, so `time.py` is what `import time` finds from beside it.
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

    /// The first element of every standard package's import path (`go list std`). The rule's
    /// module is named after its alias, and a module `time` makes every `import "time"` — the
    /// standard library's own included — ambiguous.
    pub const GO_STD: &[&str] = &[
        "archive", "bufio", "builtin", "bytes", "cmp", "compress", "container", "context",
        "crypto", "database", "debug", "embed", "encoding", "errors", "expvar", "flag", "fmt",
        "go", "hash", "html", "image", "index", "internal", "io", "iter", "log", "maps", "math",
        "mime", "net", "os", "path", "plugin", "reflect", "regexp", "runtime", "slices", "sort",
        "strconv", "strings", "structs", "sync", "syscall", "testing", "text", "time", "unicode",
        "unique", "unsafe", "weak",
    ];

    /// PHP writes a variable with a `$`, so a keyword is only a problem where a bare name goes:
    /// the function this rule becomes, which [`PHP_GLOBAL`] holds.
    pub const PHP_KW: &[&str] = &[];

    pub const PYTHON: Backend = Backend { id: "python", reserved: Words(&[python::KEYWORDS]), globals: Words(&[PY_GLOBAL]), modules: Words(&[PY_STDLIB]) };
    pub const NUMPY: Backend = Backend { id: "numpy", reserved: Words(&[NONE]), globals: Words(&[NONE]), modules: Words(&[NONE]) };
    pub const TYPESCRIPT: Backend = Backend { id: "typescript", reserved: Words(&[typescript::RESERVED, JS_KW_MORE]), globals: Words(&[JS_GLOBAL]), modules: Words(&[NONE]) };
    pub const JAVASCRIPT: Backend = Backend { id: "javascript", reserved: Words(&[typescript::RESERVED, JS_KW_MORE]), globals: Words(&[JS_GLOBAL]), modules: Words(&[NONE]) };
    pub const RUST: Backend = Backend { id: "rust", reserved: Words(&[RS_KW]), globals: Words(&[RS_GLOBAL]), modules: Words(&[NONE]) };
    pub const RUBY: Backend = Backend { id: "ruby", reserved: Words(&[RB_KW]), globals: Words(&[RB_GLOBAL]), modules: Words(&[RB_CORE]) };
    pub const PHP: Backend = Backend { id: "php", reserved: Words(&[PHP_KW]), globals: Words(&[PHP_GLOBAL]), modules: Words(&[NONE]) };
    pub const GO: Backend = Backend { id: "go", reserved: Words(&[go::KEYWORDS]), globals: Words(&[GO_GLOBAL]), modules: Words(&[GO_STD]) };
    pub const SWIFT: Backend = Backend { id: "swift", reserved: Words(&[SWIFT_KW]), globals: Words(&[SWIFT_GLOBAL]), modules: Words(&[NONE]) };
    pub const JAVA: Backend = Backend { id: "java", reserved: Words(&[JAVA_KW]), globals: Words(&[JAVA_GLOBAL]), modules: Words(&[NONE]) };
    pub const SQL: Backend = Backend { id: "sql", reserved: Words(&[NONE]), globals: Words(&[NONE]), modules: Words(&[NONE]) };
    /// The module a host calls is written in Rust, so its words are Rust's.
    pub const WASM: Backend = Backend { id: "wasm", reserved: Words(&[RS_KW]), globals: Words(&[RS_GLOBAL]), modules: Words(&[NONE]) };

    /// The twelve, in the order of rulec's `backend::ALL`.
    pub const BACKENDS: &[Backend] = &[PYTHON, NUMPY, TYPESCRIPT, JAVASCRIPT, RUST, RUBY, PHP, GO, SWIFT, JAVA, SQL, WASM];
}

/// dandori's: the names the generated Python, Go and TypeScript keep for themselves — the language's
/// words, the builtins and the globals the code reads, and the names it uses itself. A type, a
/// task you implement or a message of one of these names is W105, which stops that target
/// (dandori's DESIGN 22); a variable, which no caller reads, gets a `_` after it.
pub mod dandori {
    use super::*;

    /// The builtins of Python the generated code reads, which a name of the flow would hide (no
    /// other: `print` or `format` hide nothing, and stop nothing), and the names it uses.
    /// dandori's `tests/spellings.rs` holds the builtins to what the code of every flow reads.
    const PY_MORE: &[&str] = &[
        "abs", "all", "any", "BaseException", "bool", "dict", "enumerate", "Exception", "float",
        "getattr", "id", "input", "int", "isinstance", "len", "list", "max", "min", "next", "range",
        "round", "RuntimeError", "set", "sorted", "str", "super", "TimeoutError", "tuple",
        "ValueError", "zip", "self", "T", "dd", "io", "asyncio", "timedelta", "workflow",
        "activity", "json", "re", "Any", "Literal", "NotRequired", "Protocol", "TypedDict",
        "ApplicationError", "RetryPolicy", "NO_RETRY", "TIMESTAMP", "is_int", "rules", "workflows",
        "fail", "own", "make_activities", "WorkflowInput", "WorkflowOutput", "OwnTasks",
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

    /// `GO_EXPORTED`: the exported names every generated package has. Go has one namespace for types
    /// and functions, and exports both in upper case, so a record's, an enum's or a method's name
    /// cannot be one of them (W105); so the package keeps out of the nouns a business's types go by
    /// (`CallOutcome`, `RunHistory`, `AnswerCallback`, `RunStatus`).
    pub const GO_EXPORTED: &[&str] = &[
        "WorkflowType", "TaskQueue", "Events", "Workflow", "RegisterWorkflow", "OwnTasks",
        "OwnTasksBy", "Activities", "RegisterActivities", "BuildID", "WorkerOptions", "NewWorker",
        "RunHistory", "ReplayFailure", "Replay", "Start", "CallbackAnswer", "AnswerCallback", "Send",
        "Where", "RunStatus", "Histories", "CallOutcome", "HTTPRequest", "HTTPResponse", "AgentCall",
        "Transport", "TransportOptions", "NewTransport", "DefaultTransport", "AgentHTTPError",
        "AgentStopped", "JevURL", "DecisionsURL", "ClaudeMaxTokens", "WorkflowInput", "WorkflowOutput",
        "IsWorkflowInput", "Decode", "TIMESTAMP", "Service",
    ];

    /// `TS_GLOBALS`: the names a type's name cannot be in `types.ts` (W105): the global types it
    /// reads, which a type of the name would hide, and what it declares for itself. A type hides no
    /// value (`Number.isInteger` still reads the global), and the other files read a type as `T.Date`.
    /// dandori's `tests/spellings.rs` holds the globals to what the code of every flow reads.
    pub const TS_GLOBALS: &[&str] = &["Array", "Record", "WorkflowInput", "WorkflowOutput"];
}

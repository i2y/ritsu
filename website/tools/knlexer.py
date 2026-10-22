"""Syntax highlighting for `.kn` sources on the site.

A ```kn fence is coloured at build time by Pygments, which every other code block on the site goes
through too: the theme's light and dark palettes are written against Pygments' class names, so the
colours follow the reader's scheme, and nothing runs in the browser.

Pygments finds a lexer by looking through its own table, so this module registers itself there on
import. zensical.toml names it as a Markdown extension so that something imports it; the extension
adds nothing to Markdown. build.sh puts this directory on PYTHONPATH.

What the colours say:

    keyword    the words a line starts with, the words of a function's body (`let`, `if`, `match`
               …), the bounds (`range`, `count`) and the effect (`total`, `fails`, `diverges`)
    type       the types (`number`, `money[…]`, `list[…]`, `dict[…]` …), and `key` after a type
               parameter
    constant   `true`, `false` and `none`
    class      the name a declaration gives (a module, a record, an enum, a function, a `use`)
    exception  the kinds of failure after `fails`
    builtin    a builtin called (`sum(…)`, `xs.map(…)`), and `ok` and `err` in a pattern
    attribute  the ASCII alias of a name (`請求書(invoice)`)
    number     a number with its unit, and a date
    string     a string
    operator   the operators, and the four reserved words written between values: `and`, `or`,
               `not`, `in`
    quiet      the punctuation: ( ) [ ] { } : , . .. ? |

A word is coloured by where it is written, as the parser reads it (DESIGN 2.2): `count` declares the
length of a list after a type and is the builtin that counts in a call; `total` is the effect after
a function's answer and a field anywhere else; `dict`, `set`, `entry`, `indexed` and `result` are
types only where `[` comes after them. A word before `:` is a field, a parameter or an argument, and
stays a name, whatever its spelling.

The vocabulary is crates/kaname/src/kw.rs's, and the characters of a number are
crates/kaname/src/lex.rs's. The lists below are a second copy of them in another language, which is
how a vocabulary rots, so crates/kaname/tests/docs.rs reads this file and holds each list to the one
it copies: a word added to the language and not here fails it.
"""

import re

from pygments.lexer import RegexLexer, bygroups, default, include, words
from pygments.token import (Comment, Keyword, Name, Number, Operator, Punctuation, String,
                            Whitespace)

__all__ = ["KanameLexer"]

# --- the vocabulary, from crates/kaname/src/kw.rs ----------------------------------------------

# The words, by where they are written: kw.rs's TABLE, word for word and in its order.
TABLE = (
    ("line", ("module", "description", "use", "private", "enum", "record", "fn", "examples")),
    ("use", ("rule", "dates", "book", "module", "from")),
    ("type", ("number", "bool", "string", "date", "rate", "money", "mass", "length", "area",
              "volume", "duration", "temperature", "sound", "list", "dict", "set", "entry",
              "indexed", "result", "running", "step", "incl_tax", "excl_tax", "range", "count")),
    ("type parameter", ("key",)),
    ("effect", ("total", "fails", "diverges", "rule", "date", "range", "overflow", "zero")),
    ("body", ("description", "let", "try", "if", "then", "else", "match", "some", "none", "true",
              "false", "and", "or", "not", "in")),
)

# The words a name cannot be (E002): kw.rs's RESERVED. Being no name, each is coloured wherever it
# is written.
RESERVED = ("let", "try", "if", "then", "else", "match", "some", "none", "true", "false", "and",
            "or", "not", "in")

# The kinds of failure written after `fails`: kw.rs's KINDS.
KINDS = ("rule", "date", "range", "overflow", "zero")

# The builtins written as functions, most of them after a value too: kw.rs's BUILTINS.
BUILTINS = ("min", "max", "mod", "isqrt", "allocate", "up", "down", "half_up", "half_down",
            "half_even", "count", "sum", "any", "all", "map", "filter", "first", "max_of", "min_of",
            "running_totals", "reduce", "starts_with", "concat", "text", "numbers", "char", "ok",
            "err")

# The builtins written only after a value: kw.rs's METHODS.
METHODS = ("get", "put", "remove", "has", "keys", "values", "entries", "map_values", "merge",
           "merge_with", "add", "union", "intersect", "difference", "to_list", "append", "at",
           "last", "slice", "take", "drop", "reverse", "sort", "sort_by", "sort_by_desc",
           "distinct", "contains", "flatten", "flat_map", "filter_map", "indexed", "max_by",
           "min_by", "to_set", "to_dict", "group_by", "join", "chars", "code_points", "to_text",
           "index_of", "ends_with", "split", "words", "replace", "trim", "trim_start", "trim_end",
           "lower_ascii", "upper_ascii", "pad_start", "pad_end", "parse_number", "parse_date")

# --- the characters of a number and a name, from crates/kaname/src/lex.rs ----------------------

# What a unit is written with after a number, beside the ASCII letters (`is_unit_char`): `3.49%`,
# `1000万円`, `-40℃`.
UNIT_CHARS = "円銭坪℃℉%％"
# The multipliers written between a number and its unit (`number`): `1000万円`.
MULTIPLIERS = "万億兆"
# The characters that are a word by themselves (`starts_name`): `temperature[℃]`.
WORD_CHARS = "℃℉"

# --- how each word is coloured ----------------------------------------------------------------

_PLACES = dict(TABLE)


def _of(place, chosen):
    """Words of a place of TABLE, checked to be there: a word misspelt here fails the build."""
    missing = [w for w in chosen if w not in _PLACES[place]]
    assert not missing, f"knlexer.py: {missing} are not words of the place {place!r} of TABLE"
    return tuple(chosen)


# The words the patterns below write out, each where TABLE has it.
_of("line", ("module", "description", "use", "private", "enum", "record", "fn", "examples"))
_of("use", ("from",))
_of("effect", ("fails",))
# What a `use` reads: the word of the first line of the file, then `from`.
USE_KINDS = _of("use", ("rule", "dates", "book", "module"))
# The types written alone, as `number` and `rate`; the others of the place take `[` after them
# (`money[USDc]`, `list[Line]`, `dict[string, number]`), and are types only there.
BARE_TYPES = _of("type", ("number", "bool", "string", "date", "rate"))
TAX = _of("type", ("incl_tax", "excl_tax"))
BOUNDS = _of("type", ("range", "count"))
STEP = _of("type", ("step",))
BRACKETED_TYPES = tuple(w for w in _PLACES["type"] if w not in BARE_TYPES + TAX + BOUNDS + STEP)
TYPE_PARAMETER = _of("type parameter", ("key",))
EFFECTS = tuple(w for w in _PLACES["effect"] if w not in KINDS)
assert all(w in _PLACES["effect"] for w in KINDS), "knlexer.py: KINDS are not words of the effect"
# The reserved words, by what they are: constants, the words written between values, and the rest.
CONSTANTS = tuple(w for w in RESERVED if w in ("true", "false", "none"))
OPERATOR_WORDS = tuple(w for w in RESERVED if w in ("and", "or", "not", "in"))
RESERVED_KEYWORDS = tuple(w for w in RESERVED if w not in CONSTANTS + OPERATOR_WORDS)
assert all(w in _PLACES["body"] for w in RESERVED), "knlexer.py: RESERVED are not words of the body"

# --- the patterns ------------------------------------------------------------------------------

_DIGIT = "[0-9０-９]"
_DIGITS = rf"{_DIGIT}(?:_?{_DIGIT})*"
_UNIT = rf"[A-Za-z{UNIT_CHARS}][A-Za-z0-9{UNIT_CHARS}]*"
NUMBER = rf"{_DIGITS}(?:\.{_DIGITS})?[{MULTIPLIERS}]?(?:{_UNIT})?"
DATE = rf"{_DIGIT}{{4}}-{_DIGIT}{{2}}-{_DIGIT}{{2}}(?!\w)"
# A name: letters of any script, digits and `_`, not starting with a digit.
NAME = r"[^\W\d]\w*"
ALIAS = r"[A-Za-z_][A-Za-z0-9_]*"
_WS = r"[^\S\n]+"
_COMPARE = r"(?:<=|>=|<|>|≦|≧)"


def _alt(ws):
    """The words as alternatives of a pattern, the longest first."""
    return "|".join(re.escape(w) for w in sorted(ws, key=len, reverse=True))


class KanameLexer(RegexLexer):
    """kaname, the language of a Functional Core."""

    name = "kaname"
    aliases = ["kn", "kaname"]
    filenames = ["*.kn"]

    _function = rf"^(?:(private)({_WS}))?(fn)({_WS})({NAME})(?:(\()({ALIAS})(\)))?"
    _record = rf"^(?:(private)({_WS}))?(record)({_WS})({NAME})(?:(\()({ALIAS})(\)))?"
    _enum = rf"^(?:(private)({_WS}))?(enum)({_WS})({NAME})(?:(\()({ALIAS})(\)))?"
    _head = (Keyword, Whitespace, Keyword, Whitespace)
    _alias = (Punctuation, Name.Attribute, Punctuation)

    tokens = {
        "root": [
            # The first line of a function's body may be its description. Tried before the
            # whitespace rule, which would otherwise eat the indentation.
            (rf"^([^\S\n]*)(description)\b(?={_WS}\")", bygroups(Whitespace, Keyword)),
            (r"[^\S\n]+", Whitespace),
            (r"\n", Whitespace),
            (r"#[^\n]*", Comment.Single),
            # A string has no escape: everything up to the next `"`.
            (r'"[^"\n]*"', String.Double),
            # The lines of a module, at the left margin, and the names they declare.
            (rf"^(module)({_WS})({NAME})(?:(\()({ALIAS})(\)))?(?:({_WS})(v\d+))?",
             bygroups(Keyword, Whitespace, Name.Namespace, *_alias, Whitespace, Name.Constant)),
            (rf"^(use)({_WS})({_alt(USE_KINDS)})({_WS})({NAME})({_WS})(from)\b",
             bygroups(Keyword, Whitespace, Keyword, Whitespace, Name.Namespace, Whitespace, Keyword)),
            (_function + r"(?=\[)", bygroups(*_head, Name.Function, *_alias), "type-parameters"),
            (_function, bygroups(*_head, Name.Function, *_alias)),
            (_record + r"(?=\[)", bygroups(*_head, Name.Class, *_alias), "type-parameters"),
            (_record, bygroups(*_head, Name.Class, *_alias)),
            (_enum + r"(?=\[)", bygroups(*_head, Name.Class, *_alias), ("enum", "type-parameters")),
            (_enum + rf"(?=[^\S\n]*=)", bygroups(*_head, Name.Class, *_alias), "enum-line"),
            (_enum, bygroups(*_head, Name.Class, *_alias), "enum"),
            (r"^(description|examples)\b", Keyword),
            # A function's answer: its type, its bounds and its effect, to the end of the line, or
            # to what closes the parameter a function type is the type of.
            (r"->", Operator, "answer"),
            (r"=>", Operator),
            (r"\.\.", Punctuation),
            (rf"(\.)({_alt(BUILTINS + METHODS)})(?=\()", bygroups(Punctuation, Name.Builtin)),
            (rf"(\.)({NAME})", bygroups(Punctuation, Name)),
            # A field or a parameter with its alias: `請求書(invoice): 請求書`.
            (rf"({NAME})(\()({ALIAS})(\))(?=[^\S\n]*:)", bygroups(Name, *_alias)),
            (words(CONSTANTS, prefix=r"\b", suffix=r"\b"), Keyword.Constant),
            (words(OPERATOR_WORDS, prefix=r"\b", suffix=r"\b"), Operator.Word),
            (words(RESERVED_KEYWORDS, prefix=r"\b", suffix=r"\b"), Keyword),
            # `fails` and the kind of failure an example expects: `f(x: 0) fails zero`.
            (rf"(fails)\b(?={_WS}(?:{_alt(KINDS)})\b)", Keyword, "kinds"),
            # A word before `:` is a field, a parameter or an argument: `total: total(lines: …)`.
            (rf"{NAME}(?=[^\S\n]*:)", Name),
            (rf"({_alt(BOUNDS)})\b(?=[^\S\n]*{_COMPARE})", Keyword),
            # `ok x =>` and `err e =>`: the values of a `result` in a pattern.
            (rf"(ok|err)\b(?={_WS}{NAME}[^\S\n]*(?:,|=>))", Name.Builtin),
            (rf"({_alt(BUILTINS)})\b(?=\()", Name.Builtin),
            # The call of a function of the module, a rule, a date or a function given.
            (rf"{NAME}(?=\()", Name),
            include("types"),
            include("literals"),
            (r"!=|<=|>=|≦|≧|[=<>+\-−*×/÷]", Operator),
            (r"[()\[\]{}:,.?|]", Punctuation),
            (r"…", Punctuation),
            (rf"[{WORD_CHARS}]", Name),
            (NAME, Name),
        ],
        "types": [
            (words(BARE_TYPES + TAX, prefix=r"\b", suffix=r"\b"), Keyword.Type),
            (rf"({_alt(BRACKETED_TYPES)})\b(?=\[)", Keyword.Type),
            (rf"(?<=\[)({_alt(STEP)})\b", Keyword),
        ],
        "literals": [
            # A date is tried before a number, or `2026` would be taken by itself.
            (DATE, Number),
            (NUMBER, Number),
        ],
        # `[T, K: key]` after the name a declaration gives.
        "type-parameters": [
            (r"\[", Punctuation),
            (r"[^\S\n]+", Whitespace),
            (r"(:)([^\S\n]*)(" + _alt(TYPE_PARAMETER) + r")\b", bygroups(Punctuation, Whitespace, Keyword.Type)),
            (r",", Punctuation),
            (NAME, Name),
            (r"\]", Punctuation, "#pop"),
            default("#pop"),
        ],
        # After `->`: the type of the answer, its bounds and its effect. The end of the line ends it,
        # and so does what ends the type of a parameter (`f: (T) -> T, x: T`), a list's type
        # (`list[(T) -> T]`) or the type of a `let` (`let f : (T) -> T = …`).
        "answer": [
            (r"[^\S\n]+", Whitespace),
            (r"\n", Whitespace, "#pop"),
            (r"#[^\n]*", Comment.Single),
            (r"(fails)\b", Keyword, "kinds"),
            (words(EFFECTS, prefix=r"\b", suffix=r"\b"), Keyword),
            (words(BOUNDS, prefix=r"\b", suffix=r"\b"), Keyword),
            (r"[(\[]", Punctuation, "type-group"),
            (r"->", Operator),
            (_COMPARE, Operator),
            (r"[-−]", Operator),
            (r"(?=[)\],=])", Whitespace, "#pop"),
            (r"[?.]", Punctuation),
            include("types"),
            include("literals"),
            (rf"[{WORD_CHARS}]", Name),
            (NAME, Name),
            default("#pop"),
        ],
        # What is between brackets in a type: `money[USDc]`, `dict[string, list[T]]`, `(T, U)`.
        "type-group": [
            (r"\s+", Whitespace),
            (r"#[^\n]*", Comment.Single),
            (r"[(\[]", Punctuation, "#push"),
            (r"[)\]]", Punctuation, "#pop"),
            (r"->", Operator, "answer"),
            (r"[,?.]", Punctuation),
            include("types"),
            include("literals"),
            (rf"[{WORD_CHARS}]", Name),
            (NAME, Name),
            default("#pop"),
        ],
        # After `fails`: the kinds, a `,` between them.
        "kinds": [
            (r"[^\S\n]+", Whitespace),
            (words(KINDS, prefix=r"\b", suffix=r"\b"), Name.Exception),
            (rf",(?=[^\S\n]*(?:{_alt(KINDS)})\b(?![^\S\n]*[:(]))", Punctuation),
            default("#pop"),
        ],
        # `enum size = small < medium < large`: the values on the line of the declaration.
        "enum-line": [
            (r"\n", Whitespace, "#pop"),
            (r"[^\S\n]+", Whitespace),
            (r"#[^\n]*", Comment.Single),
            (r'"[^"\n]*"', String.Double),
            (rf"({NAME})(?:(\()({ALIAS})(\)))?", bygroups(Name, Punctuation, Name.Attribute, Punctuation)),
            (r"[=<]", Operator),
            (r"\|", Punctuation),
            default("#pop"),
        ],
        # The values of an enum, a line each, with their fields under them; the enum ends at the
        # next line at the left margin.
        "enum": [
            (r"^(?=\S)", Whitespace, "#pop"),
            (rf"^({_WS})({NAME})(?:(\()({ALIAS})(\)))?(?=[^\S\n]*(?:\"[^\"\n]*\"[^\S\n]*)?(?:#[^\n]*)?$)",
             bygroups(Whitespace, Name, Punctuation, Name.Attribute, Punctuation)),
            include("root"),
        ],
    }


def _register():
    """Put the lexer where `get_lexer_by_name("kn")` looks: the table and the cache both, so the
    lookup never has to import this module a second time by name."""
    import pygments.lexers as pl

    pl.LEXERS["KanameLexer"] = (__name__, KanameLexer.name, tuple(KanameLexer.aliases), tuple(KanameLexer.filenames), ())
    pl._lexer_cache[KanameLexer.name] = KanameLexer


_register()


def makeExtension(**kwargs):
    """The Markdown extension that exists only so that this module gets imported."""
    from markdown.extensions import Extension

    class KanameHighlighting(Extension):
        def extendMarkdown(self, md):
            pass

    return KanameHighlighting(**kwargs)

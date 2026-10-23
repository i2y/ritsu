"""Syntax highlighting for `.gate` sources on the site.

A ```gate fence is coloured at build time by Pygments, which every other code block on the site goes
through too: the theme's light and dark palettes are written against Pygments' class names, so the
colours follow the reader's scheme, and nothing runs in the browser.

Pygments finds a lexer by looking through its own table, so this module registers itself there on
import. zensical.toml names it as a Markdown extension so that something imports it; the extension
adds nothing to Markdown. build.sh puts this directory on PYTHONPATH.

What the colours say:

    keyword    the words a line starts with, at the margin and in a block, the kind of what a `use`
               reads and its `from`, `from` after a workflow and after a resource's types, `range`
               and `offset`, `allow` and `deny` after `expect`, `open` in `today is open in`, `step`
               in a rate's type, and the reserved words that are neither types, constants nor
               operators (`principal`, `resource`, `workflow`, `today`, `allowed`, `any`)
    type       the types: `bool`, `date`, `number`, `rate`, and `money`, `mass` and the others where
               `[` comes after them; `incl_tax` and `excl_tax` in a type's brackets
    constant   `true` and `false`, and the version after the file's name (`v1`)
    class      the name a declaration gives (an enum, a role, a principal, a workflow, a resource, an
               action, a policy, an expectation, a separation), and the file's own name, the
               namespace's and the name a `use` gives
    attribute  the ASCII alias of a name (`返金する(refund_order)`)
    number     a number with its unit, a date, and the offset after `offset`
    string     a string
    operator   `=`, `<`, `<=`, `>`, `>=`, the minus of a number below zero, and the five reserved
               words written between values: `and`, `or`, `not`, `in`, `is`
    quiet      the punctuation: ( ) [ ] : :: , . ? | and the `…` a page leaves lines out with

A word is coloured by where it is written, as the parser reads it (sekisho's DESIGN 9): the words
of the lines and of the blocks are keywords only at the start of their line, so a name spelled like
one (an enum's value `open`, an attribute `description`, an input `range`) stays a name everywhere
else; a word before `:` is a field, an attribute, an input or an argument, and a word before `=` at
the start of an indented line a computed value, whatever its spelling; a word after `.` is an
attribute; and every word of a `guards` line, of the values of an enum and of the types an action's
`resource` line lists is a name. The reserved words cannot be names (E002), so each is coloured
wherever it is written. Cedar's `==` and `||`, which sekisho's lexer does not read (E001), are marked
as what this lexer could not read, as a character the language does not have is.

The vocabulary is crates/sekisho/src/kw.rs's. The characters of a number are those of
crates/sekisho/src/lex.rs, which reads a unit with ritsu-units' characters (`ritsu_units::chars`),
and the words in a type's brackets are ritsu-units' (`Tax`, and the `step` of a rate). The lists
below are a second copy of them in another language, which is how a vocabulary rots, so
crates/sekisho/tests/docs.rs reads this file and holds each list to the one it copies: a word added
to the language and not here fails it.
"""

import re

from pygments.lexer import RegexLexer, bygroups, default, include, words
from pygments.token import (Comment, Error, Keyword, Name, Number, Operator, Punctuation, String,
                            Whitespace)

__all__ = ["GateLexer"]

# --- the vocabulary, from crates/sekisho/src/kw.rs --------------------------------------------

# The words, by where they are written: kw.rs's TABLE, word for word and in its order.
TABLE = (
    ("line", ("gate", "description", "namespace", "use", "today", "enum", "role", "principal",
              "workflow", "resource", "action", "permit", "forbid", "expect", "separate")),
    ("use", ("rule", "dates", "calendar", "openapi", "proto", "asyncapi", "book", "gate", "from")),
    ("block", ("description", "includes", "can", "roles", "attributes", "guards", "principal",
               "resource", "from", "nobody", "input", "context", "action", "actions", "when",
               "unless")),
    ("condition", ("in", "is", "not", "and", "or", "any", "workflow", "open", "today", "allow",
                   "deny", "allowed", "principal", "resource", "true", "false")),
    ("type and range", ("bool", "date", "number", "rate", "money", "mass", "length", "area",
                        "volume", "duration", "temperature", "sound", "range", "offset")),
)

# The words a name or an alias cannot be (E002): kw.rs's RESERVED, in its order. Being no name,
# each is coloured wherever it is written.
RESERVED = ("and", "or", "not", "in", "is", "principal", "resource", "workflow", "true", "false",
            "allowed", "any", "today", "bool", "date", "number", "rate")

# --- the words in a type's brackets, from ritsu-units -----------------------------------------

# Whether an amount of money is with tax or without (`money[GBP, incl_tax]`): `Tax`'s words.
TAX = ("incl_tax", "excl_tax")
# The word a rate's type writes its step with (`rate[step 1%]`), as `Unit` reads and writes it.
STEP = ("step",)

# --- the characters of a number, from crates/sekisho/src/lex.rs and ritsu_units::chars ---------

# The letters of other scripts a unit is written with right after a number (`is_unit_char`, beside
# the ASCII letters): each is a unit by itself (`100万円`, `5銭`, `30坪`).
UNIT_LETTERS = "円銭坪"
# The signs a unit is written with (`is_sign`): one character right after a number (`5%`, `-40℃`),
# the full-width `％` as `%`.
SIGNS = "%％℃℉"
# The signs that are a word by themselves right after a type's `[` (`is_word_sign`):
# `temperature[℃]`.
WORD_SIGNS = "℃℉"
# The multipliers written between a number and its unit (`unit_after`): `100万円`.
MULTIPLIERS = "万億"

# --- how each word is coloured ------------------------------------------------------------------

_PLACES = dict(TABLE)


def _of(place, chosen):
    """Words of a place of TABLE, checked to be there: a word misspelt here fails the build."""
    missing = [w for w in chosen if w not in _PLACES[place]]
    assert not missing, f"gatelexer.py: {missing} are not words of the place {place!r} of TABLE"
    return tuple(chosen)


# The lines at the left margin: the patterns below write out the ones with a shape of their own, and
# the ones that declare a name and nothing else (DECLARING); every word of the place starts a line
# as a keyword.
_of("line", ("gate", "namespace", "use", "today", "enum", "workflow", "expect"))
DECLARING = _of("line", ("role", "principal", "resource", "action", "permit", "forbid", "separate"))
LINES = _PLACES["line"]
# What a `use` reads (`use rule refund_limit from "…"`); `use gate "…"` gives no name.
USE_KINDS = tuple(w for w in _PLACES["use"] if w not in ("gate", "from"))
_of("use", ("gate", "from"))
# The words an indented line starts with; `from` is the one of the place written after the types of
# an action's `resource` line, and `resource` and `guards` take lines of their own below.
_of("block", ("from",))
BLOCK_LINES = tuple(w for w in _PLACES["block"] if w not in ("from", "resource", "guards"))
_of("block", ("resource", "guards"))
# The words of a condition that are no reserved word, each a keyword only where it is written.
_of("condition", ("open", "allow", "deny"))
# The types, and the two words of a range: `range` after a field's type and after `today`, and
# `offset` after the range of `today`.
BOUNDS = _of("type and range", ("range", "offset"))
TYPES = tuple(w for w in _PLACES["type and range"] if w not in BOUNDS)
# The reserved words, by what they are: types, constants, the words written between values, and the
# rest.
assert all(any(w in ws for _, ws in TABLE) for w in RESERVED), "gatelexer.py: RESERVED are not words of TABLE"
BARE_TYPES = tuple(w for w in RESERVED if w in TYPES)
CONSTANTS = tuple(w for w in RESERVED if w in ("true", "false"))
OPERATOR_WORDS = tuple(w for w in RESERVED if w in ("and", "or", "not", "in", "is"))
RESERVED_KEYWORDS = tuple(w for w in RESERVED if w not in BARE_TYPES + CONSTANTS + OPERATOR_WORDS)

# --- the patterns ------------------------------------------------------------------------------

# A name: letters of any script, digits and `_`, not starting with an ASCII digit, which starts a
# number; and a name that starts with digits and goes on in another script (`3日以内`), which the
# lexer cuts out as a word when what follows the digits is no unit.
NAME = r"[^\W0-9]\w*"
DIGIT_NAME = r"[0-9]+(?![0-9])\w+"
ALIAS = r"[A-Za-z_][A-Za-z0-9_]*"
_WS = r"[^\S\n]+"
_DIGITS = r"[0-9](?:[0-9]|_(?=[0-9]))*"
_NUMBER = rf"{_DIGITS}(?:\.[0-9]+)?"
# A number with its unit: a sign, or what a name's characters run on with after the digits when it
# is a unit (a multiplier, then ASCII letters and digits or a letter of UNIT_LETTERS). A run that is
# no unit and is not ASCII makes the whole a name (`3日以内`, `100万`).
NUMBER = (rf"{_NUMBER}(?:[{SIGNS}]|[{MULTIPLIERS}]?(?:[A-Za-z][A-Za-z0-9]*|[{UNIT_LETTERS}])(?!\w)"
          rf"|(?!\w))")
DATE = r"[0-9]{4}-[0-9]{2}-[0-9]{2}(?!\w)"
_COMPARE = r"(?:<=|>=|<|>)"


def _alt(ws):
    """The words as alternatives of a pattern, the longest first."""
    return "|".join(re.escape(w) for w in sorted(ws, key=len, reverse=True))


def _declared(kind):
    """A name a declaration gives, with its alias: `返金する(refund_order)`."""
    return (rf"({NAME}|{DIGIT_NAME})(?:(\()({ALIAS})(\)))?", (kind, Punctuation, Name.Attribute, Punctuation))


_NAME, _NAMED = _declared(Name.Class)


class GateLexer(RegexLexer):
    """sekisho, the language of who may do what."""

    name = "sekisho"
    aliases = ["gate", "sekisho"]
    filenames = ["*.gate"]

    tokens = {
        "root": [
            # The lines of a block, and of the block under one of them: a field (`amount : …`), a
            # computed value (`in_period = …`), or a line a word of the block starts.
            (rf"^({_WS}){_NAME}([^\S\n]*)(:)(?!:)", bygroups(Whitespace, Name, *_NAMED[1:], Whitespace, Punctuation), "field"),
            (rf"^({_WS}){_NAME}([^\S\n]*)(=)", bygroups(Whitespace, Name, *_NAMED[1:], Whitespace, Operator)),
            (rf"^({_WS})(resource)\b", bygroups(Whitespace, Keyword), "resource-line"),
            (rf"^({_WS})(guards)\b", bygroups(Whitespace, Keyword), "names"),
            (rf"^({_WS})({_alt(BLOCK_LINES)})\b", bygroups(Whitespace, Keyword)),
            (r"[^\S\n]+", Whitespace),
            (r"\n", Whitespace),
            (r"#[^\n]*", Comment.Single),
            (r'"(?:[^"\\\n]|\\["\\]|\\(?!["\\]))*"', String.Double),
            # The lines at the left margin, and the names they declare.
            (rf"^(gate)({_WS}){_NAME}({_WS})(v[0-9]+)\b",
             bygroups(Keyword, Whitespace, Name.Namespace, *_NAMED[1:], Whitespace, Name.Constant)),
            (rf"^(namespace)({_WS})", bygroups(Keyword, Whitespace), "namespace"),
            (rf"^(use)({_WS})({_alt(USE_KINDS)})({_WS})({NAME})({_WS})(from)\b",
             bygroups(Keyword, Whitespace, Keyword, Whitespace, Name.Namespace, Whitespace, Keyword)),
            (rf"^(use)({_WS})(gate)\b", bygroups(Keyword, Whitespace, Keyword)),
            (r"^(today)\b", Keyword, "today"),
            (rf"^(enum)({_WS}){_NAME}", bygroups(Keyword, Whitespace, *_NAMED), "enum"),
            (rf"^(workflow)({_WS}){_NAME}(?:({_WS})(from)\b)?", bygroups(Keyword, Whitespace, *_NAMED, Whitespace, Keyword)),
            (rf"^(expect)({_WS})(allow|deny)\b(?:({_WS}){_NAME})?",
             bygroups(Keyword, Whitespace, Keyword, Whitespace, *_NAMED)),
            (rf"^({_alt(DECLARING)})({_WS}){_NAME}", bygroups(Keyword, Whitespace, *_NAMED)),
            (rf"^({_alt(LINES)})\b", Keyword),
            # `today is open in uk`: whether today is a day the calendar is open.
            (rf"(today)({_WS})(is)({_WS})(open)({_WS})(in)\b",
             bygroups(Keyword, Whitespace, Operator.Word, Whitespace, Keyword, Whitespace, Operator.Word)),
            # After `.` an attribute, and before `:` the name of an argument, whatever the spelling.
            (rf"(\.)({NAME})", bygroups(Punctuation, Name)),
            (rf"{NAME}(?=[^\S\n]*:(?!:))", Name),
            (words(CONSTANTS, prefix=r"\b", suffix=r"\b"), Keyword.Constant),
            (words(OPERATOR_WORDS, prefix=r"\b", suffix=r"\b"), Operator.Word),
            (words(BARE_TYPES, prefix=r"\b", suffix=r"\b"), Keyword.Type),
            (words(RESERVED_KEYWORDS, prefix=r"\b", suffix=r"\b"), Keyword),
            include("literals"),
            # Cedar's `==` and `||`, which sekisho's lexer does not read (E001 says `is` and `or`).
            (r"==|\|\|", Error),
            (r"<=|>=|[=<>\-]", Operator),
            (r"::|[()\[\]:,.?|]", Punctuation),
            (r"…", Punctuation),
            (NAME, Name),
            (DIGIT_NAME, Name),
        ],
        "literals": [
            # A date is tried before a number, or `2026` would be taken by itself.
            (DATE, Number),
            (NUMBER, Number),
        ],
        # After `name :` in `attributes` and `input`: the type, which is the first word whatever its
        # spelling (an enum or an entity type when it is no type of the language), then `?` and the
        # range.
        "field": [
            (r"[^\S\n]+", Whitespace),
            (rf"({_alt(TYPES)})\b(?=\[)", Keyword.Type, ("#pop", "field-rest")),
            (words(BARE_TYPES, prefix=r"\b", suffix=r"\b"), Keyword.Type, ("#pop", "field-rest")),
            (rf"{NAME}|{DIGIT_NAME}", Name, ("#pop", "field-rest")),
            default(("#pop", "field-rest")),
        ],
        "field-rest": [
            (r"\n", Whitespace, "#pop"),
            (r"[^\S\n]+", Whitespace),
            (r"#[^\n]*", Comment.Single),
            (r"\[", Punctuation, "type"),
            (r"[?]", Punctuation),
            (r"(range)\b", Keyword),
            (_COMPARE, Operator),
            (r"-", Operator),
            (words(CONSTANTS, prefix=r"\b", suffix=r"\b"), Keyword.Constant),
            include("literals"),
            (NAME, Name),
            (r"…", Punctuation),
        ],
        # What is between a type's brackets: `money[GBP, incl_tax]`, `rate[step 1%]`,
        # `temperature[℃]`.
        "type": [
            (r"[^\S\n]+", Whitespace),
            (r"\]", Punctuation, "#pop"),
            (r",", Punctuation),
            (words(TAX, prefix=r"\b", suffix=r"\b"), Keyword.Type),
            (words(STEP, prefix=r"\b", suffix=r"\b"), Keyword),
            include("literals"),
            (rf"[{WORD_SIGNS}]", Name),
            (NAME, Name),
            default("#pop"),
        ],
        # After `namespace`: `Shop` or `Shop::Orders`.
        "namespace": [
            (r"\n", Whitespace, "#pop"),
            (r"[^\S\n]+", Whitespace),
            (r"#[^\n]*", Comment.Single),
            (r"::", Punctuation),
            (NAME, Name.Namespace),
        ],
        # `today range >=2026-10-01 <=2028-10-31 offset +00:00`.
        "today": [
            (r"\n", Whitespace, "#pop"),
            (r"[^\S\n]+", Whitespace),
            (r"#[^\n]*", Comment.Single),
            (r"(range)\b", Keyword),
            (rf"(offset)(?:({_WS})([+-][0-9]{{2}}:[0-9]{{2}}))?", bygroups(Keyword, Whitespace, Number)),
            (_COMPARE, Operator),
            (r"-", Operator),
            include("literals"),
            (NAME, Name),
            (r"[/+:]", Punctuation),
        ],
        # `enum status = paid | shipped | returned`: the values, a name each with its alias.
        "enum": [
            (r"\n", Whitespace, "#pop"),
            (r"[^\S\n]+", Whitespace),
            (r"#[^\n]*", Comment.Single),
            (r"=", Operator),
            (r"\|", Punctuation),
            (_NAME, bygroups(Name, *_NAMED[1:])),
            (r"…", Punctuation),
        ],
        # The types of an action's `resource` line, then the parameter after `from`.
        "resource-line": [
            (r"\n", Whitespace, "#pop"),
            (r"[^\S\n]+", Whitespace),
            (r"#[^\n]*", Comment.Single),
            (rf"({NAME}|{DIGIT_NAME})([^\S\n]*)(,)", bygroups(Name, Whitespace, Punctuation)),
            (rf"({NAME}|{DIGIT_NAME})(?:({_WS})(from)\b)?", bygroups(Name, Whitespace, Keyword), ("#pop", "names")),
            (r"…", Punctuation),
        ],
        # A line whose every word is a name: what a `guards` line guards (`guards orders
        # refundOrder`, `guards stock receive.do`, a string), and the parameter after a `from`.
        "names": [
            (r"\n", Whitespace, "#pop"),
            (r"[^\S\n]+", Whitespace),
            (r"#[^\n]*", Comment.Single),
            (r'"(?:[^"\\\n]|\\["\\]|\\(?!["\\]))*"', String.Double),
            (r"\.", Punctuation),
            (NAME, Name),
            (DIGIT_NAME, Name),
            (r"…", Punctuation),
        ],
    }


def _register():
    """Put the lexer where `get_lexer_by_name("gate")` looks: the table and the cache both, so the
    lookup never has to import this module a second time by name."""
    import pygments.lexers as pl

    pl.LEXERS["GateLexer"] = (__name__, GateLexer.name, tuple(GateLexer.aliases), tuple(GateLexer.filenames), ())
    pl._lexer_cache[GateLexer.name] = GateLexer


_register()


def makeExtension(**kwargs):
    """The Markdown extension that exists only so that this module gets imported."""
    from markdown.extensions import Extension

    class GateHighlighting(Extension):
        def extendMarkdown(self, md):
            pass

    return GateHighlighting(**kwargs)

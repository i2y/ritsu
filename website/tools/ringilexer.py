"""Syntax highlighting for `.ringi` sources on the site.

A ```ringi fence is coloured at build time by Pygments, as every other code block on the site is:
the theme's light and dark palettes are written against Pygments' class names, so the colours
follow the reader's scheme, and nothing runs in the browser.

Pygments finds a lexer by looking through its own table, so this module registers itself there on
import. zensical.toml names it as a Markdown extension so that something imports it; the extension
adds nothing to Markdown. build.sh puts this directory on PYTHONPATH.

What the colours say:

    keyword    the words of each place the language writes them (a line, a `use`, a field's
               marks, a stage, the ways to choose an approver, a deadline, the rules of a ticket,
               a claim, a scenario), and the columns of a scenario's table
    type       a field's type, after its `:` (`money[JPY]`, `long text`, `choice from`)
    constant   `true`, `false` and `none`, and the states of a ticket in a scenario's table
    class      the name the first line of a file gives, and the name a `use`, a `table`, a
               `scenario` or a `master` gives
    builtin    the events in a scenario's table and the endings after `on`, `sum(…)` and the ways
               to round, and `requester` and `ticket` where an expression reads them
    attribute  the ASCII name of a name written in another script (`上長(manager)`)
    number     a number with its unit, a date and a time
    string     a string
    operator   the operators, and the four reserved words written between values: `and`, `or`,
               `not`, `is`
    quiet      the punctuation: ( ) [ ] { } : , . | and the `…` a page leaves lines out with

A word is coloured by where it is written, as the parser reads it (DESIGN 3.2): `person` is a type
after a field's `:` and a way to choose an approver after `who`'s `=`; `manager` is a word only in
`manager of`, so a stage or a way to choose an approver may be named `manager`. A word before `:`
or after `.` is a name, whatever its spelling.

The vocabulary is crates/ringi/src/kw.rs's, and the characters of a number are
crates/ringi/src/lex.rs's. The lists below are a second copy of them in another language, which is
how a vocabulary rots, so crates/ringi/tests/docs.rs reads this file and holds each list to the
one it copies: a word added to the language and not here fails it.
"""

import re

from pygments.lexer import RegexLexer, bygroups, default, include, words
from pygments.token import (Comment, Keyword, Name, Number, Operator, Punctuation, String,
                            Whitespace)

__all__ = ["RingiLexer"]

# --- the vocabulary, from crates/ringi/src/kw.rs -----------------------------------------------

# The words, by where they are written: kw.rs's WORDS, word for word and in its order. `…` stands
# for what is written between two words, `(s)` for a plural the word may take.
WORDS = (
    ("line", ("request", "route", "organization", "masters", "description", "owner",
              "effective from", "use", "form", "who", "every stage", "table", "ticket", "claims",
              "examples", "scenario", "labels", "inputs", "positions", "team kinds", "data",
              "master", "key", "fields")),
    ("use", ("organization", "masters", "calendar", "book", "flow", "rule", "dates", "route",
             "gate", "from")),
    ("type", ("text", "long text", "number", "money", "rate", "date", "choice", "choice from",
              "attachment", "person", "team", "bool", "rows")),
    ("mark", ("range", "count", "optional", "required when", "shown when", "approvers only",
              "open in", "round")),
    ("stage", ("at least", "of", "%", "all of", "circulate to", "note from", "when", "and")),
    ("who", ("person", "team", "manager of", "head of", "nearest … above", "requester",
             "chosen by", "previous approver", "in", "lookup … by", "agent", "else")),
    ("deadline", ("due in", "remind after", "business day(s)", "at end of day", "at", "when late",
                  "when empty", "escalate", "by", "then", "expire", "stay", "skip")),
    ("ticket", ("self approval", "allowed", "not allowed", "same approver approves once",
                "at the last stage", "submitted by", "everyone", "everyone in", "team",
                "position", "or higher", "in", "person", "except", "send back",
                "to the requester", "to any earlier stage", "at most", "times",
                "resubmit within", "withdraw until", "it is decided",
                "retract approval until the next stage answers", "pause at most",
                "approvers edit", "comment required for", "reject", "on", "hold", "post", "void",
                "launch")),
    ("claim", ("only when", "possible", "is", "every stage reminds before it is due",
               "every ticket ends within")),
    ("scenario", ("requester", "given", "form", "at", "event", "by", "comment", "-> stage",
                  "status")),
    ("expression", ("is", "is not", "none", "and", "or", "not", "true", "false", "sum(…)", "+",
                    "-", "*", "/", "business days in")),
)

# The words a name cannot be (E002): kw.rs's RESERVED. Being no name, each is coloured wherever it
# is written.
RESERVED = ("and", "or", "not", "is", "none", "true", "false")

# What an expression reads besides the fields, which no field can be named: kw.rs's NOT_FIELDS.
NOT_FIELDS = ("requester", "ticket")

# The units of measure a field's type takes besides money and a rate: kw.rs's QUANTITIES.
QUANTITIES = ("mass", "length", "area", "volume", "duration", "temperature", "sound")

# rulec's ways of rounding, after `round` (`round down(1JPY)`): kw.rs's ROUNDINGS.
ROUNDINGS = ("up", "down", "half_up", "half_down", "half_even")

# What a scenario's table is made of: its columns (kw.rs's SCENARIO_COLUMNS), the events its rows
# write (EVENTS) and the states it says a ticket is in (STATES).
COLUMNS = ("at", "event", "by", "comment", "stage", "status")
EVENTS = ("submit", "resubmit", "approve", "reject", "send back", "retract", "pause", "resume",
          "withdraw", "discard", "edit", "add approver", "hand over", "comment", "remind", "late",
          "resubmit late", "pause ends", "agent answered", "note ready", "flow failed")
STATES = ("in progress", "paused", "with requester", "approved", "rejected", "withdrawn",
          "expired")

# Where a ticket can be taken, after `on`: kw.rs's ENDINGS.
ENDINGS = ("submitted", "approved", "sent back", "rejected", "withdrawn", "expired")

# The languages a `labels` block gives its texts in: kw.rs's LANGS.
LANGS = ("ja", "en")

# --- the characters of a number and a name, from crates/ringi/src/lex.rs -----------------------

# What a unit is written with after a number, beside the ASCII letters (`is_unit_char`): `60%`,
# `1000万円`, `30_000JPY`.
UNIT_CHARS = "円銭坪℃℉%％"
# The multipliers written between a number and its unit (`number`): `1000万円`.
MULTIPLIERS = "万億兆"
# The characters that are a word by themselves (`starts_name`): `temperature[℃]`.
WORD_CHARS = "℃℉"

# --- how each word is coloured ----------------------------------------------------------------

_PLACES = dict(WORDS)


def _of(place, chosen):
    """Words of a place of WORDS, checked to be there: a word misspelt here fails the build."""
    missing = [w for w in chosen if w not in _PLACES[place]]
    assert not missing, f"ringilexer.py: {missing} are not words of the place {place!r} of WORDS"
    return tuple(chosen)


# The first line of a file says its kind.
FILE_KINDS = _of("line", ("request", "route", "organization", "masters"))
# The lines that give a name: `table next_up`, `scenario a_long_leave`, `master accounts "…"`.
NAMING_LINES = _of("line", ("table", "scenario", "master"))
_of("line", ("use", "labels", "who", "every stage"))
# What a `use` reads, then `from`: the kinds of `use` but the gate, which has no name.
USE_KINDS = tuple(w for w in _PLACES["use"] if w not in ("gate", "from"))
_of("use", ("gate", "from"))
# The places of a `labels` block: a stage (the word of `every stage`) and a way to choose an
# approver (`who`).
LABEL_PLACES = ("stage", "who")
_of("scenario", ("-> stage",))
# The types after a field's `:`, with the units of measure.
TYPES = _PLACES["type"] + QUANTITIES
assert all(w in _PLACES["expression"] for w in RESERVED), "ringilexer.py: RESERVED are not words of an expression"
CONSTANTS = tuple(w for w in RESERVED if w in ("true", "false", "none"))
OPERATOR_WORDS = tuple(w for w in RESERVED if w not in CONSTANTS)
# The words written as a sign or with `…`, and the words of a scenario's table, are read by the
# patterns of their own below; every other word of every place but the type is a keyword where it
# is written.
_SIGNS = ("%", "+", "-", "*", "/", "sum(…)", "-> stage")
_HEAD = tuple(w for w in COLUMNS if w not in ("at", "by"))
assert all(w in _PLACES["scenario"] or f"-> {w}" in _PLACES["scenario"] for w in COLUMNS), \
    "ringilexer.py: COLUMNS are not words of a scenario"


def _phrase(w):
    """A word of WORDS as a pattern: the words of a phrase apart by any spaces, `(s)` a plural."""
    return r"[^\S\n]+".join(re.escape(p).replace(r"\(s\)", "s?") for p in w.split(" "))


def _alt(ws):
    """The words as alternatives of a pattern, the longest first."""
    return "|".join(_phrase(w) for w in sorted(ws, key=len, reverse=True))


_split = []
for _place, _ws in WORDS:
    if _place == "type":
        continue
    for _w in _ws:
        # a sign, a word of a scenario's head, and the reserved words alone or together (`is not`)
        if _w in _SIGNS or _w in _HEAD or all(p in RESERVED for p in _w.split(" ")):
            continue
        # `nearest … above`, `lookup … by`: the two words, each where it is written
        _split.extend(p.strip() for p in _w.split("…")) if "…" in _w else _split.append(_w)
KEYWORDS = tuple(dict.fromkeys(_split))
# The reserved words written together between values: `is not`.
OPERATOR_PHRASES = tuple(w for w in _PLACES["expression"] if " " in w and all(p in OPERATOR_WORDS for p in w.split(" ")))

# --- the patterns ------------------------------------------------------------------------------

_DIGIT = "[0-9０-９]"
_DIGITS = rf"{_DIGIT}(?:_?{_DIGIT})*"
_UNIT = rf"[A-Za-z{UNIT_CHARS}][A-Za-z0-9{UNIT_CHARS}]*"
NUMBER = rf"{_DIGITS}(?:\.{_DIGITS})?[{MULTIPLIERS}]?(?:{_UNIT})?"
DATE = rf"{_DIGIT}{{4}}-{_DIGIT}{{2}}-{_DIGIT}{{2}}(?!\w)"
TIME = rf"{_DIGIT}{{2}}:{_DIGIT}{{2}}(?!{_DIGIT})"
# A name: letters of any script, digits and `_`, not starting with a digit.
NAME = r"[^\W\d]\w*"
# A name with a letter outside ASCII, which may be given an ASCII name in brackets.
WIDE_NAME = r"(?=\w*[^\x00-\x7f])[^\W\d]\w*"
ALIAS = r"[A-Za-z_][A-Za-z0-9_]*"
_WS = r"[^\S\n]+"
_ALIASED = rf"({NAME})(?:(\()({ALIAS})(\)))?"
_ALIAS_TOKENS = (Punctuation, Name.Attribute, Punctuation)


class RingiLexer(RegexLexer):
    """ringi, the language of requests and their approval."""

    name = "ringi"
    aliases = ["ringi"]
    filenames = ["*.ringi"]

    tokens = {
        "root": [
            # `on approved launch …`: where the ticket is taken, then what is done. Tried before
            # the whitespace rule, which would otherwise eat the indentation.
            (rf"^({_WS})(on)\b(?={_WS})", bygroups(Whitespace, Keyword), "endings"),
            (r"[^\S\n]+", Whitespace),
            (r"\n", Whitespace),
            (r"#[^\n]*", Comment.Single),
            # A string has no escape: everything up to the next `"`.
            (r'"[^"\n]*"', String.Double),
            # The first line: the kind of the file, its name and its version.
            (rf"^({_alt(FILE_KINDS)})({_WS}){_ALIASED}({_WS})(v{_DIGIT}+)\b",
             bygroups(Keyword, Whitespace, Name.Class, *_ALIAS_TOKENS, Whitespace, Name.Constant)),
            (rf"^(use)({_WS})({_alt(USE_KINDS)})({_WS}){_ALIASED}({_WS})(from)\b",
             bygroups(Keyword, Whitespace, Keyword, Whitespace, Name.Namespace, *_ALIAS_TOKENS,
                      Whitespace, Keyword)),
            (rf"^(use)({_WS})(gate)\b", bygroups(Keyword, Whitespace, Keyword)),
            (rf"^({_alt(NAMING_LINES)})({_WS}){_ALIASED}",
             bygroups(Keyword, Whitespace, Name.Class, *_ALIAS_TOKENS)),
            (rf"^(labels)({_WS})({_alt(LANGS)})\b", bygroups(Keyword, Whitespace, Keyword), "labels"),
            # A scenario's table: its head, then its rows, to the first line that is no row.
            (rf"^(?=\|[^\S\n]*at[^\S\n]*\|[^\S\n]*event[^\S\n]*\|)", Whitespace,
             ("scenario-rows", "scenario-head")),
            include("names"),
            include("words"),
            include("literals"),
            include("signs"),
        ],
        "names": [
            # The ASCII name of a name written in another script: `上長(manager)`.
            (rf"({WIDE_NAME})(\()({ALIAS})(\))", bygroups(Name, *_ALIAS_TOKENS)),
            # After `.`, a name: a field of a use, a value of a choice (`kind.paid`).
            (rf"(\.)({NAME})", bygroups(Punctuation, Name)),
            # A word before `:` is a field, an argument or a claim, and a word before `=` a way to
            # choose an approver or a field given a value, whatever its spelling.
            (rf"{NAME}(?=[^\S\n]*(?::|=(?!=)))", Name),
            # A field's type, after its `:`.
            (rf"(:)([^\S\n]*)({_alt(TYPES)})\b", bygroups(Punctuation, Whitespace, Keyword.Type)),
            (rf"({_alt(NOT_FIELDS)})(?=\.)", Name.Builtin),
            (rf"(sum|{_alt(ROUNDINGS)})(?=\()", Name.Builtin),
        ],
        "words": [
            (words(CONSTANTS, prefix=r"\b", suffix=r"\b"), Keyword.Constant),
            # a phrase that starts with a reserved word (`not allowed`) before the word alone
            (rf"\b({_alt(KEYWORDS)})\b", Keyword),
            (rf"\b({_alt(OPERATOR_PHRASES)})\b", Operator.Word),
            (words(OPERATOR_WORDS, prefix=r"\b", suffix=r"\b"), Operator.Word),
        ],
        "literals": [
            # A date and a time are tried before a number, or `2026` would be taken by itself.
            (DATE, Number),
            (TIME, Number),
            (NUMBER, Number),
        ],
        "signs": [
            (r"->|→|<=|>=|≦|≧|[=<>＜＞+＋\-−*×/÷]", Operator),
            (r"[()\[\]{}:,.|、，・…]", Punctuation),
            (rf"[{WORD_CHARS}]", Name),
            (NAME, Name),
        ],
        # The head of a scenario's table: its columns.
        "scenario-head": [
            (r"\n", Whitespace, "#pop"),
            (r"[^\S\n]+", Whitespace),
            (r"->", Operator),
            (words(COLUMNS, prefix=r"\b", suffix=r"\b"), Keyword),
            (r"\|", Punctuation),
        ],
        # A row of a scenario's table: an event, who did it, what they wrote, the stage and the
        # state the ticket is in after it.
        "scenario-rows": [
            (r"^(?!\|)", Whitespace, "#pop"),
            (r"\n", Whitespace),
            (r"[^\S\n]+", Whitespace),
            (rf"(\|)([^\S\n]*)({_alt(EVENTS)})(?=[^\S\n]*[|(])", bygroups(Punctuation, Whitespace, Name.Builtin)),
            (rf"(\|)([^\S\n]*)({_alt(STATES)})(?=[^\S\n]*\|)", bygroups(Punctuation, Whitespace, Name.Constant)),
            (r"#[^\n]*", Comment.Single),
            (r'"[^"\n]*"', String.Double),
            include("names"),
            include("words"),
            include("literals"),
            include("signs"),
        ],
        # After `on`: the endings, a `,` between them.
        "endings": [
            (r"[^\S\n]+", Whitespace),
            (rf"({_alt(ENDINGS)})\b", Name.Builtin),
            (rf",(?=[^\S\n]*(?:{_alt(ENDINGS)})\b)", Punctuation),
            default("#pop"),
        ],
        # The texts of a `labels` block, a line each: the place of a stage or a way to choose an
        # approver, then its name. The block ends at the next line at the left margin.
        "labels": [
            (r"^(?=\S)", Whitespace, "#pop"),
            (rf"^({_WS})({_alt(LABEL_PLACES)})(?={_WS})", bygroups(Whitespace, Keyword)),
            include("root"),
        ],
    }


def _register():
    """Put the lexer where `get_lexer_by_name("ringi")` looks: the table and the cache both, so the
    lookup never has to import this module a second time by name."""
    import pygments.lexers as pl

    pl.LEXERS["RingiLexer"] = (__name__, RingiLexer.name, tuple(RingiLexer.aliases), tuple(RingiLexer.filenames), ())
    pl._lexer_cache[RingiLexer.name] = RingiLexer


_register()


def makeExtension(**kwargs):
    """The Markdown extension that exists only so that this module gets imported."""
    from markdown.extensions import Extension

    class RingiHighlighting(Extension):
        def extendMarkdown(self, md):
            pass

    return RingiHighlighting(**kwargs)

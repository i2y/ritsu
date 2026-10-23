"""Syntax highlighting for Cedar's policies and schemas on the site.

A ```cedar fence (policies, `.cedar`) and a ```cedarschema fence (a schema in the Cedar schema format,
`.cedarschema`) are coloured at build time by Pygments, which every other code block on the site goes
through too: the theme's light and dark palettes are written against Pygments' class names, so the
colours follow the reader's scheme, and nothing runs in the browser. Pygments has no lexer for Cedar.

Pygments finds a lexer by looking through its own table, so this module registers both there on
import. zensical.toml names it as a Markdown extension so that something imports it; the extension
adds nothing to Markdown. build.sh puts this directory on PYTHONPATH.

What the colours say:

    keyword    the words of the grammar that are neither constants nor operators: in a policy, its
               effect (`permit`, `forbid`), its conditions (`when`, `unless`), `if`, `then`, `else`
               and the four variables (`principal`, `action`, `resource`, `context`); in a schema,
               the words of its declarations (`namespace`, `entity`, `action`, `type`, `in`,
               `appliesTo`, `tags`, `enum`, `attributes`, and `principal`, `resource` and `context`
               in an `appliesTo`)
    type       in a schema, the primitive types (`Long`, `String`, `Bool`) and `Set`
    constant   `true` and `false`
    class      an entity type: in a policy, the type of an entity (`Role` in `Shop::Role::"clerk"`)
               and the type after `is`; in a schema, the name an `entity`, a `type` or an `action`
               declares, and the types its declarations and attributes are of
    namespace  the namespaces of a name (`Shop` in `Shop::Order`), and the one a schema declares
    function   a function of an extension, called (`ip("10.0.0.1")`, `x.lessThan(y)`)
    decorator  an annotation (`@id`, `@doc`)
    variable   a slot of a template (`?principal`, `?resource`)
    number     a number
    string     a string: an entity's id, an action's name written in quotes, a pattern, a value
    operator   the operators, and the four words of the policy grammar written between values: `in`,
               `has`, `like`, `is`
    quiet      the punctuation, and the `…` a page leaves lines out with

A word is coloured by where it is written, as the grammar reads it: after `.` and `has` a word is an
attribute, and before `:` a key of a record or, in a schema, an attribute, whatever its spelling; only
in an `appliesTo` are `principal`, `resource` and `context` before `:` the grammar's words.

The vocabulary is Cedar 4.13.0's, the version ritsu reads (`ritsu_base::cedar`, ritsu's DESIGN 4.18):
the words and the punctuation the `match` of each of its two grammars lists, at the tag v4.13.0 of
<https://github.com/cedar-policy/cedar>:

    cedar-policy-core/src/parser/grammar.lalrpop                  the policies
    cedar-policy-core/src/validator/cedar_schema/grammar.lalrpop  the schemas

The documents of the grammars write the same words as their terminals
(<https://docs.cedarpolicy.com/policies/syntax-grammar.html>,
<https://docs.cedarpolicy.com/schema/human-readable-schema-grammar.html>); the one of the schemas
writes `external` too, which 4.13.0 does not read. The lists below are a second copy of what ritsu
reads, which is how a vocabulary rots, so crates/ritsu-base/tests/cedar.rs reads this file and holds
each list to ritsu_base::cedar: the words its readers read by name, the reserved words, the
punctuation its lexer reads in each grammar, and the version. A word ritsu comes to read and this
file does not fails there.
"""

import re

from pygments.lexer import RegexLexer, bygroups, default, include, words
from pygments.token import (Comment, Keyword, Name, Number, Operator, Punctuation, String,
                            Whitespace)

__all__ = ["CedarLexer", "CedarSchemaLexer"]

# The version of Cedar the lists are of: the one ritsu_base::cedar reads.
CEDAR = "4.13.0"

# --- the vocabulary of Cedar 4.13.0 --------------------------------------------------------------

# The words of the policy grammar, in the order of its `match`.
POLICY_WORDS = ("true", "false", "if", "permit", "forbid", "when", "unless", "in", "has", "like",
                "is", "then", "else", "principal", "action", "resource", "context")
# The words of the schema grammar, in the order of its `match`.
SCHEMA_WORDS = ("namespace", "entity", "in", "type", "Set", "appliesTo", "principal", "action",
                "resource", "context", "attributes", "tags", "Long", "String", "Bool", "enum")
# The words no identifier may be (and `__cedar`, which no namespace may be).
RESERVED = ("true", "false", "if", "then", "else", "in", "is", "like", "has")
# The punctuation and the operators each grammar reads, as ritsu's lexer of Cedar lists them: the
# longer before the shorter that starts it.
POLICY_PUNCTUATION = ("::", "==", "!=", "<=", ">=", "||", "&&", "@", ".", ",", ";", ":", "(", ")",
                      "{", "}", "[", "]", "<", ">", "+", "-", "*", "/", "%", "!", "=")
SCHEMA_PUNCTUATION = ("::", ",", ";", ":", "{", "}", "[", "]", "<", ">", "=", "?", "@", "(", ")")

# --- how each word is coloured ------------------------------------------------------------------


def _of(listed, chosen, what):
    """Words of a list above, checked to be there: a word misspelt here fails the build."""
    missing = [w for w in chosen if w not in listed]
    assert not missing, f"cedarlexer.py: {missing} are not {what}"
    return tuple(chosen)


CONSTANTS = _of(POLICY_WORDS, ("true", "false"), "words of the policy grammar")
OPERATOR_WORDS = _of(POLICY_WORDS, ("in", "has", "like", "is"), "words of the policy grammar")
POLICY_KEYWORDS = tuple(w for w in POLICY_WORDS if w not in CONSTANTS + OPERATOR_WORDS)
assert all(w in POLICY_WORDS for w in RESERVED), "cedarlexer.py: RESERVED are not words of the policy grammar"
PRIMITIVE_TYPES = _of(SCHEMA_WORDS, ("Long", "String", "Bool"), "words of the schema grammar")
SET = _of(SCHEMA_WORDS, ("Set",), "words of the schema grammar")
# The words of an `appliesTo`, and the words that declare a name, which the patterns write out.
APPLIES = _of(SCHEMA_WORDS, ("principal", "resource", "context"), "words of the schema grammar")
_of(SCHEMA_WORDS, ("namespace", "entity", "type", "action", "appliesTo"), "words of the schema grammar")
SCHEMA_KEYWORDS = tuple(w for w in SCHEMA_WORDS if w not in PRIMITIVE_TYPES + SET + APPLIES)
POLICY_OPERATORS = _of(POLICY_PUNCTUATION, ("==", "!=", "<=", ">=", "||", "&&", "<", ">", "+", "-", "*",
                                            "/", "%", "!", "="), "punctuation of the policy grammar")
SCHEMA_OPERATORS = _of(SCHEMA_PUNCTUATION, ("=",), "punctuation of the schema grammar")

# --- the patterns ------------------------------------------------------------------------------

IDENT = r"[_a-zA-Z][_a-zA-Z0-9]*"
# A string: escapes are read where they are used, so a `\` takes the character after it whatever it
# is but a line feed.
STRING = r'"(?:\\.|[^"\\])*"'


def _alt(ws):
    """The punctuation as alternatives of a pattern, the longest first."""
    return "|".join(re.escape(w) for w in sorted(ws, key=len, reverse=True))


def _quiet(listed, operators):
    """The rules for the punctuation of a grammar: an operator, or punctuation."""
    return [
        (_alt(operators), Operator),
        (_alt([p for p in listed if p not in operators and p != "@"]), Punctuation),
        (r"@", Punctuation),
        (r"…", Punctuation),
    ]


_COMMON = [
    (r"\s+", Whitespace),
    (r"//[^\n\r]*", Comment.Single),
    # `@id("…")`
    (rf"(@)(\s*)({IDENT})", bygroups(Name.Decorator, Whitespace, Name.Decorator)),
]


class CedarLexer(RegexLexer):
    """Cedar's policies."""

    name = "Cedar"
    aliases = ["cedar"]
    filenames = ["*.cedar"]

    tokens = {
        "root": _COMMON + [
            (STRING, String.Double),
            (rf"\?{IDENT}", Name.Variable),
            (r"[0-9]+", Number.Integer),
            # an attribute, or a method of an extension
            (rf"(\.)(\s*)({IDENT})(?=\s*\()", bygroups(Punctuation, Whitespace, Name.Function)),
            (rf"(\.)(\s*)({IDENT})", bygroups(Punctuation, Whitespace, Name)),
            (rf"(has)(\s+)({IDENT})\b", bygroups(Operator.Word, Whitespace, Name)),
            (r"(is)\b", Operator.Word, "type"),
            # a key of a record: `{ amount: 50 }`
            (rf"{IDENT}(?=\s*:(?!:))", Name),
            (words(CONSTANTS, prefix=r"\b", suffix=r"\b"), Keyword.Constant),
            (words(OPERATOR_WORDS, prefix=r"\b", suffix=r"\b"), Operator.Word),
            (words(POLICY_KEYWORDS, prefix=r"\b", suffix=r"\b"), Keyword),
            include("names"),
            *_quiet(POLICY_PUNCTUATION, POLICY_OPERATORS),
        ],
        # A name: an entity type before an id (`Shop::Role::"clerk"`), the namespaces before it, a
        # function of an extension, or a name.
        "names": [
            (rf"{IDENT}(?=\s*::\s*\")", Name.Class),
            (rf"{IDENT}(?=\s*::)", Name.Namespace),
            (rf"{IDENT}(?=\s*\()", Name.Function),
            (IDENT, Name),
        ],
        # After `is`: an entity type, `Shop::Order`.
        "type": [
            (r"\s+", Whitespace),
            (r"//[^\n\r]*", Comment.Single),
            (rf"{IDENT}(?=\s*::)", Name.Namespace),
            (r"::", Punctuation),
            (IDENT, Name.Class, "#pop"),
            default("#pop"),
        ],
    }


class CedarSchemaLexer(RegexLexer):
    """Cedar's schemas, in the Cedar schema format."""

    name = "Cedar schema"
    aliases = ["cedarschema"]
    filenames = ["*.cedarschema"]

    tokens = {
        "root": _COMMON + [
            # an attribute: `status: String`, `refund_band?: String`, `"the name": Long`
            (rf"({IDENT})(\s*)(\?)?(\s*)(:)(?!:)", bygroups(Name, Whitespace, Punctuation, Whitespace, Punctuation)),
            (rf"({STRING})(\s*)(\?)?(\s*)(:)(?!:)", bygroups(String.Double, Whitespace, Punctuation, Whitespace, Punctuation)),
            (STRING, String.Double),
            (r"(namespace)\b", Keyword, "namespace"),
            (r"(entity)\b", Keyword, "declared"),
            (rf"(type)(\s+)({IDENT})", bygroups(Keyword, Whitespace, Name.Class)),
            (r"(action)\b", Keyword, "declared"),
            (r"(appliesTo)(\s*)(\{)", bygroups(Keyword, Whitespace, Punctuation), "applies"),
            (words(SCHEMA_KEYWORDS, prefix=r"\b", suffix=r"\b"), Keyword),
            include("types"),
            (r"[0-9]+", Number.Integer),
            *_quiet(SCHEMA_PUNCTUATION, SCHEMA_OPERATORS),
        ],
        # A type: `Long`, `Set<String>`, `Shop::Order`, `__cedar::ipaddr`.
        "types": [
            (rf"({_alt(SET)})(\s*)(<)", bygroups(Keyword.Type, Whitespace, Punctuation)),
            (words(PRIMITIVE_TYPES, prefix=r"\b", suffix=r"\b"), Keyword.Type),
            (rf"{IDENT}(?=\s*::)", Name.Namespace),
            (IDENT, Name.Class),
        ],
        # After `namespace`: `Shop`, `Shop::Orders`.
        "namespace": [
            (r"\s+", Whitespace),
            (r"//[^\n\r]*", Comment.Single),
            (r"::", Punctuation),
            (IDENT, Name.Namespace),
            default("#pop"),
        ],
        # After `entity` and `action`: the names declared, a `,` between them.
        "declared": [
            (r"\s+", Whitespace),
            (r"//[^\n\r]*", Comment.Single),
            (rf"({IDENT})(\s*)(,)", bygroups(Name.Class, Whitespace, Punctuation)),
            (rf"({STRING})(\s*)(,)", bygroups(String.Double, Whitespace, Punctuation)),
            (STRING, String.Double, "#pop"),
            (IDENT, Name.Class, "#pop"),
            default("#pop"),
        ],
        # `appliesTo { principal: …, resource: …, context: { … } }`.
        "applies": [
            (rf"({_alt(APPLIES)})(\s*)(:)", bygroups(Keyword, Whitespace, Punctuation)),
            (r"\{", Punctuation, "record"),
            (r"\}", Punctuation, "#pop"),
            *_COMMON,
            (STRING, String.Double),
            include("types"),
            *_quiet(SCHEMA_PUNCTUATION, SCHEMA_OPERATORS),
        ],
        # A record's attributes: `{ amount: Long, refund_band?: String }`.
        "record": [
            (r"\{", Punctuation, "#push"),
            (r"\}", Punctuation, "#pop"),
            include("root"),
        ],
    }


def _register():
    """Put the lexers where `get_lexer_by_name("cedar")` looks: the table and the cache both, so the
    lookup never has to import this module a second time by name."""
    import pygments.lexers as pl

    for cls in (CedarLexer, CedarSchemaLexer):
        pl.LEXERS[cls.__name__] = (__name__, cls.name, tuple(cls.aliases), tuple(cls.filenames), ())
        pl._lexer_cache[cls.name] = cls


_register()


def makeExtension(**kwargs):
    """The Markdown extension that exists only so that this module gets imported."""
    from markdown.extensions import Extension

    class CedarHighlighting(Extension):
        def extendMarkdown(self, md):
            pass

    return CedarHighlighting(**kwargs)

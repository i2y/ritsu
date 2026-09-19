"""Syntax highlighting for `.rule` sources on the site.

A ```rule fence is coloured at build time by Pygments, which is what every other code block
on this site already goes through: the theme's light and dark palettes are written against
Pygments' own class names, so the colours follow the reader's scheme, the copy button and
the line anchors keep working, and nothing runs in the browser.

Pygments finds a lexer by looking through its own table, so the module registers itself
there on import. Importing it is arranged by naming it as a Markdown extension in
zensical.toml; the extension itself adds nothing to Markdown, and exists only so that
something imports this file. `build.sh` puts this directory on PYTHONPATH.

What the colours are trying to say, in the order a reader needs them:

    keyword    the word that starts a line - the shape of the file
    function   the name being declared, and min / max
    constant   the fixed vocabulary of values: types, policies, rounding, true / none,
               and what a fold is made of (over, next, take_unique, empty …)
    number     a literal, with its unit attached - the business values in a table
    operator   a comparison in a cell, and the arithmetic
    quiet      the table's own furniture: | -> [ ] : , ( )

The vocabulary is `src/kw.rs` and nothing else (PLAN §0). The lists below are a second copy
of it in another language, which is exactly how a vocabulary rots, so `tests/website.rs`
reads this file and holds every list to `rulec::kw`.
"""

from pygments.lexer import RegexLexer, bygroups, words
from pygments.token import (Comment, Keyword, Name, Number, Operator, Punctuation, String,
                            Text, Whitespace)

__all__ = ["RuleLexer"]

# --- the vocabulary, from src/kw.rs -----------------------------------------

# Words that start a line. The seven that name something push `decl`, so that what follows
# is coloured as a declaration rather than as a bare word.
HEAD_NAMED = ("rule", "enum", "group", "derive", "define", "table", "result", "elements", "fold", "count", "sequence", "clause", "source", "apply")
HEAD_PLAIN = ("description", "import", "inputs", "outputs", "policy", "overrides", "examples", "constraint")

MODIFIERS = ("range", "round", "contract_only", "default", "step")
CLAUSE = ("when", "then", "always")
# The body of an `apply`: what the callee's definitions it leaves out are introduced with.
APPLY = ("except",)
SOURCE = ("law", "file", "asof")
TYPES = ("money", "mass", "length", "rate", "number", "bool", "string", "date")
TAX = ("incl_tax", "excl_tax")
POLICIES = ("unique", "first")
ROUNDING = ("up", "down", "half_up", "half_down", "half_even")
CONSTANTS = ("true", "false", "none")
# What a `fold` and a `count` are made of: the connectors of their headings and the arms.
ARMS = ("over", "where", "next", "stop", "with", "take_unique", "take_first", "keep_max",
        "by", "empty", "exhausted", "held")
FUNCTIONS = ("min", "max")
NAMESPACE = "std"

# The units a literal may carry (§2.1). Longest first, so `kg` is not read as `g` and `cm`
# is not read as `m`; `万` and `億` are the spoken groupings that 1000万円 is written with.
UNITS = r"(?:\s*(?:万|億))?(?:円|銭|kg|g|cm|m|%)"


class RuleLexer(RegexLexer):
    """The rulec decision-table language."""

    name = "rulec"
    aliases = ["rule", "rulec"]
    filenames = ["*.rule"]

    tokens = {
        "root": [
            (r"[^\S\n]+", Whitespace),
            (r"\n", Whitespace),
            (r"#[^\n]*", Comment.Single),
            (r'"[^"\n]*"', String.Double),
            # A line head, and the name it declares.
            (words(HEAD_NAMED, prefix=r"^", suffix=r"\b"), Keyword, "decl"),
            (words(HEAD_PLAIN, prefix=r"^", suffix=r"\b"), Keyword),
            # `import std/都道府県`
            (rf"\b({NAMESPACE})(/)(\S+)", bygroups(Name.Builtin, Punctuation, Name)),
            # The ASCII alias (§1.3), which is the public name in the generated code.
            (r"(\()([A-Za-z_][A-Za-z0-9_]*)(\))",
             bygroups(Punctuation, Name.Attribute, Punctuation)),
            (words(MODIFIERS, suffix=r"\b"), Keyword),
            (words(TYPES + TAX + POLICIES + ROUNDING + CONSTANTS + ARMS + CLAUSE + APPLY + SOURCE, suffix=r"\b"),
             Name.Builtin),
            (words(FUNCTIONS, suffix=r"\b"), Name.Function),
            (r"\bnot\b", Operator.Word),
            (r"\bv\d+\b", Name.Constant),
            # A date is a literal like any other, and has to be tried before the number
            # rule or `2026` would be taken on its own.
            (r"\d{4}-\d{2}-\d{2}", Number.Integer),
            (rf"\d[\d,]*(?:\.\d+)?{UNITS}?", Number.Integer),
            # The table's own furniture, kept quiet so that a table reads as a table.
            (r"->|→", Punctuation),
            (r"[|\[\]:,()]", Punctuation),
            (r"<=|>=|≦|≧|<|>|=|\+|-|−|×|÷|\*|/", Operator),
            # Anything else is a name: the business words, which are most of the file. The
            # operator characters are excluded so that `商品合計-値引` is three tokens.
            (r"[^\s|:()\[\],#\"=<>+\-*/×÷−→]+", Name),
        ],
        # What one of the seven declaring words names, up to the alias or the end of line.
        "decl": [
            (r"[^\S\n]+", Whitespace),
            (r"(?=\n)", Text, "#pop"),
            (r"[^\s(|]+", Name.Class, "#pop"),
            (r"", Text, "#pop"),
        ],
    }


def _register():
    """Put the lexer where `get_lexer_by_name("rule")` looks. Both the table and the cache
    are filled, so the lookup never has to import this module a second time by name."""
    import pygments.lexers as pl

    pl.LEXERS["RuleLexer"] = (
        __name__, RuleLexer.name, tuple(RuleLexer.aliases), tuple(RuleLexer.filenames), ()
    )
    pl._lexer_cache[RuleLexer.name] = RuleLexer


_register()


# --- the Markdown extension that exists only to be imported -----------------

def makeExtension(**kwargs):
    from markdown.extensions import Extension

    class RuleHighlighting(Extension):
        def extendMarkdown(self, md):
            pass

    return RuleHighlighting(**kwargs)

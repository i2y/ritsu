"""Syntax highlighting for `.book` sources on the site.

A ```book fence is coloured at build time by Pygments, which every other code block on the site
goes through too: the theme's light and dark palettes are written against Pygments' class
names, so the colours follow the reader's scheme, and nothing runs in the browser.

Pygments finds a lexer by looking through its own table, so this module registers itself there
on import. zensical.toml names it as a Markdown extension so that something imports it; the
extension adds nothing to Markdown. build.sh puts this directory on PYTHONPATH.

What the colours say:

    keyword    the language's words - the shape of the file
    class      the name a declaration gives (a book, a unit, an account, a transfer)
    exception  the reason a bound refuses with, after `refused as`
    type       what a `:` gives a type: `string`, or a unit of the book
    builtin    the words of a duration, which are keywords only after `expires after <n>`
    string     a description
    quiet      the punctuation: ( ) : ,

KEYWORDS is `src/syntax.rs`'s KEYWORDS (crates/chobo), word for word and in its order, and
crates/chobo/tests/docs.rs holds it there: a second copy of a vocabulary is how a vocabulary rots.
"""

from pygments.lexer import RegexLexer, bygroups
from pygments.token import Comment, Keyword, Name, Number, Punctuation, String, Whitespace

__all__ = ["BookLexer"]

KEYWORDS = ("book", "description", "unit", "account", "transfer", "key", "pending", "move",
    "scale", "incl_tax", "excl_tax", "outside", "at least", "at most", "refused as",
    "expires after", "never expires", "from", "to", "string", "second", "seconds", "minute",
    "minutes", "hour", "hours", "day", "days")

# The words of a duration are keywords only after `expires after <n>`; anywhere else they are
# names (a parameter called `day`).
DURATIONS = ("second", "seconds", "minute", "minutes", "hour", "hours", "day", "days")
TYPES = ("string",)
# The words that give a name to what follows them.
DECLARING = ("book", "unit", "account", "transfer")
# A keyword of two words may have any spaces between them, as the parser reads it.
WORDS = [k for k in KEYWORDS if k not in DURATIONS + TYPES + DECLARING]


def _word(k):
    return r"\b" + r"\s+".join(k.split(" ")) + r"\b"


class BookLexer(RegexLexer):
    """chobo's language of books."""

    name = "chobo"
    aliases = ["book", "chobo"]
    filenames = ["*.book"]

    tokens = {
        "root": [
            (r"[^\S\n]+", Whitespace),
            (r"\n", Whitespace),
            (r"#[^\n]*", Comment.Single),
            (r'"(?:[^"\\\n]|\\.)*"', String.Double),
            (r"(" + "|".join(DECLARING) + r")(\s+)([^\s(:#]+)", bygroups(Keyword, Whitespace, Name.Class)),
            (r"(refused)(\s+)(as)(\s+)([^\s#]+)", bygroups(Keyword, Whitespace, Keyword, Whitespace, Name.Exception)),
            (r"(expires)(\s+)(after)(\s+)(\d+)(\s+)(" + "|".join(DURATIONS) + r")\b",
             bygroups(Keyword, Whitespace, Keyword, Whitespace, Number, Whitespace, Name.Builtin)),
            ("(" + "|".join(_word(k) for k in sorted(WORDS, key=len, reverse=True)) + ")", Keyword),
            # what a `:` gives a type: `string`, or a unit the book declares
            (r"(:)(\s*)([^\s(),:#\"…]+)", bygroups(Punctuation, Whitespace, Keyword.Type)),
            (r"\bstring\b", Keyword.Type),
            (r"\bv\d+\b", Name.Constant),
            (r"-?\d+(?:\.\d+)?", Number),
            (r"[(),:]", Punctuation),
            (r"…", Punctuation),
            # Anything else is a name: the book's own words, in any script.
            (r"[^\s#\"(),:…]+", Name),
        ],
    }


def _register():
    """Put the lexer where `get_lexer_by_name("book")` looks: the table and the cache both, so
    the lookup never has to import this module a second time by name."""
    import pygments.lexers as pl

    pl.LEXERS["BookLexer"] = (__name__, BookLexer.name, tuple(BookLexer.aliases), tuple(BookLexer.filenames), ())
    pl._lexer_cache[BookLexer.name] = BookLexer


_register()


def makeExtension(**kwargs):
    """The Markdown extension that exists only so that this module gets imported."""
    from markdown.extensions import Extension

    class BookHighlighting(Extension):
        def extendMarkdown(self, md):
            pass

    return BookHighlighting(**kwargs)

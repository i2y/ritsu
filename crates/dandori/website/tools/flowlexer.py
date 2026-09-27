"""Syntax highlighting for `.flow` sources on the site.

A ```flow fence is coloured at build time by Pygments, which every other code block on the site
goes through too: the theme's light and dark palettes are written against Pygments' class
names, so the colours follow the reader's scheme, and nothing runs in the browser.

Pygments finds a lexer by looking through its own table, so this module registers itself there
on import. zensical.toml names it as a Markdown extension so that something imports it; the
extension adds nothing to Markdown. build.sh puts this directory on PYTHONPATH.

What the colours say:

    keyword    the language's reserved words - the shape of the file
    class      the name a declaration gives (a workflow, a task, a record, an enum)
    type       the types, and the units' kinds
    builtin    the words the parser reads only in their place (`agent`, `every 5 seconds`,
               `on failure`) and the HTTP methods
    string     instructions, URLs and the text a flow writes
    quiet      the punctuation: ( ) { } [ ] : , . ?

KEYWORDS is `src/syntax.rs`'s KEYWORDS, word for word, and tests/docs.rs holds it there: a
second copy of a vocabulary is how a vocabulary rots.
"""

from pygments.lexer import RegexLexer, bygroups, words
from pygments.token import Comment, Keyword, Name, Number, Operator, Punctuation, String, Whitespace

__all__ = ["FlowLexer"]

KEYWORDS = ("workflow", "description", "kind", "use", "rule", "from", "enum", "record",
    "inputs", "outputs", "task", "case", "follows", "flow", "on", "let", "match", "wait",
    "repeat", "break", "succeed", "fail", "leaving", "lambda", "http", "aws", "connection",
    "queue", "machine", "durable", "function", "image", "template", "errors", "retry",
    "timeout", "key", "idempotent", "starts", "sends", "observes", "refused", "callback",
    "held", "external", "state", "then", "true", "false", "until", "pass", "for", "in", "at",
    "most", "parallel", "yield", "some", "none", "list", "json", "range", "openapi", "smithy",
    "proto", "connect", "url")

# The words the parser reads only where they belong, so they stay usable as names elsewhere.
CONTEXTUAL = ("agent", "claude", "model", "every", "times", "backoff", "second", "seconds",
              "minute", "minutes", "hour", "hours", "day", "days", "event", "failure", "cancel",
              "local", "express", "form", "when", "effort")
TYPES = ("int", "string", "bool", "timestamp", "money", "mass", "length", "area", "volume",
         "duration", "temperature", "sound", "rate", "incl_tax", "excl_tax")
METHODS = ("GET", "POST", "PUT", "PATCH", "DELETE")
CONSTANTS = ("true", "false", "none")
# The words that give a name to what follows them.
DECLARING = ("workflow", "task", "record", "enum")


class FlowLexer(RegexLexer):
    """dandori's workflow language."""

    name = "dandori"
    aliases = ["flow", "dandori"]
    filenames = ["*.flow"]

    tokens = {
        "root": [
            (r"[^\S\n]+", Whitespace),
            (r"\n", Whitespace),
            (r"#[^\n]*", Comment.Single),
            (r'"(?:[^"\\\n]|\\.)*"', String.Double),
            (r"(" + "|".join(DECLARING) + r")(\s+)([^\s(]+)", bygroups(Keyword, Whitespace, Name.Class)),
            (words(CONSTANTS, prefix=r"\b", suffix=r"\b"), Keyword.Constant),
            (words(KEYWORDS, prefix=r"\b", suffix=r"\b"), Keyword),
            (words(TYPES, prefix=r"\b", suffix=r"\b"), Keyword.Type),
            (words(CONTEXTUAL + METHODS, prefix=r"\b", suffix=r"\b"), Name.Builtin),
            (r"\bv\d+\b", Name.Constant),
            (r"\d+(?:\.\d+)?", Number),
            (r"->|=>|<-|>=|<=", Operator),
            (r"[=<>]", Operator),
            (r"[(){}\[\]:,.?|]", Punctuation),
            (r"…", Punctuation),
            # Anything else is a name: the flow's own words, in any script.
            (r"[^\s#\"(){}\[\]:,.?|=<>…-]+", Name),
            (r"-", Operator),
        ],
    }


def _register():
    """Put the lexer where `get_lexer_by_name("flow")` looks: the table and the cache both, so
    the lookup never has to import this module a second time by name."""
    import pygments.lexers as pl

    pl.LEXERS["FlowLexer"] = (__name__, FlowLexer.name, tuple(FlowLexer.aliases), tuple(FlowLexer.filenames), ())
    pl._lexer_cache[FlowLexer.name] = FlowLexer


_register()


def makeExtension(**kwargs):
    """The Markdown extension that exists only so that this module gets imported."""
    from markdown.extensions import Extension

    class FlowHighlighting(Extension):
        def extendMarkdown(self, md):
            pass

    return FlowHighlighting(**kwargs)

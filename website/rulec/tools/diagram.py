"""What the front page's diagrams share, so that they look drawn by one hand.

Two scripts draw pictures for the front page - make_flow.py (who makes
what, and who takes it) and make_overview.py (what the tool does to a
table). Their palettes, fonts, text metrics and the two shapes live here,
so a colour or a corner radius changed in one place changes in both and
the second picture cannot drift away from the first.

Nothing here decides a layout; that is each script's own business.

The picture vocabulary has two kinds of shape and nothing else:

  * actors - the agent, rulec, a person - are cards: a filled surface with a
    coloured bar along the top and a bold name;
  * artifacts - the things that move between them - are sheets: a thin outline
    with a folded corner, the silhouette of a document.

There are three colours for the three kinds of path: grey for what enters
and leaves the whole system, indigo for the loop between the agent and
rulec, amber for anything that needs a person.
"""

# --- palettes ---------------------------------------------------------------
#
# "actor" is the card surface, "sheet" the paper of an artifact. On the light
# page the sheets are a shade greyer than the cards, on the dark page a shade
# lighter: either way the two kinds of shape differ in fill as well as in
# outline, so the distinction survives a small rendering. The amber is the
# second accent: everything on the path through a person carries it.

DARK = dict(
    ink="#e8ebf4", dim="#98a1b8",
    actor="#171b24", actor_edge="#2b3242",
    sheet="#1e2331", sheet_edge="#3a4257", fold="#2b3242",
    accent="#8e9cff", human="#e0b45c", neutral="#6b7590",
)
LIGHT = dict(
    ink="#10131a", dim="#5b6478",
    actor="#ffffff", actor_edge="#dfe3ee",
    sheet="#f5f6fa", sheet_edge="#cdd3e2", fold="#e3e7f1",
    accent="#4457d8", human="#9c6508", neutral="#8b94ab",
)

FONT = ('-apple-system, BlinkMacSystemFont, "Hiragino Sans", "Noto Sans JP", '
        '"Yu Gothic", Meiryo, "Segoe UI", Roboto, sans-serif')
MONO = 'ui-monospace, SFMono-Regular, Menlo, Consolas, monospace'

CARD_RX, BAR = 10, 5                 # card corners; the coloured bar on top
FOLD, PAD = 12, 9                    # the sheet's dog-ear; text inset in both shapes
LINE_W = 1.8


# --- measuring text ---------------------------------------------------------

def width(s, size, weight=400, mono=False):
    """A rough advance width - enough to catch a line that will not fit.

    Ideographs and kana are one em square; Latin averages about half an em,
    more for capitals, less for punctuation; bold runs a little wider. The
    renders are still looked at, this only turns an overflow into an error
    instead of a picture with text sticking out of a box."""
    if mono:
        return sum(1.0 if ord(ch) > 0x2E7F else 0.6 for ch in s) * size
    bold = 1.08 if weight >= 600 else 1.0
    w = 0.0
    for ch in s:
        o = ord(ch)
        if o > 0x2E7F:                   # CJK, kana, full-width forms, 「」
            w += 1.0
        elif ch in "→—":                 # arrows and dashes are an em wide too
            w += 1.0
        elif ch == " ":
            w += 0.27
        elif ch in ",.;:'!|“”":
            w += 0.28
        elif ch in "()[]-=&":
            w += 0.36 * bold
        elif ch.isupper():
            w += 0.66 * bold
        elif ch.isdigit():
            w += 0.56 * bold
        else:
            w += 0.5 * bold
    return w * size


def fit(s, size, avail, weight=400, mono=False):
    if width(s, size, weight, mono) > avail:
        raise ValueError(f"{s!r} needs {width(s, size, weight, mono):.0f}, has {avail}")


def esc(s):
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


# --- primitives -------------------------------------------------------------

def text(x, y, s, size, fill, weight=400, anchor="middle", mono=False, extra=""):
    fam = f' font-family="{MONO}"' if mono else ""
    return (f'<text x="{x}" y="{y}" font-size="{size}" font-weight="{weight}" '
            f'text-anchor="{anchor}" fill="{fill}"{fam}{extra}>{esc(s)}</text>')


def card(rect, spec, bar, c, cid):
    """An actor. The bar along the top is the colour of the path it belongs
    to, which is the only decoration a card gets; the bold name does the rest.
    A clipPath rounds the bar's corners with the card's."""
    x, y, w, h = rect
    name, lines = spec
    cx = x + w / 2
    fit(name, 17, w - 2 * PAD, 700)
    o = [f'<clipPath id="{cid}"><rect x="{x}" y="{y}" width="{w}" height="{h}" '
         f'rx="{CARD_RX}"/></clipPath>',
         f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{CARD_RX}" '
         f'fill="{c["actor"]}" stroke="{c["actor_edge"]}"/>',
         f'<rect x="{x}" y="{y}" width="{w}" height="{BAR}" fill="{bar}" '
         f'clip-path="url(#{cid})"/>',
         text(cx, y + 43, name, 17, c["ink"], 700)]
    for i, line in enumerate(lines):
        fit(line, 12.5, w - 2 * PAD)
        o.append(text(cx, y + 72 + i * 19, line, 12.5, c["dim"]))
    if 72 + (len(lines) - 1) * 19 + 8 > h:
        raise ValueError(f"{name}: {len(lines)} lines do not fit in {h}")
    return o


def sheet_frame(rect, c):
    """The outline of an artifact: a document silhouette, thin outline, folded
    corner. The fold is what says "a thing that is handed over" at any size,
    and it is the one feature no card has."""
    x, y, w, h = rect
    f = FOLD
    return [f'<path d="M{x},{y} H{x + w - f} L{x + w},{y + f} V{y + h} H{x} Z" '
            f'fill="{c["sheet"]}" stroke="{c["sheet_edge"]}" stroke-linejoin="round"/>',
            f'<path d="M{x + w - f},{y} V{y + f} H{x + w} Z" fill="{c["fold"]}" '
            f'stroke="{c["sheet_edge"]}" stroke-linejoin="round"/>']


def sheet_title(rect, title, tag, c):
    """The first line of a sheet: a bold title and, at the top right, an
    optional tag - the file format, set in monospace."""
    x, y, w, h = rect
    inner = w - 2 * PAD
    # A title with a tag shares its line with it; leave the tag's width free.
    fit(title, 13, inner - (width(tag, 9.5, mono=True) + 8 if tag else 0), 600)
    o = [text(x + PAD, y + 22, title, 13, c["ink"], 600, "start")]
    if tag:
        o.append(text(x + w - PAD, y + 22, tag, 9.5, c["dim"], anchor="end", mono=True))
    return o


def sheet(rect, spec, c):
    """An artifact: frame, title, tag, and quiet lines of text underneath."""
    x, y, w, h = rect
    title, lines = spec[0], spec[1]
    tag = spec[2] if len(spec) > 2 else None
    inner = w - 2 * PAD
    o = sheet_frame(rect, c) + sheet_title(rect, title, tag, c)
    for i, line in enumerate(lines):
        fit(line, 11.5, inner)
        o.append(text(x + PAD, y + 40 + i * 15, line, 11.5, c["dim"], anchor="start"))
    if 40 + (len(lines) - 1) * 15 + 6 > h:
        raise ValueError(f"{title}: {len(lines)} lines do not fit in {h}")
    return o


def arrow(x1, y1, x2, y2, kind, c):
    """kind names the path - "neutral", "accent" or "human" - which picks both
    the stroke and the matching arrowhead."""
    return (f'<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{c[kind]}" '
            f'stroke-width="{LINE_W}" marker-end="url(#head-{kind})"/>')


def marker(kind, c):
    # userSpaceOnUse keeps the head the same size whatever the stroke width.
    return (f'<marker id="head-{kind}" viewBox="0 0 10 10" refX="9" refY="5" '
            f'markerWidth="9" markerHeight="9" markerUnits="userSpaceOnUse" '
            f'orient="auto"><path d="M0,1 L9,5 L0,9 Z" fill="{c[kind]}"/></marker>')

"""Draw the chart on the home page: the chart of how things move that `chobo doc --format html`
draws of a book, cut out of the page as it is and set on its own, in the page's colours for the
dark and the light scheme.

    $ python3 tools/make_chart.py [<chobo>]      # in website/chobo; <chobo> is the binary to run

Nothing on the picture is drawn here: the boxes, the arrows, the words and where each goes are the
SVG chobo writes into the page, kept byte for byte between the `<svg>` and the `</svg>` it writes,
and crates/chobo/tests/docs.rs holds the files to what `chobo doc` draws now. What this adds is
what the page gives the chart around it: the colours its style sheet gives the classes of the
chart, the font, and a background the size of the chart. Run it again after a change to `doc`, or
to the books below; the SVGs are committed, so building the site needs nothing but Zensical.
"""

import os
import re
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SITE = os.path.dirname(HERE)
CRATE = os.path.join(SITE, "..", "..", "crates", "chobo")

# The book of the home page, in English and as its Japanese version, and the file each is drawn
# into (`-light` beside each, for the light scheme).
CHARTS = [
    ("examples/refunds/refunds.book", "en", "docs/images/chart.svg"),
    ("examples/refunds/refunds.ja.book", "ja", "docs/images/chart-ja.svg"),
]


def page(chobo, book, lang):
    """What `chobo doc <book> --format html --lang <lang>` writes, run where the crate is."""
    out = subprocess.run([chobo, "doc", book, "--format", "html", "--lang", lang], cwd=CRATE, capture_output=True, text=True)
    if out.returncode != 0:
        sys.exit(f"chobo doc {book} failed:\n{out.stderr}")
    return out.stdout


def chart(html):
    """The chart of how things move, as the page holds it: `<svg class="flow" …>…</svg>`."""
    start = html.index('<svg class="flow"')
    return html[start : html.index("</svg>", start) + len("</svg>")]


def palettes(html):
    """The page's colours: its variables for the light scheme, and for the dark one."""
    style = html[html.index("<style>") : html.index("</style>")]
    light = dict(re.findall(r"--([\w-]+):\s*([^;]+);", re.search(r"^:root \{([^}]*)\}", style, re.M).group(1)))
    dark = dict(re.findall(r"--([\w-]+):\s*([^;]+);", re.search(r'^:root\[data-theme="dark"\] \{([^}]*)\}', style, re.M).group(1)))
    return light, dark


def rules(html):
    """The rules of the page's style sheet that draw the chart, and the font of its body."""
    style = html[html.index("<style>") : html.index("</style>")]
    lines = [l for l in style.splitlines() if re.match(r"(svg|\.node|\.edge|\.head|\.label)\b", l)]
    font = re.search(r"^body \{[^}]*font: [^ ]+ ([^;]+);", style, re.M).group(1)
    return lines, font


def standalone(svg, lines, font, colours):
    """The chart as a file of its own: the page's rules with its colours in place of the variables,
    and a background, put into the chart after its `<title>` (which stays its first child), and the
    namespace a file of SVG names on its `<svg>`. Take those two away, and what is left is the
    chart as the page holds it."""
    paint = lambda l: re.sub(r"var\(--([\w-]+)\)", lambda m: colours[m.group(1)], l)
    svg = svg.replace("<svg ", '<svg xmlns="http://www.w3.org/2000/svg" ', 1)
    at = svg.index("</title>") + len("</title>")
    style = "\n".join([f"svg {{ font-family: {font}; }}", ".cb-bg { fill: " + colours["bg"] + "; }"] + [paint(l) for l in lines])
    return f'{svg[:at]}\n<style>\n{style}\n</style>\n<rect class="cb-bg" width="100%" height="100%" rx="8"/>{svg[at:]}\n'


def main():
    chobo = sys.argv[1] if len(sys.argv) > 1 else shutil.which("chobo")
    if not chobo:
        sys.exit("give the chobo binary to run: python3 tools/make_chart.py <chobo>")
    chobo = os.path.abspath(chobo) if os.sep in chobo else chobo
    for book, lang, target in CHARTS:
        html = page(chobo, book, lang)
        svg = chart(html)
        light, dark = palettes(html)
        lines, font = rules(html)
        for colours, name in [(dark, target), (light, target.replace(".svg", "-light.svg"))]:
            with open(os.path.join(SITE, name), "w", encoding="utf-8") as f:
                f.write(standalone(svg, lines, font, colours))
            print(f"wrote {name} from {book} ({lang})")


if __name__ == "__main__":
    main()

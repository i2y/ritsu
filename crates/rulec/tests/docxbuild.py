# Writes a minimal but real .docx from a JSON spec on stdin (test fixture builder).
#
# The shape is the one Word writes: a ZIP whose `word/document.xml` holds `w:tbl` elements
# under `w:body`, with the `w:` prefix, `gridSpan` on a cell that spans columns and `vMerge`
# on the continuation of a vertical merge.
import json, sys, zipfile

spec = json.load(sys.stdin)
out = spec["out"]
comp = zipfile.ZIP_STORED if spec.get("compress") == "stored" else zipfile.ZIP_DEFLATED


def esc(s):
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace('"', "&quot;")


def cell(c):
    # A cell is either a string, or {"text":…, "span":n, "vmerge":"restart"|"continue"}.
    if isinstance(c, str):
        c = {"text": c}
    props = ""
    if c.get("span", 1) > 1:
        props += f'<w:gridSpan w:val="{c["span"]}"/>'
    if c.get("vmerge") == "restart":
        props += '<w:vMerge w:val="restart"/>'
    elif c.get("vmerge") == "continue":
        props += "<w:vMerge/>"
    props = f"<w:tcPr>{props}</w:tcPr>" if props else ""
    # Several paragraphs in one cell, so that the reader's word separation is exercised.
    paras = "".join(f"<w:p><w:r><w:t>{esc(t)}</w:t></w:r></w:p>" for t in str(c["text"]).split("\n"))
    return f"<w:tc>{props}{paras or '<w:p/>'}</w:tc>"


def table(rows):
    trs = "".join("<w:tr>" + "".join(cell(c) for c in r) + "</w:tr>" for r in rows)
    return f'<w:tbl><w:tblPr><w:tblStyle w:val="a"/></w:tblPr>{trs}</w:tbl>'


body = ""
for block in spec["blocks"]:
    if block["kind"] == "p":
        body += f'<w:p><w:r><w:t>{esc(block["text"])}</w:t></w:r></w:p>'
    elif block["kind"] == "table":
        body += table(block["rows"])

parts = {
    "[Content_Types].xml": '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
    '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
    '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>'
    '<Default Extension="xml" ContentType="application/xml"/>'
    '<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>'
    "</Types>",
    "_rels/.rels": '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
    '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
    '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>'
    "</Relationships>",
    "word/document.xml": '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
    '<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">'
    f"<w:body>{body}<w:sectPr/></w:body></w:document>",
}

with zipfile.ZipFile(out, "w", comp) as z:
    for name, text in parts.items():
        z.writestr(name, text)
print(out)

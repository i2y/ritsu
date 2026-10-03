# Writes a minimal but real .xlsx from a JSON spec on stdin (test fixture builder).
import json, sys, zipfile, datetime

spec = json.load(sys.stdin)
out = spec["out"]
comp = zipfile.ZIP_STORED if spec.get("compress") == "stored" else zipfile.ZIP_DEFLATED

shared, index = [], {}
def sid(text):
    if text not in index:
        index[text] = len(shared)
        shared.append(text)
    return index[text]

def esc(s):
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace('"', "&quot;")

def col(n):
    s = ""
    while n:
        n, r = divmod(n - 1, 26)
        s = chr(65 + r) + s
    return s

# style 0 plain, 1 yen, 2 percent, 3 date, 4 japanese era-ish date
STYLE = {"n": 0, "yen": 1, "pct": 2, "date": 3, "jdate": 4, "date1904": 3}

def cell(ref, kind, value):
    if kind == "s":
        return f'<c r="{ref}" t="s"><v>{sid(value)}</v></c>'
    if kind == "inline":
        return f'<c r="{ref}" t="inlineStr"><is><t>{esc(value)}</t></is></c>'
    if kind == "empty":
        return f'<c r="{ref}" s="0"/>'
    if kind in ("date", "jdate", "date1904"):
        d = datetime.date(*[int(x) for x in value.split("-")])
        epoch = datetime.date(1904, 1, 1) if kind == "date1904" else datetime.date(1899, 12, 30)
        value = str((d - epoch).days)
    return f'<c r="{ref}" s="{STYLE[kind]}"><v>{value}</v></c>'

sheets = spec["sheets"]
parts = {}
for i, sh in enumerate(sheets, start=1):
    rows = []
    for r, line in enumerate(sh["rows"], start=sh.get("first_row", 1)):
        cells = "".join(
            cell(f"{col(c)}{r}", k, v)
            for c, (k, v) in enumerate(line, start=sh.get("first_col", 1))
            if k != "gap"
        )
        rows.append(f'<row r="{r}">{cells}</row>')
    parts[f"xl/worksheets/sheet{i}.xml"] = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">'
        "<sheetData>" + "".join(rows) + "</sheetData></worksheet>"
    )

parts["[Content_Types].xml"] = (
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
    '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
    '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>'
    '<Default Extension="xml" ContentType="application/xml"/></Types>'
)
parts["_rels/.rels"] = (
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
    '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
    '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>'
    "</Relationships>"
)
parts["xl/workbook.xml"] = (
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
    '<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" '
    'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">'
    + ('<workbookPr date1904="1"/>' if spec.get("date1904") else "")
    + "<sheets>"
    + "".join(
        f'<sheet name="{esc(sh["name"])}" sheetId="{i}" r:id="rId{i}"/>'
        for i, sh in enumerate(sheets, start=1)
    )
    + "</sheets></workbook>"
)
parts["xl/_rels/workbook.xml.rels"] = (
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
    '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
    + "".join(
        f'<Relationship Id="rId{i}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{i}.xml"/>'
        for i in range(1, len(sheets) + 1)
    )
    + "</Relationships>"
)
parts["xl/styles.xml"] = (
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
    '<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">'
    '<numFmts count="3">'
    '<numFmt numFmtId="164" formatCode="#,##0&quot;円&quot;"/>'
    '<numFmt numFmtId="165" formatCode="0.0%"/>'
    '<numFmt numFmtId="166" formatCode="yyyy\\-mm\\-dd"/>'
    '<numFmt numFmtId="167" formatCode="[$-411]yyyy&quot;年&quot;m&quot;月&quot;d&quot;日&quot;"/>'
    "</numFmts>"
    '<cellStyleXfs count="1"><xf numFmtId="0"/></cellStyleXfs>'
    '<cellXfs count="5"><xf numFmtId="0"/><xf numFmtId="164"/><xf numFmtId="165"/>'
    '<xf numFmtId="166"/><xf numFmtId="167"/></cellXfs></styleSheet>'
)
# The strings are written last: the sheets fill the table while they are rendered.
parts["xl/sharedStrings.xml"] = (
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
    f'<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="{len(shared)}" uniqueCount="{len(shared)}">'
    + "".join(
        # Half of them carry a phonetic reading, the way Japanese Excel writes them.
        f"<si><t>{esc(t)}</t>" + (f"<rPh sb=\"0\" eb=\"1\"><t>ヨミ</t></rPh>" if i % 2 == 0 else "") + "</si>"
        for i, t in enumerate(shared)
    )
    + "</sst>"
)

with zipfile.ZipFile(out, "w", comp) as z:
    for name, body in parts.items():
        z.writestr(name, body)
print(out)

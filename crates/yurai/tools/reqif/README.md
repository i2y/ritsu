# The ReqIF schema, for the tests

`yurai export reqif` writes ReqIF 1.2. The tests validate what it writes against the
schema the OMG publishes, with xmllint and without the network:

```console
tools/reqif/fetch.sh
XML_CATALOG_FILES=tools/reqif/xsd/catalog.xml xmllint --nonet --noout \
  --schema tools/reqif/xsd/www.omg.org/spec/ReqIF/20110401/reqif.xsd out.reqif
```

`fetch.sh` takes 24 files: the OMG's `reqif.xsd` (ReqIF 1.2) and `driver.xsd`, and the W3C
schemas they import (`xml.xsd`, twice, and 20 modules of XHTML Modularization). It checks
each against the SHA-256 recorded in the script when yurai was designed (2026-10-03), puts
it at `xsd/<host>/<path>` of its URL, and writes `xsd/catalog.xml`, which points each URL at
its file. A file whose hash differs stops the script. `fetch.sh <dir>` puts them in
another directory; the tests then read `YURAI_REQIF_XSD=<dir>`.

The files belong to the OMG and the W3C, under their own terms, and are not kept in this
repository (`tools/reqif/xsd/` is in `.gitignore`). Without them the schema check prints a
`SKIP:` line; the tests yurai runs on its own — that every reference in the document lands
on an element of its kind — run either way.

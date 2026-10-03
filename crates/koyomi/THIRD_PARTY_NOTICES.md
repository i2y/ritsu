# Third-party notices

The repository carries copies of public data that the examples and the tests read, kept as their
sources served them, and koyomi carries a table made from a web standard. Each keeps the terms of
its source, as below. What `koyomi gen`, `koyomi doc` and `koyomi vectors` make from a copy (the
holidays written into generated code, the articles quoted on a page) is made from these copies,
and the head of every generated file names the copy, its digest and where it was taken from.

## The national holidays of Japan, from the Cabinet Office

[examples/calendars/data/syukujitsu.csv](examples/calendars/data/syukujitsu.csv) is, byte for byte,
the file 「昭和30年（1955年）から令和9年（2027年）国民の祝日（csv形式）」
(<https://www8.cao.go.jp/chosei/shukujitsu/syukujitsu.csv>) that the Cabinet Office of Japan
publishes on its page 「国民の祝日」について (<https://www8.cao.go.jp/chosei/shukujitsu/gaiyou.html>),
taken on 2026-10-02. Its SHA-256 is
`cec37a743c96995cdb9cb52b685c9003634682a9b0e1a640a6b9b96881fe964a`; it is in Shift_JIS, as served.

The Cabinet Office's terms of use for its site (内閣府ホームページ利用規約, as of 2025-03-25,
<https://www.cao.go.jp/notice/rule.html>) put its content under the Public Data License, version
1.0 (公共データ利用規約（第1.0版）, PDL1.0,
<https://www.digital.go.jp/resources/open_data/public_data_license_v1.0>), which lets anyone copy,
adapt and use the content, commercially too, and asks that the source be given, in the form of
its examples:

```
出典：内閣府ホームページ「「国民の祝日」について」（https://www8.cao.go.jp/chosei/shukujitsu/gaiyou.html）の「昭和30年（1955年）から令和9年（2027年）国民の祝日（csv形式）」（https://www8.cao.go.jp/chosei/shukujitsu/syukujitsu.csv）、PDL1.0（https://www.digital.go.jp/resources/open_data/public_data_license_v1.0）（2026年10月2日に取得）
```

PDL1.0 also asks that content which was edited or processed say so, and by whom, apart from the
source, and that it not be presented as if the government had made it as it is. The copy here is
not edited.

## The bank holidays of the United Kingdom, from GOV.UK

[examples/calendars/data/bank-holidays.json](examples/calendars/data/bank-holidays.json) is, byte
for byte, <https://www.gov.uk/bank-holidays.json>, taken on 2026-10-02. Its SHA-256 is
`538b3482c28b85ecd2db606a0d5ae6ad17248900b6498700ce0a48d26a3ecde6`.

GOV.UK says that all its content is available under the Open Government Licence v3.0, except
where otherwise stated
(<https://www.nationalarchives.gov.uk/doc/open-government-licence/version/3/>). The licence asks
that the source be acknowledged with the attribution statement the provider gives, or else with
this one, and, where possible, a link to the licence:

```
Contains public sector information licensed under the Open Government Licence v3.0.
```

## Articles of the Civil Code of Japan, from e-Gov

[examples/sources/law/129AC0000000089@2026-10-01/](examples/sources/law/129AC0000000089@2026-10-01)
holds, as e-Gov served them, the XML of Articles 140, 141, 142 and 143 of the Civil Code (民法,
明治二十九年法律第八十九号, law ID `129AC0000000089`) as of 2026-10-01, revision
`129AC0000000089_20260624_508AC0000000045` (in `revision.txt`), which the e-Gov Law Search's law
API, version 2, returned on 2026-10-02 (`law_data`, one element at a time). The files under
`tests/mutants/sources/law/` are the same copies, for the tests.

The terms of use of the e-Gov Law Search (e-Gov 法令検索 利用規約, <https://laws.e-gov.go.jp/terms/>)
put its content under PDL1.0 unless stated otherwise, and ask that the source be given, in the
form of their examples:

```
出典：e-Gov法令検索（https://laws.e-gov.go.jp/）の「民法」（明治二十九年法律第八十九号）第百四十条から第百四十三条（2026年10月1日時点、版 129AC0000000089_20260624_508AC0000000045）、法令API Version2 で2026年10月2日に取得、PDL1.0
```

They also ask that content which was edited or processed say so, apart from the source, and that
it not be presented as if the government had made it. The copies here are not edited; the page
`koyomi doc` writes quotes the text of an article from its copy with the XML tags removed, and
says which copy, as of which date and which revision.

## The Shift_JIS table, from the WHATWG Encoding Standard

[src/sjis_table.rs](src/sjis_table.rs) holds, for every pointer of the index jis0208 of the WHATWG
Encoding Standard (<https://encoding.spec.whatwg.org/index-jis0208.txt>, Identifier
`cbaa91f3deb7d0841faf5c33041fc15a285da0e87e64ab802c4bf04b7c4da861`, Date 2024-09-18), the
character it decodes to. [tools/sjis/make_table.py](tools/sjis/make_table.py) makes it from the
index, which the repository does not keep. koyomi reads the Cabinet Office's table with it, and
the table is compiled into the `koyomi` binary.

The Encoding Standard says:

```
Copyright © WHATWG (Apple, Google, Mozilla, Microsoft).

This work is licensed under a Creative Commons Attribution 4.0 International
License. To the extent portions of it are incorporated into source code,
such portions in the source code are licensed under the BSD 3-Clause License instead.
```

The table is such a portion. The BSD 3-Clause License, as the standard gives it:

```
Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this
   list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice,
   this list of conditions and the following disclaimer in the documentation
   and/or other materials provided with the distribution.

3. Neither the name of the copyright holder nor the names of its
   contributors may be used to endorse or promote products derived from
   this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

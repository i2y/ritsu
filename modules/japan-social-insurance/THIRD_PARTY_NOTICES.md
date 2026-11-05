# Third-party notices

[sources/nenkin/R08ryougaku.xlsx](sources/nenkin/R08ryougaku.xlsx) is, byte for byte, the premium table
of employees' pension insurance from September 2020 (paid in October), fiscal 2026 edition
(令和2年9月分（10月納付分）からの厚生年金保険料額表（令和8年度版）), that the Japan Pension Service
publishes on <https://www.nenkin.go.jp/service/kounen/hokenryo/ryogaku/ryogakuhyo/index.html>. Its
SHA-256 starts with `462a199ad4f3b69c`. The table the rules cite is taken out into `.fragments` beside
it.

The Japan Pension Service says on its page about the site
(<https://www.nenkin.go.jp/info/aboutweb/aboutsite.html>) that the site may in principle be linked to
and reproduced with no need to contact it, unless a page notes a restriction, and that it takes no
responsibility for anything done with the information. No such restriction is noted on that page.
`LicenseRef-JPS-Site-Terms` in `ritsu.mod` stands for those terms.

# yuen の名前空間

`https://i2y.github.io/ritsu/ns/yuen#` は、yuen が `yuen export prov` でプロジェクトの来歴を [W3C PROV](https://www.w3.org/TR/prov-overview/) に書き出すときに、PROV に足す語の名前空間です。書き出す形は、既定の [PROV-N](https://www.w3.org/TR/prov-n/) と、`--format json` の [PROV-JSON](https://www.w3.org/submissions/prov-json/) です。書き出した文書では接頭辞 `yuen` で表すので、`yuen:Requirement` は IRI `https://i2y.github.io/ritsu/ns/yuen#Requirement` のことです。このアドレスを開くと、下の `Requirement` の項に着きます。

文書が宣言するもう一つの接頭辞 `y` は `urn:yuen:` で、ものそのものの識別子（`y:requirement/extinguisher_distance/v1`、`y:role/safety`）に使います。こちらはアドレスではないので、開くものはありません。

yuen が書き出すのは `.req` に書いてあることで、ここにある語は、PROV に語の無いものだけです。残りは PROV の語そのものです。`entity`、`agent`、`activity`、`used`、`wasAssociatedWith`、`wasAttributedTo`、`wasInfluencedBy`、`wasDerivedFrom`（条から読んだ要件には `prov:type='prov:PrimarySource'`、前の版や置き換えた要件には `'prov:Revision'` を付けます）、ものの名前を入れる `prov:label` です。

例として、yuen の例 `civil_code_periods`（民法 142 条の読み方を決めた例）を `ritsu yuen export prov` で書き出すと、次のようになります（行はコマンドが出したままで、`…` は省いた行です）。

```console
$ cd crates/yuen
$ ritsu yuen export prov examples/civil_code_periods/civil_code_periods.ja.req --root examples/civil_code_periods
document
  prefix yuen <https://i2y.github.io/ritsu/ns/yuen#>
  prefix y <urn:yuen:>

  agent(y:role/法務, [prov:type='yuen:Role', prov:label="法務", yuen:description="条文の読み方を決める"])
…
  entity(y:source/285ace083045d8368c284034af2d5c53, [prov:type='yuen:Source', prov:label="民法 第142条", yuen:law="egov 129AC0000000089", yuen:asof="2026-10-01", yuen:revision="129AC0000000089_20260624_508AC0000000045", yuen:sha256="fc8c35a0769d3b35"])
…
  entity(y:artifact/2f859ab8a626903e47c8508378278a96, [prov:type='yuen:Artifact', prov:label="koyomi \"civil_code_period_end.ja.cal\" date 満了日_142条", yuen:sha256="bba4761410179e6e", yuen:end="item"])
…
  entity(y:requirement/last_day_142/v1, [prov:type='yuen:Requirement', prov:label="満了日_142条", yuen:version="1", yuen:text="期間の末日が日曜日、国民の祝日に関する法律に規定する休日その他の休日に当たるときは、その日に取引をしない慣習がある場合に限り、期間は、その翌日に満了する", yuen:inForce="2026-10-01..", yuen:sha256="d4f2d2a67322df17"])
…
  wasAttributedTo(y:requirement/last_day_142/v1, y:role/法務, [prov:type='yuen:owner'])
  wasDerivedFrom(y:requirement/last_day_142/v1, y:source/285ace083045d8368c284034af2d5c53, -, -, -, [prov:type='prov:PrimarySource'])
  activity(y:decision/6155d37b6d2ddccf3646283416862c48, 2026-10-03T00:00:00, -, [prov:type='yuen:Decision', yuen:why="「その翌日」は文字どおり末日の翌日と読み、翌日も休みでもそれ以上は動かさない。休みが明けるまで動かす読み方とは、起点 2026 年と月数 1〜12 の 4,380 通りのうち 121 通りで分かれる。この例のために決めたもので、法令の読み方を示すものではない"])
  wasAssociatedWith(y:decision/6155d37b6d2ddccf3646283416862c48, y:role/法務, -)
  wasInfluencedBy(y:requirement/last_day_142/v1, y:decision/6155d37b6d2ddccf3646283416862c48)
  used(y:decision/6155d37b6d2ddccf3646283416862c48, y:source/285ace083045d8368c284034af2d5c53, -)
  wasInfluencedBy(y:artifact/2f859ab8a626903e47c8508378278a96, y:requirement/last_day_142/v1, [prov:type='yuen:satisfies'])
  wasInfluencedBy(y:artifact/95a45ec6b28de53151335fc687dc4161, y:requirement/last_day_142/v1, [prov:type='yuen:verifies'])
  activity(y:review/4daae64cc4a1efff4b9a85fe63f7b65e, 2026-10-04T00:00:00, -, [prov:type='yuen:Review', yuen:link="from", yuen:up="fc8c35a0769d3b35", yuen:down="d4f2d2a67322df17", yuen:status="ok"])
  used(y:review/4daae64cc4a1efff4b9a85fe63f7b65e, y:source/285ace083045d8368c284034af2d5c53, -)
  used(y:review/4daae64cc4a1efff4b9a85fe63f7b65e, y:requirement/last_day_142/v1, -)
  wasAssociatedWith(y:review/4daae64cc4a1efff4b9a85fe63f7b65e, y:role/法務, -)
…
  wasInfluencedBy(y:artifact/80565239d83f9f6329e705a3334eec16, y:source/285ace083045d8368c284034af2d5c53, [prov:type='yuen:pins'])
…
endDocument
```

## 語の書き方

- **型**は、`entity`、`agent`、`activity` の `prov:type` の値です。`prov:type='yuen:Requirement'` と書きます。
- **関係の種類**は、`wasAttributedTo` か `wasInfluencedBy` の `prov:type` の値で、どんな関係かを言います。`[prov:type='yuen:satisfies']` と書きます。
- **属性**は、記録の属性の並びにある名前です。`yuen:version="1"` と書きます。値は、数も日付もハッシュも、文字列です。

PROV-JSON では、型と関係の種類は修飾名（`{"$": "yuen:Requirement", "type": "prov:QUALIFIED_NAME"}`）、属性は文字列を値に持つキーです。

## 型 { #types }

### Requirement { #Requirement }

`yuen:Requirement` · [`entity`](https://www.w3.org/TR/prov-dm/#concept-entity) の型

要件の一つの版です。`.req` の `requirement` が宣言するもので、版が二つある要件は、これが二つになります。`prov:label` は要件の名前です。属性は [`version`](#version)、[`text`](#text)、[`inForce`](#inForce)、[`sha256`](#sha256) です。要件の持ち主は[役割](#Role)で（[`owner`](#owner)）、要件は[出典](#Source)や別の要件から読み出されます。要件を満たすものと確かめるものは[成果物](#Artifact)として書き、要件がその成果物に影響した、という関係で結びます（[`satisfies`](#satisfies)、[`verifies`](#verifies)）。

### Source { #Source }

`yuen:Source` · [`entity`](https://www.w3.org/TR/prov-dm/#concept-entity) の型

要件の読み出し元です。ある時点の法令の一つの条か、一つのファイルで、`.req` の `source` が指します。この出典を引く要件は、そこから導かれたものとして書きます（`prov:type='prov:PrimarySource'`）。`prov:label` は出典の名前と条です（`osha §1910.157`）。条には [`law`](#law) と [`asof`](#asof) があり、コピーが記録していれば [`revision`](#revision) もあります。ファイルには [`file`](#file) があり、`.req` に書いてあれば [`url`](#url) もあります。どちらにも、コピーを固定しているハッシュ[`sha256`](#sha256) があります。

### Artifact { #Artifact }

`yuen:Artifact` · [`entity`](https://www.w3.org/TR/prov-dm/#concept-entity) の型

要件を満たすもの、または確かめるもので、`satisfied by` や `verified by` が指します。ファイル一つのこともあれば、ほかの言語のファイルの中の一つ（規則の表、カレンダーの日付、主張）のこともあります。`prov:label` は、`.req` に書いてある参照のままです（`rulec "osha_extinguisher.rule" table distance`）。属性は [`sha256`](#sha256) と [`end`](#end) です。

### Role { #Role }

`yuen:Role` · [`agent`](https://www.w3.org/TR/prov-dm/#concept-agent) の型

プロジェクトが `role` で宣言する役割で、人でも部署でもかまいません。要件を持ち、決め、リンクを確かめ、見送りを承認します。`prov:label` は役割の名前で、属性は [`description`](#description) です。

### Decision { #Decision }

`yuen:Decision` · [`activity`](https://www.w3.org/TR/prov-dm/#concept-activity) の型

要件の下の `decided` の行が書くことです。要件の文や出典の読み方を、誰が、いつ、なぜそう決めたかを表します。この `activity` は決めた日に始まり、決めた役割と結び付き、要件が引く出典を使っています。要件は、これに生成されたのではなく、これから影響を受けたものとして書きます。決めたことは、要件が続くかぎり、あとから足せるからです。属性は [`why`](#why) です。

### Review { #Review }

`yuen:Review` · [`activity`](https://www.w3.org/TR/prov-dm/#concept-activity) の型

人が一つのリンクを確かめた、その一回です。`yuen review` がリンクの下に書く記録（`reviewed 2026-10-04 by safety sha256:… -> sha256:…`）から作ります。この `activity` は確かめた日に始まり、確かめた役割と結び付き、リンクの両端を使っています。確かめ直せば別の `Review` になり、一度も確かめていないリンクには、これがありません。属性は [`link`](#link)、[`up`](#up)、[`down`](#down)、[`status`](#status) です。

### Waiver { #Waiver }

`yuen:Waiver` · [`activity`](https://www.w3.org/TR/prov-dm/#concept-activity) の型

要件に、満たすものを置かない（`not satisfied`）、あるいは確かめるものを置かない（`not verified`）と決めたことで、理由が付きます。要件を使っています。人が `approved` で承認すれば、この `activity` は承認した日に始まり、承認した役割と結び付き、そのときの要件のハッシュを持ちます。承認するまでは、始まりが無く、状態は `unapproved` です。承認し直せば別の `Waiver` になります。属性は [`side`](#side)、[`why`](#why)、[`sha256`](#sha256)、[`status`](#status) です。

## 関係の種類 { #relations }

### owner { #owner }

`yuen:owner` · [`wasAttributedTo`](https://www.w3.org/TR/prov-dm/#concept-attribution) の種類

`wasAttributedTo(要件, 役割, [prov:type='yuen:owner'])` は、`owner` の行が書く、要件の持ち主の役割を表します。要件に変更が及んだとき、`yuen affected` が挙げる役割です。

### satisfies { #satisfies }

`yuen:satisfies` · [`wasInfluencedBy`](https://www.w3.org/TR/prov-dm/#concept-influence) の種類

`wasInfluencedBy(成果物, 要件, [prov:type='yuen:satisfies'])` は、その成果物が要件を満たすことを表します。`satisfied by` の行ごとに一つです。

### verifies { #verifies }

`yuen:verifies` · [`wasInfluencedBy`](https://www.w3.org/TR/prov-dm/#concept-influence) の種類

`wasInfluencedBy(成果物, 要件, [prov:type='yuen:verifies'])` は、その成果物が要件を確かめることを表します。`verified by` の行ごとに一つです。

### pins { #pins }

`yuen:pins` · [`wasInfluencedBy`](https://www.w3.org/TR/prov-dm/#concept-influence) の種類

`wasInfluencedBy(ファイル, 条, [prov:type='yuen:pins'])` は、成果物のファイル（規則やカレンダー）がその条を固定していることを表します。ファイルは、プロジェクトが読むのと同じコピーのハッシュを持っています。言語はファイルの単位で固定するので、影響を受けるのはファイル全体です。どのリンクもそのファイルを丸ごとは指していなければ、そのファイルを、[`end`](#end) が `file` の[成果物](#Artifact)として、このために書きます。

## 属性 { #attributes }

### version { #version }

`yuen:version` · [`Requirement`](#Requirement) の属性

版の番号を、文字列で書きます。`"1"` のようにです。版は、前の版から導かれたものとして書きます（`prov:type='prov:Revision'`）。

### text { #text }

`yuen:text` · [`Requirement`](#Requirement) の属性

要件の文で、`text` に書いたとおりです。yuen は文の意味を読みません。確かめるのは、文のまわりのリンクとハッシュです。

### inForce { #inForce }

`yuen:inForce` · [`Requirement`](#Requirement) の属性

版が効力を持つ期間で、`in force` に書いたとおりです。`2026-10-01..`（その日から）、`..2027-03-31`（その日まで）、`2026-10-01..2027-03-31` のように書きます。`.req` に書いてあるときだけ付きます。

### law { #law }

`yuen:law` · [`Source`](#Source) の属性

条が属するデータベースと法令で、`source` の行に書いたとおりです。`ecfr 29 CFR 1910`（米国の eCFR）、`egov 129AC0000000089`（日本の e-Gov）のように書きます。

### asof { #asof }

`yuen:asof` · [`Source`](#Source) の属性

条を読む時点の日付で、`source` の行に書いたとおりです。`2026-01-01` のように書きます。

### revision { #revision }

`yuen:revision` · [`Source`](#Source) の属性

コピーの元になった法令の版の ID で、コピーが記録しているときだけ付きます。e-Gov のものは `129AC0000000089_20260624_508AC0000000045` の形です。

### file { #file }

`yuen:file` · [`Source`](#Source) の属性

`file` の出典が固定しているファイルのパスで、プロジェクトのルートからの相対パスです。

### url { #url }

`yuen:url` · [`Source`](#Source) の属性

`file` の出典のファイルを取ってくる場所で、`.req` に書いてあるときだけ付きます。

### end { #end }

`yuen:end` · [`Artifact`](#Artifact) の属性

成果物がファイル全体なら `file`、ファイルの中の一つなら `item` です。

### description { #description }

`yuen:description` · [`Role`](#Role) の属性

役割が何を決めるかで、`role` の行に書いたとおりです。

### why { #why }

`yuen:why` · [`Decision`](#Decision) と [`Waiver`](#Waiver) の属性

理由で、人が書いたとおりです。yuen は読みません。

### side { #side }

`yuen:side` · [`Waiver`](#Waiver) の属性

置かないと決めたほうで、`not satisfied` か `not verified` です。

### link { #link }

`yuen:link` · [`Review`](#Review) の属性

確かめたリンクの種類で、`from`、`satisfied by`、`verified by` のどれかです。

### up { #up }

`yuen:up` · [`Review`](#Review) の属性

確かめた人が見たときの、リンク元の端のハッシュで、複数なら空白で区切ります。`from` の行なら、読み出した条か要件、`satisfied by` と `verified by` なら要件のものです。

### down { #down }

`yuen:down` · [`Review`](#Review) の属性

確かめた人が見たときの、リンク先の端のハッシュです。`from` の行なら要件、ほかなら成果物のものです。

### status { #status }

`yuen:status` · [`Review`](#Review) と [`Waiver`](#Waiver) の属性

記録といまのプロジェクトの突き合わせの結果で、`yuen api` と同じ語で書きます。`ok`（どの端も、確かめたときのまま）、`up_changed`（リンク元の端か、見送りを承認したときの要件が変わった）、`down_changed`（リンク先の端が変わった）、`unapproved`（誰も承認していない見送り）、`bad_record`（記録の書き方が正しくない）、`unreadable`（端が作れず、比べられなかった）のどれかです。

### sha256 { #sha256 }

`yuen:sha256` · [`Requirement`](#Requirement)、[`Source`](#Source)、[`Artifact`](#Artifact)、[`Waiver`](#Waiver) の属性

そのものを表すハッシュで、SHA-256 の先頭 16 桁の 16 進数です。出典なら、条かファイルのコピーで、`.req` が固定している値です。成果物なら、リンクが指すものを、その言語が読んだとおりに（表、日付、ファイルのバイト列）です。要件なら、文と、読み出し元と、期間です。見送りなら、承認したときの要件のハッシュです。

## 語の出どころ

語は、[`crates/yuen/src/export/prov.rs`](https://github.com/i2y/ritsu/blob/main/crates/yuen/src/export/prov.rs)の一覧 `TERMS` にあるものです。ritsu のテストは、このページを一覧に突き合わせます。一覧の語が一つずつ、種類ごとの見出しの下に、書かれる型といっしょにあること。上の例が `ritsu yuen export prov` の出力であること。yuen のテストは、その一覧を、テストの全プロジェクトで `yuen export prov` が書く語に突き合わせます。

関連：[yuen](https://github.com/i2y/ritsu/blob/main/crates/yuen/README.ja.md)、[リファレンス](https://github.com/i2y/ritsu/blob/main/crates/yuen/docs/reference.md)（英語）、[ritsu](../index.md)。

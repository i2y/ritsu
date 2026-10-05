# 挨拶の仕様

## Purpose
名前で挨拶し、足した数の合計をリクエストをまたいで覚えておく、小さな HTTP サービス。geas の例 greeter のサーバーが、この仕様のとおりに答えることを、主張で確かめる。この例のために書いた仕様である。

## Requirements

### Requirement: 名前で挨拶する
サービスは `GET /greet?name=<名前>` に、ステータス 200 と、`message` が `Hello, <名前>` の JSON で答える（SHALL）。空の名前には、ステータス 400 で答える（SHALL）。

#### Scenario: 名前で挨拶する
- **WHEN** クライアントが `/greet?name=Alice` を求める
- **THEN** ステータスは 200
- **AND** `message` は `Hello, Alice`

#### Scenario: 空の名前は受け付けない
- **WHEN** クライアントが `/greet?name=` を求める
- **THEN** ステータスは 400

### Requirement: 足した数の合計
サービスは、リクエストをまたいで合計を覚えておく（SHALL）。`POST /reset` で 0 にし、`POST /add` で本文の数を足し、`GET /total` で合計を返す。

#### Scenario: 足した数が積み上がる
- **GIVEN** 合計を 0 にした
- **WHEN** クライアントが 5 と 7 を足す
- **THEN** `GET /total` は 12 を返す

### Requirement: 知らないパス
サービスは、知らないパスにステータス 404 で答える（MUST）。

#### Scenario: 知らないパスには 404 を返す
- **WHEN** クライアントが `/nope` を求める
- **THEN** ステータスは 404

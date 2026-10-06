## MODIFIED Requirements

### Requirement: 名前で挨拶する
サービスは `GET /greet?name=<名前>` に、ステータス 200 と、`message` が、名前の前後の空白を除いた `Hello, <名前>` の JSON で答える（SHALL）。空白を除くと空になる名前には、ステータス 400 で答える（SHALL）。

#### Scenario: 名前で挨拶する
- **WHEN** クライアントが `/greet?name=Alice` を求める
- **THEN** ステータスは 200
- **AND** `message` は `Hello, Alice`

#### Scenario: 空の名前は受け付けない
- **WHEN** クライアントが `/greet?name=` を求める
- **THEN** ステータスは 400

#### Scenario: 空白だけの名前は受け付けない
- **WHEN** クライアントが `/greet?name=%20%20` を求める
- **THEN** ステータスは 400

## ADDED Requirements

### Requirement: ヘルスチェック
サービスは `GET /health` にステータス 200 で答える（SHALL）。

#### Scenario: ヘルスチェックに答える
- **WHEN** クライアントが `/health` を求める
- **THEN** ステータスは 200

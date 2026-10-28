# kaname のモジュールの試験で、app の依存。注文の合計が base の税率を準用し、帳簿がお金を持ち、
# モジュールが注文を請求する。
module example.com/acme/lib
version 1.0.0
description "モジュールの試験の注文の合計、台帳、請求"

require base path "../土台"

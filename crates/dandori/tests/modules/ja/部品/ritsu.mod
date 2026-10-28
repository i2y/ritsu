# dandori のモジュールの試験で、app の依存。注文の合計が base の税率を準用する。台帳と、注文のサービスの
# .proto と、注文の値段を決める流れを持つ。
module example.com/acme/lib
version 1.0.0
description "モジュールの試験の注文の合計、台帳、注文、値付けの流れ"

require base path "../土台"

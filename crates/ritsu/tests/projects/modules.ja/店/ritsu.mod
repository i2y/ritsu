# Acme の店。手元のモジュールで、注文とかごの値段を pricing で決める。pricing の注文の合計は、tax の
# 税率を準用する。
module example.com/acme/shop
version 0.1.0
description "Acme の店の注文のフローと会計"

require pricing path "../価格"

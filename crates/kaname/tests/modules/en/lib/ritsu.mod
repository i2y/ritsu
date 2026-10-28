# The dependency of app in kaname's tests of modules: its order total applies the VAT rate of base, its
# book holds the money, and its module charges an order.
module example.com/acme/lib
version 1.0.0
description "The order totals, the ledger and the charges of the tests of modules"

require base path "../base"

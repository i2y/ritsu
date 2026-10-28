# The dependency of app in dandori's tests of modules: its order total applies the VAT rate of base;
# it has a ledger, the order service's .proto and a flow that prices an order.
module example.com/acme/lib
version 1.0.0
description "The order totals, the ledger, the orders and the pricing flow of the tests of modules"

require base path "../base"

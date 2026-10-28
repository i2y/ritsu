# The Acme shop, the module at hand: it prices its orders and its baskets with pricing, whose order
# total applies the VAT rate of tax.
module example.com/acme/shop
version 0.1.0
description "The order flow and the checkout of the Acme shop"

require pricing path "../pricing"

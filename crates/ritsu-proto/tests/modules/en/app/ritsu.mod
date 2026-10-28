# The module at hand in ritsu-proto's tests of modules: it reads the orders of lib, and has a money
# of its own at the path lib's money is at in lib.
module example.com/acme/app
version 0.1.0
description "The invoices of the tests of modules"

require lib path "../lib"

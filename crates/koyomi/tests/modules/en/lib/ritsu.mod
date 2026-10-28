# The dependency of app in koyomi's tests of modules: its calendar reads the closed days of base.
module example.com/acme/lib
version 1.0.0
description "The calendar of the office of the tests of modules"

require base path "../base"

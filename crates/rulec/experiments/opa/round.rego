# The five rounding modes of §7.3 in Rego: integers only, computed on the magnitude with
# the sign put back — the shape the generated Python, Rust and Go helpers all have.
package rulec.round

_q(x, g) := floor(abs(x) / g)
_r(x, g) := abs(x) % g

_m(x, g, "down") := _q(x, g) * g

_m(x, g, "up") := (_q(x, g) + 1) * g if _r(x, g) != 0
_m(x, g, "up") := _q(x, g) * g if _r(x, g) == 0

_m(x, g, "half") := (_q(x, g) + 1) * g if 2 * _r(x, g) >= g
_m(x, g, "half") := _q(x, g) * g if 2 * _r(x, g) < g

_m(x, g, "half_down") := (_q(x, g) + 1) * g if 2 * _r(x, g) > g
_m(x, g, "half_down") := _q(x, g) * g if 2 * _r(x, g) <= g

_m(x, g, "bankers") := (_q(x, g) + 1) * g if 2 * _r(x, g) > g
_m(x, g, "bankers") := (_q(x, g) + 1) * g if {
	2 * _r(x, g) == g
	_q(x, g) % 2 == 1
}
_m(x, g, "bankers") := _q(x, g) * g if {
	2 * _r(x, g) == g
	_q(x, g) % 2 == 0
}
_m(x, g, "bankers") := _q(x, g) * g if 2 * _r(x, g) < g

apply(mode, x, g) := -v if {
	x < 0
	v := _m(x, g, mode)
}

apply(mode, x, g) := v if {
	x >= 0
	v := _m(x, g, mode)
}

# The cases that produced a value. A body that fails is silently skipped in Rego, so the
# count is what tells a skipped case apart from a passing one.
evaluated contains i if {
	some i, c in input.cases
	apply(c[0], c[1], c[2])
}

failures contains {"mode": c[0], "x": c[1], "g": c[2], "want": c[3], "got": got} if {
	some c in input.cases
	got := apply(c[0], c[1], c[2])
	got != c[3]
}

@id("escapes")
@note("tab\there, newline\nthere, quote \" and backslash \\, \0 nul, A and 😀 and A")
permit (
  principal,
  action == Action::"read/write",
  resource
)
when
{
  context.s == "a\"b" &&
  context.t == "it's" &&
  context.u == "'" &&
  context.v like "\*literal star then wildcard*" &&
  context.w like "*" &&
  context.x like "" &&
  context.y == "" &&
  context.z == "􏿿\u{10}" &&
  context.raw == "line one\nline two"
};

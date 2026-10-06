permit (principal, action, resource)
when {
  1 + 2 * 3 - 4 == 3 &&
  -5 < -(3) && --7 >= 1 && !!true && !!!false && !!!!true &&
  (1 + 2) * 3 > 8 && 2 * (3 + 4) <= 14 &&
  (true || false) && (true && (false || true)) || false &&
  context.n != 3 && !(context.n == 4) &&
  -9223372036854775808 < 9223372036854775807 &&
  -(-9223372036854775808) != 0 && 007 == 7 && ----1 == 1
};

permit (principal, action, resource)
when { if context.a then if context.b then 1 else 2 else 3 == 1 };

permit (principal, action, resource)
when { (if context.a then 1 else 2) + 3 == 4 && [if true then 1 else 2] == [1] };

permit (principal, action, resource)
when {
  principal has name && principal has "full name" && context has a.b.c &&
  principal.name like "*@example.com" && context.path like "/docs/\*/*" &&
  principal is User && resource is Doc in Folder::"root" &&
  principal in Group::"admins" && principal in [Group::"a", Group::"b"] &&
  principal.tags.contains("x") && [1, 2, 3].containsAll([1]) && [1].containsAny([]) &&
  context.list.isEmpty() && principal.hasTag("t") && principal.getTag("t") == "v" &&
  principal["full name"] == context["key"]["inner"] && context.a.b.c.d == 1
};

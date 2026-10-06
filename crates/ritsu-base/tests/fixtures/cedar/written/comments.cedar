@id("commented")
@reason("why")
permit (
  principal == User::"alice",
  action in [Action::"a", Action::"b"],
  resource is Photo
)
when
{
  context.a &&
  !context.b ||
  1 + 2 == 3 &&
  context.list.contains(1) &&
  { k: 1 } has k &&
  (if true then 1 else 2) == 1 &&
  context.x == "y"
};

forbid (principal, action, resource)
when { true };

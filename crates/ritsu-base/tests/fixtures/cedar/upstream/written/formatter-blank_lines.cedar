permit (
  principal == User::"alice",
  action,
  resource in Folder::"Name\n\n\t\nwith a newline"
)
when
{
  context.foo == "string\n\n\twith\n  \n  newlines and other\tstrange characters🐈👍\"\n\n// even something that looks like a comment\n  \n"
};

permit (
  principal is User in Group::"friends",
  action,
  resource is Photo in Album::"vacation"
)
when
{
  User::"alice" is User && User::"alice" in Group::"\n\n\n\n\n\n\n\n\nfriends"
};

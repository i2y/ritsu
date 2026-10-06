permit (
  principal is User,
  action,
  resource is Photo
)
when { User::"alice" is User };

permit (
  principal is User in Group::"friends",
  action,
  resource is Photo in Album::"vacation"
)
when { User::"alice" is User in Group::"friends" };

permit (
  principal is User,
  action,
  resource is Photo
)
when { User::"alice" is User };

permit (
  principal is User in Group::"friends",
  action,
  resource is Photo in Album::"vacation"
)
when { User::"alice" is User in Group::"friends" };

permit (
  principal,
  action,
  resource is List
)
when { resource.owner == principal };

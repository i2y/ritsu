permit (
  principal,
  action,
  resource == a::""
);

permit (
  principal,
  action in Action::"",
  resource
);

permit (principal, action, resource)
when { a::"" };

permit (principal, action, resource)
when { if true then 1 else 2 };

@a("")
permit (
  principal == User::"",
  action,
  resource
)
when { ip("") };

permit (
  principal == User::"Bob",
  action in [Action::"read", Action::"write"],
  resource in Folder::"Document"
)
when { principal.is_authenticated };

@a("")
permit (
  principal == User::"",
  action,
  resource
)
when { !!!1 };

@normal("this is a policy written by real people!")
permit (
  principal == User::"Alice",
  action in [Action::"read", Action::"write", Action::"execute"],
  resource in Shared::"Scripts"
)
when { principal.is_admin };

@abnormal("this is a policy for stress testing the foramtter")
permit (
  principal == User::"Alice",
  action == Action::"Do",
  resource == Resource::"Something"
)
when { 1 != 2 };

forbid (principal, action, resource)
when { principal == resource && principal.x == resource.x };

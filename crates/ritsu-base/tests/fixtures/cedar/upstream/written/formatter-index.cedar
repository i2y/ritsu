permit (principal, action, resource)
when { principal.is_admin };

permit (principal, action, resource)
when { principal.roles.admin };

permit (principal, action, resource)
when { principal.roles.admin };

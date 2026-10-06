permit (principal, action, resource)
when { principal.is_admin };

permit (principal, action, resource)
when { principal.roles.admin };

permit (principal, action, resource)
when { principal.roles.admin };

permit (principal, action, resource)
when { principal.profile.settings.theme == "dark" };

permit (principal, action, resource)
when { resource.data.user_config.enabled };

permit (principal, action, resource)
when
{
  principal.organization
    .departments
    .engineering
    .teams
    .backend
    .members
    .alice
    .permissions
    .read == true
};

permit (principal, action, resource)
when { resource.metadata.security.permissions.roles.admin.capabilities.delete };

permit (principal, action, resource)
when
{
  principal.company
    .divisions
    .technology
    .departments
    .software
    .teams
    .platform
    .projects
    .auth
    .settings
    .security
    .encryption
    .algorithms
    .aes256
    .keySize == 256
};

permit (principal, action, resource)
when { resource.config.services.database.connections.primary.timeout > 30 };

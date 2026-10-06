@id("alice's friends view policy")
permit (
  principal in UserGroup::"alice_friends",
  action == Action::"view",
  resource in Account::"alice"
);

@id("privacy rule")
forbid (principal, action, resource)
when { resource.private }
unless { resource.account has owner && resource.account.owner == principal };

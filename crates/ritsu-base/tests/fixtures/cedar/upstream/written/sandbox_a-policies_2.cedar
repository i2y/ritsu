@id("alice's access policy")
permit (
  principal == User::"alice",
  action in [Action::"view", Action::"edit", Action::"delete"],
  resource in Album::"jane_vacation"
);

@id("bob's view policy")
permit (
  principal == User::"bob",
  action == Action::"view",
  resource in Album::"jane_vacation"
);

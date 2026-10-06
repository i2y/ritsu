@id("jane_vacation public")
permit (
  principal,
  action in [Action::"view", Action::"listPhotos"],
  resource in Album::"jane_vacation"
);

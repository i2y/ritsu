permit (
  principal == User::"alice",
  action == Action::"view",
  resource == Photo::"VacationPhoto94.jpg"
);

permit (
  principal in Group::"jane_friends",
  action == Action::"view",
  resource == Photo::"VacationPhoto94.jpg"
);

permit (
  principal == User::"alice",
  action == Action::"view",
  resource in Album::"jane_vacation"
);

permit (
  principal == User::"alice",
  action in [Action::"view", Action::"edit", Action::"delete"],
  resource in Album::"jane_vacation"
);

permit (
  principal == User::"alice",
  action in Action::"admin",
  resource in Album::"jane_vacation"
);

permit (
  principal == User::"alice",
  action in Action::"admin",
  resource in Album::"jane_vacation"
);

permit (
  principal == User::"alice",
  action == Action::"edit",
  resource in Album::"jane_vacation"
);

permit (
  principal == User::"alice",
  action,
  resource in Album::"jane_vacation"
)
when { action in PhotoflashRole::"viewer" || action == Action::"edit" };

permit (
  principal,
  action == Action::"view",
  resource in Album::"jane_vacation"
);

permit (
  principal,
  action == Action::"view",
  resource in Album::"jane_vacation"
);

permit (
  principal == User::"alice",
  action in [Action::"listAlbums", Action::"listPhotos", Action::"view"],
  resource in Account::"jane"
);

permit (
  principal == User::"alice",
  action,
  resource in Account::"jane"
);

permit (
  principal,
  action in
    [Action::"UpdateList",
     Action::"CreateTask",
     Action::"UpdateTask",
     Action::"DeleteTask"],
  resource
)
when { principal in resource.editors };

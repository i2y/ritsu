@id("jane's friends view-permission policy")
permit (
  principal in UsrGroup::"jane_friends",
  action == Action::"view",
  resource == Photo::"VacationPhoto94.jpg"
);

@id("disallow tim policy")
forbid (
  principal == User::"tim",
  action,
  resource == Photo::"VacationPhoto94.jpg"
);

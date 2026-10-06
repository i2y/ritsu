@id("owners-edit")
@description("The owner of a photo may edit and delete it.")
permit (
  principal,
  action in [PhotoApp::Action::"edit", PhotoApp::Action::"delete"],
  resource is PhotoApp::Photo
)
when { resource.owner == principal };

@id("viewers")
permit (
  principal in PhotoApp::Group::"friends",
  action == PhotoApp::Action::"view",
  resource in PhotoApp::Album::"vacation"
)
unless { resource.private };

forbid (principal, action, resource)
when { resource has locked && resource.locked }
unless { principal is PhotoApp::Admin };

permit (
  principal == PhotoApp::User::"alice",
  action,
  resource
);

permit (
  principal is PhotoApp::User in PhotoApp::Group::"staff",
  action,
  resource is PhotoApp::Photo in PhotoApp::Album::"public"
);

@advice
@empty("")
forbid (
  principal,
  action == PhotoApp::Action::"delete",
  resource == PhotoApp::Photo::"cover.jpg"
);

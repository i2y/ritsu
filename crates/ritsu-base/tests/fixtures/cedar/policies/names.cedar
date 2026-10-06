permit (
  principal == Org::Team::User::"alice",
  action in Org::Action::"read",
  resource is Org::Team::Doc
)
when {
  principal in Org::Team::Group::"eng" && resource.team == Org::Team::"core" &&
  context.permit == permit::forbid::"x" && context.unless == when::unless::"y" &&
  principal is Org::Team::User &&
  context.principal == principal && context.context == context && context.action == action
};

permit (principal, action == Action::"Action", resource);
permit (principal, action == Org::Sub::Action::"nested", resource);

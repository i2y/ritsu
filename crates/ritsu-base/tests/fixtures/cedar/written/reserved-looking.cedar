@__cedar("an annotation may be named so")
@if("and so")
permit (principal, action, resource)
when
{
  context["__cedar"] == context["if"] &&
  principal has "is" &&
  principal has "__cedar" &&
  resource.permit == resource.forbid &&
  resource.when.unless == resource.principal.context &&
  { "like": 1, "has": 2, "then": 3 }["like"] == 1
};

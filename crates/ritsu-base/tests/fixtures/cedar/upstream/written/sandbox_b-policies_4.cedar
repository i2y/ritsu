@id("prototypes access policy")
permit (
  principal,
  action == Action::"view",
  resource in Album::"device_prototypes"
)
when
{ principal.department == "HardwareEngineering" && principal.jobLevel >= 5 };

@id("所有者は編集できる")
@description("写真の所有者は、その写真を編集できます。")
permit (
  principal,
  action == Action::"編集",
  resource is Photo
)
when { resource["所有者"] == principal && context["場所"] == "東京" };

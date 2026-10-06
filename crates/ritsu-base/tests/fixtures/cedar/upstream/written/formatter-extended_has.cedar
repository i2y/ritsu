permit (
  principal is User,
  action == Action::"preview",
  resource == Movie::"Blockbuster"
)
when
{
  principal has contactInfo.address.zip &&
  principal.contactInfo.address.zip == "90210"
};

permit (
  principal is User,
  action == Action::"preview",
  resource == Movie::"Blockbuster"
)
when
{
  principal has contactInfo.address.zip &&
  principal.contactInfo.address.zip == "90210"
};

permit (
  principal is User,
  action == Action::"preview",
  resource == Movie::"Blockbuster"
)
when
{
  principal
  has
  contactInfooooooooooooooooooooooooooooooooooooooooooooooo.addressssssssssssssssssssss
      .zipppppppppppp &&
  principal.contactInfo.address.zip == "90210"
};

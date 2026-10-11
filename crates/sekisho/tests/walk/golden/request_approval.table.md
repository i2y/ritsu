| principal | employee | approver | status | resource.requester is principal | band | allowed approve | -> | policies |
|---|---|---|---|---|---|---|---|---|
| User | yes | yes | submitted | yes | within_limit | no | allow | approvers_ask_for_what_they_cannot_approve |
| User | yes | yes | submitted | any | over_limit | no | allow | approvers_ask_for_what_they_cannot_approve |
| User | yes | yes | not submitted | any | any | no | deny | (no permit) |
| User | yes | yes | submitted | no | within_limit | yes | deny | (no permit) |
| User | any | no | any | any | any | no | deny | (no permit) |

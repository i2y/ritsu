| principal | clerk | manager | auditor | workflow | suspended | status | amount | refund_band | in_period | business_day | -> | policies |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| User | yes | no | no | - | no | not refunded | any | within_limit | yes | any | allow | clerks_refund_within_their_limit |
| User | yes | yes | no | - | no | not refunded | any | within_limit | yes | any | allow | clerks_refund_within_their_limit, managers_refund_in_period |
| User | yes | yes | no | - | no | not refunded | any | over_limit | yes | any | allow | managers_refund_in_period |
| User | yes | yes | no | - | no | not refunded | any | any | no | yes | allow | managers_refund_late_on_business_days |
| Workflow | - | - | - | returns | - | returned | <=50GBP | - | any | any | allow | returns_refunds_returned_orders |
| User | yes | yes | no | - | no | refunded | any | any | any | any | deny | no_second_refund |
| User | any | no | no | - | no | refunded | any | any | any | any | deny | no_second_refund |
| Workflow | - | - | - | returns | - | refunded | any | - | any | any | deny | no_second_refund |
| User | yes | yes | yes | - | no | refunded | any | any | any | any | deny | no_second_refund, auditors_do_not_refund |
| User | any | no | yes | - | no | refunded | any | any | any | any | deny | no_second_refund, auditors_do_not_refund |
| User | yes | yes | yes | - | yes | refunded | any | any | any | any | deny | no_second_refund, auditors_do_not_refund, suspended_staff_do_nothing |
| User | any | no | yes | - | yes | refunded | any | any | any | any | deny | no_second_refund, auditors_do_not_refund, suspended_staff_do_nothing |
| User | yes | yes | no | - | yes | refunded | any | any | any | any | deny | no_second_refund, suspended_staff_do_nothing |
| User | any | no | no | - | yes | refunded | any | any | any | any | deny | no_second_refund, suspended_staff_do_nothing |
| User | yes | yes | yes | - | no | not refunded | any | any | any | any | deny | auditors_do_not_refund |
| User | any | no | yes | - | no | not refunded | any | any | any | any | deny | auditors_do_not_refund |
| User | yes | yes | yes | - | yes | not refunded | any | any | any | any | deny | auditors_do_not_refund, suspended_staff_do_nothing |
| User | any | no | yes | - | yes | not refunded | any | any | any | any | deny | auditors_do_not_refund, suspended_staff_do_nothing |
| User | yes | yes | no | - | yes | not refunded | any | any | any | any | deny | suspended_staff_do_nothing |
| User | any | no | no | - | yes | not refunded | any | any | any | any | deny | suspended_staff_do_nothing |
| User | yes | yes | no | - | no | not refunded | any | any | no | no | deny | (no permit) |
| User | any | no | no | - | no | not refunded | any | any | no | any | deny | (no permit) |
| User | any | no | no | - | no | not refunded | any | over_limit | yes | any | deny | (no permit) |
| User | no | no | no | - | no | not refunded | any | within_limit | yes | any | deny | (no permit) |
| Workflow | - | - | - | returns | - | paid, shipped | <=50GBP | - | any | any | deny | (no permit) |
| Workflow | - | - | - | returns | - | not refunded | >50GBP | - | any | any | deny | (no permit) |

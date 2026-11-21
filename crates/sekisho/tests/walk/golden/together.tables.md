### read_doc

| principal | staff | amount | a | b | -> | policies |
|---|---|---|---|---|---|---|
| User | yes | any | low | low | allow | staff_read_low_a |
| User | yes | >100JPY | high | high | allow | staff_read_high_b |
| User | no | any | low | low | deny | (no permit) |
| User | no | >100JPY | high | high | deny | (no permit) |

### file_doc

| principal | staff | a | c | -> | policies |
|---|---|---|---|---|---|
| User | yes | low | low | allow | staff_file_low_a |
| User | yes | low | high | allow | staff_file_low_a, staff_file_high_c |
| User | yes | high | high | allow | staff_file_high_c |
| User | no | low | low | deny | (no permit) |
| User | no | any | high | deny | (no permit) |

### send_doc

| principal | staff | a | step | -> | policies |
|---|---|---|---|---|---|
| User | yes | low | small | allow | staff_send_low_a |
| User | yes | low | large | allow | staff_send_low_a, staff_send_large |
| User | yes | high | large | allow | staff_send_large |
| User | no | low | small | deny | (no permit) |
| User | no | any | large | deny | (no permit) |

### weigh_doc

| principal | staff | kind | -> | policies |
|---|---|---|---|---|
| User | yes | any | allow | staff_weigh_what_is_not_mixed |
| User | no | any | deny | (no permit) |

### compare_doc

| principal | staff | side | -> | policies |
|---|---|---|---|---|
| User | yes | same | allow | staff_compare_what_is_on_one_side |
| User | no | same | deny | (no permit) |

### price_doc

| principal | staff | a | b | -> | policies |
|---|---|---|---|---|---|
| User | yes | low | low | allow | staff_price_low_a |
| User | yes | high | high | allow | staff_price_high_b |
| User | any | - | - | deny | (no permit) |
| User | no | low | low | deny | (no permit) |
| User | no | high | high | deny | (no permit) |


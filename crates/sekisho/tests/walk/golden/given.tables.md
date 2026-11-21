### read_doc

| principal | reader | band | -> | policies |
|---|---|---|---|---|
| User | yes | any | allow | readers_of_a_band_read |
| User | no | any | deny | (no permit) |

### skim_doc

| principal | band | -> | policies |
|---|---|---|---|
| Guest | adult | allow | adult_guests_skim |
| Guest | - | deny | (no permit) |
| Guest | child | deny | (no permit) |


# The Family and Medical Leave Act of the United States as its regulations state it, in the languages of
# ritsu: who is eligible and for how long (rules), the days a notice is due (dates), the leave an employee
# has left (a book), what an employer owes from a request to its designation (a flow), and who may learn
# what from the records (a gate). Each file says which section of 29 CFR 825 it reads.
module ghcr.io/i2y/ritsu-fmla
version 1.0.0
description "FMLA leave as 29 CFR 825 states it: who is eligible, the notices and their deadlines, the leave left, and who may read the medical records"
ritsu >=0.41.0
license "MIT OR Apache-2.0"
source "https://github.com/i2y/ritsu"

require holidays oci "ghcr.io/i2y/ritsu-holidays" 1.1.0
signature "ghcr.io/i2y/ritsu-holidays" by matching "^https://github[.]com/i2y/ritsu/" issuer "https://token.actions.githubusercontent.com"

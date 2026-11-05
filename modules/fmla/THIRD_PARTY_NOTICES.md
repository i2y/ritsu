# Third-party notices

The module carries copies of sections of the Code of Federal Regulations, as of 2026-01-01, as the eCFR's
versioner API (<https://www.ecfr.gov/>) served them. The rules read the first three and pin each by its
SHA-256; the comments of the other files name the rest:

- [825.110.xml](sources/law/29-CFR-825@2026-01-01/825.110.xml): 29 CFR 825.110, Eligible employee (`fmla_eligibility.rule`)
- [825.200.xml](sources/law/29-CFR-825@2026-01-01/825.200.xml): 29 CFR 825.200, Amount of leave (`fmla_entitlement.rule`, `fmla_leave.book`)
- [825.205.xml](sources/law/29-CFR-825@2026-01-01/825.205.xml): 29 CFR 825.205, Increments of FMLA leave for intermittent or reduced schedule leave (`fmla_entitlement.rule`, `fmla_leave.book`)
- [825.300.xml](sources/law/29-CFR-825@2026-01-01/825.300.xml): 29 CFR 825.300, Employer notice requirements (`fmla_deadlines.cal`, `fmla_request.flow`)
- [825.305.xml](sources/law/29-CFR-825@2026-01-01/825.305.xml): 29 CFR 825.305, Certification, general rule (`fmla_deadlines.cal`, `fmla_request.flow`)
- [825.307.xml](sources/law/29-CFR-825@2026-01-01/825.307.xml): 29 CFR 825.307, Authentication and clarification of medical certification (`fmla_records.gate`)
- [825.500.xml](sources/law/29-CFR-825@2026-01-01/825.500.xml): 29 CFR 825.500, Recordkeeping requirements (`fmla_records.gate`)

The Code of Federal Regulations is a work of the United States government, and under no copyright
in the United States (17 U.S.C. 105).

`.ritsu/modules/` is the copy of the module this one requires, `ghcr.io/i2y/ritsu-holidays`, which is not
sent with this module. Its `THIRD_PARTY_NOTICES.md` is in that copy.

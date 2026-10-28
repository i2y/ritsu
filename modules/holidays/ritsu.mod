# The public holidays of the United Kingdom and of Japan, as calendars of koyomi for other modules to
# use. Each calendar closes the holidays and nothing else: the module that uses one closes its own
# weekends. The data files are byte for byte what GOV.UK and the Cabinet Office serve, pinned by
# their SHA-256.
module ghcr.io/i2y/ritsu-holidays
version 1.0.0
description "The public holidays of the United Kingdom and of Japan, as calendars of koyomi"
ritsu >=0.36.0
license "(MIT OR Apache-2.0) AND OGL-UK-3.0 AND LicenseRef-PDL-1.0"
source "https://github.com/i2y/ritsu"

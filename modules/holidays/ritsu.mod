# The public holidays of the United States, the United Kingdom and Japan, as calendars of koyomi for
# other modules to use. Each calendar closes the holidays and nothing else: the module that uses one
# closes its own weekends. The data files of the United Kingdom and Japan are byte for byte what
# GOV.UK and the Cabinet Office serve, pinned by their SHA-256; that of the United States holds the
# dates of the iCalendar file the Office of Personnel Management serves, kept beside it.
module ghcr.io/i2y/ritsu-holidays
version 1.1.0
description "The public holidays of the United States, the United Kingdom and Japan, as calendars of koyomi"
ritsu >=0.37.0
license "(MIT OR Apache-2.0) AND OGL-UK-3.0 AND LicenseRef-PDL-1.0"
source "https://github.com/i2y/ritsu"

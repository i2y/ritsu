## Why
Names typed with spaces around them are greeted with the spaces, and a name of spaces only is greeted as if it were a name.

## What Changes
- Trim the name before greeting; a name that is empty after trimming is refused.
- Add a health check at `/health`.

## Impact
- Affected specs: greeting

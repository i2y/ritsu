## RENAMED Requirements
* FROM: `### Requirement: Unknown paths`
* TO: `### Requirement: Paths it does not know`
- FROM: ### Requirement: Never paired

## REMOVED Requirements

### Requirement: Running total
**Reason**: The total moves to another service.
**Migration**: Ask that service.

## MODIFIED Requirements

### Requirement: Paths it does not know
The service MUST answer any path it does not know with status 404 and a body `not found`.

#### Scenario: unknown paths are 404
- **WHEN** a client asks for `/nope`
- **THEN** the status is 404
- **AND** the body is `not found`

```markdown
## ADDED Requirements
### Requirement: Inside a fence
```

## ADDED Requirements

### Requirement: Version
The service SHALL answer `GET /version` with its version.

#### Scenario: answers its version
- **WHEN** a client asks for `/version`
- **THEN** the status is 200

# Greeting Specification

## Purpose
A small HTTP service that greets people by name and keeps a running total.

## Requirements

### Requirement: Greeting by name
The service SHALL answer `GET /greet?name=<name>` with status 200 and a JSON body whose `message` is `Hello, <name>` with the spaces around the name removed, and SHALL refuse a name that is empty once they are removed with status 400.

#### Scenario: greets by name
- **WHEN** a client asks for `/greet?name=Alice`
- **THEN** the status is 200
- **AND** `message` is `Hello, Alice`

#### Scenario: rejects an empty name
- **WHEN** a client asks for `/greet?name=`
- **THEN** the status is 400

#### Scenario: rejects a name of spaces
- **WHEN** a client asks for `/greet?name=%20%20`
- **THEN** the status is 400

### Requirement: Running total
The service SHALL keep a total across requests: `POST /reset` sets it to 0, `POST /add` adds the number in the body, and `GET /total` answers it.

#### Scenario: totals accumulate across requests
- **GIVEN** the total was reset
- **WHEN** a client adds 5 and then 7
- **THEN** `GET /total` answers 12

### Requirement: Unknown paths
The service MUST answer any path it does not know with status 404.

#### Scenario: unknown paths are 404
- **WHEN** a client asks for `/nope`
- **THEN** the status is 404

### Requirement: Health check
The service SHALL answer `GET /health` with status 200.

#### Scenario: answers the health check
- **WHEN** a client asks for `/health`
- **THEN** the status is 200

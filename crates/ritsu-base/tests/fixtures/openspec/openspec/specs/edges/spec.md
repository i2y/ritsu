# Edges

## Purpose
What OpenSpec's readers do at the edges of the format.

## Requirements

A line before the first requirement belongs to none.

### Requirement: Fenced headers ###
The reader SHALL pass over a header inside a fenced block.

```markdown
### Requirement: Not a requirement
#### Scenario: Not a scenario
```

#### Scenario: a fence of tildes ####
- **WHEN** a block is fenced with `~~~~`
- **THEN** a `~~~` line inside it does not close it

~~~~
~~~
## Not a section
~~~~

#### Scenario:   

#### Edge case without the word
- **WHEN** a level-4 header does not say `Scenario:`
- **THEN** it is a scenario all the same
##### A deeper header
stays in the scenario's body

### Requirement: C#
A name MUST keep a `#` that closes no heading.

#### Scenario: keeps the hash
- **THEN** the name is `C#`

### Notes for readers
A level-3 header that is not a requirement stays in the block before it.

   

## Notes
After the section, nothing is a requirement.

### Requirement: After the section
This is outside the Requirements section.

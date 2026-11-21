/-
  RingiCore: the ticket machine of ringi (ringi's DESIGN 7, 17), as the machine's code
  (`ticket.kn`, the module the generated code of every request runs) writes it, with what the
  theorems say of it and the comparison's side of it.

  - `Types`, `Records`: the ticket, the events, the part of the organization, what is to do, a
    refusal, and what the machine keeps while it works a move out.
  - `Prelude`: what the machine's code reads from its language (a stable sort, a day as text,
    white space).
  - `Org`, `Stages`, `Moves`: the machine, function by function (days and the organization, who
    answers; the stages, `same approver approves once`, coming to a stage; the `on` lines, a
    request made, each event, `next`).
  - `Glue`: `submitted by`, what the machine asks of the request's own code (`Oracle`), and the
    doors a host calls.
  - `Theorems.*`: DESIGN 17's theorems, as they hold of the machine.
  - `Json`, `Run`: the vectors read and answered (`ritsu-model ringi`).
-/
import RingiCore.Types
import RingiCore.Prelude
import RingiCore.Records
import RingiCore.Org
import RingiCore.Stages
import RingiCore.Moves
import RingiCore.Glue
import RingiCore.Json
import RingiCore.Run
import RingiCore.Theorems.Next
import RingiCore.Theorems.Climb
import RingiCore.Theorems.Approvers
import RingiCore.Theorems.Quorum
import RingiCore.Theorems.Holds

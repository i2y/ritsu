import Lean
/-!
The check of `--lean` (kaname's DESIGN 38.6, step 6). It is run once, after `lake build` has built
the proofs and `leanchecker` has replayed them in the kernel, as `lake env lean --run Check.lean
<file.json>`; the file lists each module's alias with the properties a proof of it is asked for:

    [{"module": "refund_split", "properties": ["shares_add_up", "no_share_is_negative"]}]

For each module, it reads the generated module `Kaname.<alias>` and the proof module
`Proofs.<alias>` into an environment of their own (`loadExts := false`: no extension's state and no
initializer of either is read), and says one line of JSON for each property:

    {"module": "refund_split", "property": "shares_add_up", "verdict": "proved", "axioms": ["propext"], "type": ""}

The verdicts:
  proved            a theorem `Proofs.<alias>.<property>` of the proof module, whose type is the
                    constant `Kaname.<alias>.<property>` of the generated module, resting on no axiom
                    but `propext`, `Classical.choice` and `Quot.sound` (`axioms` lists those it rests on)
  missing           no declaration of that name
  not-a-theorem     a declaration of that name that is not a theorem (`type` says what it is)
  elsewhere         a theorem of that name declared in a module other than the proof module
  statement-moved   the statement is not the generated module's
  states-otherwise  a theorem whose type is not the statement (`type` is its type, as Lean prints
                    it beside the generated module alone)
  rests-on          a theorem resting on other axioms (`axioms` lists those)

A module the two cannot be read together for (the proof file declares a name of the generated
module again) gets one line, `{"module": …, "error": …}`. The axioms are gathered here from the
bodies of the declarations the theorem uses, down to the axioms, and not from what an `.olean`
records of them. The program imports only `Lean` itself: no macro, elaborator or initializer of a
proof file is in force while it runs. Only to print the type of a theorem that states otherwise does
it read extensions, those of Lean's library and of the generated module, in an environment without
the proof module.
-/
open Lean

/-- The axioms a proof may rest on: those Lean itself stands on. -/
def standard : List Name := [``propext, ``Classical.choice, ``Quot.sound]

/-- The axioms the declaration `c` rests on, gathered from its type and its value and from those of
every declaration they use. -/
partial def gather (env : Environment) (c : Name) : StateT (NameSet × NameSet) Id Unit := do
  let (seen, _) ← get
  if seen.contains c then return
  modify fun (s, a) => (s.insert c, a)
  let go (e : Expr) : StateT (NameSet × NameSet) Id Unit := e.getUsedConstants.forM (gather env)
  match env.find? c with
  | some (.axiomInfo v) => modify (fun (s, a) => (s, a.insert c)); go v.type
  | some (.defnInfo v) => go v.type; go v.value
  | some (.thmInfo v) => go v.type; go v.value
  | some (.opaqueInfo v) => go v.type; go v.value
  | some (.quotInfo _) => pure ()
  | some (.ctorInfo v) => go v.type
  | some (.recInfo v) => go v.type
  | some (.inductInfo v) => go v.type; v.ctors.forM (gather env)
  | none => pure ()

def axiomsOf (env : Environment) (c : Name) : List Name :=
  let ((), (_, axs)) := (gather env c).run ({}, {}) |>.run
  (axs.toList.map toString |>.mergeSort (· ≤ ·)).map String.toName

def kindOf : ConstantInfo → String
  | .axiomInfo _ => "axiom"
  | .defnInfo _ => "def"
  | .thmInfo _ => "theorem"
  | .opaqueInfo _ => "opaque"
  | .quotInfo _ => "quot"
  | .inductInfo _ => "inductive"
  | .ctorInfo _ => "constructor"
  | .recInfo _ => "recursor"

def line (m p verdict : String) (axs : List Name := []) (type : String := "") : String :=
  (Json.mkObj [("module", m), ("property", p), ("verdict", verdict), ("axioms", toJson (axs.map toString)), ("type", type)]).compress

/-- The type `e` as Lean prints it, read beside the generated module alone, with the notations of
Lean's library. The proof module is not read here, so no notation of its own is in force; a name it
declares is printed as written. Reading the library's extensions has initializers run, which this
environment, of Lean's library and of the generated module alone, does not reach the proof file
with. -/
unsafe def prettyImpl (genMod : Name) (e : Expr) : IO String := do
  enableInitializersExecution
  let env ← importModules #[{ module := genMod }] {} (loadExts := true)
  let (fmt, _) ← (Meta.ppExpr e |>.run' {} {}).toIO { fileName := "<kaname>", fileMap := default } { env }
  return fmt.pretty 100000

@[implemented_by prettyImpl] opaque pretty (genMod : Name) (e : Expr) : IO String

/-- The lines of one module's properties. -/
def checkModule (alias : String) (props : List String) : IO Unit := do
  let genMod := Name.mkStr `Kaname alias
  let prfMod := Name.mkStr `Proofs alias
  let env ← importModules #[{ module := genMod }, { module := prfMod }] {} (loadExts := false)
  let modOf (n : Name) : Option Name := do
    let idx ← env.getModuleIdxFor? n
    env.header.moduleNames[idx.toNat]?
  for p in props do
    let stmt := Name.mkStr genMod p
    let thm := Name.mkStr prfMod p
    match env.find? thm with
    | none => IO.println (line alias p "missing")
    | some (.thmInfo info) =>
      if modOf thm != some prfMod then
        IO.println (line alias p "elsewhere" (type := toString (modOf thm |>.getD .anonymous)))
      else if modOf stmt != some genMod then
        IO.println (line alias p "statement-moved")
      else if info.type != mkConst stmt then
        IO.println (line alias p "states-otherwise" (type := (← pretty genMod info.type)))
      else
        let axs := axiomsOf env thm
        let extra := axs.filter (fun a => !standard.contains a)
        if extra.isEmpty then IO.println (line alias p "proved" axs)
        else IO.println (line alias p "rests-on" extra)
    | some other => IO.println (line alias p "not-a-theorem" (type := kindOf other))

def main (args : List String) : IO UInt32 := do
  let [file] := args | do IO.eprintln "usage: lean --run Check.lean <file.json>"; return 2
  let asked ← IO.ofExcept (Json.parse (← IO.FS.readFile file))
  let modules ← IO.ofExcept (asked.getArr?)
  initSearchPath (← findSysroot)
  for m in modules do
    let alias ← IO.ofExcept (m.getObjValAs? String "module")
    let props ← IO.ofExcept (m.getObjValAs? (List String) "properties")
    try
      checkModule alias props
    catch e =>
      IO.println (Json.mkObj [("module", alias), ("error", toString e)]).compress
  return 0

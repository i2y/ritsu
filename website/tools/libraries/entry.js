// What website/docs/playground/libraries.js holds: the parts of CodeMirror 6 the page's editor is
// made of, three of CodeMirror's modes for the files of a project that are no language of ritsu's
// (a .proto, a .json, a .yaml), and marked, which turns the Markdown a language's `doc` writes into
// HTML. The page's own code (playground.js) builds the editor and the colours of ritsu's languages
// from these; nothing of ritsu's is in this file. website/tools/make_libraries.sh bundles it.

export { EditorSelection, EditorState } from "@codemirror/state";
export {
  Decoration,
  EditorView,
  MatchDecorator,
  ViewPlugin,
  drawSelection,
  highlightActiveLine,
  highlightActiveLineGutter,
  highlightSpecialChars,
  keymap,
  lineNumbers,
} from "@codemirror/view";
export { HighlightStyle, StreamLanguage, indentUnit, syntaxHighlighting } from "@codemirror/language";
export { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
export { highlightSelectionMatches, search, searchKeymap } from "@codemirror/search";
export { lintGutter, lintKeymap, setDiagnostics } from "@codemirror/lint";
export { tags } from "@lezer/highlight";
export { protobuf } from "@codemirror/legacy-modes/mode/protobuf";
export { yaml } from "@codemirror/legacy-modes/mode/yaml";
export { json } from "@codemirror/legacy-modes/mode/javascript";
export { Marked } from "marked";

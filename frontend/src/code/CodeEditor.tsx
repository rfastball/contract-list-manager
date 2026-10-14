import { useEffect, useId, useRef } from "react";
import { Compartment, EditorState, type Extension } from "@codemirror/state";
import { EditorView, keymap, placeholder as placeholderExt } from "@codemirror/view";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { autocompletion, closeBrackets, completionKeymap, startCompletion } from "@codemirror/autocomplete";
import { HighlightStyle, syntaxHighlighting, bracketMatching } from "@codemirror/language";
import { tags as t } from "@lezer/highlight";

/**
 * CodeMirror 6 를 React 래퍼 없이 감싼다(§2.3). 흐름식과 SQL 에만 쓴다 — 고정폭 글꼴은 코드에만 둔다.
 * 색은 모두 테마 변수라 어두운 테마를 따라간다.
 */
const theme = EditorView.theme({
  "&": { color: "var(--a-ink)", backgroundColor: "var(--a-card)", border: "1px solid var(--n-border-control)", borderRadius: "var(--rad-control)", fontSize: "var(--fs-body)" },
  "&.cm-focused": { outline: "2px solid var(--a-primary)", outlineOffset: "1px" },
  ".cm-scroller": { fontFamily: "var(--font-code)", lineHeight: "1.65", minHeight: "6.5rem" },
  ".cm-content": { caretColor: "var(--a-primary)", padding: "var(--sp-8) 0" },
  ".cm-line": { padding: "0 var(--sp-12)" },
  ".cm-cursor, .cm-dropCursor": { borderLeftColor: "var(--a-primary)" },
  "&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-selectionBackground, ::selection": { backgroundColor: "var(--a-sel)" },
  ".cm-placeholder": { color: "var(--n-ink-soft)" },
  ".cm-tooltip": { border: "1px solid var(--n-border-strong)", backgroundColor: "var(--a-card)", color: "var(--a-ink)", borderRadius: "var(--rad-control)", boxShadow: "0 4px 16px var(--a-shadow)" },
  ".cm-tooltip-autocomplete > ul": { fontFamily: "var(--font-code)", maxHeight: "16rem" },
  ".cm-tooltip-autocomplete > ul > li[aria-selected]": { backgroundColor: "var(--a-sel)", color: "var(--a-ink)" },
  ".cm-completionDetail": { color: "var(--n-ink-soft)", fontStyle: "normal", marginLeft: "var(--sp-8)" },
  ".cm-completionInfo": { fontFamily: "\"Pretendard GOV Variable\", sans-serif" },
  ".cm-diagnostic": { fontFamily: "\"Pretendard GOV Variable\", sans-serif", padding: "var(--sp-4) var(--sp-8)" },
  ".cm-diagnostic-error": { borderLeft: "0", color: "var(--a-danger)" },
  ".cm-lintRange-error": { backgroundImage: "none", textDecoration: "underline wavy var(--a-danger)", textUnderlineOffset: "3px" },
  ".cm-matchingBracket": { backgroundColor: "var(--n-pressed)", outline: "0" },
  "&.cm-editor[aria-readonly] .cm-content, .cm-content[aria-readonly='true']": { caretColor: "transparent" },
});

const highlight = HighlightStyle.define([
  { tag: [t.keyword, t.operatorKeyword], color: "var(--a-primary)", fontWeight: "600" },
  { tag: t.string, color: "var(--a-ok)" },
  { tag: [t.number, t.bool, t.null], color: "var(--a-warn)" },
  { tag: [t.comment, t.lineComment, t.blockComment], color: "var(--n-ink-soft)", fontStyle: "italic" },
  { tag: [t.atom, t.special(t.variableName)], color: "var(--fb-ack-fg)", fontWeight: "600" },
  { tag: [t.function(t.variableName)], color: "var(--fb-ack-fg)" },
  { tag: [t.operator, t.compareOperator, t.punctuation], color: "var(--n-ink-control)" },
  { tag: t.invalid, color: "var(--a-danger)" },
]);

type Props = {
  value: string;
  onChange?: (value: string) => void;
  readOnly?: boolean;
  label: string;
  extensions: Extension[];
  placeholder?: string;
  describedBy?: string;
};

export function CodeEditor({ value, onChange, readOnly = false, label, extensions, placeholder, describedBy }: Props) {
  const keysHint = useId();
  const host = useRef<HTMLDivElement>(null);
  const view = useRef<EditorView | null>(null);
  const extra = useRef(new Compartment());
  const change = useRef(onChange);
  change.current = onChange;

  useEffect(() => {
    const state = EditorState.create({
      doc: value,
      extensions: [
        theme, syntaxHighlighting(highlight), bracketMatching(), history(),
        ...(readOnly ? [] : [closeBrackets(), autocompletion({ activateOnTyping: true, icons: false })]),
        keymap.of([{ key: "Ctrl-Space", run: startCompletion }, ...completionKeymap, ...defaultKeymap, ...historyKeymap, ...(readOnly ? [] : [indentWithTab])]),
        EditorState.readOnly.of(readOnly), EditorView.editable.of(!readOnly),
        EditorView.lineWrapping,
        EditorView.contentAttributes.of({ "aria-label": label, "aria-describedby": [describedBy, !readOnly && keysHint].filter(Boolean).join(" "), ...(readOnly ? { "aria-readonly": "true" } : {}) }),
        EditorView.updateListener.of(u => { if (u.docChanged) change.current?.(u.state.doc.toString()); }),
        ...(placeholder ? [placeholderExt(placeholder)] : []),
        extra.current.of(extensions),
      ],
    });
    view.current = new EditorView({ state, parent: host.current! });
    return () => { view.current?.destroy(); view.current = null; };
    // 편집기는 한 번 세운다. 언어·완성 재료가 바뀌면 칸막이만 갈아 끼운다.
  }, [readOnly]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => { view.current?.dispatch({ effects: extra.current.reconfigure(extensions) }); }, [extensions]);

  useEffect(() => {
    const v = view.current;
    if (v && v.state.doc.toString() !== value) v.dispatch({ changes: { from: 0, to: v.state.doc.length, insert: value } });
  }, [value]);

  return <>
    <div className="code-editor" ref={host} />
    {!readOnly && <p className="column-hint" id={keysHint}>Tab 들여쓰기 · Shift+Tab 내어쓰기 · Ctrl+M 후 Tab으로 다음 항목 이동</p>}
  </>;
}

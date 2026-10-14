import { StreamLanguage, type StringStream } from "@codemirror/language";
import type { CompletionContext, CompletionResult, Completion } from "@codemirror/autocomplete";
import { linter, type Diagnostic } from "@codemirror/lint";
import type { Extension } from "@codemirror/state";
import { checkFlow, docWord, FlowSyntaxError, ident, linkWord, parseFlow, qualified, rowColumns, verbByWord, verbs, verbsFor, type FlowEnv, type LinkKind } from "../flow";

/** 흐름식 하이라이터. 파서와 같은 낱말 경계를 쓴다. */
const wordRe = /^[^\s"[\],.|=≠<>≥≤→!]+/u;
const glue = new Set(["그리고", "또는", "포함", "비어있음", "안비어있음", "오름", "내림", "그외", "일", "개월", "년"]);

/** 하이라이터. 원천 낱말이 가공열(연결공고…)과 맞춤 뷰(공고…)에서 다르다. */
const streamFor = (sourceWords: Set<string>) => StreamLanguage.define<{ afterPipe: boolean }>({
  name: "flow",
  startState: () => ({ afterPipe: true }),
  token(stream: StringStream, state) {
    if (stream.eatSpace()) return null;
    if (stream.match("|>")) { state.afterPipe = true; return "operator"; }
    if (stream.peek() === "\"") {
      stream.next();
      let escaped = false, ch: string | void;
      while ((ch = stream.next()) !== undefined) { if (ch === "\"" && !escaped) break; escaped = !escaped && ch === "\\"; }
      state.afterPipe = false; return "string";
    }
    if (stream.peek() === "[") { stream.skipTo("]") ? stream.next() : stream.skipToEnd(); state.afterPipe = false; return "variableName"; }
    if (stream.match(/^\d+번째/u)) { state.afterPipe = false; return "keyword"; }
    if (stream.match(/^-?\d+(\.\d+)?/)) { state.afterPipe = false; return "number"; }
    if (stream.match(/^(→|->|!=|>=|<=|[=≠<>≥≤])/u)) return "compareOperator";
    if (stream.match(/^[,.]/)) return "punctuation";
    const m = stream.match(wordRe) as RegExpMatchArray | null;
    if (m) {
      const w = m[0], head = state.afterPipe;
      state.afterPipe = false;
      if (head && verbByWord.has(w)) return "keyword";
      if (sourceWords.has(w)) return "atom";
      if (glue.has(w)) return "operatorKeyword";
      return "variableName";
    }
    stream.next();
    return "invalid";
  },
});

const flowStream = streamFor(new Set(["품목", "연결공고", "연결접수", "연결계약"]));
const viewStream = streamFor(new Set(["접수", "공고", "계약"]));

/** 흐름식 편집기의 재료: 하이라이트·완성·밑줄. 맞춤 뷰(<code>env.view</code>)면 문서 이름으로 읽고 완성한다. */
export function flowExtensions(env: FlowEnv): Extension[] {
  const mode = { view: !!env.view };
  const words: Record<LinkKind, string> = env.view ? docWord : linkWord;
  const stream = env.view ? viewStream : flowStream;
  const column = (f: string) => { const q = env.view ? qualified(f) : null; return q ? `${docWord[q.link]}.${ident(q.column)}` : ident(f); };
  const complete = (ctx: CompletionContext): CompletionResult | null => {
    const word = ctx.matchBefore(/[^\s"[\],.|=≠<>≥≤→!]*$/u);
    if (!word) return null;
    if (word.from === word.to && !ctx.explicit) return null;
    const before = ctx.state.sliceDoc(0, word.from);
    const pipe = before.lastIndexOf("|>");
    const inStep = before.slice(pipe < 0 ? 0 : pipe + 2);
    const dot = (env.view ? /(접수|공고|계약)\.$/u : /(연결공고|연결접수|연결계약)\.$/u).exec(before);
    let options: Completion[] = [];
    if (dot) {
      const kind = (Object.entries(words).find(([, w]) => w === dot[1])?.[0]) as LinkKind;
      const link = env.linked[kind];
      options = link ? [{ label: "품목", type: "keyword", detail: "연결 문서의 품목" }, ...link.fields.map(f => ({ label: ident(f), type: "variable", detail: "필드" }))] : [];
    } else if (pipe < 0 && !before.trim()) {
      options = [
        ...(env.view ? [] : [{ label: "품목", type: "keyword", detail: "이 행의 품목" }]),
        ...Object.entries(words).filter(([k]) => env.linked[k as LinkKind]).map(([, w]) => ({ label: w, type: "keyword", detail: env.view ? "문서" : "연결 문서" })),
        ...env.base.map(f => ({ label: ident(f), type: "variable", detail: "이 행의 열" })),
      ];
    } else if (pipe >= 0 && !inStep.trim()) {
      let shape = null as ReturnType<typeof checkFlow>["final"] | null;
      try {
        const c = checkFlow(parseFlow(before.slice(0, pipe), mode).flow, env);
        shape = c.shapes.filter(Boolean).at(-1) ?? null;
      } catch { /* 앞이 아직 글이 아니면 동사 전부 */ }
      options = (shape ? verbsFor(shape) : verbs).map(v => ({ label: v.op === "nth" ? "1번째" : v.word, type: "keyword", detail: v.group, info: v.desc }));
    } else {
      let fields: string[] = [];
      try {
        const c = checkFlow(parseFlow(before.slice(0, pipe < 0 ? before.length : pipe), mode).flow, env);
        fields = c.fields[c.fields.length - 1] ?? [];
      } catch { /* 필드 목록 없이 */ }
      options = [
        ...fields.map(f => ({ label: ident(f), type: "variable", detail: "행 필드" })),
        ...rowColumns(env).filter(f => !fields.includes(f)).map(f => ({ label: column(f), type: "variable", detail: env.view ? "문서의 열" : "이 행의 열" })),
      ];
    }
    return { from: word.from, options, validFor: /^[^\s"[\],.|=≠<>≥≤→!]*$/u };
  };

  const lint = linter(view => {
    const text = view.state.doc.toString();
    if (!text.trim()) return [];
    const out: Diagnostic[] = [];
    try {
      const { flow, spans } = parseFlow(text, mode);
      for (const e of checkFlow(flow, env).errors) {
        const span = spans[Math.min(e.index, spans.length - 1)];
        out.push({ from: span.from, to: span.to, severity: "error", message: e.message });
      }
    } catch (e) {
      if (e instanceof FlowSyntaxError) out.push({ from: Math.min(e.from, text.length), to: Math.min(e.to, text.length), severity: "error", message: e.message });
    }
    return out;
  }, { delay: 250 });

  return [stream, stream.data.of({ autocomplete: complete }), lint];
}

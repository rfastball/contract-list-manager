import { StreamLanguage, type StringStream } from "@codemirror/language";
import type { Completion, CompletionContext, CompletionResult } from "@codemirror/autocomplete";
import { linter } from "@codemirror/lint";
import type { Extension } from "@codemirror/state";
import { checkSqlMock, contractViews, sqlIdent } from "../flow/sql";

/**
 * SQLite 하이라이터와 완성. <code>@codemirror/lang-sql</code> 은 식별자를 ASCII 로만 읽어 한글 열 이름이
 * 오류 토큰이 되므로 쓰지 않는다(§2.3). 유니코드 낱말, <code>"…"</code>·<code>[…]</code>·백틱 식별자를 받는다.
 */
const keywords = new Set(("select from where and or not as on join left inner cross outer group by order having limit offset case when then else end " +
  "distinct all is null in between like glob exists asc desc union cast collate escape true false").split(" "));
export const sqlFunctions = ("abs avg ceil coalesce count date datetime floor group_concat hex ifnull iif instr julianday length lower ltrim " +
  "max min nullif printf replace round rtrim strftime substr sum total trim upper zeroblob").split(" ");
const functionSet = new Set(sqlFunctions);
const word = /^[\p{L}_][\p{L}\p{N}_$]*/u;
/** 바깥 행의 별칭 — 가공열의 현재, 맞춤 뷰의 접수·공고·계약. */
const outerWords = new Set(["현재", "접수", "공고", "계약"]);

type State = { comment: boolean };
const sqlStream = StreamLanguage.define<State>({
  name: "sqlite",
  startState: () => ({ comment: false }),
  token(stream: StringStream, state) {
    if (state.comment) {
      if (stream.skipTo("*/")) { stream.next(); stream.next(); state.comment = false; } else stream.skipToEnd();
      return "comment";
    }
    if (stream.eatSpace()) return null;
    if (stream.match("--")) { stream.skipToEnd(); return "comment"; }
    if (stream.match("/*")) { state.comment = true; return "comment"; }
    const ch = stream.peek();
    if (ch === "'") {
      stream.next();
      for (;;) {
        const c = stream.next();
        if (c === undefined) break;
        if (c === "'") { if (stream.peek() === "'") stream.next(); else break; }
      }
      return "string";
    }
    if (ch === "\"" || ch === "`" || ch === "[") {
      const close = ch === "[" ? "]" : ch;
      stream.next();
      for (;;) {
        const c = stream.next();
        if (c === undefined) break;
        if (c === close) { if (close !== "]" && stream.peek() === close) stream.next(); else break; }
      }
      return "variableName";
    }
    if (stream.match(/^(\d+(\.\d*)?|\.\d+)([eE][-+]?\d+)?/)) return "number";
    const m = stream.match(word) as RegExpMatchArray | null;
    if (m) {
      const w = m[0].toLowerCase();
      if (keywords.has(w)) return w === "null" ? "null" : w === "true" || w === "false" ? "bool" : "keyword";
      if (functionSet.has(w) && /^\s*\(/.test(stream.string.slice(stream.pos))) return "variableName.function";
      if (outerWords.has(m[0])) return "atom";
      return "variableName";
    }
    if (stream.match(/^(\|\||<>|!=|>=|<=|==|[-+*/%<>=&|~])/)) return "operator";
    stream.next();
    return "punctuation";
  },
});

const unquote = (s: string) => s.startsWith("\"") || s.startsWith("`") ? s.slice(1, -1).replace(/""/g, "\"") : s.startsWith("[") ? s.slice(1, -1) : s;
const identSrc = String.raw`("(?:[^"]|"")+"|\[[^\]]+\]|` + "`[^`]+`" + String.raw`|[\p{L}_][\p{L}\p{N}_]*)`;

/** 글 안에서 FROM·JOIN 뒤에 선 뷰와 그 별칭. */
function referenced(doc: string, schema: Record<string, string[]>) {
  const views = new Set<string>();
  const aliases = new Map<string, string>();
  const re = new RegExp(String.raw`\b(?:from|join)\s+${identSrc}(?:\s+(?:as\s+)?${identSrc})?`, "giu");
  for (const m of doc.matchAll(re)) {
    const view = unquote(m[1]);
    if (!schema[view]) continue;
    views.add(view);
    if (m[2] && !keywords.has(m[2].toLowerCase())) aliases.set(unquote(m[2]), view);
  }
  return { views, aliases };
}

/**
 * @param schema 계약면 뷰 → 열
 * @param target 이 자료 종류의 계약면 뷰. <code>현재</code> 는 그 행이다.
 * @param outer 바깥 행의 별칭 → 계약면 뷰. 없으면 <code>현재</code> 하나(target). 맞춤 뷰는 접수·공고·계약 셋이다(docs/user-views.md §3).
 */
export function sqlExtensions(schema: Record<string, string[]>, target: string, onIssues?: (n: number) => void,
  outer: Record<string, string> = { 현재: target }): Extension[] {
  const fnOptions: Completion[] = sqlFunctions.map(f => ({ label: f, type: "function", apply: `${f}(`, detail: "함수" }));
  const complete = (ctx: CompletionContext): CompletionResult | null => {
    const quoted = ctx.matchBefore(/"[^"]*$/u);
    const bare = ctx.matchBefore(/[\p{L}\p{N}_]*$/u);
    const tok = quoted ?? bare;
    if (!tok) return null;
    if (tok.from === tok.to && !ctx.explicit) return null;
    const before = ctx.state.sliceDoc(0, tok.from);
    const doc = ctx.state.doc.toString();
    const { views, aliases } = referenced(doc, schema);
    const ident = (name: string) => quoted ? `"${name.replace(/"/g, "\"\"")}"` : sqlIdent(name);
    const col = (view: string) => (schema[view] ?? []).map(c => ({ label: c, apply: ident(c), type: "property", detail: view }));
    const member = new RegExp(`${identSrc}\\.$`, "u").exec(before);
    let options: Completion[];
    if (member) {
      const owner = unquote(member[1]);
      const view = outer[owner] ?? aliases.get(owner) ?? (schema[owner] ? owner : undefined);
      if (!view) return null;
      options = col(view);
    } else if (/\b(from|join)\s+$/iu.test(before)) {
      options = contractViews.filter(v => schema[v]).map(v => ({ label: v, apply: ident(v), type: "class", detail: "계약면 뷰" }));
    } else {
      const seen = new Set<string>();
      options = [...views].flatMap(col).filter(o => !seen.has(o.label) && !!seen.add(o.label));
      options.push(...Object.entries(outer).map(([alias, view]) => ({ label: alias, type: "variable", detail: `바깥 행 · ${view}` })));
      if (!quoted) options.push(...fnOptions, ...["SELECT", "FROM", "WHERE", "AND", "OR", "ORDER BY", "GROUP BY", "LIMIT", "CASE", "WHEN", "THEN", "ELSE", "END", "AS"].map(k => ({ label: k, type: "keyword" })));
    }
    return { from: tok.from, options, validFor: quoted ? /^"[^"]*$/u : /^[\p{L}\p{N}_]*$/u };
  };
  const lint = linter(view => {
    const issues = checkSqlMock(view.state.doc.toString());
    onIssues?.(issues.length);
    const len = view.state.doc.length;
    return issues.map(i => ({ from: Math.min(i.from, len), to: Math.min(Math.max(i.to, i.from), len), severity: "error" as const, message: i.message }));
  }, { delay: 250 });
  return [sqlStream, sqlStream.data.of({ autocomplete: complete }), lint];
}

/** 읽기 전용 SQL 보기 — 하이라이트만. */
export const sqlReadOnly: Extension[] = [sqlStream];

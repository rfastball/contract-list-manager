import type { Branch, Clause, Compare, Flow, LinkKind, Source, Step } from "./ast";
import { docWord, linkWord, qualified } from "./ast";
import { reserved, verbByOp, verbByWord, type Arg } from "./verbs";

/**
 * 흐름식 글 ↔ 단계(§4.1). 표준 출력은 하나이고, 주석·임의 공백은 보존하지 않는다.
 * 파싱은 같은 JSON 을 내야 한다 — <code>parse(print(f))</code> 가 <code>normalize(f)</code> 와 같다.
 */

export class FlowSyntaxError extends Error {
  constructor(message: string, public from: number, public to: number) { super(message); }
}

type TokType = "pipe" | "str" | "num" | "word" | "nth" | "cmp" | "arrow" | "comma" | "dot" | "eof";
export type Token = { t: TokType; v: string; from: number; to: number; quoted?: boolean };

const special = new Set([..."\"[],.|=≠<>≥≤→!"]);
const isSpace = (c: string) => /\s/u.test(c);

export function tokenize(src: string): Token[] {
  const out: Token[] = [];
  let i = 0;
  while (i < src.length) {
    const c = src[i];
    if (isSpace(c)) { i++; continue; }
    const from = i;
    if (c === "|") {
      if (src[i + 1] !== ">") throw new FlowSyntaxError("‘|’ 다음에는 ‘>’ 가 와야 합니다 (|>)", i, i + 1);
      out.push({ t: "pipe", v: "|>", from, to: i + 2 }); i += 2; continue;
    }
    if (c === "\"") {
      let v = ""; i++;
      for (;;) {
        if (i >= src.length) throw new FlowSyntaxError("따옴표가 닫히지 않았습니다", from, src.length);
        const d = src[i];
        if (d === "\\" && i + 1 < src.length) { v += src[i + 1]; i += 2; continue; }
        if (d === "\"") { i++; break; }
        v += d; i++;
      }
      out.push({ t: "str", v, from, to: i }); continue;
    }
    if (c === "[") {
      const end = src.indexOf("]", i + 1);
      if (end < 0) throw new FlowSyntaxError("‘[’ 가 닫히지 않았습니다", from, src.length);
      const v = src.slice(i + 1, end);
      if (!v.trim()) throw new FlowSyntaxError("[ ] 안에 필드 이름이 없습니다", from, end + 1);
      out.push({ t: "word", v, from, to: end + 1, quoted: true }); i = end + 1; continue;
    }
    if (c === "→" || (c === "-" && src[i + 1] === ">")) {
      const len = c === "→" ? 1 : 2;
      out.push({ t: "arrow", v: "→", from, to: i + len }); i += len; continue;
    }
    if (c === ",") { out.push({ t: "comma", v: ",", from, to: i + 1 }); i++; continue; }
    if (c === "." && !/\d/.test(src[i + 1] ?? "")) { out.push({ t: "dot", v: ".", from, to: i + 1 }); i++; continue; }
    const two = src.slice(i, i + 2);
    const cmp2: Record<string, Compare> = { "!=": "≠", ">=": "≥", "<=": "≤" };
    if (cmp2[two]) { out.push({ t: "cmp", v: cmp2[two], from, to: i + 2 }); i += 2; continue; }
    if ("=≠<>≥≤".includes(c)) { out.push({ t: "cmp", v: c, from, to: i + 1 }); i++; continue; }
    if (c === "!") throw new FlowSyntaxError("‘!’ 는 ‘!=’ 로만 씁니다", i, i + 1);
    const num = /^-?\d+(\.\d+)?/.exec(src.slice(i));
    if (num && (c !== "-" || /\d/.test(src[i + 1] ?? ""))) {
      i += num[0].length;
      if (src.startsWith("번째", i)) {
        if (num[0].includes(".") || num[0].startsWith("-")) throw new FlowSyntaxError("‘번째’ 앞에는 1 이상의 정수가 와야 합니다", from, i + 2);
        out.push({ t: "nth", v: num[0], from, to: i + 2 }); i += 2; continue;
      }
      out.push({ t: "num", v: num[0], from, to: i }); continue;
    }
    let v = "";
    while (i < src.length && !isSpace(src[i]) && !special.has(src[i]) && !(src[i] === "-" && src[i + 1] === ">")) v += src[i++];
    if (!v) throw new FlowSyntaxError(`읽을 수 없는 글자: ${c}`, i, i + 1);
    out.push({ t: "word", v, from, to: i });
  }
  out.push({ t: "eof", v: "", from: src.length, to: src.length });
  return out;
}

const links: Record<string, LinkKind> = { 연결공고: "notice", 연결접수: "request", 연결계약: "contract" };
/** 맞춤 뷰의 문서 이름(docs/user-views.md §3). 맞춤 뷰에서만 원천과 행의 열 앞에 온다. */
const docs: Record<string, LinkKind> = { 접수: "request", 공고: "notice", 계약: "contract" };

/** 글 꼴의 갈래 — 맞춤 뷰면 <code>공고.공고명</code>, 아니면 <code>연결공고.공고명</code>. */
export type SyntaxMode = { view?: boolean };
const wordCmp = new Set(["포함", "비어있음", "안비어있음"]);
const units: Record<string, Step["unit"]> = { 일: "day", 개월: "month", 년: "year" };
export const unitWord: Record<NonNullable<Step["unit"]>, string> = { day: "일", month: "개월", year: "년" };

export type Parsed = { flow: Flow; spans: { from: number; to: number }[] };

/** 흐름식 글을 단계로. 실패하면 위치를 단 FlowSyntaxError. 맞춤 뷰면 문서 이름(공고·접수·계약)으로 읽는다. */
export function parseFlow(src: string, mode: SyntaxMode = {}): Parsed {
  const sources = mode.view ? docs : links;
  const toks = tokenize(src);
  let p = 0;
  const peek = () => toks[p];
  const next = () => toks[p++];
  const kw = (v: string) => peek().t === "word" && !peek().quoted && peek().v === v;
  const fail = (message: string, tok = peek()): never => { throw new FlowSyntaxError(message, tok.from, Math.max(tok.to, tok.from + 1)); };
  const atEnd = () => peek().t === "pipe" || peek().t === "eof";

  const spans: Parsed["spans"] = [];
  const first = peek();
  const expected = mode.view ? "시작 데이터가 와야 합니다 — 접수 · 공고 · 계약 · 글자 · 수" : "시작 데이터가 와야 합니다 — 품목 · 연결공고 · 이 행의 열 · 글자 · 수";
  if (first.t === "eof") fail(expected);
  const source = parseSource();
  spans.push({ from: first.from, to: toks[p - 1].to });
  const steps: Step[] = [];
  while (peek().t !== "eof") {
    if (peek().t !== "pipe") fail("단계 사이에는 ‘|>’ 가 와야 합니다");
    next();
    const start = peek();
    if (start.t === "eof") fail("‘|>’ 뒤에 동사가 와야 합니다");
    steps.push(parseStep());
    spans.push({ from: start.from, to: toks[p - 1].to });
  }
  return { flow: { from: source, steps }, spans };

  function parseSource(): Source {
    const t = next();
    if (t.t === "str") return { kind: "text", text: t.v };
    if (t.t === "num") return { kind: "number", value: Number(t.v) };
    if (t.t !== "word") return fail(expected, t);
    if (!t.quoted && t.v === "품목") return { kind: "items" };
    if (!t.quoted && sources[t.v]) {
      const link = sources[t.v];
      if (peek().t !== "dot") return { kind: "linked", link };
      next();
      const f = next();
      if (f.t !== "word") return fail(`‘${t.v}.’ 뒤에는 품목이나 필드가 와야 합니다`, f);
      if (!f.quoted && f.v === "품목") return { kind: "linked", link, items: true };
      return { kind: "linked", link, field: f.v };
    }
    if (!t.quoted && verbByWord.has(t.v)) return fail(`‘${t.v}’ 는 동사입니다 — 맨 앞에는 시작 데이터가 옵니다`, t);
    return { kind: "field", field: t.v };
  }

  function parseStep(): Step {
    const t = next();
    if (t.t === "nth") {
      const n = Number(t.v);
      if (n < 1) fail("‘번째’ 는 1부터 셉니다", t);
      return finish({ op: "nth", n }, "n번째");
    }
    if (t.t !== "word" || t.quoted) return fail("동사가 와야 합니다", t);
    const verb = verbByWord.get(t.v);
    if (!verb) return fail(`알 수 없는 동사: ${t.v}`, t);
    const step: Step = { op: verb.op };
    for (const a of verb.args) readArg(step, a, verb.word);
    return finish(step, verb.word);
  }

  /** 행의 열 이름 하나. 맞춤 뷰면 <code>공고.공고명</code> 꼴을 한 이름으로 읽는다. */
  function column(f: Token): string {
    if (mode.view && !f.quoted && docs[f.v] && peek().t === "dot") {
      next();
      const c = next();
      if (c.t !== "word") fail(`‘${f.v}.’ 뒤에는 필드가 와야 합니다`, c);
      return `${f.v}.${c.v}`;
    }
    return f.v;
  }

  function finish(step: Step, word: string): Step {
    if (!atEnd()) fail(`‘${word}’ 뒤에 알 수 없는 것이 있습니다: ${peek().v}`);
    return step;
  }

  function readArg(step: Step, a: Arg, word: string) {
    switch (a.k) {
      case "cond": step.cond = readCond(); return;
      case "field": {
        if (a.optional && (atEnd() || peek().t !== "word" || (!peek().quoted && (peek().v === "오름" || peek().v === "내림")))) return;
        const f = next();
        if (f.t !== "word") fail(`‘${word}’ 뒤에 ${a.label}가 와야 합니다`, f);
        step.field = column(f); return;
      }
      case "fields": {
        if (atEnd()) return;
        const list: string[] = [];
        for (;;) {
          const f = next();
          if (f.t !== "word") fail(`‘${word}’ 뒤에는 필드 이름이 와야 합니다`, f);
          list.push(f.v);
          if (peek().t !== "comma") break;
          next();
        }
        step.fields = list; return;
      }
      case "dir":
        if (kw("내림")) { next(); step.dir = "desc"; } else if (kw("오름")) next();
        return;
      case "str": {
        const s = next();
        if (s.t !== "str") fail(`‘${word}’ 에는 따옴표로 감싼 ${a.label}가 와야 합니다`, s);
        step[a.slot] = s.v; return;
      }
      case "int": {
        if (a.optional && atEnd()) return;
        const n = next();
        if (n.t !== "num" || !/^-?\d+$/.test(n.v)) fail(`‘${word}’ 에는 정수 ${a.label}가 와야 합니다`, n);
        step[a.slot] = Number(n.v); return;
      }
      case "unit": {
        const u = next();
        if (u.t !== "word" || u.quoted || !units[u.v]) fail("단위는 일 · 개월 · 년 중 하나입니다", u);
        step.unit = units[u.v]; return;
      }
      case "pairs": {
        const pairs: [string, string][] = [];
        for (;;) {
          const a1 = next();
          if (a1.t !== "str") fail("대응은 \"원래\"→\"바꿀\" 꼴입니다", a1);
          const arrow = next();
          if (arrow.t !== "arrow") fail("두 글 사이에 ‘→’ 가 와야 합니다", arrow);
          const b = next();
          if (b.t !== "str") fail("‘→’ 뒤에는 따옴표로 감싼 글이 와야 합니다", b);
          pairs.push([a1.v, b.v]);
          if (peek().t !== "comma") break;
          next();
        }
        step.pairs = pairs;
        if (kw("그외")) {
          next();
          const o = next();
          if (o.t !== "str") fail("‘그외’ 뒤에는 따옴표로 감싼 글이 와야 합니다", o);
          step.otherwise = o.v;
        }
        return;
      }
      case "branches": {
        // 조건 → "글", … [, 그외 "글"]. 그외 앞의 쉼표는 없어도 읽는다(대응과 같은 꼴).
        const branches: Branch[] = [];
        for (;;) {
          if (kw("그외")) {
            next();
            const o = next();
            if (o.t !== "str") fail("‘그외’ 뒤에는 따옴표로 감싼 글이 와야 합니다", o);
            step.otherwise = o.v;
            break;
          }
          const cond = readCond();
          const arrow = next();
          if (arrow.t !== "arrow") fail("조건 뒤에 ‘→’ 가 와야 합니다", arrow);
          const b = next();
          if (b.t !== "str") fail("‘→’ 뒤에는 따옴표로 감싼 글이 와야 합니다", b);
          branches.push({ cond, then: b.v });
          if (peek().t === "comma") { next(); continue; }
          if (!kw("그외")) break;
        }
        step.branches = branches;
        return;
      }
      case "numOrField": {
        const v = next();
        if (v.t === "num") step.value = Number(v.v);
        else if (v.t === "word") step.field = column(v);
        else fail(`‘${word}’ 뒤에는 수나 열 이름이 와야 합니다`, v);
        return;
      }
      case "strOrField": {
        const v = next();
        if (v.t === "str") step.text = v.v;
        else if (v.t === "word") step.field = column(v);
        else fail(`‘${word}’ 뒤에는 글이나 열 이름이 와야 합니다`, v);
        return;
      }
      case "dayWord":
        if (kw("일")) next();
        return;
    }
  }

  function readCond(): Clause[] {
    const clauses: Clause[] = [];
    let join: Clause["join"];
    for (;;) {
      let field = "";
      // 값 하나를 견주면 적힌 필드는 바깥의 열이라 맞춤 뷰에서 「공고.공고명」 꼴로 온다.
      if (peek().t === "word" && !(!peek().quoted && wordCmp.has(peek().v))) field = column(next());
      const c = next();
      let cmp: Compare;
      if (c.t === "cmp") cmp = c.v as Compare;
      else if (c.t === "word" && !c.quoted && wordCmp.has(c.v)) cmp = c.v as Compare;
      else return fail("조건에는 비교(= ≠ > < ≥ ≤ 포함 비어있음 안비어있음)가 와야 합니다", c);
      const clause: Clause = join ? { join, field, cmp } : { field, cmp };
      if (cmp !== "비어있음" && cmp !== "안비어있음") {
        const v = next();
        if (v.t === "str") clause.value = v.v;
        else if (v.t === "num") clause.value = Number(v.v);
        else fail("비교할 값은 따옴표 글이나 수입니다", v);
      }
      clauses.push(clause);
      if (kw("그리고")) { next(); join = "and"; continue; }
      if (kw("또는")) { next(); join = "or"; continue; }
      return clauses;
    }
  }
}

// ── 표준 출력 ─────────────────────────────────────────

const plainWord = /^[^\s"[\],.|=≠<>≥≤→!{}]+$/u;
export function ident(name: string): string {
  if (!name || !plainWord.test(name) || /^-?\d/.test(name) || name.includes("->") || reserved.has(name) || /^\d+번째$/.test(name)) return `[${name}]`;
  return name;
}
export const quote = (s: string) => `"${s.replace(/\\/g, "\\\\").replace(/"/g, "\\\"")}"`;
const num = (n: number) => String(n);

export function printSource(s: Source, mode: SyntaxMode = {}): string {
  switch (s.kind) {
    case "items": return "품목";
    case "field": return ident(s.field);
    case "text": return quote(s.text);
    case "number": return num(s.value);
    case "linked": return (mode.view ? docWord : linkWord)[s.link] + (s.items ? ".품목" : s.field !== undefined ? `.${ident(s.field)}` : "");
  }
}

/** 행의 열 이름. 맞춤 뷰의 <code>공고.공고명</code> 은 문서 이름을 그대로 두고 열 이름만 감싼다. */
function columnText(field: string, mode: SyntaxMode): string {
  const q = mode.view ? qualified(field) : null;
  return q ? `${docWord[q.link]}.${ident(q.column)}` : ident(field);
}

export function printCond(cond: Clause[], mode: SyntaxMode = {}): string {
  return cond.map(c => {
    const head = c.join ? (c.join === "and" ? "그리고 " : "또는 ") : "";
    const value = c.value === undefined ? "" : typeof c.value === "number" ? ` ${num(c.value)}` : ` ${quote(c.value)}`;
    return `${head}${c.field ? `${columnText(c.field, mode)} ` : ""}${c.cmp}${value}`;
  }).join(" ");
}

/** 동사 뒤의 인자만. 보표의 단계 줄이 동사와 따로 찍는다. */
export function printArgs(step: Step, mode: SyntaxMode = {}): string {
  const verb = verbByOp.get(step.op);
  if (!verb) return "";
  const parts: string[] = [];
  for (const a of verb.args) {
    switch (a.k) {
      case "cond": if (step.cond) parts.push(printCond(step.cond, mode)); break;
      case "field": if (step.field !== undefined) parts.push(columnText(step.field, mode)); break;
      case "fields": if (step.fields?.length) parts.push(step.fields.map(ident).join(", ")); break;
      case "dir": if (step.dir === "desc") parts.push("내림"); break;
      case "str": parts.push(quote(step[a.slot] ?? "")); break;
      case "int": if (step.op !== "nth" && step[a.slot] !== undefined) parts.push(num(step[a.slot]!)); break;
      case "unit": parts.push(unitWord[step.unit ?? "day"]); break;
      case "pairs": {
        parts.push((step.pairs ?? []).map(([x, y]) => `${quote(x)}→${quote(y)}`).join(", "));
        if (step.otherwise !== undefined) parts.push(`그외 ${quote(step.otherwise)}`);
        break;
      }
      case "branches": {
        const list = (step.branches ?? []).map(b => `${printCond(b.cond, mode)} → ${quote(b.then)}`);
        if (step.otherwise !== undefined) list.push(`그외 ${quote(step.otherwise)}`);
        if (list.length) parts.push(list.join(", "));
        break;
      }
      case "numOrField": parts.push(step.field !== undefined ? columnText(step.field, mode) : num(step.value ?? 0)); break;
      case "strOrField": parts.push(step.field !== undefined ? columnText(step.field, mode) : quote(step.text ?? "")); break;
      case "dayWord": break;
    }
  }
  return parts.join(" ");
}

export function verbWord(step: Step): string {
  return step.op === "nth" ? `${step.n ?? 1}번째` : verbByOp.get(step.op)?.word ?? step.op;
}

export function printStep(step: Step, mode: SyntaxMode = {}): string {
  const args = printArgs(step, mode);
  return args ? `${verbWord(step)} ${args}` : verbWord(step);
}

/** 표준 출력. 한 줄이 길면 단계마다 줄을 바꾼다. */
export function printFlow(flow: Flow, mode: SyntaxMode = {}): string {
  const parts = [printSource(flow.from, mode), ...flow.steps.map(s => printStep(s, mode))];
  const line = parts.join(" |> ");
  return line.length <= 64 || parts.length < 3 ? line : parts.join("\n  |> ");
}

/**
 * 동사가 쓰는 자리만 남긴다. 단계 편집면이 꼴을 바꾸며 남긴 자리(예: 서식에서 꺼내기로 바꾼 뒤의 text)가
 * 저장되어 왕복을 깨지 않게 한다.
 */
function normalizeCond(cond: Clause[] | undefined): Clause[] {
  return (cond ?? []).map((c, i) => {
    const x: Clause = i && c.join ? { join: c.join, field: c.field, cmp: c.cmp } : { field: c.field, cmp: c.cmp };
    if (i && !c.join) x.join = "and";
    if (c.cmp !== "비어있음" && c.cmp !== "안비어있음") x.value = c.value ?? "";
    return x;
  });
}

export function normalizeStep(step: Step): Step {
  const verb = verbByOp.get(step.op);
  const out: Step = { op: step.op };
  if (!verb) return out;
  for (const a of verb.args) {
    switch (a.k) {
      case "cond": out.cond = normalizeCond(step.cond); break;
      case "field": if (step.field !== undefined) out.field = step.field; break;
      case "fields": if (step.fields?.length) out.fields = [...step.fields]; break;
      case "dir": if (step.dir === "desc") out.dir = "desc"; break;
      case "str": out[a.slot] = step[a.slot] ?? ""; break;
      case "int": if (step[a.slot] !== undefined) out[a.slot] = step[a.slot]; else if (!a.optional) out[a.slot] = 1; break;
      case "unit": out.unit = step.unit ?? "day"; break;
      case "pairs": out.pairs = (step.pairs ?? []).map(([x, y]) => [x, y]); if (step.otherwise !== undefined) out.otherwise = step.otherwise; break;
      case "branches":
        out.branches = (step.branches ?? []).map(b => ({ cond: normalizeCond(b.cond), then: b.then ?? "" }));
        if (step.otherwise !== undefined) out.otherwise = step.otherwise;
        break;
      case "numOrField": if (step.field !== undefined) out.field = step.field; else out.value = step.value ?? 0; break;
      case "strOrField": if (step.field !== undefined) out.field = step.field; else out.text = step.text ?? ""; break;
      case "dayWord": break;
    }
  }
  return out;
}

export function normalizeFlow(flow: Flow): Flow {
  return { from: { ...flow.from }, steps: flow.steps.map(normalizeStep) };
}

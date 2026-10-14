import type { Clause, Flow, FlowEnv, LinkKind, Step } from "./ast";
import { checkFlow, placeholders } from "./check";

/**
 * 흐름식을 TypeScript 로 돈다. <b>목업 다리만 쓴다</b>(§9) — 진짜 앱은 같은 식을 SQL 로 컴파일해 SQLite 가 돈다.
 * 단계마다 그 시점의 값을 남겨 보표가 그린다.
 */
export type Row = Record<string, string>;
export type EvalInput = {
  /** 이 행의 기본 열. */
  row: Row;
  items: Row[];
  /** 연결 문서. 연결이 없으면 null — 원천은 빈 행이 되고 결과는 빈 값이다. 여러 건(연결계약)이면 행 목록. */
  linked: Partial<Record<LinkKind, { row: Row; items: Row[] } | Row[] | null>>;
};

/** 보표 한 칸. 목록이면 수와 앞 몇 값, 줄어든 수·빈 값 수. */
export type TraceValue =
  | { kind: "list"; count: number; items: string[]; dropped?: number; blank?: number; rows?: Row[] }
  | { kind: "value"; text: string; blank?: boolean; row?: Row };

type V =
  | { k: "rows"; rows: Row[] }
  | { k: "values"; list: string[] }
  | { k: "row"; row: Row | null }
  | { k: "value"; text: string }
  | { k: "number"; n: number | null }
  | { k: "date"; d: string | null };

const SHOW = 3;
export const toNumber = (s: string): number | null => {
  const t = s.replace(/,/g, "").trim();
  // 지수 표기는 받지 않는다 — SQL 쪽(FlowCompiler.Number)이 GLOB 로 같은 꼴만 읽는다.
  if (!t || !/^[-+]?(\d+\.?\d*|\.\d+)$/.test(t)) return null;
  return Number(t);
};
export const toDate = (s: string): string | null => {
  const t = s.slice(0, 10).replace(/\//g, "-");
  if (!/^\d{4}-\d{2}-\d{2}$/.test(t)) return null;
  const d = new Date(`${t}T00:00:00Z`);
  return Number.isNaN(d.getTime()) || d.toISOString().slice(0, 10) !== t ? null : t;
};
const clean = (n: number) => Number(n.toPrecision(15));
export const plain = (n: number) => String(clean(n));
export function thousands(n: number): string {
  const t = Math.trunc(n);
  const s = String(Math.abs(t)).replace(/\B(?=(\d{3})+(?!\d))/g, ",");
  return t < 0 ? `-${s}` : s;
}
/** 글 견주기는 SQLite 의 ORDER BY·비교처럼 글자 차례로 — 지역 정렬(localeCompare)을 쓰면 SQL 과 갈린다. */
const order = (a: string, b: string) => a < b ? -1 : a > b ? 1 : 0;
const rowLabel = (r: Row | null) => r ? Object.values(r).filter(Boolean).slice(0, 2).join(" · ") : "";

function compare(a: string, cmp: Clause["cmp"], b: string | number | undefined): boolean {
  if (cmp === "비어있음") return a === "";
  if (cmp === "안비어있음") return a !== "";
  const bs = b === undefined ? "" : String(b);
  if (cmp === "포함") return a.includes(bs);
  const an = toNumber(a), bn = typeof b === "number" ? b : toNumber(bs);
  const numeric = an !== null && bn !== null;
  const d = numeric ? an! - bn! : order(a, bs);
  switch (cmp) {
    case "=": return numeric ? d === 0 : a === bs;
    case "≠": return numeric ? d !== 0 : a !== bs;
    case ">": return d > 0;
    case "<": return d < 0;
    case "≥": return d >= 0;
    case "≤": return d <= 0;
  }
  return false;
}

/** 그리고가 또는보다 먼저 묶인다(SQL 과 같다). */
function test(cond: Clause[], get: (field: string) => string): boolean {
  const groups: Clause[][] = [[]];
  cond.forEach((c, i) => { if (i && c.join === "or") groups.push([]); groups.at(-1)!.push(c); });
  return groups.some(g => g.every(c => compare(get(c.field), c.cmp, c.value)));
}

function fill(text: string, r: Row | null) {
  return text.replace(/\{([^{}]+)\}/g, (_, f: string) => r?.[f] ?? "");
}

function datePlus(d: string, n: number, unit: Step["unit"]): string {
  const x = new Date(`${d}T00:00:00Z`);
  if (unit === "year") x.setUTCFullYear(x.getUTCFullYear() + n);
  else if (unit === "month") x.setUTCMonth(x.getUTCMonth() + n);
  else x.setUTCDate(x.getUTCDate() + n);
  return x.toISOString().slice(0, 10);
}

function dateText(d: string, fmt: string): string {
  const [y, m, day] = d.split("-");
  return fmt.replace(/YYYY|MM|M|DD|D/g, t => ({ YYYY: y, MM: m, M: String(Number(m)), DD: day, D: String(Number(day)) })[t]!);
}

/** 단계 하나. 꼴은 검사가 이미 맞춰 두었다. dropped·blank 는 보표의 꼬리. */
function apply(step: Step, v: V, input: EvalInput): { v: V; dropped?: number; blank?: number } {
  const base = (f: string) => input.row[f] ?? "";
  const scalarArg = () => step.field !== undefined ? toNumber(base(step.field)) : step.value ?? 0;
  const list = () => v.k === "rows" ? v.rows : (v as { list: string[] }).list;
  const nums = (): number[] => (v.k === "rows" ? v.rows.map(r => r[step.field!] ?? "") : (v as { list: string[] }).list)
    .map(toNumber).filter((x): x is number => x !== null);
  switch (step.op) {
    case "filter": {
      const rows = (v as { rows: Row[] }).rows;
      const kept = rows.filter(r => test(step.cond ?? [], f => r[f] ?? ""));
      return { v: { k: "rows", rows: kept }, dropped: rows.length - kept.length };
    }
    case "sort": {
      const dir = step.dir === "desc" ? -1 : 1;
      const key = (x: Row | string) => typeof x === "string" ? x : step.field ? x[step.field] ?? "" : "";
      const items = [...list()] as (Row | string)[];
      if (step.field !== undefined || v.k === "values") {
        const allNum = items.every(x => toNumber(key(x)) !== null);
        items.sort((a, b) => dir * (allNum ? toNumber(key(a))! - toNumber(key(b))! : order(key(a), key(b))));
      } else if (dir < 0) items.reverse();
      return { v: v.k === "rows" ? { k: "rows", rows: items as Row[] } : { k: "values", list: items as string[] } };
    }
    case "distinct": {
      const seen = new Set<string>();
      if (v.k === "rows") {
        const kept = v.rows.filter(r => {
          const id = JSON.stringify(step.fields?.length ? step.fields.map(f => r[f] ?? "") : r);
          if (seen.has(id)) return false; seen.add(id); return true;
        });
        return { v: { k: "rows", rows: kept }, dropped: v.rows.length - kept.length };
      }
      const all = (v as { list: string[] }).list;
      const kept = all.filter(x => !seen.has(x) && !!seen.add(x));
      return { v: { k: "values", list: kept }, dropped: all.length - kept.length };
    }
    case "take": {
      const all = list(), n = step.n ?? 0;
      return { v: v.k === "rows" ? { k: "rows", rows: v.rows.slice(0, n) } : { k: "values", list: (all as string[]).slice(0, n) }, dropped: Math.max(0, all.length - n) };
    }
    case "nth": case "first": case "last": {
      const all = list();
      const at = step.op === "first" ? 0 : step.op === "last" ? all.length - 1 : (step.n ?? 1) - 1;
      const x = all[at];
      return { v: v.k === "rows" ? { k: "row", row: (x as Row) ?? null } : { k: "value", text: (x as string) ?? "" } };
    }
    case "format": {
      const ph = placeholders(step.text ?? "") as string[];
      const hole = (r: Row | null) => ph.some(f => !(r?.[f] ?? ""));
      if (v.k === "row") return { v: { k: "value", text: v.row ? fill(step.text ?? "", v.row) : "" } };
      const rows = (v as { rows: Row[] }).rows;
      return { v: { k: "values", list: rows.map(r => fill(step.text ?? "", r)) }, blank: rows.filter(hole).length };
    }
    case "get": {
      if (v.k === "row") return { v: { k: "value", text: v.row?.[step.field!] ?? "" } };
      const out = (v as { rows: Row[] }).rows.map(r => r[step.field!] ?? "");
      return { v: { k: "values", list: out }, blank: out.filter(x => !x).length };
    }
    case "join": return { v: { k: "value", text: (v as { list: string[] }).list.join(step.text ?? "") } };
    case "abbrev": {
      const all = (v as { list: string[] }).list, n = step.n ?? 1;
      const text = all.length <= n ? all.join(", ")
        // 함수로 바꾼다 — 글로 주면 값 안의 $& 같은 것이 치환 문법으로 읽힌다.
        : (step.text ?? "").replace(/\{앞\}/g, () => all.slice(0, n).join(", ")).replace(/\{나머지\}/g, () => String(all.length - n));
      return { v: { k: "value", text } };
    }
    case "count": return { v: { k: "number", n: list().length } };
    case "sum": case "min": case "max": case "avg": {
      const xs = nums();
      if (!xs.length) return { v: { k: "number", n: null } };
      const n = step.op === "sum" ? xs.reduce((a, b) => a + b, 0) : step.op === "min" ? Math.min(...xs)
        : step.op === "max" ? Math.max(...xs) : xs.reduce((a, b) => a + b, 0) / xs.length;
      return { v: { k: "number", n } };
    }
  }
  if (v.k === "row" && step.op === "if")
    return { v: { k: "value", text: v.row && test(step.cond ?? [], f => v.row![f] ?? "") ? step.then ?? "" : step.else ?? "" } };
  if (v.k === "value") {
    const s = v.text;
    const text = (t: string): { v: V } => ({ v: { k: "value", text: t } });
    switch (step.op) {
      case "trim": return text(s.trim());
      case "replace": return text(step.text ? s.split(step.text).join(step.to ?? "") : s);
      case "before": { const i = s.indexOf(step.text ?? ""); return text(i < 0 ? s : s.slice(0, i)); }
      case "after": { const i = s.indexOf(step.text ?? ""); return text(i < 0 ? "" : s.slice(i + (step.text ?? "").length)); }
      case "substr": return text([...s].slice((step.n ?? 1) - 1, (step.n ?? 1) - 1 + (step.length ?? 0)).join(""));
      case "padStart": return text(s.padStart(step.length ?? 0, step.text || "0"));
      case "ifEmpty": return text(s || (step.field !== undefined ? base(step.field) : step.text ?? ""));
      case "map": { const hit = step.pairs?.find(([a]) => a === s); return text(hit ? hit[1] : step.otherwise ?? s); }
      // 빈 필드는 지금 값, 적힌 필드는 바깥의 열(비면과 같은 자리).
      case "if": return text(test(step.cond ?? [], f => f ? base(f) : s) ? step.then ?? "" : step.else ?? "");
      case "cases": {
        // 차례대로 견주어 처음 맞는 갈래가 이긴다(SQL 의 CASE WHEN 과 같다).
        const hit = step.branches?.find(b => test(b.cond, f => f ? base(f) : s));
        return text(hit ? hit.then : step.otherwise ?? "");
      }
      case "toNumber": return { v: { k: "number", n: toNumber(s) } };
      case "toDate": return { v: { k: "date", d: toDate(s) } };
    }
  }
  if (v.k === "number") {
    const n = v.n;
    const num = (x: number | null): { v: V } => ({ v: { k: "number", n: x === null || !Number.isFinite(x) ? null : x } });
    switch (step.op) {
      case "add": { const b = scalarArg(); return num(n === null || b === null ? null : n + b); }
      case "sub": { const b = scalarArg(); return num(n === null || b === null ? null : n - b); }
      case "mul": { const b = scalarArg(); return num(n === null || b === null ? null : n * b); }
      case "div": { const b = scalarArg(); return num(n === null || b === null || b === 0 ? null : n / b); }
      case "round": { const p = 10 ** (step.n ?? 0); return num(n === null ? null : Math.sign(n) * Math.round(Math.abs(n) * p) / p); }
      case "floor": { const p = 10 ** (step.n ?? 0); return num(n === null ? null : Math.floor(n * p) / p); }
      case "thousands": return { v: { k: "value", text: n === null ? "" : thousands(n) } };
      case "plain": return { v: { k: "value", text: n === null ? "" : plain(n) } };
    }
  }
  if (v.k === "date") {
    const d = v.d;
    switch (step.op) {
      case "dateAdd": return { v: { k: "date", d: d === null ? null : datePlus(d, step.n ?? 0, step.unit) } };
      case "dateDiff": {
        const other = toDate(base(step.field ?? ""));
        return { v: { k: "number", n: d === null || other === null ? null : Math.round((Date.parse(d) - Date.parse(other)) / 86_400_000) } };
      }
      case "year": return { v: { k: "number", n: d === null ? null : Number(d.slice(0, 4)) } };
      case "month": return { v: { k: "number", n: d === null ? null : Number(d.slice(5, 7)) } };
      case "quarter": return { v: { k: "number", n: d === null ? null : Math.floor((Number(d.slice(5, 7)) - 1) / 3) + 1 } };
      case "dateFormat": return { v: { k: "value", text: d === null ? "" : dateText(d, step.text ?? "") } };
    }
  }
  throw new Error(`이 꼴에서 돌 수 없는 연산: ${step.op}`);
}

function show(v: V, dropped?: number, blank?: number): TraceValue {
  const extra = { ...(dropped ? { dropped } : {}), ...(blank ? { blank } : {}) };
  switch (v.k) {
    case "rows": return { kind: "list", count: v.rows.length, items: v.rows.slice(0, SHOW).map(rowLabel), rows: v.rows.slice(0, 5), ...extra };
    case "values": return { kind: "list", count: v.list.length, items: v.list.slice(0, SHOW), ...extra, ...(blank === undefined && v.list.some(x => !x) ? { blank: v.list.filter(x => !x).length } : {}) };
    case "row": return v.row ? { kind: "value", text: rowLabel(v.row), row: v.row } : { kind: "value", text: "", blank: true };
    case "value": return v.text ? { kind: "value", text: v.text } : { kind: "value", text: "", blank: true };
    case "number": return v.n === null ? { kind: "value", text: "", blank: true } : { kind: "value", text: plain(v.n) };
    case "date": return v.d === null ? { kind: "value", text: "", blank: true } : { kind: "value", text: v.d.replace(/-/g, "/") };
  }
}

/** 열에 찍힐 글(§4.6). 수는 정수면 천단위, 소수는 그대로. 날짜는 YYYY/MM/DD. */
function finalText(v: V): string {
  switch (v.k) {
    case "value": return v.text;
    case "number": return v.n === null ? "" : Number.isInteger(clean(v.n)) ? thousands(clean(v.n)) : plain(v.n);
    case "date": return v.d === null ? "" : v.d.replace(/-/g, "/");
    default: return "";
  }
}

export type Evaluated = { values: (TraceValue | null)[]; error?: { index: number; message: string }; final: string };

/** 단계마다의 값. 꼴 검사가 막은 단계부터는 null. */
export function evaluate(flow: Flow, env: FlowEnv, input: EvalInput): Evaluated {
  const checked = checkFlow(flow, env);
  const values: (TraceValue | null)[] = Array(flow.steps.length + 1).fill(null);
  if (checked.shapes[0] === null) return { values, final: "" };
  const from = flow.from;
  let v: V;
  switch (from.kind) {
    case "items": v = { k: "rows", rows: input.items }; break;
    case "field": v = { k: "value", text: input.row[from.field] ?? "" }; break;
    case "text": v = { k: "value", text: from.text }; break;
    case "number": v = { k: "number", n: from.value }; break;
    case "linked": {
      const doc = input.linked[from.link] ?? null;
      if (env.linked[from.link]?.many) { v = { k: "rows", rows: Array.isArray(doc) ? doc : doc ? [doc.row] : [] }; break; }
      const one = Array.isArray(doc) ? null : doc;
      v = from.items ? { k: "rows", rows: one?.items ?? [] } : from.field === undefined ? { k: "row", row: one?.row ?? null }
        : { k: "value", text: one?.row[from.field] ?? "" };
    }
  }
  values[0] = show(v);
  for (let i = 0; i < flow.steps.length; i++) {
    if (checked.shapes[i + 1] === null) break;
    try {
      const r = apply(flow.steps[i], v, input);
      v = r.v; values[i + 1] = show(v, r.dropped, r.blank);
    } catch (e) {
      return { values, error: { index: i + 1, message: (e as Error).message }, final: "" };
    }
  }
  return { values, final: checked.ok ? finalText(v) : "" };
}

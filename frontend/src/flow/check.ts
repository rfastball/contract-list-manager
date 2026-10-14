import type { Clause, Flow, FlowEnv, Shape, Source, Step } from "./ast";
import { 은는 } from "./josa";
import { docWord, qualified, shapeLabel, wordOf } from "./ast";
import { verbByOp } from "./verbs";
import { verbWord } from "./syntax";

/**
 * 꼴 검사(§4.2·§4.4). 오류는 그것을 만든 단계에 붙는다. 첫 오류 뒤의 단계는 꼴을 모른다(null) —
 * 화면이 「앞 단계 오류」로 흐린다. 색인은 보표와 같다: 0 이 원천, i+1 이 steps[i].
 */
export type FlowIssue = { index: number; message: string; final?: boolean };
export type Checked = {
  shapes: (Shape | null)[];
  /** 그 단계를 지난 뒤 행이 가진 필드. 행목록·행이 아니면 빈 목록. */
  fields: string[][];
  errors: FlowIssue[];
  final: Shape | null;
  ok: boolean;
};

const ends: Shape[] = ["value", "number", "date"];
/** 「경우」 의 갈래 수 상한 — 백엔드 Flow.MaxBranches 와 같다. */
export const maxBranches = 20;

export function sourceShape(from: Source, env: FlowEnv): { shape: Shape; fields: string[] } | string {
  if (env.view && from.kind === "items") return "맞춤 뷰에는 「이 행」 이 없습니다. ‘공고.품목’처럼 문서를 붙여 읽으세요";
  if (env.view && from.kind === "field") return `맞춤 뷰에는 「이 행」 이 없습니다. ‘공고.${from.field}’처럼 문서를 붙여 읽으세요`;
  switch (from.kind) {
    case "items": return { shape: "rows", fields: env.items };
    case "text": return { shape: "value", fields: [] };
    case "number": return { shape: "number", fields: [] };
    case "field":
      return env.base.includes(from.field) ? { shape: "value", fields: [] } : `이 행에 없는 열: ${from.field}`;
    case "linked": {
      const link = env.linked[from.link];
      const word = wordOf(env, from.link);
      if (!link) return `${은는(word)} 이 자료 종류에서 쓸 수 없습니다`;
      if (from.items) return link.many ? `${은는(word)} 여러 건이라 품목을 바로 읽을 수 없습니다. ‘${word} |> 처음’처럼 한 건을 먼저 고르세요` : { shape: "rows", fields: link.items };
      if (from.field === undefined) return { shape: link.many ? "rows" : "row", fields: link.fields };
      if (link.many) return `${은는(word)} 여러 건입니다. ‘${word} |> 처음 |> 꺼내기 ${from.field}’처럼 한 건을 골라 읽으세요`;
      return link.fields.includes(from.field) ? { shape: "value", fields: [] } : `${word}에 없는 필드: ${from.field}`;
    }
  }
}

/** 서식의 {필드} 를 순서대로. 짝이 안 맞으면 오류 글. */
export function placeholders(text: string): string[] | string {
  const out: string[] = [];
  for (let i = 0; i < text.length; i++) {
    if (text[i] === "}") return "서식의 ‘}’ 짝이 없습니다";
    if (text[i] !== "{") continue;
    const end = text.indexOf("}", i + 1);
    const open = text.indexOf("{", i + 1);
    if (end < 0 || (open >= 0 && open < end)) return "서식의 ‘{’ 가 닫히지 않았습니다";
    const name = text.slice(i + 1, end);
    if (!name) return "서식에 빈 {} 가 있습니다";
    out.push(name); i = end;
  }
  return out;
}

/**
 * 조건 검사. 행을 견주면 마디의 필드는 그 행의 필드다. 값 하나를 견주면 빈 필드가 지금 값이고, 적힌 필드는 바깥의 열이다 —
 * 「비면」 이 대신 읽는 열과 같은 자리(가공열은 이 행의 열, 맞춤 뷰는 「공고.공고명」)이고 같은 검사(<code>outer</code>)를 탄다.
 */
function condProblem(cond: Clause[] | undefined, shape: Shape, fields: string[], outer: (f: string) => string | null): string | null {
  if (!cond?.length) return "조건이 비었습니다";
  for (const c of cond) {
    if (shape === "value") { const p = c.field ? outer(c.field) : null; if (p) return p; continue; }
    if (!c.field) return "견줄 필드를 고르세요";
    if (!fields.includes(c.field)) return `없는 필드: ${c.field}`;
  }
  return null;
}

/** 맞춤 뷰에서 단계가 읽는 행의 열(<code>공고.공고명</code>)이 그 문서에 있는가. 백엔드 Flow.DocField 와 같은 말이다. */
function docField(field: string, env: FlowEnv): string | null {
  const q = qualified(field);
  if (!q) return `문서를 붙여 적으세요 — ‘공고.${field}’처럼 읽습니다`;
  return env.linked[q.link]?.fields.includes(q.column) ? null : `${docWord[q.link]}에 없는 필드: ${q.column}`;
}

function stepProblem(step: Step, input: Shape, fields: string[], env: FlowEnv): string | null {
  const rowish = input === "rows" || input === "row";
  const needField = (f: string | undefined) => f === undefined || f === "" ? "필드를 고르세요" : !fields.includes(f) ? `없는 필드: ${f}` : null;
  const baseField = (f: string) => env.view ? docField(f, env) : env.base.includes(f) ? null : `이 행에 없는 열: ${f}`;
  switch (step.op) {
    case "filter": return condProblem(step.cond, input, fields, baseField);
    case "if": return condProblem(step.cond, input === "row" ? "row" : "value", fields, baseField);
    case "sort": return step.field === undefined ? null : rowish ? needField(step.field) : "값목록은 필드 없이 정렬합니다";
    case "distinct":
      if (!step.fields?.length) return null;
      if (!rowish) return "값목록은 기준 필드 없이 같은 값끼리 하나로 합칩니다";
      for (const f of step.fields) { const p = needField(f); if (p) return p; }
      return null;
    case "take": case "nth": return (step.n ?? 0) >= 1 ? null : "1 이상의 수를 적으세요";
    case "abbrev": {
      if ((step.n ?? 0) < 1) return "보일 개수는 1 이상이어야 합니다";
      const ph = placeholders(step.text ?? "");
      if (typeof ph === "string") return ph;
      const bad = ph.find(p => p !== "앞" && p !== "나머지");
      return bad ? `줄여쓰기에는 {앞}·{나머지}만 씁니다: {${bad}}` : null;
    }
    case "format": {
      const ph = placeholders(step.text ?? "");
      if (typeof ph === "string") return ph;
      const bad = ph.find(p => !fields.includes(p));
      return bad ? `서식에 없는 필드: {${bad}}` : null;
    }
    case "get": return needField(step.field);
    case "sum": case "min": case "max": case "avg":
      if (input === "rows") return needField(step.field);
      return step.field !== undefined ? "값목록에는 필드를 적지 않습니다" : null;
    case "substr": return (step.n ?? 0) >= 1 && (step.length ?? -1) >= 0 ? null : "시작은 1 이상, 길이는 0 이상이어야 합니다";
    case "padStart": return (step.length ?? 0) >= 1 && (step.text ?? "").length >= 1 ? null : "길이와 채울 글자를 적으세요";
    case "ifEmpty": return step.field !== undefined ? baseField(step.field) : null;
    case "map": return step.pairs?.length ? null : "대응할 값을 하나 이상 적으세요";
    case "cases": {
      const branches = step.branches ?? [];
      if (!branches.length) return "경우를 하나 이상 적으세요";
      if (branches.length > maxBranches) return `경우는 ${maxBranches}개까지입니다`;
      for (let i = 0; i < branches.length; i++) {
        const p = condProblem(branches[i].cond, "value", fields, baseField);
        if (p) return `${i + 1}번째 경우: ${p}`;
      }
      return null;
    }
    case "add": case "sub": case "mul": case "div":
      if (step.field !== undefined) return baseField(step.field);
      return step.op === "div" && step.value === 0 ? "0으로 나눌 수 없습니다" : null;
    case "dateDiff": return step.field ? baseField(step.field) : "뺄 날짜 열을 고르세요";
    case "dateFormat": return step.text ? null : "서식을 적으세요";
    default: return null;
  }
}

export function checkFlow(flow: Flow, env: FlowEnv): Checked {
  const n = flow.steps.length + 1;
  const shapes: (Shape | null)[] = Array(n).fill(null);
  const fields: string[][] = Array.from({ length: n }, () => []);
  const errors: FlowIssue[] = [];
  const src = sourceShape(flow.from, env);
  if (typeof src === "string") {
    errors.push({ index: 0, message: src });
    return { shapes, fields, errors, final: null, ok: false };
  }
  shapes[0] = src.shape; fields[0] = src.fields;
  for (let i = 0; i < flow.steps.length; i++) {
    const step = flow.steps[i], input = shapes[i]!;
    const verb = verbByOp.get(step.op);
    if (!verb) { errors.push({ index: i + 1, message: `알 수 없는 연산: ${step.op}` }); break; }
    const out = verb.io.find(([a]) => a === input)?.[1];
    if (!out) {
      const accepts = [...new Set(verb.io.map(([a]) => shapeLabel[a]))].join("·");
      errors.push({ index: i + 1, message: `‘${verbWord(step)}’${은는(verbWord(step)).slice(verbWord(step).length)} ${shapeLabel[input]}에 쓸 수 없습니다. ${accepts}에 쓰는 동사입니다` });
      break;
    }
    const problem = stepProblem(step, input, fields[i], env);
    if (problem) { errors.push({ index: i + 1, message: problem }); break; }
    shapes[i + 1] = out;
    fields[i + 1] = out === "rows" || out === "row" ? fields[i] : [];
  }
  const final = errors.length ? null : shapes[n - 1];
  if (final && !ends.includes(final))
    errors.push({ index: n - 1, final: true, message: `마지막 결과가 ${shapeLabel[final]}입니다. 값·수·날짜 중 하나로 끝나야 저장할 수 있습니다` });
  return { shapes, fields, errors, final: errors.length ? null : final, ok: !errors.length };
}

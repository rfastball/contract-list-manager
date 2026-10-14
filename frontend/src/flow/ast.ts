/**
 * 가공식 v2 — 흐름식의 원본(docs/column-management.md §4.5).
 *
 * <p>원본은 이 JSON 이고 흐름식 글은 그 표준 출력이다. 동사(한국어)와 <code>op</code>(영문)는 1:1 이다.
 * 단계는 동사마다 정해진 자리만 쓴다(verbs.ts 의 <code>args</code>) — 쓰지 않는 자리는 비워 둔다.
 * 그래야 글 → 단계 → 글 왕복이 같은 JSON 을 낸다.</p>
 */

/** 단계마다의 값의 꼴. 행목록·값목록·행·값(글)·수·날짜. */
export type Shape = "rows" | "values" | "row" | "value" | "number" | "date";

export const shapeLabel: Record<Shape, string> = {
  rows: "행목록", values: "값목록", row: "행", value: "값", number: "수", date: "날짜",
};

/** 연결 문서. 자기 자신 종류는 없다. */
export type LinkKind = "notice" | "request" | "contract";
export const linkWord: Record<LinkKind, string> = { notice: "연결공고", request: "연결접수", contract: "연결계약" };
/** 맞춤 뷰의 문서 이름(docs/user-views.md §3) — 글 꼴의 <code>공고.공고명</code>. 백엔드 Flow.DocWords 와 같다. */
export const docWord: Record<LinkKind, string> = { request: "접수", notice: "공고", contract: "계약" };
/** 이 재료에서 연결 문서를 부르는 말 — 가공열은 연결공고, 맞춤 뷰는 공고. */
export const wordOf = (env: Pick<FlowEnv, "view"> | null | undefined, link: LinkKind) => (env?.view ? docWord : linkWord)[link];
/** 맞춤 뷰에서 단계가 읽는 행의 열(<code>공고.공고명</code>)을 (종류, 열)로 가른다. 문서 이름이 앞에 없으면 null. */
export function qualified(field: string): { link: LinkKind; column: string } | null {
  const dot = field.indexOf(".");
  if (dot <= 0) return null;
  const link = (Object.keys(docWord) as LinkKind[]).find(k => docWord[k] === field.slice(0, dot));
  return link ? { link, column: field.slice(dot + 1) } : null;
}
/** 단계가 고를 수 있는 행의 열 — 가공열은 이 행의 기본 열, 맞춤 뷰는 세 문서의 열을 문서 이름을 붙여. */
export function rowColumns(env: FlowEnv): string[] {
  if (!env.view) return env.base;
  return (Object.keys(docWord) as LinkKind[]).flatMap(k => (env.linked[k]?.fields ?? []).map(f => `${docWord[k]}.${f}`));
}

export type Source =
  | { kind: "items" }
  | { kind: "field"; field: string }
  | { kind: "linked"; link: LinkKind; field?: string; items?: true }
  | { kind: "text"; text: string }
  | { kind: "number"; value: number };

export type Compare = "=" | "≠" | ">" | "<" | "≥" | "≤" | "포함" | "비어있음" | "안비어있음";
/**
 * 조건의 한 마디. 행을 견주면 <code>field</code> 는 그 행의 필드다. 값 하나를 견주면 빈 글이 지금 값 자체이고,
 * 적힌 글은 바깥의 열이다(「비면」 이 읽는 자리 — 가공열은 이 행의 열, 맞춤 뷰는 <code>공고.공고명</code>).
 */
export type Clause = { join?: "and" | "or"; field: string; cmp: Compare; value?: string | number };
/** 「경우」 의 갈래 하나 — 조건(값 조건과 같은 규칙)이 맞으면 그 글. 차례대로 견주어 처음 맞는 것이 이긴다. */
export type Branch = { cond: Clause[]; then: string };

export type Op =
  | "filter" | "sort" | "distinct" | "take" | "nth" | "first" | "last" | "format" | "get" | "join" | "abbrev"
  | "count" | "sum" | "min" | "max" | "avg"
  | "trim" | "replace" | "before" | "after" | "substr" | "padStart" | "ifEmpty" | "map" | "if" | "cases"
  | "toNumber" | "add" | "sub" | "mul" | "div" | "round" | "floor" | "thousands" | "plain"
  | "toDate" | "dateAdd" | "dateDiff" | "year" | "month" | "quarter" | "dateFormat";

export type Step = {
  op: Op;
  field?: string;
  fields?: string[];
  text?: string;
  to?: string;
  n?: number;
  length?: number;
  dir?: "desc";
  unit?: "day" | "month" | "year";
  value?: number;
  cond?: Clause[];
  then?: string;
  else?: string;
  pairs?: [string, string][];
  branches?: Branch[];
  otherwise?: string;
};

export type Flow = { from: Source; steps: Step[] };

/** 단계의 조건 마디 모두(「경우」 의 갈래 포함). 값 하나를 견주는 마디의 필드는 바깥의 열이라 맞춤 뷰에서 읽는 문서·열 이름 바꾸기가 이것을 본다. */
export const clausesOf = <C extends { field: string }>(step: { cond?: C[]; branches?: { cond?: C[] }[] }): C[] =>
  [...(step.cond ?? []), ...(step.branches ?? []).flatMap(b => b.cond ?? [])];

export type FormulaV2 =
  | { version: 2; id: string; source: "flow"; flow: Flow }
  | { version: 2; id: string; source: "sql"; sql: string };

/** 가공식 v1 — 물품 행 위의 pick·distinct·format·join·at·get·count. 읽을 때 v2 로 옮긴다. */
export type StepV1 = { op: string; fields?: string[] | null; text?: string | null; index?: number | null };
export type FormulaV1 = { version: 1; id: string; steps: StepV1[] };

/**
 * 꼴 검사와 완성이 보는 재료. 가공열에서는 연결 문서의 사용자 열이 들지 않는다(§4.1).
 * <code>view</code> 면 맞춤 뷰의 재료다(docs/user-views.md §3) — 「이 행」 이 없고(base·items 가 빈다) 세 문서가 모두 한 건이며,
 * 글 꼴은 <code>공고.공고명</code>, 단계가 읽는 행의 열도 같은 꼴이다.
 */
export type FlowEnv = {
  /** 이 행의 기본 열(정정 반영 뒤 값). */
  base: string[];
  /** 이 행의 품목 필드. */
  items: string[];
  /** 이을 수 있는 연결 문서. 자기 종류는 빠진다. many 면 원천이 행목록이다(공고·접수의 연결계약, §11.1-2). */
  linked: Partial<Record<LinkKind, { fields: string[]; items: string[]; many?: boolean }>>;
  /** 맞춤 뷰의 재료인가. */
  view?: true;
};

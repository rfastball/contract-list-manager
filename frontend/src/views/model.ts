import type { UserView, UserViewContext, UserViewContracts } from "../types";
import type { Flow, FlowEnv, FormulaV2, LinkKind } from "../flow/ast";
import { clausesOf, docWord, qualified } from "../flow/ast";
import { presets } from "../flow";
import { toV2 } from "../flow/migrate";
import { printSource, verbWord } from "../flow/syntax";

/**
 * 맞춤 뷰 화면의 재료(docs/user-views.md §7). 화면 없이 셈만 한다 — 열 목록의 후보, 식 요약, 이름 검사, 저장 꼴.
 */

/** 열 목록의 무리 차례 — 접수 → 공고 → 계약. */
export const kinds: LinkKind[] = ["request", "notice", "contract"];

/** 미리보기의 한 줄. formula 가 없으면 「+ 식으로 만들기」 로 막 세운 빈 열이다. */
export type Col = { id: string; name: string; formula: FormulaV2 | null };
/** 고치는 중인 뷰. id 가 없으면 저장 전이다. */
export type Draft = { id: string | null; name: string; contracts: UserViewContracts; columns: Col[] };

/** 맞춤 뷰의 재료 — 「이 행」 이 없고 세 문서가 모두 한 건이다(§3). */
export const envOf = (c: UserViewContext | null): FlowEnv | null => c?.available ? {
  base: [], items: [], view: true,
  linked: Object.fromEntries(c.available.linked.map(l => [l.kind, { fields: l.fields, items: l.items }])),
} : null;

export function newId(): string {
  const uuid = typeof crypto !== "undefined" && "randomUUID" in crypto ? crypto.randomUUID() : Math.random().toString(16).slice(2);
  return `c-${uuid}`;
}

export function fromView(view: UserView): Draft {
  return {
    id: view.id, name: view.name, contracts: view.definition.contracts === "fold" ? "fold" : "split",
    columns: view.definition.columns.map(c => ({ id: c.id, name: c.name, formula: toV2({ ...c.formula, id: (c.formula as { id?: string }).id ?? "" }) })),
  };
}

/** 저장 꼴(§4) — 열의 가공식에는 id 를 싣지 않는다. 식이 없는 열은 빠진다(저장은 그 전에 막힌다). */
export function serialize(draft: Draft) {
  return {
    name: draft.name.trim(),
    definition: {
      version: 2 as const,
      contracts: draft.contracts,
      columns: draft.columns.filter(c => c.formula).map(c => {
        const { id: _drop, ...formula } = c.formula!;
        void _drop;
        return { id: c.id, name: c.name.trim(), formula };
      }),
    },
  };
}

/** 키 차례에 흔들리지 않는 JSON — 열 목록의 후보와 열의 식을 견줄 때 쓴다. */
export function stable(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(stable).join(",")}]`;
  if (value && typeof value === "object")
    return `{${Object.keys(value).sort().map(k => `${JSON.stringify(k)}:${stable((value as Record<string, unknown>)[k])}`).join(",")}}`;
  return JSON.stringify(value);
}

export type Candidate = { key: string; name: string; flow: Flow; link: LinkKind };
export type CandidateGroup = { key: LinkKind; title: string; items: Candidate[] };

const presetNames: Record<string, string> = { first: "대표품명", count: "품목 수", amount: "금액 합계" };

/**
 * 열 목록(§7.4): 접수 → 공고 → 계약 세 무리. 무리마다 그 문서의 열(기본 열 → 사람 열·가공열) 뒤에 품목 묶음 — 대표품명·품목 수,
 * 그 문서의 품목에 금액이 있으면 금액 합계.
 */
export function candidates(env: FlowEnv): CandidateGroup[] {
  return kinds.flatMap(link => {
    const l = env.linked[link];
    if (!l) return [];
    const fields: Candidate[] = l.fields.map(f => ({ key: `${link}:${f}`, name: f, link, flow: { from: { kind: "linked", link, field: f }, steps: [] } }));
    const bundles: Candidate[] = presets({ base: [], items: l.items, linked: {} }).filter(p => p.flow && presetNames[p.id])
      .map(p => ({ key: `${link}:묶음:${p.id}`, name: presetNames[p.id], link, flow: { from: { kind: "linked", link, items: true }, steps: p.flow!.steps } }));
    return [{ key: link, title: docWord[link], items: [...fields, ...bundles] }];
  });
}

/** 흐름식이 읽는 문서 종류 — 원천과 단계가 읽는 행의 열(공고.공고명). SQL 열은 없다. 진짜 다리의 UserViewBuilder.Kinds 와 같다. */
const rowFieldSteps = new Set(["ifEmpty", "add", "sub", "mul", "div", "dateDiff"]);
export function kindsOf(col: Col): Set<LinkKind> {
  const out = new Set<LinkKind>();
  const f = col.formula;
  if (f?.source !== "flow") return out;
  if (f.flow.from.kind === "linked") out.add(f.flow.from.link);
  for (const s of f.flow.steps) {
    const q = s.field !== undefined && rowFieldSteps.has(s.op) ? qualified(s.field) : null;
    if (q) out.add(q.link);
    // 값 하나를 견주는 조건의 마디가 읽는 바깥의 열(만약 공고.f = …).
    for (const c of clausesOf(s)) { const k = qualified(c.field); if (k) out.add(k.link); }
  }
  return out;
}

/** 열의 식 요약. 단계가 없으면 원천, 하나면 원천 |> 동사, 더 있으면 원천 … 마지막 동사. */
export function summary(col: Col): string {
  const f = col.formula;
  if (!f) return "";
  if (f.source === "sql") return "SQL";
  const src = printSource(f.flow.from, { view: true }), steps = f.flow.steps;
  if (!steps.length) return src;
  if (steps.length === 1) return `${src} |> ${verbWord(steps[0])}`;
  return `${src} … ${verbWord(steps[steps.length - 1])}`;
}

/** 열 이름과 원천 열 이름이 같은 그대로 옮기는 열인가. 요약을 흐리게 줄인다(§7.4 — `공고명 = 공고.공고명`). */
export function passThrough(col: Col): boolean {
  const f = col.formula;
  return f?.source === "flow" && !f.flow.steps.length && f.flow.from.kind === "linked" && f.flow.from.field === col.name.trim();
}

/** 열마다의 이름 문제. 빈 이름과 겹친 이름은 그 줄에서 바로 말하고 저장을 막는다. */
export function nameProblems(columns: Col[]): Map<string, string> {
  const out = new Map<string, string>();
  const count = new Map<string, number>();
  for (const c of columns) count.set(c.name.trim(), (count.get(c.name.trim()) ?? 0) + 1);
  for (const c of columns) {
    const name = c.name.trim();
    if (!name) out.set(c.id, "이름을 적으세요.");
    else if ((count.get(name) ?? 0) > 1) out.set(c.id, `‘${name}’ 이름이 겹칩니다. 서식은 이름으로 값을 찾습니다.`);
  }
  return out;
}

/** 담을 때 이름이 겹치면 문서 종류의 이름을 앞에 붙인다(공고와 계약의 수요기관 → 둘째는 계약수요기관). */
export function uniqueName(name: string, columns: Col[], prefix: string): string {
  const taken = new Set(columns.map(c => c.name.trim()));
  if (!taken.has(name)) return name;
  const first = `${prefix}${name}`;
  if (!taken.has(first)) return first;
  for (let n = 2; ; n++) if (!taken.has(`${first}${n}`)) return `${first}${n}`;
}

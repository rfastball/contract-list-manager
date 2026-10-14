import { describe, expect, it } from "vitest";
import type { FlowEnv, LinkKind, Shape } from "./ast";
import { checkFlow } from "./check";
import { evaluate, type EvalInput, type Row } from "./evaluate";
import { toV2 } from "./migrate";
import fixture from "../../../tests/flow-cases.json";

/**
 * 공유 사례표(docs/column-management.md §11.7). xunit(FlowCaseTests)이 <b>같은 파일</b>을 C# 꼴 검사와 SQL 컴파일러로
 * 실제 SQLite 위에서 돌린다 — 두 쪽이 갈리면 한쪽이 붉어진다. 값은 모두 지어낸 것이다.
 */

type Entity = "contract" | "notice" | "request";
type Doc = {
  entity: Entity; base: string; seq: string;
  fields: Row; items: Row[];
  links: Partial<Record<LinkKind, string | string[] | null>>;
};
type Case = {
  name: string; entity: Entity; formula: unknown; migrated?: unknown;
  shapes?: (Shape | null)[]; error?: { index: number }; final?: Record<string, string>;
};

const docs = fixture.docs as unknown as Record<string, Doc>;
const cases = fixture.cases as unknown as Case[];
const envs = fixture.env as unknown as Record<Entity, FlowEnv>;

/** 연결 문서를 평가기 입력으로. 여러 건이면 행 목록, 없으면 null. */
function input(doc: Doc): EvalInput {
  const linked: EvalInput["linked"] = {};
  for (const [kind, target] of Object.entries(doc.links) as [LinkKind, string | string[] | null][]) {
    linked[kind] = Array.isArray(target) ? target.map(id => docs[id].fields)
      : target ? { row: docs[target].fields, items: docs[target].items } : null;
  }
  return { row: doc.fields, items: doc.items, linked };
}

describe("공유 사례표", () => {
  it("사례가 스물다섯을 넘는다", () => expect(cases.length).toBeGreaterThanOrEqual(25));

  it.each(cases.map(c => [c.name, c] as const))("%s", (_, c) => {
    const formula = toV2(c.formula);
    if (c.migrated) expect(formula).toEqual(c.migrated);
    if (formula.source !== "flow") throw new Error("사례는 흐름식이다");
    const env = envs[c.entity];
    const checked = checkFlow(formula.flow, env);
    if (c.shapes) expect(checked.shapes).toEqual(c.shapes);
    if (c.error) {
      expect(checked.ok).toBe(false);
      expect(checked.errors[0].index).toBe(c.error.index);
      return;
    }
    expect(checked.errors).toEqual([]);
    for (const [id, expected] of Object.entries(c.final ?? {})) {
      const doc = docs[id];
      expect(doc.entity).toBe(c.entity);
      const r = evaluate(formula.flow, env, input(doc));
      expect(r.error).toBeUndefined();
      expect([id, r.final]).toEqual([id, expected]);
    }
  });
});

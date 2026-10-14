import type { FormulaV1, FormulaV2, Step } from "./ast";

/**
 * v1 → v2(§4.5). v1 은 물품 행 위에서만 돌았다: <code>pick</code> 으로 필드를 고르고 그 투영 위에서
 * <code>distinct</code> 했다. 그래서 pick 은 단계로 남기지 않고 뒤 단계의 필드로 흡수한다 —
 * distinct 는 고른 필드를 기준으로, at+get 은 n번째+꺼내기로.
 */
export function migrateV1(f: FormulaV1): Extract<FormulaV2, { source: "flow" }> {
  let picked: string[] = [];
  const steps: Step[] = [];
  for (const s of f.steps) {
    switch (s.op) {
      case "pick": picked = [...(s.fields ?? [])]; break;
      case "distinct": steps.push(picked.length ? { op: "distinct", fields: picked } : { op: "distinct" }); break;
      case "at": steps.push({ op: "nth", n: s.index ?? 1 }); break;
      case "get": steps.push({ op: "get", field: s.text ?? "" }); break;
      case "format": steps.push({ op: "format", text: s.text ?? "" }); break;
      case "join": steps.push({ op: "join", text: s.text ?? "" }); break;
      case "count": steps.push({ op: "count" }); break;
      default: throw new Error(`가공식 v1 에 모르는 연산이 있습니다: ${s.op}`);
    }
  }
  return { version: 2, id: f.id, source: "flow", flow: { from: { kind: "items" }, steps } };
}

/** 저장된 가공식을 v2 로. 이미 v2 면 그대로. */
export function toV2(formula: unknown): FormulaV2 {
  const f = formula as { version?: number };
  if (f?.version === 2) return formula as FormulaV2;
  if (f?.version === 1) return migrateV1(formula as FormulaV1);
  throw new Error("가공식의 판을 알 수 없습니다.");
}

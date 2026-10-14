export * from "./ast";
export * from "./verbs";
export * from "./syntax";
export * from "./check";
export * from "./migrate";

import type { Flow, FlowEnv } from "./ast";

/** 빠른 시작의 묶음(§6.4). 보통 단계로 깔린다 — 깔린 뒤에는 여느 단계처럼 고친다. */
export type Preset = { id: string; label: string; name: string; flow: Flow | null; unavailable?: string };

export function presets(env: FlowEnv): Preset[] {
  const name = env.items.includes("세부품명") ? "세부품명" : env.items.includes("품명") ? "품명" : env.items[0];
  const number = env.items.includes("세부품명번호") ? "세부품명번호" : env.items.includes("물품분류번호") ? "물품분류번호" : undefined;
  const amount = env.items.find(f => f === "금액");
  const none = !env.items.length ? "품목 필드가 없습니다" : undefined;
  return [
    { id: "names", label: "이름(번호) 나열", name: `${name ?? "품명"}과 번호`, unavailable: none ?? (number ? undefined : "번호 필드가 없습니다"),
      flow: name && number ? { from: { kind: "items" }, steps: [
        { op: "distinct", fields: [name, number] }, { op: "format", text: `{${name}}({${number}})` }, { op: "join", text: ", " }] } : null },
    { id: "first", label: "첫 품목 외 n종", name: "대표 품명", unavailable: none,
      flow: name ? { from: { kind: "items" }, steps: [
        { op: "get", field: name }, { op: "distinct" }, { op: "abbrev", n: 1, text: "{앞} 외 {나머지}종" }] } : null },
    { id: "count", label: "품목 수", name: "품목 수", flow: { from: { kind: "items" }, steps: [{ op: "count" }] } },
    { id: "amount", label: "금액 합계", name: "품목 합계", unavailable: amount ? undefined : "이 자료의 품목에는 금액이 없습니다",
      flow: amount ? { from: { kind: "items" }, steps: [{ op: "sum", field: amount }, { op: "thousands" }] } : null },
  ];
}

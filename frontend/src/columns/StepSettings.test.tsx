import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { afterEach, describe, expect, it } from "vitest";
import type { Flow, FlowEnv } from "../flow/ast";
import { checkFlow } from "../flow/check";
import { printFlow } from "../flow/syntax";
import { StepSettings } from "./StepSettings";

/**
 * 단계 설정의 조건 편집 — 값 하나를 견주는 조건은 「지금 값」 이 기본이고 「비면」 이 읽는 바깥의 열을 고를 수 있다.
 * 값은 모두 지어낸 것이다.
 */

const env: FlowEnv = {
  base: ["계약번호", "계약건명", "계약금액"],
  items: ["세부품명", "수량"],
  linked: {},
};

/** 고친 흐름을 다시 넣어 주는 받침 — 편집면은 제어 컴포넌트다. 마지막 흐름을 밖에서 본다. */
function Harness({ initial, at = 1, seen }: { initial: Flow; at?: number; seen: { flow: Flow } }) {
  const [flow, setFlow] = useState(initial);
  const checked = checkFlow(flow, env);
  return <StepSettings index={at} flow={flow} env={env} fields={checked.fields[at - 1] ?? []} input={checked.shapes[at - 1]} readOnly={false}
    entityLinks={[]} onMove={() => {}} onRemove={() => {}} onChange={f => { seen.flow = f; setFlow(f); }} />;
}

afterEach(cleanup);

describe("조건 편집", () => {
  it("값 조건은 지금 값이 기본이고 이 행의 열로 바꿀 수 있다", async () => {
    const seen = { flow: null as unknown as Flow };
    render(<Harness seen={seen} initial={{ from: { kind: "field", field: "계약건명" }, steps: [{ op: "if", cond: [{ field: "", cmp: "비어있음" }], then: "", else: "" }] }} />);
    const pick = screen.getByRole("combobox", { name: "견줄 값" }) as HTMLSelectElement;
    expect(pick.value).toBe("");
    expect([...pick.options].map(o => o.text)).toEqual(["지금 값", "계약번호", "계약건명", "계약금액"]);
    await userEvent.selectOptions(pick, "계약금액");
    expect(seen.flow.steps[0].cond).toEqual([{ field: "계약금액", cmp: "비어있음" }]);
    await userEvent.click(screen.getByRole("button", { name: /조건 더하기/ }));
    expect(seen.flow.steps[0].cond?.[1]).toEqual({ join: "and", field: "", cmp: "=", value: "" });
  });

  it("행 조건은 그 행의 필드만 고른다", () => {
    render(<Harness seen={{ flow: null as unknown as Flow }} initial={{ from: { kind: "items" }, steps: [{ op: "filter", cond: [{ field: "수량", cmp: "안비어있음" }] }] }} />);
    expect(screen.queryByRole("combobox", { name: "견줄 값" })).toBeNull();
    const pick = screen.getByRole("combobox", { name: "견줄 열" }) as HTMLSelectElement;
    expect([...pick.options].map(o => o.text)).toEqual(["세부품명", "수량"]);
  });
});

describe("경우 편집", () => {
  const start: Flow = { from: { kind: "field", field: "계약번호" }, steps: [{ op: "cases", branches: [{ cond: [{ field: "", cmp: "안비어있음" }], then: "계약" }] }] };

  it("갈래를 더하고 바깥 열을 견주며 글과 그외를 적는다", async () => {
    const seen = { flow: start };
    render(<Harness seen={seen} initial={start} />);
    await userEvent.click(screen.getByRole("button", { name: /경우 더하기/ }));
    const second = screen.getByRole("group", { name: "경우 2" });
    await userEvent.selectOptions(within(second).getByRole("combobox", { name: "견줄 값" }), "계약건명");
    await userEvent.type(within(second).getByRole("textbox", { name: "경우 2 글" }), "공고");
    await userEvent.type(screen.getByRole("textbox", { name: "그외" }), "접수");
    expect(printFlow(seen.flow)).toBe("계약번호 |> 경우 안비어있음 → \"계약\", 계약건명 안비어있음 → \"공고\", 그외 \"접수\"");
    expect(checkFlow(seen.flow, env).ok).toBe(true);
  });

  it("차례를 옮기고 지운다 — 하나 남으면 지우기가 없다", async () => {
    const seen = { flow: start };
    render(<Harness seen={seen} initial={{ ...start, steps: [{ op: "cases", branches: [
      { cond: [{ field: "", cmp: "안비어있음" }], then: "가" }, { cond: [{ field: "", cmp: "비어있음" }], then: "나" }] }] }} />);
    expect(screen.getByRole("button", { name: "경우 1 위로" })).toHaveProperty("disabled", true);
    await userEvent.click(screen.getByRole("button", { name: "경우 2 위로" }));
    expect(seen.flow.steps[0].branches?.map(b => b.then)).toEqual(["나", "가"]);
    await userEvent.click(screen.getByRole("button", { name: "경우 1 지우기" }));
    expect(seen.flow.steps[0].branches?.map(b => b.then)).toEqual(["가"]);
    expect(screen.queryByRole("button", { name: "경우 1 지우기" })).toBeNull();
  });
});

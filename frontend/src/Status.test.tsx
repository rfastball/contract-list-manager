import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Status } from "./Status";
import type { StatusReport } from "./types";

afterEach(cleanup);

/**
 * 현황 화면이 <b>오는 대로</b> 그리는가.
 *
 * <p>여기서 지킬 것은 무엇이 몇 건 뜨는가가 아니다 — 지표는 뒤의 배열
 * (<code>Status.Metrics</code>) 하나에 있고 버릴 것이다. 지켜야 하는 것은 <b>화면이 알 리
 * 없는 묶음·이름이 와도 그려진다</b>는 것 하나다. 그것이 무너지면 지표를 더하는 일이
 * 화면을 고치는 일이 된다.</p>
 */
describe("Status", () => {
  /** 화면이 알 리 없는 이름들. 붙박이 지표와 한 글자도 겹치지 않게 지어냈다. */
  const 낯선보고: StatusReport = {
    groups: [
      {
        이름: "뒤에서 지은 묶음",
        cells: [
          { 이름: "낯선 칸", 수: 12, 거르개: "요구분류=특수" },
          { 이름: "또 다른 칸", 수: 0, 거르개: null },
        ],
      },
      {
        이름: "두 번째 묶음",
        cells: [{ 이름: "혼자 선 칸", 수: 1234, 거르개: "담당자=홍길동" }],
      },
    ],
  };

  it("화면이 알 리 없는 묶음·이름도 그대로 그린다", () => {
    render(<Status report={낯선보고} onFilter={vi.fn()} />);

    const 묶음 = screen.getByRole("region", { name: "뒤에서 지은 묶음" });
    expect(within(묶음).getByText("낯선 칸")).toBeTruthy();
    expect(within(묶음).getByText("12")).toBeTruthy();

    expect(screen.getByRole("region", { name: "두 번째 묶음" })).toBeTruthy();
    expect(screen.getByText("혼자 선 칸")).toBeTruthy();
    // 큰 수는 자릿점을 찍는다 — 문서에 찍히는 값이 아니라 세어 보는 수다.
    expect(screen.getByText("1,234")).toBeTruthy();
  });

  it("항목 이름이나 건수를 누르면 그 거르개가 그대로 올라온다", async () => {
    const user = userEvent.setup();
    const onFilter = vi.fn();
    render(<Status report={낯선보고} onFilter={onFilter} />);

    await user.click(screen.getByText("낯선 칸"));
    expect(onFilter).toHaveBeenCalledWith("요구분류=특수");

    await user.click(screen.getByText("1,234"));
    expect(onFilter).toHaveBeenLastCalledWith("담당자=홍길동");
  });

  /** 거르개가 없으면 누를 수 없는 줄이다 — 눌러도 아무 일이 없는 단추를 세우지 않는다. */
  it("거르개가 없는 줄에는 단추가 서지 않는다", () => {
    render(<Status report={낯선보고} onFilter={vi.fn()} />);

    expect(screen.queryByRole("button", { name: "또 다른 칸 만 보기" })).toBeNull();
    expect(screen.getAllByRole("button")).toHaveLength(2);
  });

  it("셀 것이 없으면 그렇게 적는다", () => {
    render(<Status report={{ groups: [] }} onFilter={vi.fn()} />);
    expect(screen.getByText("표시할 현황이 없습니다.")).toBeTruthy();
  });
});

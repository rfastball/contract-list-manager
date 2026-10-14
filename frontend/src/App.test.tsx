import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { receiveNotice } from "./bridge";
import { invoke, 사용자뷰초기화 } from "./mock";
import type { DashboardReport } from "./types";

/**
 * 가짜 다리를 <b>그대로 태우되</b> 무엇을 불렀는지만 엿듣는다.
 *
 * <p>배선 시험이라 값이 아니라 <b>부른 이름</b>이 판정인 자리가 있다 — 접수 쪽 단추가 계약 쪽
 * 다리를 부르면 저쪽에 그런 키가 없어 아무 일도 없이 지나가고, 화면만 보아서는 알 수 없다.</p>
 */
vi.mock("./mock", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./mock")>();
  return { ...actual, invoke: vi.fn(actual.invoke) };
});

const 부름 = vi.mocked(invoke);
const 본디 = 부름.getMockImplementation()!;

// jsdom 에는 없다. 표가 커서를 따라 굴리는 자리에서 걸려 넘어지지 않게 둔다.
beforeAll(() => {
  Element.prototype.scrollIntoView = () => {};
  // jsdom은 네이티브 모달의 top layer를 구현하지 않는다. 브라우저 동작을 흉내내지 않고 API만 대역 처리한다.
  HTMLDialogElement.prototype.showModal = function () { this.open = true; };
  HTMLDialogElement.prototype.close = function () { this.open = false; };
});

beforeEach(() => { 부름.mockClear(); 부름.mockImplementation(본디); });

afterEach(cleanup);

/**
 * 화면 전체를 <b>가짜 다리에 붙여</b> 한 번 돌려 본다.
 *
 * <p>부품마다 시험이 있어도 <b>이어 붙이는 자리</b>는 비어 있었다 — 다리에 없는 이름을 부르거나,
 * 탭이 엉뚱한 뷰를 열거나, 단추가 아무 데도 닿지 않는 일은 부품 시험을 모두 통과한다.
 * 여기서 보는 것은 값이 아니라 <b>배선</b>이다.</p>
 */
describe("App", () => {
  it("클릭 없이 변경된 자료와 건수를 갱신하고 같은 버전은 다시 읽지 않는다", async () => {
    let version = 0;
    let fail = false;
    부름.mockImplementation(async (method, args) => {
      if (method === "dataVersion") return version;
      if (method === "dashboard" && fail) throw new Error("일시적인 조회 실패");
      const result = await 본디(method, args);
      if (method === "dashboard" && version > 0) {
        const report = structuredClone(result as DashboardReport);
        report.rows[1].title = "자동 수신된 접수";
        report.documents.requests = 100 + version;
        return report;
      }
      return result;
    });
    vi.useFakeTimers();
    try {
      const { unmount } = render(<App />);
      await act(async () => { await vi.advanceTimersByTimeAsync(500); });
      expect(screen.getByRole("table", { name: "단계별 조달 자료" })).toBeTruthy();
      부름.mockClear();
      await act(async () => { await vi.advanceTimersByTimeAsync(1000); });
      expect(부름).toHaveBeenCalledWith("dataVersion", []);
      expect(부름).not.toHaveBeenCalledWith("dashboard", []);
      expect(부름).not.toHaveBeenCalledWith("status", []);

      // 이미 떠 있는 표의 다시 읽기가 실패하면 표는 그대로 두고 알리기만 한다.
      version++;
      fail = true;
      await act(async () => { await vi.advanceTimersByTimeAsync(1500); });
      expect(screen.queryByRole("heading", { name: "자료를 읽지 못했습니다" })).toBeNull();
      expect(screen.getByRole("table", { name: "단계별 조달 자료" })).toBeTruthy();
      expect(screen.getByRole("alert").textContent).toBe("일시적인 조회 실패");
      // 실패가 이어지는 동안 알림은 다시 서지 않는다.
      await act(async () => { await vi.advanceTimersByTimeAsync(7000); });
      expect(screen.queryByRole("alert")).toBeNull();
      expect(screen.getByRole("table", { name: "단계별 조달 자료" })).toBeTruthy();
      fail = false;
      await act(async () => { await vi.advanceTimersByTimeAsync(1500); });
      expect(screen.getByRole("row", { name: "자동 수신된 접수" })).toBeTruthy();
      expect(screen.getByLabelText("전체 자료 수").textContent).toContain("접수 101");
      // 다시 성공한 뒤의 실패는 또 알린다.
      version++;
      fail = true;
      await act(async () => { await vi.advanceTimersByTimeAsync(1500); });
      expect(screen.getByRole("alert").textContent).toBe("일시적인 조회 실패");
      fail = false;
      await act(async () => { await vi.advanceTimersByTimeAsync(1500); });

      unmount();
      부름.mockClear();
      await act(async () => { await vi.advanceTimersByTimeAsync(2000); });
      expect(부름).not.toHaveBeenCalled();
    } finally {
      cleanup();
      vi.useRealTimers();
    }
  });

  it("늦은 조회와 오류가 다른 탭의 자료를 덮지 않고 실패한 조회를 재시도한다", async () => {
    const user = userEvent.setup();
    let finish!: (value: unknown) => void;
    let fail = true;
    부름.mockImplementation((method, args) => {
      if (method === "sheet" && args[0] === "v_공고") return new Promise(resolve => { finish = resolve; });
      if (method === "sheet" && args[0] === "v_계약" && fail) return Promise.reject(new Error("시험용 조회 실패"));
      return 본디(method, args);
    });
    render(<App />);
    await screen.findByRole("searchbox");
    await user.click(screen.getByRole("tab", { name: /^공고/ }));
    await waitFor(() => expect(finish).toBeDefined());
    expect(screen.queryByRole("grid")).toBeNull();
    await user.click(screen.getByRole("tab", { name: /^계약/ }));
    await screen.findByRole("heading", { name: "자료를 읽지 못했습니다" });
    fail = false;
    await user.click(screen.getByRole("button", { name: "다시 읽기" }));
    await screen.findByRole("columnheader", { name: "계약번호" });
    await act(async () => { finish(await 본디("sheet", ["v_공고"])); });
    expect(screen.getByRole("columnheader", { name: "계약번호" })).toBeTruthy();
    expect(screen.queryByRole("columnheader", { name: "공고종류" })).toBeNull();
  });

  it("구조에서 선택한 접수를 표에 전달한다", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "구조" }));
    const chain = await screen.findByRole("row", { name: "26년한별윈치8종구매" });
    await user.click(within(chain).getByText("MPKPLA26910286-000"));
    await user.click(screen.getByRole("button", { name: "표에서 보기" }));
    expect((await screen.findByRole("searchbox") as HTMLInputElement).value).toBe("MPKPLA26910286-000");
    expect(screen.getByRole("tab", { name: /^접수/ }).getAttribute("aria-selected")).toBe("true");

  });

  it("구조는 문서에 들어갔다 돌아와도 검색어·펼침·커서를 지킨다", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "구조" }));
    await user.type(await screen.findByRole("searchbox", { name: "조달 흐름 검색" }), "한별");
    const folds = (row: HTMLElement) => within(row).getAllByRole("button").filter(b => b.hasAttribute("aria-expanded"));
    const chain = await screen.findByRole("row", { name: "26년한별윈치8종구매" });
    const was = folds(chain)[0].getAttribute("aria-expanded");
    await user.click(folds(chain)[0]);
    const toggled = folds(chain)[0].getAttribute("aria-expanded");
    expect(toggled).not.toBe(was);
    await user.click(within(chain).getByText("MPKPLA26910286-000"));
    const grid = document.querySelector("[aria-activedescendant]")!;
    const cursor = grid.getAttribute("aria-activedescendant");
    expect(cursor).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "표에서 보기" }));
    expect(await screen.findByRole("columnheader", { name: "접수번호" })).toBeTruthy();
    await user.click(screen.getByRole("tab", { name: "대시보드" }));

    expect(await screen.findByRole("searchbox", { name: "조달 흐름 검색" })).toHaveProperty("value", "한별");
    const back = await screen.findByRole("row", { name: "26년한별윈치8종구매" });
    expect(folds(back)[0].getAttribute("aria-expanded")).toBe(toggled);
    expect(document.querySelector("[aria-activedescendant]")!.getAttribute("aria-activedescendant")).toBe(cursor);
  });

  it("브리핑은 단계와 검색을 함께 적용하고 문서에서 돌아와도 작업 조건을 지킨다", async () => {
    const user = userEvent.setup();
    render(<App />);
    expect(await screen.findByRole("region", { name: "저장 자료 집계" })).toBeTruthy();
    const stages = within(screen.getByRole("group", { name: "조달 단계 필터" }));
    await user.click(stages.getByRole("button", { name: /^계약 대기/ }));
    await user.type(screen.getByRole("searchbox", { name: "조달 자료 검색" }), "가람");
    const row = await screen.findByRole("row", { name: "26년 가람 조립대 구매" });
    expect(within(screen.getByRole("table", { name: "단계별 조달 자료" })).getAllByRole("row")).toHaveLength(2);
    expect(screen.queryByRole("columnheader", { name: "분류 근거" })).toBeNull();
    expect(within(row).queryByRole("button", { name: "근거 보기" })).toBeNull();
    await user.click(within(row).getByRole("button", { name: /^공고 R26BK/ }));
    expect(await screen.findByRole("columnheader", { name: "입찰공고번호" })).toBeTruthy();
    expect(screen.getByRole("searchbox")).toHaveProperty("value", "R26BK09012082-000");
    await user.click(screen.getByRole("button", { name: "전체 차수" }));
    await user.click(screen.getByRole("tab", { name: "대시보드" }));
    expect(await screen.findByRole("searchbox", { name: "조달 자료 검색" })).toHaveProperty("value", "가람");
    expect(within(screen.getByRole("group", { name: "조달 단계 필터" })).getByRole("button", { name: /^계약 대기/ }).getAttribute("aria-pressed")).toBe("true");
    await user.type(screen.getByRole("searchbox"), "없는번호");
    expect(await screen.findByRole("heading", { name: "이 조건에 맞는 자료가 없습니다" })).toBeTruthy();
    await user.click(screen.getAllByRole("button", { name: "조건 초기화" })[0]);
    expect(screen.getByRole("searchbox")).toHaveProperty("value", "");
    expect(within(screen.getByRole("group", { name: "조달 단계 필터" })).getByRole("button", { name: /^전체/ }).getAttribute("aria-pressed")).toBe("true");
    await user.click(screen.getByRole("button", { name: "자료 새로고침" }));
    expect(부름).toHaveBeenCalledWith("dashboard", []);
  });

  it("브리핑은 계약 건명·현행 미확정 공고도 찾고, 페이지·금액 정렬·확인 조건을 적용한다", async () => {
    const original = await 본디("dashboard", []) as DashboardReport;
    const zero = { ...original.rows[0], key: "zero", title: "제로 계약", contractAmount: 0,
      contracts: [{ number: "ZERO", title: "별도 계약명", date: "2026/10/08", amount: 0, draft: false, terminated: false }], issues: [] };
    const unknown = { ...zero, key: "unknown", title: "금액 미확인 건", contractAmount: null, contracts: [{ ...zero.contracts[0], number: "UNKNOWN", title: "미확인 계약명", amount: null }], issues: ["금액 누락"] };
    const review = { ...original.rows[1], key: "review", title: "현행 검토 건", stage: null,
      noticeNumber: "", noticeNumbers: ["REVIEW-A-000", "REVIEW-B-000"], contracts: [], contractAmount: null, issues: ["현행 공고를 확인하세요."] };
    const report = { ...original, totalCases: 43, rows: [zero, unknown, review, ...Array.from({ length: 40 }, (_, index) => ({ ...zero, key: `page-${index}`, title: `페이지 자료 ${index}`, contractAmount: index + 1, contracts: [{ ...zero.contracts[0], number: `PAGE-${index}`, title: `페이지 계약 ${index}` }] }))] };
    report.dataCheckCount = 1;
    report.stages = report.stages.map(stage => ({ ...stage, count: report.rows.filter(row => row.stage === stage.key).length }));
    부름.mockImplementation((method, args) => method === "dashboard" ? Promise.resolve(report) : 본디(method, args));
    const user = userEvent.setup();
    render(<App />);
    const search = await screen.findByRole("searchbox", { name: "조달 자료 검색" });
    await user.click(screen.getByRole("button", { name: "다음" }));
    expect(screen.getByText("2 / 2페이지")).toBeTruthy();
    await user.selectOptions(screen.getByRole("combobox", { name: "정렬" }), "amount");
    expect(screen.getByText("1 / 2페이지")).toBeTruthy();
    const table = within(screen.getByRole("table", { name: "단계별 조달 자료" }));
    expect(table.getAllByRole("row")[1].getAttribute("aria-label")).toBe("페이지 자료 39");
    await user.type(search, "ZERO");
    expect(within(screen.getByRole("row", { name: "제로 계약" })).getByText("0원")).toBeTruthy();
    await user.clear(search);
    await user.type(search, "별도 계약명");
    expect(screen.getByRole("row", { name: "제로 계약" })).toBeTruthy();
    await user.clear(search);
    await user.type(search, "UNKNOWN");
    expect(within(screen.getByRole("row", { name: "금액 미확인 건" })).getByText("금액 미확인")).toBeTruthy();
    await user.clear(search);
    await user.type(search, "REVIEW-B");
    expect(within(screen.getByRole("row", { name: "현행 검토 건" })).getByRole("button", { name: "공고 REVIEW-B-000" })).toBeTruthy();
    await user.click(screen.getAllByRole("button", { name: "조건 초기화" })[0]);
    expect(screen.queryByRole("checkbox", { name: /단계 확인 필요/ })).toBeNull();
    const unstaged = within(screen.getByRole("group", { name: "조달 단계 필터" })).getByRole("button", { name: /^단계 미정/ });
    expect(unstaged.textContent).toContain("1");
    await user.click(unstaged);
    expect(unstaged.getAttribute("aria-pressed")).toBe("true");
    expect(within(screen.getByRole("table", { name: "단계별 조달 자료" })).getAllByRole("row")).toHaveLength(2);
    const reviewRow = screen.getByRole("row", { name: "현행 검토 건" });
    expect(within(reviewRow).getByText("자료 확인 필요 1").getAttribute("title")).toBe("현행 공고를 확인하세요.");
    expect(within(reviewRow).getAllByText("단계 미정")).toHaveLength(1);
  });

  it("납품기한은 품목 날짜·상속·미확인을 함께 찾고 기간·해지 조건과 문서 복귀를 유지한다", async () => {
    const report = structuredClone(await 본디("dashboard", []) as DashboardReport);
    const sample = report.deliveries[0];
    const due = (date: string | null, daysRemaining: number | null, raw = date ?? "") => ({ date, daysRemaining, raw });
    const item = (name: string, date: string | null, days: number | null, raw = date ?? "") => ({ lineNo: 1, name, specification: "규격 A", agency: "품목 기관", quantity: "1", unit: "대", due: due(date, days, raw) });
    const mixed = { ...sample, title: "혼합 기한 계약", due: due("2026-10-01", -8), items: [item("특별 품목", "2026-10-09", 0), item("다음 품목", "2026-10-12", 3), item("모호한 품목", null, null, "추후 협의")] };
    report.deliveries = [mixed,
      { ...sample, number: "INHERIT", title: "계약 기한 상속", due: due("2026-10-16", 7), items: [item("빈 기한 품목", null, null)] },
      { ...sample, number: "ITEM-ONLY", title: "품목 기한만 확인", due: due(null, null), items: [item("개별 품목", "2026-10-20", 11)] },
      { ...sample, number: "UNKNOWN-DUE", title: "기한 해석 불가", due: due(null, null, "미정"), items: [item("미정 품목", null, null, "나중")] },
      { ...sample, number: "MONTH", title: "30일 경계 계약", due: due("2026-11-08", 30), items: [] },
      { ...sample, number: "FUTURE", title: "먼 기한 계약", due: due("2026-11-20", 42), items: [] },
      { ...sample, number: "TERMINATED", title: "해지된 계약", terminated: true, due: due("2026-10-11", 2), items: [] }];
    부름.mockImplementation((method, args) => method === "dashboard" ? Promise.resolve(report) : 본디(method, args));
    const user = userEvent.setup(); render(<App />);
    await screen.findByRole("region", { name: "저장 자료 집계" });
    await user.click(screen.getByRole("button", { name: "납품기한" }));
    const filters = () => within(screen.getByRole("group", { name: "납품기한 필터" }));
    const table = () => within(screen.getByRole("table", { name: "납품기한 계약 자료" }));
    expect(table().getAllByRole("row")).toHaveLength(7);
    await user.click(filters().getByRole("button", { name: /^기한 경과/ }));
    expect(table().getAllByRole("row")).toHaveLength(2);
    expect(within(screen.getByRole("row", { name: mixed.title })).getByText("8일 지남")).toBeTruthy();
    await user.type(screen.getByRole("searchbox", { name: "납품기한 자료 검색" }), "다음 품목");
    expect(await screen.findByRole("heading", { name: "이 조건에 맞는 납품기한 자료가 없습니다" })).toBeTruthy();
    await user.clear(screen.getByRole("searchbox", { name: "납품기한 자료 검색" }));
    await user.click(filters().getByRole("button", { name: /^7일 이내/ }));
    expect(table().getAllByRole("row")).toHaveLength(3);
    const row = screen.getByRole("row", { name: mixed.title });
    expect(within(row).getByText("2026-10-09")).toBeTruthy();
    expect(within(row).getByText("특별 품목")).toBeTruthy();
    expect(within(row).queryByText("2026-10-01")).toBeNull();
    await user.click(within(row).getByRole("button", { name: "기한 펼치기" }));
    expect(screen.getByRole("table", { name: `${mixed.number} 품목별 납품기한` }).textContent).toContain("추후 협의");
    await user.type(screen.getByRole("searchbox", { name: "납품기한 자료 검색" }), "특별 품목");
    expect(table().getAllByRole("row")).toHaveLength(2);
    await user.click(within(screen.getByRole("row", { name: mixed.title })).getByRole("button", { name: mixed.number }));
    expect(await screen.findByRole("columnheader", { name: "계약번호" })).toBeTruthy();
    expect(screen.getByRole("searchbox")).toHaveProperty("value", mixed.number);
    await user.click(screen.getByRole("tab", { name: "대시보드" }));
    expect(await screen.findByRole("searchbox", { name: "납품기한 자료 검색" })).toHaveProperty("value", "특별 품목");
    expect(filters().getByRole("button", { name: /^7일 이내/ }).getAttribute("aria-pressed")).toBe("true");
    await user.click(screen.getByRole("button", { name: "기한 조건 초기화" }));
    await user.click(filters().getByRole("button", { name: /^기한 미확인/ }));
    expect(table().getAllByRole("row")).toHaveLength(3);
    expect(screen.queryByRole("row", { name: "품목 기한만 확인" })).toBeNull();
    expect(screen.queryByRole("row", { name: "계약 기한 상속" })).toBeNull();
    await user.click(filters().getByRole("button", { name: /^30일 이내/ }));
    expect(table().getAllByRole("row")).toHaveLength(5);
    await user.click(screen.getByRole("checkbox", { name: "해지 계약 포함" }));
    expect(table().getAllByRole("row")).toHaveLength(6);
    await user.click(filters().getByRole("button", { name: "기간 지정" }));
    await user.type(screen.getByLabelText("시작일"), "2026-10-12");
    await user.type(screen.getByLabelText("종료일"), "2026-10-16");
    expect(table().getAllByRole("row")).toHaveLength(3);
    expect(within(screen.getByRole("row", { name: mixed.title })).getByText("2026-10-12")).toBeTruthy();
    await user.click(within(screen.getByRole("row", { name: "계약 기한 상속" })).getByRole("button", { name: "기한 펼치기" }));
    expect(screen.getByText("계약 기한 적용")).toBeTruthy();
    await user.type(screen.getByRole("searchbox", { name: "납품기한 자료 검색" }), "없는 품목");
    expect(await screen.findByRole("heading", { name: "이 조건에 맞는 납품기한 자료가 없습니다" })).toBeTruthy();
    await user.click(screen.getAllByRole("button", { name: "기한 조건 초기화" })[0]);
    expect(table().getAllByRole("row")).toHaveLength(7);
  });

  /**
   * 계획·현황은 동결되어 탐색에 나오지 않는다. 기본 탭은 통합 그대로다.
   * 방향키·Home·End 는 보이는 차례를 그대로 따른다.
   */
  it("활성 자료 탭만 키보드로 선택하고 처음으로 돌아온다", async () => {
    const user = userEvent.setup();
    render(<App />);

    // 숨긴 계획·현황은 방향키·Home·End로도 닿지 않는다.
    const tabs = await screen.findAllByRole("tab");
    expect(tabs.map((t) => t.id)).toEqual(
      ["tab-통합", "tab-나라장터", "tab-접수", "tab-공고", "tab-계약"]);
    expect(tabs[0].getAttribute("aria-selected")).toBe("true");
    tabs[0].focus();
    await user.keyboard("{ArrowDown}{ArrowDown}");
    expect(document.activeElement).toBe(tabs[2]);
    expect(tabs[2].getAttribute("aria-selected")).toBe("true");
    await user.keyboard("{ArrowDown}{ArrowDown}");
    expect(document.activeElement).toBe(tabs[4]);
    expect(tabs[4].getAttribute("aria-selected")).toBe("true");
    expect(await screen.findByRole("heading", { name: "계약 목록" })).toBeTruthy();
    await user.keyboard("{ArrowDown}");
    expect(document.activeElement).toBe(tabs[0]);
    expect(tabs[0].getAttribute("aria-selected")).toBe("true");
    expect(await screen.findByRole("heading", { name: "대시보드", level: 1 })).toBeTruthy();
    await user.keyboard("{End}");
    expect(document.activeElement).toBe(tabs[4]);
    expect(tabs[4].getAttribute("aria-selected")).toBe("true");
    await user.keyboard("{ArrowDown}");
    expect(document.activeElement).toBe(tabs[0]);
    expect(tabs[0].getAttribute("aria-selected")).toBe("true");
    await user.keyboard("{End}{Home}");
    expect(tabs[0].getAttribute("aria-selected")).toBe("true");
    await user.keyboard("{ArrowUp}");
    expect(document.activeElement).toBe(tabs[4]);
    expect(tabs[4].getAttribute("aria-selected")).toBe("true");
  });

  it("통합은 표로도 볼 수 있다", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "표" }));

    // 표에는 머리글이 선다. 구조 보기에는 없던 것이다.
    expect(await screen.findByRole("columnheader", { name: /계약번호/ })).toBeTruthy();
  });

  it("계약 탭은 계약 뷰를 열고 손으로 채우는 열이 붙는다", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("tab", { name: /^계약/ }));

    expect(await screen.findByRole("columnheader", { name: /진행상태/ })).toBeTruthy();
  });

  /** 접수는 세 번째 개체다. 제 뷰를 열지 못하면 탭만 서고 몸통은 영영 「읽는 중…」에 머문다. */
  it("접수 탭은 v_접수 를 열고 조달요구번호를 낸다", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("tab", { name: /^접수/ }));

    expect(await screen.findByRole("columnheader", { name: "조달요구번호" })).toBeTruthy();
    expect(screen.getByRole("columnheader", { name: "접수번호" })).toBeTruthy();
    expect(부름).toHaveBeenCalledWith("sheet", ["v_접수"]);
  });

  /**
   * 통합이 보는 것은 <b>v_통합</b>이다. 접수 아홉 열이 빠진 표를 열면 아무 말 없이 그럴듯하게
   * 선다 — 그래서 접수 쪽 열이 실제로 서는지 본다.
   */
  it("통합 표는 v_통합을 연다", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "표" }));

    // 이 열은 통합이 접수에서 붙여 오는 것이다.
    expect(await screen.findByRole("columnheader", { name: "조달요구번호" })).toBeTruthy();
    expect(부름).toHaveBeenCalledWith("sheet", ["v_통합"]);
  });

  /** 자료 목록 사이에는 차수 선택을 유지하고 대시보드 보기 방식은 따로 기억한다. */
  it("자료 목록의 차수 선택과 대시보드 보기 방식은 따로 유지한다", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("tab", { name: /^공고/ }));
    await user.click(await screen.findByRole("button", { name: "전체 차수" }));
    // 다리는 가짜 다리를 동적으로 불러 부르므로 누름과 같은 틱에 닿지 않는다 — 기다린다.
    await waitFor(() => expect(부름).toHaveBeenCalledWith("sheet", ["v_공고차수"]));

    await user.click(screen.getByRole("tab", { name: /^계약/ }));
    await waitFor(() => expect(부름).toHaveBeenCalledWith("sheet", ["v_계약차수"]));

    await user.click(screen.getByRole("tab", { name: "대시보드" }));
    expect(await screen.findByRole("region", { name: "저장 자료 집계" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "전체 차수" }));
    await waitFor(() => expect(부름).toHaveBeenCalledWith("sheet", ["v_통합차수"]));
  });

  it("「최신」↔「전체 차수」 전환은 지금 표의 검색어를 이어받고, 사람이 지운 대시보드발 검색어를 되살리지 않는다", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("tab", { name: /^계약/ }));
    await user.type(await screen.findByRole("searchbox", { name: "목록 검색" }), "R26TA");
    await user.click(screen.getByRole("button", { name: "전체 차수" }));
    await waitFor(() => expect(부름).toHaveBeenCalledWith("sheet", ["v_계약차수"]));
    expect(await screen.findByRole("searchbox", { name: "목록 검색" })).toHaveProperty("value", "R26TA");
    // 이어 준 검색어는 초점을 옮기지 않는다 — 누른 칩에 남는다.
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "전체 차수" }));
    await user.click(screen.getByRole("button", { name: "최신" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "최신" }).getAttribute("aria-pressed")).toBe("true"));
    expect(await screen.findByRole("searchbox", { name: "목록 검색" })).toHaveProperty("value", "R26TA");

    // 대시보드에서 번호를 들고 왔다가 지우면, 전환해도 그 번호가 돌아오지 않는다.
    await user.click(screen.getByRole("tab", { name: "대시보드" }));
    await user.click((await screen.findAllByRole("button", { name: /^공고 R/ }))[0]);
    const search = await screen.findByRole("searchbox", { name: "목록 검색" }) as HTMLInputElement;
    await waitFor(() => expect(search.value).toMatch(/^R/));
    await user.clear(search);
    await user.click(screen.getByRole("button", { name: "전체 차수" }));
    await waitFor(() => expect(부름).toHaveBeenCalledWith("sheet", ["v_공고차수"]));
    expect(await screen.findByRole("searchbox", { name: "목록 검색" })).toHaveProperty("value", "");
  });

  it("통합 표를 내보내면 파일·시트 이름이 「대시보드」 가 아니라 보는 표의 이름이다", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "표" }));
    await user.click(await screen.findByLabelText("엑셀 내보내기"));
    await user.click(screen.getByRole("button", { name: /^검색 결과 \d+행 내보내기$/ }));
    await waitFor(() => expect(부름).toHaveBeenCalledWith("exportSheet", ["통합", expect.any(String)]));
    // 고르면 펼침이 접혀 표 위를 가리지 않는다.
    expect((screen.getByLabelText("엑셀 내보내기").closest("details") as HTMLDetailsElement).open).toBe(false);

    await user.click(screen.getByRole("button", { name: "전체 차수" }));
    await waitFor(() => expect(부름).toHaveBeenCalledWith("sheet", ["v_통합차수"]));
    await user.click(await screen.findByLabelText("엑셀 내보내기"));
    await user.click(screen.getByRole("button", { name: /^검색 결과 \d+행 내보내기$/ }));
    await waitFor(() => expect(부름).toHaveBeenCalledWith("exportSheet", ["통합차수", expect.any(String)]));
  });

  it("알림은 마지막 것의 시간만 따른다 — 앞 알림의 타이머가 뒤 알림을 일찍 걷지 않는다", async () => {
    let n = 0;
    부름.mockImplementation(async (m, args) => m === "closeWindow" ? Promise.reject(new Error(`창을 닫지 못했습니다 ${++n}`)) : 본디(m, args));
    vi.useFakeTimers();
    try {
      render(<App />);
      await act(async () => { await vi.advanceTimersByTimeAsync(500); });
      await act(async () => { receiveNotice("closeRequested"); await vi.advanceTimersByTimeAsync(100); });
      expect(screen.getByRole("alert").textContent).toBe("창을 닫지 못했습니다 1");
      await act(async () => { await vi.advanceTimersByTimeAsync(3000); });
      await act(async () => { receiveNotice("closeRequested"); await vi.advanceTimersByTimeAsync(100); });
      expect(screen.getByRole("alert").textContent).toBe("창을 닫지 못했습니다 2");
      // 첫 알림의 6초가 지나도 둘째 알림은 남는다.
      await act(async () => { await vi.advanceTimersByTimeAsync(3500); });
      expect(screen.getByRole("alert").textContent).toBe("창을 닫지 못했습니다 2");
      await act(async () => { await vi.advanceTimersByTimeAsync(3000); });
      expect(screen.queryByRole("alert")).toBeNull();
    } finally {
      cleanup();
      vi.useRealTimers();
    }
  });

  /**
   * 「차수」 눈이 없는 탭에서는 접힌 것을 그대로 낸다. 물러서 주지 않으면 공고에서 펴 둔 채로
   * 접수에 닿는 순간 없는 뷰를 열어 표가 통째로 빈다.
   */
  it("차수 눈이 없는 탭에서는 본 뷰로 물러선다", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("tab", { name: /^공고/ }));
    await user.click(await screen.findByRole("button", { name: "전체 차수" }));

    await user.click(screen.getByRole("tab", { name: /^접수/ }));

    expect(await screen.findByRole("columnheader", { name: "조달요구번호" })).toBeTruthy();
    expect(부름).toHaveBeenCalledWith("sheet", ["v_접수"]);
  });

  /**
   * 차수를 편 표는 <b>고칠 수 없다</b>. 다리가 editable·correctable 을 빈 채로 보내는 것이
   * 그 약속이고(Views.ReadOnly), 화면은 받은 것을 그대로 믿는다 — 여기서 한 번 더 판정하면
   * 목록이 두 벌이 되어 갈릴 자리가 생긴다.
   */
  it("차수를 편 표에서는 취소된 원공고까지 서고 고칠 수 없다", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("tab", { name: /^공고/ }));

    // 최신만 내는 표에는 갈린 건의 현행 한 장만 선다 — 취소된 당초의 번호는 어디에도 없다.
    expect(await screen.findByRole("columnheader", { name: /현행공고/ })).toBeTruthy();
    expect(screen.queryByText("R26BK09011054-000")).toBeNull();

    await user.click(screen.getByRole("button", { name: "전체 차수" }));

    // 펴면 취소공고와 그 당초가 함께 선다. 셋 다 같은 공고건에 매달려 있다.
    // 번호는 제 칸과 남의 관련공고 칸에 함께 서므로 하나로 좁히지 않는다.
    expect((await screen.findAllByText("R26BK09011054-000")).length).toBeGreaterThan(0);
    expect(screen.getAllByText("R26BK09011054-001").length).toBeGreaterThan(0);

    // 고칠 수 있는 표라면 머리글이 그렇게 말한다. 여기에는 그런 열이 하나도 없어야 한다.
    expect(screen.queryByTitle("문서 값을 수정할 수 있는 열 (클릭하면 정렬)")).toBeNull();
    expect(screen.queryByTitle("사용자 입력 열 (클릭하면 정렬)")).toBeNull();
  });

  it("빈 목록에서 바로 만들고, 저장한 이름으로 다시 열며, 떠날 때 변경을 보호한다", async () => {
    사용자뷰초기화();
    부름.mockImplementation((method, args) => method === "userViews" ? Promise.resolve([]) : 본디(method, args));
    const user = userEvent.setup();
    render(<App />);
    const sidebar = within(screen.getByRole("complementary", { name: "작업 탐색" }));
    const 자리 = sidebar.getByRole("button", { name: "보기 만들기" });
    expect(자리.closest(".sidebar-review")).toBeNull();
    await user.click(자리);
    const 영역 = within(await screen.findByRole("main", { name: "내 보기" }));
    const 이름 = await 영역.findByRole("textbox", { name: "보기 이름" });
    expect(이름).toHaveProperty("value", "");
    await waitFor(() => expect(document.activeElement).toBe(이름));
    expect(document.getElementById("workspace")!.hidden).toBe(true);
    await user.type(이름, "월간보고");
    const 필드 = within(영역.getByRole("complementary", { name: "열 목록" }));
    await user.click(await 필드.findByRole("checkbox", { name: "공고명" }));
    await user.click(영역.getByRole("button", { name: "저장" }));
    const 저장됨 = await sidebar.findByRole("button", { name: "월간보고" });
    expect(저장됨.getAttribute("aria-pressed")).toBe("true");
    부름.mockImplementation(본디);
    await user.click(screen.getByRole("tab", { name: /^공고/ }));
    await user.click(저장됨);
    expect(await screen.findByRole("table")).toBeTruthy();
    expect(screen.queryByRole("textbox", { name: "보기 이름" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "보기 편집" }));
    expect(await screen.findByRole("textbox", { name: "보기 이름" })).toHaveProperty("value", "월간보고");
    await user.clear(screen.getByRole("textbox", { name: "보기 이름" }));
    await user.type(screen.getByRole("textbox", { name: "보기 이름" }), "분기보고");
    await user.click(screen.getByRole("button", { name: "저장" }));
    expect(await sidebar.findByRole("button", { name: "분기보고" })).toBeTruthy();
    expect(sidebar.queryByRole("button", { name: "월간보고" })).toBeNull();
    await user.click(sidebar.getByRole("button", { name: "계약서류" }));
    await user.click(await screen.findByRole("button", { name: "보기 편집" }));
    expect(await screen.findByRole("textbox", { name: "보기 이름" })).toHaveProperty("value", "계약서류");
    await user.type(screen.getByRole("textbox", { name: "보기 이름" }), "수정");

    await user.click(screen.getByRole("tab", { name: /^공고/ }));
    const guard = within(await screen.findByRole("dialog", { name: "저장하지 않은 보기 변경" }));
    await user.click(guard.getByRole("button", { name: "변경 버리기" }));
    await waitFor(() => expect(screen.queryByRole("main", { name: "내 보기" })).toBeNull());
    expect(document.getElementById("workspace")!.hidden).toBe(false);
    사용자뷰초기화();
  });

  it("「내 보기 다시 불러오기」 는 빈 「보기 만들기」 가 아니라 첫 보기를 연다", async () => {
    let 실패 = true;
    부름.mockImplementation((method, args) => method === "userViews" && 실패 ? Promise.reject(new Error("보기를 읽지 못했습니다")) : 본디(method, args));
    const user = userEvent.setup();
    render(<App />);
    const sidebar = within(screen.getByRole("complementary", { name: "작업 탐색" }));
    const 다시 = await sidebar.findByRole("button", { name: "내 보기 다시 불러오기" });
    실패 = false;
    await user.click(다시);
    const 영역 = within(await screen.findByRole("main", { name: "내 보기" }));
    expect(await 영역.findByRole("heading", { level: 1, name: "공고문서" })).toBeTruthy();
    expect(영역.queryByRole("textbox", { name: "보기 이름" })).toBeNull();
    expect(await sidebar.findByRole("button", { name: "공고문서" })).toBeTruthy();
  });

  /** 지금 어느 자료를 여는지 창이 말해야 한다 — 명령줄은 이미 첫 줄에 적는다. */
  it("설정이 저장 위치를 적는다", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "설정" }));

    // 경로 구분자는 정규식 이스케이프에 걸리므로 양끝만 본다.
    expect(await screen.findByText(/AppData.+계약자료\.pclm/)).toBeTruthy();
    // 작업자료를 가리키는 홈도 함께 적는다 — 쪽지·백업·확장 연결이 거기 있다.
    expect(await screen.findByText(/홈 C:.+Pclm/)).toBeTruthy();
  });

  /**
   * 자리 옮기기는 「다음 실행에 옮긴다」 는 예약이 아니다 — 그 예약은 그 사이의 편집이 어디에 쌓이는지를 흐렸다.
   * 이제 옮기기는 전환 절차(ADR-032)로 그 자리에서 끝나고, 고른 자리를 확인받기 전에는 아무것도 하지 않는다.
   */
  it("자리 옮기기는 예약하지 않고 확인을 받기 전에는 아무것도 바꾸지 않는다", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "설정" }));

    const dialog = await screen.findByRole("dialog", { name: "설정" });
    await user.click(await within(dialog).findByRole("button", { name: "작업자료 옮기기…" }));

    expect(await screen.findByRole("dialog", { name: "작업자료 옮기기" })).toBeTruthy();
    expect(부름.mock.calls.map(([m]) => m)).not.toContain("moveWorkfile");
    expect(부름.mock.calls.map(([m]) => m)).not.toContain("moveDataLocation");
  });

  it("설정에서 제출자 이름을 고친다", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "설정" }));

    const dialog = await screen.findByRole("dialog", { name: "설정" });
    const 이름 = within(dialog).getByDisplayValue("홍길동");
    await user.clear(이름);
    await user.type(이름, "김철수");

    await user.click(within(dialog).getByRole("button", { name: "저장" }));
    expect(await screen.findByText("설정을 저장했습니다.")).toBeTruthy();
  });

  // ── 찾기 단축키 ──────────────────────────────────────

  /**
   * 표에 초점이 있을 때만 듣던 시절, 창을 막 연 사람이 눌러도 아무 일이 없었다.
   * 칸에 적힌 안내가 사실이 아니게 되는 자리다.
   */
  it("아무 데도 초점이 없어도 Ctrl+F 로 찾기 칸에 간다", async () => {
    const user = userEvent.setup();
    render(<App />);

    const box = await screen.findByRole("searchbox");
    expect(document.activeElement).not.toBe(box);

    await user.keyboard("{Control>}f{/Control}");

    expect(document.activeElement).toBe(box);
  });

  it("표 탭에서도 듣는다", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("tab", { name: /^계약/ }));
    await screen.findByRole("columnheader", { name: /계약번호/ });

    await user.keyboard("{Control>}f{/Control}");

    expect(document.activeElement).toBe(screen.getByRole("searchbox"));
  });

  /** 가려진 칸으로 초점이 가면, 눌러도 아무 일이 없어 보이는 채로 글자가 안 보이는 곳에 쌓인다. */
  it("설정 창이 떠 있으면 물러난다", async () => {
    const user = userEvent.setup();
    render(<App />);

    const box = await screen.findByRole("searchbox");
    await user.click(screen.getByRole("button", { name: "설정" }));
    const dialog = await screen.findByRole("dialog", { name: "설정" });
    expect(dialog.contains(document.activeElement)).toBe(true);

    await user.keyboard("{Control>}f{/Control}");

    expect(document.activeElement).not.toBe(box);
    fireEvent(dialog, new Event("cancel", { cancelable: true }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "설정" }));
  });

  // ── 손으로 채우는 열 ─────────────────────────────────

  /** 미리 세워 주는 열은 둘뿐이고, 진행상태는 후보 목록이 아니라 자유 입력이다. */
  it("진행상태는 드롭다운이 아니라 자유 입력이다", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("tab", { name: /^계약/ }));
    await screen.findByRole("columnheader", { name: /진행상태/ });

    // 진행상태 칸을 열어 본다 — 고르기 상자가 아니라 입력칸이어야 한다.
    await user.click(screen.getAllByText("준비")[0]);
    await user.keyboard("{F2}");

    expect(screen.queryByRole("combobox")).toBeNull();
    expect(screen.getByRole("textbox")).toBeTruthy();
  });

  /**
   * 새 열은 목록 맨 아래 「+ 새 열」 줄에서 연다 — 이름 뒤에 `:` 를 치면 입력열, `=` 를 치면 가공열(§6.1).
   * 저장하기 전에는 다리에 아무것도 쓰지 않는다.
   */
  it("열을 더하면 표에 곧바로 선다", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "구조" }));
    await user.click(await screen.findByRole("button", { name: "열 관리" }));
    await user.type(await screen.findByRole("textbox", { name: "새 열 이름" }), "담당:");
    expect(screen.getByRole("textbox", { name: "열 이름" })).toHaveProperty("value", "담당");
    expect(screen.getByRole("button", { name: /^입력열\. / }).textContent).toBe(":");
    const preview = within(await screen.findByRole("region", { name: "목록 미리보기" }));
    expect(await preview.findByRole("columnheader", { name: "담당" })).toBeTruthy();
    expect(preview.getByText(/새 열은 사용자 열 맨 뒤에 붙습니다/)).toBeTruthy();
    await user.type(screen.getByLabelText("입력해 보기"), "미리보기 담당자");
    expect(preview.getAllByText("미리보기 담당자").length).toBeGreaterThan(0);
    expect(부름.mock.calls.some(([method]) => method === "setField" || method === "addColumn")).toBe(false);
    await user.click(screen.getByRole("button", { name: "저장" }));
    await waitFor(() => expect(부름).toHaveBeenCalledWith("addColumn", ["contract", "담당", "text", ""]));
    expect(await screen.findByRole("button", { name: /^담당 · 입력 · 글/ })).toBeTruthy();
    await screen.findByText(/열을 저장했습니다\.$/);
    await user.click(screen.getByRole("button", { name: "닫기" }));
    await user.click(await screen.findByText("기본 열과 첫 문서의 값 보기"));
    expect(await screen.findByRole("rowheader", { name: "계약금액" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "목록으로 돌아가기" }));
    await user.click(screen.getByRole("tab", { name: /^계약/ }));

    expect(await screen.findByRole("columnheader", { name: /담당/ })).toBeTruthy();
  });

  /** 창 닫기도 탭을 옮길 때와 같은 확인을 거친다 — 앱은 깃발을 보고 닫기를 멈춘 뒤 「닫기 요청」 을 보낸다. */
  it("저장하지 않은 열 편집이 있으면 창 닫기 요청에 확인을 띄우고, 버려야 정말 닫는다", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "구조" }));
    await user.click(await screen.findByRole("button", { name: "열 관리" }));
    await user.type(await screen.findByRole("textbox", { name: "새 열 이름" }), "담당:");
    expect(부름).not.toHaveBeenCalledWith("setUnsaved", ["true"]);
    await user.type(screen.getByRole("textbox", { name: "열 이름" }), "자");
    await waitFor(() => expect(부름).toHaveBeenCalledWith("setUnsaved", ["true"]));

    await act(async () => receiveNotice("closeRequested"));
    const guard = within(await screen.findByRole("dialog", { name: "저장하지 않은 열 변경" }));
    expect(부름).not.toHaveBeenCalledWith("closeWindow", []);
    await user.click(guard.getByRole("button", { name: "계속 편집" }));
    expect(부름).not.toHaveBeenCalledWith("closeWindow", []);

    await act(async () => receiveNotice("closeRequested"));
    await user.click(within(await screen.findByRole("dialog", { name: "저장하지 않은 열 변경" })).getByRole("button", { name: "변경 버리기" }));
    await waitFor(() => expect(부름).toHaveBeenCalledWith("closeWindow", []));
  });

  it("저장하지 않은 편집이 없으면 창 닫기 요청을 곧바로 닫기로 돌려보낸다", async () => {
    render(<App />);
    await screen.findByRole("region", { name: "저장 자료 집계" });
    await act(async () => receiveNotice("closeRequested"));
    await waitFor(() => expect(부름).toHaveBeenCalledWith("closeWindow", []));
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("고르기 꼴은 후보를 칩으로 넣고 다리에는 쉼표로 넘긴다", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "구조" }));
    await user.click(await screen.findByRole("button", { name: "열 관리" }));
    await user.type(await screen.findByRole("textbox", { name: "새 열 이름" }), "검토단계");
    await user.click(screen.getByRole("button", { name: "입력열 만들기" }));
    await user.click(within(screen.getByRole("group", { name: "값 종류" })).getByRole("button", { name: "고르기" }));
    await user.type(screen.getByRole("textbox", { name: "선택 항목" }), "검토 전, 검토 중{Enter}");
    expect(screen.getByRole("button", { name: "검토 전 빼기" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "저장" }));
    await waitFor(() => expect(부름).toHaveBeenCalledWith("addColumn", ["contract", "검토단계", "choice", "검토 전|검토 중"]));
  });

  it("이미 있는 이름은 막고 까닭을 알린다", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "구조" }));
    await user.click(await screen.findByRole("button", { name: "열 관리" }));
    await user.type(await screen.findByRole("textbox", { name: "새 열 이름" }), "메모");
    await user.click(screen.getByRole("button", { name: "입력열 만들기" }));
    await user.click(screen.getByRole("button", { name: "저장" }));

    expect(await within(screen.getByRole("region", { name: "열 관리" })).findByText(/이미 있는 열입니다/)).toBeTruthy();
    expect(screen.getByRole("textbox", { name: "열 이름" })).toHaveProperty("value", "메모");
  });

  /** 표가 이미 쓰는 이름이 들어가면 SQLite 가 조용히 뒤엣것의 이름을 바꾼다. */
  it("표가 쓰는 이름도 막는다", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "구조" }));
    await user.click(await screen.findByRole("button", { name: "열 관리" }));
    await user.type(await screen.findByRole("textbox", { name: "새 열 이름" }), "계약금액:");
    await user.click(screen.getByRole("button", { name: "저장" }));

    expect(await within(screen.getByRole("region", { name: "열 관리" })).findByText(/표가 이미 쓰는 이름입니다/)).toBeTruthy();
    expect(screen.getByRole("textbox", { name: "열 이름" })).toHaveProperty("value", "계약금액");
  });

  /** 열 제거는 정의줄 메뉴에 있고, 확인해야 하며 취소하면 표의 열 정의를 유지한다. */
  it("열을 지우려면 한 번 더 물어본다", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "구조" }));
    await user.click(await screen.findByRole("button", { name: "열 관리" }));
    await user.click(await screen.findByRole("button", { name: /^진행상태 · 입력/ }));
    await user.click(screen.getByRole("button", { name: "열 메뉴" }));
    await user.click(screen.getByRole("menuitem", { name: "열 제거…" }));
    let dialog = await screen.findByRole("dialog", { name: "열 제거 확인" });
    await user.click(within(dialog).getByRole("button", { name: "취소" }));
    expect(screen.getByRole("textbox", { name: "열 이름" })).toHaveProperty("value", "진행상태");

    await user.click(screen.getByRole("button", { name: "열 메뉴" }));
    await user.click(screen.getByRole("menuitem", { name: "열 제거…" }));
    dialog = await screen.findByRole("dialog", { name: "열 제거 확인" });
    await user.click(within(dialog).getByRole("button", { name: "열 제거" }));
    await waitFor(() => expect(screen.queryByRole("button", { name: /^진행상태 · 입력/ })).toBeNull());
  });

  /** 검색칸도 같은 문법이다 — `=` 로 시작하면 가공열만, `:` 로 시작하면 입력열만. */
  it("검색의 = 와 : 가 열의 성질로 거른다", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "구조" }));
    await user.click(await screen.findByRole("button", { name: "열 관리" }));
    await user.click(within(await screen.findByRole("group", { name: "자료 종류" })).getByRole("button", { name: "공고" }));
    const list = within(screen.getByRole("navigation", { name: "열 목록" }));
    expect(await list.findByRole("button", { name: /^품명 모음 · 식 · …잇기/ })).toBeTruthy();
    await user.type(list.getByRole("searchbox", { name: "열 이름 검색" }), "=");
    expect(list.queryByRole("button", { name: /^메모 · 입력/ })).toBeNull();
    expect(list.getByRole("button", { name: /^품명 모음 · 식/ })).toBeTruthy();
    await user.clear(list.getByRole("searchbox", { name: "열 이름 검색" }));
    await user.type(list.getByRole("searchbox", { name: "열 이름 검색" }), ":메");
    expect(list.getByRole("button", { name: /^메모 · 입력/ })).toBeTruthy();
    expect(list.queryByRole("button", { name: /^품명 모음/ })).toBeNull();
  });

  // ── 고치기와 지우기 ───────────────────────────────────
  // 부품 시험이 아니라 배선을 본다 — 다리에 없는 이름을 부르거나 표를 다시 읽지 않는 일.

  /**
   * 파서가 읽은 값도 고칠 수 있어야 한다. 검산을 하지 않기로 한 이상 틀린 값을 바로잡는 것은
   * 오직 사람인데, 지금까지 그 사람에게 연필이 없었다.
   */
  it("파서가 읽은 칸을 고치면 그 값이 표에 선다", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("tab", { name: /^계약/ }));
    await screen.findByRole("columnheader", { name: /계약건명/ });

    // 같은 건명이 여러 줄에 서므로 글자가 아니라 자리로 짚는다 (0행은 머리글).
    const 건명 = () => within(screen.getAllByRole("row")[1]).getAllByRole("cell")[4];

    await user.click(건명());
    await user.keyboard("{F2}");
    await user.clear(screen.getByRole("textbox"));
    await user.keyboard("고쳐 적은 건명{Enter}");

    await waitFor(() => expect(건명().textContent).toBe("고쳐 적은 건명"));
  });

  it("고친 값의 저장은 됐는데 다시 읽기만 실패하면 저장 실패가 아니라 「저장은 완료했습니다」 로 가른다", async () => {
    let 고침 = false;
    부름.mockImplementation(async (m, args) => {
      if (m === "setOverride") { 고침 = true; return 본디(m, args); }
      if (m === "sheet" && 고침) throw new Error("일시적인 조회 실패");
      return 본디(m, args);
    });
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("tab", { name: /^계약/ }));
    await screen.findByRole("columnheader", { name: /계약건명/ });
    const 건명 = () => within(screen.getAllByRole("row")[1]).getAllByRole("cell")[4];
    await user.click(건명());
    await user.keyboard("{F2}");
    await user.clear(screen.getByRole("textbox"));
    // 앞 시험이 가짜 다리에 남긴 값과 겹치지 않게 다른 글자로 — 같으면 저장하러 가지 않는다.
    await user.keyboard("다시 읽기 전에 고친 건명{Enter}");

    expect((await screen.findByRole("alert", {}, { timeout: 3000 })).textContent).toBe("저장은 완료했습니다. 화면을 다시 읽어 주세요. 일시적인 조회 실패");
    expect(고침).toBe(true);
  });

  /**
   * 쌓는 길만 있고 걷어내는 길이 없으면 잘못 들어온 자료가 영원히 남는다.
   * 되돌릴 수 없는 일이라 <b>세어 보인 뒤에</b> 지운다.
   */
  it("줄을 지우려면 무엇이 사라지는지 먼저 보인다", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "표" }));
    await user.click((await screen.findAllByRole("checkbox", { name: / 행 선택$/ }))[0]);
    await user.click(await screen.findByRole("button", { name: "선택 자료 삭제…" }));

    const dialog = await screen.findByRole("dialog", { name: "자료 삭제" });
    await user.selectOptions(within(dialog).getByRole("combobox", { name: "삭제할 자료 종류" }), "contract");
    expect(await within(dialog).findByText("함께 삭제되는 자료")).toBeTruthy();
    expect(within(dialog).getByText(/되돌릴 수 없습니다/)).toBeTruthy();

    await user.click(within(dialog).getByRole("button", { name: "삭제" }));

    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: "자료 삭제" })).toBeNull());
    expect(await screen.findByText(/삭제했습니다/)).toBeTruthy();
    // 창은 다시 읽은 뒤에 닫혀, 연 단추가 지운 줄과 함께 걷혔어도 초점이 body 로 떨어지지 않는다.
    expect(document.activeElement).not.toBe(document.body);
    expect(document.activeElement!.isConnected).toBe(true);
  });

  it("그만두면 아무것도 지우지 않는다", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "표" }));
    await user.click((await screen.findAllByRole("checkbox", { name: / 행 선택$/ }))[0]);
    await user.click(await screen.findByRole("button", { name: "선택 자료 삭제…" }));

    const dialog = await screen.findByRole("dialog", { name: "자료 삭제" });
    await user.click(within(dialog).getByRole("button", { name: "취소" }));

    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: "자료 삭제" })).toBeNull());
    expect(screen.queryByText(/삭제했습니다/)).toBeNull();
  });

  /** CLI 에만 있던 길이다. 화면에서도 뽑히는지, 그리고 어디에 떨어졌는지 말해 주는지 본다. */
  it("엑셀로 내보내기가 저장한 자리를 알린다", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "표" }));
    await user.click(await screen.findByLabelText("엑셀 내보내기"));
    expect(screen.queryByRole("button", { name: "전체 자료 내보내기" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "설정" }));
    await user.click(within(await screen.findByRole("dialog", { name: "설정" })).getByRole("button", { name: "전체 자료 내보내기" }));

    expect(await screen.findByText(/내보냈습니다.*관리대장_\d{8}\.xlsx/)).toBeTruthy();
  });
});

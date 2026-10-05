import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { invoke } from "./mock";
import type { Outline, Summary } from "./types";

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
      if (method === "outline" && fail) throw new Error("일시적인 조회 실패");
      const result = await 본디(method, args);
      if (method === "status") return { ...(result as Summary), requestBases: 100 + version };
      if (method === "outline" && version > 0) {
        const tree = structuredClone(result as Outline);
        tree.chains[0].request!.title = "자동 수신된 접수";
        return tree;
      }
      return result;
    });
    vi.useFakeTimers();
    try {
      const { unmount } = render(<App />);
      await act(async () => { await vi.advanceTimersByTimeAsync(500); });
      expect(screen.getByRole("row", { name: "26년한별윈치8종구매" })).toBeTruthy();
      부름.mockClear();
      await act(async () => { await vi.advanceTimersByTimeAsync(1000); });
      expect(부름).toHaveBeenCalledWith("dataVersion", []);
      expect(부름).not.toHaveBeenCalledWith("outline", []);
      expect(부름).not.toHaveBeenCalledWith("status", []);

      version++;
      fail = true;
      await act(async () => { await vi.advanceTimersByTimeAsync(1500); });
      expect(screen.getByRole("heading", { name: "자료를 읽지 못했습니다" })).toBeTruthy();
      fail = false;
      await act(async () => { await vi.advanceTimersByTimeAsync(1500); });
      expect(screen.getByRole("row", { name: "자동 수신된 접수" })).toBeTruthy();
      expect(screen.getByLabelText("전체 자료 수").textContent).toContain("접수 101");

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

  it("구조에서 선택한 접수를 표와 연결 검토에 전달한다", async () => {
    const user = userEvent.setup();
    render(<App />);
    const chain = await screen.findByRole("row", { name: "26년한별윈치8종구매" });
    await user.click(within(chain).getByText("MPKPLA26910286-000"));
    await user.click(screen.getByRole("button", { name: "표에서 보기" }));
    expect((await screen.findByRole("searchbox") as HTMLInputElement).value).toBe("MPKPLA26910286-000");
    expect(screen.getByRole("tab", { name: /^접수/ }).getAttribute("aria-selected")).toBe("true");
    await user.click(screen.getByRole("tab", { name: "통합" }));
    await user.click(screen.getByRole("button", { name: "구조" }));
    await user.click(within(await screen.findByRole("row", { name: "26년한별윈치8종구매" })).getByText("MPKPLA26910286-000"));
    await user.click(screen.getByRole("button", { name: "선택 건 연결 검토" }));
    const list = await screen.findByRole("listbox", { name: "접수 목록" });
    expect(within(list).getByRole("option", { selected: true }).textContent).toContain("MPKPLA26910286");
  });

  it("처음 열면 구조 보기가 선다", async () => {
    render(<App />);

    // 통합 탭이 골라져 있고, 몸통은 사슬 카드다.
    await waitFor(() => expect(screen.getByRole("tab", { name: "통합" })).toHaveProperty("ariaSelected", "true"));
    expect(await screen.findByRole("searchbox")).toBeTruthy();
    expect(await screen.findByRole("row", { name: "26년한별윈치8종구매" })).toBeTruthy();

    // 빠진 칸은 지워지지 않고 자리를 지킨다 — 지우면 무엇이 없는지 보이지 않는다.
    expect(screen.getAllByText("(아직 없음)").length).toBeGreaterThan(0);
  });

  /**
   * 통합이 맨 앞이고, 그다음은 나라장터 한 원천에서 갈라지는 접수 → 공고 → 계약이다. 계획은 그 앞칸이 아니라
   * <b>분모</b>라 갈래 뒤에, 현황은 지표가 아직 가상이라 맨 뒤에 선다 — 기본 탭은 통합 그대로다.
   * 방향키·Home·End 는 보이는 차례를 그대로 따른다.
   */
  it("일곱 자료 탭을 키보드로 선택하고 처음으로 돌아온다", async () => {
    const user = userEvent.setup();
    render(<App />);

    const tabs = await screen.findAllByRole("tab");
    expect(tabs.map((t) => t.id)).toEqual(
      ["tab-통합", "tab-나라장터", "tab-접수", "tab-공고", "tab-계약", "tab-계획", "tab-현황"]);
    tabs[0].focus();
    await user.keyboard("{ArrowDown}");
    expect(document.activeElement).toBe(tabs[1]);
    expect(tabs[1].getAttribute("aria-selected")).toBe("true");
    expect(await screen.findByRole("heading", { name: "나라장터", level: 1 })).toBeTruthy();
    await user.keyboard("{ArrowDown}");
    expect(document.activeElement).toBe(tabs[2]);
    expect(tabs[2].getAttribute("aria-selected")).toBe("true");
    await user.keyboard("{ArrowDown}{ArrowDown}{ArrowDown}");
    expect(tabs[5].getAttribute("aria-selected")).toBe("true");
    expect(await screen.findByRole("heading", { name: "조달 계획" })).toBeTruthy();
    await user.keyboard("{End}");
    expect(tabs[6].getAttribute("aria-selected")).toBe("true");
    await user.keyboard("{ArrowDown}");
    expect(document.activeElement).toBe(tabs[0]);
    expect(tabs[0].getAttribute("aria-selected")).toBe("true");
    await user.keyboard("{ArrowUp}");
    expect(tabs[6].getAttribute("aria-selected")).toBe("true");
    await user.keyboard("{Home}");
    expect(tabs[0].getAttribute("aria-selected")).toBe("true");
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

  /**
   * 차수를 편 표는 <b>같은 자료를 접지 않고 본 것</b>이라 탭이 아니라 눈이다.
   *
   * <p>공고 탭에서 펴 두고 통합으로 건너가면 통합도 펴진 채로 서야 한다 — 눈이 탭마다
   * 따로 놀면 어느 탭이 무엇을 보이고 있는지를 탭마다 따로 기억해야 한다.</p>
   */
  it("차수 눈은 탭을 옮겨도 따라온다", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("tab", { name: /^공고/ }));
    await user.click(await screen.findByRole("button", { name: "전체 차수" }));
    // 다리는 가짜 다리를 동적으로 불러 부르므로 누름과 같은 틱에 닿지 않는다 — 기다린다.
    await waitFor(() => expect(부름).toHaveBeenCalledWith("sheet", ["v_공고차수"]));

    await user.click(screen.getByRole("tab", { name: /^계약/ }));
    await waitFor(() => expect(부름).toHaveBeenCalledWith("sheet", ["v_계약차수"]));

    await user.click(screen.getByRole("tab", { name: "통합" }));
    await waitFor(() => expect(부름).toHaveBeenCalledWith("sheet", ["v_통합차수"]));
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

  /**
   * 연결되지 않은 채 남은 것이 몇인지는 머리에서 사라지면 안 된다.
   *
   * <p>계약과 접수를 <b>함께</b> 센다. 갈래마다 따로 세우면 접수 큐는 그 갈래를 열어 보기
   * 전까지 없는 것과 같다.</p>
   */
  it("연결되지 않은 계약과 접수 수를 배지로 낸다", async () => {
    render(<App />);

    // 요약이 늦게 오므로 배지가 설 때까지 기다린다. mock.ts — 계약 3 · 접수 1.
    const 배지 = await screen.findByLabelText("연결되지 않은 계약·접수 4건");
    expect(배지.closest("button")?.textContent).toContain("공고 연결");
  });

  /** 공고 연결은 탭이 아니라 단추다. 눌러야 열리고 다시 누르면 닫힌다. */
  it("공고 연결은 단추로 열고 닫는다", async () => {
    const user = userEvent.setup();
    render(<App />);

    const 연결 = await screen.findByRole("button", { name: /공고 연결/ });
    await user.click(연결);

    const list = await screen.findByRole("listbox", { name: "계약 목록" });
    expect(within(list).getByText("26년 가람 절단기 구매")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "계약 ↔ 공고" }));
    expect(screen.getByRole("listbox", { name: "계약 목록" })).toBeTruthy();

    // 탭을 누르면 공고 연결이 닫히고 몸통이 돌아온다.
    await user.click(screen.getByRole("tab", { name: /^공고/ }));
    expect(screen.queryByRole("listbox", { name: "계약 목록" })).toBeNull();
  });

  /**
   * 접수도 계약도 공고 하나에 매달린다. 얼개가 같아 <b>화면은 하나를 쓰되 다리는 갈라져야
   * 한다</b> — 접수 쪽 단추가 계약 쪽 다리를 부르면 저쪽에 그런 키가 없어 아무 일도 없이
   * 지나가고, 화면만 보아서는 눌린 것과 구별이 가지 않는다.
   */
  it("「접수 ↔ 공고」 갈래는 접수 쪽 다리를 부른다", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: /공고 연결/ }));
    await user.click(await screen.findByRole("button", { name: "접수 ↔ 공고" }));

    // 왼쪽에 서는 것이 접수라고 화면이 적는다 — 견주기 표의 머리가 그 낱말을 쓴다.
    const list = await screen.findByRole("listbox", { name: "접수 목록" });
    expect(within(list).getByText("26년가람절단기구매")).toBeTruthy();
    expect((await screen.findAllByRole("columnheader", { name: "접수" })).length).toBeGreaterThan(0);

    // 물리치기 — 그 짝만 후보에서 걷힌다.
    const 뒤엣것 = (await screen.findByText("R26BK09017099-000")).closest(".card")!;
    await user.click(within(뒤엣것 as HTMLElement).getByRole("button", { name: "후보 제외" }));
    await waitFor(() => expect(부름).toHaveBeenCalledWith(
      "rejectRequestLink", ["MPKPLA26910412", "R26BK09017099-000"]));

    // 확정
    const 앞엣것 = (await screen.findByText("R26BK09017030-001")).closest(".card")!;
    await user.click(within(앞엣것 as HTMLElement).getByRole("button", { name: "연결" }));
    await waitFor(() => expect(부름).toHaveBeenCalledWith(
      "confirmRequestLink", ["MPKPLA26910412", "R26BK09017030-001"]));

    // 끊기 — 이어진 것은 「연결됨」 쪽에 서 있어야 되돌릴 수 있다.
    await user.click(await screen.findByRole("button", { name: /^연결됨/ }));
    await user.click(await screen.findByRole("button", { name: "연결 해제" }));
    await waitFor(() => expect(부름).toHaveBeenCalledWith("unlinkRequest", ["MPKPLA26910286"]));
  });

  /**
   * 참조를 담은 자료가 늦게 들어오면 명시 참조로 다시 이어야 한다. 그 길이 창에 없으면
   * 배포물에서는 이을 수 없다 — 배포물에는 명령줄이 없다.
   */
  it("명시 참조로 다시 잇기가 다리를 부르고 결과를 알린다", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: /공고 연결/ }));
    await user.click(await screen.findByRole("button", { name: "명시 참조로 다시 잇기" }));

    await waitFor(() => expect(부름).toHaveBeenCalledWith("relinkExplicit", []));
    expect(await screen.findByText(/명시 참조 연결 1건 · 기존 연결 유지/)).toBeTruthy();
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

  it("열을 더하면 표에 곧바로 선다", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "설정" }));
    const dialog = await screen.findByRole("dialog", { name: "설정" });

    const [계약칸] = within(dialog).getAllByPlaceholderText("새 열 이름");
    await user.type(계약칸, "담당");
    await user.click(within(dialog).getAllByRole("button", { name: "열 추가" })[0]);

    await waitFor(() => expect(within(dialog).getByDisplayValue("담당")).toBeTruthy());

    await user.click(within(dialog).getByRole("button", { name: "닫기" }));
    await user.click(screen.getByRole("tab", { name: /^계약/ }));

    expect(await screen.findByRole("columnheader", { name: /담당/ })).toBeTruthy();
  });

  it("이미 있는 이름은 막고 까닭을 알린다", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "설정" }));
    const dialog = await screen.findByRole("dialog", { name: "설정" });

    await user.type(within(dialog).getAllByPlaceholderText("새 열 이름")[0], "메모");
    await user.click(within(dialog).getAllByRole("button", { name: "열 추가" })[0]);

    expect(await screen.findByText(/이미 있는 열입니다/)).toBeTruthy();
  });

  /** 표가 이미 쓰는 이름이 들어가면 SQLite 가 조용히 뒤엣것의 이름을 바꾼다. */
  it("표가 쓰는 이름도 막는다", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "설정" }));
    const dialog = await screen.findByRole("dialog", { name: "설정" });

    await user.type(within(dialog).getAllByPlaceholderText("새 열 이름")[0], "계약금액");
    await user.click(within(dialog).getAllByRole("button", { name: "열 추가" })[0]);

    expect(await screen.findByText(/표가 이미 쓰는 이름입니다/)).toBeTruthy();
  });

  /** 백 건의 메모가 단추 한 번에 사라지면 되돌릴 길이 없다. */
  it("열을 지우려면 한 번 더 물어본다", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "설정" }));
    const dialog = await screen.findByRole("dialog", { name: "설정" });

    await user.click(within(dialog).getByRole("button", { name: "진행상태 삭제" }));
    expect(within(dialog).getByRole("button", { name: "열 삭제" })).toBeTruthy();

    await user.click(within(dialog).getByRole("button", { name: "취소" }));
    expect(within(dialog).getByDisplayValue("진행상태")).toBeTruthy();

    await user.click(within(dialog).getByRole("button", { name: "진행상태 삭제" }));
    await user.click(within(dialog).getByRole("button", { name: "열 삭제" }));

    await waitFor(() => expect(within(dialog).queryByDisplayValue("진행상태")).toBeNull());
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
    const 건명 = () => within(screen.getAllByRole("row")[1]).getAllByRole("cell")[3];

    await user.click(건명());
    await user.keyboard("{F2}");
    await user.clear(screen.getByRole("textbox"));
    await user.keyboard("고쳐 적은 건명{Enter}");

    await waitFor(() => expect(건명().textContent).toBe("고쳐 적은 건명"));
  });

  /**
   * 쌓는 길만 있고 걷어내는 길이 없으면 잘못 들어온 자료가 영원히 남는다.
   * 되돌릴 수 없는 일이라 <b>세어 보인 뒤에</b> 지운다.
   */
  it("줄을 지우려면 무엇이 사라지는지 먼저 보인다", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "표" }));
    await user.click(await screen.findByRole("button", { name: "선택 자료 삭제…" }));

    const dialog = await screen.findByRole("dialog", { name: "자료 삭제" });
    expect(within(dialog).getByText("함께 삭제되는 자료")).toBeTruthy();
    expect(within(dialog).getByText(/되돌릴 수 없습니다/)).toBeTruthy();

    await user.click(within(dialog).getByRole("button", { name: "삭제" }));

    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: "자료 삭제" })).toBeNull());
    expect(await screen.findByText(/삭제했습니다/)).toBeTruthy();
  });

  it("그만두면 아무것도 지우지 않는다", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "표" }));
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

    await user.click(await screen.findByRole("button", { name: "엑셀로 내보내기" }));

    expect(await screen.findByText(/내보냈습니다.*관리대장_\d{8}\.xlsx/)).toBeTruthy();
  });
});

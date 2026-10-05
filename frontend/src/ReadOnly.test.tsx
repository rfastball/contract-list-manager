import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { invoke, 스위치 } from "./mock";

/**
 * 열람 창(ADR-031 넷째 겹). 다리가 <code>session.readOnly</code> 를 내면 화면은 띠를 띄우고 고치는 자리를
 * 모두 잠근다 — 다리와 SQLite 가 이미 거절하지만, 눌러 본 뒤에 거절을 받게 두지 않는다.
 *
 * <p>작업자료 창은 그대로여야 한다. 잠그는 조건이 새면 내 자료를 고치지 못하는 창이 된다.</p>
 *
 * <p>값은 전부 가짜 다리가 지어낸 것이다.</p>
 */
vi.mock("./mock", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./mock")>();
  return { ...actual, invoke: vi.fn(actual.invoke) };
});

const 부름 = vi.mocked(invoke);
const 본디 = 부름.getMockImplementation()!;

/**
 * 가짜 다리를 열람 창처럼 답하게 한다. 응답을 가로채지 않고 스위치를 켜는 까닭은 가짜 다리에 적었다 —
 * 처음 뜰 때의 <code>session</code> 은 엿듣개를 거치지 않을 수 있다.
 */
const 열람으로 = () => { 스위치.열람 = true; };

beforeAll(() => {
  Element.prototype.scrollIntoView = () => {};
  HTMLDialogElement.prototype.showModal = function () { this.open = true; };
  HTMLDialogElement.prototype.close = function () { this.open = false; };
});

beforeEach(() => { 부름.mockClear(); 부름.mockImplementation(본디); 스위치.열람 = false; });

afterEach(cleanup);

/** 띠가 설 때까지 기다린다 — 그 전에는 작업자료 창으로 그린다. */
const 띠 = () => screen.findByText("열람 중");

/** 단추가 눌리지 않는가. 둘러싼 fieldset 이 막은 것도 친다. */
const 잠김 = (el: Element) => el.matches(":disabled");

describe("열람 창", () => {
  it("띠에 역할과 원본 경로를 낸다", async () => {
    열람으로();
    render(<App />);

    await 띠();
    const banner = screen.getByText("열람 중").closest(".readonly-banner")!;
    expect(banner.textContent).toContain("제출본");
    expect(banner.textContent).toContain("제출_홍길동_20260901.pclm");
    expect(banner.textContent).toContain("읽기 전용");
  });

  it("표의 칸을 고칠 수 없고 삭제 단추가 서지 않는다", async () => {
    열람으로();
    const user = userEvent.setup();
    render(<App />);
    await 띠();

    await user.click(screen.getByRole("tab", { name: /^계약/ }));
    expect(await screen.findByRole("columnheader", { name: /진행상태/ })).toBeTruthy();

    expect(document.querySelector("td.editable, th.editable")).toBeNull();
    expect(screen.queryByRole("button", { name: "선택 자료 삭제…" })).toBeNull();
  });

  it("연결 화면의 잇고 끊는 단추가 잠기고 다시 잇기가 없다", async () => {
    열람으로();
    const user = userEvent.setup();
    render(<App />);
    await 띠();

    await user.click(screen.getByRole("button", { name: /공고 연결/ }));
    await screen.findByRole("listbox", { name: "계약 목록" });
    await screen.findAllByRole("button", { name: "후보 제외" });

    for (const b of [
      ...screen.getAllByRole("button", { name: "연결" }),
      ...screen.getAllByRole("button", { name: "후보 제외" }),
    ]) expect(잠김(b)).toBe(true);
    expect(screen.queryByRole("button", { name: "명시 참조로 다시 잇기" })).toBeNull();
  });

  it("설정의 고치는 자리는 모두 잠기고 열어 보기·폴더 열기·닫기만 남는다", async () => {
    열람으로();
    const user = userEvent.setup();
    render(<App />);
    await 띠();

    await user.click(screen.getByRole("button", { name: "설정" }));
    const dialog = await screen.findByRole("dialog", { name: "설정" });
    await within(dialog).findByText("열람 중인 자료");

    // 나라장터로 가는 길은 고치는 일이 아니라 옮겨 가는 일이다.
    const 남는것 = new Set(["폴더 열기", "다른 자료 열어 보기…", "닫기", "라이선스 정보", "나라장터 열기"]);
    for (const b of within(dialog).getAllByRole("button"))
      expect([b.textContent, 잠김(b)]).toEqual([b.textContent, !남는것.has(b.textContent ?? "")]);

    for (const input of within(dialog).getAllByRole("textbox")) expect(잠김(input)).toBe(true);

    // 수집 도구는 설정에 없다 — 나라장터로 가는 길만 남는다.
    expect(within(dialog).queryByText("고급 · 수집 매핑과 JSON 검토")).toBeNull();
    expect(within(dialog).getByText("브라우저 수집은 왼쪽 메뉴의 나라장터에서 봅니다.")).toBeTruthy();

    // 작업자료를 바꾸는 일은 본 창에서만 한다. 열람 창은 그 길을 알려 줄 뿐이다.
    for (const name of ["작업자료 옮기기…", "작업자료 바꾸기…", "새 계약자료…", "백업 만들기…"])
      expect(잠김(within(dialog).getByRole("button", { name }))).toBe(true);
    expect(within(dialog).getByText("이 자료로 일하려면 본 창의 「작업자료 바꾸기」를 쓰세요.")).toBeTruthy();
  });

  it("나라장터는 수집이 들어오지 않는다고 말하고, 남은 기록만 보이며 수집 도구를 세우지 않는다", async () => {
    열람으로();
    const user = userEvent.setup();
    render(<App />);
    await 띠();

    const source = screen.getByRole("tab", { name: /^나라장터/ });
    expect(source.textContent).toContain("열람 창 · 수집 안 함");
    await user.click(source);

    expect(await screen.findByText("이 창에는 수집한 자료가 저장되지 않습니다.")).toBeTruthy();
    expect(screen.getByText(/작업자료 창에만 저장합니다/)).toBeTruthy();
    expect(await screen.findByRole("heading", { name: "이 파일에 남은 수집 기록" })).toBeTruthy();
    expect(screen.getByText("열람 중이라 기록을 보기만 합니다.")).toBeTruthy();
    // 통로는 그리지 않는다 — 이 창은 수집 대상이 아니다.
    expect(screen.queryByRole("region", { name: "수집 통로" })?.textContent ?? "").not.toMatch(/단축키/);
    expect(잠김(screen.getByRole("button", { name: "JSON 파일로 가져오기…" }))).toBe(true);

    // 수집 도구는 아예 서지 않는다 — 확장 연결도 매핑도 작업자료의 일이다.
    await user.click(screen.getByRole("button", { name: /고급 · 수집 규칙과 지난 수집 다시 보기/ }));
    expect(screen.getByText(/열람 중에는 수집 도구를 쓰지 않습니다/)).toBeTruthy();
    expect(screen.queryByText("고급 · 수집 매핑과 JSON 검토")).toBeNull();

    // 상태 보드의 확장 관리 열기는 작업자료 창의 일이다.
    await user.click(screen.getByRole("button", { name: "Chrome 확장 상태" }));
    expect(screen.getByText("수집한 자료는 작업자료 창에 저장됩니다")).toBeTruthy();
    expect(잠김(screen.getByRole("button", { name: "Chrome 확장 관리 열기" }))).toBe(true);
  });

  it("다른 자료 열어 보기는 열람 창에서도 새 창을 부른다", async () => {
    열람으로();
    const user = userEvent.setup();
    render(<App />);
    await 띠();

    await user.click(screen.getByRole("button", { name: "설정" }));
    const dialog = await screen.findByRole("dialog", { name: "설정" });
    await user.click(await within(dialog).findByRole("button", { name: "다른 자료 열어 보기…" }));

    await waitFor(() => expect(부름).toHaveBeenCalledWith("openOther", []));
    expect(await screen.findByText(/새 창에서 열어 봅니다/)).toBeTruthy();
  });
});

describe("작업자료 창", () => {
  it("띠가 없고 칸·삭제·설정이 그대로 열려 있다", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("tab", { name: /^계약/ }));
    expect(await screen.findByRole("columnheader", { name: /진행상태/ })).toBeTruthy();

    expect(screen.queryByText("열람 중")).toBeNull();
    expect(document.querySelector("th.editable")).not.toBeNull();
    expect(screen.getByRole("button", { name: "선택 자료 삭제…" })).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "설정" }));
    const dialog = await screen.findByRole("dialog", { name: "설정" });
    expect(within(dialog).getByText("저장 위치")).toBeTruthy();
    for (const name of [
      "제출본 만들기", "저장", "다른 자료 열어 보기…",
      "작업자료 옮기기…", "작업자료 바꾸기…", "새 계약자료…", "백업 만들기…",
    ]) expect(잠김(within(dialog).getByRole("button", { name }))).toBe(false);
    expect(within(dialog).queryByText(/본 창의 「작업자료 바꾸기」/)).toBeNull();

    // 수집 도구는 나라장터에 선다. 설정은 그리로 가는 길만 남기고, 가면 설정이 닫힌다.
    await user.click(within(dialog).getByRole("button", { name: "나라장터 열기" }));
    expect(screen.queryByRole("dialog", { name: "설정" })).toBeNull();
    expect(screen.getByRole("tab", { name: /^나라장터/ }).getAttribute("aria-selected")).toBe("true");
    await user.click(await screen.findByRole("button", { name: /고급 · 수집 규칙과 지난 수집 다시 보기/ }));
    expect(await screen.findByText("고급 · 수집 매핑과 JSON 검토")).toBeTruthy();
  });
});

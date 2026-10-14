import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { ColumnManager } from "./ColumnManager";
import { invoke, 복사자리, 사용자뷰초기화, 스위치, 창 } from "./mock";
import type { ColumnTrace, FreezePlan, UserView } from "./types";
import { EditorView } from "@codemirror/view";
import { parseFlow, printFlow } from "./flow/syntax";

/**
 * 설정 안의 <b>제출·취합</b>.
 *
 * <p>부품만 띄우지 않고 <code>App</code> 을 통째로 세우는 까닭: <b>알림(토스트)을 세우는 것은 App</b>
 * 이라, <code>SettingsPanel</code> 만 띄우면 단추를 눌러도 무엇을 알렸는지 볼 데가 없다. 여기서
 * 붙드는 것이 바로 그 알림이다 — 제출본이 어디에 떨어졌는지, 무엇이 겹쳤는지는 알림에
 * 서지 않으면 사람에게 닿지 않는다.</p>
 *
 * <p>값은 전부 가짜 다리가 지어낸 것이다.</p>
 */
vi.mock("./mock", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./mock")>();
  return { ...actual, invoke: vi.fn(actual.invoke) };
});

const 부름 = vi.mocked(invoke);

/** 가짜 다리의 본디 구현. 한 이름만 갈아 끼우고 나머지는 이것으로 흘려보낸다. */
const 본디 = 부름.getMockImplementation()!;

/** 한 이름만 다른 답으로 바꾼다 — 나머지 화면은 그대로 살아 있어야 눌러 볼 수 있다. */
const 가로채기 = (method: string, 대신: () => Promise<unknown>) =>
  부름.mockImplementation((m, args) => (m === method ? 대신() : 본디(m, args)));

beforeAll(() => {
  Element.prototype.scrollIntoView = () => {};
  // jsdom에는 코드 편집기가 커서를 측정할 때 쓰는 Range 레이아웃 API가 없다.
  Range.prototype.getClientRects = () => [] as unknown as DOMRectList;
  Range.prototype.getBoundingClientRect = () => new DOMRect();
  HTMLDialogElement.prototype.showModal = function () { this.open = true; };
  HTMLDialogElement.prototype.close = function () { this.open = false; };
});

beforeEach(() => {
  localStorage.clear();
  부름.mockClear();
  부름.mockImplementation(본디);
  스위치.열람 = false;
  창.몸가짐 = { closeToTray: false, autostart: false, autostartAvailable: true, autostartReason: null, autostartCopyTarget: null };
});

afterEach(cleanup);

/** 설정을 열고 그 창을 돌려준다. */
async function 설정열기() {
  const user = userEvent.setup();
  render(<App />);

  await user.click(await screen.findByRole("button", { name: "설정" }));
  return { user, dialog: await screen.findByRole("dialog", { name: "설정" }) };
}

describe("설정 — 제출·취합", () => {
  it("제출본 만들기 단추가 submit 을 부르고 경로를 알린다", async () => {
    const { user, dialog } = await 설정열기();

    await user.click(within(dialog).getByRole("button", { name: "제출본 만들기" }));

    await waitFor(() => expect(부름).toHaveBeenCalledWith("submit", []));
    expect(await screen.findByText(/제출본을 만들었습니다.+제출_홍길동_/)).toBeTruthy();
  });

  /** 제출본은 저장된 이름으로 뜬다 — 칸에 적고 저장하지 않은 이름은 만들기 전에 먼저 저장한다. */
  it("저장하지 않은 이름으로 제출본을 만들면 그 이름을 먼저 저장하고 만든다", async () => {
    const { user, dialog } = await 설정열기();
    const 이름 = within(dialog).getByDisplayValue("홍길동");
    await user.clear(이름);
    await user.type(이름, "김철수");

    await user.click(within(dialog).getByRole("button", { name: "제출본 만들기" }));

    await waitFor(() => expect(부름).toHaveBeenCalledWith("submit", []));
    const 차례 = 부름.mock.calls.map(([m]) => m);
    expect(부름).toHaveBeenCalledWith("saveSettings", ["김철수"]);
    expect(차례.indexOf("saveSettings")).toBeLessThan(차례.indexOf("submit"));
    await 본디("saveSettings", ["홍길동"]);   // 가짜 다리의 설정은 파일 안에서 이어진다 — 다음 시험을 위해 되돌린다.
  });

  it("이름을 저장하지 못하면 제출본을 만들지 않고, 이미 저장된 이름이면 다시 저장하지 않는다", async () => {
    가로채기("saveSettings", () => Promise.reject(new Error("시험용 저장 실패")));
    const { user, dialog } = await 설정열기();
    const 이름 = within(dialog).getByDisplayValue("홍길동");
    await user.clear(이름);
    await user.type(이름, "김철수");
    await user.click(within(dialog).getByRole("button", { name: "제출본 만들기" }));
    expect(await screen.findByText("시험용 저장 실패")).toBeTruthy();
    expect(부름).not.toHaveBeenCalledWith("submit", []);

    부름.mockImplementation(본디);
    await user.clear(이름);
    await user.type(이름, "홍길동");
    부름.mockClear();
    await user.click(within(dialog).getByRole("button", { name: "제출본 만들기" }));
    await waitFor(() => expect(부름).toHaveBeenCalledWith("submit", []));
    expect(부름.mock.calls.some(([m]) => m === "saveSettings")).toBe(false);
  });

  it("「저장」 은 설정 창을 닫지 않는다", async () => {
    const { user, dialog } = await 설정열기();
    await user.click(within(dialog).getByRole("button", { name: "저장" }));
    expect(await screen.findByText("설정을 저장했습니다.")).toBeTruthy();
    expect(screen.getByRole("dialog", { name: "설정" })).toBe(dialog);
    expect((dialog as HTMLDialogElement).open).toBe(true);
  });

  /** 그만둔 것은 실패가 아니다. 아무 말도 하지 않는 것이 맞다. */
  it("제출을 그만두면 아무 말도 하지 않는다", async () => {
    가로채기("submit", () => Promise.resolve(null));

    const { user, dialog } = await 설정열기();
    await user.click(within(dialog).getByRole("button", { name: "제출본 만들기" }));

    await waitFor(() => expect(부름).toHaveBeenCalledWith("submit", []));
    expect(screen.queryByText(/제출본을 만들었습니다/)).toBeNull();
  });

  it("취합 단추가 merge 를 부르고 요약을 알린다", async () => {
    const { user, dialog } = await 설정열기();

    await user.click(within(dialog).getByRole("button", { name: "제출본 취합하기…" }));

    await waitFor(() => expect(부름).toHaveBeenCalledWith("merge", []));

    // 셈과 두 자리가 함께 서야 한다 — 겹친 것이 무엇이었는지는 그 글(txt)에 있다.
    const 알림 = await screen.findByText(/제출본 3개/);
    expect(알림.textContent).toMatch(/충돌 2건/);
    expect(알림.textContent).toMatch(/취합_\d{8}\.pclm/);
    expect(알림.textContent).toMatch(/취합_\d{8}\.txt/);
  });

  /** 빈 폴더를 고른 사람에게 "다 되었다" 고 말하지 않는다 — 까닭을 그대로 보인다. */
  it("취합이 실패하면 까닭을 그대로 보인다", async () => {
    가로채기("merge", () => Promise.reject(
      new Error("합칠 제출본을 찾지 못했습니다 — 이 폴더에 제출본(.pclm)이 없습니다.")));

    const { user, dialog } = await 설정열기();
    await user.click(within(dialog).getByRole("button", { name: "제출본 취합하기…" }));

    expect(await screen.findByText(/합칠 제출본을 찾지 못했습니다/)).toBeTruthy();
  });
});

/**
 * 가공열(§6.4). 보표는 단계 × 표본이고, 칸은 그 단계까지의 값이다. 붙드는 위험:
 * v1 으로 저장된 열이 열리지 않는 것, 늦게 온 옛 계산이 새 값을 덮는 것, 꼴이 깨진 식이 저장되는 것,
 * 저장에 실패한 초안이 사라지는 것, 저장하지 않은 변경이 이동에 묻혀 사라지는 것.
 */
it("가공열은 단계마다 표본 값을 보이고, 꼴이 깨진 식과 실패한 저장에서 초안을 지킨다", async () => {
  let hold = false;
  let release: (() => void) | undefined;
  부름.mockImplementation(async (method, args) => {
    const result = await 본디(method, args);
    if (method === "traceComputedColumn" && hold) {
      hold = false;
      const stale = result as ColumnTrace;
      return new Promise(resolve => { release = () => resolve({ ...stale, final: stale.final.map(() => "늦게 도착한 이전 결과") }); });
    }
    return result;
  });
  const { user, dialog } = await 설정열기();
  await user.click(within(dialog).getByRole("button", { name: "닫기" }));
  await user.click(screen.getByRole("button", { name: "구조" }));
    await user.click(screen.getByRole("button", { name: "열 관리" }));
  expect(screen.queryByRole("dialog", { name: "설정" })).toBeNull();
  const ui = within(await screen.findByRole("region", { name: "열 관리" }));
  await user.click(within(ui.getByRole("group", { name: "자료 종류" })).getByRole("button", { name: "공고" }));

  // v1 으로 저장된 열은 v2 로 옮겨 연다 — pick 은 고유의 기준 필드로 흡수되고, 겹친 책상 한 줄이 빠진 것이 보인다.
  await user.click(await ui.findByRole("button", { name: /^품명 모음 · 식 · …잇기/ }));
  let grid = within(await ui.findByRole("grid", { name: "값 흐름" }));
  expect(grid.getByRole("rowheader", { name: /^고유\s*세부품명/ })).toBeTruthy();
  expect(await grid.findByText("1행 겹쳐 빠짐")).toBeTruthy();
  expect(grid.getAllByText("책상, 의자").length).toBeGreaterThan(0);

  // 새 = 열 — 이름 뒤에 = 를 치면 빠른 시작이 선다.
  hold = true;
  await user.type(ui.getByRole("textbox", { name: "새 열 이름" }), "품명번호요약=");
  expect(ui.getByRole("textbox", { name: "열 이름" })).toHaveProperty("value", "품명번호요약");
  await user.click(within(ui.getByRole("region", { name: "빠른 시작" })).getByRole("button", { name: "이름(번호) 나열" }));
  await waitFor(() => expect(release).toBeDefined());
  grid = within(ui.getByRole("grid", { name: "값 흐름" }));
  await user.click(grid.getByRole("rowheader", { name: /^잇기/ }));
  const separator = within(ui.getByRole("region", { name: "단계 설정" })).getByRole("textbox", { name: "구분자" });
  await user.clear(separator);
  await user.type(separator, " / ");
  expect((await grid.findAllByText("책상(001) / 의자(002)")).length).toBeGreaterThan(0);
  await act(async () => { release!(); });
  expect(grid.queryByText("늦게 도착한 이전 결과")).toBeNull();
  // 번호가 빈 표본 B 는 서식 단계에서 빈 값 수로 드러난다.
  expect(grid.getAllByText("빈 값 1").length).toBeGreaterThan(0);
  const request = 부름.mock.calls.filter(([method]) => method === "traceComputedColumn").slice(-1)[0];
  expect(request[1][0]).toBe("notice");
  const sent = JSON.parse(request[1][1] as string) as { version: number; flow: { steps: { op: string; text?: string }[] } };
  expect(sent.version).toBe(2);
  expect(sent.flow.steps.map(s => s.op)).toEqual(["distinct", "format", "join"]);
  expect(sent.flow.steps[1].text).toBe("{세부품명}({세부품명번호})");

  // 잇기를 지우면 끝이 값목록이라 오류가 그 단계에 붙고, 저장은 막힌다.
  grid.getByRole("rowheader", { name: /^잇기/ }).closest("tr")!.focus();
  await user.keyboard("{Delete}");
  expect(grid.queryByRole("rowheader", { name: /^잇기/ })).toBeNull();
  expect(await grid.findByText(/마지막 결과가 값목록입니다/)).toBeTruthy();
  expect(ui.getByText("오류 1개")).toBeTruthy();
  await user.click(ui.getByRole("button", { name: "저장" }));
  expect((await ui.findByRole("alert")).textContent).toContain("저장할 수 없습니다");
  expect(부름.mock.calls.some(([method]) => method === "saveComputedColumn")).toBe(false);

  // + 단계 는 지금 꼴(값목록)에 쓸 수 있는 동사만 보이고, 쳐서 찾는다.
  await user.click(ui.getByRole("button", { name: "단계 추가" }));
  const picker = within(ui.getByRole("dialog", { name: "단계 추가" }));
  expect(picker.queryByRole("option", { name: /^거르기/ })).toBeNull();
  await user.type(picker.getByRole("combobox", { name: "동사 찾기" }), "잇기{Enter}");
  expect(grid.getByRole("rowheader", { name: /^잇기/ })).toBeTruthy();
  expect((await grid.findAllByText("책상(001), 의자(002)")).length).toBeGreaterThan(0);

  가로채기("saveComputedColumn", () => Promise.reject(new Error("이미 있는 열입니다: 품명번호요약")));
  await user.click(ui.getByRole("button", { name: "저장" }));
  expect(await ui.findByRole("alert")).toHaveProperty("textContent", "이미 있는 열입니다: 품명번호요약");
  expect(ui.getByRole("textbox", { name: "열 이름" })).toHaveProperty("value", "품명번호요약");
  expect(grid.getByRole("rowheader", { name: /^잇기/ })).toBeTruthy();
  부름.mockImplementation(본디);
  await user.click(ui.getByRole("button", { name: "저장" }));
  expect(await ui.findByText(/열을 저장했습니다\.$/)).toBeTruthy();
  const saved = 부름.mock.calls.filter(([method]) => method === "saveComputedColumn").slice(-1)[0];
  expect(saved[1].slice(0, 2)).toEqual(["notice", "품명번호요약"]);
  expect(JSON.parse(saved[1][2] as string)).toMatchObject({ version: 2, source: "flow" });
  expect(await ui.findByRole("button", { name: /^품명번호요약 · 식 · …잇기/ })).toBeTruthy();
  expect(ui.getByText("저장됨")).toBeTruthy();

  // 저장하지 않은 변경은 다른 열로 옮길 때 묻는다.
  await user.click(ui.getByRole("button", { name: "단계 추가" }));
  await user.type(within(ui.getByRole("dialog", { name: "단계 추가" })).getByRole("combobox", { name: "동사 찾기" }), "다듬{Enter}");
  await user.click(ui.getByRole("button", { name: /^메모 · 입력/ }));
  let guard = await screen.findByRole("dialog", { name: "저장하지 않은 열 변경" });
  await user.click(within(guard).getByRole("button", { name: "계속 편집" }));
  expect(grid.getByRole("rowheader", { name: /^다듬기/ })).toBeTruthy();
  await user.click(ui.getByRole("button", { name: /^메모 · 입력/ }));
  guard = await screen.findByRole("dialog", { name: "저장하지 않은 열 변경" });
  await user.click(within(guard).getByRole("button", { name: "변경 버리기" }));
  expect(await ui.findByRole("textbox", { name: "열 이름" })).toHaveProperty("value", "메모");

  await user.click(ui.getByRole("button", { name: /^품명번호요약 · 식/ }));
  await user.click(ui.getByRole("button", { name: "열 메뉴" }));
  await user.click(ui.getByRole("menuitem", { name: "열 제거…" }));
  const confirm = await screen.findByRole("dialog", { name: "열 제거 확인" });
  expect(confirm.textContent).toContain("같은 이름으로 다시 만들어도 식별자가 달라");
  await user.click(within(confirm).getByRole("button", { name: "열 제거" }));
  await waitFor(() => expect(ui.queryByRole("button", { name: /^품명번호요약/ })).toBeNull());
});

/** SQL 은 일방통행 탈출구다(§5) — 꺼낸 뒤에는 단계로 돌아가지 못하고, 저장은 SQL 원천으로 간다. */
it("SQL 탭은 컴파일된 SQL 을 읽기 전용으로 보이고, 확인을 거쳐서만 꺼낸다", async () => {
  const { user, dialog } = await 설정열기();
  await user.click(within(dialog).getByRole("button", { name: "닫기" }));
  await user.click(screen.getByRole("button", { name: "구조" }));
    await user.click(screen.getByRole("button", { name: "열 관리" }));
  const ui = within(await screen.findByRole("region", { name: "열 관리" }));
  await user.type(ui.getByRole("textbox", { name: "새 열 이름" }), "품목 셈=");
  await user.click(within(ui.getByRole("region", { name: "빠른 시작" })).getByRole("button", { name: "품목 수" }));
  await user.click(ui.getByRole("tab", { name: "SQL" }));
  const compiled = await ui.findByRole("textbox", { name: "컴파일된 SQL" });
  await waitFor(() => expect(compiled.textContent).toContain("FROM v_계약품목 WHERE 계약번호 = 현재.계약번호"));
  await user.click(ui.getByRole("button", { name: "SQL로 꺼내 직접 편집…" }));
  const escape = await screen.findByRole("dialog", { name: "SQL로 꺼내기 확인" });
  expect(escape.textContent).toContain("되돌릴 수 없습니다");
  await user.click(within(escape).getByRole("button", { name: "SQL로 꺼내기" }));
  expect(ui.getByRole("tab", { name: "단계" })).toHaveProperty("disabled", true);
  expect(await ui.findByRole("textbox", { name: "SQL" })).toBeTruthy();
  // 꺼낸 SQL 은 다리가 컴파일한 그 글이고, 편집면은 고칠 때마다 다리의 검증을 부른다.
  const compiledCall = 부름.mock.calls.filter(([method]) => method === "compileComputedColumn").slice(-1)[0];
  expect(compiledCall[1][0]).toBe("contract");
  expect(await ui.findByText(/검사를 통과했습니다/)).toBeTruthy();
  const checked = 부름.mock.calls.filter(([method]) => method === "validateComputedSql").slice(-1)[0];
  expect(checked[1][1]).toContain("FROM v_계약품목");
  // 실제 편집기의 키맵을 거쳐 여러 줄 들여쓰기와 키보드 탈출을 확인한다.
  const editor = ui.getByRole("textbox", { name: "SQL" });
  const lines = () => [...editor.querySelectorAll(".cm-line")].map(line => line.textContent ?? "");
  const original = lines();
  act(() => editor.focus());
  fireEvent.keyDown(editor, { key: "a", ctrlKey: true });
  fireEvent.keyDown(editor, { key: "Tab" });
  expect(lines()).toEqual(original.map(line => `  ${line}`));
  expect(document.activeElement).toBe(editor);
  fireEvent.keyDown(editor, { key: "Tab", shiftKey: true });
  expect(lines()).toEqual(original);
  fireEvent.keyDown(editor, { key: "End", ctrlKey: true });
  fireEvent.keyDown(editor, { key: "Tab" });
  fireEvent.keyDown(editor, { key: "Enter" });
  expect(lines().at(-1)).toBe(/^\s*/.exec(lines().at(-2)!)![0]);
  fireEvent.keyDown(editor, { key: "m", ctrlKey: true });
  const indented = lines();
  // Tab을 취소하지 않아 브라우저의 기본 초점 이동에 맡긴다.
  expect(fireEvent.keyDown(editor, { key: "Tab", keyCode: 9 })).toBe(true);
  expect(lines()).toEqual(indented);
  await ui.findByText(/검사를 통과했습니다/);
  await user.click(ui.getByRole("button", { name: "저장" }));
  await waitFor(() => expect(부름.mock.calls.some(([method, args]) => method === "saveComputedColumn"
    && JSON.parse(args[2] as string).source === "sql")).toBe(true));
});

it("굳히기 전에 칸을 세지 못했으면 까닭과 「다시 확인」 을 보이고, 다시 세면 굳힐 수 있다", async () => {
  가로채기("planFreezeColumn", () => Promise.reject(new Error("칸을 세지 못했습니다.")));
  const { user, dialog } = await 설정열기();
  await user.click(within(dialog).getByRole("button", { name: "닫기" }));
  await user.click(screen.getByRole("button", { name: "구조" }));
  await user.click(screen.getByRole("button", { name: "열 관리" }));
  const ui = within(await screen.findByRole("region", { name: "열 관리" }));
  await user.click(within(ui.getByRole("group", { name: "자료 종류" })).getByRole("button", { name: "공고" }));
  await user.click(await ui.findByRole("button", { name: /^품명 모음 · 식/ }));
  await user.click(ui.getByRole("button", { name: "가공열. 입력열로 바꾸기" }));
  const confirm = within(await screen.findByRole("dialog", { name: "열 성질 바꾸기 확인" }));
  expect((await confirm.findByRole("alert")).textContent).toBe("칸을 세지 못했습니다.");
  expect(confirm.getByRole("button", { name: "값으로 굳히기" })).toHaveProperty("disabled", true);
  부름.mockImplementation(본디);
  await user.click(confirm.getByRole("button", { name: "다시 확인" }));
  expect(await confirm.findByText(/입력값을 쓸 문서/)).toBeTruthy();
  expect(confirm.queryByRole("alert")).toBeNull();
  expect(confirm.queryByRole("button", { name: "다시 확인" })).toBeNull();
  expect(confirm.getByRole("button", { name: "값으로 굳히기" })).toHaveProperty("disabled", false);
});

/** 성질 바꾸기(§3.1) — 칸 수는 다리가 세어 확인 창에 보이고, 굳히기는 그 수를 함께 보낸다. */
it("가공열을 값으로 굳히면 다리가 센 칸을 보이고 그 수로 굳힌다", async () => {
  const counted = await 본디("planFreezeColumn", ["notice", "품명 모음"]) as FreezePlan;
  가로채기("planFreezeColumn", () => Promise.resolve({ ...counted, hidden: 2 }));
  const { user, dialog } = await 설정열기();
  await user.click(within(dialog).getByRole("button", { name: "닫기" }));
  await user.click(screen.getByRole("button", { name: "구조" }));
    await user.click(screen.getByRole("button", { name: "열 관리" }));
  const ui = within(await screen.findByRole("region", { name: "열 관리" }));
  await user.click(within(ui.getByRole("group", { name: "자료 종류" })).getByRole("button", { name: "공고" }));
  await user.click(await ui.findByRole("button", { name: /^품명 모음 · 식/ }));
  await user.click(ui.getByRole("button", { name: "가공열. 입력열로 바꾸기" }));
  const confirm = await screen.findByRole("dialog", { name: "열 성질 바꾸기 확인" });
  expect(await within(confirm).findByText(/입력값을 쓸 문서/)).toBeTruthy();
  expect(counted.cells).toBeGreaterThan(0);
  expect(confirm.textContent).toContain(`중 ${counted.cells}건`);
  expect(confirm.textContent).toContain("숨은 입력값: 2칸");
  expect(confirm.textContent).not.toContain("시안");
  부름.mockImplementation(본디);
  await user.click(within(confirm).getByRole("button", { name: "값으로 굳히기" }));
  const frozen = 부름.mock.calls.filter(([method]) => method === "freezeColumn").slice(-1)[0];
  expect(frozen[1]).toEqual(["notice", "품명 모음", String(counted.cells)]);
  expect(await ui.findByText(`‘품명 모음’ 열을 값으로 굳혔습니다(${counted.cells}칸).`)).toBeTruthy();
  expect(await ui.findByRole("button", { name: /^품명 모음 · 입력/ })).toBeTruthy();
  expect(ui.getByRole("button", { name: "입력열. 가공열로 바꾸기" })).toBeTruthy();

  // 다시 = 로 켜면 비활성으로 남은 식이 돌아오고, 숨길 입력값 수를 먼저 보인다.
  가로채기("planFreezeColumn", () => Promise.resolve({ kind: "text", cells: 0, hidden: 4, series: 0 }));
  await user.click(ui.getByRole("button", { name: "입력열. 가공열로 바꾸기" }));
  const back = await screen.findByRole("dialog", { name: "열 성질 바꾸기 확인" });
  expect(await within(back).findByText(/4칸/)).toBeTruthy();
  await user.click(within(back).getByRole("button", { name: "가공열로 바꾸기" }));
  expect(within(await ui.findByRole("grid", { name: "값 흐름" })).getByRole("rowheader", { name: /^잇기/ })).toBeTruthy();
});

/** 깨진 정의는 목록과 정의줄에서 까닭과 함께 드러난다 — 뷰에서는 빈 값으로 나가고 있다. */
it("깨진 가공열은 목록 꼬리와 정의줄에 까닭을 보인다", async () => {
  const props = {
    initialEntity: "notice" as const, initialField: "품목수", onClose: vi.fn(),
    onAddColumn: vi.fn(), onUpdateColumn: vi.fn(), onRemoveColumn: vi.fn(), onMoveColumn: vi.fn(), onSaveComputed: vi.fn(), onFreezeColumn: vi.fn(),
  };
  가로채기("validateComputedSql", () => Promise.resolve({ ok: false, error: "no such table: v_없는뷰", samples: [], final: [] }));
  render(<ColumnManager {...props} columns={[{ entityType: "notice", fieldName: "품목수", kind: "computed", choices: [],
    formula: { version: 2, id: "x", source: "sql", sql: "SELECT count(*) FROM v_없는뷰" }, broken: "no such table: v_없는뷰" }]} />);
  const ui = within(await screen.findByRole("region", { name: "열 관리" }));
  expect(ui.getByRole("button", { name: /^품목수 · 식 · 깨진 식/ })).toBeTruthy();
  expect((await ui.findByRole("alert")).textContent).toContain("no such table: v_없는뷰");
  expect(ui.getAllByText("깨진 식").length).toBeGreaterThan(1);
  // SQL 편집면은 다리의 검증 결과를 그대로 보인다.
  expect(await ui.findByText("저장할 수 없습니다: no such table: v_없는뷰")).toBeTruthy();
});

it("입력열은 저장할 때만 바꾸며 실패한 초안과 이동 취소를 보존한다", async () => {
  const update = vi.fn(async () => { throw new Error("열을 저장하지 못했습니다."); });
  const props = {
    initialEntity: "contract" as const, initialField: "메모", onClose: vi.fn(),
    onAddColumn: vi.fn(), onUpdateColumn: update, onRemoveColumn: vi.fn(), onMoveColumn: vi.fn(), onSaveComputed: vi.fn(), onFreezeColumn: vi.fn(),
  };
  const user = userEvent.setup();
  const view = render(<ColumnManager {...props} columns={[]} />);
  const ui = within(await screen.findByRole("region", { name: "열 관리" }));
  expect(ui.queryByLabelText("열 이름")).toBeNull();
  view.rerender(<ColumnManager {...props} columns={[{ entityType: "contract", fieldName: "메모", kind: "text", choices: [] }]} />);
  expect((await ui.findByLabelText("열 이름") as HTMLInputElement).value).toBe("메모");
  await user.clear(ui.getByLabelText("열 이름"));
  await user.type(ui.getByLabelText("열 이름"), "메모변경");
  await user.tab();
  expect(update).not.toHaveBeenCalled();
  await user.click(ui.getByRole("button", { name: "저장" }));
  expect(await ui.findByRole("alert")).toHaveProperty("textContent", "열을 저장하지 못했습니다.");
  expect((ui.getByLabelText("열 이름") as HTMLInputElement).value).toBe("메모변경");
  await user.click(ui.getByRole("button", { name: "목록으로 돌아가기" }));
  let guard = await screen.findByRole("dialog", { name: "저장하지 않은 열 변경" });
  await user.click(within(guard).getByRole("button", { name: "계속 편집" }));
  expect((ui.getByLabelText("열 이름") as HTMLInputElement).value).toBe("메모변경");
  await user.click(ui.getByRole("button", { name: "목록으로 돌아가기" }));
  guard = await screen.findByRole("dialog", { name: "저장하지 않은 열 변경" });
  await user.click(within(guard).getByRole("button", { name: "변경 버리기" }));
  expect(props.onClose).toHaveBeenCalledOnce();
});


/** 흐름식 편집기의 글을 바꾼다 — 사람이 친 것처럼 편집기의 변경 알림을 거친다. */
function 흐름식쓰기(editor: HTMLElement, text: string) {
  const view = EditorView.findFromDOM(editor.closest(".cm-editor") as HTMLElement)!;
  act(() => { view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text } }); });
}
const 흐름식글 = (editor: HTMLElement) => EditorView.findFromDOM(editor.closest(".cm-editor") as HTMLElement)!.state.doc.toString();

/**
 * 흐름식 편집기는 연 열의 식을 보인다. 열을 바꿔도 앞 열의 글이 남으면 한 글자 치는 순간 그 글로 새 열을 덮는다.
 * 문구를 비우거나 읽을 수 없게 둔 채 저장하면 단계에 남은 옛 식이 조용히 저장된다 — 그동안 저장을 막는다.
 */
it("흐름식 편집기는 다른 열로 옮기면 그 열의 식을 보이고, 읽을 수 없거나 빈 문구로는 저장하지 않는다", async () => {
  const 품명 = parseFlow('품목 |> 꺼내기 세부품명 |> 잇기 ", "').flow;
  const 번호 = parseFlow('품목 |> 꺼내기 세부품명번호 |> 잇기 " / "').flow;
  const props = {
    initialEntity: "notice" as const, initialField: "품명 나열", onClose: vi.fn(),
    onAddColumn: vi.fn(), onUpdateColumn: vi.fn(), onRemoveColumn: vi.fn(), onMoveColumn: vi.fn(), onSaveComputed: vi.fn(async () => {}), onFreezeColumn: vi.fn(),
  };
  const user = userEvent.setup();
  render(<ColumnManager {...props} columns={[
    { entityType: "notice", fieldName: "품명 나열", kind: "computed", choices: [], formula: { version: 2, id: "a", source: "flow", flow: 품명 } },
    { entityType: "notice", fieldName: "번호 나열", kind: "computed", choices: [], formula: { version: 2, id: "b", source: "flow", flow: 번호 } },
  ]} />);
  const ui = within(await screen.findByRole("region", { name: "열 관리" }));
  await user.click(await ui.findByRole("tab", { name: "흐름식" }));
  expect(흐름식글(await ui.findByRole("textbox", { name: "흐름식" }))).toBe(printFlow(품명));

  await user.click(ui.getByRole("button", { name: /^번호 나열 · 식/ }));
  await user.click(await ui.findByRole("tab", { name: "흐름식" }));
  const editor = await ui.findByRole("textbox", { name: "흐름식" });
  expect(흐름식글(editor)).toBe(printFlow(번호));

  // 읽히는 글로 고쳐 초안을 바꾼 뒤 문구를 비운다 — 단계에는 고친 식이 남지만 저장은 하지 않는다.
  흐름식쓰기(editor, '품목 |> 꺼내기 세부품명번호 |> 잇기 ", "');
  흐름식쓰기(editor, "");
  expect(await ui.findByText("식이 비어 있습니다. 단계에는 마지막으로 읽은 식이 남아 있습니다.")).toBeTruthy();
  await user.click(ui.getByRole("button", { name: "저장" }));
  expect((await ui.findByRole("alert")).textContent).toBe("흐름식을 읽을 수 없어 저장하지 않았습니다 — 식이 비어 있습니다");

  흐름식쓰기(editor, "품목 |> 없는동사");
  expect(await ui.findByText(/^이 식을 읽을 수 없습니다: .+단계에는 마지막으로 읽은 식이 남아 있습니다\.$/)).toBeTruthy();
  await user.click(ui.getByRole("button", { name: "저장" }));
  expect((await ui.findByRole("alert")).textContent).toMatch(/^흐름식을 읽을 수 없어 저장하지 않았습니다 — /);
  expect(props.onSaveComputed).not.toHaveBeenCalled();

  // 다시 읽히면 저장한다 — 그 식 그대로.
  흐름식쓰기(editor, '품목 |> 꺼내기 세부품명번호 |> 잇기 ", "');
  await user.click(ui.getByRole("button", { name: "저장" }));
  await waitFor(() => expect(props.onSaveComputed).toHaveBeenCalledOnce());
  expect(props.onSaveComputed.mock.calls[0]).toMatchObject(["notice", "번호 나열", { source: "flow", flow: parseFlow('품목 |> 꺼내기 세부품명번호 |> 잇기 ", "').flow }, "번호 나열"]);
});


/**
 * 브라우저 수집(확장 준비·상태·고급 JSON 검토)은 나라장터 화면으로 옮겼다 — 그 약속은 Nara.test 가 붙든다.
 * 설정에는 자료 관리와 화면 표시, 보기 구성만 남긴다.
 */
it("설정에는 중복 바로가기가 없고 수집은 사이드바에서 연다", async () => {
  const { user, dialog } = await 설정열기();
  expect(within(dialog).queryByRole("button", { name: "계획 엑셀" })).toBeNull();
  expect(within(dialog).queryByRole("heading", { name: "계획 엑셀" })).toBeNull();
  expect(within(dialog).queryByRole("heading", { name: "브라우저 수집" })).toBeNull();
  expect(within(dialog).queryByRole("heading", { name: "열 관리" })).toBeNull();
  expect(within(dialog).queryByText("Edge · Chrome 확장 추가")).toBeNull();
  expect(within(dialog).queryByText("고급 · 수집 매핑과 JSON 검토")).toBeNull();
  expect(within(dialog).queryByRole("button", { name: "확장 준비" })).toBeNull();

  await user.click(within(dialog).getByRole("button", { name: "닫기" }));
  await user.click(screen.getByRole("tab", { name: /^나라장터/ }));
  expect(screen.queryByRole("dialog", { name: "설정" })).toBeNull();
  expect(screen.getByRole("tab", { name: /^나라장터/ }).getAttribute("aria-selected")).toBe("true");
  expect(await screen.findByRole("heading", { name: "나라장터", level: 1 })).toBeTruthy();
});

it("표시 설정은 즉시 적용하고 다시 열어도 유지하며 시스템 테마로 돌아온다", async () => {
  const { user, dialog } = await 설정열기();
  await user.selectOptions(within(dialog).getByLabelText("글자 크기"), "larger");
  await user.selectOptions(within(dialog).getByLabelText("화면 테마"), "dark");
  expect(document.documentElement.dataset.fontScale).toBe("larger");
  expect(document.documentElement.dataset.theme).toBe("dark");
  cleanup();
  render(<App />);
  await user.click(await screen.findByRole("button", { name: "설정" }));
  const reopened = within(await screen.findByRole("dialog", { name: "설정" }));
  expect((reopened.getByLabelText("글자 크기") as HTMLSelectElement).value).toBe("larger");
  expect((reopened.getByLabelText("화면 테마") as HTMLSelectElement).value).toBe("dark");
  await user.selectOptions(reopened.getByLabelText("화면 테마"), "system");
  expect(document.documentElement.dataset.theme).toBeUndefined();
});

it("보기 구성을 가져오면 사이드바와 열 정의를 다시 읽는다", async () => {
  const current = await 본디("userViews", []) as UserView[];
  const added = { ...current[0], id: "imported-dashboard-test", name: "가져온 보고서" };
  let imported = false;
  부름.mockImplementation((method, args) => {
    if (method === "pickViewPackage") return Promise.resolve({ name: "보기.json", json: "{}", preview: {
      views: [{ name: added.name, importName: added.name }], columns: [], errors: [], includesAllColumnsForSql: false, token: "checked",
    } });
    if (method === "importViewPackage") {
      imported = true;
      return 본디("addColumn", ["contract", "가져온 확인열", "text", ""]).then(() => [...current, added]);
    }
    if (method === "userViews" && imported) return Promise.resolve([...current, added]);
    return 본디(method, args);
  });
  try {
    const { user, dialog } = await 설정열기();
    await waitFor(() => expect((within(dialog).getByRole("button", { name: "구성 파일 가져오기…" }) as HTMLButtonElement).closest("fieldset")!.disabled).toBe(false));
    await user.click(within(dialog).getByRole("button", { name: "구성 파일 가져오기…" }));
    expect(imported).toBe(false);
    await user.click(await within(dialog).findByRole("button", { name: "확인한 구성 추가하기" }));
    expect(await within(dialog).findByText(/1개 보기를 추가했습니다/)).toBeTruthy();
    await user.click(within(dialog).getByRole("button", { name: "닫기" }));
    expect(await within(screen.getByRole("complementary", { name: "작업 탐색" })).findByRole("button", { name: added.name })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "구조" }));
    await user.click(screen.getByRole("button", { name: "열 관리" }));
    expect(await within(screen.getByRole("region", { name: "열 관리" })).findByRole("button", { name: /^가져온 확인열/ })).toBeTruthy();
  } finally { await 본디("removeColumn", ["contract", "가져온 확인열"]); }
});

it("시작 때 설정을 읽지 못했으면 「설정」 이 다시 읽고, 또 못 읽으면 까닭을 알린다 — 말없이 지나가지 않는다", async () => {
  // 시작 때 한 번, 「설정」 을 누른 첫 번에 한 번 거절한다.
  스위치.설정오류 = 2;
  try {
    const user = userEvent.setup();
    render(<App />);
    // 시작 때의 실패. 알림이 걷히기를 기다리지 않고 누른다 — 누른 뒤의 알림이 그 까닭을 말한다.
    await waitFor(() => expect(screen.getByRole("alert").textContent).toBe("설정 파일이 잠겨 있습니다."));
    await user.click(await screen.findByRole("button", { name: "설정" }));
    await waitFor(() => expect(screen.getByRole("alert").textContent).toBe("설정을 읽지 못했습니다. 설정 파일이 잠겨 있습니다."));
    expect(screen.queryByRole("dialog", { name: "설정" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "설정" }));
    expect(await screen.findByRole("dialog", { name: "설정" })).toBeTruthy();
  } finally { 스위치.설정오류 = 0; }
});

it("다른 보기 구성 파일을 고르다 실패하면 앞 파일의 미리보기와 「확인한 구성 추가하기」 를 거둔다", async () => {
  let 실패 = false;
  부름.mockImplementation((method, args) => {
    if (method === "pickViewPackage") return 실패 ? Promise.reject(new Error("보기 구성 파일은 2MB까지 가져올 수 있습니다."))
      : Promise.resolve({ name: "앞.json", json: "{}", preview: {
        views: [{ name: "앞 보기", importName: "앞 보기" }], columns: [], errors: [], includesAllColumnsForSql: false, token: "checked",
      } });
    return 본디(method, args);
  });
  const { user, dialog } = await 설정열기();
  const 고르기 = within(dialog).getByRole("button", { name: "구성 파일 가져오기…" }) as HTMLButtonElement;
  await waitFor(() => expect(고르기.closest("fieldset")!.disabled).toBe(false));
  await user.click(고르기);
  expect(await within(dialog).findByText("앞.json 가져오기 미리보기")).toBeTruthy();
  실패 = true;
  await user.click(고르기);
  expect((await within(dialog).findByRole("alert")).textContent).toBe("보기 구성 파일은 2MB까지 가져올 수 있습니다.");
  expect(within(dialog).queryByText("앞.json 가져오기 미리보기")).toBeNull();
  expect(within(dialog).queryByRole("button", { name: "확인한 구성 추가하기" })).toBeNull();
});

/**
 * 서식용 예시 보기는 처음부터 들어 있다. 「예시 보기 다시 추가…」 는 파일을 고르지 않고 exe 에 실린 구성을 같은 미리보기 → 추가
 * 흐름으로 들인다 — 지웠으면 그 이름으로, 남아 있으면 복사본으로 선다.
 */
it("예시 보기 다시 추가는 미리보기를 거쳐 들이고 이름이 겹치면 복사본으로 보인다", async () => {
  try {
    const { user, dialog } = await 설정열기();
    expect(await within(dialog).findByRole("checkbox", { name: /서식용 예시/ })).toBeTruthy();
    // 설명은 짧게 — 무엇을 하는지와 무엇을 담지 않는지만, 예시는 무엇이고 다시 추가하면 어떻게 되는지만.
    expect(within(dialog).getByText("보기 구성을 JSON 파일로 내보내거나 가져옵니다. 조달 자료와 입력한 값은 담지 않습니다.")).toBeTruthy();
    expect(within(dialog).getByText("「서식용 예시」는 입찰공고·구매추진안내·계약체결안내 서식을 채우는 기본 보기입니다. 다시 추가하면 처음 구성으로 하나 더 생깁니다.")).toBeTruthy();
    const 단추 = await within(dialog).findByRole("button", { name: "예시 보기 다시 추가…" });
    await waitFor(() => expect((단추 as HTMLButtonElement).closest("fieldset")!.disabled).toBe(false));
    await user.click(단추);
    expect(await within(dialog).findByText("예시 보기 가져오기 미리보기")).toBeTruthy();
    expect(부름.mock.calls.some(([m]) => m === "importViewPackage")).toBe(false);
    expect(within(dialog).getByText("서식용 예시 (복사본)", { selector: "li strong" })).toBeTruthy();
    await user.click(within(dialog).getByRole("button", { name: "확인한 구성 추가하기" }));
    expect(await within(dialog).findByText(/1개 보기를 추가했습니다/)).toBeTruthy();
    expect(within(dialog).getByRole("checkbox", { name: /서식용 예시 \(복사본\)/ })).toBeTruthy();
    // 사이드바가 다시 읽힐 때까지 기다린다 — 추가 뒤 다시 읽기가 시험 밖으로 새지 않게.
    expect(await within(screen.getByRole("complementary", { name: "작업 탐색" })).findByRole("button", { name: "서식용 예시 (복사본)" })).toBeTruthy();
  } finally { 사용자뷰초기화(); }
});

/** 고지는 100KB 쯤이라 설정을 열 때마다 끌어오지 않는다 — 처음 열 때 한 번, 닫았다 다시 열어도 다시 부르지 않는다. */
it("라이선스 정보를 열면 about 을 한 번만 부르고 판과 라이선스를 보인다", async () => {
  const { user, dialog } = await 설정열기();
  expect(부름.mock.calls.some(([m]) => m === "about")).toBe(false);
  // 설정 본문에는 싣지 않는다 — 바닥의 작은 글씨로만 닿는다.
  expect(within(dialog).queryByLabelText("제3자 고지")).toBeNull();

  const link = within(dialog).getByRole("button", { name: "라이선스 정보" });
  await user.click(link);
  const about = await screen.findByRole("dialog", { name: "라이선스 정보" });
  expect(await within(about).findByText("계약 목록 0.5.0 · MIT 라이선스")).toBeTruthy();
  expect(within(about).getByLabelText("라이선스").textContent).toMatch(/Copyright \(c\) 2026 JM/);
  expect(within(about).getByLabelText("제3자 고지").textContent).toMatch(/== nuget ClosedXML/);

  await user.click(within(about).getByRole("button", { name: "닫기" }));
  expect(screen.queryByRole("dialog", { name: "라이선스 정보" })).toBeNull();
  await user.click(link);
  expect(await within(await screen.findByRole("dialog", { name: "라이선스 정보" })).findByLabelText("라이선스")).toBeTruthy();
  expect(부름.mock.calls.filter(([m]) => m === "about")).toHaveLength(1);
});

/**
 * 작업자료 바꾸기(ADR-032). 고르기만으로는 바꾸지 않는다 — 고른 자리와 그 뜻을 확인 창에 보이고, 확인을 받아야
 * 바꾸기를 부른다. 바꾸면 창이 다시 뜨고 브라우저 수집 대상이 바뀌는 일이라서다.
 */
describe("설정 — 작업자료", () => {
  const 바꾸는요청 = ["moveWorkfile", "switchWorkfile", "newWorkfile"];
  const 바꿨나 = () => 부름.mock.calls.some(([m]) => 바꾸는요청.includes(m));

  it("옮기기는 고른 자리·재시작·수집·옛 파일 삭제를 보이고 확인한 뒤에야 옮긴다", async () => {
    const { user, dialog } = await 설정열기();
    await user.click(within(dialog).getByRole("button", { name: "작업자료 옮기기…" }));

    await waitFor(() => expect(부름).toHaveBeenCalledWith("pickWorkfile", ["move"]));
    const confirm = await screen.findByRole("dialog", { name: "작업자료 옮기기" });
    expect(confirm.textContent).toContain("D:\\계약\\계약자료.pclm");
    expect(confirm.textContent).toMatch(/창을 다시 띄웁니다/);
    expect(confirm.textContent).toMatch(/브라우저 수집은 이제 새 자리에 저장합니다/);
    expect(confirm.textContent).toMatch(/옛 파일을 지웁니다.+계약자료\.pclm/);
    expect(바꿨나()).toBe(false);

    await user.click(within(confirm).getByRole("button", { name: "옮기고 다시 띄우기" }));

    await waitFor(() => expect(부름).toHaveBeenCalledWith("moveWorkfile", ["D:\\계약\\계약자료.pclm"]));
    expect(await screen.findByText(/작업자료를 바꿨습니다 — 창을 다시 띄웁니다/)).toBeTruthy();
    expect(screen.queryByRole("dialog", { name: "작업자료 옮기기" })).toBeNull();
  });

  it("확인 창에서 취소하면 아무것도 바꾸지 않는다", async () => {
    const { user, dialog } = await 설정열기();
    await user.click(within(dialog).getByRole("button", { name: "새 계약자료…" }));

    const confirm = await screen.findByRole("dialog", { name: "새 계약자료 만들기" });
    expect(confirm.textContent).toMatch(/지금 작업자료는 그대로 남습니다/);
    await user.click(within(confirm).getByRole("button", { name: "취소" }));

    expect(screen.queryByRole("dialog", { name: "새 계약자료 만들기" })).toBeNull();
    expect(바꿨나()).toBe(false);
  });

  it("제출본으로 바꾸면 원본은 그대로 두고 사본을 새 계약자료로 만든다고 말한 뒤 둘을 함께 넘긴다", async () => {
    const { user, dialog } = await 설정열기();
    await user.click(within(dialog).getByRole("button", { name: "작업자료 바꾸기…" }));

    await waitFor(() => expect(부름).toHaveBeenCalledWith("pickWorkfile", ["switch"]));
    const confirm = await screen.findByRole("dialog", { name: "새 계약자료로 이어 쓰기" });
    expect(confirm.textContent).toMatch(/제출본의 사본을/);
    expect(confirm.textContent).toMatch(/원본은 바뀌지 않습니다/);
    expect(confirm.textContent).toMatch(/열려 있던 확장 검토 화면은 다시 확인해야 합니다/);

    await user.click(within(confirm).getByRole("button", { name: "만들고 다시 띄우기" }));

    await waitFor(() => expect(부름).toHaveBeenCalledWith("switchWorkfile", [
      "C:\\Users\\홍길동\\Documents\\제출_홍길동_20260901.pclm",
      "C:\\Users\\홍길동\\Documents\\계약자료.pclm",
    ]));
  });

  it("새 계약자료는 확인한 뒤 newWorkfile 을 부른다", async () => {
    const { user, dialog } = await 설정열기();
    await user.click(within(dialog).getByRole("button", { name: "새 계약자료…" }));

    const confirm = await screen.findByRole("dialog", { name: "새 계약자료 만들기" });
    await user.click(within(confirm).getByRole("button", { name: "만들고 다시 띄우기" }));

    await waitFor(() => expect(부름).toHaveBeenCalledWith("newWorkfile", ["C:\\Users\\홍길동\\Documents\\계약자료.pclm"]));
  });

  /** 고르다 그만둔 것은 실패가 아니다. 확인 창도 알림도 서지 않는다. */
  it("고르기를 그만두면 확인 창이 뜨지 않는다", async () => {
    가로채기("pickWorkfile", () => Promise.resolve(null));
    const { user, dialog } = await 설정열기();
    await user.click(within(dialog).getByRole("button", { name: "작업자료 옮기기…" }));

    await waitFor(() => expect(부름).toHaveBeenCalledWith("pickWorkfile", ["move"]));
    expect(screen.queryByRole("dialog", { name: "작업자료 옮기기" })).toBeNull();
    expect(바꿨나()).toBe(false);
  });

  /** 쓸 수 없는 자리는 고른 자리에서 거절된다 — 확인을 받은 뒤에 거절하지 않는다. */
  it("고른 자리를 쓸 수 없으면 까닭을 알리고 확인 창을 띄우지 않는다", async () => {
    가로채기("pickWorkfile", () => Promise.reject(new Error("그 자리에 이미 파일이 있어 덮지 않습니다 — 다른 이름을 고르세요")));
    const { user, dialog } = await 설정열기();
    await user.click(within(dialog).getByRole("button", { name: "작업자료 옮기기…" }));

    expect(await screen.findByText(/이미 파일이 있어 덮지 않습니다/)).toBeTruthy();
    expect(screen.queryByRole("dialog", { name: "작업자료 옮기기" })).toBeNull();
  });

  it("백업은 확인 없이 뜨고 떨어진 자리를 알린다", async () => {
    const { user, dialog } = await 설정열기();
    await user.click(within(dialog).getByRole("button", { name: "백업 만들기…" }));

    await waitFor(() => expect(부름).toHaveBeenCalledWith("backupWorkfile", []));
    expect(await screen.findByText(/백업을 만들었습니다.+계약자료_백업_\d{8}\.pclm/)).toBeTruthy();
    expect(screen.getAllByRole("dialog")).toHaveLength(1);   // 설정만 — 확인 창이 서지 않았다
  });

  /** 복구는 「작업자료 바꾸기」 가 곧 「새 계약자료로 만들 자리」 저장 대화상자를 띄운다 — 안내가 그것을 미리 말한다. */
  it("백업 안내는 복구할 때 새 작업자료로 둘 자리를 묻고 지금 작업자료는 남는다고 말한다", async () => {
    const { dialog } = await 설정열기();
    expect(within(dialog).getByText(/백업 파일을 고르면 그 사본을 새 작업자료로 둘 자리를 묻습니다\. 지금 작업자료는 그대로 남습니다\./)).toBeTruthy();
  });
});

/**
 * 「창」 — 이 컴퓨터에서 창을 어떻게 다룰지. 자료가 아니라 저장 단추를 타지 않고 누르는 그 자리에서 다리로 간다.
 * 다리의 값은 글이라 켜짐·꺼짐은 "true"·"false" 로 간다.
 */
describe("설정 — 창", () => {
  const 알림영역 = (dialog: HTMLElement) =>
    within(dialog).findByRole("checkbox", { name: "닫아도 끝내지 않고 알림 영역에 둡니다" });

  it("알림 영역 두기를 켜고 끄면 그 자리에서 saveWindowPrefs 를 부른다", async () => {
    const { user, dialog } = await 설정열기();
    const box = await 알림영역(dialog);
    expect((box as HTMLInputElement).checked).toBe(false);

    await user.click(box);
    await waitFor(() => expect(부름).toHaveBeenCalledWith("saveWindowPrefs", ["true", "false"]));
    await waitFor(() => expect((box as HTMLInputElement).checked).toBe(true));

    await user.click(box);
    await waitFor(() => expect(부름).toHaveBeenCalledWith("saveWindowPrefs", ["false", "false"]));
    await waitFor(() => expect((box as HTMLInputElement).checked).toBe(false));
    expect(부름.mock.calls.some(([m]) => m === "saveSettings")).toBe(false);
  });

  /** 하나를 바꿀 때 다른 하나는 지금 값 그대로 함께 간다 — 빠뜨리면 다리가 그것을 꺼짐으로 바꾼다. */
  it("자동 실행을 켜면 알림 영역 두기는 지금 값 그대로 함께 넘긴다", async () => {
    창.몸가짐 = { ...창.몸가짐, closeToTray: true };
    const { user, dialog } = await 설정열기();
    const box = await within(dialog).findByRole("checkbox", { name: "Windows 에 로그인하면 자동으로 켭니다" });
    expect(within(dialog).getByText("알림 영역 두기가 켜져 있으면 창 없이 조용히 시작합니다.")).toBeTruthy();

    await user.click(box);
    await waitFor(() => expect(부름).toHaveBeenCalledWith("saveWindowPrefs", ["true", "true"]));
    await waitFor(() => expect((box as HTMLInputElement).checked).toBe(true));
    expect((within(dialog).getByRole("checkbox", { name: "닫아도 끝내지 않고 알림 영역에 둡니다" }) as HTMLInputElement).checked).toBe(true);
  });

  /** 시험 홈의 창은 컴퓨터에 하나뿐인 자동 실행을 건드리지 않는다 — 칸은 잠기고 까닭이 그 자리에 선다. */
  it("자동 실행을 바꿀 수 없는 자리면 칸을 잠그고 까닭을 보인다", async () => {
    창.몸가짐 = {
      ...창.몸가짐,
      autostartAvailable: false,
      autostartReason: "시험 홈(--home)으로 띄운 창이라 자동 실행을 바꾸지 않습니다. 업무 홈의 창에서 바꾸세요.",
    };
    const { dialog } = await 설정열기();
    const box = await within(dialog).findByRole("checkbox", { name: "Windows 에 로그인하면 자동으로 켭니다" });

    expect((box as HTMLInputElement).disabled).toBe(true);
    expect(within(dialog).getByText(/시험 홈\(--home\)으로 띄운 창이라/)).toBeTruthy();
    expect(within(dialog).queryByText("알림 영역 두기가 켜져 있으면 창 없이 조용히 시작합니다.")).toBeNull();
    // 알림 영역 두기는 그대로 열려 있다.
    expect((within(dialog).getByRole("checkbox", { name: "닫아도 끝내지 않고 알림 영역에 둡니다" }) as HTMLInputElement).disabled).toBe(false);
  });

  /** 압축 안에서 켠 창도 자동 실행을 켤 수 있다 — 복사해 둘 자리를 미리 알리고, 켜면 다리가 그 복사본을 적는다. */
  it("압축 안에서 켠 창은 복사해 둘 자리를 알리고 자동 실행을 켤 수 있다", async () => {
    창.몸가짐 = { ...창.몸가짐, autostartCopyTarget: 복사자리 };
    const { user, dialog } = await 설정열기();
    const box = await within(dialog).findByRole("checkbox", { name: "Windows 에 로그인하면 자동으로 켭니다" });

    expect((box as HTMLInputElement).disabled).toBe(false);
    expect(within(dialog).getByText(`압축 파일 안에서 켠 프로그램입니다. 자동 실행을 켜면 이 프로그램을 ${복사자리}에 복사해 두고 그 복사본을 로그인할 때 켭니다.`)).toBeTruthy();
    expect(within(dialog).queryByText(/자동 실행을 걸 수 없습니다/)).toBeNull();

    await user.click(box);
    await waitFor(() => expect(부름).toHaveBeenCalledWith("saveWindowPrefs", ["false", "true"]));
    await waitFor(() => expect((box as HTMLInputElement).checked).toBe(true));
  });

  it("제자리에서 켠 창에는 복사 안내가 서지 않는다", async () => {
    const { dialog } = await 설정열기();
    await within(dialog).findByRole("checkbox", { name: "Windows 에 로그인하면 자동으로 켭니다" });
    expect(within(dialog).queryByText(/압축 파일 안에서 켠 프로그램입니다/)).toBeNull();
  });

  it("바꾸지 못하면 까닭을 알리고 칸을 되돌린다", async () => {
    가로채기("saveWindowPrefs", () => Promise.reject(new Error("홈에 쓰지 못했습니다")));
    const { user, dialog } = await 설정열기();
    const box = await 알림영역(dialog);

    await user.click(box);
    expect(await screen.findByText("홈에 쓰지 못했습니다")).toBeTruthy();
    await waitFor(() => expect((box as HTMLInputElement).checked).toBe(false));
  });

  /** 열람 창은 따로 뜬 프로세스라 알림 영역에 두지 않는다 — 절이 서지 않고, 다리도 거절한다. */
  it("열람 창에는 창 절이 서지 않는다", async () => {
    스위치.열람 = true;
    const { dialog } = await 설정열기();
    await within(dialog).findByRole("heading", { name: "열람 중인 자료" });
    // 가짜 다리가 답하고도 남을 만큼 기다린 뒤에 본다 — 아직 읽지 않아 비어 있는 것과 갈라야 한다.
    // (처음 뜰 때의 요청은 엿듣개를 비껴갈 수 있어 그 답을 직접 기다리지 못한다.)
    await new Promise((r) => setTimeout(r, 300));

    expect(within(dialog).queryByText("닫아도 끝내지 않고 알림 영역에 둡니다")).toBeNull();
    expect(within(dialog).queryByText("Windows 에 로그인하면 자동으로 켭니다")).toBeNull();
    expect(부름.mock.calls.some(([m]) => m === "saveWindowPrefs")).toBe(false);
  });
});

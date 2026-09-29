import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { invoke } from "./mock";

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
  HTMLDialogElement.prototype.showModal = function () { this.open = true; };
  HTMLDialogElement.prototype.close = function () { this.open = false; };
});

beforeEach(() => {
  부름.mockClear();
  부름.mockImplementation(본디);
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


it("내장 확장 준비 실패는 재시도할 수 있고 준비 후 브라우저 추가를 안내한다", async () => {
  let complete: (value: unknown) => void = () => {};
  const { user, dialog } = await 설정열기();
  await user.click(within(dialog).getByText("Edge · Chrome 확장 추가"));
  const prepare = within(dialog).getByRole("button", { name: "확장 준비" });
  가로채기("prepareExtension", () => Promise.reject(new Error("등록 권한을 확인하세요.")));
  await user.click(prepare);
  expect(await within(dialog).findByRole("alert")).toHaveProperty("textContent", "등록 권한을 확인하세요.");
  expect(within(dialog).queryByRole("button", { name: "Edge 확장 관리 열기" })).toBeNull();
  가로채기("prepareExtension", () => new Promise(resolve => { complete = resolve; }));
  await user.click(prepare);
  expect((prepare as HTMLButtonElement).disabled).toBe(true);
  const folder = "C:\\Users\\테스트 사용자\\AppData\\Local\\Pclm\\extension";
  complete({ folder, version: "0.3.0" });
  expect(await within(dialog).findByText(/브라우저에서 추가를 완료하세요/)).toBeTruthy();
  // 폴더 선택 창에 바로 붙여 넣게 경로를 복사해 둔다.
  expect(await within(dialog).findByText(/폴더 경로를 복사했습니다/)).toBeTruthy();
  expect(await navigator.clipboard.readText()).toBe(folder);
  expect(within(dialog).getByLabelText(/확장 폴더 ·/)).toHaveProperty("value", folder);
  expect(within(dialog).queryByRole("alert")).toBeNull();
  가로채기("openExtensionSetup", () => Promise.resolve(null));
  for (const [target, label] of [["edge", "Edge 확장 관리 열기"], ["chrome", "Chrome 확장 관리 열기"], ["folder", "확장 폴더 열기"]]) {
    await user.click(within(dialog).getByRole("button", { name: label }));
    await waitFor(() => expect(부름).toHaveBeenCalledWith("openExtensionSetup", [target]));
  }
});

it("고급 수집은 충돌 선택 전 저장을 막고 검토한 선택을 전달한다", async () => {
  부름.mockImplementation(async (m, args) => {
    if (m !== "erpTools" || args[0] === "list") return 본디(m, args);
    if (args[0] === "saveMapping") return "revision";
    if (args[0] === "inspect") return { entity: "R26BK00000001-001", itemCount: 1, baseToken: "token", unmatched: [],
      changes: [{ id: "header/title", table: "notice", line: 0, field: "title", before: "이전", after: "새 제목", conflict: true }] };
    if (args[0] === "capture") return { changed: true };
    throw new Error("예상하지 않은 작업");
  });
  const { user, dialog } = await 설정열기();
  await user.click(within(dialog).getByText("고급 · 수집 매핑과 JSON 검토"));
  const file = await within(dialog).findByLabelText("JSON 파일 선택");
  await waitFor(() => expect(file.closest("fieldset")!.disabled).toBe(false));
  await user.upload(file, new File([JSON.stringify({ pointInfo: {}, tables: {} })], "공고.json", { type: "application/json" }));
  await within(dialog).findByText("공고.json");
  await user.click(within(dialog).getByRole("button", { name: "활성 매핑으로 미리보기" }));
  const save = await within(dialog).findByRole("button", { name: "검토한 자료 저장" });
  expect((save as HTMLButtonElement).disabled).toBe(true);
  await user.selectOptions(within(dialog).getByLabelText("title 선택"), "keep");
  await user.click(save);
  await waitFor(() => expect(부름.mock.calls.some(([m, args]) => m === "erpTools" && args[0] === "capture")).toBe(true));
  const args = 부름.mock.calls.find(([m, args]) => m === "erpTools" && args[0] === "capture")![1];
  expect(JSON.parse(String(args[1])).choices).toEqual({ "header/title": "keep" });
  expect(await within(dialog).findByText("검토한 자료를 저장했습니다.")).toBeTruthy();
});

it("확장 상태는 연결이 없거나, 같은 판이 돌거나, 옛 판이 돌고 있음을 한 줄로 보인다", async () => {
  const 상태 = (contacts: { browser: string; version: string; at: string }[]) =>
    가로채기("extensionStatus", () => Promise.resolve({ prepared: true, embeddedVersion: "0.4.1", diskVersion: "0.4.1", contacts }));

  상태([]);
  let { dialog } = await 설정열기();
  expect(await within(dialog).findByText("확장 파일은 준비했지만 아직 브라우저에서 연결된 적이 없습니다.")).toBeTruthy();
  expect(within(dialog).getByText(/개발자 모드/)).toBeTruthy();
  // 폴더 경로는 아직 보이지 않으므로 「위 확장 폴더」 를 가리키지 않는다.
  expect(within(dialog).getByText(/「확장 준비」를 누르면 보이는 확장 폴더/)).toBeTruthy();
  cleanup();

  상태([{ browser: "Edge", version: "0.4.1", at: "2026-09-28T14:02:37" }]);
  ({ dialog } = await 설정열기());
  expect(await within(dialog).findByText("Edge 연결됨 · 확장 0.4.1 · 마지막 연결 2026/09/28 14:02")).toBeTruthy();
  expect(within(dialog).queryByText(/연결된 적이 없습니다/)).toBeNull();
  cleanup();

  상태([{ browser: "Chrome", version: "0.4.0", at: "2026-09-27T09:00:00" }]);
  ({ dialog } = await 설정열기());
  expect(await within(dialog).findByText(
    "Chrome 확장이 옛 판(0.4.0)입니다. 나라장터 화면을 열면 새 판을 스스로 불러옵니다. 그대로면 확장 관리에서 새로고침하세요.")).toBeTruthy();
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

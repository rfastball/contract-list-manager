import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { invoke, 스위치, 창 } from "./mock";

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
  스위치.열람 = false;
  창.몸가짐 = { closeToTray: false, autostart: false, autostartAvailable: true, autostartReason: null };
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


/**
 * 브라우저 수집(확장 준비·상태·고급 JSON 검토)은 나라장터 화면으로 옮겼다 — 그 약속은 Nara.test 가 붙든다.
 * 설정에는 그리로 가는 길 한 줄만 남는다.
 */
it("브라우저 수집 절은 나라장터를 가리키고, 누르면 설정을 닫고 그리로 간다", async () => {
  const { user, dialog } = await 설정열기();
  expect(within(dialog).getByText("브라우저 수집은 왼쪽 메뉴의 나라장터에서 봅니다.")).toBeTruthy();
  expect(within(dialog).queryByText("Edge · Chrome 확장 추가")).toBeNull();
  expect(within(dialog).queryByText("고급 · 수집 매핑과 JSON 검토")).toBeNull();
  expect(within(dialog).queryByRole("button", { name: "확장 준비" })).toBeNull();

  await user.click(within(dialog).getByRole("button", { name: "나라장터 열기" }));
  expect(screen.queryByRole("dialog", { name: "설정" })).toBeNull();
  expect(screen.getByRole("tab", { name: /^나라장터/ }).getAttribute("aria-selected")).toBe("true");
  expect(await screen.findByRole("heading", { name: "나라장터", level: 1 })).toBeTruthy();
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
    await within(dialog).findByText("열람 중인 자료");
    // 가짜 다리가 답하고도 남을 만큼 기다린 뒤에 본다 — 아직 읽지 않아 비어 있는 것과 갈라야 한다.
    // (처음 뜰 때의 요청은 엿듣개를 비껴갈 수 있어 그 답을 직접 기다리지 못한다.)
    await new Promise((r) => setTimeout(r, 300));

    expect(within(dialog).queryByText("닫아도 끝내지 않고 알림 영역에 둡니다")).toBeNull();
    expect(within(dialog).queryByText("Windows 에 로그인하면 자동으로 켭니다")).toBeNull();
    expect(부름.mock.calls.some(([m]) => m === "saveWindowPrefs")).toBe(false);
  });
});

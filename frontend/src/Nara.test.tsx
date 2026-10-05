import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { invoke, 수집, 수집줄, 스위치 } from "./mock";
import { 원천상태 } from "./Nara";
import type { ExtensionStatus } from "./types";

/**
 * 나라장터 — 사이드바의 원천과 세 갈래, 그리고 나라장터 화면.
 *
 * <p>앱이 아는 것은 지금 붙은 브라우저(상시 연결)·붙고 끊긴 기록·인사 흔적·성공한 저장의 기록이다. 붙어 있을 때만
 * 「연결됨」 을 말하는지, 끊긴 까닭을 지어내지 않는지, 새로 들어온 것이 +N 으로 섰다가 그 갈래를 열면 걷히는지를 본다.
 * 값은 모두 가짜 다리가 지어낸 것이다.</p>
 */
vi.mock("./mock", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./mock")>();
  return { ...actual, invoke: vi.fn(actual.invoke) };
});

const 부름 = vi.mocked(invoke);
const 본디 = 부름.getMockImplementation()!;

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
  스위치.확장 = "연결";
  스위치.확장상태 = null;
  수집.판 = 0;
  수집.줄 = [];
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

const 오늘 = (hhmm: string) => {
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}T${hhmm}:00`;
};

/** 기본 수집 셋(가짜 다리의 것과 같은 번호). */
const 기본 = () => [
  수집줄(오늘("09:12"), "계약", "R26TA0911050100", "2026년 가람 수질측정기 조달", true),
  수집줄(오늘("08:57"), "공고", "R26BK09017002-000", "26년 한별 공압식승강판 20톤 구매", false),
  수집줄(오늘("08:41"), "접수", "MPKPLA26910286-000", "26년한별윈치8종구매", true),
];

async function 나라장터열기() {
  const user = userEvent.setup();
  render(<App />);
  await user.click(await screen.findByRole("tab", { name: /^나라장터/ }));
  await screen.findByRole("heading", { name: "나라장터", level: 1 });
  return user;
}

describe("원천 상태", () => {
  const status = (over: Partial<ExtensionStatus>): ExtensionStatus => ({
    prepared: true, embeddedVersion: "0.9.0", diskVersion: "0.9.0", contacts: [], folder: "C:\\확장", live: [], log: [], errors: [], pastError: null, ...over,
  });
  const 인사 = (version: string, at = "2026-10-05T09:12:00", browser = "Chrome") => ({ contacts: [{ browser, version, at }] });
  const 붙음 = (version: string, connectedAt = "2026-10-05T09:12:00", browser = "Chrome") => ({ browser, version, connectedAt });

  it("붙어 있을 때만 「연결됨」 을 말한다 — 인사 흔적만 있으면 「연결 안 됨」", () => {
    const 말 = [
      원천상태(status({ prepared: false, diskVersion: null }), false).text,
      원천상태(status({}), false).text,
      원천상태(status({ ...인사("0.9.0"), live: [붙음("0.9.0")] }), false).text,
      원천상태(status({ ...인사("0.8.0"), live: [붙음("0.8.0")] }), false).text,
      원천상태(status(인사("0.9.0")), false).text,
      원천상태(status({ ...인사("0.9.0"), live: [붙음("0.9.0")] }), true).text,
    ];
    expect(말).toEqual(["확장 설치 필요", "확장 연결 기다림", "Chrome 연결됨", "확장이 옛 판", "연결 안 됨", "열람 창 · 수집 안 함"]);
    // 옛 판의 인사 흔적만 남고 지금 붙은 것이 없으면 옛 판이 아니라 끊긴 것이다.
    expect(원천상태(status(인사("0.8.0")), false).kind).toBe("offline");
  });

  it("여럿이 붙어 있으면 이름을 잇고, 가장 먼저 붙은 때부터 연결됨이다", () => {
    const s = status({ live: [붙음("0.9.0", "2026-10-05T10:02:00", "Edge"), 붙음("0.9.0", "2026-10-05T09:12:00")] });
    expect(원천상태(s, false)).toMatchObject({ kind: "live", text: "Edge · Chrome 연결됨", since: "2026-10-05T09:12:00" });
  });

  it("끊겼을 때의 마지막 연결은 인사 흔적과 오늘의 기록 중 늦은 것이다", () => {
    const s = status({ ...인사("0.9.0", "2026-10-05T09:12:00"),
      log: [{ at: "2026-10-05T18:02:00", event: "disconnected", browser: "Edge", version: "0.9.0", previous: "" }] });
    expect(원천상태(s, false)).toMatchObject({ kind: "offline", lastSeen: { browser: "Edge", at: "2026-10-05T18:02:00" } });
  });

  it("디스크에 풀어 둔 확장이 앱의 판과 다르면 붙은 판이 같아도 옛 판이다", () => {
    const s = status({ diskVersion: "0.8.0", live: [붙음("0.9.0")] });
    expect(원천상태(s, false)).toMatchObject({ kind: "stale", staleVersion: "0.8.0" });
  });

  it("상태를 읽지 못한 창은 준비 전으로 본다", () => {
    expect(원천상태(null, false).text).toBe("확장 설치 필요");
  });

  it("사이드바 원천 밑에 상태 한 줄을 내고, 붙어 있으면 줄기를 실선으로 긋는다", async () => {
    render(<App />);
    const source = await screen.findByRole("tab", { name: /^나라장터/ });
    await waitFor(() => expect(source.textContent).toContain("Chrome 연결됨"));
    expect(document.querySelector(".source-tree")!.className).toContain("connected");
    expect(document.querySelector(".source-icon")!.className).toContain("ok");
    cleanup();

    스위치.확장 = "옛 판";
    render(<App />);
    const stale = await screen.findByRole("tab", { name: /^나라장터/ });
    await waitFor(() => expect(stale.textContent).toContain("확장이 옛 판"));
    expect(document.querySelector(".source-tree")!.className).toContain("connected");
    cleanup();

    스위치.확장 = "끊김";
    render(<App />);
    const offline = await screen.findByRole("tab", { name: /^나라장터/ });
    await waitFor(() => expect(offline.textContent).toContain("연결 안 됨"));
    expect(document.querySelector(".source-tree")!.className).not.toContain("connected");
    expect(document.querySelector(".source-icon")!.className).not.toMatch(/ok|warn/);
  });

  it("확장 상태는 화면과 상관없이 다시 읽어, 브라우저가 붙고 끊기면 사이드바가 저절로 바뀐다", async () => {
    스위치.확장 = "끊김";
    render(<App />);
    const source = await screen.findByRole("tab", { name: /^나라장터/ });
    await waitFor(() => expect(source.textContent).toContain("연결 안 됨"));
    스위치.확장 = "연결";
    await waitFor(() => expect(source.textContent).toContain("Chrome 연결됨"), { timeout: 4000 });
  }, 10000);
});

describe("들어옴", () => {
  /** 새 수집 한 건을 맨 앞에 넣고 판을 올린다 — 다리가 저장한 뒤의 모습이다. */
  const 들어온다 = (kind: "접수" | "공고" | "계약", number: string, title: string) => {
    수집.줄 = [수집줄(오늘("09:30"), kind, number, title, true), ...수집.줄];
    수집.판++;
  };

  it("창이 떠 있는 동안 들어온 것은 그 갈래에 +N 으로 서고, 그 갈래를 열면 걷힌다", async () => {
    수집.줄 = 기본();
    const user = userEvent.setup();
    render(<App />);

    // 처음 읽은 오늘 것은 이미 본 것으로 친다.
    const contract = await screen.findByRole("tab", { name: /^계약 42건$/ });
    await waitFor(() => expect(부름).toHaveBeenCalledWith("captures", []));
    expect(contract.getAttribute("aria-label")).toBe("계약 42건");

    들어온다("계약", "R26TA0911050200", "26년 한별 공압식승강판 20톤 구매");
    await waitFor(() => expect(contract.getAttribute("aria-label")).toBe("계약 42건, 새로 들어온 1건"), { timeout: 4000 });
    expect(contract.textContent).toContain("+1");
    // 다른 갈래에는 서지 않는다.
    expect(screen.getByRole("tab", { name: /^공고/ }).getAttribute("aria-label")).toBe("공고 5건");

    들어온다("계약", "R26TA0911050300", "26년 가람 이온분리-성분측정기 구매");
    await waitFor(() => expect(contract.getAttribute("aria-label")).toBe("계약 42건, 새로 들어온 2건"), { timeout: 4000 });

    await user.click(contract);
    expect(contract.getAttribute("aria-selected")).toBe("true");
    await waitFor(() => expect(contract.getAttribute("aria-label")).toBe("계약 42건"));
    expect(contract.textContent).not.toContain("+");
  }, 15000);

  it("그 갈래를 보고 있는 동안 들어온 것은 세지 않는다", async () => {
    수집.줄 = 기본();
    const user = userEvent.setup();
    render(<App />);
    const request = await screen.findByRole("tab", { name: /^접수/ });
    await waitFor(() => expect(부름).toHaveBeenCalledWith("captures", []));
    await user.click(request);

    들어온다("접수", "MPKPLA26910301-000", "지어낸 접수");
    await waitFor(() => expect(부름.mock.calls.filter(([m]) => m === "captures").length).toBeGreaterThan(2), { timeout: 4000 });
    await new Promise((r) => setTimeout(r, 1200));
    expect(request.getAttribute("aria-label")).not.toMatch(/새로 들어온/);
  }, 15000);

  it("움직임을 줄이면 흐름 없이 +N 만 선다", async () => {
    vi.stubGlobal("matchMedia", (q: string) => ({ matches: q.includes("reduce"), media: q, addEventListener() {}, removeEventListener() {} }));
    수집.줄 = 기본();
    render(<App />);
    const notice = await screen.findByRole("tab", { name: /^공고/ });
    await waitFor(() => expect(부름).toHaveBeenCalledWith("captures", []));

    들어온다("공고", "R26BK09017003-000", "지어낸 공고");
    await waitFor(() => expect(notice.getAttribute("aria-label")).toMatch(/새로 들어온 1건$/), { timeout: 4000 });
    expect(document.querySelector(".source-flow")).toBeNull();
    expect(document.querySelector(".tree-node.flash")).toBeNull();
  }, 15000);

  it("끊겨 있으면 통로를 따라 미끄러지지 않는다 — 새 줄은 그대로 펼쳐 들어온다", async () => {
    스위치.확장 = "끊김";
    수집.줄 = 기본();
    await 나라장터열기();
    await screen.findAllByRole("row", { name: /수질측정기/ });

    들어온다("공고", "R26BK09017003-000", "지어낸 공고");
    // 줄은 수집 기록이 닿은 그 그림에 서고, 펼침 표시(fresh)는 들어옴 효과가 다음 그림에 단다 — 줄이 보인 순간이 아니라 그 뒤를 본다.
    await waitFor(() => expect(screen.getByRole("row", { name: /지어낸 공고/ }).className).toContain("fresh"), { timeout: 4000 });
    expect(document.querySelector(".conduit-glide")).toBeNull();
  }, 15000);

  it("움직임을 줄이지 않으면 원천에서 그 갈래로 흐르고 새 줄이 펼쳐 들어온다", async () => {
    수집.줄 = 기본();
    await 나라장터열기();
    await screen.findAllByRole("row", { name: /수질측정기/ });

    들어온다("공고", "R26BK09017003-000", "지어낸 공고");
    await waitFor(() => expect(document.querySelector(".source-flow")).not.toBeNull(), { timeout: 4000 });
    const row = await screen.findByRole("row", { name: /지어낸 공고/ });
    expect(row.className).toContain("fresh");
    expect(document.querySelector(".conduit-glide")).not.toBeNull();
  }, 15000);
});

describe("나라장터 화면", () => {
  it("설치 전에는 1단계 「확장 준비」 만 서고, 헤더 동작 대신 각주에 JSON 길이 있다", async () => {
    스위치.확장 = "설치 전";
    await 나라장터열기();

    const setup = await screen.findByRole("region", { name: "확장 설치" });
    expect(within(setup).getByRole("button", { name: "확장 준비" })).toBeTruthy();
    expect(within(setup).queryByLabelText("확장 폴더")).toBeNull();
    expect(within(setup).queryByText("Chrome 연결을 기다리는 중")).toBeNull();
    expect(screen.queryByRole("button", { name: "JSON 파일로 가져오기…" })).toBeNull();
    expect(within(setup).getByRole("button", { name: "JSON 파일로 가져오기" })).toBeTruthy();
    expect(screen.getByRole("tab", { name: /^나라장터/ }).textContent).toContain("확장 설치 필요");
  });

  it("준비했으나 인사가 없으면 1단계를 마치고 폴더와 확장 관리 여는 길을 세운 뒤, 첫 인사가 들어오면 저절로 통로로 넘어간다", async () => {
    스위치.확장 = "기다림";
    const user = await 나라장터열기();

    const setup = await screen.findByRole("region", { name: "확장 설치" });
    expect(within(setup).getByText("준비했습니다 · 확장 0.9.0")).toBeTruthy();
    expect(within(setup).getByText("개발자 모드")).toBeTruthy();
    expect(within(setup).getByLabelText("확장 폴더")).toHaveProperty("value", "C:\\Users\\홍길동\\AppData\\Local\\Pclm\\extension");
    expect(within(setup).getByRole("status").textContent).toBe("Chrome 연결을 기다리는 중");
    expect(screen.getByRole("tab", { name: /^나라장터/ }).textContent).toContain("확장 연결 기다림");

    await user.click(within(setup).getByRole("button", { name: "경로 복사" }));
    expect(await navigator.clipboard.readText()).toBe("C:\\Users\\홍길동\\AppData\\Local\\Pclm\\extension");
    expect(within(setup).getByText(/폴더 경로를 복사했습니다/)).toBeTruthy();

    for (const [target, label] of [["chrome", "Chrome 확장 관리 열기"], ["edge", "Edge 확장 관리 열기"]]) {
      const button = within(setup).getByRole("button", { name: label }) as HTMLButtonElement;
      await waitFor(() => expect(button.disabled).toBe(false));
      await user.click(button);
      await waitFor(() => expect(부름).toHaveBeenCalledWith("openExtensionSetup", [target]));
    }

    // 브라우저가 처음 붙는다 — 2초마다 다시 읽으므로 따로 누를 것이 없다.
    스위치.확장 = "연결";
    const conduit = await screen.findByRole("region", { name: "수집 통로" }, { timeout: 4000 });
    expect(within(conduit).getByText("연결됨 · 09:12부터")).toBeTruthy();
    expect(screen.queryByRole("region", { name: "확장 설치" })).toBeNull();
    expect(screen.getByRole("tab", { name: /^나라장터/ }).textContent).toContain("Chrome 연결됨");
  }, 15000);

  it("붙어 있으면 통로에 브라우저·확장·작업자료를 실선으로 잇고 「연결됨 · HH:MM부터」 를 말한다", async () => {
    await 나라장터열기();
    const conduit = await screen.findByRole("region", { name: "수집 통로" });
    expect(within(conduit).getByText("Chrome")).toBeTruthy();
    expect(within(conduit).getByText("09:12부터 연결됨")).toBeTruthy();
    expect(within(conduit).getByText("확장 0.9.0")).toBeTruthy();
    expect(within(conduit).getByText("단축키 Alt+Shift+S")).toBeTruthy();
    expect(within(conduit).getByText("계약자료.pclm")).toBeTruthy();
    expect(within(conduit).getByText("지금 열린 작업자료")).toBeTruthy();
    expect(within(conduit).getByText("연결됨 · 09:12부터")).toBeTruthy();
    expect(within(conduit).getByText("나라장터에서 저장하면 아래 목록과 왼쪽 메뉴에 바로 나타납니다.")).toBeTruthy();
    expect([...conduit.querySelectorAll(".conduit-dash")].every((d) => d.classList.contains("solid"))).toBe(true);
    expect(conduit.querySelector(".dot")!.className).toContain("ok");
    expect(within(conduit).queryByRole("alert")).toBeNull();
    expect(screen.getByRole("button", { name: "JSON 파일로 가져오기…" })).toBeTruthy();
  });

  it("끊겼으면 점선으로 긋고, 실제 브라우저 이름으로 다시 켜면 이어진다고만 말한다", async () => {
    스위치.확장 = "끊김";
    await 나라장터열기();
    const conduit = await screen.findByRole("region", { name: "수집 통로" });
    expect(within(conduit).getByText("Chrome · 연결 안 됨")).toBeTruthy();
    expect(within(conduit).getByText("마지막 연결 오늘 18:02")).toBeTruthy();
    expect(within(conduit).getByText("연결 안 됨")).toBeTruthy();
    expect(within(conduit).getByText("Chrome이 꺼져 있거나 확장이 꺼져 있습니다. Chrome을 켜면 자동으로 다시 연결됩니다.")).toBeTruthy();
    expect([...conduit.querySelectorAll(".conduit-dash")].some((d) => d.classList.contains("solid"))).toBe(false);
    expect(conduit.querySelector(".dot")!.className).toContain("hollow");
    expect(conduit.textContent).not.toMatch(/연결됨/);
  });

  it("옛 판이면 실제 판 번호로 경고하고 확장 관리를 연다", async () => {
    스위치.확장 = "옛 판";
    const user = await 나라장터열기();
    const alert = await screen.findByRole("alert");
    const conduit = screen.getByRole("region", { name: "수집 통로" });
    expect(within(conduit).getByText("연결됨 · 확장이 옛 판")).toBeTruthy();
    expect(within(conduit).getByText("옛 판으로도 저장은 되지만, 새 수집 규칙이 적용되지 않을 수 있습니다.")).toBeTruthy();
    expect(within(conduit).getByText("옛 판 · 새 판은 0.9.0")).toBeTruthy();
    expect(alert.textContent).toContain("Chrome의 확장이 옛 판(0.8.0)입니다.");
    expect(alert.textContent).toContain("새 판(0.9.0)으로 자동으로 바뀝니다");
    await user.click(within(alert).getByRole("button", { name: "Chrome 확장 관리 열기" }));
    await waitFor(() => expect(부름).toHaveBeenCalledWith("openExtensionSetup", ["chrome"]));
  });

  it("들어온 자료는 오늘 것을 늦은 것부터 세우고, 「보기」 는 그 종류 탭에서 번호로 찾는다", async () => {
    수집.줄 = 기본();
    const user = await 나라장터열기();

    const list = await screen.findByRole("region", { name: "들어온 자료" });
    expect(await within(list).findByText("오늘 3건")).toBeTruthy();
    const rows = within(list).getAllByRole("row").slice(1);
    expect(rows.map((r) => within(r).getAllByRole("cell").slice(0, 5).map((c) => c.textContent))).toEqual([
      ["09:12", "계약", "R26TA0911050100", "2026년 가람 수질측정기 조달", "저장"],
      ["08:57", "공고", "R26BK09017002-000", "26년 한별 공압식승강판 20톤 구매", "변경 없음"],
      ["08:41", "접수", "MPKPLA26910286-000", "26년한별윈치8종구매", "저장"],
    ]);
    expect(within(rows[0]).getByText("저장").className).toContain("ok-strong");
    expect(within(rows[1]).getByText("변경 없음").className).toContain("soft");
    expect(within(list).getByText(/저장된 자료만 보입니다/)).toBeTruthy();

    await user.click(within(list).getByRole("button", { name: "공고 R26BK09017002-000 보기" }));
    expect(screen.getByRole("tab", { name: /^공고/ }).getAttribute("aria-selected")).toBe("true");
    expect((await screen.findByRole("searchbox") as HTMLInputElement).value).toBe("R26BK09017002-000");
  });

  it("오늘 들어온 것이 없으면 한 줄로 말하고 마지막 수집을 덧붙인다", async () => {
    수집.줄 = [수집줄("2026-09-01T17:40:00", "계약", "R26TA0911050100", "지어낸 계약", true)];
    await 나라장터열기();
    expect(await screen.findByText(/오늘 들어온 자료가 없습니다\. 마지막 수집 09\/01 17:40 · 계약 R26TA0911050100/)).toBeTruthy();
    expect(screen.getByText("오늘 0건")).toBeTruthy();
  });

  it("「JSON 파일로 가져오기…」 는 고급을 펴고 JSON 고르는 자리로 초점을 옮긴다", async () => {
    const user = await 나라장터열기();
    await user.click(screen.getByRole("button", { name: "JSON 파일로 가져오기…" }));
    expect(screen.getByRole("button", { name: /고급 · 수집 규칙과 지난 수집 다시 보기/ }).getAttribute("aria-expanded")).toBe("true");
    const file = await screen.findByLabelText("JSON 파일 선택");
    await waitFor(() => expect(document.activeElement).toBe(file));
  });

  it("Chrome 확장 상태는 아는 것만 — 연결·판·오늘 저장·연결 기록·최근 연결·확장 폴더·저장 대상·오늘 들어온 수", async () => {
    수집.줄 = 기본();
    const user = await 나라장터열기();
    await user.click(screen.getByRole("button", { name: "Chrome 확장 상태" }));
    expect(screen.getByRole("button", { name: "Chrome 확장 상태" }).getAttribute("aria-pressed")).toBe("true");

    const summary = await screen.findByRole("region", { name: "연결 요약" });
    expect(within(summary).getByRole("heading", { name: "Chrome 확장" })).toBeTruthy();
    expect(within(summary).getByText("연결됨").className).toContain("ok");
    expect(within(summary).getByText("Chrome · 확장 0.9.0 · 09:12부터 연결됨")).toBeTruthy();
    // 검토 대기는 열린 탭이 말한다 — 가짜 다리의 탭 셋 중 하나. 오늘 오류는 연결 기록이 말한다.
    await waitFor(() => expect(within(summary).getAllByRole("definition").map((d) => d.textContent)).toEqual(["0.9.0", "2", "1", "0"]));

    // 연결 기록 · 오늘 — 늦은 것부터. 붙음은 찬 점, 끊김은 빈 점, 판 바뀜은 파란 점. 끊긴 까닭은 없다.
    const log = screen.getByRole("region", { name: "연결 기록" });
    expect(within(log).getByRole("heading", { name: "연결 기록 · 오늘" })).toBeTruthy();
    const items = within(log).getAllByRole("listitem");
    expect(items.map((li) => li.textContent)).toEqual([
      "09:12연결됨", "08:57연결 끊김", "08:31연결됨", "08:30확장이 0.9.0으로 바뀜 앱의 새 판에 맞춰 자동으로",
    ]);
    expect(items.map((li) => li.querySelector(".log-dot")!.className)).toEqual([
      "log-dot connected", "log-dot disconnected", "log-dot connected", "log-dot updated",
    ]);

    const recent = screen.getByRole("region", { name: "최근 연결" });
    expect(recent.textContent).toMatch(/오늘 09:12.*Chrome.*확장 0\.9\.0/);

    const info = screen.getByRole("complementary", { name: "확장 정보" });
    expect(info.textContent).toContain("0.9.0 · 앱과 같음");
    expect(info.textContent).toContain("판 바뀐 때오늘 08:30 · 자동");
    expect(info.textContent).toContain("C:\\Users\\홍길동\\AppData\\Local\\Pclm\\extension");
    expect(info.textContent).toContain("계약자료.pclm · 작업자료");
    expect(info.textContent).toMatch(/접수1공고1계약1/);
    expect(info.textContent).toContain("오늘 09:12 · 계약 R26TA0911050100");

    await user.click(within(info).getByRole("button", { name: "들어온 자료에서 보기" }));
    expect(await screen.findByRole("region", { name: "들어온 자료" })).toBeTruthy();
  });

  it("상태 보드는 끊겼으면 「연결 안 됨」 과 마지막 연결을, 옛 판이면 「옛 판」 을 말한다", async () => {
    스위치.확장 = "끊김";
    const user = await 나라장터열기();
    await user.click(screen.getByRole("button", { name: "Chrome 확장 상태" }));
    const summary = await screen.findByRole("region", { name: "연결 요약" });
    expect(within(summary).getByText("연결 안 됨")).toBeTruthy();
    expect(within(summary).getByText("Chrome · 마지막 연결 오늘 18:02")).toBeTruthy();
    const log = screen.getByRole("region", { name: "연결 기록" });
    expect(within(log).getAllByRole("listitem")[0].textContent).toBe("18:02연결 끊김");
    expect(log.textContent).not.toMatch(/닫|껐|꺼져|멈/);
    expect(screen.getByRole("complementary", { name: "확장 정보" }).textContent).toContain("0.9.0 · 마지막 연결 때");
    cleanup();

    스위치.확장 = "옛 판";
    const again = await 나라장터열기();
    await again.click(screen.getByRole("button", { name: "Chrome 확장 상태" }));
    const staleSummary = await screen.findByRole("region", { name: "연결 요약" });
    expect(within(staleSummary).getByText("옛 판")).toBeTruthy();
    expect(within(staleSummary).getByText("09:12부터 연결됨 · 새 판으로 바꿔야 함")).toBeTruthy();
    expect(within(screen.getByRole("region", { name: "연결 기록" })).getByRole("listitem").textContent)
      .toBe("09:12연결됨 확장 0.8.0 · 앱은 0.9.0");
  });
});

/** 설정에 있던 확장 준비·상태·고급 수집이 옮겨 온 자리. 같은 약속을 새 자리에서 붙든다. */
describe("옮겨 온 수집 도구", () => {
  it("내장 확장 준비 실패는 재시도할 수 있고 준비 후 브라우저 추가를 안내한다", async () => {
    스위치.확장 = "설치 전";
    let complete: (value: unknown) => void = () => {};
    const user = await 나라장터열기();
    const setup = await screen.findByRole("region", { name: "확장 설치" });
    const prepare = within(setup).getByRole("button", { name: "확장 준비" });
    가로채기("prepareExtension", () => Promise.reject(new Error("등록 권한을 확인하세요.")));
    await user.click(prepare);
    expect(await within(setup).findByRole("alert")).toHaveProperty("textContent", "등록 권한을 확인하세요.");
    expect(within(setup).queryByRole("button", { name: "Edge 확장 관리 열기" })).toBeNull();
    가로채기("prepareExtension", () => new Promise(resolve => { complete = resolve; }));
    await user.click(prepare);
    expect((prepare as HTMLButtonElement).disabled).toBe(true);
    const folder = "C:\\Users\\테스트 사용자\\AppData\\Local\\Pclm\\extension";
    complete({ folder, version: "0.3.0" });
    expect(await within(setup).findByText(/브라우저에서 추가를 완료하세요/)).toBeTruthy();
    // 폴더 선택 창에 바로 붙여 넣게 경로를 복사해 둔다.
    expect(await within(setup).findByText(/폴더 경로를 복사했습니다/)).toBeTruthy();
    expect(await navigator.clipboard.readText()).toBe(folder);
    expect(within(setup).getByLabelText("확장 폴더")).toHaveProperty("value", folder);
    expect(within(setup).queryByRole("alert")).toBeNull();
    가로채기("openExtensionSetup", () => Promise.resolve(null));
    for (const [target, label] of [["edge", "Edge 확장 관리 열기"], ["chrome", "Chrome 확장 관리 열기"]]) {
      await user.click(within(setup).getByRole("button", { name: label }));
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
    const user = await 나라장터열기();
    await user.click(screen.getByRole("button", { name: /고급 · 수집 규칙과 지난 수집 다시 보기/ }));
    const tools = document.getElementById("nara-advanced")!;
    const file = await within(tools).findByLabelText("JSON 파일 선택");
    await waitFor(() => expect(file.closest("fieldset")!.disabled).toBe(false));
    await user.upload(file, new File([JSON.stringify({ pointInfo: {}, tables: {} })], "공고.json", { type: "application/json" }));
    await within(tools).findByText("공고.json");
    await user.click(within(tools).getByRole("button", { name: "활성 매핑으로 미리보기" }));
    const save = await within(tools).findByRole("button", { name: "검토한 자료 저장" });
    expect((save as HTMLButtonElement).disabled).toBe(true);
    await user.selectOptions(within(tools).getByLabelText("title 선택"), "keep");
    await user.click(save);
    await waitFor(() => expect(부름.mock.calls.some(([m, args]) => m === "erpTools" && args[0] === "capture")).toBe(true));
    const args = 부름.mock.calls.find(([m, args]) => m === "erpTools" && args[0] === "capture")![1];
    expect(JSON.parse(String(args[1])).choices).toEqual({ "header/title": "keep" });
    expect(await within(tools).findByText("검토한 자료를 저장했습니다.")).toBeTruthy();
  });

  /**
   * 받는 화면은 화면 번호로만 가린다(ADR-037). 이력에 걸러 담은 자료에는 메뉴 경로가 없어, 재검토는 이력에 적힌 번호를
   * 싣는다. 번호를 적기 전의 이력은 어느 화면의 것인지 알 수 없어 부르지 않는다.
   */
  it("수집 이력의 재검토는 이력의 화면 번호를 싣고, 번호가 없는 옛 이력은 원본 JSON 파일을 열라고 한다", async () => {
    const 이력 = (id: string, extra: object) => ({ capture_id: id, entity_base: "R26BK00000001", entity_seq: id === "old" ? "001" : "002",
      snapshot_json: JSON.stringify({ profile: "g2b-notice-a-v1", data: { pointInfo: { bidPbancNo: "R26BK00000001" }, tables: {} }, scope: "live", ...extra }) });
    부름.mockImplementation(async (m, args) => {
      if (m !== "erpTools" || args[0] === "list") return 본디(m, args);
      if (args[0] === "references") return [];
      if (args[0] === "history") return [이력("old", {}), 이력("new", { screen: "01179" })];
      if (args[0] === "saveMapping") return "revision";
      if (args[0] === "inspect") return { entity: "R26BK00000001-002", itemCount: 0, baseToken: "token", unmatched: [], changes: [] };
      throw new Error("예상하지 않은 작업");
    });
    const user = await 나라장터열기();
    await user.click(screen.getByRole("button", { name: /고급 · 수집 규칙과 지난 수집 다시 보기/ }));
    const tools = document.getElementById("nara-advanced")!;
    const check = await within(tools).findByRole("button", { name: "연결·이력 확인" });
    await waitFor(() => expect((check as HTMLButtonElement).disabled).toBe(false));
    (within(tools).getByText("연결 상태·수집 이력").parentElement as HTMLDetailsElement).open = true;
    await user.click(check);
    const [old, recent] = await within(tools).findAllByRole("button", { name: "현재 매핑으로 재검토" });

    await user.click(old);
    expect(await within(tools).findByRole("alert")).toHaveProperty("textContent",
      "이 수집 이력에는 화면 번호가 없어 다시 검토할 수 없습니다. 원본 JSON 파일을 여세요.");
    expect(within(tools).getByText("선택한 자료 없음")).toBeTruthy();

    await user.click(recent);
    await within(tools).findByText("허용 필드 수집 이력");
    await user.click(within(tools).getByRole("button", { name: "활성 매핑으로 미리보기" }));
    await waitFor(() => expect(부름.mock.calls.some(([m, args]) => m === "erpTools" && args[0] === "inspect")).toBe(true));
    const sent = JSON.parse(String(부름.mock.calls.find(([m, args]) => m === "erpTools" && args[0] === "inspect")![1][1]));
    expect(sent).toMatchObject({ profile: "g2b-notice-a-v1", scope: "file", screen: "01179" });
  });

  it("확장 상태는 연결이 없거나, 같은 판이 돌거나, 옛 판이 돌고 있음을 한 줄로 보인다", async () => {
    const 상태 = (contacts: { browser: string; version: string; at: string }[], live: ExtensionStatus["live"] = []) => {
      스위치.확장상태 = { prepared: true, embeddedVersion: "0.4.1", diskVersion: "0.4.1", contacts, folder: "C:\\확장", live, log: [], errors: [], pastError: null };
    };

    상태([]);
    await 나라장터열기();
    expect(await screen.findByText("Chrome 연결을 기다리는 중")).toBeTruthy();
    expect(screen.getByText("개발자 모드")).toBeTruthy();
    expect(screen.getByLabelText("확장 폴더")).toHaveProperty("value", "C:\\확장");
    cleanup();

    상태([{ browser: "Edge", version: "0.4.1", at: "2026-09-28T14:02:37" }]);
    await 나라장터열기();
    const conduit = await screen.findByRole("region", { name: "수집 통로" });
    expect(within(conduit).getByText("Edge · 연결 안 됨")).toBeTruthy();
    expect(within(conduit).getByText("확장 0.4.1")).toBeTruthy();
    expect(within(conduit).getByText("마지막 연결 09/28 14:02")).toBeTruthy();
    expect(within(conduit).getByText("Edge가 꺼져 있거나 확장이 꺼져 있습니다. Edge를 켜면 자동으로 다시 연결됩니다.")).toBeTruthy();
    expect(screen.getByRole("tab", { name: /^나라장터/ }).textContent).toContain("연결 안 됨");
    expect(screen.queryByText("Chrome 연결을 기다리는 중")).toBeNull();
    cleanup();

    상태([{ browser: "Chrome", version: "0.4.0", at: "2026-09-27T09:00:00" }],
      [{ browser: "Chrome", version: "0.4.0", connectedAt: "2026-09-27T09:00:00" }]);
    await 나라장터열기();
    expect((await screen.findByRole("alert")).textContent).toContain(
      "Chrome의 확장이 옛 판(0.4.0)입니다. 나라장터 화면을 열면 새 판(0.4.1)으로 자동으로 바뀝니다. 바뀌지 않으면 확장 관리에서 새로고침하세요.");
  });
});

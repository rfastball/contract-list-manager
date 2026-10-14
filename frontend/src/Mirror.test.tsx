import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { invoke, 기본수집화면, 기본탭, 수집, 스위치, 지금화면 } from "./mock";

/**
 * 지금 보는 화면(ADR-036) — 셋째 기둥. Chrome 에서 보고 있는 나라장터 화면이 작업자료에서 무엇이 되는지를 비추고,
 * 확장의 저장과 같은 절차로 가져온다. 탭·번호·값은 모두 가짜 다리가 지어낸 것이다.
 */
vi.mock("./mock", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./mock")>();
  return { ...actual, invoke: vi.fn(actual.invoke) };
});

const 부름 = vi.mocked(invoke);
const 본디 = 부름.getMockImplementation()!;
const 길게 = { timeout: 3000 };

beforeAll(() => {
  Element.prototype.scrollIntoView = () => {};
  HTMLDialogElement.prototype.showModal = function () { this.open = true; };
  HTMLDialogElement.prototype.close = function () { this.open = false; };
});

beforeEach(() => {
  localStorage.clear();
  부름.mockClear();
  부름.mockImplementation(본디);
  스위치.열람 = false;
  스위치.확장 = "연결";
  스위치.확장상태 = null;
  수집.판 = 0;
  수집.줄 = [];
  지금화면.탭 = 기본탭();
  지금화면.수집화면 = 기본수집화면();
  지금화면.앞 = "4242:1";
  지금화면.낡음 = false;
  지금화면.사라짐 = false;
  지금화면.명령 = [];
  지금화면.가져옴 = [];
  지금화면.설정 = { pid: 4242, browser: "Chrome", siteAccess: true };
});

afterEach(() => {
  cleanup();
});

const 기둥 = () => screen.getByRole("region", { name: "지금 보는 화면" });
const 상태줄 = () => 기둥().querySelector(".dock-live")!.textContent;
const 비추는번호 = () => 기둥().querySelector(".dock-no")?.textContent ?? "";

async function 열기() {
  const user = userEvent.setup();
  render(<App />);
  await waitFor(() => expect(비추는번호()).toBe("R26TA0000010101"), 길게);
  return user;
}

describe("레일", () => {
  it("연결되지 않았으면 레일로 접히고, 누르면 까닭을 펼치며, 접기로 돌아간다", async () => {
    스위치.확장 = "끊김";
    const user = userEvent.setup();
    render(<App />);
    const rail = await screen.findByRole("button", { name: "지금 보는 화면 — 연결 안 됨, 펼치기" });
    expect(rail.getAttribute("aria-expanded")).toBe("false");
    await user.click(rail);
    expect(기둥().textContent).toContain("Chrome이 연결되어 있지 않아 지금 보는 화면을 알 수 없습니다.");
    expect(기둥().textContent).toContain("Chrome을 켜면 자동으로 다시 연결되고");
    await user.click(within(기둥()).getByRole("button", { name: "접기" }));
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "지금 보는 화면 — 연결 안 됨, 펼치기" }));
    // 저절로 접히는 자리에서 접은 것은 남기지 않는다 — 남기면 연결된 뒤에도 접힌 채로 있다.
    expect(localStorage.getItem("pclm.dock.folded")).toBeNull();
    expect(부름.mock.calls.some(([m]) => m === "mirror")).toBe(false); // 붙어 있지 않으면 읽지 않는다.
  });

  it("붙어 있어도 열린 탭이 없으면 접혀 있다가, 탭이 들어오면 저절로 펼친다", async () => {
    지금화면.탭 = [];
    지금화면.앞 = null;
    render(<App />);
    const rail = await screen.findByRole("button", { name: "지금 보는 화면 — 열린 탭 없음, 펼치기" }, 길게);
    expect(rail.querySelector(".live-dot")!.className).toContain("on");
    // 레일에는 글이 없다 — 세로로 세우면 한 글자씩 끊겨 읽히지 않는다. 이름과 상태는 단추의 이름에 있다.
    expect(rail.textContent).toBe("");
    // 건너뛰기 길은 접혀 있어도 이 단추에 닿는다.
    fireEvent.click(screen.getByRole("link", { name: "지금 보는 화면으로 건너뛰기" }));
    expect(document.activeElement).toBe(rail);
    expect(document.getElementById("workspace")!.hidden).toBe(false);
    지금화면.탭 = 기본탭();
    지금화면.앞 = "4242:1";
    await waitFor(() => expect(비추는번호()).toBe("R26TA0000010101"), 길게);
    // 탭이 다시 사라지면 다시 접힌다.
    지금화면.탭 = [];
    지금화면.앞 = null;
    expect(await screen.findByRole("button", { name: "지금 보는 화면 — 열린 탭 없음, 펼치기" }, 길게)).toBeTruthy();
  });

  it("사람이 접으면 탭이 새로 들어와도 접힌 채로 있고, 다시 켜도 남으며, 레일을 누르면 펼친다", async () => {
    const user = await 열기();
    await user.click(within(기둥()).getByRole("button", { name: "접기" }));
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "지금 보는 화면 — 탭 3개, 펼치기" }));
    expect(localStorage.getItem("pclm.dock.folded")).toBe("1");

    const [first] = 기본탭();
    지금화면.탭 = [...기본탭(), { ...first, id: "4242:4", tabId: 4 }];
    지금화면.앞 = "4242:4";
    expect(await screen.findByRole("button", { name: "지금 보는 화면 — 탭 4개, 펼치기" }, 길게)).toBeTruthy();
    cleanup();

    render(<App />);
    expect(await screen.findByRole("button", { name: "지금 보는 화면 — 탭 4개, 펼치기" }, 길게)).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "지금 보는 화면 — 탭 4개, 펼치기" }));
    expect(document.activeElement).toBe(screen.getByRole("heading", { name: "지금 보는 화면", level: 2 }));
    expect(localStorage.getItem("pclm.dock.folded")).toBeNull();
    expect(within(기둥()).getByRole("group", { name: "Chrome의 나라장터 탭 4개" })).toBeTruthy();
  });

  it("저장소를 쓸 수 없어도 접고 펼친다", async () => {
    const 막힘 = () => { throw new Error("저장소 막힘"); };
    const spies = [
      vi.spyOn(Storage.prototype, "getItem").mockImplementation(막힘),
      vi.spyOn(Storage.prototype, "setItem").mockImplementation(막힘),
      vi.spyOn(Storage.prototype, "removeItem").mockImplementation(막힘),
    ];
    try {
      const user = await 열기();
      await user.click(within(기둥()).getByRole("button", { name: "접기" }));
      await user.click(screen.getByRole("button", { name: "지금 보는 화면 — 탭 3개, 펼치기" }));
      expect(비추는번호()).toBe("R26TA0000010101");
    } finally {
      spies.forEach((s) => s.mockRestore());
    }
  });

  it("준비 전이면 「확장 준비 전」, 열람 창이면 「열람 창」 이고 열람 창은 탭을 읽지 않는다", async () => {
    스위치.확장 = "설치 전";
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "지금 보는 화면 — 확장 준비 전, 펼치기" }));
    expect(기둥().textContent).toContain("확장을 준비하면 Chrome에서 보고 있는 나라장터 화면이 여기에 나타납니다.");
    cleanup();

    스위치.확장 = "연결";
    스위치.열람 = true;
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "지금 보는 화면 — 열람 창, 펼치기" }));
    expect(기둥().textContent).toContain("열람 창에서는 지금 보는 화면을 쓸 수 없습니다.");
    await new Promise((r) => setTimeout(r, 300));
    expect(부름.mock.calls.some(([m]) => m === "mirror" || m === "importShot")).toBe(false);
  });

  it("맨 앞에 지금 보는 화면으로 건너뛰는 길이 있다", async () => {
    await 열기();
    const skip = screen.getByRole("link", { name: "지금 보는 화면으로 건너뛰기" });
    fireEvent.click(skip);
    expect(document.activeElement).toBe(screen.getByRole("heading", { name: "지금 보는 화면", level: 2 }));
  });
});

describe("탭과 따라가기", () => {
  it("탭마다 화면·번호·상태를 글로 달고, 앞 탭과 비추는 탭을 가른다", async () => {
    await 열기();
    const group = within(기둥()).getByRole("group", { name: "Chrome의 나라장터 탭 3개" });
    const tabs = within(group).getAllByRole("button");
    expect(tabs.map((b) => b.getAttribute("aria-label"))).toEqual([
      "계약 상세 R26TA0000010101, 새 차수, Chrome에서 앞에 있는 탭, 지금 보고 있는 탭",
      "계약 상세 R26TA0000010201, 검토 대기",
      "개찰 결과, 수집 안 함",
    ]);
    expect(tabs.map((b) => b.getAttribute("aria-pressed"))).toEqual(["true", "false", "false"]);
    expect(상태줄()).toBe("Chrome에서 보는 탭을 따라갑니다");
    // 검토 대기가 다른 탭에 있으면 머리에 그 수와 가는 길이 선다.
    expect(within(기둥()).getByRole("button", { name: "검토 대기 1건 보기" })).toBeTruthy();
  });

  it("따라가면 Chrome 의 앞 탭을 비추고, 사람이 기둥 안에 있는 동안은 멈췄다가 떠나면 따라잡는다", async () => {
    await 열기();
    지금화면.앞 = "4242:2";
    await waitFor(() => expect(비추는번호()).toBe("R26TA0000010201"), 길게);

    fireEvent.mouseEnter(기둥());
    지금화면.앞 = "4242:3";
    await waitFor(() => expect(상태줄()).toBe("살펴보는 동안은 Chrome에서 탭을 옮겨도 따라가지 않습니다"), 길게);
    await new Promise((r) => setTimeout(r, 1200));
    expect(비추는번호()).toBe("R26TA0000010201");

    fireEvent.mouseLeave(기둥());
    await waitFor(() => expect(within(기둥()).getByRole("heading", { name: "이 화면은 수집하지 않습니다" })).toBeTruthy(), 길게);
    expect(상태줄()).toBe("Chrome에서 보는 탭을 따라갑니다");
  });

  it("Chrome 의 앞 탭이 나라장터가 아니면 마지막 탭에 머물고 그 까닭을 말한다", async () => {
    await 열기();
    지금화면.앞 = null;
    await waitFor(() => expect(상태줄()).toBe("Chrome 앞 탭이 나라장터가 아니어서 마지막으로 본 나라장터 탭에 머뭅니다"), 길게);
    expect(비추는번호()).toBe("R26TA0000010101");
  });

  it("탭을 누르면 그 탭에 고정하고, 따라가기를 다시 켜면 앞 탭으로 돌아간다", async () => {
    const user = await 열기();
    await user.click(within(기둥()).getByRole("button", { name: /^계약 상세 R26TA0000010201/ }));
    expect(비추는번호()).toBe("R26TA0000010201");
    const follow = within(기둥()).getByRole("button", { name: "Chrome 따라가기" });
    expect(follow.getAttribute("aria-pressed")).toBe("false");
    fireEvent.mouseLeave(기둥());
    (document.activeElement as HTMLElement).blur();
    await waitFor(() => expect(상태줄()).toBe("이 탭에 고정 · Chrome에서는 다른 탭을 보는 중"));
    지금화면.앞 = "4242:3";
    await new Promise((r) => setTimeout(r, 1300));
    expect(비추는번호()).toBe("R26TA0000010201"); // 고정한 탭은 앞 탭을 따라가지 않는다.

    await user.click(follow);
    expect(follow.getAttribute("aria-pressed")).toBe("true");
    await waitFor(() => expect(within(기둥()).getByRole("heading", { name: "이 화면은 수집하지 않습니다" })).toBeTruthy());
  });

  it("수집 규칙에 없는 화면은 앱이 준 수집 화면을 안내하고, 내보내기는 확장의 그 기능을 부른다", async () => {
    const user = await 열기();
    await user.click(within(기둥()).getByRole("button", { name: /^개찰 결과/ }));
    const section = within(기둥()).getByRole("region", { name: "수집하지 않는 화면" });
    expect(section.textContent).toContain("아래 화면을 열면 가져올 수 있습니다.");
    // 메뉴 번호를 알면 곁에 보인다(ADR-037).
    expect(section.querySelector(".dock-menu")!.textContent).toBe("화면 번호 01175");
    expect([...section.querySelectorAll(".dock-rules dt, .dock-rules dd")].map((e) => e.textContent)).toEqual(
      ["접수", "조달요구 접수(목록형)", "공고", "입찰공고 상세", "공고", "입찰공고상세조회(통합)", "계약", "계약 상세", "계약", "물품·일반용역 변경계약서 작성"]);
    await user.click(within(section).getByRole("button", { name: "현재 화면 엑셀로 내보내기" }));
    await waitFor(() => expect(지금화면.명령).toContainEqual({ tabId: 3, command: "export" }));
    expect(기둥().querySelector(".dock-foot")).toBeNull(); // 가져올 것이 없다.
  });

  it("앱이 부른 내보내기는 요청했다고만 말하고, 확장이 돌려준 결과가 와야 성공·실패를 말한다", async () => {
    const user = await 열기();
    await user.click(within(기둥()).getByRole("button", { name: /^개찰 결과/ }));
    const section = within(기둥()).getByRole("region", { name: "수집하지 않는 화면" });
    const 결과 = (export_: { at: number; ok: boolean; message: string }) =>
      지금화면.탭 = 지금화면.탭.map((t) => t.tabId === 3 ? { ...t, export: export_ } : t);

    // 실패 — 확장이 준 까닭을 오류로 띄우고 내려받았다고 말하지 않는다.
    await user.click(within(section).getByRole("button", { name: "현재 화면 엑셀로 내보내기" }));
    expect(await screen.findByText("Chrome에 내보내기를 요청했습니다")).toBeTruthy();
    expect(screen.queryByText(/내려받/)).toBeNull();
    결과({ at: 1000, ok: false, message: "표를 찾지 못했습니다." });
    expect(await screen.findByText("표를 찾지 못했습니다.", undefined, 길게)).toBeTruthy();
    expect(screen.queryByText(/내려받기를 시작했습니다/)).toBeNull();

    // 성공 — 요청할 때 알던 결과보다 늦은 것만 이 요청의 답이다.
    await user.click(within(section).getByRole("button", { name: "현재 화면 엑셀로 내보내기" }));
    expect(await screen.findByText("Chrome에 내보내기를 요청했습니다")).toBeTruthy();
    결과({ at: 2000, ok: true, message: "" });
    expect(await screen.findByText("Chrome에서 내려받기를 시작했습니다", undefined, 길게)).toBeTruthy();
  });

  it("수집 화면의 안내는 창에 적어 두지 않고 앱이 매핑에서 읽어 준 목록을 그대로 그린다", async () => {
    지금화면.수집화면 = [{ code: "01179", entityType: "notice", kind: "공고", name: "지어낸 공고 화면" }];
    const user = await 열기();
    await user.click(within(기둥()).getByRole("button", { name: /^개찰 결과/ }));
    const section = within(기둥()).getByRole("region", { name: "수집하지 않는 화면" });
    expect([...section.querySelectorAll(".dock-rules dt, .dock-rules dd")].map((e) => e.textContent)).toEqual(["공고", "지어낸 공고 화면"]);
    expect(section.textContent).not.toContain("계약 상세");
  });
});

describe("비춘 것", () => {
  it("새 차수는 견준 차수와 다른 칸을 먼저 세우고 같은 칸은 접으며, 품목·자리·차수를 계약면 표기 그대로 보인다", async () => {
    const user = await 열기();
    const id = within(기둥()).getByRole("region", { name: "이 화면의 자료" });
    expect(id.textContent).toContain("새 차수");
    expect(id.textContent).toContain("시험 음향설비 개선");
    expect(id.textContent).toContain("작업자료에는 차수 00까지 있습니다. 차수 00과 다른 칸 2개, 품목 1행.");
    expect(id.textContent).toContain("09:20에 읽음");

    // 가져오면 들어갈 것 — 바뀐 칸 둘과 바뀐 품목 한 행. 남겨 둘 것이 없으면 그 구역은 서지 않는다.
    const fields = within(기둥()).getByRole("region", { name: "가져오면 들어갈 것" });
    expect(within(fields).getByRole("heading", { name: "가져오면 들어갈 것 3" })).toBeTruthy();
    expect(fields.textContent).toContain("계약번호 R26TA0000010101 → 계약본번호 R26TA00000101 · 차수 01");
    const rows = [...fields.querySelectorAll(".dock-field")].map((r) => r.textContent);
    expect(rows).toEqual(["계약일자2026/10/022026/09/14화면 표기 2026-10-02", "계약금액34,400,00030,400,000"]);
    expect([...fields.querySelectorAll(".dock-item")].map((r) => r.textContent)).toEqual(
      ["2무선마이크 세트 8세트 × 2,000,000차수 00은 6세트 · 12,000,00016,000,000"]);
    expect(within(기둥()).queryByRole("region", { name: "남겨 둘 것" })).toBeNull();
    const same = within(fields).getByRole("button", { name: "같은 칸 6개" });
    expect(same.getAttribute("aria-expanded")).toBe("false");
    await user.click(same);
    expect(fields.textContent).toContain("대표자홍길동");
    expect(fields.textContent).toContain("나머지 19열 · 이 화면에서는 읽지 않음");

    // 품목 전체는 접혀 있다가 펴면 모두 보인다.
    const items = within(기둥()).getByRole("region", { name: "품목" });
    const all = within(items).getByRole("button", { name: "품목 전체 2행" });
    expect(all.getAttribute("aria-expanded")).toBe("false");
    expect(items.querySelectorAll(".dock-item")).toHaveLength(0);
    await user.click(all);
    expect(items.textContent).toContain("화면의 품목 표 2행을 모두 읽었습니다");
    expect([...items.querySelectorAll(".dock-item .clip")].map((e) => e.textContent!.trim())).toEqual(["디지털 믹서", "무선마이크 세트"]);

    const place = within(기둥()).getByRole("region", { name: "작업자료에서의 자리" });
    expect(place.textContent).toContain("접수아직 수집되지 않음");
    expect(place.textContent).toContain("공고참조 R26BK00000101 · 아직 수집되지 않음");
    expect(place.textContent).toContain("차수 0030,400,00009/16 저장");
    expect(place.textContent).toContain("차수 0134,400,000가져오면 여기에 추가됩니다");

    const foot = 기둥().querySelector(".dock-foot")!;
    expect(foot.textContent).toContain("가져오면 차수 01이 새로 추가됩니다");
    expect(foot.textContent).toContain("차수 00은 그대로 남습니다.");
    // 가져오는 길은 이 단추 하나다 — 다른 길을 안내하지 않는다.
    expect(foot.textContent).not.toMatch(/단축키|수집기/);
    expect(within(foot as HTMLElement).getByRole("button", { name: "R26TA0000010101 가져오기" })).toBeTruthy();
  });

  it("사람이 고친 칸은 그대로 두는 것이 기본이고, 복원을 고르면 단추가 지울 수를 말하며 따라가기를 멈춘다", async () => {
    // 이 시험은 덮개만 본다 — 화면에 없는 품목 행은 아래에서.
    지금화면.탭 = 기본탭().map((t) => t.tabId === 2 ? { ...t, shot: { ...t.shot!, missing: [], totalsWithDefaults: true, totalsWithoutMissing: true } } : t);
    지금화면.앞 = "4242:2"; // Chrome 에서 그 탭을 보고 있다 — 따라가는 채로 고른다.
    const user = userEvent.setup();
    render(<App />);
    await waitFor(() => expect(비추는번호()).toBe("R26TA0000010201"), 길게);
    expect(within(기둥()).queryByRole("button", { name: /검토 대기 \d건 보기/ })).toBeNull(); // 비추는 탭이 그것이다.
    const incoming = within(기둥()).getByRole("region", { name: "가져오면 들어갈 것" });
    expect(within(incoming).getByRole("heading", { name: "가져오면 들어갈 것 0" })).toBeTruthy();
    expect(incoming.textContent).toContain("작업자료와 같습니다");
    const kept = within(기둥()).getByRole("region", { name: "남겨 둘 것" });
    expect(within(kept).getByRole("heading", { name: "남겨 둘 것 · 고르면 지움 3" })).toBeTruthy();
    expect(kept.textContent).toContain("품목 칸 1개는 화면에서 비어 있어 그대로 둡니다");
    const foot = () => 기둥().querySelector(".dock-foot") as HTMLElement;
    expect(foot().textContent).toContain("작업자료와 같습니다");
    expect(foot().textContent).toContain("4곳은 그대로 둡니다.");
    expect(within(foot()).getByRole("button", { name: "R26TA0000010201 가져오기" })).toBeTruthy();

    const restore = within(kept).getByRole("button", { name: "납품장소 — 수집 원값으로 복원" });
    expect(restore.getAttribute("aria-pressed")).toBe("false");
    expect(kept.textContent).toContain("시험동 B1 기계실화면 시험동 지하 기계실");
    await user.click(restore);
    expect(restore.getAttribute("aria-pressed")).toBe("true");
    expect(kept.textContent).toContain("시험동 B1 기계실 지움");
    expect(foot().querySelector(".danger")!.textContent).toBe("1곳을 지웁니다. 지운 값은 되돌릴 수 없습니다.");
    expect(상태줄()).toBe("고른 칸이 있어 Chrome에서 탭을 옮겨도 따라가지 않습니다");

    await user.click(within(foot()).getByRole("button", { name: "1곳을 지우고 R26TA0000010201 가져오기" }));
    await waitFor(() => expect(foot().textContent).toMatch(/저장했습니다 · \d\d:\d\d/));
    expect(지금화면.가져옴).toEqual([{ tabId: 2, baseToken: "mock-검토", picks: ["override/납품장소"], captureId: expect.stringMatching(/^[0-9a-f-]{36}$/) }]);
    expect(foot().textContent).toContain("「들어온 자료」와 계약 목록에서 볼 수 있습니다.");
    expect(within(기둥()).getByRole("status").textContent).toMatch(/^계약 R26TA0000010201 · 저장했습니다 · \d\d:\d\d$/);
    await waitFor(() => expect(within(기둥()).getByRole("region", { name: "이 화면의 자료" }).textContent)
      .toContain("고친 값 1개를 지우고 나머지 1개는 그대로 두고 저장했습니다. 수집 원값은 따로 남아 있습니다."), 길게);
    // 들어온 자료로 가는 길.
    await user.click(within(foot()).getByRole("button", { name: "들어온 자료에서 보기" }));
    expect(await screen.findByRole("heading", { name: "나라장터", level: 1 })).toBeTruthy();
    // 나라장터 화면에서 「확장 상태」 를 보던 중이어도 들어온 자료로 돌아선다.
    const 보기 = within(screen.getByRole("group", { name: "나라장터 보기" }));
    await user.click(보기.getByRole("button", { name: "Chrome 확장 상태" }));
    expect(보기.getByRole("button", { name: "들어온 자료" }).getAttribute("aria-pressed")).toBe("false");
    await user.click(within(foot()).getByRole("button", { name: "들어온 자료에서 보기" }));
    expect(보기.getByRole("button", { name: "들어온 자료" }).getAttribute("aria-pressed")).toBe("true");
  });

  it("가져오는 사이 화면이나 작업자료가 바뀌었으면 쓰지 않고 다시 읽는다고 말한다", async () => {
    const user = await 열기();
    지금화면.낡음 = true;
    await user.click(within(기둥()).getByRole("button", { name: "R26TA0000010101 가져오기" }));
    await waitFor(() => expect(상태줄()).toBe("화면이나 작업자료가 바뀌었습니다. 다시 읽습니다."));
    expect(지금화면.가져옴).toHaveLength(1);
    expect(기둥().querySelector(".dock-foot")!.textContent).not.toContain("저장했습니다");
    // 다시 누르면 새 요청이다 — 낡은 것은 끝난 답이다.
    지금화면.낡음 = false;
    await user.click(within(기둥()).getByRole("button", { name: "R26TA0000010101 가져오기" }));
    await waitFor(() => expect(지금화면.가져옴).toHaveLength(2));
    expect(지금화면.가져옴[1].captureId).not.toBe(지금화면.가져옴[0].captureId);
  });

  it("가져오려는 탭이 닫혔거나 끊겼으면 다시 읽는다고 하지 않고, 화면을 다시 열라는 오류로 알린다", async () => {
    const user = await 열기();
    지금화면.사라짐 = true;
    const 명령 = 지금화면.명령.length;
    await user.click(within(기둥()).getByRole("button", { name: "R26TA0000010101 가져오기" }));
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toBe("이 탭이 닫혔거나 브라우저 연결이 끊겨 가져오지 않았습니다. 나라장터에서 화면을 다시 여세요.");
    expect(상태줄()).not.toContain("다시 읽습니다");
    expect(지금화면.명령.slice(명령).some((c) => c.command === "read")).toBe(false);
    expect(기둥().querySelector(".dock-foot")!.textContent).not.toContain("저장했습니다");
  });

  it("화면이 비워 보낸 칸은 있던 값을 두는 것이 기본이고, 「비우기」 를 고르면 그 자리를 보낸다", async () => {
    지금화면.탭 = 기본탭().map((t) => t.tabId === 2 ? { ...t, shot: { ...t.shot!, missing: [], totalsWithDefaults: true, totalsWithoutMissing: true } } : t);
    지금화면.앞 = "4242:2";
    const user = userEvent.setup();
    render(<App />);
    await waitFor(() => expect(비추는번호()).toBe("R26TA0000010201"), 길게);
    const kept = within(기둥()).getByRole("region", { name: "남겨 둘 것" });
    const clear = within(kept).getByRole("button", { name: "납품기한 — 비우기" });
    expect(clear.getAttribute("aria-pressed")).toBe("false");
    expect(kept.textContent).toContain("납품기한2026/11/30화면에서 빈 칸");
    await user.click(clear);
    expect(clear.getAttribute("aria-pressed")).toBe("true");
    expect(kept.textContent).toContain("2026/11/30 지움");
    const foot = 기둥().querySelector(".dock-foot") as HTMLElement;
    await user.click(within(foot).getByRole("button", { name: "1곳을 지우고 R26TA0000010201 가져오기" }));
    await waitFor(() => expect(지금화면.가져옴).toHaveLength(1));
    expect(지금화면.가져옴[0].picks).toEqual(["contract/R26TA00000102-01/delivery_due"]);
  });

  it("가져오기가 실패한 뒤 같은 선택으로 다시 누르면 같은 수집 ID 로, 고른 것을 바꿨으면 새 ID 로 보낸다", async () => {
    지금화면.탭 = 기본탭().map((t) => t.tabId === 2 ? { ...t, shot: { ...t.shot!, missing: [], totalsWithDefaults: true, totalsWithoutMissing: true } } : t);
    지금화면.앞 = "4242:2";
    const 보낸 = [] as string[];
    부름.mockImplementation((m, args) => {
      if (m !== "importShot") return 본디(m, args);
      보낸.push(String(args[4]));
      return 보낸.length < 3 ? Promise.reject(new Error("작업자료를 잠시 쓸 수 없습니다.")) : 본디(m, args);
    });
    const user = userEvent.setup();
    render(<App />);
    await waitFor(() => expect(비추는번호()).toBe("R26TA0000010201"), 길게);
    const foot = () => 기둥().querySelector(".dock-foot") as HTMLElement;
    const 단추 = (name: string) => within(foot()).getByRole("button", { name }) as HTMLButtonElement;
    await user.click(단추("R26TA0000010201 가져오기"));
    await waitFor(() => expect(보낸).toHaveLength(1));
    await waitFor(() => expect(단추("R26TA0000010201 가져오기").disabled).toBe(false));
    // 같은 선택으로 다시 — 같은 ID 다.
    await user.click(단추("R26TA0000010201 가져오기"));
    await waitFor(() => expect(보낸).toHaveLength(2));
    expect(보낸[1]).toBe(보낸[0]);
    await waitFor(() => expect(단추("R26TA0000010201 가져오기").disabled).toBe(false));
    // 지울 칸을 고른 뒤에는 처음 선택이 아니라 지금 선택으로 — 새 ID 다.
    await user.click(within(within(기둥()).getByRole("region", { name: "남겨 둘 것" })).getByRole("button", { name: "납품기한 — 비우기" }));
    await user.click(단추("1곳을 지우고 R26TA0000010201 가져오기"));
    await waitFor(() => expect(지금화면.가져옴).toHaveLength(1));
    expect(보낸[2]).not.toBe(보낸[0]);
    expect(지금화면.가져옴[0]).toMatchObject({ captureId: 보낸[2], picks: ["contract/R26TA00000102-01/delivery_due"] });
  });

  it("화면에 없는 행은 그대로 두는 것이 기본이고, 계약 총액이 맞지 않으면 그렇다고 말하며 모두 지우는 길을 낸다", async () => {
    지금화면.앞 = "4242:2";
    const user = userEvent.setup();
    render(<App />);
    await waitFor(() => expect(비추는번호()).toBe("R26TA0000010201"), 길게);
    const kept = within(기둥()).getByRole("region", { name: "남겨 둘 것" });
    expect(kept.textContent).toContain("품목 2배기 덕트 보강 · 2,400,000화면에 없음");
    const row = within(kept).getByRole("button", { name: "품목 2 — 지우기" });
    expect(row.getAttribute("aria-pressed")).toBe("false");
    const foot = () => 기둥().querySelector(".dock-foot") as HTMLElement;
    // 남겨 두면 저장이 총액으로 거절한다 — 앱이 미리 셈해 준 그대로 말하고 단추를 막는다.
    expect(foot().textContent).toContain("화면에 없는 품목 행을 남겨 두면 품목 금액의 합이 계약금액과 달라 가져올 수 없습니다.");
    expect((within(foot()).getByRole("button", { name: "R26TA0000010201 가져오기" }) as HTMLButtonElement).disabled).toBe(true);

    await user.click(within(foot()).getByRole("button", { name: "화면에 없는 품목 행 모두 지우기" }));
    expect(row.getAttribute("aria-pressed")).toBe("true");
    expect(kept.textContent).toContain("배기 덕트 보강 · 2,400,000 지움");
    expect(foot().textContent).not.toContain("계약금액과 달라");
    expect(foot().querySelector(".danger")!.textContent).toBe("1곳을 지웁니다. 지운 값은 되돌릴 수 없습니다.");
    expect((within(foot()).getByRole("button", { name: "1곳을 지우고 R26TA0000010201 가져오기" }) as HTMLButtonElement).disabled).toBe(false);

    // 다시 남겨 두면 경고가 돌아온다.
    await user.click(row);
    expect(foot().textContent).toContain("계약금액과 달라");
    await user.click(row);
    await user.click(within(foot()).getByRole("button", { name: "1곳을 지우고 R26TA0000010201 가져오기" }));
    await waitFor(() => expect(지금화면.가져옴).toHaveLength(1));
    expect(지금화면.가져옴[0].picks).toEqual(["remove/contract_item/2"]);
  });

  it("화면에 없는 행을 지워도 총액이 맞지 않으면 모두 지우는 길을 내지 않는다", async () => {
    지금화면.탭 = 기본탭().map((t) => t.tabId === 2 ? { ...t, shot: { ...t.shot!, totalsWithoutMissing: false } } : t);
    지금화면.앞 = "4242:2";
    render(<App />);
    await waitFor(() => expect(비추는번호()).toBe("R26TA0000010201"), 길게);
    const foot = 기둥().querySelector(".dock-foot") as HTMLElement;
    expect(foot.textContent).toContain("품목 금액의 합이 계약금액과 달라 이대로는 가져올 수 없습니다. 화면과 작업자료의 품목을 확인하세요.");
    expect(within(foot).queryByRole("button", { name: "화면에 없는 품목 행 모두 지우기" })).toBeNull();
  });

  it("읽는 동안에는 바탕이 머물고 점이 숨쉬며, 가져오기를 막는다", async () => {
    지금화면.탭 = 기본탭().map((t) => t.tabId === 1 ? { ...t, state: "reading" as const } : t);
    render(<App />);
    await waitFor(() => expect(상태줄()).toBe("화면을 읽는 중"), 길게);
    expect(기둥().querySelector(".live-dot")!.className).toContain("reading");
    expect(within(기둥()).getByRole("region", { name: "이 화면의 자료" }).className).toContain("reading");
    expect((within(기둥()).getByRole("button", { name: "R26TA0000010101 가져오기" }) as HTMLButtonElement).disabled).toBe(true);
  });
});

describe("확장 상태 보드", () => {
  it("열린 나라장터 탭을 화면·번호·상태로 늘어놓고, 지금 보는 화면에서 보거나 그 탭으로 간다", async () => {
    const user = await 열기();
    await user.click(screen.getByRole("tab", { name: /^나라장터/ }));
    await user.click(await screen.findByRole("button", { name: "Chrome 확장 상태" }));
    const table = await screen.findByRole("table", { name: "열린 나라장터 탭" });
    const rows = within(table).getAllByRole("row");
    expect(rows.map((r) => [...r.querySelectorAll("[role=cell]")].slice(0, 3).map((c) => c.textContent))).toEqual([
      ["계약 상세", "R26TA0000010101", "새 차수 · 아직 가져오지 않음"],
      ["계약 상세", "R26TA0000010201", "고친 칸 2개가 화면과 다름"],
      ["개찰 결과", "", "수집하지 않는 화면"],
    ]);
    await user.click(within(rows[1]).getByRole("button", { name: "지금 보는 화면에서 보기" }));
    await waitFor(() => expect(비추는번호()).toBe("R26TA0000010201"));
    expect(within(기둥()).getByRole("button", { name: "Chrome 따라가기" }).getAttribute("aria-pressed")).toBe("false");
    await user.click(within(rows[2]).getByRole("button", { name: "이 탭으로 가기" }));
    await waitFor(() => expect(지금화면.명령).toContainEqual({ tabId: 3, command: "focusTab" }));
  });

  it("끊겨 있으면 열린 탭을 지어내지 않고 검토 대기도 세지 않는다", async () => {
    스위치.확장 = "끊김";
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("tab", { name: /^나라장터/ }));
    await user.click(await screen.findByRole("button", { name: "Chrome 확장 상태" }));
    const tabs = await screen.findByRole("region", { name: "열린 나라장터 탭" });
    expect(tabs.textContent).toContain("Chrome이 연결되어 있지 않아 열린 탭을 알 수 없습니다.");
    const summary = screen.getByRole("region", { name: "연결 요약" });
    expect(within(summary).getAllByRole("definition").map((d) => d.textContent)[2]).toBe("—");
  });
});

// ── 3b: 펼침 · 잇기 · 눈 단추 · 사이트 접근 ─────────────────────

const 왼줄 = (label: string) =>
  [...기둥().querySelectorAll<HTMLElement>(".wide-src")].find((r) => r.querySelector("[role=rowheader]")!.textContent === label)!;
const 칸줄 = (column: string) =>
  within(screen.getByRole("table", { name: "저장될 칸" })).getAllByRole("row").find((r) => r.querySelector(".wide-col .strong")?.textContent === column)!;
const 품목줄 = (line: string) =>
  within(screen.getByRole("table", { name: "품목" })).getAllByRole("row").find((r) => r.querySelector("[role=cell]")?.textContent === line)!;
const 자리칸 = (kind: string) =>
  within(screen.getByRole("list", { name: "접수에서 계약까지" })).getAllByRole("listitem").find((c) => c.querySelector(".place-kind")?.textContent === kind)!;
const 밝음 = (el: Element) => el.classList.contains("lit");

async function 펼치기() {
  const user = await 열기();
  await user.click(within(기둥()).getByRole("button", { name: "넓히기" }));
  await waitFor(() => expect(screen.getByRole("table", { name: "저장될 칸" })).toBeTruthy());
  return user;
}

describe("펼침", () => {
  it("펼치면 작업 영역을 비키고 화면 그대로와 작업자료에 설 모양을 나란히 보이며, 왼쪽 탐색을 누르면 좁아진다", async () => {
    const user = await 펼치기();
    expect(document.getElementById("workspace")!.hidden).toBe(true);
    expect(within(기둥()).getByRole("button", { name: "좁히기" })).toBeTruthy();
    // 왼쪽: 화면의 주소 줄·구역·이름표·보이는 글. 견줄 차수와 다른 줄에 파란 점.
    const figure = 기둥().querySelector("figure")!;
    expect(figure.querySelector(".wide-url")!.textContent).toBe("www.g2b.go.kr › 계약 상세");
    expect(within(figure).getByRole("table", { name: "계약 기본정보" })).toBeTruthy();
    expect(왼줄("계약일자").textContent).toBe("계약일자2026-10-02");
    expect(within(왼줄("계약일자")).getByRole("img", { name: "차수 00과 다름" })).toBeTruthy();
    expect(within(왼줄("2 무선마이크 세트")).getByRole("img", { name: "차수 00과 다름" })).toBeTruthy();
    expect(within(왼줄("계약명")).queryByRole("img")).toBeNull();
    expect(figure.querySelector("figcaption")!.textContent).toBe("Chrome에서 보고 있는 화면을 줄인 것입니다. 칸에 마우스를 올리거나 " +
      "Tab·화살표로 옮기면 오른쪽에 그 값이 저장될 칸이 강조됩니다. 가리키는 칸이 없으면 계약일자를 강조합니다. 파란 점은 차수 00과 다른 칸입니다.");
    // 오른쪽: 칸마다 화면의 이름표와 화면 표기, 견준 차수와의 비교. 계약면 몇 열 중 몇 열을 읽는지.
    const fields = screen.getByRole("region", { name: "저장될 칸" });
    expect(fields.textContent).toContain("v_계약 30열 중 이 화면에서 읽는 11열");
    expect(칸줄("계약일자").textContent).toBe("계약일자화면 · 계약일자2026/10/02화면 표기 2026-10-02바뀜2026/09/14");
    expect(칸줄("계약번호").textContent).toContain("식별");
    expect(칸줄("계약건명").textContent).toBe("계약건명화면 · 계약명시험 음향설비 개선같음");
    expect(품목줄("2").textContent).toContain("차수 00은 6세트 · 12,000,000");
    expect(자리칸("공고").textContent).toContain("참조 R26BK00000101 · 아직 수집되지 않음");
    expect(within(기둥()).getByRole("region", { name: "가져오기" }).textContent).toContain("R26TA0000010101 가져오기");

    await user.click(within(기둥()).getByRole("button", { name: "좁히기" }));
    expect(document.getElementById("workspace")!.hidden).toBe(false);
    expect(screen.queryByRole("table", { name: "저장될 칸" })).toBeNull();

    // 왼쪽 탐색의 어느 항목을 눌러도 좁아진다.
    await user.click(within(기둥()).getByRole("button", { name: "넓히기" }));
    expect(document.getElementById("workspace")!.hidden).toBe(true);
    await user.click(screen.getByRole("tab", { name: /^계약/ }));
    expect(document.getElementById("workspace")!.hidden).toBe(false);
    await user.click(within(기둥()).getByRole("button", { name: "넓히기" }));
    await user.click(screen.getByRole("tab", { name: "대시보드" }));
    expect(document.getElementById("workspace")!.hidden).toBe(false);
  });

  it("펼친 채로 접으면 작업 영역이 돌아오고, 다시 펼쳐도 좁은 기둥으로 선다", async () => {
    const user = await 펼치기();
    expect(document.getElementById("workspace")!.hidden).toBe(true);
    await user.click(within(기둥()).getByRole("button", { name: "접기" }));
    expect(document.getElementById("workspace")!.hidden).toBe(false);
    await user.click(screen.getByRole("button", { name: "지금 보는 화면 — 탭 3개, 펼치기" }));
    expect(document.getElementById("workspace")!.hidden).toBe(false);
    expect(within(기둥()).getByRole("button", { name: "넓히기" })).toBeTruthy();
  });

  it("펼친 보기에서는 비운 칸과 화면에 없는 품목 행을 「유지 | 비우기」·「유지 | 지우기」 로 고른다", async () => {
    지금화면.앞 = "4242:2";
    const user = userEvent.setup();
    render(<App />);
    await waitFor(() => expect(비추는번호()).toBe("R26TA0000010201"), 길게);
    await user.click(within(기둥()).getByRole("button", { name: "넓히기" }));
    await waitFor(() => expect(screen.getByRole("table", { name: "저장될 칸" })).toBeTruthy());
    const blank = within(칸줄("납품기한")).getByRole("group", { name: "납품기한 — 있던 값을 둘지, 비울지" });
    expect(within(blank).getByRole("button", { name: "유지" }).getAttribute("aria-pressed")).toBe("true");
    await user.click(within(blank).getByRole("button", { name: "비우기" }));
    expect(칸줄("납품기한").textContent).toContain("2026/11/30 지움");
    const gone = within(품목줄("2")).getByRole("group", { name: "품목 2 — 남겨 둘지, 지울지" });
    await user.click(within(gone).getByRole("button", { name: "지우기" }));
    expect(품목줄("2").textContent).toContain("배기 덕트 보강 지움");
    const foot = within(기둥()).getByRole("region", { name: "가져오기" });
    await user.click(within(foot).getByRole("button", { name: "2곳을 지우고 R26TA0000010201 가져오기" }));
    await waitFor(() => expect(지금화면.가져옴).toHaveLength(1));
    expect(지금화면.가져옴[0].picks).toEqual(["contract/R26TA00000102-01/delivery_due", "remove/contract_item/2"]);
  });

  it("연결되지 않았으면 펼침은 쉰다", async () => {
    스위치.확장 = "끊김";
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: /^지금 보는 화면 — .*, 펼치기$/ }));
    expect(within(기둥()).queryByRole("button", { name: "넓히기" })).toBeNull();
    expect(document.getElementById("workspace")!.hidden).toBe(false);
  });

  it("수집 규칙에 없는 화면은 왼쪽에 화면의 칸을, 오른쪽에 안내를 둔다 — 잇지 않는다", async () => {
    const user = await 펼치기();
    await user.click(within(기둥()).getByRole("button", { name: /^개찰 결과/ }));
    await waitFor(() => expect(within(기둥()).getByRole("heading", { name: "이 화면은 수집하지 않습니다" })).toBeTruthy());
    expect(왼줄("개찰일시").textContent).toBe("개찰일시2026-09-08 14:00");
    expect(왼줄("개찰일시").getAttribute("tabindex")).toBeNull();
    expect(기둥().querySelector("figcaption")!.textContent).toBe("Chrome에서 보고 있는 화면을 줄인 것입니다.");
  });

  it("잇기: 마우스를 올리면 같은 자리를 가리키는 줄·칸·품목·자리가 함께 밝아지고, 떠나면 기본 밝힘으로 돌아간다", async () => {
    await 펼치기();
    // 기본은 첫 「바뀜」 칸.
    await waitFor(() => expect(밝음(칸줄("계약일자"))).toBe(true));
    expect(밝음(왼줄("계약일자"))).toBe(true);
    expect(밝음(칸줄("계약건명"))).toBe(false);

    fireEvent.mouseEnter(왼줄("계약명"));
    await waitFor(() => expect(밝음(칸줄("계약건명"))).toBe(true));
    expect(밝음(칸줄("계약일자"))).toBe(false);
    expect(밝음(왼줄("계약일자"))).toBe(false);

    fireEvent.mouseEnter(왼줄("2 무선마이크 세트"));
    await waitFor(() => expect(밝음(품목줄("2"))).toBe(true));
    expect(밝음(품목줄("1"))).toBe(false);

    // 화면의 공고번호는 자리의 공고 카드를 밝힌다.
    fireEvent.mouseEnter(왼줄("공고번호"));
    await waitFor(() => expect(밝음(자리칸("공고"))).toBe(true));

    // 오른쪽에서 가리켜도 왼쪽이 밝아진다. 번호 열은 문서를 가리는 칸이다.
    fireEvent.mouseEnter(칸줄("차수"));
    await waitFor(() => expect(밝음(왼줄("계약번호"))).toBe(true));
    expect(밝음(자리칸("계약"))).toBe(true);

    fireEvent.mouseLeave(기둥().querySelector(".wide-grid")!);
    await waitFor(() => expect(밝음(칸줄("계약일자"))).toBe(true));
    expect(밝음(왼줄("계약번호"))).toBe(false);
  });

  it("잇기: 목록마다 Tab 자리는 하나이고, 화살표·Home·End 로 옮기면 초점이 밝힘을 데려가며, 둘 다 떠나면 거둔다", async () => {
    await 펼치기();
    const left = [...기둥().querySelectorAll<HTMLElement>(".wide-src")];
    await waitFor(() => expect(left.filter((r) => r.tabIndex === 0)).toEqual([왼줄("계약일자")]));
    const fieldRows = within(screen.getByRole("table", { name: "저장될 칸" })).getAllByRole("row").filter((r) => r.dataset.link);
    expect(fieldRows.filter((r) => r.tabIndex === 0)).toEqual([칸줄("계약일자")]);

    왼줄("계약일자").focus();
    fireEvent.keyDown(왼줄("계약일자"), { key: "ArrowDown" });
    expect(document.activeElement).toBe(왼줄("공고번호"));
    await waitFor(() => expect(밝음(자리칸("공고"))).toBe(true));
    fireEvent.keyDown(왼줄("공고번호"), { key: "End" });
    expect(document.activeElement).toBe(왼줄("2 무선마이크 세트"));
    await waitFor(() => expect(밝음(품목줄("2"))).toBe(true));
    fireEvent.keyDown(왼줄("2 무선마이크 세트"), { key: "Home" });
    expect(document.activeElement).toBe(왼줄("계약번호"));
    fireEvent.keyDown(왼줄("계약번호"), { key: "ArrowUp" });
    expect(document.activeElement).toBe(왼줄("계약번호"));

    // 자리 카드는 가로 목록이라 오른쪽 화살표로도 옮긴다.
    자리칸("접수").focus();
    fireEvent.keyDown(자리칸("접수"), { key: "ArrowRight" });
    expect(document.activeElement).toBe(자리칸("공고"));

    // 펼친 보기 밖으로 초점이 나가고 마우스도 없으면 기본 밝힘으로.
    const narrow = within(기둥()).getByRole("button", { name: "좁히기" });
    fireEvent.blur(자리칸("공고"), { relatedTarget: narrow });
    await waitFor(() => expect(밝음(칸줄("계약일자"))).toBe(true));
    expect(밝음(자리칸("공고"))).toBe(false);
  });
});

describe("사이드바의 눈", () => {
  it("Chrome 의 앞 탭이 수집할 수 있는 화면이면 그 갈래 옆에 서고, 누르면 따라가기를 켜고 앞 탭을 비춘다", async () => {
    const user = await 열기();
    const eye = screen.getByRole("button", { name: "Chrome에서 보고 있는 계약 R26TA0000010101 — 지금 보는 화면에서 보기" });
    expect(eye.closest(".source-branch")!.querySelector("[role=tab]")!.id).toBe("tab-계약");
    await user.click(within(기둥()).getByRole("button", { name: /^계약 상세 R26TA0000010201/ }));
    await waitFor(() => expect(비추는번호()).toBe("R26TA0000010201"));
    expect(within(기둥()).getByRole("button", { name: "Chrome 따라가기" }).getAttribute("aria-pressed")).toBe("false");
    await user.click(eye);
    await waitFor(() => expect(비추는번호()).toBe("R26TA0000010101"));
    expect(within(기둥()).getByRole("button", { name: "Chrome 따라가기" }).getAttribute("aria-pressed")).toBe("true");

    // 사람이 접어 두었어도 눈을 누르면 펴서 비춘다.
    await user.click(within(기둥()).getByRole("button", { name: "접기" }));
    await user.click(eye);
    await waitFor(() => expect(비추는번호()).toBe("R26TA0000010101"));
    expect(localStorage.getItem("pclm.dock.folded")).toBeNull();

    // 앞 탭이 수집하지 않는 화면이면 눈은 없다.
    지금화면.앞 = "4242:3";
    await waitFor(() => expect(screen.queryByRole("button", { name: /지금 보는 화면에서 보기$/ })).toBeNull(), 길게);
  });
});

describe("확장 상태 보드의 사이트 접근", () => {
  async function 보드(user: ReturnType<typeof userEvent.setup>) {
    await user.click(await screen.findByRole("tab", { name: /^나라장터/ }));
    await user.click(await screen.findByRole("button", { name: "Chrome 확장 상태" }));
    return screen.findByRole("complementary", { name: "확장 정보" });
  }

  it("확장이 알린 사이트 접근만 보이고, 걷은 설정(수집기 띄우기·단축키)과 오류 칸은 없다 — 확장에 설정 명령을 보내지 않는다(ADR-042)", async () => {
    const user = await 열기();
    const info = await 보드(user);
    await waitFor(() => expect(info.textContent).toContain("사이트 접근www.g2b.go.kr 허용됨"));
    expect(screen.queryByRole("region", { name: "설정" })).toBeNull();
    expect(screen.queryByRole("region", { name: "오류와 재시도" })).toBeNull();
    expect(screen.queryByRole("button", { name: /항상 띄우기|툴바 버튼으로만|단축키/ })).toBeNull();
    const summary = screen.getByRole("region", { name: "연결 요약" });
    expect(within(summary).getAllByRole("term").map((d) => d.textContent)).toEqual(["확장 판", "오늘 저장", "검토 대기"]);
    expect(document.body.textContent).not.toMatch(/수집기|단축키|Alt\+Shift/);
    expect(지금화면.명령).toEqual([]);
  });

  it("허용하지 않았으면 그렇다고 강조하고, 끊겨 있으면 모른다", async () => {
    지금화면.설정 = { pid: 4242, browser: "Chrome", siteAccess: false };
    const user = await 열기();
    const info = await 보드(user);
    await waitFor(() => expect(within(info).getByText("www.g2b.go.kr 허용 안 됨").className).toContain("warn-strong"));
    cleanup();

    스위치.확장 = "끊김";
    const again = userEvent.setup();
    render(<App />);
    const off = await 보드(again);
    expect(off.textContent).toContain("사이트 접근—");
  });

  it("사이트 접근이 꺼져 탭이 없으면 두 자리 모두 「탭이 없습니다」 대신 그 까닭과 확장 관리 여는 길을 보인다", async () => {
    지금화면.탭 = [];
    지금화면.앞 = null;
    지금화면.설정 = { pid: 4242, browser: "Chrome", siteAccess: false };
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "지금 보는 화면 — 열린 탭 없음, 펼치기" }, 길게));
    const note = await within(기둥()).findByRole("region", { name: "사이트 접근 안 됨" });
    expect(note.textContent).toContain("Chrome이 나라장터 사이트 접근을 허용하지 않아 탭을 볼 수 없습니다.");
    expect(기둥().textContent).not.toContain("열린 나라장터 탭이 없습니다");
    await user.click(within(note).getByRole("button", { name: "Chrome 확장 관리 열기" }));
    await waitFor(() => expect(부름).toHaveBeenCalledWith("openExtensionSetup", ["chrome"]));

    await 보드(user);
    const card = screen.getByRole("region", { name: "열린 나라장터 탭" });
    expect(card.textContent).toContain("Chrome이 나라장터 사이트 접근을 허용하지 않아 탭을 볼 수 없습니다.");
    expect(card.textContent).not.toContain("열린 나라장터 탭이 없습니다");
    부름.mockClear();
    await user.click(within(card).getByRole("button", { name: "Chrome 확장 관리 열기" }));
    await waitFor(() => expect(부름).toHaveBeenCalledWith("openExtensionSetup", ["chrome"]));
  });

  it("사이트 접근을 모르면(설정 없음) 탭이 없다는 말은 그대로다", async () => {
    지금화면.탭 = [];
    지금화면.앞 = null;
    지금화면.설정 = null;
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "지금 보는 화면 — 열린 탭 없음, 펼치기" }, 길게));
    expect(기둥().textContent).toContain("Chrome에 열린 나라장터 탭이 없습니다.");
    expect(within(기둥()).queryByRole("region", { name: "사이트 접근 안 됨" })).toBeNull();
  });
});

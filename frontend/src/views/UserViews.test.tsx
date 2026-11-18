import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { ColumnManager } from "../ColumnManager";
import { invoke, 사용자뷰초기화, 스위치 } from "../mock";
import { UserViews } from "./UserViews";
import { EditorView } from "@codemirror/view";

/**
 * 보기(docs/user-views.md §7). 목업 다리가 지은 값(지어낸 자료)을 그대로 보고 붙든다 — 한 줄은 조달 건이고, 공고문서는
 * 계약이 여럿인 건을 한 줄로 모은다(조달 건 마흔다섯, 계약이 둘인 건은 셋째 공고 하나). 접수가 선 건은 셋이다.
 */
vi.mock("../mock", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../mock")>();
  return { ...actual, invoke: vi.fn(actual.invoke) };
});

const 부름 = vi.mocked(invoke);
const 본디 = 부름.getMockImplementation()!;
const 가로채기 = (method: string, 대신: () => Promise<unknown>) =>
  부름.mockImplementation((m, args) => (m === method ? 대신() : 본디(m, args)));

beforeAll(() => {
  Element.prototype.scrollIntoView = () => {};
  Range.prototype.getClientRects = () => [] as unknown as DOMRectList;
  Range.prototype.getBoundingClientRect = () => new DOMRect();
  HTMLDialogElement.prototype.showModal = function () { this.open = true; };
  HTMLDialogElement.prototype.close = function () { this.open = false; };
});

beforeEach(() => {
  부름.mockClear();
  부름.mockImplementation(본디);
  스위치.열람 = false;
  스위치.뷰맥락오류 = null;
  사용자뷰초기화();
});

afterEach(cleanup);

/** 첫 보기를 명시적으로 편집하여 기존 편집 동작의 소유를 유지한다. */
async function 뷰열기(props: Partial<Parameters<typeof UserViews>[0]> = {}) {
  const user = userEvent.setup();
  const view = render(<UserViews readOnly={false} {...props} />);
  await user.click(await screen.findByRole("button", { name: "뷰 편집" }));
  const record = within(await screen.findByRole("region", { name: "미리보기" }));
  await waitFor(() => expect(줄(record, "공고명").textContent).toContain("2026년 가람 수질측정기 조달"));
  return { user, record, view };
}

const 줄 = (scope: ReturnType<typeof within>, name: string) => scope.getByRole("listitem", { name: new RegExp(`^${name} = `) });
const 줄이름 = (scope: ReturnType<typeof within>) =>
  within(scope.getByRole("list", { name: "열" })).getAllByRole("listitem").map((li: HTMLElement) => li.getAttribute("aria-label")!.split(" = ")[0]);
const 열보기 = () => within(screen.getByRole("complementary", { name: "열 목록" }));

/** 뷰 만들기를 열어 빈 뷰와 열 목록을 세운다. */
async function 새뷰(user: ReturnType<typeof userEvent.setup>) {
  await user.click(within(screen.getByRole("navigation", { name: "뷰" })).getByRole("button", { name: "뷰 만들기" }));
  await screen.findByRole("heading", { name: "열이 없습니다" });
  await waitFor(() => expect(within(열보기().getByRole("region", { name: "공고" })).getByRole("checkbox", { name: "공고명" }).closest("label")!.textContent)
    .toContain("2026년 가람 수질측정기 조달"));
}

describe("보기", () => {
  it("저장된 보기는 모든 열과 실제 행의 표로 열고 검색하며, 편집은 명시적으로 연다", async () => {
    const user = userEvent.setup();
    render(<UserViews readOnly={false} />);
    const result = within(await screen.findByRole("region", { name: "뷰 결과" }));
    const table = within(await result.findByRole("grid", { name: "자료 목록" }));
    expect(table.getAllByRole("columnheader").slice(1).map(c => c.textContent)).toEqual(["접수번호", "공고번호", "계약번호", "공고명", "대표품명", "진행"]);
    expect(table.getAllByRole("row")).toHaveLength(46);
    // 표에서도 지울 수 있다 — 「지우기 · 뷰 편집」 차례로 선다.
    const actions = [...document.querySelectorAll(".uv-actions button")].map(b => b.textContent);
    expect(actions).toEqual(["지우기", "뷰 편집"]);
    expect(screen.getByRole("button", { name: "뷰 지우기" }).textContent).toBe("지우기");
    expect(table.getByRole("gridcell", { name: /R26TA9800070300, R26TA9800070800/ })).toBeTruthy();
    expect(screen.queryByRole("region", { name: "미리보기" })).toBeNull();
    expect(screen.queryByRole("complementary", { name: "열 목록" })).toBeNull();
    expect(부름.mock.calls.some(([m]) => m === "previewUserView" || m === "userViewSamples")).toBe(false);
    fireEvent.change(result.getByRole("searchbox", { name: "목록 검색" }), { target: { value: "R26BK98000502-000" } });
    expect(table.getAllByRole("row")).toHaveLength(2);
    await user.click(screen.getByRole("button", { name: "뷰 편집" }));
    expect(await screen.findByRole("textbox", { name: "뷰 이름" })).toHaveProperty("value", "공고문서");
    expect(screen.getByRole("complementary", { name: "열 목록" })).toBeTruthy();
    const record = within(await screen.findByRole("region", { name: "미리보기" }));
    expect(screen.getByRole("heading", { level: 1, name: "공고문서" })).toBeTruthy();
    expect(screen.getByRole("combobox", { name: "계약이 여럿인 건" })).toHaveProperty("value", "fold");
    expect(await screen.findByText("· 45건")).toBeTruthy();
    expect(줄이름(record)).toEqual(["접수번호", "공고번호", "계약번호", "공고명", "대표품명", "진행"]);
    // 이름과 원천 열 이름이 다를 때만 원천이 또렷하다 — 같으면 그대로 옮기는 열로 흐리게 선다.
    expect(줄(record, "공고번호").querySelector(".uv-formula")!.textContent).toBe("= 공고.입찰공고번호");
    expect(줄(record, "공고명").querySelector(".uv-formula.is-same")!.textContent).toBe("= 공고.공고명");
    // 저장된 그대로면 상태를 적지 않는다 — 고친 뒤에만 「변경됨」 이다.
    expect(screen.queryByText(/저장됨|저장 전|변경됨/)).toBeNull();
    expect(screen.getByRole("navigation", { name: "뷰" })).toBeTruthy();
    // 「기준」 은 낱말로만 본다 — 물가변동적용기준 같은 자료의 이름은 제외한다.
    expect(document.body.textContent).not.toMatch(/사용자 뷰|레코드|(?<![가-힣])기준|연결계약/);
    await user.click(screen.getByRole("button", { name: "표로 돌아가기" }));
    expect(await screen.findByRole("grid", { name: "자료 목록" })).toBeTruthy();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("표 조회 오류를 다시 읽고, 빈 결과와 자료 갱신도 편집 화면 없이 처리한다", async () => {
    const user = userEvent.setup();
    가로채기("sheet", () => Promise.reject(new Error("조회 실패")));
    const view = render(<UserViews readOnly={false} />);
    expect(await screen.findByText("조회 실패")).toBeTruthy();
    가로채기("sheet", () => Promise.resolve({ name: "u_공고문서", columns: ["공고명"], rows: [], editable: [], correctable: [], overrides: {} }));
    await user.click(screen.getByRole("button", { name: "다시 읽기" }));
    expect(await screen.findByRole("heading", { name: "이 뷰에 표시할 자료가 없습니다" })).toBeTruthy();
    부름.mockImplementation(본디);
    view.rerender(<UserViews readOnly={false} refreshVersion={1} />);
    expect(await screen.findByRole("grid", { name: "자료 목록" })).toBeTruthy();
    expect(screen.queryByRole("region", { name: "미리보기" })).toBeNull();
  });

  it("표로 돌아갈 때 계속 편집·변경 버리기·저장 후 이동이 마지막 저장 구성을 지킨다", async () => {
    const { user } = await 뷰열기();
    const changeName = async (name: string) => {
      const input = screen.getByRole("textbox", { name: "뷰 이름" });
      fireEvent.change(input, { target: { value: name } });
      await user.click(screen.getByRole("button", { name: "표로 돌아가기" }));
      return within(await screen.findByRole("dialog", { name: "저장하지 않은 뷰 변경" }));
    };
    await user.click((await changeName("버릴 이름")).getByRole("button", { name: "계속 편집" }));
    expect(screen.getByRole("textbox", { name: "뷰 이름" })).toHaveProperty("value", "버릴 이름");
    await user.click(screen.getByRole("button", { name: "표로 돌아가기" }));
    await user.click(within(await screen.findByRole("dialog", { name: "저장하지 않은 뷰 변경" })).getByRole("button", { name: "변경 버리기" }));
    expect(await screen.findByRole("grid", { name: "자료 목록" })).toBeTruthy();
    expect(screen.getByRole("heading", { level: 1, name: "공고문서" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "뷰 편집" }));
    await user.click((await changeName('보고 "보기"')).getByRole("button", { name: "저장 후 이동" }));
    expect(await screen.findByRole("grid", { name: "자료 목록" })).toBeTruthy();
    expect(screen.getByRole("heading", { level: 1, name: '보고 "보기"' })).toBeTruthy();
    expect(부름.mock.calls.filter(([m]) => m === "sheet").at(-1)![1]).toEqual(['u_보고 "보기"']);
    await user.click(screen.getByRole("button", { name: "뷰 편집" }));
    expect(screen.getByRole("textbox", { name: "뷰 이름" })).toHaveProperty("value", '보고 "보기"');
  });

  it("저장할 수 없는 변경을 두고 떠나면 「저장 후 이동」 이 꺼진 까닭을 확인 창 본문에 보인다", async () => {
    const { user } = await 뷰열기();
    await user.clear(screen.getByRole("textbox", { name: "뷰 이름" }));
    await user.click(screen.getByRole("button", { name: "표로 돌아가기" }));
    const confirm = within(await screen.findByRole("dialog", { name: "저장하지 않은 뷰 변경" }));
    const go = confirm.getByRole("button", { name: "저장 후 이동" });
    expect(go).toHaveProperty("disabled", true);
    expect(confirm.getByText("지금은 저장할 수 없습니다. 뷰 이름을 적으세요.")).toBeTruthy();
    expect(go.getAttribute("aria-describedby")).toBe(confirm.getByText("지금은 저장할 수 없습니다. 뷰 이름을 적으세요.").id);
    // 이름을 다시 적으면 까닭이 걷히고 저장할 수 있다.
    await user.click(confirm.getByRole("button", { name: "계속 편집" }));
    await user.type(screen.getByRole("textbox", { name: "뷰 이름" }), "공고문서");
    await user.click(screen.getByRole("button", { name: "표로 돌아가기" }));
    expect(await screen.findByRole("grid", { name: "자료 목록" })).toBeTruthy();
  });

  /** 실패를 삼키면 줄은 「계산 중」 에, 고르개는 「맞는 미리보기 자료가 없습니다」 에 머물러 무엇이 잘못됐는지 알 길이 없다. */
  it("미리보기를 계산하지 못하면 까닭과 「다시 계산」 을 보이고 값 칸은 「—」 이며, 다시 계산하면 값이 선다", async () => {
    가로채기("previewUserView", () => Promise.reject(new Error("미리보기 계산이 끊겼습니다.")));
    const user = userEvent.setup();
    render(<UserViews readOnly={false} />);
    await user.click(await screen.findByRole("button", { name: "뷰 편집" }));
    const record = within(await screen.findByRole("region", { name: "미리보기" }));
    const alert = await record.findByRole("alert");
    expect(alert.textContent).toContain("미리보기를 계산하지 못했습니다: 미리보기 계산이 끊겼습니다.");
    expect(within(줄(record, "공고명")).getByText("—")).toBeTruthy();
    expect(record.queryByText("계산 중")).toBeNull();

    부름.mockImplementation(본디);
    await user.click(within(alert).getByRole("button", { name: "다시 계산" }));
    await waitFor(() => expect(줄(record, "공고명").textContent).toContain("2026년 가람 수질측정기 조달"));
    expect(record.queryByRole("alert")).toBeNull();
  });

  it("미리보기 자료를 찾지 못하면 「맞는 자료가 없다」 가 아니라 까닭과 「다시 찾기」 를 보인다", async () => {
    가로채기("userViewSamples", () => Promise.reject(new Error("자료를 읽지 못했습니다.")));
    const user = userEvent.setup();
    render(<UserViews readOnly={false} />);
    await user.click(await screen.findByRole("button", { name: "뷰 편집" }));
    const picker = within(await screen.findByRole("group", { name: "미리보기 자료" }));
    expect(await picker.findByText("미리보기 자료를 찾지 못했습니다: 자료를 읽지 못했습니다.")).toBeTruthy();
    expect(picker.queryByText("맞는 미리보기 자료가 없습니다")).toBeNull();
    const record = within(screen.getByRole("region", { name: "미리보기" }));
    await waitFor(() => expect(within(줄(record, "공고명")).getByText("—")).toBeTruthy());

    부름.mockImplementation(본디);
    await user.click(picker.getByRole("button", { name: "다시 찾기" }));
    await waitFor(() => expect(줄(record, "공고명").textContent).toContain("2026년 가람 수질측정기 조달"));
    expect(picker.queryByText(/찾지 못했습니다/)).toBeNull();
    expect(picker.queryByRole("button", { name: "다시 찾기" })).toBeNull();
  });

  it("열과 미리보기 자료를 불러오지 못하면 「자료 다시 불러오기」 로 다시 부른다", async () => {
    스위치.뷰맥락오류 = "열 목록을 읽지 못했습니다.";
    const user = userEvent.setup();
    render(<UserViews readOnly={false} />);
    await user.click(await screen.findByRole("button", { name: "뷰 편집" }));
    const record = within(await screen.findByRole("region", { name: "미리보기" }));
    const alert = await record.findByRole("alert");
    expect(alert.textContent).toContain("열 목록을 읽지 못했습니다.");

    스위치.뷰맥락오류 = null;
    await user.click(within(alert).getByRole("button", { name: "자료 다시 불러오기" }));
    await waitFor(() => expect(within(열보기().getByRole("region", { name: "공고" })).getByRole("checkbox", { name: "공고명" })).toBeTruthy());
    expect(record.queryByRole("alert")).toBeNull();
  });

  it("표본 ‹ › 와 가장자리 칩이 찍히는 값을 바꾼다 — 표본 하나는 조달 건 하나다", async () => {
    const { user, record } = await 뷰열기();
    // 접수가 선 건은 마흔다섯 중 셋뿐이다.
    expect(줄(record, "접수번호").textContent).toContain("TSTPLA26500301-000");
    expect(within(줄(record, "접수번호")).getByText("빈 값 42건")).toBeTruthy();
    const picker = within(screen.getByRole("group", { name: "미리보기 자료" }));
    expect(picker.getByRole("searchbox").closest("details")?.open).toBe(false);
    expect(picker.getByText("R26BK98000501-000")).toBeTruthy();
    expect(picker.getByRole("button", { name: "이전 미리보기 자료" })).toHaveProperty("disabled", true);
    await user.click(picker.getByRole("button", { name: "다음 미리보기 자료" }));
    expect(await picker.findByText("R26BK98000502-000")).toBeTruthy();
    await waitFor(() => expect(줄(record, "공고번호").textContent).toContain("R26BK98000502-000"));
    const blank = within(줄(record, "접수번호")).getByText("(빈 값)");
    expect(blank.className).toBe("uv-blank");
    expect(줄(record, "접수번호").innerHTML).not.toContain("fb-blank");

    await user.click(picker.getByText("다른 자료로 확인"));
    expect(picker.getByRole("button", { name: "계약 여러 건" }).getAttribute("aria-pressed")).toBe("false");
    await user.click(picker.getByRole("button", { name: "계약 여러 건" }));
    expect(await picker.findByText("R26BK98000503-000")).toBeTruthy();
    expect(picker.getByRole("button", { name: "계약 여러 건" }).getAttribute("aria-pressed")).toBe("true");
    const [definition, , edge] = 부름.mock.calls.filter(([m]) => m === "userViewSamples").at(-1)![1] as [string, null, string];
    expect(edge).toBe("manyContracts");
    expect(JSON.parse(definition)).toMatchObject({ name: "공고문서", definition: { version: 2, contracts: "fold" } });
    expect(picker.getByText("1/1")).toBeTruthy();
    await waitFor(() => expect(줄(record, "계약번호").textContent).toContain("R26TA9800070300, R26TA9800070800"));
    expect(within(줄(record, "계약번호")).getByText("이어 붙임")).toBeTruthy();

    await user.click(picker.getByRole("button", { name: "일부 자료 없음" }));
    expect(await picker.findByText("R26BK98000502-000")).toBeTruthy();
    await waitFor(() => expect(within(줄(record, "접수번호")).getByText("(빈 값)")).toBeTruthy());
  });

  it("계약이 여럿인 건은 머리의 「한 줄 = …」 에서 정하고, 한 줄 = 계약으로 두면 이어 붙이지 않는다", async () => {
    const { user, record } = await 뷰열기();
    const choice = screen.getByRole("combobox", { name: "계약이 여럿인 건" });
    expect(choice.closest(".uv-meta")).toBeTruthy();
    expect([...(choice as HTMLSelectElement).options].map(o => o.textContent)).toEqual(["한 줄 = 계약", "한 줄 = 조달 건"]);
    expect(choice).toHaveProperty("value", "fold");
    expect(within(줄(record, "계약번호")).getByText("이어 붙임")).toBeTruthy();

    await user.selectOptions(choice, "한 줄 = 계약");
    expect(choice).toHaveProperty("value", "split");
    expect(within(줄(record, "계약번호")).queryByText("이어 붙임")).toBeNull();
    expect(screen.getByText("변경됨")).toBeTruthy();
    // 계약마다 한 줄이면 계약이 둘인 건이 두 줄이다 — 건 수가 하나 는다.
    expect(await screen.findByText("· 46건")).toBeTruthy();
    await user.click(screen.getByText("다른 자료로 확인"));
    await user.click(screen.getByRole("button", { name: "계약 여러 건" }));
    expect(await screen.findByText("1/2")).toBeTruthy();
    await waitFor(() => expect(줄(record, "계약번호").textContent).toContain("R26TA9800070300"));
    expect(줄(record, "계약번호").textContent).not.toContain(", ");
  });

  it("깨진 식은 그 줄에 까닭과 함께 서고, 머리와 보기는 「깨진 식 n」 이라 센다", async () => {
    const { user } = await 뷰열기();
    await user.click(within(screen.getByRole("navigation", { name: "뷰" })).getByRole("button", { name: /^계약서류/ }));
    expect(screen.getByText(/깨진 식의 값은 빈 칸으로 표시합니다/)).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "뷰 편집" }));
    const record = within(screen.getByRole("region", { name: "미리보기" }));
    await waitFor(() => expect(within(줄(record, "진행")).getByText("깨진 식")).toBeTruthy());
    expect(줄(record, "진행").textContent).toContain("계약에 없는 필드: 진행단계");
    expect(screen.getAllByText("깨진 식 1")).toHaveLength(2);
    expect(screen.getByText("저장할 수 없습니다. 식이 깨진 열이 1개 있습니다.")).toBeTruthy();
  });

  it("「뷰 만들기」는 빈 이름과 열린 열 목록으로 시작하고 이름·열이 있어야 저장한다", async () => {
    const { user } = await 뷰열기();
    await 새뷰(user);
    expect(screen.getByRole("textbox", { name: "뷰 이름" })).toHaveProperty("value", "");
    expect(screen.getByText("저장할 수 없습니다. 뷰 이름을 적으세요.")).toBeTruthy();
    fireEvent.change(screen.getByRole("textbox", { name: "뷰 이름" }), { target: { value: "월간보고" } });
    expect(screen.getByText("저장할 수 없습니다. 열을 하나 이상 추가하세요.")).toBeTruthy();
    expect(screen.getByRole("button", { name: "열 추가" }).getAttribute("aria-expanded")).toBe("true");
    expect(document.querySelector(".uv-collect")!.className).toContain("is-open");
    expect(screen.queryByText(/어느 문서마다/)).toBeNull();
    // 무리는 접수 → 공고 → 계약이고, 무리마다 문서의 열 뒤에 품목 묶음이 선다. 금액 합계는 품목에 금액이 있을 때만이다.
    const panel = 열보기();
    expect(panel.getByRole("heading", { name: "열" })).toBeTruthy();
    expect(panel.getByPlaceholderText("열 이름이나 값으로 찾기")).toBeTruthy();
    expect(panel.getAllByRole("region").map(r => r.getAttribute("aria-label"))).toEqual(["접수", "공고", "계약"]);
    const notice = within(panel.getByRole("region", { name: "공고" })).getAllByRole("checkbox").map(b => b.getAttribute("aria-label"));
    expect(notice.slice(-5)).toEqual(["검토여부", "메모", "품명 모음", "대표품명", "품목 수"]);
    expect(within(panel.getByRole("region", { name: "계약" })).getAllByRole("checkbox").map(b => b.getAttribute("aria-label")).slice(-3))
      .toEqual(["대표품명", "품목 수", "금액 합계"]);
    // 계약 열이 없으면 고를 것도 없다 — 한 줄은 조달 건이다.
    expect(screen.queryByRole("combobox", { name: "계약이 여럿인 건" })).toBeNull();
    expect(screen.getByText("한 줄 = 조달 건")).toBeTruthy();
    await user.click(within(열보기().getByRole("region", { name: "공고" })).getByRole("checkbox", { name: "공고명" }));
    await user.click(screen.getByRole("button", { name: "저장" }));
    expect(await screen.findByRole("grid", { name: "자료 목록" })).toBeTruthy();
    expect(screen.getByRole("heading", { level: 1, name: "월간보고" })).toBeTruthy();
    expect(screen.queryByRole("textbox", { name: "뷰 이름" })).toBeNull();
  });

  it("열람 중 빈 보기는 생성 단추 없이 안내하고 이동할 수 있다", async () => {
    가로채기("userViews", () => Promise.resolve([]));
    const proceed = vi.fn();
    const view = render(<UserViews readOnly />);
    expect(await screen.findByRole("heading", { name: "아직 뷰가 없습니다" })).toBeTruthy();
    expect(screen.queryByRole("region", { name: "미리보기" })).toBeNull();
    expect(screen.queryByRole("button", { name: "뷰 만들기" })).toBeNull();
    view.rerender(<UserViews readOnly navigationRequest={{ id: 1, proceed }} />);
    await waitFor(() => expect(proceed).toHaveBeenCalledOnce());
  });

  it("열 목록에서 체크하면 끝에 줄이 서고 이 표본의 값이 찍힌다. 계약 열이 들면 머리에서 계약 처리를 고른다", async () => {
    const { user } = await 뷰열기();
    await 새뷰(user);
    const notice = within(열보기().getByRole("region", { name: "공고" }));
    const box = notice.getByRole("checkbox", { name: "공고명" });
    await user.click(box);
    const record = within(screen.getByRole("region", { name: "미리보기" }));
    await waitFor(() => expect(줄(record, "공고명").textContent).toContain("2026년 가람 수질측정기 조달"));
    expect(box).toHaveProperty("checked", true);
    // 공고 열만 담으면 공고가 선 건만 — 매달리지 못한 줄이 빠진다.
    expect(await screen.findByText("· 45건")).toBeTruthy();
    await user.click(within(열보기().getByRole("region", { name: "계약" })).getByRole("checkbox", { name: "계약번호" }));
    expect(screen.getByRole("combobox", { name: "계약이 여럿인 건" })).toHaveProperty("value", "split");
    await user.click(box);
    expect(줄이름(record)).toEqual(["계약번호"]);
  });

  it("담을 때 이름이 겹치면 문서 종류의 이름을 앞에 붙인다", async () => {
    const { user } = await 뷰열기();
    await 새뷰(user);
    await user.click(within(열보기().getByRole("region", { name: "공고" })).getByRole("checkbox", { name: "수요기관" }));
    await user.click(within(열보기().getByRole("region", { name: "계약" })).getByRole("checkbox", { name: "수요기관" }));
    await user.click(within(열보기().getByRole("region", { name: "공고" })).getByRole("checkbox", { name: "품목 수" }));
    await user.click(within(열보기().getByRole("region", { name: "계약" })).getByRole("checkbox", { name: "품목 수" }));
    const record = within(screen.getByRole("region", { name: "미리보기" }));
    expect(줄이름(record)).toEqual(["수요기관", "계약수요기관", "품목 수", "계약품목 수"]);
    expect(줄(record, "계약품목 수").querySelector(".uv-formula")!.textContent).toBe("= 계약.품목 |> 개수");
  });

  it("Enter 로 줄을 열면 경로 「이름 › 열」 아래 가공열 편집기가 서고 Esc 로 돌아온다", async () => {
    const { user, record } = await 뷰열기();
    줄(record, "계약번호").focus();
    await user.keyboard("{Enter}");
    const editor = within(await screen.findByRole("region", { name: "열 편집" }));
    const crumbs = within(editor.getByRole("navigation", { name: "경로" }));
    expect(crumbs.getByRole("button", { name: "공고문서" })).toBeTruthy();
    expect(editor.getByRole("textbox", { name: "열 이름" })).toHaveProperty("value", "계약번호");
    // 편집하는 동안 쪽 제목은 눈에서만 숨어 접근성 트리에는 남는다.
    expect(screen.getByRole("heading", { level: 1, name: "공고문서" }).closest("header")!.className).toContain("sr-only");
    const grid = await editor.findByRole("grid", { name: "값 흐름" });
    // 원천은 문서 이름으로 읽힌다 — 연결계약이 아니라 계약.
    expect(within(grid).getByRole("rowheader", { name: /계약\.계약번호/ })).toBeTruthy();
    // 보표는 보기의 환경으로 돈다 — 기준 인자가 없다.
    await waitFor(() => expect(부름.mock.calls.some(([m]) => m === "traceUserViewColumn")).toBe(true));
    const [json] = 부름.mock.calls.find(([m]) => m === "traceUserViewColumn")![1] as [string, string[]];
    expect(JSON.parse(json).flow.from).toEqual({ kind: "linked", link: "contract", field: "계약번호" });
    expect(부름.mock.calls.some(([m]) => m === "traceComputedColumn")).toBe(false);
    expect(screen.queryByRole("complementary", { name: "열 목록" })).toBeNull();
    fireEvent.keyDown(editor.getByRole("textbox", { name: "열 이름" }), { key: "Escape" });
    const back = within(await screen.findByRole("region", { name: "미리보기" }));
    expect(줄(back, "계약번호")).toBeTruthy();
    await waitFor(() => expect(document.activeElement).toBe(줄(back, "계약번호")));
  });

  it("미완성 흐름식은 탭과 열 닫기에도 남고, 이동에서 버리거나 고친 뒤에만 저장한다", async () => {
    const onDirtyChange = vi.fn();
    const { user, record, view } = await 뷰열기({ onDirtyChange });
    줄(record, "계약번호").focus();
    await user.keyboard("{Enter}");
    let editor = within(await screen.findByRole("region", { name: "열 편집" }));
    await user.click(editor.getByRole("tab", { name: "흐름식" }));
    const 코드 = () => EditorView.findFromDOM(screen.getByRole("textbox", { name: "흐름식" }).closest(".cm-editor") as HTMLElement)!;
    await editor.findByRole("textbox", { name: "흐름식" });
    const 쓰기 = (text: string) => act(() => { const code = 코드(); code.dispatch({ changes: { from: 0, to: code.state.doc.length, insert: text } }); });
    const 원래 = 코드().state.doc.toString();
    // 저장된 AST를 고치지 않은 채 원문만 깨뜨려도 미저장 변경이다.
    쓰기(`${원래} |> 없는동사`);
    expect(await editor.findByText(/^이 식을 읽을 수 없습니다: /)).toBeTruthy();
    expect(onDirtyChange).toHaveBeenLastCalledWith(true);
    expect(screen.getByRole("button", { name: "저장" })).toHaveProperty("disabled", true);
    await user.click(editor.getByRole("tab", { name: "단계" }));
    await user.click(editor.getByRole("tab", { name: "흐름식" }));
    expect(코드().state.doc.toString()).toBe(`${원래} |> 없는동사`);
    await user.click(editor.getByRole("button", { name: "공고문서" }));
    expect(await screen.findByRole("region", { name: "미리보기" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "표로 돌아가기" }));
    let guard = within(await screen.findByRole("dialog", { name: "저장하지 않은 뷰 변경" }));
    expect(guard.getByRole("button", { name: "저장 후 이동" })).toHaveProperty("disabled", true);
    await user.click(guard.getByRole("button", { name: "계속 편집" }));
    줄(within(screen.getByRole("region", { name: "미리보기" })), "계약번호").focus();
    await user.keyboard("{Enter}");
    editor = within(await screen.findByRole("region", { name: "열 편집" }));
    await user.click(editor.getByRole("tab", { name: "흐름식" }));
    expect(코드().state.doc.toString()).toBe(`${원래} |> 없는동사`);
    쓰기("");
    expect(await editor.findByText("식이 비어 있습니다. 단계에는 마지막으로 읽은 식이 남아 있습니다.")).toBeTruthy();
    await user.click(editor.getByRole("tab", { name: "SQL" }));
    await user.click(editor.getByRole("tab", { name: "흐름식" }));
    expect(코드().state.doc.toString()).toBe("");
    const proceed = vi.fn();
    view.rerender(<UserViews readOnly={false} onDirtyChange={onDirtyChange} navigationRequest={{ id: 1, proceed }} />);
    guard = within(await screen.findByRole("dialog", { name: "저장하지 않은 뷰 변경" }));
    expect(proceed).not.toHaveBeenCalled();
    await user.click(guard.getByRole("button", { name: "변경 버리기" }));
    expect(proceed).toHaveBeenCalledOnce();
    expect(onDirtyChange).toHaveBeenLastCalledWith(false);
    expect(코드().state.doc.toString()).toBe(원래);
    expect(부름.mock.calls.some(([m]) => m === "saveUserView")).toBe(false);
    쓰기(`${원래} |> 다듬기`);
    await user.click(screen.getByRole("button", { name: "저장" }));
    expect(await screen.findByRole("grid")).toBeTruthy();
    const saved = 부름.mock.calls.find(([m]) => m === "saveUserView")!;
    expect(JSON.parse(saved[1][0] as string).definition.columns.find((c: { name: string }) => c.name === "계약번호").formula.flow.steps).toEqual([{ op: "trim" }]);
    expect(onDirtyChange).toHaveBeenLastCalledWith(false);
  });

  it("흐름식 탭에도 미리보기 자료마다 열에 찍힐 값이 서고, 원문이 깨지면 표가 사라진다", async () => {
    const { user, record } = await 뷰열기();
    줄(record, "계약번호").focus();
    await user.keyboard("{Enter}");
    const editor = within(await screen.findByRole("region", { name: "열 편집" }));
    await user.click(editor.getByRole("tab", { name: "흐름식" }));
    await editor.findByRole("textbox", { name: "흐름식" });
    const results = await editor.findByRole("table", { name: "미리보기 자료 결과" });
    expect(results.textContent).toContain("R26TA9800070300");
    const code = EditorView.findFromDOM(screen.getByRole("textbox", { name: "흐름식" }).closest(".cm-editor") as HTMLElement)!;
    act(() => { code.dispatch({ changes: { from: code.state.doc.length, insert: " |> 없는동사" } }); });
    expect(await editor.findByText(/^이 식을 읽을 수 없습니다: /)).toBeTruthy();
    expect(editor.queryByRole("table", { name: "미리보기 자료 결과" })).toBeNull();
  });

  it("편집기 보표의 표본 A 는 미리보기가 보는 표본이고, 고르개 목록의 다음 둘이 뒤따른다", async () => {
    const { user, record } = await 뷰열기();
    await user.click(screen.getByRole("button", { name: "다음 미리보기 자료" }));
    await waitFor(() => expect(줄(record, "공고번호").textContent).toContain("R26BK98000502-000"));
    줄(record, "계약번호").focus();
    await user.keyboard("{Enter}");
    const editor = within(await screen.findByRole("region", { name: "열 편집" }));
    await waitFor(() => expect(부름.mock.calls.some(([m]) => m === "traceUserViewColumn")).toBe(true));
    const [, keys] = 부름.mock.calls.find(([m]) => m === "traceUserViewColumn")![1] as [string, string[]];
    expect(keys).toEqual(["|R26BK98000502-000|R26TA9800070200", "|R26BK98000503-000|", "|R26BK98000504-000|R26TA9800070400"]);
    const grid = await editor.findByRole("grid", { name: "값 흐름" });
    // 고르개는 값 열의 머리글 안에 있고, 고른 자료 한 건의 칸만 그린다.
    const picker = await within(grid).findByRole("combobox", { name: "미리보기 자료" }) as HTMLSelectElement;
    expect(within(grid).getAllByRole("columnheader")[1].contains(picker)).toBe(true);
    await waitFor(() => expect(picker.selectedOptions[0].textContent).toMatch(/^A · 26년 한별 공압식승강판 20톤 구매 · \d+품목$/));
    expect([...picker.options].map(o => o.textContent![0])).toEqual(["A", "B", "C"]);
    const final = within(grid).getByRole("row", { name: /^열에 찍힐 값/ });
    const start = within(grid).getAllByRole("row")[1];
    expect(final.querySelectorAll("td")).toHaveLength(1);
    expect(start.querySelectorAll("td")).toHaveLength(1);
    // 표본 A 의 열에 찍힐 값이 미리보기가 그 표본에서 보인 값과 같고, 모은 건(표본 B)은 이어 붙인 값이다.
    expect(final.querySelector("td")!.textContent).toBe("R26TA9800070200");
    expect(start.querySelector("td")!.textContent).toContain("R26TA9800070200");
    await user.selectOptions(picker, "1");
    expect(final.querySelector("td")!.textContent).toBe("R26TA9800070300, R26TA9800070800");
    expect(start.querySelector("td")!.textContent).toContain("R26TA9800070300");
    expect(start.querySelector("td")!.textContent).not.toContain("R26TA9800070200");
  });

  it("좁은 창의 열 목록 서랍은 열면 첫 조작부로, Esc·닫기로 닫으면 「열 추가」 로 초점을 옮기고, 편집기로 가면 걷힌다", async () => {
    const { user } = await 뷰열기();
    const toggle = screen.getByRole("button", { name: "열 추가" });
    const drawer = document.querySelector(".uv-collect")!;
    await user.click(toggle);
    expect(toggle.getAttribute("aria-expanded")).toBe("true");
    expect(document.activeElement).toBe(열보기().getByRole("button", { name: "열 목록 닫기" }));
    await user.keyboard("{Escape}");
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
    expect(drawer.className).not.toContain("is-open");
    expect(document.activeElement).toBe(toggle);
    await user.click(toggle);
    await user.click(열보기().getByRole("button", { name: "열 목록 닫기" }));
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
    expect(document.activeElement).toBe(toggle);

    // 서랍을 편 채 「식으로 만들기」 로 편집기에 갔다 Esc 로 돌아오면 서랍이 다시 덮지 않는다.
    await user.click(toggle);
    await user.click(screen.getByRole("button", { name: "식으로 만들기" }));
    await screen.findByRole("region", { name: "열 편집" });
    await user.keyboard("{Escape}");
    await screen.findByRole("region", { name: "미리보기" });
    expect(screen.getByRole("button", { name: "열 추가" }).getAttribute("aria-expanded")).toBe("false");
    expect(document.querySelector(".uv-collect")!.className).not.toContain("is-open");
  });

  it("「+ 식으로 만들기」 는 빈 줄을 세우고 편집기의 빠른 시작에서 문서를 고르게 한다", async () => {
    const { user } = await 뷰열기();
    await user.click(screen.getByRole("button", { name: "식으로 만들기" }));
    const editor = within(await screen.findByRole("region", { name: "열 편집" }));
    const name = editor.getByRole("textbox", { name: "열 이름" });
    expect(document.activeElement).toBe(name);
    expect(editor.getByText("열 이름")).toBeTruthy();
    expect(within(editor.getByRole("navigation", { name: "경로" })).queryByRole("textbox")).toBeNull();
    const quick = within(await editor.findByRole("region", { name: "빠른 시작" }));
    expect(quick.getAllByRole("button").map(b => b.textContent)).toEqual(["접수", "공고", "계약", "글자"]);
    expect(quick.queryByRole("button", { name: "품목" })).toBeNull();
    await user.click(quick.getByRole("button", { name: "공고" }));
    expect(await editor.findByRole("rowheader", { name: /^공고 시작 데이터/ })).toBeTruthy();
    const add = editor.getByRole("button", { name: "단계 추가" });
    await user.click(add);
    await user.click(within(editor.getByRole("dialog", { name: "단계 추가" })).getByRole("option", { name: /^꺼내기/ }));
    await user.click(editor.getByRole("rowheader", { name: /^공고 시작 데이터/ }));
    // 끝 단추는 고른 줄과 무관하게 마지막에 붙인다.
    await user.click(add);
    await user.click(within(editor.getByRole("dialog", { name: "단계 추가" })).getByRole("option", { name: /^비면/ }));
    await waitFor(() => expect(document.activeElement).toBe(editor.getByRole("rowheader", { name: /^비면/ }).closest("tr")));
    await user.keyboard("{ArrowUp}");
    await user.click(editor.getByRole("button", { name: "1단계 뒤에 단계 추가" }));
    const search = editor.getByRole("combobox", { name: "동사 찾기" });
    await user.type(search, "다듬기");
    await user.keyboard("{Enter}");
    expect(editor.getAllByRole("rowheader").slice(0, 4).map(r => r.querySelector(".trace-verb")?.textContent)).toEqual(["공고", "꺼내기", "다듬기", "비면"]);
  });

  it("단계 설정은 고른 줄 바로 아래로 옮겨지고 입력의 키가 단계를 지우지 않는다", async () => {
    const { user, record } = await 뷰열기();
    줄(record, "대표품명").focus();
    await user.keyboard("{Enter}");
    const editor = within(await screen.findByRole("region", { name: "열 편집" }));
    const grid = editor.getByRole("grid", { name: "값 흐름" });
    const flow = within(grid);
    expect(flow.getByRole("rowheader", { name: /^공고.품목 시작 데이터/ }).querySelector(".trace-meta[aria-hidden]")?.textContent).toBe("행목록");
    expect(flow.getByRole("rowheader", { name: /^꺼내기/ }).querySelector(".trace-meta[aria-hidden]")?.textContent).toBe("행목록 → 값목록");
    expect(flow.getByRole("rowheader", { name: /^고유/ }).querySelector(".trace-meta[aria-hidden]")).toBeNull();
    expect(flow.getByRole("rowheader", { name: /^줄여쓰기/ }).querySelector(".trace-meta[aria-hidden]")?.textContent).toBe("값목록 → 값");
    for (const name of [/^꺼내기/, /^고유/, /^줄여쓰기/]) {
      const row = within(grid).getByRole("rowheader", { name }).closest("tr")!;
      await user.click(within(row).getByRole("rowheader"));
      expect(editor.getByRole("region", { name: "단계 설정" }).closest("tr")).toBe(row.nextElementSibling);
    }
    const row = within(grid).getByRole("rowheader", { name: /^줄여쓰기/ }).closest("tr")!;
    act(() => row.focus());
    await user.keyboard("{Enter}");
    expect(editor.getByRole("region", { name: "단계 설정" }).contains(document.activeElement)).toBe(true);
    await user.click(within(editor.getByRole("region", { name: "단계 설정" })).getByRole("spinbutton", { name: "보일 개수" }));
    await user.keyboard("{Delete}");
    expect(within(grid).getByRole("rowheader", { name: /^줄여쓰기/ })).toBeTruthy();
    expect(Number(grid.getAttribute("aria-rowcount"))).toBe(within(grid).getAllByRole("row").length);
  });

  it("중간 삽입 고르개는 닫으면 단추로 돌아오고 마지막 단계에는 삽입 단추가 없다", async () => {
    const { user, record } = await 뷰열기();
    줄(record, "대표품명").focus();
    await user.keyboard("{Enter}");
    const editor = within(await screen.findByRole("region", { name: "열 편집" }));
    await user.click(editor.getByRole("rowheader", { name: /^꺼내기/ }));
    const insert = editor.getByRole("button", { name: "1단계 뒤에 단계 추가" });
    await user.click(insert);
    const pickerRow = editor.getByRole("dialog", { name: "단계 추가" }).closest("tr")!;
    expect(pickerRow.previousElementSibling).toBe(editor.getByRole("region", { name: "단계 설정" }).closest("tr"));
    await user.keyboard("{Escape}");
    await waitFor(() => expect(document.activeElement).toBe(insert));
    await user.click(insert);
    const append = editor.getByRole("button", { name: "단계 추가" });
    await user.click(append);
    await user.keyboard("다듬기");
    expect(editor.getByRole("combobox", { name: "동사 찾기" })).toHaveProperty("value", "다듬기");
    await user.keyboard("{Escape}");
    await waitFor(() => expect(document.activeElement).toBe(append));
    await user.click(insert);
    await user.pointer({ target: editor.getByRole("tab", { name: "단계" }), keys: "[MouseLeft>]" });
    expect(editor.queryByRole("dialog", { name: "단계 추가" })).toBeNull();
    await waitFor(() => expect(document.activeElement).toBe(insert));
    await user.pointer({ keys: "[/MouseLeft]" });
    await user.click(editor.getByRole("rowheader", { name: /^줄여쓰기/ }));
    expect(editor.queryByRole("button", { name: /뒤에 단계 추가$/ })).toBeNull();
    expect(editor.getByRole("button", { name: "단계 추가" }).closest("tr")?.parentElement?.tagName).toBe("TFOOT");
  });

  it("F2 로 이름을 바꾸고, 겹친 이름은 그 줄에서 바로 말하며 저장을 막는다", async () => {
    const { user, record } = await 뷰열기();
    줄(record, "공고명").focus();
    await user.keyboard("{F2}");
    const input = within(줄(record, "공고명")).getByRole("textbox", { name: "열 이름" });
    await user.clear(input);
    await user.type(input, "공고번호");
    expect(record.getAllByRole("alert").map(a => a.textContent)).toContain("‘공고번호’ 이름이 겹칩니다.");
    expect(screen.getByRole("button", { name: "저장" })).toHaveProperty("disabled", true);
    expect(screen.getByText("저장할 수 없습니다. 이름이 비었거나 겹치는 열이 2개 있습니다.")).toBeTruthy();
    await user.keyboard("{Escape}");
    expect(줄이름(record)).toContain("공고명");
    expect(record.queryAllByRole("alert")).toHaveLength(0);
  });

  it("식을 정하지 않은 열과 같은 이름의 뷰는 저장을 막는다", async () => {
    const { user } = await 뷰열기();
    await user.click(screen.getByRole("button", { name: "식으로 만들기" }));
    await user.type(await screen.findByRole("textbox", { name: "열 이름" }), "빈열");
    fireEvent.keyDown(screen.getByRole("textbox", { name: "열 이름" }), { key: "Escape" });
    expect(await screen.findByText("저장할 수 없습니다. 식을 정하지 않은 열이 1개 있습니다.")).toBeTruthy();
    const name = screen.getByRole("textbox", { name: "뷰 이름" });
    fireEvent.change(name, { target: { value: "계약서류" } });
    expect(screen.getByText("저장할 수 없습니다. ‘계약서류’ 뷰가 이미 있습니다.")).toBeTruthy();
  });

  it("저장은 §4 꼴의 정의를 saveUserView 로 보낸다 — 기준이 없고 열의 가공식에는 id 를 싣지 않는다", async () => {
    const { user, record } = await 뷰열기();
    줄(record, "공고번호").focus();
    await user.keyboard("{Alt>}{ArrowUp}{/Alt}");
    expect(줄이름(record).slice(0, 2)).toEqual(["공고번호", "접수번호"]);
    await user.keyboard("{Alt>}{ArrowDown}{/Alt}{Alt>}{ArrowDown}{/Alt}");
    expect(줄이름(record).slice(0, 3)).toEqual(["접수번호", "계약번호", "공고번호"]);
    줄(record, "진행").focus();
    await user.keyboard("{Delete}");
    expect(줄이름(record)).not.toContain("진행");
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.getByText("변경됨")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "저장" }));
    await waitFor(() => expect(부름.mock.calls.some(([m]) => m === "saveUserView")).toBe(true));
    const [json, previousId] = 부름.mock.calls.find(([m]) => m === "saveUserView")![1] as [string, string];
    expect(previousId).toBe("uv-0001");
    const sent = JSON.parse(json);
    expect(sent).toMatchObject({ name: "공고문서", definition: { version: 2, contracts: "fold" } });
    expect(sent).not.toHaveProperty("base");
    expect(sent.definition.columns.map((c: { name: string }) => c.name)).toEqual(["접수번호", "계약번호", "공고번호", "공고명", "대표품명"]);
    expect(sent.definition.columns[0]).toEqual({ id: "c-0001", name: "접수번호",
      formula: { version: 2, source: "flow", flow: { from: { kind: "linked", link: "request", field: "접수번호" }, steps: [] } } });
    expect(await screen.findByText("‘공고문서’ 뷰를 저장했습니다.")).toBeTruthy();
    expect(screen.queryByText("변경됨")).toBeNull();
    const table = within(await screen.findByRole("grid", { name: "자료 목록" }));
    expect(table.queryByRole("columnheader", { name: /진행/ })).toBeNull();
    expect(screen.queryByRole("region", { name: "미리보기" })).toBeNull();
  });

  it("뷰를 지우기 전에 이름과 서식이 잃는 것을 말한다", async () => {
    const onViewsChange = vi.fn();
    const { user } = await 뷰열기({ onViewsChange });
    await user.click(screen.getByRole("button", { name: "뷰 지우기" }));
    const dialog = within(await screen.findByRole("dialog", { name: "뷰 지우기 확인" }));
    expect(dialog.getByRole("heading", { name: "‘공고문서’ 뷰를 지울까요?" })).toBeTruthy();
    expect(dialog.getByText("이 뷰를 쓰는 서식은 u_공고문서 를 더 찾지 못합니다.")).toBeTruthy();
    expect(dialog.getByText("뷰의 구성만 지우며 되돌릴 수 없습니다.")).toBeTruthy();
    await user.click(dialog.getByRole("button", { name: "뷰 지우기" }));
    await waitFor(() => expect(부름).toHaveBeenCalledWith("deleteUserView", ["uv-0001"]));
    expect(await screen.findByText("‘공고문서’ 뷰를 지웠습니다.")).toBeTruthy();
    expect(screen.queryByRole("button", { name: /^공고문서/ })).toBeNull();
    expect(await screen.findByRole("grid", { name: "자료 목록" })).toBeTruthy();
    expect(screen.getByRole("heading", { level: 1, name: "계약서류" })).toBeTruthy();
    expect(onViewsChange.mock.lastCall![0].map((view: { name: string }) => view.name)).toEqual(["계약서류", "문서 서식"]);
  });

  it("문서 서식은 「뷰 편집」 에서 한 번 묻고 제자리에서 고쳐 저장한다 — 지우기 · 「복사해서 고치기」 는 없다", async () => {
    const user = userEvent.setup();
    render(<UserViews readOnly={false} initialViewId="pclm-document-view" />);
    await screen.findByRole("heading", { level: 1, name: "문서 서식" });
    expect(screen.queryByRole("button", { name: "복사해서 고치기" })).toBeNull();
    expect(screen.queryByRole("button", { name: "뷰 지우기" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "뷰 편집" }));
    let dialog = within(await screen.findByRole("dialog", { name: "문서 서식 고치기" }));
    expect(dialog.getByText("이 뷰를 고치면 확장 프로그램에서 만드는 문서에 값이 다르게 들어갈 수 있습니다.")).toBeTruthy();
    await user.click(dialog.getByRole("button", { name: "취소" }));
    expect(screen.queryByRole("textbox", { name: "뷰 이름" })).toBeNull();
    // 들어갈 때마다 묻는다.
    await user.click(screen.getByRole("button", { name: "뷰 편집" }));
    dialog = within(await screen.findByRole("dialog", { name: "문서 서식 고치기" }));
    await user.click(dialog.getByRole("button", { name: "고치기" }));
    const 이름 = await screen.findByRole("textbox", { name: "뷰 이름" }) as HTMLInputElement;
    expect(이름.disabled).toBe(false);
    expect(screen.queryByRole("button", { name: "뷰 지우기" })).toBeNull();
    await user.clear(이름);
    await user.type(이름, "내 서식");
    await user.click(screen.getByRole("button", { name: "저장" }));
    await waitFor(() => expect(부름).toHaveBeenCalledWith("saveUserView", [expect.stringContaining("\"내 서식\""), "pclm-document-view"]));
    expect(await screen.findByText("‘내 서식’ 뷰를 저장했습니다.")).toBeTruthy();
  });

  it("저장 전의 새 보기는 지우기 대신 「취소」 로 만들던 것을 버린다고 묻고, 첫 보기로 돌아간다", async () => {
    const { user } = await 뷰열기();
    await 새뷰(user);
    await user.type(screen.getByRole("textbox", { name: "뷰 이름" }), "월간보고");
    expect(screen.queryByRole("button", { name: "뷰 지우기" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "취소" }));
    let dialog = within(await screen.findByRole("dialog", { name: "만들던 뷰 버리기 확인" }));
    expect(dialog.getByRole("heading", { name: "만들던 뷰를 버릴까요?" })).toBeTruthy();
    expect(dialog.getByText("저장하지 않은 ‘월간보고’ 의 이름과 열 구성을 버리고 ‘공고문서’ 뷰로 돌아갑니다.")).toBeTruthy();
    expect(screen.queryByText(/를 더 찾지 못합니다/)).toBeNull();
    await user.click(dialog.getByRole("button", { name: "계속 만들기" }));
    expect(screen.getByRole("textbox", { name: "뷰 이름" })).toHaveProperty("value", "월간보고");

    await user.click(screen.getByRole("button", { name: "취소" }));
    dialog = within(await screen.findByRole("dialog", { name: "만들던 뷰 버리기 확인" }));
    await user.click(dialog.getByRole("button", { name: "버리기" }));
    expect(await screen.findByRole("heading", { level: 1, name: "공고문서" })).toBeTruthy();
    expect(부름.mock.calls.some(([m]) => m === "deleteUserView")).toBe(false);
  });

  it("담은 열의 식을 고쳐도 체크가 그 열을 가리킨다 — 풀면 그 열이 빠지고, 다시 체크해도 겹치지 않는다", async () => {
    const { user, record } = await 뷰열기();
    const 계약번호 = () => within(열보기().getByRole("region", { name: "계약" })).getByRole("checkbox", { name: "계약번호" });
    await waitFor(() => expect(계약번호()).toHaveProperty("checked", true));
    줄(record, "계약번호").focus();
    await user.keyboard("{Enter}");
    const editor = within(await screen.findByRole("region", { name: "열 편집" }));
    await user.click(editor.getByRole("tab", { name: "흐름식" }));
    const code = await editor.findByRole("textbox", { name: "흐름식" });
    const view = EditorView.findFromDOM(code.closest(".cm-editor") as HTMLElement)!;
    act(() => { view.dispatch({ changes: { from: view.state.doc.length, insert: " |> 다듬기" } }); });
    await user.click(editor.getByRole("button", { name: "공고문서" }));
    const 다시 = within(await screen.findByRole("region", { name: "미리보기" }));
    expect(줄(다시, "계약번호").querySelector(".uv-formula")!.textContent).toContain("다듬기");
    expect(계약번호()).toHaveProperty("checked", true);
    await user.click(계약번호());
    expect(줄이름(다시)).not.toContain("계약번호");
    await user.click(계약번호());
    expect(줄이름(다시).filter(n => n === "계약번호")).toHaveLength(1);
  });

  it("저장하지 않은 변경을 두고 떠나면 열 관리와 같은 확인을 거친다", async () => {
    const { user, record, view } = await 뷰열기();
    줄(record, "진행").focus();
    await user.keyboard("{Delete}");
    // 보기에서 다른 뷰로.
    await user.click(within(screen.getByRole("navigation", { name: "뷰" })).getByRole("button", { name: /^계약서류/ }));
    const guard = within(await screen.findByRole("dialog", { name: "저장하지 않은 뷰 변경" }));
    expect(guard.getByText("‘공고문서’ 뷰의 변경을 저장하고 이동할까요?")).toBeTruthy();
    await user.click(guard.getByRole("button", { name: "계속 편집" }));
    expect(줄이름(record)).not.toContain("진행");
    // 사이드바처럼 작업 영역 밖에서 떠나려 해도 같다.
    const proceed = vi.fn();
    view.rerender(<UserViews readOnly={false} navigationRequest={{ id: 1, proceed }} />);
    await user.click(within(await screen.findByRole("dialog", { name: "저장하지 않은 뷰 변경" })).getByRole("button", { name: "변경 버리기" }));
    expect(proceed).toHaveBeenCalledTimes(1);
  });

  it("열람 중이면 보기를 고칠 수 없다고 말하고 고치는 단추를 세우지 않는다", async () => {
    render(<UserViews readOnly />);
    expect(await screen.findByRole("grid", { name: "자료 목록" })).toBeTruthy();
    expect(screen.getByText("열람 중이라 뷰를 고칠 수 없습니다.")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "뷰 만들기" })).toBeNull();
    expect(screen.queryByRole("button", { name: "저장" })).toBeNull();
    expect(screen.queryByRole("button", { name: "식으로 만들기" })).toBeNull();
    expect(screen.queryByRole("button", { name: "뷰 편집" })).toBeNull();
  });
});

describe("열 관리와 보기", () => {
  it("보기가 쓰는 열을 지우려 하면 몇 곳이 쓰는지와 무엇이 깨지는지 말한다", async () => {
    const user = userEvent.setup();
    render(<ColumnManager columns={[{ entityType: "notice", fieldName: "검토여부", kind: "text", choices: [], viewReaders: 1 }]} initialEntity="notice"
      onClose={vi.fn()} onAddColumn={vi.fn()} onUpdateColumn={vi.fn()} onRemoveColumn={vi.fn()} onMoveColumn={vi.fn()} onSaveComputed={vi.fn()} onFreezeColumn={vi.fn()} />);
    await user.click(await screen.findByRole("button", { name: /^검토여부 · 입력/ }));
    await user.click(screen.getByRole("button", { name: "열 메뉴" }));
    await user.click(screen.getByRole("menuitem", { name: "열 제거…" }));
    const dialog = await screen.findByRole("dialog", { name: "열 제거 확인" });
    expect(dialog.textContent).toContain("뷰 1곳이 이 열을 씁니다. 지우면 그 열은 식이 깨져 빈 값으로 나갑니다.");
  });
});

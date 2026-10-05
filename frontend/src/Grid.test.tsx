import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { Grid } from "./Grid";
import type { Sheet, UserColumn } from "./types";

// jsdom 에는 없다. 커서를 따라 굴리는 코드가 여기서 걸려 넘어지지 않게 둔다.
beforeAll(() => {
  Element.prototype.scrollIntoView = () => {};
});

// globals 를 켜지 않아 자동 정리가 걸리지 않는다. 안 치우면 렌더가 쌓여 요소가 겹쳐 잡힌다.
afterEach(cleanup);

const columns: UserColumn[] = [
  { entityType: "contract", fieldName: "담당", kind: "text", choices: [] },
  { entityType: "contract", fieldName: "진행상태", kind: "choice", choices: ["준비", "계약", "종료"] },
];

function sheet(): Sheet {
  return {
    name: "v_통합",
    columns: ["계약번호", "계약건명", "계약금액", "담당", "진행상태"],
    // 사람이 세운 열과 파서가 읽은 칸은 담기는 표가 달라 길이 갈린다.
    editable: ["담당", "진행상태"],
    correctable: ["계약건명", "계약금액"],
    // 둘째 줄의 금액은 이미 손으로 고쳐져 있다 — 표시와 되돌리기를 볼 자리다.
    overrides: { "R26TA0200": { 계약금액: "1,000원" } },
    rows: [
      { 계약번호: "R26TA0100", 계약건명: "수질측정기", 계약금액: "900", 담당: "김담당", 진행상태: "계약" },
      { 계약번호: "R26TA0200", 계약건명: "공압식승강판", 계약금액: "1,000", 담당: "", 진행상태: "준비" },
      { 계약번호: "R26TA0300", 계약건명: "성분측정기", 계약금액: "80", 담당: "박담당", 진행상태: "종료" },
    ],
  };
}

type Hands = {
  onEdit?: (key: string, field: string, value: string) => Promise<void>;
  onCorrect?: (key: string, field: string, value: string) => Promise<void>;
  onRevert?: (key: string, field: string) => Promise<void>;
  onDelete?: (key: string) => void;
};

/** 표를 그리고, 바깥이 하듯 성공한 편집만 sheet 에 되먹인다. */
function draw(hands: Hands = {}) {
  const data = sheet();

  const props = {
    columns,
    keyColumn: "계약번호",
    identityColumns: ["계약번호"],
    onEdit: hands.onEdit ?? vi.fn().mockResolvedValue(undefined),
    onCorrect: hands.onCorrect ?? vi.fn().mockResolvedValue(undefined),
    onRevert: hands.onRevert ?? vi.fn().mockResolvedValue(undefined),
    onDelete: hands.onDelete ?? vi.fn(),
  };

  const view = render(<Grid sheet={data} {...props} />);
  const redraw = (next: Sheet) => view.rerender(<Grid sheet={next} {...props} />);

  return { data, redraw };
}

function cell(rowIndex: number, text: string) {
  const row = screen.getAllByRole("row")[rowIndex + 1]; // 0행은 머리글
  return within(row).getByText(text);
}

describe("Grid", () => {
  /**
   * 이 표가 지켜야 할 첫째 약속.
   *
   * 예전에는 입력 상자가 비제어라, 저장이 거부돼도 사람이 친 글자가 화면에 그대로 남았다.
   * 화면은 새 값을, DB 는 옛 값을 들고 조용히 갈라졌다 — 오류도 나지 않았다.
   */
  it("저장에 실패하면 화면이 옛 값으로 돌아간다", async () => {
    const user = userEvent.setup();
    const onEdit = vi.fn().mockRejectedValue(new Error("넣지 못했습니다"));
    draw({ onEdit });

    await user.click(cell(0, "김담당"));
    await user.keyboard("{F2}");
    await user.clear(screen.getByRole("textbox"));
    await user.keyboard("이담당{Enter}");

    expect(onEdit).toHaveBeenCalledWith("R26TA0100", "담당", "이담당");
    expect(screen.queryByText("이담당")).toBeNull();
    expect(cell(0, "김담당")).toBeTruthy();
  });

  it("저장에 성공하면 그 자리에서 값이 바뀐다", async () => {
    const user = userEvent.setup();
    const onEdit = vi.fn().mockResolvedValue(undefined);
    const { data, redraw } = draw({ onEdit });

    await user.click(cell(0, "김담당"));
    await user.keyboard("{F2}");
    await user.clear(screen.getByRole("textbox"));
    await user.keyboard("이담당{Enter}");

    // 바깥이 sheet 를 제자리에서 갈아 끼운다 — 표 전체를 다시 읽지 않는다.
    data.rows[0]["담당"] = "이담당";
    redraw({ ...data, rows: [...data.rows] });

    expect(cell(0, "이담당")).toBeTruthy();
  });

  it("Esc 로 물리면 옛 값이 그대로다", async () => {
    const user = userEvent.setup();
    const onEdit = vi.fn().mockResolvedValue(undefined);
    draw({ onEdit });

    await user.click(cell(0, "김담당"));
    await user.keyboard("{F2}");
    await user.clear(screen.getByRole("textbox"));
    await user.keyboard("이담당{Escape}");

    expect(onEdit).not.toHaveBeenCalled();
    expect(cell(0, "김담당")).toBeTruthy();
  });

  /** Enter 로 넣으면 초점이 표로 돌아가며 blur 가 뒤따른다. 막지 않으면 두 번 저장된다. */
  it("Enter 로 넣으면 꼭 한 번만 저장된다", async () => {
    const user = userEvent.setup();
    const onEdit = vi.fn().mockResolvedValue(undefined);
    draw({ onEdit });

    await user.click(cell(0, "김담당"));
    await user.keyboard("{F2}");
    await user.clear(screen.getByRole("textbox"));
    await user.keyboard("이담당{Enter}");

    expect(onEdit).toHaveBeenCalledTimes(1);
  });

  /** 레코드를 가리키는 열이라 고치면 값이 바뀌는 것이 아니라 레코드가 옮겨간다. */
  it("키 열은 타자로 편집에 들어가지 않는다", async () => {
    const user = userEvent.setup();
    const onEdit = vi.fn().mockResolvedValue(undefined);
    const onCorrect = vi.fn().mockResolvedValue(undefined);
    draw({ onEdit, onCorrect });

    await user.click(cell(0, "R26TA0100"));
    await user.keyboard("망가뜨리기");

    expect(screen.queryByRole("textbox")).toBeNull();
    expect(onEdit).not.toHaveBeenCalled();
    expect(onCorrect).not.toHaveBeenCalled();
  });

  // ── 파서가 읽은 값 고치기 ──────────────────────────────
  // 화면에서는 사람 열과 똑같이 타자를 치지만, 담기는 표가 다르므로 길이 갈려야 한다.

  it("파서가 읽은 칸을 고치면 덮개 길로 간다", async () => {
    const user = userEvent.setup();
    const onEdit = vi.fn().mockResolvedValue(undefined);
    const onCorrect = vi.fn().mockResolvedValue(undefined);
    draw({ onEdit, onCorrect });

    await user.click(cell(0, "수질측정기"));
    await user.keyboard("{F2}");
    await user.clear(screen.getByRole("textbox"));
    await user.keyboard("수질측정기(정정){Enter}");

    expect(onCorrect).toHaveBeenCalledWith("R26TA0100", "계약건명", "수질측정기(정정)");
    expect(onEdit).not.toHaveBeenCalled();
  });

  it("사람이 세운 열은 덮개 길로 가지 않는다", async () => {
    const user = userEvent.setup();
    const onEdit = vi.fn().mockResolvedValue(undefined);
    const onCorrect = vi.fn().mockResolvedValue(undefined);
    draw({ onEdit, onCorrect });

    await user.click(cell(0, "김담당"));
    await user.keyboard("{F2}");
    await user.clear(screen.getByRole("textbox"));
    await user.keyboard("이담당{Enter}");

    expect(onEdit).toHaveBeenCalledWith("R26TA0100", "담당", "이담당");
    expect(onCorrect).not.toHaveBeenCalled();
  });

  /**
   * 잘못 읽힌 칸을 <b>비우는 것</b>과 파서 값으로 <b>돌아가는 것</b>은 다른 일이다.
   * 한 키에 둘을 얹으면 앞의 것을 할 수 없다.
   */
  it("Delete 는 빈 값을 넣고 Ctrl+Delete 는 되돌린다", async () => {
    const user = userEvent.setup();
    const onCorrect = vi.fn().mockResolvedValue(undefined);
    const onRevert = vi.fn().mockResolvedValue(undefined);
    draw({ onCorrect, onRevert });

    await user.click(cell(1, "1,000"));
    await user.keyboard("{Delete}");

    expect(onCorrect).toHaveBeenCalledWith("R26TA0200", "계약금액", "");
    expect(onRevert).not.toHaveBeenCalled();

    await user.keyboard("{Control>}{Delete}{/Control}");

    expect(onRevert).toHaveBeenCalledWith("R26TA0200", "계약금액");
  });

  it("고치지 않은 칸에서는 되돌릴 것이 없다", async () => {
    const user = userEvent.setup();
    const onRevert = vi.fn().mockResolvedValue(undefined);
    draw({ onRevert });

    await user.click(cell(0, "900"));
    await user.keyboard("{Control>}{Delete}{/Control}");

    expect(onRevert).not.toHaveBeenCalled();
  });

  /** 무엇이 문서에서 읽은 값이고 무엇이 사람이 적은 것인지 보이지 않으면 안전하지 않다. */
  it("고친 칸에는 원래 값이 표시로 붙는다", () => {
    draw();

    const row = screen.getAllByRole("row")[2];
    const 금액 = within(row).getAllByRole("cell")[2];

    expect(금액.getAttribute("title")).toContain("1,000원");
    expect(금액.className).toContain("overridden");
  });

  it("고치지 않은 칸에는 표시가 붙지 않는다", () => {
    draw();

    const row = screen.getAllByRole("row")[1];
    expect(within(row).getAllByRole("cell")[2].className).not.toContain("overridden");
  });

  it("지우기는 커서가 선 줄의 키로 묻는다", async () => {
    const user = userEvent.setup();
    const onDelete = vi.fn();
    draw({ onDelete });

    await user.click(cell(1, "공압식승강판"));
    await user.click(screen.getByText("선택 자료 삭제…"));

    expect(onDelete).toHaveBeenCalledWith("R26TA0200");
  });

  it("타자를 치면 그 글자로 편집이 시작된다", async () => {
    const user = userEvent.setup();
    draw();

    await user.click(cell(0, "김담당"));
    await user.keyboard("최");

    expect(screen.getByRole("textbox")).toHaveProperty("value", "최");
  });

  it("화살표로 커서가 옮겨 간다", async () => {
    const user = userEvent.setup();
    draw();

    await user.click(cell(0, "김담당"));
    await user.keyboard("{ArrowDown}{F2}");

    // 한 칸 아래는 둘째 줄의 담당(빈 값)이다.
    expect(screen.getByRole("textbox")).toHaveProperty("value", "");
  });

  /** 금액은 자릿점이 붙은 문자열이라, 문자열로 세우면 1,000 이 900 보다 앞선다. */
  it("금액 열은 수로 정렬한다", async () => {
    const user = userEvent.setup();
    draw();

    await user.click(screen.getByText("계약금액"));

    const shown = screen.getAllByRole("row").slice(1)
      .map((r) => within(r).getAllByRole("cell")[2].textContent);

    expect(shown).toEqual(["80", "900", "1,000"]);
  });

  it("찾기가 행을 거르고 결과가 없으면 검색을 지워 복구한다", async () => {
    const user = userEvent.setup();
    draw();

    await user.type(screen.getByPlaceholderText("찾기 (Ctrl+F)"), "공압식");

    const shown = screen.getAllByRole("row").slice(1);
    expect(shown).toHaveLength(1);
    expect(within(shown[0]).getByText("공압식승강판")).toBeTruthy();
    await user.type(screen.getByRole("searchbox"), "없는검색어");
    expect(screen.getByRole("heading", { name: "검색 결과가 없습니다" })).toBeTruthy();
    await user.tab();
    await user.tab();
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "검색 지우기" }));
    await user.keyboard("{Enter}");
    expect(screen.getAllByRole("row").slice(1)).toHaveLength(3);
    expect(document.activeElement).toBe(screen.getByRole("searchbox"));
  });

  // ── 커서는 줄을 붙든다 ──────────────────────────────
  // 창에 돌아올 때마다 바깥이 표를 다시 읽고, 정렬 중에 저장하면 다시 세워진다. 커서가 자리
  // 번호만 들면 같은 번호가 다른 레코드를 가리켜, 편집하던 글자가 오류 없이 남의 줄에 저장된다.

  it("편집 중에 줄 차례가 바뀌어도 원래 줄에 저장된다", async () => {
    const user = userEvent.setup();
    const onEdit = vi.fn().mockResolvedValue(undefined);
    const { data, redraw } = draw({ onEdit });

    await user.click(cell(0, "김담당"));
    await user.keyboard("{F2}");
    await user.clear(screen.getByRole("textbox"));
    await user.keyboard("이담당");

    redraw({ ...data, rows: [data.rows[2], data.rows[0], data.rows[1]] });
    await user.keyboard("{Enter}");

    expect(onEdit).toHaveBeenCalledTimes(1);
    expect(onEdit).toHaveBeenCalledWith("R26TA0100", "담당", "이담당");
  });

  it("편집 중에 위에 줄이 새로 서도 친 글자가 남고 원래 줄에 저장된다", async () => {
    const user = userEvent.setup();
    const onEdit = vi.fn().mockResolvedValue(undefined);
    const { data, redraw } = draw({ onEdit });

    await user.click(cell(0, "김담당"));
    await user.keyboard("{F2}");
    await user.clear(screen.getByRole("textbox"));
    await user.keyboard("이담당");

    const 새줄 = { 계약번호: "R26TA0050", 계약건명: "새로온건", 계약금액: "10", 담당: "최담당", 진행상태: "준비" };
    redraw({ ...data, rows: [새줄, ...data.rows] });

    const box = screen.getByRole("textbox");
    expect(box).toHaveProperty("value", "이담당");
    expect(within(screen.getAllByRole("row")[2]).getByText("R26TA0100")).toBeTruthy();
    expect(screen.getAllByRole("row")[2].contains(box)).toBe(true);

    await user.keyboard("{Enter}");

    expect(onEdit).toHaveBeenCalledTimes(1);
    expect(onEdit).toHaveBeenCalledWith("R26TA0100", "담당", "이담당");
  });

  it("머리글로 정렬해도 커서는 그 줄에 남는다", async () => {
    const user = userEvent.setup();
    const onCorrect = vi.fn().mockResolvedValue(undefined);
    const onDelete = vi.fn();
    draw({ onCorrect, onDelete });

    await user.click(cell(0, "수질측정기"));
    await user.click(screen.getByText("계약금액")); // 80 · 900 · 1,000 — 첫 줄이 가운데로 간다

    expect(screen.getByText("2 / 3")).toBeTruthy();

    await user.keyboard("{Delete}");
    expect(onCorrect).toHaveBeenCalledWith("R26TA0100", "계약건명", "");

    await user.click(screen.getByText("선택 자료 삭제…"));
    expect(onDelete).toHaveBeenCalledWith("R26TA0100");
  });

  it("상태줄이 자리와 전체 수를 알린다", async () => {
    const user = userEvent.setup();
    draw();

    await user.click(cell(1, "공압식승강판"));

    expect(screen.getByText("2 / 3")).toBeTruthy();
  });

  /**
   * 통합은 계약이 아직 없는 줄도 낸다(접수만, 접수+공고). 그 줄의 계약 열을 고치면 덮개가
   * 공고나 접수에 걸려 <b>어느 뷰도 읽지 않는 자리로 사라진다</b> — 오류 없이 값만
   * 없어지므로, 열려서는 안 되는 자리다.
   */
  it("고친 값이 매달릴 레코드가 없는 줄은 고칠 수 없다", async () => {
    const user = userEvent.setup();
    const onCorrect = vi.fn().mockResolvedValue(undefined);
    const data = sheet();

    // 계약이 아직 없는 줄. 이 줄을 가리키는 이름은 공고번호에서 온다.
    data.columns = ["계약번호", "입찰공고번호", ...data.columns.slice(1)];
    data.rows = [
      { 계약번호: "", 입찰공고번호: "R26BK01-000", 계약건명: "", 계약금액: "", 담당: "", 진행상태: "" },
    ];

    render(
      <Grid
        sheet={data}
        columns={columns}
        keyColumn="계약번호"
        identityColumns={["계약번호", "입찰공고번호"]}
        onEdit={vi.fn()}
        onCorrect={onCorrect}
        onRevert={vi.fn()}
        onDelete={vi.fn()}
      />,
    );

    await user.click(screen.getAllByRole("row")[1].querySelectorAll("td")[2]);
    await user.keyboard("{F2}고친값{Enter}");

    expect(onCorrect).not.toHaveBeenCalled();
    expect(screen.queryByText(/F2 또는 타자로 고치기/)).toBeNull();
  });

  /** 그런 줄도 이름은 있다 — 계약번호가 비면 공고번호가 그 줄을 가리킨다. */
  it("계약이 없는 줄은 다음 키로 제 이름을 갖는다", async () => {
    const user = userEvent.setup();
    const onDelete = vi.fn();
    const data = sheet();

    data.columns = ["계약번호", "입찰공고번호", ...data.columns.slice(1)];
    data.rows = [
      { 계약번호: "", 입찰공고번호: "R26BK01-000", 계약건명: "", 계약금액: "", 담당: "", 진행상태: "" },
    ];

    render(
      <Grid
        sheet={data}
        columns={columns}
        keyColumn="계약번호"
        identityColumns={["계약번호", "입찰공고번호"]}
        onEdit={vi.fn()}
        onCorrect={vi.fn()}
        onRevert={vi.fn()}
        onDelete={onDelete}
      />,
    );

    await user.click(screen.getByText("선택 자료 삭제…"));

    expect(onDelete).toHaveBeenCalledWith("R26BK01-000");
  });

  /**
   * 칸에는 너비 상한이 있다 — 조달요구번호처럼 줄줄이 이어진 한 칸이 목록을 다 차지했다.
   * 잘린 값은 커서가 서면 전체가 펼쳐져야 읽을 수 있다. jsdom 은 그리지 않으므로 잘림을
   * 흉내 낸다: 셋째 줄의 건명만 칸보다 길다.
   */
  it("잘린 칸에 커서가 서면 전체 값을 펼친다", async () => {
    const user = userEvent.setup();
    const 긴값 = "성분측정기";
    const scroll = vi.spyOn(HTMLElement.prototype, "scrollWidth", "get")
      .mockImplementation(function (this: HTMLElement) { return this.textContent === 긴값 ? 500 : 0; });
    const client = vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(100);

    try {
      draw();

      await user.click(cell(0, "수질측정기"));
      expect(screen.queryByRole("tooltip")).toBeNull();

      await user.click(cell(2, 긴값));
      expect(screen.getByRole("tooltip").textContent).toBe(긴값);

      // 커서가 떠나면 거둔다.
      await user.keyboard("{ArrowRight}");
      expect(screen.queryByRole("tooltip")).toBeNull();
    } finally {
      scroll.mockRestore();
      client.mockRestore();
    }
  });
});

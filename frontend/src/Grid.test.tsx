import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { Grid, parseClipboard } from "./Grid";
import type { BatchChange, Row, Sheet, SheetExport, UserColumn } from "./types";

// jsdom 에는 없다. 커서를 따라 굴리는 코드가 여기서 걸려 넘어지지 않게 둔다.
beforeAll(() => {
  Element.prototype.scrollIntoView = () => {};
  HTMLDialogElement.prototype.showModal = function () { this.open = true; };
  HTMLDialogElement.prototype.close = function () { this.open = false; };
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
  onDelete?: (key: string) => void;
  onDeleteRows?: (rows: Row[]) => void;
  onBatchEdit?: (changes: BatchChange[]) => Promise<void>;
  onExport?: (data?: SheetExport) => Promise<void>;
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
    onDelete: hands.onDelete ?? vi.fn(),
    onDeleteRows: hands.onDeleteRows,
    onBatchEdit: hands.onBatchEdit,
    onExport: hands.onExport,
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
  it("행 범위 선택은 정렬 후 자료를 유지하고 입력·필터와 섞이지 않는다", async () => {
    const user = userEvent.setup();
    const onDeleteRows = vi.fn(), onEdit = vi.fn(), onExport = vi.fn().mockResolvedValue(undefined);
    draw({ onDeleteRows, onEdit, onExport });
    await user.click(cell(0, "김담당"));
    await user.click(screen.getByLabelText("R26TA0100 행 선택"));
    await user.keyboard("{Shift>}");
    await user.click(screen.getByLabelText("R26TA0200 행 선택"));
    await user.keyboard("{/Shift}");
    expect(screen.getByText("2행 선택")).toBeTruthy();
    (document.querySelector(".grid-board") as HTMLElement).focus();
    await user.keyboard("잘못입력{Delete}");
    expect(screen.queryByRole("textbox")).toBeNull();
    expect(onEdit).not.toHaveBeenCalled();
    await user.click(screen.getByText("계약금액"));
    await user.click(screen.getByText("선택 자료 삭제…"));
    expect(onDeleteRows.mock.calls[0][0].map((r: Row) => r.계약번호)).toEqual(["R26TA0100", "R26TA0200"]);
    await user.click(screen.getByText("엑셀 내보내기"));
    await user.click(screen.getByText("선택한 2행 내보내기"));
    expect(onExport.mock.calls[0][0].rows.map((r: string[]) => r[0])).toEqual(["R26TA0100", "R26TA0200"]);
    await user.type(screen.getByRole("searchbox"), "수질");
    expect(screen.queryByText("2행 선택")).toBeNull();
    expect(screen.queryByText("선택 자료 삭제…")).toBeNull();
  });

  it("엑셀 붙여넣기는 범위·잠금 검토 후 한번에 저장하고 실패한 초안을 유지한다", async () => {
    const user = userEvent.setup();
    const onBatchEdit = vi.fn().mockRejectedValueOnce(new Error("자료가 변경되었습니다"))
      .mockResolvedValue(undefined);
    draw({ onBatchEdit });
    expect(parseClipboard('"여러\n줄"\t"따옴표""값"\r\n')).toEqual([["여러\n줄", '따옴표"값']]);
    await user.click(cell(0, "김담당"));
    const board = document.querySelector(".grid-board")!;
    fireEvent.paste(board, { clipboardData: { getData: () => "이담당\r\n박담당" } });
    const dialog = await screen.findByRole("dialog", { name: "일괄 변경 검토" });
    expect(onBatchEdit).not.toHaveBeenCalled();
    await user.click(within(dialog).getByRole("button", { name: "2셀 변경 적용" }));
    expect(await within(dialog).findByRole("alert")).toHaveProperty("textContent", "자료가 변경되었습니다");
    expect(onBatchEdit.mock.calls[0][0]).toEqual([
      { rowKey: "R26TA0100", field: "담당", value: "이담당", original: "김담당", operation: "set" },
      { rowKey: "R26TA0200", field: "담당", value: "박담당", original: "", operation: "set" },
    ]);
    await user.click(within(dialog).getByRole("button", { name: "2셀 변경 적용" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await user.click(cell(0, "R26TA0100"));
    fireEvent.paste(board, { clipboardData: { getData: () => "다른번호" } });
    expect(screen.getByRole("alert").textContent).toContain("수정할 수 없는 칸");
    expect(onBatchEdit).toHaveBeenCalledTimes(2);
  });

  it("Shift 셀 범위 복사와 행 선택 일괄 입력은 서로 다른 범위로 적용된다", async () => {
    const user = userEvent.setup();
    const onBatchEdit = vi.fn().mockResolvedValue(undefined);
    draw({ onBatchEdit });
    await user.click(cell(0, "김담당"));
    await user.keyboard("{Shift>}{ArrowDown}{/Shift}");
    expect(screen.getByText("2셀 선택")).toBeTruthy();
    const setData = vi.fn();
    fireEvent.copy(document.querySelector(".grid-board")!, { clipboardData: { setData } });
    expect(setData).toHaveBeenCalledWith("text/plain", "김담당\r\n");
    await user.click(screen.getByLabelText("검색 결과 3행 전체 선택"));
    expect(screen.queryByText("2셀 선택")).toBeNull();
    await user.click(screen.getByText("같은 값 입력…"));
    await user.type(screen.getByLabelText("같은 값"), "공동담당");
    await user.click(screen.getByText("변경 검토"));
    await user.click(screen.getByRole("button", { name: "3셀 변경 적용" }));
    expect(onBatchEdit.mock.calls[0][0]).toHaveLength(3);
  });

  it("행 선택 일괄 입력을 적용하면 연 단추가 걷혀도 초점이 표로 돌아온다 — body 로 떨어지지 않는다", async () => {
    const user = userEvent.setup();
    const onBatchEdit = vi.fn().mockResolvedValue(undefined);
    draw({ onBatchEdit });
    await user.click(screen.getByLabelText("검색 결과 3행 전체 선택"));
    await user.click(screen.getByText("같은 값 입력…"));
    await user.type(screen.getByLabelText("같은 값"), "공동담당");
    await user.click(screen.getByText("변경 검토"));
    await user.click(screen.getByRole("button", { name: "3셀 변경 적용" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "일괄 변경 검토" })).toBeNull());
    expect(screen.queryByText("변경 검토")).toBeNull();
    expect(document.activeElement).toBe(document.querySelector(".grid-board"));
  });

  it("범위의 「원래 값 복원…」 이 바꿀 것이 없거나 「복사」 가 성공하면 띠에 보이는 글로 알리고 잠시 뒤 걷는다 — 오류와 가른다", async () => {
    const user = userEvent.setup();
    const onBatchEdit = vi.fn().mockResolvedValue(undefined);
    draw({ onBatchEdit });
    await user.click(cell(0, "김담당"));
    await user.keyboard("{Shift>}{ArrowDown}{/Shift}");
    await user.click(screen.getByRole("button", { name: "원래 값 복원…" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(onBatchEdit).not.toHaveBeenCalled();
    const seen = () => document.querySelector(".grid-notice");
    expect(seen()!.textContent).toBe("변경할 값이 없습니다.");
    expect(seen()!.getAttribute("role")).toBeNull();
    expect(screen.getAllByRole("status").some((s) => s.textContent === "변경할 값이 없습니다.")).toBe(true);
    expect(screen.queryByRole("alert")).toBeNull();
    // 몇 초 뒤 보이는 글은 걷히고, 읽어 주던 알림은 남는다.
    await waitFor(() => expect(seen()).toBeNull(), { timeout: 5000 });
    expect(screen.getAllByRole("status").some((s) => s.textContent === "변경할 값이 없습니다.")).toBe(true);

    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText } });
    await user.click(screen.getByRole("button", { name: "복사" }));
    expect(writeText).toHaveBeenCalledWith("김담당\r\n");
    await waitFor(() => expect(seen()?.textContent).toBe("복사했습니다."));
  }, 10000);

  /** 통합 표에는 계약이 아직 없는 줄이 정상으로 섞인다 — 그 줄 하나 때문에 행 선택 일괄 입력 전체가 막히면 안 된다. */
  it("행 선택 일괄 입력은 계약 없는 줄을 건너뛰고 몇 줄인지 보이며, 범위 선택은 그대로 엄격하다", async () => {
    const user = userEvent.setup();
    const onBatchEdit = vi.fn().mockResolvedValue(undefined);
    const data = sheet();
    data.columns = ["계약번호", "입찰공고번호", ...data.columns.slice(1)];
    data.rows = [
      { ...data.rows[0], 입찰공고번호: "" },
      { 계약번호: "", 입찰공고번호: "R26BK01-000", 계약건명: "", 계약금액: "", 담당: "", 진행상태: "" },
      { ...data.rows[2], 입찰공고번호: "" },
    ];
    const view = render(<Grid sheet={data} columns={columns} keyColumn="계약번호" identityColumns={["계약번호", "입찰공고번호"]}
      onEdit={vi.fn()} onCorrect={vi.fn()} onBatchEdit={onBatchEdit} />);

    await user.click(screen.getByLabelText("검색 결과 3행 전체 선택"));
    await user.click(screen.getByText("같은 값 입력…"));
    await user.type(screen.getByLabelText("같은 값"), "공동담당");
    await user.click(screen.getByText("변경 검토"));
    const dialog = await screen.findByRole("dialog", { name: "일괄 변경 검토" });
    expect(within(dialog).getByText("계약이 없는 1행은 건너뜁니다.")).toBeTruthy();
    await user.click(within(dialog).getByRole("button", { name: "2셀 변경 적용" }));
    expect(onBatchEdit.mock.calls[0][0].map((c: BatchChange) => c.rowKey)).toEqual(["R26TA0100", "R26TA0300"]);

    // 셀 범위는 지금처럼 계약 없는 줄이 섞이면 아무것도 올리지 않는다.
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await user.click(screen.getByRole("button", { name: "선택 해제" }));
    await user.click(cell(0, "R26TA0100").closest("tr")!.querySelectorAll("td")[5]);
    await user.keyboard("{Shift>}{ArrowDown}{/Shift}{Delete}");
    expect(screen.getByRole("alert").textContent).toContain("수정할 수 없는 칸");
    expect(screen.queryByRole("dialog")).toBeNull();
    view.unmount();

    // 모두 건너뛰게 되면 지금처럼 거절한다.
    render(<Grid sheet={{ ...data, rows: [data.rows[1]] }} columns={columns} keyColumn="계약번호" identityColumns={["계약번호", "입찰공고번호"]}
      onEdit={vi.fn()} onCorrect={vi.fn()} onBatchEdit={onBatchEdit} />);
    await user.click(screen.getByLabelText("검색 결과 1행 전체 선택"));
    await user.click(screen.getByText("같은 값 입력…"));
    await user.type(screen.getByLabelText("같은 값"), "공동담당");
    await user.click(screen.getByText("변경 검토"));
    expect(screen.getByRole("alert").textContent).toContain("수정할 수 없는 칸");
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(onBatchEdit).toHaveBeenCalledTimes(1);
  });

  it("편집 중 다른 셀을 누르면 기존 입력을 저장하고 새 셀로 이동한다", async () => {
    const user = userEvent.setup();
    const onEdit = vi.fn().mockResolvedValue(undefined);
    draw({ onEdit });
    await user.click(cell(0, "김담당"));
    await user.keyboard("{F2}");
    await user.clear(screen.getByRole("textbox"));
    await user.type(screen.getByRole("textbox"), "다른담당");
    await user.click(cell(2, "박담당"));
    expect(onEdit).toHaveBeenCalledTimes(1);
    expect(onEdit).toHaveBeenCalledWith("R26TA0100", "담당", "다른담당");
  });
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

  it("저장하는 동안 친 글자는 버리지 않고, 끝나면 지금 칸의 편집으로 이어 받는다", async () => {
    const user = userEvent.setup();
    let 끝 = () => {};
    const onEdit = vi.fn(() => new Promise<void>(resolve => { 끝 = resolve; }));
    draw({ onEdit });

    await user.click(cell(0, "김담당"));
    await user.keyboard("{F2}");
    await user.clear(screen.getByRole("textbox"));
    await user.keyboard("이담당{Enter}");
    // 아래 줄(담당이 빈 칸)로 내려와 저장이 끝나기 전에 친다.
    await user.keyboard("정담");
    expect(screen.queryByRole("textbox")).toBeNull();
    끝();
    await waitFor(() => expect(screen.getByRole("textbox")).toHaveProperty("value", "정담"));
    expect(onEdit).toHaveBeenCalledTimes(1);
  });

  it("「엑셀 내보내기」 펼침은 바깥을 누르거나 Esc 로 접힌다", async () => {
    const user = userEvent.setup();
    draw({ onExport: vi.fn().mockResolvedValue(undefined) });
    const summary = screen.getByLabelText("엑셀 내보내기");
    const menu = summary.closest("details") as HTMLDetailsElement;
    await user.click(summary);
    expect(menu.open).toBe(true);
    await user.click(cell(0, "김담당"));
    expect(menu.open).toBe(false);
    await user.click(summary);
    expect(menu.open).toBe(true);
    screen.getByRole("button", { name: /^검색 결과 3행 내보내기$/ }).focus();
    await user.keyboard("{Escape}");
    expect(menu.open).toBe(false);
    expect(document.activeElement).toBe(summary);
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
  it("Delete 는 빈 값을 넣고 Ctrl+Delete 는 검토 창을 거쳐 되돌린다", async () => {
    const user = userEvent.setup();
    const onCorrect = vi.fn().mockResolvedValue(undefined);
    const onBatchEdit = vi.fn().mockResolvedValue(undefined);
    draw({ onCorrect, onBatchEdit });

    await user.click(cell(1, "1,000"));
    await user.keyboard("{Delete}");

    expect(onCorrect).toHaveBeenCalledWith("R26TA0200", "계약금액", "");
    expect(onBatchEdit).not.toHaveBeenCalled();

    // 사람이 고친 값을 지우는 일이라 칸 하나여도 범위와 같은 검토 창을 거친다 — 확인 전에는 아무것도 지우지 않는다.
    await user.keyboard("{Control>}{Delete}{/Control}");
    const dialog = await screen.findByRole("dialog", { name: "일괄 변경 검토" });
    expect(within(dialog).getByText("수집 원래 값")).toBeTruthy();
    expect(onBatchEdit).not.toHaveBeenCalled();

    await user.click(within(dialog).getByRole("button", { name: "1셀 변경 적용" }));
    expect(onBatchEdit).toHaveBeenCalledWith([
      { rowKey: "R26TA0200", field: "계약금액", value: "", original: "1,000", operation: "revert" },
    ]);
  });

  it("Ctrl+Backspace 검토 창을 취소하면 아무것도 지우지 않는다", async () => {
    const user = userEvent.setup();
    const onBatchEdit = vi.fn().mockResolvedValue(undefined);
    draw({ onBatchEdit });

    await user.click(cell(1, "1,000"));
    await user.keyboard("{Control>}{Backspace}{/Control}");
    const dialog = await screen.findByRole("dialog", { name: "일괄 변경 검토" });
    await user.click(within(dialog).getByRole("button", { name: "취소" }));

    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(onBatchEdit).not.toHaveBeenCalled();
  });

  it("고치지 않은 칸이나 고칠 수 없는 표에서는 되돌릴 것이 없다", async () => {
    const user = userEvent.setup();
    const onBatchEdit = vi.fn().mockResolvedValue(undefined);
    draw({ onBatchEdit });

    await user.click(cell(0, "900"));
    await user.keyboard("{Control>}{Delete}{/Control}");

    expect(screen.queryByRole("dialog")).toBeNull();
    expect(onBatchEdit).not.toHaveBeenCalled();
    cleanup();

    // 일괄 편집 손잡이가 없는 표(열람·차수)에서는 고친 칸이어도 그대로다.
    draw();
    await user.click(cell(1, "1,000"));
    await user.keyboard("{Control>}{Delete}{/Control}");
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  /** 무엇이 문서에서 읽은 값이고 무엇이 사람이 적은 것인지 보이지 않으면 안전하지 않다. */
  it("고친 칸에는 원래 값이 표시로 붙는다", () => {
    draw();

    const row = screen.getAllByRole("row")[2];
    const 금액 = within(row).getAllByRole("cell")[3];

    expect(금액.getAttribute("title")).toContain("1,000원");
    expect(금액.className).toContain("overridden");
  });

  it("고치지 않은 칸에는 표시가 붙지 않는다", () => {
    draw();

    const row = screen.getAllByRole("row")[1];
    expect(within(row).getAllByRole("cell")[3].className).not.toContain("overridden");
  });

  it("지우기는 커서가 아닌 체크한 줄의 키로 묻는다", async () => {
    const user = userEvent.setup();
    const onDelete = vi.fn();
    draw({ onDelete });

    await user.click(cell(1, "공압식승강판"));
    expect(screen.queryByText("선택 자료 삭제…")).toBeNull();
    await user.click(screen.getByLabelText("R26TA0200 행 선택"));
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
      .map((r) => within(r).getAllByRole("cell")[3].textContent);

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

    (document.querySelector(".grid-board") as HTMLElement).focus();
    await user.keyboard("{Delete}");
    expect(onCorrect).toHaveBeenCalledWith("R26TA0100", "계약건명", "");

    await user.click(screen.getByLabelText("R26TA0100 행 선택"));
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
        onDelete={vi.fn()}
      />,
    );

    await user.click(screen.getAllByRole("row")[1].querySelectorAll("td")[3]);
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
        onDelete={onDelete}
      />,
    );

    await user.click(screen.getByLabelText("R26BK01-000 행 선택"));
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

/**
 * 가공열(열 관리 §6.5). 머리글 앞에 = 가 서고, 칸을 고치려 하면 편집 상자 대신 식 첫 줄과
 * 「식 고치기」 가 뜬다 — 값은 식이 채우므로 칸을 고치는 길은 없다.
 */
describe("Grid · 가공열", () => {
  const computed: UserColumn = {
    entityType: "contract", fieldName: "품명과 번호", kind: "computed", choices: [],
    formula: { version: 2, id: "f1", source: "flow", flow: { from: { kind: "items" }, steps: [{ op: "get", field: "품명" }, { op: "join", text: ", " }] } },
  };
  const data = (): Sheet => ({ ...sheet(), columns: [...sheet().columns, "품명과 번호"],
    rows: sheet().rows.map(r => ({ ...r, "품명과 번호": "책상, 의자" })) });

  it("머리글에 = 를 두고, F2·두 번 누르기는 안내를 띄워 식 고치기로 보낸다", async () => {
    const user = userEvent.setup();
    const onManageColumns = vi.fn(), onEdit = vi.fn(), onCorrect = vi.fn();
    render(<Grid sheet={data()} columns={[...columns, computed]} keyColumn="계약번호" identityColumns={["계약번호"]}
      onEdit={onEdit} onCorrect={onCorrect} onManageColumns={onManageColumns} />);

    const head = screen.getByRole("columnheader", { name: /품명과 번호/ });
    expect(head.textContent?.startsWith("=")).toBe(true);
    expect(screen.getByRole("columnheader", { name: /^담당/ }).textContent?.startsWith("=")).toBe(false);

    await user.dblClick(cell(0, "책상, 의자"));
    const note = await screen.findByRole("dialog", { name: "가공열 안내" });
    expect(note.textContent).toContain("= 품목 |> 꺼내기 품명 |> 잇기 \", \"");
    expect(screen.queryByRole("textbox")).toBeNull();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog", { name: "가공열 안내" })).toBeNull();

    await user.click(cell(1, "책상, 의자"));
    await user.keyboard("{F2}");
    await user.click(within(await screen.findByRole("dialog", { name: "가공열 안내" })).getByRole("button", { name: "식 고치기" }));
    expect(onManageColumns).toHaveBeenCalledWith("품명과 번호");
    expect(onEdit).not.toHaveBeenCalled();
    expect(onCorrect).not.toHaveBeenCalled();
  });
});

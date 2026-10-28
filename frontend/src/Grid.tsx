import { useCallback, useEffect, useId, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { BatchChange, Row, Sheet, SheetExport, UserColumn } from "./types";
import { Modal } from "./Modal";
import "../css/grid-work.css";
import { useFindShortcut } from "./useFindShortcut";
import { toV2 } from "./flow/migrate";
import { printFlow } from "./flow/syntax";

/** 가공열 안내에 찍을 식 첫 줄. */
function formulaLine(column: UserColumn | undefined): string {
  if (!column?.formula) return "";
  try {
    const f = toV2(column.formula);
    return (f.source === "sql" ? f.sql.trim() : printFlow(f.flow)).split("\n")[0];
  } catch { return ""; }
}

/** 값이 숫자로 읽히면 오른쪽으로 붙인다 — 금액 자릿수가 눈으로 맞아야 한다. */
function isNumeric(value: string): boolean {
  return value.length > 0 && /^[\d,.\-]+$/.test(value);
}

/** 자릿점을 떼고 수로. 정렬에 쓴다 — 문자열로 세우면 1,000 이 900 보다 앞선다. */
function asNumber(value: string): number | null {
  if (!isNumeric(value)) return null;
  const n = Number(value.replaceAll(",", ""));
  return Number.isFinite(n) && (!Number.isInteger(n) || Number.isSafeInteger(n)) ? n : null;
}

type Props = {
  initialQuery?: string;
  /** 처음 검색어가 있으면 검색 칸에 초점을 줄지. 대시보드에서 막 왔을 때만 준다 — 보기 전환이 이어 준 검색어는 초점을 옮기지 않는다. */
  focusQuery?: boolean;
  /** 검색어가 바뀔 때마다 — 보기를 바꿔 표가 새로 서도 바깥이 지금 검색어를 이어 줄 수 있게. */
  onQueryChange?: (query: string) => void;
  sheet: Sheet;
  columns: UserColumn[];
  /**
   * 고칠 값이 매달리는 자리. 통합에서는 계약번호다 — 손댈 수 있는 것이 계약 쪽 열뿐이라
   * 그 줄의 계약을 가리켜야 덮개가 읽히는 자리에 걸린다.
   */
  keyColumn: string;
  /**
   * 그 줄을 부르는 이름. <b>앞에서부터 차 있는 것</b>을 쓴다 — 통합은 계약이 아직 없는 줄도
   * 내므로(접수만, 접수+공고) 계약번호가 비어 있을 수 있고, 그때는 공고번호가, 그것도 없으면
   * 접수번호가 그 줄의 이름이 된다.
   */
  identityColumns: string[];
  /** 사람이 세운 열에 값을 넣는다. 실패하면 던진다 — 표는 넣히지 않은 값을 화면에 남기지 않는다. */
  onEdit: (rowKey: string, field: string, value: string) => Promise<void>;
  /** 파서가 읽은 값을 고친다. 담기는 표가 달라 길을 가른다. */
  onCorrect: (rowKey: string, field: string, value: string) => Promise<void>;
  /**
   * 줄 하나를 지운다. 무엇이 사라지는지 묻는 것은 바깥의 몫이다.
   *
   * <p><b>없으면 단추를 그리지 않는다.</b> 모든 표의 줄이 지우는 개체인 것은 아니다 —
   * 계획은 엑셀에서 오는 읽기 전용이라 여기서 지울 것이 없고, 지울 수 있는 것처럼 보이면
   * 눌러 본 사람에게 다리가 "그런 접수·공고·계약이 없다" 고만 답한다.</p>
   */
  onDelete?: (rowKey: string) => void;
  onDeleteRows?: (rows: Row[]) => void;
  onBatchEdit?: (changes: BatchChange[]) => Promise<void>;
  onManageColumns?: (field?: string) => void;
  onExport?: (data: SheetExport) => Promise<void>;
};

type Sort = { column: string; dir: 1 | -1 };

const quoteCell = (value: string) => /[\t\r\n"]/.test(value) ? `"${value.replaceAll('"', '""')}"` : value;

/** Excel clipboard TSV includes quoted cells containing tabs or newlines. */
export function parseClipboard(text: string): string[][] {
  if (text.length > 2_000_000) throw new Error("붙여넣기는 한 번에 2MB까지 가능합니다.");
  const rows: string[][] = [[]];
  let value = "", quoted = false;
  for (let i = 0; i < text.length; i++) {
    const char = text[i];
    if (char === '"' && (quoted || !value)) {
      if (quoted && text[i + 1] === '"') { value += '"'; i++; } else quoted = !quoted;
    } else if (!quoted && (char === "\t" || char === "\n" || char === "\r")) {
      rows[rows.length - 1].push(value); value = "";
      if (char !== "\t") { if (char === "\r" && text[i + 1] === "\n") i++; rows.push([]); }
    } else value += char;
  }
  if (quoted) throw new Error("붙여넣을 값의 따옴표가 닫히지 않았습니다.");
  rows[rows.length - 1].push(value);
  if (rows.length > 1 && /[\r\n]$/.test(text) && rows.at(-1)!.length === 1 && !rows.at(-1)![0]) rows.pop();
  if (rows.some(r => r.length !== rows[0].length)) throw new Error("행마다 열 개수가 다릅니다. 사각형 범위를 복사하세요.");
  if (rows.length * rows[0].length > 10000) throw new Error("한 번에 10,000셀까지 붙여넣을 수 있습니다.");
  return rows;
}

/**
 * 계약면 뷰를 표로 보여 주고, 고칠 수 있는 칸을 고치게 한다.
 *
 * <p>고칠 수 있는 칸은 <b>두 종류</b>다. 사람이 세운 열(진행상태·메모)은 <code>user_field</code>
 * 에 담기고, 파서가 읽은 칸의 정정은 <b>덮개</b>로 얹힌다. 화면에서는 둘 다 그냥 타자를 치면
 * 되지만 담기는 표가 다르고 다시 가져올 때의 운명도 달라, 넣는 길이 갈린다.</p>
 *
 * <p><b>고친 칸은 표시가 붙는다.</b> 무엇이 문서에서 읽은 값이고 무엇이 사람이 적은 것인지
 * 보이지 않으면, 고칠 수 있다는 것 자체가 안전하지 않다.</p>
 *
 * <p>키 문법은 Excel 을 그대로 베낀다 — 화살표로 옮기고, F2 나 그냥 타자로 편집에 들어가고,
 * Enter 로 넣고 아래로, Tab 으로 넣고 오른쪽으로, Esc 로 되돌린다. 이 표의 조상이
 * 부서 엑셀 관리대장이라, 익숙한 문법을 그대로 쓰면 배울 것이 없다.</p>
 *
 * <p><b>화면에 보이는 값은 언제나 <code>sheet</code> 에서 온다.</b> 편집 중인 글자만 따로 들고
 * 있다가 저장에 성공하면 버린다. 예전에는 입력 상자가 비제어라 저장이 실패해도 거부된 값이
 * 화면에 남아, <b>화면과 DB 가 조용히 갈라졌다</b> — 파서에서 그토록 경계한 그 일이 화면에 있었다.</p>
 */
export function Grid({
  sheet, columns, keyColumn, identityColumns, onEdit, onCorrect, onDelete, onDeleteRows,
  onBatchEdit, onManageColumns, onExport, initialQuery = "", focusQuery = true, onQueryChange,
}: Props) {
  const [query, setQuery] = useState(initialQuery);
  const queryReport = useRef(onQueryChange);
  queryReport.current = onQueryChange;
  useEffect(() => { queryReport.current?.(query); }, [query]);
  const [sort, setSort] = useState<Sort | null>(null);
  const [heading, setHeading] = useState(false);
  const gridId = useId();
  /**
   * 커서는 <b>줄의 신원</b>으로 든다. <code>row</code> 는 마지막으로 본 자리일 뿐이다.
   *
   * <p>예전에는 자리 번호만 들어, 커서가 선 채로 <code>rows</code> 가 바뀌면(창에 돌아올 때마다
   * 바깥이 다시 읽고, 정렬 중에 저장하면 다시 세워지고, 고친 값이 찾기에서 빠지면) 같은 번호가
   * <b>다른 레코드</b>를 가리켰다 — 편집 중이던 글자가 오류 없이 남의 줄에 저장됐다.</p>
   */
  const [cursor, setCursor] = useState<{ id: string | null; row: number; col: number }>(
    { id: null, row: 0, col: 0 },
  );

  /** 편집 중인 글자. null 이면 보기 상태다. */
  const [draft, setDraft] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  /** 저장하는 동안 친 글자. 끝나면 지금 칸의 편집으로 이어 받는다 — Enter 로 넣고 곧바로 친 글자를 버리지 않는다. */
  const typedWhileSaving = useRef("");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [anchor, setAnchor] = useState<{ id: string; col: number } | null>(null);
  const [bulkField, setBulkField] = useState("");
  const [bulkValue, setBulkValue] = useState("");
  const [bulkOpen, setBulkOpen] = useState(false);
  const [pending, setPending] = useState<BatchChange[] | null>(null);
  /** 행 선택 일괄 입력에서 값을 매달 수 없어 건너뛴 행 수. 검토 창이 함께 말한다. */
  const [skipped, setSkipped] = useState(0);
  const [batchError, setBatchError] = useState("");
  const [notice, setNotice] = useState("");
  /** 알림을 띠에 보이는 동안. 스크린리더 알림(role=status)은 그대로 두고, 보이는 글만 잠시 뒤 걷는다. */
  const [noticeShown, setNoticeShown] = useState(false);
  const noticeTimer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const tell = (text: string) => {
    setNotice(text); setNoticeShown(true);
    clearTimeout(noticeTimer.current);
    noticeTimer.current = setTimeout(() => setNoticeShown(false), 4000);
  };
  useEffect(() => () => clearTimeout(noticeTimer.current), []);
  const [exportBusy, setExportBusy] = useState(false);
  /** 「엑셀 내보내기」 펼침. 고르거나 바깥을 누르거나 Esc 면 접는다 — 열린 채 두면 표 위를 가린다. */
  const exportMenu = useRef<HTMLDetailsElement>(null);
  const [exportOpen, setExportOpen] = useState(false);
  const closeExport = () => { if (exportMenu.current) exportMenu.current.open = false; setExportOpen(false); };
  useEffect(() => {
    if (!exportOpen) return;
    const away = (e: PointerEvent) => { if (!exportMenu.current?.contains(e.target as Node)) closeExport(); };
    document.addEventListener("pointerdown", away);
    return () => document.removeEventListener("pointerdown", away);
  }, [exportOpen]);
  /** 가공열 칸을 고치려 할 때 뜨는 안내. 줄 신원과 열 번호. */
  const [explain, setExplain] = useState<{ id: string; col: number } | null>(null);
  const dragging = useRef(false);
  const dragMoved = useRef(false);
  const rowAnchor = useRef<string | null>(null);
  const bulkLock = useRef(false);

  const editable = useMemo(() => new Set(sheet.editable), [sheet.editable]);
  const correctable = useMemo(() => new Set(sheet.correctable), [sheet.correctable]);
  const definitions = useMemo(
    () => new Map(columns.map((c) => [c.fieldName, c])),
    [columns],
  );

  const board = useRef<HTMLDivElement>(null);
  const afterBoard = useRef<HTMLDivElement>(null);
  const search = useRef<HTMLInputElement>(null);
  const here = useRef<HTMLTableCellElement>(null);
  useEffect(() => { if (initialQuery && focusQuery) search.current?.focus(); }, [initialQuery]); // eslint-disable-line react-hooks/exhaustive-deps

  useFindShortcut(search);

  const rows = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const found = needle
      ? sheet.rows.filter((r) => sheet.columns.some((c) => (r[c] ?? "").toLowerCase().includes(needle)))
      : sheet.rows;

    if (!sort) return found;

    // 열 전체에 같은 비교를 쓴다. 두 값마다 방식을 바꾸면 2 < 10 < 1a < 2가 된다.
    const numeric = sheet.rows.every(r => !r[sort.column] || asNumber(r[sort.column]) !== null);

    // 원본을 건드리지 않으려고 베껴서 세운다.
    return [...found].sort((a, b) => {
      const x = a[sort.column] ?? "";
      const y = b[sort.column] ?? "";

      // 빈 값은 언제나 뒤로. 오름차순에서 빈 칸이 위를 덮으면 표를 읽을 수 없다.
      if (!x && !y) return 0;
      if (!x) return 1;
      if (!y) return -1;

      const cmp = numeric ? asNumber(x)! - asNumber(y)! : x.localeCompare(y, "ko");
      return cmp * sort.dir;
    });
  }, [sheet, query, sort]);

  /**
   * 그 줄을 부르는 이름. 앞에서부터 차 있는 것을 쓴다 — 지우기와 커서가 이것을 본다.
   * 통합에서 계약이 아직 없는 줄도 이름을 갖게 하는 자리다.
   */
  const 이름 = useCallback(
    (r: Record<string, string>) => identityColumns.map((c) => r[c] ?? "").find(Boolean) ?? "",
    [identityColumns],
  );

  /**
   * 커서와 React 키가 붙드는 줄의 신원 — 이름에 <b>원본 순서에서 몇 번째 같은 이름인지</b>를
   * 붙인다. 차수를 편 뷰는 한 이름이 여러 줄로 서므로 이름만으로는 줄 하나를 가리키지 못한다.
   * 원본 순서로 세므로 정렬·찾기로는 바뀌지 않는다.
   */
  const 신원 = useMemo(() => {
    const seen = new Map<string, number>();
    const ids = new Map<Record<string, string>, string>();
    for (const r of sheet.rows) {
      const name = identityColumns.map(c => r[c] ?? "").join("\u0001") + "\u0001" + (r["차수"] ?? "");
      const nth = seen.get(name) ?? 0;
      seen.set(name, nth + 1);
      ids.set(r, `${name}\u0000${nth}`);
    }
    return (r: Record<string, string>) => ids.get(r) ?? 이름(r);
  }, [sheet.rows, 이름, identityColumns]);

  const last = Math.max(rows.length - 1, 0);

  // 커서가 선 줄을 이번 rows 에서 찾는다. 같은 자리에 그대로 있으면 그것을, 옮겨 갔으면 옮긴
  // 자리를 쓴다. 사라졌으면(지웠거나 찾기에서 빠졌으면) 마지막으로 본 자리 근처로 당긴다.
  const found = useMemo(() => {
    if (cursor.id === null) return -1;
    const there = rows[cursor.row];
    if (there !== undefined && 신원(there) === cursor.id) return cursor.row;
    return rows.findIndex((r) => 신원(r) === cursor.id);
  }, [rows, cursor.id, cursor.row, 신원]);
  const at = found >= 0 ? found : Math.min(cursor.row, last);
  /** 편집하던 줄이 사라졌다 — 그 글자를 옆 줄에 넣을 수는 없다. */
  const lost = cursor.id !== null && found < 0;

  // 찾은 자리를 커서에 되적는다. 사라졌으면 당겨 선 줄을 새로 붙들고 편집 중이던 글자를 버린다.
  useEffect(() => {
    if (lost) setDraft(null);
    const r = rows[at];
    if (r === undefined) return; // 찾기 결과가 비었다 — 검색을 지우면 그 줄로 돌아간다
    const id = 신원(r);
    if (id !== cursor.id || at !== cursor.row) setCursor((c) => ({ ...c, id, row: at }));
  }, [rows, at, lost, cursor.id, cursor.row, 신원]);

  // 열이 줄면 커서를 안으로 당긴다. 열 번호가 가리키는 열이 달라졌을 수 있어 편집도 접는다.
  useEffect(() => {
    setCursor((c) => ({ ...c, col: Math.min(c.col, sheet.columns.length - 1) }));
    setDraft(null);
  }, [sheet.columns.length]);

  useEffect(() => {
    here.current?.scrollIntoView({ block: "nearest", inline: "nearest" });
  }, [at, cursor.col]);
  useEffect(() => { setExplain(e => e && (e.id !== cursor.id || e.col !== cursor.col) ? null : e); }, [cursor.id, cursor.col]);

  /** 화면에 보이는 자리로 커서를 옮긴다 — 옮긴 뒤로는 그 자리의 줄을 붙든다. */
  const go = useCallback((index: number, col: number) => {
    const i = Math.min(Math.max(index, 0), last);
    const r = rows[i];
    setCursor({
      id: r === undefined ? null : 신원(r),
      row: i,
      col: Math.min(Math.max(col, 0), sheet.columns.length - 1),
    });
  }, [rows, last, 신원, sheet.columns.length]);

  /**
   * 이 줄에 <b>고친 값이 매달릴 레코드가 있는가</b>.
   *
   * <p>통합의 계약 없는 줄에서는 없다. 그런 줄에서 계약 열을 고치면 덮개가 공고나 접수에
   * 걸려 <b>어느 뷰도 읽지 않는 자리로 사라진다</b> — 오류 없이 값만 없어지므로, 고칠 수
   * 있는 것처럼 보이지도 않게 잠근다.</p>
   */
  const 매달린다 = (r: Record<string, string> | undefined) => (r?.[keyColumn] ?? "") !== "";

  const column = sheet.columns[cursor.col];
  const row = rows[at];
  const canEdit =
    row !== undefined && 매달린다(row) && (editable.has(column) || correctable.has(column));
  /** 줄이 사라진 그 한 번의 그리기에서는 글자를 옆 줄에 그리지 않는다 — 곧 버린다. */
  const editing = lost ? null : draft;

  const selectedRows = rows.filter(r => selected.has(신원(r)));
  const anchorRow = anchor ? rows.findIndex(r => 신원(r) === anchor.id) : -1;
  const bounds = { top: Math.min(anchorRow < 0 ? at : anchorRow, at), bottom: Math.max(anchorRow < 0 ? at : anchorRow, at),
    left: Math.min(anchor?.col ?? cursor.col, cursor.col), right: Math.max(anchor?.col ?? cursor.col, cursor.col) };
  const rangeSize = (bounds.bottom - bounds.top + 1) * (bounds.right - bounds.left + 1);
  const clearSelection = () => { setSelected(new Set()); setAnchor(null); setBulkOpen(false); rowAnchor.current = null; };
  const chooseCell = (ri: number, ci: number, extend = false) => {
    if (saving || pending) return;
    setHeading(false);
    setSelected(new Set()); setBulkOpen(false); setDraft(null);
    setAnchor(extend ? anchor ?? { id: 신원(rows[at]), col: cursor.col } : null);
    go(ri, ci); board.current?.focus();
  };
  const chooseRow = (ri: number, checked: boolean, extend: boolean) => {
    if (saving || pending) return;
    setAnchor(null); setDraft(null);
    const start = extend && rowAnchor.current ? rows.findIndex(r => 신원(r) === rowAnchor.current) : ri;
    setSelected(previous => {
      const next = new Set(previous);
      for (let i = Math.min(start < 0 ? ri : start, ri); i <= Math.max(start, ri); i++) {
        if (checked) next.add(신원(rows[i])); else next.delete(신원(rows[i]));
      }
      return next;
    });
    rowAnchor.current = 신원(rows[ri]);
  };
  const selectAllRows = (checked: boolean) => {
    setAnchor(null); setDraft(null); setSelected(new Set(checked ? rows.map(신원) : []));
  };
  useEffect(() => {
    const live = new Set(sheet.rows.map(신원));
    setSelected(previous => {
      const kept = new Set([...previous].filter(id => live.has(id)));
      return kept.size === previous.size ? previous : kept;
    });
    if (anchor && !live.has(anchor.id)) setAnchor(null);
  }, [sheet.rows, 신원, anchor]);
  useEffect(() => {
    const stop = () => { dragging.current = false; };
    window.addEventListener("pointerup", stop); window.addEventListener("pointercancel", stop); window.addEventListener("blur", stop);
    return () => { window.removeEventListener("pointerup", stop); window.removeEventListener("pointercancel", stop); window.removeEventListener("blur", stop); };
  }, []);

  /**
   * 바꿀 칸을 검토 창에 올린다. 고칠 수 없는 칸이 하나라도 섞이면 아무것도 올리지 않는다 — 범위를 잘못 잡은 것이다.
   *
   * <p><code>skipUnattached</code> 는 행 선택의 「같은 값 입력」 만 쓴다. 통합 표에는 계약이 아직 없는 줄이 정상으로
   * 섞여 있어, 그런 줄 하나 때문에 전체를 거절하면 그 기능을 쓸 수가 없다. 그 줄은 건너뛰고 몇 줄인지 검토 창에 보인다.
   * 모두 건너뛰게 되면 지금처럼 거절한다.</p>
   */
  const prepare = (asked: { ri: number; ci: number; value: string }[], operation: "set" | "revert" = "set", skipUnattached = false) => {
    setBatchError("");
    if (!onBatchEdit || saving) return;
    const attached = skipUnattached ? asked.filter(c => 매달린다(rows[c.ri])) : asked;
    // 모두 건너뛰게 되면 거르지 않고 그대로 보낸다 — 아래에서 첫 줄이 거절되며 까닭이 선다.
    const cells = attached.length ? attached : asked;
    const changes = new Map<string, BatchChange>();
    for (const { ri, ci, value } of cells) {
      const r = rows[ri], field = sheet.columns[ci];
      if (!r || !field || !매달린다(r) || !(editable.has(field) || correctable.has(field))) {
        setBatchError(`${ri + 1}행 · ${field ?? "표 밖"}: 수정할 수 없는 칸입니다. 범위를 다시 선택하세요. 저장한 값은 없습니다.`); return;
      }
      if (operation === "revert" && sheet.overrides[r[keyColumn]]?.[field] === undefined) continue;
      const definition = definitions.get(field);
      if (operation === "set" && definition?.kind === "choice" && value && !definition.choices.includes(value)) {
        setBatchError(`${ri + 1}행 · ${field}: 선택 항목에 없는 값입니다 (${value}). 저장한 값은 없습니다.`); return;
      }
      const id = `${r[keyColumn]}\u0000${field}`;
      const next: BatchChange = { rowKey: r[keyColumn], field, value, original: r[field] ?? "", operation };
      if (changes.has(id) && changes.get(id)!.value !== value) {
        setBatchError(`같은 자료의 ${field}에 서로 다른 값을 넣을 수 없습니다.`); return;
      }
      if (operation === "revert" || next.original !== value) changes.set(id, next);
    }
    if (!changes.size) { tell("변경할 값이 없습니다."); return; }
    setSkipped(asked.length - cells.length);
    setPending([...changes.values()]);
  };
  const rangeCells = (value: string) => {
    const cells = [];
    for (let ri = bounds.top; ri <= bounds.bottom; ri++)
      for (let ci = bounds.left; ci <= bounds.right; ci++) cells.push({ ri, ci, value });
    return cells;
  };
  const applyBatch = async () => {
    if (!pending || !onBatchEdit || bulkLock.current) return;
    bulkLock.current = true; setSaving(true); setBatchError("");
    try { await onBatchEdit(pending); tell(`${pending.length}셀 변경을 저장했습니다.`); setPending(null); setBulkOpen(false); }
    catch (e) { setBatchError((e as Error).message); }
    finally { bulkLock.current = false; setSaving(false); }
  };
  const copyText = () => {
    const list = selected.size ? selectedRows : rows.slice(bounds.top, bounds.bottom + 1);
    const fields = selected.size ? sheet.columns : sheet.columns.slice(bounds.left, bounds.right + 1);
    return list.map(r => fields.map(c => quoteCell(r[c] ?? "")).join("\t")).join("\r\n");
  };
  const exportRows = async (list: Row[]) => {
    if (!onExport || exportBusy) return;
    setExportBusy(true); setBatchError("");
    try { await onExport({ columns: sheet.columns, rows: list.map(r => sheet.columns.map(c => r[c] ?? "")) }); }
    catch (e) { setBatchError((e as Error).message); }
    finally { setExportBusy(false); }
  };

  /** 이 칸에 걸린 덮개의 원래 값. 걸려 있지 않으면 undefined 다 — 빈 문자열과 구별된다. */
  const original = row === undefined ? undefined : sheet.overrides[row[keyColumn]]?.[column];

  const move = useCallback(
    (dRow: number, dCol: number) => go(at + dRow, cursor.col + dCol),
    [go, at, cursor.col],
  );

  const commit = useCallback(async (next: string) => {
    setDraft(null);
    if (row === undefined || next === (row[column] ?? "")) return;

    setSaving(true);
    try {
      // 사람이 세운 열은 user_field 로, 파서가 읽은 칸은 덮개로. 담기는 표가 다르다.
      const put = editable.has(column) ? onEdit : onCorrect;
      await put(row[keyColumn], column, next);
    } catch {
      // 넣지 못했다. 바깥이 까닭을 알리고, 표는 원래 값을 그대로 보여 준다.
    } finally {
      setSaving(false);
    }
  }, [row, column, keyColumn, editable, onEdit, onCorrect]);

  useEffect(() => {
    if (saving) return;
    const typed = typedWhileSaving.current;
    typedWhileSaving.current = "";
    // 보통 때 타자로 편집을 시작하는 것과 같은 조건 — 고칠 수 있는 칸에 커서가 있고 줄을 고르지 않았을 때.
    if (typed && canEdit && draft === null && !selected.size && !pending) setDraft(typed);
  }, [saving]); // eslint-disable-line react-hooks/exhaustive-deps

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (rows.length === 0) return;
    if (editing !== null) return; // 편집 중에는 입력 상자가 맡는다
    if (e.target === board.current && e.key === "Tab") {
      if (!e.shiftKey) { e.preventDefault(); afterBoard.current?.focus(); }
      return;
    }
    if ((e.target as HTMLElement).matches('input, select, button')) return;
    if (saving || pending) {
      e.preventDefault();
      if (saving && !pending) {
        if (e.key === "Escape") typedWhileSaving.current = "";
        else if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) typedWhileSaving.current += e.key;
      }
      return;
    }
    if (heading) {
      if (e.key === "ArrowDown") { e.preventDefault(); chooseCell(0, cursor.col); return; }
      if (["ArrowLeft", "ArrowRight", "Home", "End"].includes(e.key)) {
        e.preventDefault();
        go(at, e.key === "Home" ? 0 : e.key === "End" ? sheet.columns.length - 1 : cursor.col + (e.key === "ArrowLeft" ? -1 : 1));
        return;
      }
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault(); toggleSort(column); return;
      }
      if (e.key === "F2" && definitions.has(column) && onManageColumns) { e.preventDefault(); onManageColumns(column); return; }
      if (e.key === "Escape") { e.preventDefault(); setHeading(false); search.current?.focus(); return; }
      return;
    }
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "a") {
      e.preventDefault(); setSelected(new Set()); setAnchor({ id: 신원(rows[0]), col: 0 });
      go(last, sheet.columns.length - 1); return;
    }
    if (e.shiftKey && e.key === " ") {
      e.preventDefault(); setSelected(new Set(rows.slice(bounds.top, bounds.bottom + 1).map(신원))); setAnchor(null); return;
    }
    const directions: Record<string, [number, number]> = { ArrowUp: [-1, 0], ArrowDown: [1, 0], ArrowLeft: [0, -1], ArrowRight: [0, 1] };
    if (directions[e.key]) {
      e.preventDefault(); const [dr, dc] = directions[e.key];
      if (e.key === "ArrowUp" && at === 0 && !e.shiftKey) { setHeading(true); return; }
      chooseCell(Math.max(0, Math.min(last, at + dr)), Math.max(0, Math.min(sheet.columns.length - 1, cursor.col + dc)), e.shiftKey); return;
    }
    if (e.key === "Escape") { e.preventDefault(); if (explain) { setExplain(null); return; } clearSelection(); setBatchError(""); search.current?.focus(); return; }
    if (selected.size && (["Delete", "Backspace", "F2", "Enter"].includes(e.key) || (e.key.length === 1 && !e.ctrlKey && !e.metaKey))) { e.preventDefault(); return; }
    if (anchor && ["Delete", "Backspace"].includes(e.key)) {
      e.preventDefault(); prepare(rangeCells(""), e.ctrlKey ? "revert" : "set"); return;
    }

    switch (e.key) {
      case "ArrowUp": move(-1, 0); break;
      case "ArrowDown": move(1, 0); break;
      case "ArrowLeft": move(0, -1); break;
      case "ArrowRight": move(0, 1); break;
      case "PageUp": move(-15, 0); break;
      case "PageDown": move(15, 0); break;
      case "Home": go(e.ctrlKey ? 0 : at, 0); break;
      case "End": go(e.ctrlKey ? last : at, sheet.columns.length - 1); break;
      case "F2":
      case "Enter":
        if (canEdit) setDraft(row[column] ?? "");
        else if (e.key === "F2" && definitions.get(column)?.kind === "computed" && row) setExplain({ id: 신원(row), col: cursor.col });
        else if (e.key === "Enter") move(1, 0);
        break;
      case "Delete":
      case "Backspace":
        // Ctrl 을 짚으면 되돌리기, 그냥 누르면 빈 값. 잘못 읽힌 칸을 <b>비우는 것</b>과
        // 파서 값으로 <b>돌아가는 것</b>은 다른 일이라 키를 나눈다. 되돌리기는 사람이 고친 값을 지우므로
        // 범위로 할 때와 같은 검토 창을 거친다 — 칸 하나라고 확인 없이 지우지 않는다. 고칠 수 없는 표에서는 아무것도 하지 않는다.
        if (e.ctrlKey) { if (original !== undefined && onBatchEdit) prepare([{ ri: at, ci: cursor.col, value: "" }], "revert"); }
        else if (canEdit) void commit("");
        break;
      default:
        // 그냥 타자를 치면 그 글자로 편집을 시작한다 — 엑셀과 같다.
        if (canEdit && e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) setDraft(e.key);
        else return;
    }

    e.preventDefault();
  };

  const toggleSort = (c: string) => {
    setAnchor(null);
    setSort(s => s?.column !== c ? { column: c, dir: 1 } : s.dir > 0 ? { column: c, dir: -1 } : null);
  };

  return (
    <div className="grid-wrap">
      <div className="grid-toolbar">
        <input
          ref={search}
          className="grid-find"
          type="search"
          aria-label="목록 검색"
          placeholder="찾기 (Ctrl+F)"
          value={query}
          onChange={(e) => { setQuery(e.target.value); clearSelection(); setNotice("검색 조건이 바뀌어 선택을 해제했습니다."); setNoticeShown(false); }}
          onKeyDown={(e) => { if (e.key === "Escape") { setQuery(""); clearSelection(); board.current?.focus(); } }}
        />
        <span className="result-count" role="status">{rows.length}건<span> / 전체 {sheet.rows.length}건</span></span>
        {sort && (
          <button className="action" onClick={() => { setSort(null); setAnchor(null); }}>
            {sort.column} {sort.dir > 0 ? "↑" : "↓"} · 정렬 해제
          </button>
        )}

        <span className="spacer" />

        {onManageColumns && <button className="action quiet" onClick={() => onManageColumns()}>열 관리</button>}
        {onExport && <details className="grid-export" ref={exportMenu} onToggle={e => setExportOpen(e.currentTarget.open)}
          onKeyDown={e => {
            if (e.key !== "Escape" || !exportMenu.current?.open) return;
            e.preventDefault(); closeExport(); exportMenu.current.querySelector("summary")?.focus();
          }}>
          <summary className="action" aria-label="엑셀 내보내기">{exportBusy ? "내보내는 중…" : "엑셀 내보내기"}</summary>
          <div className="grid-export-options">
            <button className="action quiet" disabled={exportBusy || !rows.length} onClick={() => { closeExport(); void exportRows(rows); }}>검색 결과 {rows.length}행 내보내기</button>
            <button className="action quiet" disabled={exportBusy || !selectedRows.length} onClick={() => { closeExport(); void exportRows(selectedRows); }}>선택한 {selectedRows.length}행 내보내기</button>
            <span className="hint">현재 표와 정렬 순서로 저장합니다. 전체 자료는 설정의 「자료 관리」에서 내보냅니다.</span>
          </div>
        </details>}
      </div>
      <div className="grid-selection" aria-label="선택 작업">
        {selectedRows.length ? <>
          <strong>{selectedRows.length}행 선택</strong>
          {!!sheet.editable.length && onBatchEdit && <button className="action quiet" onClick={() => { setBulkField(sheet.editable[0]); setBulkValue(""); setBulkOpen(v => !v); }}>같은 값 입력…</button>}
          {(onDeleteRows || onDelete) && <button className="action quiet" disabled={saving || (!onDeleteRows && selectedRows.length !== 1)}
            onClick={() => onDeleteRows ? onDeleteRows(selectedRows) : onDelete?.(이름(selectedRows[0]))}>선택 자료 삭제…</button>}
          <button className="action quiet" onClick={clearSelection}>선택 해제</button>
        </> : anchor ? <>
          <strong>{rangeSize}셀 선택</strong>
          <button className="action quiet" onClick={() => void navigator.clipboard.writeText(copyText()).then(() => tell("복사했습니다."), () => setBatchError("표에 초점을 두고 Ctrl+C로 복사하세요."))}>복사</button>
          {onBatchEdit && !!(sheet.editable.length + sheet.correctable.length) && <>
            <button className="action quiet" onClick={() => prepare(rangeCells(""))}>내용 비우기…</button>
            <button className="action quiet" onClick={() => prepare(rangeCells(""), "revert")}>원래 값 복원…</button>
          </>}
          <button className="action quiet" onClick={clearSelection}>선택 해제</button>
        </> : <span className="hint">체크박스로 여러 행 선택 · Shift+클릭으로 범위 선택 · Ctrl+C / Ctrl+V</span>}
      </div>
      {bulkOpen && selectedRows.length > 0 && <form className="grid-bulk" onSubmit={e => { e.preventDefault();
        prepare(selectedRows.map(r => ({ ri: rows.indexOf(r), ci: sheet.columns.indexOf(bulkField), value: bulkValue })), "set", true); }}>
        <label>입력할 열 <select className="text" value={bulkField} onChange={e => setBulkField(e.target.value)}>{sheet.editable.map(c => <option key={c}>{c}</option>)}</select></label>
        <label>같은 값 {definitions.get(bulkField)?.kind === "choice" ? <select className="text" value={bulkValue} onChange={e => setBulkValue(e.target.value)}><option value="">빈 값</option>{definitions.get(bulkField)?.choices.map(c => <option key={c}>{c}</option>)}</select>
          : <input className="text" value={bulkValue} onChange={e => setBulkValue(e.target.value)} />}</label>
        <button className="action" type="submit">변경 검토</button><button className="action quiet" type="button" onClick={() => setBulkOpen(false)}>취소</button>
        <span className="hint">사용자 입력값은 같은 본번호의 전체 차수에서 함께 사용합니다.</span>
      </form>}
      {batchError && !pending && <p className="grid-message" role="alert">{batchError}</p>}
      {notice && noticeShown && <p className="grid-message grid-notice" aria-hidden="true">{notice}</p>}
      {notice && <span className="sr-only" role="status">{notice}</span>}
      <div className="grid-board" ref={board} tabIndex={0} onKeyDown={onKeyDown}
        role={rows.length ? "grid" : undefined} aria-label={rows.length ? "자료 목록" : undefined}
        aria-rowcount={rows.length ? rows.length + 1 : undefined} aria-colcount={rows.length ? sheet.columns.length + 1 : undefined}
        aria-multiselectable={rows.length ? true : undefined} aria-describedby={`${gridId}-keys`}
        aria-activedescendant={rows.length ? `${gridId}-${heading ? "heading" : `row-${at}`}-${cursor.col}` : undefined}
        onCopy={e => { if (editing !== null || !rows.length) return; e.preventDefault(); e.clipboardData.setData("text/plain", copyText()); }}
        onPaste={e => {
          if (editing !== null || !onBatchEdit) return;
          e.preventDefault();
          if (selected.size) { setBatchError("붙여넣을 시작 셀을 먼저 선택하세요."); return; }
          try {
            const values = parseClipboard(e.clipboardData.getData("text/plain"));
            if (anchor && rangeSize > 1 && (values.length !== bounds.bottom - bounds.top + 1 || values[0].length !== bounds.right - bounds.left + 1))
              throw new Error("붙여넣을 크기와 선택 범위가 다릅니다. 시작 셀 하나를 선택하거나 같은 크기의 범위를 선택하세요.");
            prepare(values.flatMap((line, ri) => line.map((value, ci) => ({ ri: bounds.top + ri, ci: bounds.left + ci, value }))));
          } catch (error) { setBatchError((error as Error).message); }
        }}>
        {rows.length === 0 ? (
          <div className="empty-state"><h2>검색 결과가 없습니다</h2><p>다른 검색어로 찾아보세요.</p>
            <button className="action" onClick={() => { setQuery(""); clearSelection(); search.current?.focus(); }}>검색 지우기</button>
          </div>
        ) : <table className="grid" role="presentation">
          <thead role="rowgroup">
            <tr role="row" aria-rowindex={1}>
              <th className="row-check" role="columnheader" aria-label="행 선택"><input type="checkbox" tabIndex={-1} aria-label={`검색 결과 ${rows.length}행 전체 선택`} checked={rows.length > 0 && selectedRows.length === rows.length}
                ref={node => { if (node) node.indeterminate = selectedRows.length > 0 && selectedRows.length < rows.length; }}
                onChange={e => selectAllRows(e.target.checked)} /></th>
              {sheet.columns.map((c, ci) => (
                <th
                  key={c}
                  ref={heading && cursor.col === ci ? here : undefined}
                  role="columnheader" id={`${gridId}-heading-${ci}`} aria-colindex={ci + 2}
                  className={[editable.has(c) ? "editable" : definitions.get(c)?.kind === "computed" ? "computed" : "", heading && cursor.col === ci ? "cur" : ""].filter(Boolean).join(" ") || undefined}
                  title={
                    editable.has(c) ? "사용자 입력 열 (클릭하면 정렬)"
                    : correctable.has(c) ? "문서 값을 수정할 수 있는 열 (클릭하면 정렬)"
                    : definitions.get(c)?.kind === "computed" ? "식이 채우는 가공열 (클릭하면 정렬)"
                    : "클릭하면 정렬"
                  }
                  aria-sort={sort?.column === c ? sort.dir > 0 ? "ascending" : "descending" : "none"}
                  onClick={() => toggleSort(c)}
                >
                  {definitions.get(c)?.kind === "computed" && <span className="grid-formula-mark" aria-hidden="true">=</span>}
                  <button className="grid-heading" type="button" tabIndex={-1} aria-label={`${c} 정렬`}>{c}</button>
                  {sort?.column === c && <span className="sort">{sort.dir > 0 ? " ↑" : " ↓"}</span>}
                  {onManageColumns && definitions.has(c) && <button className="grid-heading-edit" tabIndex={-1} aria-label={`${c} 열 정의 편집`} title="열 정의 편집"
                    onClick={e => { e.stopPropagation(); onManageColumns(c); }}><svg viewBox="0 0 24 24" aria-hidden="true"><path d="m5 16 11-11 3 3L8 19H5zM14 7l3 3" /></svg></button>}
                </th>
              ))}
            </tr>
          </thead>
          <tbody role="rowgroup">
            {rows.map((r, ri) => (
              // 신원으로 키를 단다 — 자리 번호를 섞으면 줄이 옮길 때마다 새로 태어나 편집 상자가 끊긴다.
              <tr key={신원(r)} role="row" aria-rowindex={ri + 2} aria-selected={selected.has(신원(r))} className={selected.has(신원(r)) ? "row-selected" : undefined}>
                <td className="row-check" role="gridcell"><input type="checkbox" tabIndex={-1} aria-label={`${이름(r)} 행 선택`} checked={selected.has(신원(r))} disabled={saving}
                  onChange={() => {}}
                  onClick={e => chooseRow(ri, e.currentTarget.checked, e.shiftKey)}
                  onKeyDown={e => { if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "a") { e.preventDefault(); selectAllRows(true); } }} /></td>
                {sheet.columns.map((c, ci) => {
                  const current = !heading && ri === at && ci === cursor.col;
                  return (
                    <Cell
                      key={c}
                      id={`${gridId}-row-${ri}-${ci}`}
                      label={`${c}, ${ri + 1}행 ${이름(r)}`}
                      colIndex={ci + 2}
                      cellRef={current ? here : undefined}
                      value={r[c] ?? ""}
                      editable={editable.has(c) && 매달린다(r)}
                      correctable={correctable.has(c) && 매달린다(r)}
                      original={sheet.overrides[r[keyColumn]]?.[c]}
                      definition={definitions.get(c)}
                      current={current}
                      inRange={!!anchor && ri >= bounds.top && ri <= bounds.bottom && ci >= bounds.left && ci <= bounds.right}
                      pinned={ci === 0}
                      draft={current ? editing : null}
                      saving={current && saving}
                      onSelect={e => { if (dragMoved.current) { dragMoved.current = false; return; } chooseCell(ri, ci, e.shiftKey); }}
                      onPointerDown={e => { if (e.button !== 0 || e.pointerType === "touch" || editing !== null || saving || pending) return;
                        e.preventDefault(); dragging.current = true; dragMoved.current = false; chooseCell(ri, ci, e.shiftKey); }}
                      onPointerEnter={e => { if (dragging.current && e.buttons === 1) { dragMoved.current = true; chooseCell(ri, ci, true); } }}
                      onOpen={() => { if (!saving) { clearSelection(); setDraft(r[c] ?? ""); } }}
                      onExplain={definitions.get(c)?.kind === "computed" ? () => { clearSelection(); go(ri, ci); setExplain({ id: 신원(r), col: ci }); } : undefined}
                      note={explain && explain.id === 신원(r) && explain.col === ci ? {
                        line: formulaLine(definitions.get(c)),
                        onFix: onManageColumns ? () => { setExplain(null); onManageColumns(c); } : undefined,
                        onClose: () => { setExplain(null); board.current?.focus(); },
                      } : undefined}

                      onDraft={setDraft}
                      onCommit={(value, move) => {
                        void commit(value);
                        // 「아래」 는 누른 순간 화면에서 바로 아래에 있던 줄이다. 그 줄을 붙들므로,
                        // 정렬 중에 저장해 고친 줄이 다른 자리로 옮겨 가도 커서는 그 줄에 남는다 —
                        // 저장 뒤의 같은 번호를 쓰면 그사이 밀려 올라온 다른 줄에 선다.
                        if (move === "down") go(at + 1, cursor.col);
                        else if (move === "right") go(at, cursor.col + 1);
                        else if (move === "left") go(at, cursor.col - 1);
                        board.current?.focus();
                      }}
                      onCancel={() => { setDraft(null); board.current?.focus(); }}
                    />
                  );
                })}
              </tr>
            ))}
          </tbody>
        </table>}
      </div>

      <div className="grid-status" ref={afterBoard} tabIndex={rows.length ? 0 : -1} role="group" aria-label="표 아래 안내"
        onKeyDown={e => { if (e.key === "Tab" && e.shiftKey) { e.preventDefault(); board.current?.focus(); } }}>
        <span>{rows.length ? at + 1 : 0} / {rows.length}</span>
        {rows.length !== sheet.rows.length && <span>· 전체 {sheet.rows.length}</span>}
        <span>· {column}</span>
        <span className="grid-key-help" id={`${gridId}-keys`}>방향키 칸 이동 · 첫 행에서 ↑ 머리글 (Enter 정렬 / F2 열 편집) · Shift+Space 행 선택 · Tab 표 밖으로 · Shift+Tab 이전 도구로 · Esc 검색으로</span>
        {original !== undefined && <span className="fixed">· 수정한 셀 (원래 값: {original || "빈 값"})</span>}
        {canEdit && (
          <span className="hint">
            · F2 또는 입력으로 편집 · Enter 저장 후 아래로 · Tab 저장 후 옆으로 · Esc 취소
            {original !== undefined && " · Ctrl+Delete 원래대로"}
          </span>
        )}
      </div>
      {pending && <Modal label="일괄 변경 검토" onClose={() => { if (!saving) { setPending(null); setBatchError(""); } }} className="batch-review"
        fallbackFocus={() => board.current}>
        <h2>{pending.length}셀 변경 검토</h2>
        <p>아래 변경을 한 번에 저장합니다. 입력열은 전체 차수에, 수집값 정정은 선택한 차수에 적용합니다.</p>
        {skipped > 0 && <p className="hint">계약이 없는 {skipped}행은 건너뜁니다.</p>}
        <div className="batch-preview"><table><thead><tr><th>자료 번호</th><th>열</th><th>현재 값</th><th>변경 값</th></tr></thead><tbody>
          {pending.slice(0, 50).map(c => <tr key={`${c.rowKey}:${c.field}`}><td>{c.rowKey}</td><td>{c.field}</td><td>{c.original || "빈 값"}</td><td>{c.operation === "revert" ? "수집 원래 값" : c.value || "빈 값"}</td></tr>)}
        </tbody></table></div>
        {pending.length > 50 && <p className="hint">처음 50셀 표시 · 전체 {pending.length}셀에 적용합니다.</p>}
        {batchError && <p role="alert">{batchError}</p>}
        <div className="row end"><button className="action" disabled={saving} onClick={() => { setPending(null); setBatchError(""); }}>취소</button>
          <button className="action primary" disabled={saving} onClick={() => void applyBatch()}>{saving ? "저장 중…" : `${pending.length}셀 변경 적용`}</button></div>
      </Modal>}
    </div>
  );
}

type CellProps = {
  id: string;
  label: string;
  colIndex: number;
  cellRef?: React.Ref<HTMLTableCellElement>;
  value: string;
  editable: boolean;
  correctable: boolean;
  /** 덮개가 걸려 있으면 고치기 전의 값. 없으면 undefined — 빈 문자열과 구별된다. */
  original?: string;
  definition?: UserColumn;
  current: boolean;
  inRange?: boolean;
  pinned?: boolean;
  draft: string | null;
  saving: boolean;
  onSelect: (event: React.MouseEvent) => void;
  onPointerDown?: (event: React.PointerEvent) => void;
  onPointerEnter?: (event: React.PointerEvent) => void;
  onOpen: () => void;
  /** 가공열 칸 — 편집 대신 안내를 연다. */
  onExplain?: () => void;
  note?: { line: string; onFix?: () => void; onClose: () => void };
  onDraft: (value: string) => void;
  onCommit: (value: string, move?: "down" | "right" | "left") => void;
  onCancel: () => void;
};

function Cell({
  id, label, colIndex, cellRef, value, editable, correctable, original, definition, current, inRange, pinned, draft, saving,
  onSelect, onPointerDown, onPointerEnter, onOpen, onExplain, note, onDraft, onCommit, onCancel,
}: CellProps) {
  const open = editable || correctable;

  // 칸에는 너비 상한이 있다. 잘렸는지는 그려 봐야 알아서 그린 뒤에 잰다.
  const shownRef = useRef<HTMLDivElement>(null);
  const fullRef = useRef<HTMLDivElement>(null);
  const [clipped, setClipped] = useState(false);
  const [flip, setFlip] = useState(false);

  useLayoutEffect(() => {
    const box = shownRef.current;
    setClipped(current && box !== null && box.scrollWidth > box.clientWidth);
  }, [current, value, draft]);

  // 오른쪽 끝 열에서 펼친 값이 표 밖으로 나가면 칸의 오른쪽 끝에 맞춰 왼쪽으로 편다.
  useLayoutEffect(() => {
    const full = fullRef.current;
    const board = full?.closest(".grid-board");
    setFlip(full != null && board != null
      && full.getBoundingClientRect().right > board.getBoundingClientRect().right);
  }, [clipped]);

  const classes = [
    editable ? "editable" : "",
    original !== undefined ? "overridden" : "",
    current ? "cur" : "",
    saving ? "saving" : "",
    inRange ? "range-selected" : "",
    pinned ? "row-key" : "",
  ].filter(Boolean).join(" ");

  if (draft === null) {
    const shown = [
      "cell",
      isNumeric(value) ? "num" : "",
      value ? "" : "empty",
    ].filter(Boolean).join(" ");

    return (
      <td
        id={id} role="gridcell" aria-colindex={colIndex} aria-label={`${label}: ${value || "빈 값"}`}
        aria-selected={!!inRange} aria-readonly={!open}
        ref={cellRef}
        className={classes || undefined}
        title={original === undefined ? undefined : `수정한 셀 · 원래 값: ${original || "(빈 값)"}`}
        onClick={onSelect}
        onPointerDown={onPointerDown}
        onPointerEnter={onPointerEnter}
        onDoubleClick={open ? onOpen : onExplain}
      >
        <div ref={shownRef} className={shown}>{value || "—"}</div>
        {note && <FormulaNote {...note} />}
        {/* 커서가 선 칸이 잘렸으면 전체를 칸 위에 펼친다. 글자를 골라 베낄 수 있게 두되,
            그 누름이 칸의 편집으로 번지지 않게 막는다. */}
        {clipped && (
          <div
            ref={fullRef}
            className={flip ? "cell-full flip" : "cell-full"}
            role="tooltip"
            onClick={(e) => e.stopPropagation()}
            onDoubleClick={(e) => e.stopPropagation()}
          >
            {value}
          </div>
        )}
      </td>
    );
  }

  return (
    <td id={id} role="gridcell" aria-colindex={colIndex} aria-label={label} aria-selected={!!inRange} ref={cellRef} className={classes}>
      <Editor label={label} draft={draft} definition={definition} onDraft={onDraft} onCommit={onCommit} onCancel={onCancel} />
    </td>
  );
}

/**
 * 편집 중인 한 칸. 편집이 시작될 때 새로 태어나고 끝나면 사라지므로,
 * "이미 매듭지었나" 를 ref 하나로 들고 있을 수 있다.
 *
 * <p><b>매듭은 반드시 한 번이어야 한다.</b> Enter 로 넣거나 Esc 로 물리면 초점이 표로
 * 돌아가면서 <code>blur</code> 가 뒤따른다. 막지 않으면 <b>물린 값이 blur 를 타고 저장되고</b>
 * 넣은 값은 두 번 저장된다 — 시험이 실제로 그 일을 잡아냈다.</p>
 */
function Editor({
  label, draft, definition, onDraft, onCommit, onCancel,
}: Pick<CellProps, "label" | "draft" | "definition" | "onDraft" | "onCommit" | "onCancel"> & { draft: string }) {
  const settled = useRef(false);

  const finish = (value: string, move?: "down" | "right" | "left") => {
    if (settled.current) return;
    settled.current = true;
    onCommit(value, move);
  };

  const abort = () => {
    if (settled.current) return;
    settled.current = true;
    onCancel();
  };

  // 후보가 정해진 열은 고르게 하되, 후보 밖 값이 이미 들어 있으면 지우지 않고 그대로 보여준다.
  if (definition?.kind === "choice" && definition.choices.length > 0) {
    const known = definition.choices.includes(draft);
    return (
      <select
        autoFocus
        aria-label={label}
        value={draft}
        onChange={(e) => finish(e.target.value, "down")}
        onKeyDown={(e) => {
          if (e.key === "Escape") { e.preventDefault(); abort(); }
          else if (e.key === "Tab") { e.preventDefault(); finish(e.currentTarget.value, e.shiftKey ? "left" : "right"); }
        }}
        onBlur={abort}
      >
        <option value=""></option>
        {!known && draft ? <option value={draft}>{draft} (후보 밖)</option> : null}
        {definition.choices.map((choice) => (
          <option key={choice} value={choice}>{choice}</option>
        ))}
      </select>
    );
  }

  return (
    <input
      autoFocus
      aria-label={label}
      value={draft}
      onChange={(e) => onDraft(e.target.value)}
      onBlur={(e) => finish(e.target.value)}
      onKeyDown={(e) => {
        if (e.key === "Enter") { e.preventDefault(); finish(e.currentTarget.value, "down"); }
        else if (e.key === "Tab") { e.preventDefault(); finish(e.currentTarget.value, e.shiftKey ? "left" : "right"); }
        else if (e.key === "Escape") { e.preventDefault(); abort(); }
      }}
    />
  );
}

/**
 * 가공열 칸을 고치려 할 때의 안내(§6.5). 칸의 값은 식이 채우므로 편집 상자 대신 식 첫 줄과
 * 「식 고치기」 를 보인다. Esc 로 닫으면 초점이 표로 돌아간다.
 */
function FormulaNote({ line, onFix, onClose }: { line: string; onFix?: () => void; onClose: () => void }) {
  const fix = useRef<HTMLButtonElement>(null);
  useEffect(() => { fix.current?.focus(); }, []);
  return (
    <div className="grid-formula-note" role="dialog" aria-label="가공열 안내"
      onClick={(e) => e.stopPropagation()} onDoubleClick={(e) => e.stopPropagation()} onPointerDown={(e) => e.stopPropagation()}
      onKeyDown={(e) => { if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); onClose(); } }}>
      <p>이 칸은 식이 채웁니다. 값을 바꾸려면 식을 고치세요.</p>
      {line && <code>= {line}</code>}
      <div className="grid-formula-actions">
        {onFix && <button ref={fix} type="button" className="action primary" onClick={onFix}>식 고치기</button>}
        <button ref={onFix ? undefined : fix} type="button" className="action" onClick={onClose}>닫기</button>
      </div>
    </div>
  );
}

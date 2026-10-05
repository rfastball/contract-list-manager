import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { Sheet, UserColumn } from "./types";
import { useFindShortcut } from "./useFindShortcut";

/** 값이 숫자로 읽히면 오른쪽으로 붙인다 — 금액 자릿수가 눈으로 맞아야 한다. */
function isNumeric(value: string): boolean {
  return value.length > 0 && /^[\d,.\-]+$/.test(value);
}

/** 자릿점을 떼고 수로. 정렬에 쓴다 — 문자열로 세우면 1,000 이 900 보다 앞선다. */
function asNumber(value: string): number | null {
  if (!isNumeric(value)) return null;
  const n = Number(value.replaceAll(",", ""));
  return Number.isFinite(n) ? n : null;
}

type Props = {
  initialQuery?: string;
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
  /** 고친 것을 걷어 파서가 읽은 값으로 되돌린다. */
  onRevert: (rowKey: string, field: string) => Promise<void>;
  /**
   * 줄 하나를 지운다. 무엇이 사라지는지 묻는 것은 바깥의 몫이다.
   *
   * <p><b>없으면 단추를 그리지 않는다.</b> 모든 표의 줄이 지우는 개체인 것은 아니다 —
   * 계획은 엑셀에서 오는 읽기 전용이라 여기서 지울 것이 없고, 지울 수 있는 것처럼 보이면
   * 눌러 본 사람에게 다리가 "그런 접수·공고·계약이 없다" 고만 답한다.</p>
   */
  onDelete?: (rowKey: string) => void;
};

type Sort = { column: string; dir: 1 | -1 };

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
  sheet, columns, keyColumn, identityColumns, onEdit, onCorrect, onRevert, onDelete, initialQuery = "",
}: Props) {
  const [query, setQuery] = useState(initialQuery);
  const [sort, setSort] = useState<Sort | null>(null);
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

  const editable = useMemo(() => new Set(sheet.editable), [sheet.editable]);
  const correctable = useMemo(() => new Set(sheet.correctable), [sheet.correctable]);
  const definitions = useMemo(
    () => new Map(columns.map((c) => [c.fieldName, c])),
    [columns],
  );

  const board = useRef<HTMLDivElement>(null);
  const search = useRef<HTMLInputElement>(null);
  const here = useRef<HTMLTableCellElement>(null);
  useEffect(() => { if (initialQuery) search.current?.focus(); }, [initialQuery]);

  useFindShortcut(search);

  const rows = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const found = needle
      ? sheet.rows.filter((r) => sheet.columns.some((c) => (r[c] ?? "").toLowerCase().includes(needle)))
      : sheet.rows;

    if (!sort) return found;

    // 원본을 건드리지 않으려고 베껴서 세운다.
    return [...found].sort((a, b) => {
      const x = a[sort.column] ?? "";
      const y = b[sort.column] ?? "";
      const nx = asNumber(x);
      const ny = asNumber(y);

      // 빈 값은 언제나 뒤로. 오름차순에서 빈 칸이 위를 덮으면 표를 읽을 수 없다.
      if (!x && !y) return 0;
      if (!x) return 1;
      if (!y) return -1;

      const cmp = nx !== null && ny !== null ? nx - ny : x.localeCompare(y, "ko");
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
      const name = 이름(r);
      const nth = seen.get(name) ?? 0;
      seen.set(name, nth + 1);
      ids.set(r, `${name}\u0000${nth}`);
    }
    return (r: Record<string, string>) => ids.get(r) ?? 이름(r);
  }, [sheet.rows, 이름]);

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

  /** 덮개를 걷는다. 파서가 읽은 값이 돌아오므로 표를 다시 읽어야 해서 바깥이 맡는다. */
  const revert = useCallback(async () => {
    if (row === undefined) return;

    setSaving(true);
    try {
      await onRevert(row[keyColumn], column);
    } catch {
      // 바깥이 까닭을 알린다.
    } finally {
      setSaving(false);
    }
  }, [row, column, keyColumn, onRevert]);

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (rows.length === 0) return;
    if (editing !== null) return; // 편집 중에는 입력 상자가 맡는다

    switch (e.key) {
      case "ArrowUp": move(-1, 0); break;
      case "ArrowDown": move(1, 0); break;
      case "ArrowLeft": move(0, -1); break;
      case "ArrowRight": move(0, 1); break;
      case "PageUp": move(-15, 0); break;
      case "PageDown": move(15, 0); break;
      case "Home": go(e.ctrlKey ? 0 : at, 0); break;
      case "End": go(e.ctrlKey ? last : at, sheet.columns.length - 1); break;
      case "Tab": move(0, e.shiftKey ? -1 : 1); break;
      case "F2":
      case "Enter":
        if (canEdit) setDraft(row[column] ?? "");
        else if (e.key === "Enter") move(1, 0);
        break;
      case "Delete":
      case "Backspace":
        // Ctrl 을 짚으면 되돌리기, 그냥 누르면 빈 값. 잘못 읽힌 칸을 <b>비우는 것</b>과
        // 파서 값으로 <b>돌아가는 것</b>은 다른 일이라 키를 나눈다.
        if (e.ctrlKey) { if (original !== undefined) void revert(); }
        else if (canEdit) void commit("");
        break;
      default:
        // 그냥 타자를 치면 그 글자로 편집을 시작한다 — 엑셀과 같다.
        if (canEdit && e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) setDraft(e.key);
        else return;
    }

    e.preventDefault();
  };

  if (sheet.rows.length === 0)
    return <div className="empty-state">표시할 자료가 없습니다. 자료를 가져왔는지, 필터가 적용되어 있는지 확인하세요.</div>;

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
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => { if (e.key === "Escape") { setQuery(""); board.current?.focus(); } }}
        />
        <span className="result-count" role="status">{rows.length}건<span> / 전체 {sheet.rows.length}건</span></span>
        {sort && (
          <button className="action" onClick={() => setSort(null)}>
            {sort.column} {sort.dir > 0 ? "↑" : "↓"} · 정렬 해제
          </button>
        )}

        <span className="spacer" />

        {/* 커서가 선 줄을 지운다. 무엇이 사라지는지는 바깥이 세어 묻는다.
            지우는 개체가 아닌 표(계획)에서는 바깥이 손잡이를 넘기지 않아 서지 않는다. */}
        {onDelete && (
          <button
            className="action danger"
            disabled={row === undefined}
            onClick={() => row && onDelete(이름(row))}
          >
            선택 자료 삭제…
          </button>
        )}
      </div>

      <div className="grid-board" ref={board} tabIndex={0} onKeyDown={onKeyDown}>
        {rows.length === 0 ? (
          <div className="empty-state"><h2>검색 결과가 없습니다</h2><p>다른 검색어로 찾아보세요.</p>
            <button className="action" onClick={() => { setQuery(""); search.current?.focus(); }}>검색 지우기</button>
          </div>
        ) : <table className="grid">
          <thead>
            <tr>
              {sheet.columns.map((c) => (
                <th
                  key={c}
                  className={editable.has(c) ? "editable" : undefined}
                  title={
                    editable.has(c) ? "사용자 입력 열 (클릭하면 정렬)"
                    : correctable.has(c) ? "문서 값을 수정할 수 있는 열 (클릭하면 정렬)"
                    : "클릭하면 정렬"
                  }
                  onClick={() =>
                    setSort((s) =>
                      s?.column !== c ? { column: c, dir: 1 } : s.dir > 0 ? { column: c, dir: -1 } : null,
                    )
                  }
                >
                  {c}
                  {sort?.column === c && <span className="sort">{sort.dir > 0 ? " ↑" : " ↓"}</span>}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.map((r, ri) => (
              // 신원으로 키를 단다 — 자리 번호를 섞으면 줄이 옮길 때마다 새로 태어나 편집 상자가 끊긴다.
              <tr key={신원(r)}>
                {sheet.columns.map((c, ci) => {
                  const current = ri === at && ci === cursor.col;
                  return (
                    <Cell
                      key={c}
                      cellRef={current ? here : undefined}
                      value={r[c] ?? ""}
                      editable={editable.has(c) && 매달린다(r)}
                      correctable={correctable.has(c) && 매달린다(r)}
                      original={sheet.overrides[r[keyColumn]]?.[c]}
                      definition={definitions.get(c)}
                      current={current}
                      draft={current ? editing : null}
                      saving={current && saving}
                      onSelect={() => { setDraft(null); go(ri, ci); board.current?.focus(); }}
                      onOpen={() => setDraft(r[c] ?? "")}

                      onDraft={setDraft}
                      onCommit={(value, move) => {
                        void commit(value);
                        // 「아래」 는 누른 순간 화면에서 바로 아래에 있던 줄이다. 그 줄을 붙들므로,
                        // 정렬 중에 저장해 고친 줄이 다른 자리로 옮겨 가도 커서는 그 줄에 남는다 —
                        // 저장 뒤의 같은 번호를 쓰면 그사이 밀려 올라온 다른 줄에 선다.
                        if (move === "down") go(at + 1, cursor.col);
                        else if (move === "right") go(at, cursor.col + 1);
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

      <div className="grid-status">
        <span>{rows.length ? at + 1 : 0} / {rows.length}</span>
        {rows.length !== sheet.rows.length && <span>· 전체 {sheet.rows.length}</span>}
        <span>· {column}</span>
        {original !== undefined && <span className="fixed">· 수정한 셀 (원래 값: {original || "빈 값"})</span>}
        {canEdit && (
          <span className="hint">
            · F2 또는 입력으로 편집 · Enter 저장 후 아래로 · Esc 취소
            {original !== undefined && " · Ctrl+Delete 원래대로"}
          </span>
        )}
      </div>
    </div>
  );
}

type CellProps = {
  cellRef?: React.Ref<HTMLTableCellElement>;
  value: string;
  editable: boolean;
  correctable: boolean;
  /** 덮개가 걸려 있으면 고치기 전의 값. 없으면 undefined — 빈 문자열과 구별된다. */
  original?: string;
  definition?: UserColumn;
  current: boolean;
  draft: string | null;
  saving: boolean;
  onSelect: () => void;
  onOpen: () => void;
  onDraft: (value: string) => void;
  onCommit: (value: string, move?: "down" | "right") => void;
  onCancel: () => void;
};

function Cell({
  cellRef, value, editable, correctable, original, definition, current, draft, saving,
  onSelect, onOpen, onDraft, onCommit, onCancel,
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
  ].filter(Boolean).join(" ");

  if (draft === null) {
    const shown = [
      "cell",
      isNumeric(value) ? "num" : "",
      value ? "" : "empty",
    ].filter(Boolean).join(" ");

    return (
      <td
        ref={cellRef}
        className={classes || undefined}
        title={original === undefined ? undefined : `수정한 셀 · 원래 값: ${original || "(빈 값)"}`}
        onClick={onSelect}
        onDoubleClick={open ? onOpen : undefined}
      >
        <div ref={shownRef} className={shown}>{value || "—"}</div>
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
    <td ref={cellRef} className={classes}>
      <Editor draft={draft} definition={definition} onDraft={onDraft} onCommit={onCommit} onCancel={onCancel} />
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
  draft, definition, onDraft, onCommit, onCancel,
}: Pick<CellProps, "draft" | "definition" | "onDraft" | "onCommit" | "onCancel"> & { draft: string }) {
  const settled = useRef(false);

  const finish = (value: string, move?: "down" | "right") => {
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
        value={draft}
        onChange={(e) => finish(e.target.value, "down")}
        onKeyDown={(e) => { if (e.key === "Escape") { e.preventDefault(); abort(); } }}
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
      value={draft}
      onChange={(e) => onDraft(e.target.value)}
      onBlur={(e) => finish(e.target.value)}
      onKeyDown={(e) => {
        if (e.key === "Enter") { e.preventDefault(); finish(e.currentTarget.value, "down"); }
        else if (e.key === "Tab") { e.preventDefault(); finish(e.currentTarget.value, "right"); }
        else if (e.key === "Escape") { e.preventDefault(); abort(); }
      }}
    />
  );
}

import { useEffect, useRef, useState } from "react";
import type { Col } from "./model";
import { passThrough, summary } from "./model";
import { Icon } from "../columns/icons";

export type RowState = {
  /** 이 표본에 찍힐 값. undefined 면 아직 셈하지 않았다. */
  value?: string;
  /** 그 뷰의 전체 건 중 빈 값 수. */
  blank?: number;
  /** 한 줄로 모으기에서 계약마다 셈해 이어 붙이는 열인가 — 꼬리 「이어 붙임」. */
  joined: boolean;
  /** 깨진 식의 까닭. */
  broken?: string;
  /** 이름 문제(빈 이름·겹침). */
  nameError?: string;
  /** 미리보기를 계산하지 못했다 — 값 칸은 「계산 중」 이 아니라 「—」 다. */
  failed?: boolean;
};

type Props = {
  columns: Col[];
  states: Map<string, RowState>;
  selected: string | null;
  readOnly: boolean;
  /** 편집기에서 돌아왔을 때 고른 줄에 초점을 돌린다. */
  focusOnMount?: boolean;
  onFocused?: () => void;
  onSelect: (id: string) => void;
  onOpen: (id: string) => void;
  onRename: (id: string, name: string) => void;
  onRemove: (id: string) => void;
  onMove: (id: string, to: number) => void;
};

/**
 * 열 목록과 미리보기를 한 목록으로 합쳤다(§7.2) — 한 줄 = 열 이름 · 식 요약 · 이 표본의 값 · 꼬리.
 * 줄 위에서: ↑↓ 고르기 · Alt+↑↓ 옮기기 · F2 이름 · Delete 빼기 · Enter 식 열기. 끌어 옮길 수도 있다.
 */
export function RecordScript({ columns, states, selected, readOnly, focusOnMount, onFocused, onSelect, onOpen, onRename, onRemove, onMove }: Props) {
  const [renaming, setRenaming] = useState<{ id: string; before: string } | null>(null);
  const [drag, setDrag] = useState<{ id: string; over: number } | null>(null);
  const rows = useRef(new Map<string, HTMLLIElement>());
  const focusRow = (id: string | undefined) => { if (id) requestAnimationFrame(() => rows.current.get(id)?.focus()); };
  const sel = selected && columns.some(c => c.id === selected) ? selected : columns[0]?.id ?? null;

  useEffect(() => {
    if (!focusOnMount || !sel) return;
    rows.current.get(sel)?.focus(); onFocused?.();
  }, []); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => { if (renaming && !columns.some(c => c.id === renaming.id)) setRenaming(null); }, [columns, renaming]);

  const endRename = (id: string, revert: boolean) => {
    if (revert && renaming) onRename(id, renaming.before);
    setRenaming(null); focusRow(id);
  };

  return <ol className="uv-script" aria-label="열">
    {columns.map((c, i) => {
      const s = states.get(c.id) ?? { joined: false };
      const text = summary(c);
      const editing = renaming?.id === c.id;
      const label = `${c.name.trim() || "이름 없음"} = ${text || "식 없음"}`;
      return <li key={c.id} ref={el => { if (el) rows.current.set(c.id, el); else rows.current.delete(c.id); }}
        className={["uv-row", c.id === sel ? "is-selected" : "", s.broken || s.nameError ? "has-error" : "",
          drag && drag.over === i && drag.id !== c.id ? "is-drop" : ""].filter(Boolean).join(" ")}
        tabIndex={c.id === sel ? 0 : -1} aria-label={label} aria-current={c.id === sel ? "true" : undefined}
        draggable={!readOnly && !editing}
        onDragStart={e => { e.dataTransfer.effectAllowed = "move"; e.dataTransfer.setData("text/plain", c.id); setDrag({ id: c.id, over: i }); }}
        onDragOver={e => { if (!drag) return; e.preventDefault(); if (drag.over !== i) setDrag({ ...drag, over: i }); }}
        onDrop={e => { e.preventDefault(); if (drag && drag.id !== c.id) { onMove(drag.id, i); focusRow(drag.id); } setDrag(null); }}
        onDragEnd={() => setDrag(null)}
        onClick={() => onSelect(c.id)} onFocus={e => { if (e.target === e.currentTarget) onSelect(c.id); }}
        onDoubleClick={() => onOpen(c.id)}
        onKeyDown={e => {
          if (e.target !== e.currentTarget) return;
          if (e.altKey && (e.key === "ArrowUp" || e.key === "ArrowDown")) {
            e.preventDefault();
            const to = i + (e.key === "ArrowUp" ? -1 : 1);
            if (!readOnly && to >= 0 && to < columns.length) { onMove(c.id, to); focusRow(c.id); }
            return;
          }
          if (e.key === "ArrowUp" || e.key === "ArrowDown") {
            e.preventDefault();
            const next = columns[Math.max(0, Math.min(columns.length - 1, i + (e.key === "ArrowUp" ? -1 : 1)))];
            onSelect(next.id); focusRow(next.id); return;
          }
          if (e.key === "F2" && !readOnly) { e.preventDefault(); setRenaming({ id: c.id, before: c.name }); return; }
          if (e.key === "Delete" && !readOnly) {
            e.preventDefault();
            const next = columns[i + 1] ?? columns[i - 1];
            onRemove(c.id); if (next) { onSelect(next.id); focusRow(next.id); }
            return;
          }
          if (e.key === "Enter") { e.preventDefault(); onOpen(c.id); }
        }}>
        <span className="uv-grip" aria-hidden="true"><Icon name="grip" /></span>
        <span className="uv-field" title={editing ? undefined : c.name}>
          {editing ? <input className="uv-rename" aria-label="열 이름" autoFocus value={c.name} maxLength={40}
            onChange={e => onRename(c.id, e.target.value)}
            onKeyDown={e => {
              e.stopPropagation();
              if (e.key === "Enter") { e.preventDefault(); endRename(c.id, false); }
              else if (e.key === "Escape") { e.preventDefault(); endRename(c.id, true); }
            }} onBlur={() => setRenaming(null)} />
            : c.name.trim() ? c.name : <span className="uv-unnamed">이름 없음</span>}
        </span>
        <span className={`uv-formula code${passThrough(c) ? " is-same" : ""}${text ? "" : " is-empty"}`} title={text}>
          <span className="uv-op">=</span> {text || <span className="uv-unnamed">식 없음</span>}</span>
        <span className="uv-value" title={!s.broken && c.formula && s.value ? s.value : undefined}>
          {s.broken ? <span className="trace-pending">—</span>
            : !c.formula ? <span className="trace-pending">—</span>
            : s.value === undefined ? <span className="trace-pending">{s.failed ? "—" : "계산 중"}</span>
            : s.value ? s.value : <span className="uv-blank">(빈 값)</span>}
        </span>
        <span className="uv-tails">
          {s.joined && !s.broken && <span className="uv-tail">이어 붙임</span>}
          {!!s.blank && !s.broken && <span className="uv-tail is-warn">빈 값 {s.blank.toLocaleString("ko-KR")}건</span>}
          {s.broken && <span className="uv-tail is-broken"><Icon name="alert" size={14} />깨진 식</span>}
        </span>
        <button type="button" className="icon-button uv-open" tabIndex={-1} aria-label={`${c.name.trim() || "이름 없는 열"} 식 열기`}
          onClick={e => { e.stopPropagation(); onOpen(c.id); }}><Icon name="arrow" /></button>
        {(s.nameError || s.broken) && <p className="uv-row-error" role={s.nameError ? "alert" : undefined}>
          <Icon name="alert" size={14} /><span>{s.nameError ?? s.broken}</span></p>}
      </li>;
    })}
  </ol>;
}

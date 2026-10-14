import { useEffect, useId, useRef, useState } from "react";
import type { Op, Shape } from "../flow/ast";
import { shapeLabel } from "../flow/ast";
import { groups, verbsFor } from "../flow/verbs";

/**
 * 「+ 단계」 의 동사 고르개. 지금 꼴에서 쓸 수 있는 동사만 묶음별로 보이고, 쳐서 찾는다(§6.4).
 * ↑↓ 로 옮기고 Enter 로 고르며 Esc 로 닫는다.
 */
export function VerbPicker({ shape, onPick, onClose }: { shape: Shape; onPick: (op: Op) => void; onClose: () => void }) {
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const root = useRef<HTMLDivElement>(null);
  const id = useId();
  const q = query.trim();
  const list = verbsFor(shape).filter(v => !q || v.word.includes(q) || v.desc.includes(q) || v.op.includes(q.toLowerCase()) || (v.op === "nth" && "n번째".includes(q)));
  const ordered = groups.flatMap(g => list.filter(v => v.group === g));
  const current = ordered[Math.min(active, ordered.length - 1)];

  // 보표 아래에서 열리면 고정 하단에 가린다 — 열 때 고르개 전체가 보이도록 감싼 스크롤을 옮긴다.
  useEffect(() => { input.current?.focus({ preventScroll: true }); root.current?.scrollIntoView?.({ block: "nearest" }); }, []);
  useEffect(() => setActive(0), [query]);
  useEffect(() => {
    const away = (e: PointerEvent) => { if (!root.current?.contains(e.target as Node)) onClose(); };
    document.addEventListener("pointerdown", away);
    return () => document.removeEventListener("pointerdown", away);
  }, [onClose]);
  useEffect(() => { document.getElementById(`${id}-${current?.op}`)?.scrollIntoView({ block: "nearest" }); }, [current, id]);

  return <div className="verb-picker" ref={root} role="dialog" aria-label="단계 추가">
    <label className="verb-search"><span className="sr-only">동사 찾기</span>
      <input ref={input} className="text" role="combobox" aria-expanded="true" aria-controls={`${id}-list`} aria-activedescendant={current ? `${id}-${current.op}` : undefined}
        aria-label="동사 찾기" placeholder={`${shapeLabel[shape]}에 쓸 동사 찾기`} value={query} onChange={e => setQuery(e.target.value)}
        onKeyDown={e => {
          if (e.key === "ArrowDown") { e.preventDefault(); setActive(i => Math.min(i + 1, ordered.length - 1)); }
          else if (e.key === "ArrowUp") { e.preventDefault(); setActive(i => Math.max(i - 1, 0)); }
          else if (e.key === "Enter") { e.preventDefault(); if (current) onPick(current.op); }
          else if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); onClose(); }
        }} /></label>
    <div className="verb-list" id={`${id}-list`} role="listbox" aria-label={`${shapeLabel[shape]}에 쓸 수 있는 동사`}>
      {groups.map(g => {
        const items = ordered.filter(v => v.group === g);
        if (!items.length) return null;
        return <div key={g} role="group" aria-label={g} className="verb-group">
          <div className="verb-group-name" aria-hidden="true">{g}</div>
          {items.map(v => <div key={v.op} id={`${id}-${v.op}`} role="option" aria-selected={v === current} className="verb-option"
            onPointerDown={e => e.preventDefault()} onClick={() => onPick(v.op)} onPointerMove={() => setActive(ordered.indexOf(v))}>
            <span className="code verb-word">{v.op === "nth" ? "n번째" : v.word}</span><span className="verb-desc">{v.desc}</span>
          </div>)}
        </div>;
      })}
      {!ordered.length && <p className="column-hint verb-empty">「{q}」에 맞는 동사가 없습니다. 지금 단계({shapeLabel[shape]})에 쓸 수 있는 동사만 보입니다.</p>}
    </div>
  </div>;
}

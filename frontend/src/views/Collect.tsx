import { useState } from "react";
import type { Candidate, CandidateGroup } from "./model";
import { Icon } from "../columns/icons";

type Props = {
  groups: CandidateGroup[];
  /** 뷰에 이미 담긴 후보의 열쇠. */
  included: Set<string>;
  /** 후보 열쇠 → 지금 표본의 값. 아직 셈하지 않았으면 없다. */
  values: Map<string, string> | null;
  readOnly: boolean;
  onToggle: (candidate: Candidate, on: boolean) => void;
  onClose: () => void;
};

/**
 * 열 목록(§7.4). 접수 → 공고 → 계약 세 무리이고, 열마다 지금 표본의 값이 붙는다 — 이름이 아니라 값을 보고 고른다(중심 순간).
 * 검색은 이름과 값 모두를 본다. 담긴 열은 체크된 채 남고, 체크를 풀면 그 열이 뷰에서 빠진다.
 */
export function Collect({ groups, included, values, readOnly, onToggle, onClose }: Props) {
  const [query, setQuery] = useState("");
  const q = query.trim().toLocaleLowerCase();
  const shown = groups.map(g => ({ ...g, items: g.items.filter(c => !q || c.name.toLocaleLowerCase().includes(q) || (values?.get(c.key) ?? "").toLocaleLowerCase().includes(q)) }))
    .filter(g => g.items.length);
  return <aside className="uv-collect-body" aria-label="열 목록">
    <div className="uv-collect-head">
      <h2>열</h2>
      <button type="button" className="icon-button uv-collect-close" aria-label="열 목록 닫기" onClick={onClose}><Icon name="remove" /></button>
      <label className="uv-collect-search"><span className="sr-only">열 찾기</span>
        <input type="search" className="text" aria-label="열 찾기" placeholder="열 이름이나 값으로 찾기" value={query} onChange={e => setQuery(e.target.value)} /></label>
    </div>
    <div className="uv-collect-scroll">
      {shown.map(g => <section key={g.key} className="uv-group" aria-label={g.title}>
        <h3>{g.title}</h3>
        <ul>
          {g.items.map(c => {
            const on = included.has(c.key);
            const value = values?.get(c.key);
            return <li key={c.key}><label className="uv-pick">
              <input type="checkbox" checked={on} disabled={readOnly} aria-label={c.name} onChange={e => onToggle(c, e.target.checked)} />
              <span className="uv-pick-name">{c.name}</span>
              <span className="uv-pick-value" title={value}>{value === undefined ? "" : value || <span className="uv-blank">(빈 값)</span>}</span>
            </label></li>;
          })}
        </ul>
      </section>)}
      {!shown.length && <div className="column-empty"><p>「{query.trim()}」에 맞는 열이 없습니다.</p>
        <button type="button" className="action" onClick={() => setQuery("")}>검색 지우기</button></div>}
    </div>
  </aside>;
}

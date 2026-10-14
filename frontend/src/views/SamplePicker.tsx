import type { UserViewSample } from "../types";
import { Icon } from "../columns/icons";

export type Edge = "manyContracts" | "unlinked";

type Props = {
  samples: UserViewSample[] | null;
  index: number;
  onIndex: (i: number) => void;
  query: string;
  onQuery: (q: string) => void;
  edge: Edge | null;
  onEdge: (e: Edge | null) => void;
  /** 미리보기 자료를 찾지 못한 까닭. 빈 글이면 찾았거나 찾는 중이다 — 맞는 자료가 없는 것과 가른다. */
  error?: string;
  onRetry?: () => void;
};

const chips: [Edge | null, string][] = [[null, "일반"], ["manyContracts", "계약 여러 건"], ["unlinked", "일부 자료 없음"]];

/**
 * 미리보기 자료 고르개(§7.4). 번호·건명 검색과 자료 조건은 미리보기에만 적용한다.
 * 일부 자료 없음은 접수·공고·계약 중 빠진 자료가 있는 건이며 연결 오류를 뜻하지 않는다.
 */
export function SamplePicker({ samples, index, onIndex, query, onQuery, edge, onEdge, error = "", onRetry }: Props) {
  const current = samples?.[index];
  const n = samples?.length ?? 0;
  return <div className="uv-sample" role="group" aria-label="미리보기 자료">
    <span className="uv-sample-label">미리보기 자료</span>
    <div className="uv-sample-step">
      <button type="button" className="icon-button" aria-label="이전 미리보기 자료" disabled={index <= 0} onClick={() => onIndex(index - 1)}><Icon name="back" /></button>
      <span className="uv-sample-current" aria-live="polite">
        {samples === null ? <span className="column-hint">미리보기 자료를 찾는 중…</span>
          : error ? <span className="warn">미리보기 자료를 찾지 못했습니다: {error}</span>
          : current ? <><span className="uv-sample-key">{current.number}</span><span className="uv-sample-title" title={current.title}>{current.title}</span>
            <span className="uv-sample-pos">{index + 1}/{n}</span></>
            : <span className="column-hint">맞는 미리보기 자료가 없습니다</span>}
      </span>
      <button type="button" className="icon-button" aria-label="다음 미리보기 자료" disabled={index >= n - 1} onClick={() => onIndex(index + 1)}><Icon name="chevron" /></button>
    </div>
    {samples !== null && error && onRetry && <button type="button" className="action" onClick={onRetry}>다시 찾기</button>}
    <details className="uv-sample-options">
      <summary>다른 자료로 확인{(query || edge) && <span className="uv-sample-filtered"> · 조건 적용 중</span>}</summary>
      <div className="uv-sample-options-body">
        <p className="column-hint">미리보기 자료만 바꿉니다. 저장되는 보기의 행이나 내보내기에는 영향을 주지 않습니다.</p>
        <label className="uv-sample-find"><Icon name="search" size={14} /><span className="sr-only">미리보기 자료 찾기</span>
          <input type="search" aria-label="미리보기 자료 찾기" placeholder="번호·건명 찾기" value={query} onChange={e => onQuery(e.target.value)} /></label>
        <span className="uv-sample-edges">
          {chips.map(([k, label]) => <button key={k ?? "general"} type="button" className="chip" aria-pressed={edge === k} onClick={() => onEdge(k)}>{label}</button>)}
        </span>
        {samples !== null && !error && !n && (query || edge) && <button type="button" className="action quiet" onClick={() => { onQuery(""); onEdge(null); }}>조건 걷기</button>}
      </div>
    </details>
  </div>;
}

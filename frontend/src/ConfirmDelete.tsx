import { useEffect, useState } from "react";
import { call } from "./bridge";
import { Modal } from "./Modal";
import type { BatchDeletionPlan, DeletionScope, EntityType, Row } from "./types";

const keys: Record<EntityType, string> = { contract: "계약번호", notice: "입찰공고번호", request: "접수번호" };
const labels: Record<EntityType, string> = { contract: "계약", notice: "공고", request: "접수" };

export function ConfirmDelete({ rows, entityType, integrated, onDelete, onClose, fallbackFocus }: {
  rows: Row[]; entityType: EntityType; integrated: boolean;
  onDelete: (keys: string[], scope: DeletionScope, fingerprint: string) => Promise<void>;
  onClose: () => void;
  /** 지운 줄과 함께 연 단추가 걷혔을 때 초점을 줄 곳 — 표. */
  fallbackFocus?: () => HTMLElement | null | undefined;
}) {
  const [target, setTarget] = useState<EntityType | "">(integrated ? "" : entityType);
  const [scope, setScope] = useState<DeletionScope>("seq");
  const [plan, setPlan] = useState<BatchDeletionPlan | null>(null);
  const [loaded, setLoaded] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [retry, setRetry] = useState(0);
  const selectedKeys = target ? [...new Set(rows.map(r => r[keys[target]]).filter(Boolean))] : [];
  const packed = JSON.stringify(selectedKeys);
  const requestKey = `${target}:${scope}:${packed}:${retry}`;
  useEffect(() => {
    let active = true;
    setPlan(null); setError("");
    if (selectedKeys.length) void call<BatchDeletionPlan>("batchDeletionPlan", packed, scope).then(result => {
      if (active) { setPlan(result); setLoaded(requestKey); }
    }, (e: Error) => { if (active) setError(e.message); });
    return () => { active = false; };
  }, [requestKey]);
  async function remove() {
    if (!plan || loaded !== requestKey || busy) return;
    setBusy(true); setError("");
    try { await onDelete(selectedKeys, scope, plan.fingerprint); }
    catch (e) { setError((e as Error).message); setPlan(null); }
    finally { setBusy(false); }
  }
  return <Modal label="자료 삭제" onClose={() => { if (!busy) onClose(); }} className="batch-review" fallbackFocus={fallbackFocus}>
    <h2>자료 삭제</h2>
    {integrated && <label className="field">삭제할 자료 종류
      <select className="text" aria-label="삭제할 자료 종류" value={target} disabled={busy} onChange={e => setTarget(e.target.value as EntityType | "")}>
        <option value="">접수·공고·계약 중 선택</option>
        {(["request", "notice", "contract"] as EntityType[]).map(t => <option key={t} value={t}>{labels[t]}</option>)}
      </select>
      <span className="hint">선택한 {rows.length}행에 포함된 해당 자료만 삭제합니다. 조달 흐름 전체를 삭제하지 않습니다.</span>
    </label>}
    <label className="field">삭제 범위
      <select className="text" aria-label="삭제 범위" value={scope} disabled={busy} onChange={e => setScope(e.target.value as DeletionScope)}>
        <option value="seq">선택한 차수만</option><option value="series">해당 본번호의 전 차수</option>
      </select>
    </label>
    {target && !selectedKeys.length && <p>선택한 행에 {labels[target]} 자료가 없습니다.</p>}
    {target && selectedKeys.length > 0 && !plan && !error && <p role="status">삭제 영향을 확인하는 중…</p>}
    {plan && loaded === requestKey && <>
      <p><b>{labels[target as EntityType]} 문서 {plan.documentCount}건</b> · {plan.seriesCount}개 계열 삭제</p>
      <div className="batch-preview"><table><thead><tr><th>삭제할 자료 번호</th><th>제목</th></tr></thead><tbody>
        {plan.targets.map(t => <tr key={`${t.entityType}:${t.display}`}><td>{t.display}</td><td>{t.title || "제목 없음"}</td></tr>)}
      </tbody></table></div>
      <h3>함께 삭제되는 자료</h3>
      <ul className="tally"><li><span>하위 자료 행</span><b>{plan.childRows}</b></li><li><span>수정한 셀</span><b>{plan.overrides}</b></li>
        <li><span>사용자 입력값</span><b>{plan.userFields}</b></li><li><span>연결</span><b>{plan.links}</b></li></ul>
      <p className="hint">차수가 남으면 남은 최신 자료를 표시합니다. 마지막 차수를 삭제하면 계열의 입력값과 연결도 사라집니다.</p>
    </>}
    {error && <p role="alert">{error} <button className="action" onClick={() => setRetry(n => n + 1)}>다시 검토</button></p>}
    <p className="warn">되돌릴 수 없습니다.</p>
    <div className="row end"><button className="action" disabled={busy} onClick={onClose}>취소</button>
      <button className="action danger" disabled={busy || !plan || loaded !== requestKey} onClick={() => void remove()}>{busy ? "삭제 중…" : "삭제"}</button></div>
  </Modal>;
}

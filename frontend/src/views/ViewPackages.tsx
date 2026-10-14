import { useEffect, useState } from "react";
import { call } from "../bridge";
import type { UserView } from "../types";
import type { ViewPackagePick } from "./packageTypes";
import "../../css/view-packages.css";

const entityNames = { notice: "공고", contract: "계약", request: "접수" };
const actionNames = { add: "새로 추가", reuse: "같은 정의 재사용", conflict: "정의 충돌" };

export function ViewPackages({ readOnly, onImported }: { readOnly: boolean; onImported?: () => void }) {
  const [views, setViews] = useState<UserView[]>([]);
  const [selected, setSelected] = useState<string[]>([]);
  const [pick, setPick] = useState<ViewPackagePick | null>(null);
  const [busy, setBusy] = useState<string | null>("읽는 중…");
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const load = async () => { const items = await call<UserView[]>("userViews"); setViews(items); return items; };
  useEffect(() => {
    let active = true;
    call<UserView[]>("userViews").then(items => { if (active) setViews(items); })
      .catch(e => { if (active) setError(String(e.message)); }).finally(() => { if (active) setBusy(null); });
    return () => { active = false; };
  }, []);
  useEffect(() => { if (readOnly) setPick(null); }, [readOnly]);
  const run = async (label: string, work: () => Promise<void>) => {
    setBusy(label); setError(""); setNotice("");
    try { await work(); } catch (e) { setError(e instanceof Error ? e.message : String(e)); }
    finally { setBusy(null); }
  };
  const exportPackage = () => run("내보내는 중…", async () => {
    const result = await call<{ path?: string; download?: boolean; name?: string } | null>("exportViewPackage", JSON.stringify(selected));
    if (result) setNotice(result.download ? `${result.name} 다운로드를 시작했습니다. 브라우저 다운로드 목록에서 확인하세요.` : `보기 구성 파일을 저장했습니다: ${result.path}`);
  });
  /** 새로 고르다 실패하면 앞 파일의 미리보기를 거둔다 — 남기면 「확인한 구성 추가하기」 가 실패한 것이 아니라 앞 파일을 들인다. */
  const withdrawOnFailure = async <T,>(work: () => Promise<T>) => {
    try { return await work(); } catch (e) { setPick(null); throw e; }
  };
  const pickPackage = () => run("파일 확인 중…", async () => {
    if (readOnly) return;
    const result = await withdrawOnFailure(() => call<ViewPackagePick | null>("pickViewPackage"));
    if (result) setPick(result);
  });
  const pickBuiltin = () => run("예시 보기 확인 중…", async () => {
    if (readOnly) return;
    setPick(await withdrawOnFailure(() => call<ViewPackagePick>("builtinViewPackage")));
  });
  const importPackage = () => run("가져오는 중…", async () => {
    if (readOnly || !pick || pick.preview.errors.length > 0) return;
    const imported = await call<UserView[]>("importViewPackage", pick.json, pick.preview.token);
    setViews(imported); setSelected([]); setPick(null);
    setNotice(`${pick.preview.views.length}개 보기를 추가했습니다. 왼쪽 보기 목록에서 열 수 있습니다.`);
    onImported?.();
  });
  return <div className="view-packages" aria-busy={busy !== null}>
    <p>보기 구성을 JSON 파일로 내보내거나 가져옵니다. 조달 자료와 입력한 값은 담지 않습니다.</p>
    <fieldset className="plain" disabled={busy !== null}>
      <legend>내보낼 보기</legend>
      {views.length > 0 ? <div className="vp-choices">{views.map(view => <label key={view.id}>
        <input type="checkbox" checked={selected.includes(view.id)} onChange={e => setSelected(old => e.target.checked ? [...old, view.id] : old.filter(id => id !== view.id))} />
        <span>{view.name}<small>{view.definition.columns.length}개 열 · {view.definition.contracts === "fold" ? "계약 모으기" : "계약마다 한 줄"}{Object.keys(view.broken).length > 0 ? " · 깨진 열 있음" : ""}</small></span>
      </label>)}</div> : <p className="vp-muted">{busy ? "보기를 읽는 중입니다." : "저장한 보기가 없습니다. 보기 만들기에서 먼저 저장하세요."}</p>}
      <div className="vp-actions">
        <button type="button" className="action" disabled={selected.length === 0} onClick={exportPackage}>선택한 보기 내보내기{selected.length > 0 ? ` (${selected.length})` : ""}</button>
        <button type="button" className="action" disabled={readOnly} onClick={pickPackage}>구성 파일 가져오기…</button>
        <button type="button" className="action" disabled={readOnly} onClick={pickBuiltin}>예시 보기 다시 추가…</button>
        {error && <button type="button" className="action" onClick={() => void run("읽는 중…", async () => { await load(); })}>목록 다시 읽기</button>}
      </div>
    </fieldset>
    <p className="vp-muted">「서식용 예시」는 입찰공고·구매추진안내·계약체결안내 서식을 채우는 기본 보기입니다. 다시 추가하면 처음 구성으로 하나 더 생깁니다.</p>
    {readOnly && <p className="vp-muted">열람 중에는 내보내기만 할 수 있습니다.</p>}
    {pick && <div className="vp-preview">
      <h4>{pick.name} 가져오기 미리보기</h4>
      <p>기존 보기는 유지합니다. 같은 이름은 복사본으로 추가합니다. 맞춤 열의 정의가 다르면 전체 가져오기를 중단합니다.</p>
      <ul>{pick.preview.views.map((view, i) => <li key={i}>{view.name}{view.name !== view.importName && <> → <strong>{view.importName}</strong></>}</li>)}</ul>
      {pick.preview.columns.length > 0 && <><h5>함께 가져올 맞춤 열</h5><ul>{pick.preview.columns.map((column, i) => <li key={i}>{entityNames[column.entityType]}.{column.fieldName} — {actionNames[column.action]}</li>)}</ul></>}
      {pick.preview.includesAllColumnsForSql && <p className="vp-muted">SQL의 간접 참조를 빠뜨리지 않도록 원본의 맞춤 열 정의 전체를 포함했습니다.</p>}
      {pick.preview.errors.length > 0 && <div className="vp-error" role="alert"><strong>가져올 수 없습니다. 아래 까닭을 해결한 뒤 다시 골라 주세요.</strong><ul>{pick.preview.errors.map((issue, i) => <li key={i}>{issue}</li>)}</ul></div>}
      <div className="vp-actions">
        <button type="button" className="action primary" disabled={readOnly || busy !== null || pick.preview.errors.length > 0} onClick={importPackage}>확인한 구성 추가하기</button>
        <button type="button" className="action" disabled={busy !== null} onClick={() => setPick(null)}>취소</button>
      </div>
    </div>}
    {busy && <p role="status">{busy}</p>}
    {error && <p className="vp-error" role="alert">{error}</p>}
    {notice && <p className="vp-success" role="status">{notice}</p>}
  </div>;
}

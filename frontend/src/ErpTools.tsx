import { useEffect, useRef, useState } from "react";
import { call } from "./bridge";

type Field = { source: string; target: string; transform: string; codes?: Record<string, string> };
type Profile = { id: string; entityType: string; fields: Field[]; tables: { source: string; target: string; keys: string[]; fields: Field[]; totalSelector?: string }[] };
type Mapping = { version: number; profiles: Profile[] };
type Versions = { active: Mapping; defaults: Mapping; versions: { revision: string; json: string; active: number; validated: number; created_at: string }[] };
/** screen: 수집 이력을 다시 검토할 때 이력에 적힌 화면 번호(ADR-037). 걸러 담은 이력에는 메뉴 경로가 없다 — 파일은 싣지 않고 원본의 경로를 본다. */
type Snapshot = { profile: string; mappingRevision: string; data: unknown; scope: string; rowMatches: Record<string, number>; screen?: string };
type Change = { id: string; table: string; line: number; field: string; before: string | null; after: string | null; conflict: boolean; override?: string };
type Preview = { entity: string; entityType: string; baseToken: string; mappingRevision: string; itemCount: number; changes: Change[];
  unmatched: { id: string; table: string; sourceKey: string; existing: Record<string, unknown>[] }[] };
const run = <T,>(operation: string, data: unknown = {}) => call<T>("erpTools", operation, JSON.stringify(data));

/**
 * 고급 도구도 동일한 inspect/capture를 사용한다. 매핑 편집은 업무자료를 쓰지 않는다.
 *
 * <p><code>startOpen</code> 이면 펼친 채로 서고 매핑을 곧바로 읽는다 — 나라장터 화면의 「고급」 을 펼친 사람은
 * 이 도구를 쓰러 온 것이라 한 번 더 펼치게 하지 않는다. <code>focusRequest</code> 가 바뀌면 JSON 파일 고르는 자리로
 * 초점을 옮긴다(「JSON 파일로 가져오기…」). 읽는 동안은 칸이 잠겨 초점을 받지 못하므로 다 읽은 뒤에 옮긴다.</p>
 */
export function ErpTools({ onChanged, startOpen = false, focusRequest = 0 }: {
  onChanged?: () => Promise<void>;
  startOpen?: boolean;
  focusRequest?: number;
}) {
  const [config, setConfig] = useState<Versions>();
  const [json, setJson] = useState("");
  const [profile, setProfile] = useState("g2b-request-v1");
  const [source, setSource] = useState("");
  const [sourceName, setSourceName] = useState("");
  /** 수집 이력에서 불러왔으면 그 이력의 화면 번호. 파일을 고르면 걷는다. */
  const [sourceScreen, setSourceScreen] = useState<string>();
  const [preview, setPreview] = useState<Preview>();
  const [pending, setPending] = useState<{ snapshot: Snapshot; baseToken: string; captureId: string; choices: Record<string, string> }>();
  const [snapshot, setSnapshot] = useState<Snapshot>();
  const [matches, setMatches] = useState<Record<string, number>>({});
  const [choices, setChoices] = useState<Record<string, string>>({});
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [refs, setRefs] = useState<{ entityType: string; base: string; target: string; state: string }[]>([]);
  const [history, setHistory] = useState<{ capture_id: string; entity_base: string; entity_seq: string; snapshot_json: string }[]>([]);
  /** 매핑을 읽으러 갔는가. 펼친 채로 서면 효과와 펼침 사건이 함께 읽으러 가지 않게 한다. */
  const asked = useRef(false);
  const jsonInput = useRef<HTMLInputElement>(null);
  const focused = useRef(0);
  async function work(fn: () => Promise<void>) {
    setBusy(true); setError(""); setMessage("");
    try { await fn(); } catch (e) { setError(e instanceof Error ? e.message : String(e)); }
    finally { setBusy(false); }
  }
  function invalidate() { setPending(undefined); setPreview(undefined); setSnapshot(undefined); setChoices({}); }
  async function load() {
    // 읽지 못했으면 다시 펼칠 때 다시 읽으러 간다.
    try { const result = await run<Versions>("list"); setConfig(result); setJson(JSON.stringify(result.active, null, 2)); invalidate(); }
    catch (e) { asked.current = false; throw e; }
  }
  let edited: Mapping | undefined;
  try {
    const value = JSON.parse(json);
    if (Array.isArray(value?.profiles) && value.profiles.every((p: Profile) => p && typeof p.id === "string" && Array.isArray(p.fields) && Array.isArray(p.tables) && [p, ...p.tables].every(g => g && Array.isArray(g.fields) && g.fields.every(f => f && typeof f.source === "string" && typeof f.target === "string")))) edited = value;
  } catch { /* 원문 편집 중에도 오류를 고칠 수 있게 둔다. */ }
  const selected = edited?.profiles?.find(p => p.id === profile);
  function fieldEdit(group: number, index: number, key: keyof Field, value: string) {
    if (!edited || !selected) return;
    const fields = group < 0 ? selected.fields : selected.tables[group].fields;
    if (key !== "codes") fields[index][key] = value;
    setJson(JSON.stringify(edited, null, 2)); invalidate();
  }
  async function inspect() {
    const current = await run<Versions>("list");
    // 서버가 활성 버전의 해시를 반환하므로 JS에 별도 해시 구현을 두지 않는다.
    const revision = await run<string>("saveMapping", { json: JSON.stringify(current.active) });
    const input: Snapshot = { profile, mappingRevision: revision, data: JSON.parse(source), scope: "file", rowMatches: matches,
      ...(sourceScreen ? { screen: sourceScreen } : {}) };
    const result = await run<Preview>("inspect", input);
    setPending(undefined); setSnapshot(input); setPreview(result); setChoices({});
    setMessage(result.unmatched.length ? "기존 행과 대응을 선택한 뒤 다시 미리보기하세요." : "파일에 있는 범위만 비교했습니다.");
  }
  useEffect(() => {
    if (startOpen && !asked.current) { asked.current = true; void work(load); }
  }, []); // 처음 한 번만 — 펼친 채로 선 때다.
  useEffect(() => {
    if (!focusRequest || focusRequest === focused.current || busy) return;
    focused.current = focusRequest;
    jsonInput.current?.scrollIntoView({ block: "center" });
    jsonInput.current?.focus();
  }, [focusRequest, busy]);
  const ready = preview && snapshot && !preview.unmatched.length && preview.changes.filter(c => c.conflict).every(c => choices[c.id]);
  return <details className="erp-tools" open={startOpen || undefined}
    onToggle={e => { if (e.currentTarget.open && !config && !busy && !asked.current) { asked.current = true; void work(load); } }}>
    <summary>고급 · 수집 매핑과 JSON 검토</summary>
    <fieldset disabled={busy}>
      <legend>매핑 관리</legend>
      <label className="field"><span className="label sub">프로필</span><select value={profile} onChange={e => { setProfile(e.target.value); setMatches({}); invalidate(); }}>
        {(edited?.profiles ?? config?.active.profiles ?? []).map(p => <option key={p.id}>{p.id}</option>)}
      </select></label>
      <div className="row">
        <label className="action file">매핑 파일 불러오기<input type="file" accept=".json" onChange={e => { const f = e.target.files?.[0]; if (f) void work(async () => { setJson(await f.text()); invalidate(); }); }}/></label>
        <button className="action" type="button" onClick={() => { if (config) { setJson(JSON.stringify(config.defaults, null, 2)); invalidate(); } }}>기본 매핑 불러오기</button>
        <button className="action" type="button" onClick={() => { const url = URL.createObjectURL(new Blob([json], { type: "application/json" })); const a = document.createElement("a"); a.href = url; a.download = "pclm-mapping.json"; a.click(); setTimeout(() => URL.revokeObjectURL(url), 1000); }}>매핑 내보내기</button>
      </div>
      {selected && <details><summary>필드 대응 편집</summary>
        {[{ target: selected.entityType, fields: selected.fields }, ...selected.tables].map((group, gi) => <div key={gi} className="erp-table-wrap">
          <h3>{group.target}</h3><table><thead><tr><th>원천 필드</th><th>저장 필드</th><th>변환</th></tr></thead><tbody>
            {group.fields.map((f, i) => <tr key={i}><td><input aria-label={`${group.target} ${f.target} 원천`} value={f.source} onChange={e => fieldEdit(gi - 1, i, "source", e.target.value)}/></td>
              <td>{f.target}</td><td><select aria-label={`${group.target} ${f.target} 변환`} value={f.transform} onChange={e => fieldEdit(gi - 1, i, "transform", e.target.value)}>
                {["text", "decimal", "integer", "date", "datetime", "boolean", "code"].map(v => <option key={v}>{v}</option>)}
              </select></td></tr>)}
          </tbody></table>
        </div>)}
      </details>}
      <details><summary>수집 경로·키·코드 대응 JSON 편집</summary>
        <p className="hint">fields 필드 대응 · tables 표와 행 키 (totalSelector 는 옛 매핑 호환용으로 남아 있고 읽지 않는다)</p>
        <textarea aria-label="매핑 JSON" rows={16} value={json} onChange={e => { setJson(e.target.value); invalidate(); }}/>
      </details>
      <div className="row">
        <button className="action" type="button" onClick={() => void work(async () => { await run("saveMapping", { json }); setConfig(await run("list")); setMessage("편집본을 저장했습니다. 샘플 검증 후 활성화하세요."); })}>편집본 저장</button>
        <button className="action" type="button" disabled={!source} onClick={() => void work(async () => { const result = await run<{ rows: unknown[] }>("sample", { json, profile, data: JSON.parse(source) }); setConfig(await run("list")); setMessage(`샘플 변환 ${result.rows.length}행 확인. 업무자료는 변경하지 않았습니다.`); })}>샘플로 검증</button>
        <button className="action" type="button" onClick={() => void work(async () => { const revision = await run<string>("saveMapping", { json }); setConfig(await run("activateMapping", { revision })); invalidate(); setMessage("매핑을 활성화했습니다. 확장에서 다시 확인하세요."); })}>검증한 매핑 활성화</button>
      </div>
      <label className="field"><span className="label sub">저장 버전</span><select defaultValue="" onChange={e => { const v = config?.versions.find(v => v.revision === e.target.value); if (v) { setJson(v.json); invalidate(); } }}>
        <option value="">버전 선택</option>{config?.versions.map(v => <option key={v.revision} value={v.revision}>{v.created_at} {v.active ? "활성" : v.validated ? "검증됨" : "편집본"}</option>)}
      </select></label>
    </fieldset>
    <fieldset disabled={busy}>
      <legend>JSON 자료 검토</legend>
      <label className="action file">JSON 파일 선택<input ref={jsonInput} type="file" accept=".json" onChange={e => { const f = e.target.files?.[0]; if (f) void work(async () => { setSource(await f.text()); setSourceName(f.name); setSourceScreen(undefined); setMatches({}); invalidate(); }); }}/></label>
      <p className="hint">{sourceName || "선택한 자료 없음"}</p>
      <button className="action" type="button" disabled={!source} onClick={() => void work(inspect)}>활성 매핑으로 미리보기</button>
      {preview && <><h3>{preview.entity} · 품목 {preview.itemCount}행</h3>
        {preview.unmatched.map(row => <label className="field" key={row.id}>{row.table} · {row.sourceKey}<select value={matches[row.id] ?? ""} onChange={e => setMatches({ ...matches, [row.id]: Number(e.target.value) })}>
          <option value="" disabled>기존 행 대응 선택</option><option value="0">새 행으로 추가</option>
          {row.existing.map(r => <option key={String(r.line_no)} value={Number(r.line_no)}>{String(r.line_no)} · {String(r.item_name ?? r.name ?? r.file_name ?? "")}</option>)}
        </select></label>)}
        <div className="erp-table-wrap"><table><thead><tr><th>필드</th><th>기존값</th><th>수집값</th><th>적용</th></tr></thead><tbody>
          {preview.changes.map(ch => <tr key={ch.id}><th>{ch.table} {ch.line || ""} · {ch.override || (ch.field === "__row" ? "기존 행 삭제" : ch.field)}</th><td>{ch.before ?? "빈값"}</td><td>{ch.after ?? "빈값으로 지우기"}</td><td>
            {ch.conflict ? <select disabled={Boolean(pending)} aria-label={`${ch.field} 선택`} value={choices[ch.id] ?? ""} onChange={e => setChoices({ ...choices, [ch.id]: e.target.value })}>
              <option value="" disabled>선택 필요</option><option value="keep">기존값 유지</option><option value="apply">{ch.override ? "수집 원값으로 복원" : ch.field === "__row" ? "행 삭제" : "수집값 적용"}</option>
            </select> : "새 값"}</td></tr>)}
        </tbody></table></div>
        <button className="action" type="button" disabled={!ready} onClick={() => void work(async () => {
          const request = pending ?? { snapshot: snapshot!, baseToken: preview.baseToken, captureId: crypto.randomUUID(), choices };
          setPending(request);
          const result = await run<{ changed: boolean }>("capture", request);
          invalidate(); await onChanged?.(); setMessage(result.changed ? "검토한 자료를 저장했습니다." : "동일 자료입니다.");
        })}>{pending ? "같은 저장 요청 결과 확인" : "검토한 자료 저장"}</button>
      </>}
    </fieldset>
    <details><summary>연결 상태·수집 이력</summary>
      <button className="action" type="button" disabled={busy} onClick={() => void work(async () => { setRefs(await run("references")); setHistory(await run("history")); })}>연결·이력 확인</button>
      <ul>{refs.map((r, i) => <li key={i}>{r.base} → {r.target || "참조 없음"} · {r.state}</li>)}</ul>
      <ul>{history.map(h => <li key={h.capture_id}>{h.entity_base}-{h.entity_seq} <button className="action" type="button" onClick={() => void work(async () => {
        const saved = JSON.parse(h.snapshot_json);
        if (!saved.data) throw new Error("이전 형식 이력입니다. 원본 JSON 파일을 다시 선택하세요.");
        // 받는 화면은 화면 번호로만 가린다(ADR-037). 번호를 적기 전의 이력은 어느 화면의 것인지 알 수 없다.
        if (typeof saved.screen !== "string" || !saved.screen) throw new Error("이 수집 이력에는 화면 번호가 없어 다시 검토할 수 없습니다. 원본 JSON 파일을 여세요.");
        setSource(JSON.stringify(saved.data)); setSourceName("허용 필드 수집 이력"); setSourceScreen(saved.screen); setProfile(saved.profile); setMatches({}); invalidate();
      })}>현재 매핑으로 재검토</button></li>)}</ul>
    </details>
    <p role="status" className="hint">{busy ? "처리 중…" : message}</p>{error && <p role="alert">{error}</p>}
  </details>;
}

import { useEffect, useMemo, useRef, useState } from "react";
import { call } from "../bridge";
import { Modal } from "../Modal";
import { Grid } from "../Grid";
import type { Sheet, UserView, UserViewContext, UserViewContracts, UserViewPreview, UserViewSample } from "../types";
import type { FormulaV2 } from "../flow/ast";
import { docWord } from "../flow/ast";
import { checkFlow } from "../flow/check";
import { ColumnFlow } from "../columns/ColumnFlow";
import { Icon } from "../columns/icons";
import { Collect } from "./Collect";
import { RecordScript, type RowState } from "./RecordScript";
import { SamplePicker, type Edge } from "./SamplePicker";
import {
  candidates, envOf, fromView, kindsOf, nameProblems, newId, serialize, stable, uniqueName,
  type Candidate, type Col, type Draft,
} from "./model";
import "../../css/columns.css";
import "../../css/views.css";

type Props = {
  readOnly: boolean;
  showList?: boolean;
  initialViewId?: string | null;
  onViewsChange?: (views: UserView[]) => void;
  onActiveChange?: (id: string | null) => void;
  /** 작업 영역을 떠나려는 요청(사이드바·탭). 저장하지 않은 변경이 있으면 열 관리처럼 묻고, 떠나도 되면 proceed 를 부른다. */
  navigationRequest?: { id: number; proceed: () => void };
  /** 저장하지 않은 편집이 있는지. 창을 닫을 때 앱이 물을 수 있게 바깥으로 올린다. */
  onDirtyChange?: (dirty: boolean) => void;
  refreshVersion?: number;
};

/**
 * 보기(docs/user-views.md §7) — 서식 하나에 필요한 열을 한자리에서 짜는 작업 영역. 왼쪽 보기 목록, 가운데 미리보기(줄 = 열 ·
 * 식 요약 · 이 표본의 값), 오른쪽 열 목록(접수 → 공고 → 계약). 줄을 열면 가운데가 가공열 편집기(값 흐름 보표)로 바뀐다.
 * 한 줄은 조달 건이라 기준을 고르지 않는다 — 사람이 정하는 것은 계약이 여럿인 건의 처리 하나다.
 */
export function UserViews({ readOnly, showList = true, initialViewId, onViewsChange, onActiveChange, navigationRequest, onDirtyChange, refreshVersion = 0 }: Props) {
  const [views, setViews] = useState<UserView[] | null>(null);
  const [listError, setListError] = useState("");
  const [listRetry, setListRetry] = useState(0);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [tableMode, setTableMode] = useState(false);
  const [sheet, setSheet] = useState<Sheet | null>(null);
  const [sheetError, setSheetError] = useState("");
  const [sheetRetry, setSheetRetry] = useState(0);
  const savedDraft = useRef<Draft | null>(null);
  const [baseline, setBaseline] = useState("");
  const [context, setContext] = useState<UserViewContext | null>(null);
  const [contextError, setContextError] = useState("");
  const [contextRetry, setContextRetry] = useState(0);
  // 찾지 못했으면 error 에 까닭을 둔다 — 빈 목록(맞는 자료가 없다)과 가른다.
  const [samples, setSamples] = useState<{ for: string; list: UserViewSample[]; error?: string } | null>(null);
  const [samplesRetry, setSamplesRetry] = useState(0);
  const [sampleIndex, setSampleIndex] = useState(0);
  const [sampleQuery, setSampleQuery] = useState("");
  const [edge, setEdge] = useState<Edge | null>(null);
  const [preview, setPreview] = useState<UserViewPreview | null>(null);
  const [previewError, setPreviewError] = useState("");
  const [previewRetry, setPreviewRetry] = useState(0);
  const [pickValues, setPickValues] = useState<{ for: string; values: Map<string, string> } | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [editing, setEditing] = useState<string | null>(null);
  const [returning, setReturning] = useState(false);
  const [drawer, setDrawer] = useState(false);
  /**
   * 담은 열이 어느 후보에서 왔는지(열 id → 후보 열쇠). 저장 꼴에는 싣지 않는다. 식만 맞대어 체크를 가르면 담은 열의 식을 고치는
   * 순간 체크가 풀린 채 열은 남고(다시 체크하면 겹친다), 같은 식의 다른 열 체크를 풀면 처음 맞는 열이 빠진다.
   */
  const [origins, setOrigins] = useState<Record<string, string>>({});
  /** 보기를 연 차례. 연 보기의 열이 어느 후보인지는 열 때 한 번 식으로 맞대어 둔다. */
  const [opened, setOpened] = useState(0);
  const [pending, setPending] = useState<{ run: () => void } | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [codeProblem, setCodeProblem] = useState("");
  const [notice, setNotice] = useState("");
  const nameInput = useRef<HTMLInputElement>(null);
  const columnNameInput = useRef<HTMLInputElement>(null);
  const requestSeen = useRef<number | undefined>(undefined);
  const drawerRef = useRef<HTMLDivElement>(null);
  const drawerToggle = useRef<HTMLButtonElement>(null);
  /** 좁은 창의 열 목록 서랍을 사람이 여닫았는가 — 그때만 초점을 옮긴다. 새 보기가 서랍을 펼 때는 이름 칸이 초점을 쥔다. */
  const drawerMove = useRef<"open" | "close" | null>(null);

  const env = useMemo(() => envOf(context), [context]);
  const current = stable(draft);
  const dirty = !!draft && current !== baseline;
  useEffect(() => { onDirtyChange?.(dirty); }, [dirty, onDirtyChange]);
  const fold = draft?.contracts === "fold";
  // 표본은 그 뷰의 줄에서 고른다 — 줄은 모으기와 담긴 문서 종류가 정한다. 이름·차례가 바뀌어도 다시 찾지 않는다.
  const kinds = useMemo(() => [...new Set((draft?.columns ?? []).flatMap(c => [...kindsOf(c)]))].sort().join(","), [draft?.columns]);
  const basis = draft ? `${draft.contracts}|${kinds}` : "";
  const samplesFor = `${basis}|${sampleQuery.trim()}|${edge ?? ""}`;
  const sampleList = samples && samples.for.startsWith(`${basis}|`) ? samples.list : null;
  const sampleError = samples && samples.for.startsWith(`${basis}|`) ? samples.error ?? "" : "";
  const sampleKey = sampleList?.[sampleIndex]?.key ?? "";
  const editingCol = draft?.columns.find(c => c.id === editing) ?? null;
  // 편집기 보표의 표본 — 미리보기가 보는 표본이 표본 A, 고르개 목록의 다음 둘이 B·C 다.
  const traceKeys = useMemo(() => sampleList?.slice(sampleIndex, sampleIndex + 3).map(s => s.key), [sampleList, sampleIndex]);

  function open(next: Draft | null, fresh = true) {
    savedDraft.current = next?.id ? next : null;
    setTableMode(!!next?.id);
    setDraft(next); setBaseline(fresh ? stable(next) : "");
    setEditing(null); setSelected(next?.columns[0]?.id ?? null); setError(""); setNotice("");
    setSampleQuery(""); setEdge(null); setSampleIndex(0); setDrawer(false); setReturning(false);
    setOrigins({}); setOpened(n => n + 1);
  }
  function navigate(run: () => void) {
    if (busy) return;
    if (dirty) setPending({ run });
    else run();
  }
  function change(patch: Partial<Draft>) {
    if (!draft || readOnly) return;
    setDraft({ ...draft, ...patch }); setError(""); setNotice("");
  }
  const setColumns = (columns: Col[]) => change({ columns });
  const patchColumn = (id: string, patch: Partial<Col>) => draft && setColumns(draft.columns.map(c => c.id === id ? { ...c, ...patch } : c));

  useEffect(() => { if (views) onViewsChange?.(views); }, [views, onViewsChange]);
  useEffect(() => { onActiveChange?.(draft?.id ?? null); }, [draft?.id, onActiveChange]);
  useEffect(() => {
    if (!tableMode || !draft?.id) return;
    let active = true;
    setSheet(null); setSheetError("");
    void call<Sheet>("sheet", `u_${draft.name}`).then(data => {
      if (active) setSheet({ ...data, editable: [], correctable: [], overrides: {} });
    }).catch((e: Error) => { if (active) setSheetError(e.message); });
    return () => { active = false; };
  }, [tableMode, draft?.id, draft?.name, sheetRetry, refreshVersion]);
  useEffect(() => {
    if (draft && !draft.id) nameInput.current?.focus();
  }, [draft?.id]);
  useEffect(() => {
    // 편집기로 가면 서랍을 걷는다 — 펼친 채 두면 Esc 로 돌아올 때 서랍이 다시 미리보기를 덮는다.
    if (editing) { setDrawer(false); columnNameInput.current?.focus(); }
  }, [editing]);
  useEffect(() => {
    const move = drawerMove.current;
    drawerMove.current = null;
    if (move === "open") drawerRef.current?.querySelector<HTMLElement>("button:not(:disabled), input:not(:disabled)")?.focus();
    else if (move === "close") drawerToggle.current?.focus();
  }, [drawer]);
  function openDrawer() { drawerMove.current = "open"; setDrawer(true); }
  function closeDrawer() { drawerMove.current = "close"; setDrawer(false); }

  // 생성 진입점과 빈 보기는 곧장 이름·열 편집으로 연다.
  useEffect(() => {
    let active = true;
    setListError("");
    void call<UserView[]>("userViews").then(list => {
      if (!active) return;
      setViews(list);
      const selected = list.find(view => view.id === initialViewId) ?? list[0];
      if (!readOnly && (initialViewId === null || !selected)) {
        open({ id: null, name: "", contracts: "split", columns: [] });
        setDrawer(true);
      } else if (selected) open(fromView(selected));
    }).catch((e: Error) => { if (active) setListError(e.message); });
    return () => { active = false; };
  }, [listRetry]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    let active = true;
    setContextError("");
    void call<UserViewContext>("userViewContext").then(data => { if (active) setContext(data); })
      .catch((e: Error) => { if (active) setContextError(e.message); });
    return () => { active = false; };
  }, [contextRetry]);
  useEffect(() => {
    if (!draft || tableMode) return;
    let active = true;
    const definition = JSON.stringify(serialize(draft));
    const timer = setTimeout(() => {
      void call<UserViewSample[]>("userViewSamples", definition, sampleQuery.trim() || null, edge)
        .then(list => { if (active) { setSamples({ for: samplesFor, list }); setSampleIndex(0); } })
        .catch((e: Error) => { if (active) setSamples({ for: samplesFor, list: [], error: e.message }); });
    }, sampleQuery ? 200 : 0);
    return () => { active = false; clearTimeout(timer); };
  }, [basis, sampleQuery, edge, tableMode, samplesRetry]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    if (!navigationRequest || requestSeen.current === navigationRequest.id || busy) return;
    requestSeen.current = navigationRequest.id;
    navigate(navigationRequest.proceed);
  }, [navigationRequest, busy]); // eslint-disable-line react-hooks/exhaustive-deps

  // 미리보기 — 식이 있는 열만 보낸다. 표본을 찾은 뒤에 센다.
  const payload = useMemo(() => draft ? JSON.stringify(serialize(draft)) : "", [draft]);
  useEffect(() => {
    if (!payload || sampleList === null || tableMode) return;
    let active = true;
    const timer = setTimeout(() => {
      void call<UserViewPreview>("previewUserView", payload, sampleKey).then(p => { if (active) { setPreview(p); setPreviewError(""); } })
        .catch((e: Error) => { if (active) setPreviewError(e.message); });
    }, 120);
    return () => { active = false; clearTimeout(timer); };
  }, [payload, sampleKey, sampleList === null, tableMode, previewRetry]); // eslint-disable-line react-hooks/exhaustive-deps

  // 열 목록의 후보와 지금 표본의 값. 후보 열쇠를 열 식별자로 써서 한 번에 센다 — 모으기도 뷰와 같다.
  const groups = useMemo(() => env ? candidates(env) : [], [env]);
  const pickPayload = useMemo(() => !draft || !groups.length ? "" : JSON.stringify({ name: "", definition: { version: 2, contracts: draft.contracts,
    columns: groups.flatMap(g => g.items).map(c => ({ id: c.key, name: c.key, formula: { version: 2, source: "flow", flow: c.flow } })) } }),
  [draft?.contracts, groups]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    if (!pickPayload || sampleList === null || tableMode) return;
    let active = true;
    const key = sampleKey;
    void call<UserViewPreview>("previewUserView", pickPayload, key).then(p => {
      if (active) setPickValues({ for: pickPayload + key, values: new Map(p.columns.filter(c => !c.error).map(c => [c.id, p.sample ? c.value : ""])) });
    }).catch(() => {}); // 곁 정보라 알리지 않는다 — 값 칸은 비어 있을 뿐 「계산 중」 으로 서지 않는다.
    return () => { active = false; };
  }, [pickPayload, sampleKey, sampleList === null, tableMode]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    if (!groups.length) return;
    // 연 보기의 열은 어느 후보에서 왔는지 적혀 있지 않다 — 열 때 한 번, 식이 같은 후보(아직 다른 열이 차지하지 않은)로 맞대어 둔다.
    const all = groups.flatMap(g => g.items);
    setOrigins(prev => {
      const next = { ...prev };
      const claimed = new Set(Object.values(next));
      for (const col of draft?.columns ?? []) {
        if (next[col.id] || col.formula?.source !== "flow") continue;
        const key = stable(col.formula.flow);
        const found = all.find(c => !claimed.has(c.key) && stable(c.flow) === key);
        if (found) { next[col.id] = found.key; claimed.add(found.key); }
      }
      return next;
    });
  }, [groups, opened]); // eslint-disable-line react-hooks/exhaustive-deps
  const included = useMemo(() => new Set((draft?.columns ?? []).flatMap(c => origins[c.id] ? [origins[c.id]] : [])), [draft?.columns, origins]);

  const names = useMemo(() => nameProblems(draft?.columns ?? []), [draft?.columns]);
  const states = useMemo(() => {
    const out = new Map<string, RowState>();
    // 미리보기나 미리보기 자료를 얻지 못했으면 지난 값을 보이지 않고 「—」 로 둔다 — 「계산 중」 에 머물지 않게.
    const failed = !!previewError || !!sampleError;
    const shown = new Map((failed ? [] : preview?.columns ?? []).map(c => [c.id, c]));
    for (const c of draft?.columns ?? []) {
      const checked = c.formula?.source === "flow" && env ? checkFlow(c.formula.flow, env) : null;
      const p = shown.get(c.id);
      const broken = checked && !checked.ok ? checked.errors[0].message : p?.error;
      out.set(c.id, {
        joined: fold && kindsOf(c).has("contract"), nameError: names.get(c.id), broken,
        value: p && !broken ? (preview?.sample ? p.value : undefined) : undefined,
        blank: p?.blank,
        ...(failed ? { failed } : {}),
      });
    }
    return out;
  }, [draft?.columns, env, preview, previewError, sampleError, names, fold]);
  const hasContract = !!draft?.columns.some(c => kindsOf(c).has("contract"));
  const brokenCount = [...states.values()].filter(s => s.broken).length;
  const emptyCount = draft?.columns.filter(c => !c.formula).length ?? 0;
  const others = (views ?? []).filter(v => v.id !== draft?.id).map(v => v.name.toLocaleLowerCase());
  const total = preview?.total ?? context?.total ?? null;
  const reason = !draft ? ""
    : !draft.name.trim() ? "보기 이름을 적으세요."
    : others.includes(draft.name.trim().toLocaleLowerCase()) ? `‘${draft.name.trim()}’ 보기가 이미 있습니다.`
    : !draft.columns.length ? "열을 하나 이상 추가하세요."
    : names.size ? `이름이 비었거나 겹치는 열이 ${names.size}개 있습니다.`
    : emptyCount ? `식을 정하지 않은 열이 ${emptyCount}개 있습니다.`
    : brokenCount ? `식이 깨진 열이 ${brokenCount}개 있습니다.`
    : "";
  const canSave = !readOnly && !busy && !!draft && !reason && (dirty || !draft.id);

  async function save(): Promise<boolean> {
    if (!draft || busy || readOnly) return false;
    if (reason) { setError(`저장할 수 없습니다. ${reason}`); return false; }
    if (editingCol && codeProblem) { setError(`흐름식을 읽을 수 없어 저장하지 않았습니다 — ${codeProblem}`); return false; }
    setBusy(true); setError("");
    try {
      const name = draft.name.trim();
      const list = await call<UserView[]>("saveUserView", JSON.stringify(serialize(draft)), draft.id);
      setViews(list);
      const saved = list.find(v => v.name === name);
      // 열 id 는 저장을 지나도 같다 — 어느 후보에서 왔는지도 이어 쥔다(저장 꼴에는 싣지 않는다).
      const kept = origins;
      if (saved) { open(fromView(saved)); setOrigins(kept); }
      setNotice(`‘${name}’ 보기를 저장했습니다.`);
      return true;
    } catch (e) { setError((e as Error).message); return false; }
    finally { setBusy(false); }
  }

  function newView() {
    if (readOnly) return;
    navigate(() => {
      open({ id: null, name: "", contracts: "split", columns: [] });
      setDrawer(true);
      requestAnimationFrame(() => nameInput.current?.focus());
    });
  }
  function toggle(c: Candidate, on: boolean) {
    if (!draft || readOnly) return;
    if (on) {
      const col: Col = { id: newId(), name: uniqueName(c.name, draft.columns, docWord[c.link]), formula: { version: 2, id: "", source: "flow", flow: structuredClone(c.flow) } };
      setColumns([...draft.columns, col]); setSelected(col.id); setOrigins(o => ({ ...o, [col.id]: c.key }));
      return;
    }
    const at = draft.columns.findIndex(col => origins[col.id] === c.key);
    if (at >= 0) setColumns(draft.columns.filter((_, i) => i !== at));
  }
  function addComputed() {
    if (!draft || readOnly) return;
    const col: Col = { id: newId(), name: "", formula: null };
    setColumns([...draft.columns, col]); setSelected(col.id); setEditing(col.id);
  }
  function move(id: string, to: number) {
    if (!draft) return;
    const from = draft.columns.findIndex(c => c.id === id);
    if (from < 0 || to < 0 || to >= draft.columns.length || from === to) return;
    const columns = [...draft.columns];
    columns.splice(to, 0, ...columns.splice(from, 1));
    setColumns(columns);
  }
  function closeEditor() {
    if (editing) setSelected(editing);
    setEditing(null); setReturning(true);
  }
  function setContracts(contracts: UserViewContracts) {
    if (draft && draft.contracts !== contracts) change({ contracts });
  }
  async function remove() {
    if (!draft || busy) return;
    const name = draft.name.trim();
    if (!draft.id) { setDeleting(false); open(views?.[0] ? fromView(views[0]) : null); return; }
    setBusy(true);
    try {
      const list = await call<UserView[]>("deleteUserView", draft.id);
      setViews(list); setDeleting(false);
      open(list[0] ? fromView(list[0]) : null);
      setNotice(`‘${name}’ 보기를 지웠습니다.`);
    } catch (e) { setError((e as Error).message); setDeleting(false); }
    finally { setBusy(false); }
  }

  const heading = <>
    <header className="top uv-top">
      <div className="page-heading"><h1>{draft?.name.trim() || (draft ? "보기 만들기" : "내 보기")}</h1><p>{tableMode ? "저장한 열 구성으로 접수·공고·계약 자료를 봅니다." : "접수·공고·계약에서 필요한 열을 모으고, 사용할 이름을 정하세요."}</p></div>
    </header>
    {readOnly && <p className="column-context">열람 중이라 보기를 고칠 수 없습니다.</p>}
  </>;

  if (views === null) return <div className="column-manager uv-page">{heading}<div className="uv-status">
    {listError ? <div role="alert" className="column-context-error">{listError}<button className="action" onClick={() => setListRetry(n => n + 1)}>다시 불러오기</button></div>
      : <p role="status" className="column-hint">보기를 불러오는 중…</p>}
  </div></div>;

  if (!draft) return <div className="column-manager uv-page">{heading}
    <section className="uv-empty" aria-label="보기 없음">
      <h2>아직 보기가 없습니다</h2>
      <p>서식이 받을 열을 고르면 한 건에 한 줄씩 자료가 섭니다. 접수·공고·계약의 열을 섞어 담아도 같은 조달 건끼리 앱이 잇습니다.</p>
      {!readOnly && <button type="button" className="action primary" onClick={newView}><Icon name="plus" />보기 만들기</button>}
      {notice && <p className="column-notice" role="status">{notice}</p>}
    </section>
  </div>;

  const contextStatus = contextError ? <div role="alert" className="column-context-error">{contextError}<button className="action" onClick={() => setContextRetry(n => n + 1)}>자료 다시 불러오기</button></div>
    : !context ? <p role="status" className="column-hint">열과 미리보기 자료를 불러오는 중…</p> : null;

  return <div className="column-manager uv-page">{heading}<div className={`uv${editingCol ? " is-editing" : ""}${tableMode ? " is-table" : ""}${showList ? "" : " without-list"}`}>
    <header className="uv-head">
      {showList && <label className="uv-picker"><span className="sr-only">보기 고르기</span>
        {/* 좁은 창에서 뷰 보기 대신 선다. 지금 뷰의 이름은 옆의 이름 칸이 보이므로 여기서는 되풀이하지 않는다. */}
        <select aria-label="보기 고르기" value="" disabled={busy} onChange={e => {
          const v = e.target.value;
          if (v === "__new") { newView(); return; }
          const view = views.find(x => x.id === v);
          if (view && view.id !== draft.id) navigate(() => open(fromView(view)));
        }}>
          <option value="" disabled hidden>{views.length ? `보기 ${views.length}개` : "내 보기 없음"}</option>
          {views.map(v => <option key={v.id} value={v.id} disabled={v.id === draft.id}>{v.name}{v.id === draft.id ? " (지금)" : ""}</option>)}
          {!readOnly && <option value="__new">+ 보기 만들기</option>}
        </select></label>}
      <div className="uv-title">
        {!tableMode && <label className={`uv-name${draft.name.trim() ? "" : " is-unnamed"}`}>
          <span>보기 이름</span>
          <input ref={nameInput} aria-label="보기 이름" value={draft.name} placeholder="보기 이름을 입력하세요" maxLength={40} disabled={readOnly || busy}
            size={Math.max(18, Math.min(24, draft.name.length + 1))} onChange={e => change({ name: e.target.value })} /></label>}
        <span className="uv-meta">
          <span>{hasContract && !fold ? "한 줄 = 계약" : "한 줄 = 조달 건"}</span>
          {!tableMode && total !== null && <span className="uv-total">· {total.toLocaleString("ko-KR")}건</span>}
        </span>
      </div>
      <div className="uv-actions">
        {!tableMode && brokenCount > 0 && <span className="cm-state is-error"><Icon name="alert" size={14} />깨진 식 {brokenCount}</span>}
        <span className={`cm-state${dirty ? " is-dirty" : ""}`}>{!draft.id ? "저장 전" : dirty ? "변경됨" : "저장됨"}</span>
        {tableMode ? !readOnly && <button type="button" className="action" onClick={() => setTableMode(false)}>보기 편집</button>
          : <>
            {draft.id && <button type="button" className="action" disabled={busy} onClick={() => navigate(() => open(savedDraft.current))}>표로 돌아가기</button>}
            {!draft.id && !readOnly && <button type="button" className="action" disabled={busy} onClick={() => { if (dirty) setDeleting(true); else void remove(); }}>취소</button>}
            {!editingCol && <button ref={drawerToggle} type="button" className="action uv-collect-toggle" aria-expanded={drawer} onClick={() => drawer ? closeDrawer() : openDrawer()}><Icon name="plus" />열 추가</button>}
            {!readOnly && <button type="button" className="icon-button danger" aria-label="보기 지우기" disabled={busy} onClick={() => setDeleting(true)}><Icon name="trash" /></button>}
            {!readOnly && <button type="button" className="action primary" disabled={!canSave} aria-describedby={reason ? "uv-reason" : undefined} onClick={() => void save()}>저장</button>}
          </>}
      </div>
      <p className="column-notice" role="status">{busy ? "처리 중…" : notice}</p>
    </header>
    {!tableMode && hasContract && <div className="uv-contracts">
      <span className="uv-contracts-label">계약이 여럿인 건 <span className="uv-contracts-count">(이 자료에 {(context?.manyContracts ?? 0).toLocaleString("ko-KR")}건)</span></span>
      <div className="seg" role="group" aria-label="계약이 여럿인 건">
        <button type="button" aria-pressed={!fold} disabled={readOnly || busy} onClick={() => setContracts("split")}>계약마다 한 줄</button>
        <button type="button" aria-pressed={fold} disabled={readOnly || busy} onClick={() => setContracts("fold")}>조달 건마다 한 줄</button>
      </div>
      {fold && <p className="column-hint uv-contracts-hint">계약 값은 ‘, ’로 이어 붙입니다. 금액도 합산하지 않고 각각 표시합니다.</p>}
    </div>}
    {!tableMode && (error || reason) && <p id="uv-reason" className={`uv-reason${error ? " is-error" : ""}`} role={error ? "alert" : undefined}>
      <Icon name="alert" size={14} />{error || `저장할 수 없습니다. ${reason}`}</p>}

    <div className="uv-body">
      {showList && <nav className="uv-list" aria-label="내 보기">
        <p className="cm-count">보기 <span>{views.length}</span></p>
        <ul className="cm-rows">
          {views.map(v => {
            const now = draft.id === v.id;
            const broken = Object.keys(v.broken).length;
            return <li key={v.id}><button type="button" className="cm-row uv-view" disabled={busy} aria-current={now ? "true" : undefined}
              aria-label={`${v.name} · 열 ${v.definition.columns.length}${broken ? ` · 깨진 식 ${broken}` : ""}`}
              onClick={() => { if (!now) navigate(() => open(fromView(v))); }}>
              <span className="cm-name">{v.name}</span>
              <span className={`cm-tail${broken ? " is-broken" : ""}`}>{broken ? <><Icon name="alert" size={14} />깨진 식 {broken}</> : v.definition.columns.length}</span>
            </button></li>;
          })}
          {!draft.id && <li><button type="button" className="cm-row uv-view" aria-current="true">
            <span className="cm-name">{draft.name.trim() || "새 보기"}</span><span className="cm-tail">저장 전</span></button></li>}
        </ul>
        {!readOnly && <button type="button" className="action quiet uv-new" disabled={busy} onClick={newView}><Icon name="plus" />보기 만들기</button>}
      </nav>}

      <div className="uv-main">
        {tableMode ? <section className="uv-table" aria-label="보기 결과">
          {Object.keys(views.find(v => v.id === draft.id)?.broken ?? {}).length > 0 && <p className="uv-reason is-error" role="alert">깨진 식의 값은 빈 칸으로 표시합니다.{!readOnly && " 보기 편집에서 식을 확인하세요."}</p>}
          {sheetError ? <div className="empty-state" role="alert"><h2>자료를 읽지 못했습니다</h2><p>{sheetError}</p><button className="action" onClick={() => setSheetRetry(n => n + 1)}>다시 읽기</button></div>
            : !sheet ? <div className="empty-state" role="status">자료를 읽는 중…</div>
            : !sheet.rows.length ? <div className="empty-state"><h2>이 보기에 표시할 자료가 없습니다</h2><p>자료를 수집하거나 보기의 열 구성을 확인하세요.</p></div>
            : <Grid sheet={sheet} columns={[]} keyColumn={sheet.columns[0] ?? ""} identityColumns={sheet.columns}
              onEdit={async () => {}} onCorrect={async () => {}} />}
        </section> : editingCol ? <section className="uv-editor" aria-label="열 편집" onKeyDown={e => {
          if (e.key !== "Escape" || e.defaultPrevented) return;
          if ((e.target as HTMLElement).closest(".cm-content, dialog")) return;
          e.preventDefault(); closeEditor();
        }}>
          <nav className="uv-crumbs" aria-label="경로">
            <button type="button" className="action quiet uv-crumb-back" onClick={closeEditor}><Icon name="back" />{draft.name.trim() || "새 보기"}</button>
            <Icon name="chevron" size={14} />
            <span>열 편집</span>
            <span className="column-hint uv-crumb-hint">Esc 돌아가기</span>
          </nav>
          <label className="uv-column-name">열 이름
            <input ref={columnNameInput} className="text" aria-label="열 이름" value={editingCol.name} placeholder="예: 계약 금액 모음" maxLength={40} disabled={readOnly || busy}
              onChange={e => patchColumn(editingCol.id, { name: e.target.value })} />
          </label>
          {names.get(editingCol.id) && <p className="uv-row-error"><Icon name="alert" size={14} /><span>{names.get(editingCol.id)}</span></p>}
          <div className="uv-editor-scroll">
            {contextStatus}
            <ColumnFlow key={editingCol.id} onCodeProblem={setCodeProblem} userView sampleKeys={traceKeys} formula={editingCol.formula} context={context} env={env} readOnly={readOnly || busy}
              onChange={(formula: FormulaV2 | null) => patchColumn(editingCol.id, { formula })}
              onSuggestName={name => { if (!editingCol.name.trim()) patchColumn(editingCol.id, { name: uniqueName(name, draft.columns, "") }); }} />
          </div>
        </section>
          : <section className="uv-record" aria-label="미리보기">
            <SamplePicker samples={sampleList} index={sampleIndex} onIndex={setSampleIndex}
              query={sampleQuery} onQuery={setSampleQuery} edge={edge} onEdge={setEdge}
              error={sampleError} onRetry={() => { setSamples(null); setSamplesRetry(n => n + 1); }} />
            <div className="uv-record-scroll">
              {contextStatus}
              {previewError && !sampleError && <div role="alert" className="column-context-error">미리보기를 계산하지 못했습니다: {previewError}
                <button className="action" onClick={() => setPreviewRetry(n => n + 1)}>다시 계산</button></div>}
              {draft.columns.length ? <RecordScript columns={draft.columns} states={states} selected={selected} readOnly={readOnly}
                focusOnMount={returning} onFocused={() => setReturning(false)} onSelect={setSelected} onOpen={id => { setSelected(id); setEditing(id); }}
                onRename={(id, name) => patchColumn(id, { name })} onRemove={id => setColumns(draft.columns.filter(c => c.id !== id))} onMove={move} />
                : <div className="uv-empty-record"><h2>열이 없습니다</h2><p>오른쪽 열 목록에서 체크하면 차례대로 여기에 섭니다.</p></div>}
              <div className="uv-record-foot">
                {!readOnly && <button type="button" className="action quiet" disabled={busy} onClick={addComputed}><Icon name="plus" />식으로 만들기</button>}
                <p className="column-hint">↑↓ 고르기 · Alt+↑↓ 옮기기 · F2 이름 · Delete 빼기 · Enter 식 열기</p>
              </div>
            </div>
          </section>}
      </div>

      {!tableMode && !editingCol && <div ref={drawerRef} className={`uv-collect${drawer ? " is-open" : ""}`} onKeyDown={e => {
        if (e.key !== "Escape" || e.defaultPrevented || !drawer) return;
        e.preventDefault(); closeDrawer();
      }}>
        {env ? <Collect groups={groups} included={included} readOnly={readOnly}
          values={pickValues && pickValues.for === pickPayload + sampleKey ? pickValues.values : null}
          onToggle={toggle} onClose={closeDrawer} /> : <div className="uv-collect-body">{contextStatus}</div>}
      </div>}
    </div>

    {pending && <Modal label="저장하지 않은 보기 변경" onClose={() => { if (!busy) setPending(null); }}>
      <h2>저장하지 않은 변경이 있습니다</h2><p>{draft.name.trim() ? `‘${draft.name.trim()}’ 보기의 ` : ""}변경을 저장하고 이동할까요?</p>
      {reason && !error && <p className="column-hint" id="uv-pending-reason">지금은 저장할 수 없습니다. {reason}</p>}
      {error && <p className="warn" role="alert">{error}</p>}
      <div className="column-controls"><button className="action" disabled={busy} onClick={() => setPending(null)}>계속 편집</button>
        <button className="action" disabled={busy} onClick={() => { const next = pending.run; setPending(null); setBaseline(current); next(); }}>변경 버리기</button>
        <button className="action primary" disabled={busy || !!reason} title={reason || undefined} aria-describedby={reason && !error ? "uv-pending-reason" : undefined}
          onClick={async () => { const next = pending.run; if (await save()) { setPending(null); next(); } }}>저장 후 이동</button></div>
    </Modal>}
    {deleting && (draft.id ? <Modal label="보기 지우기 확인" onClose={() => { if (!busy) setDeleting(false); }}>
      <h2>‘{draft.name.trim()}’ 보기를 지울까요?</h2>
      <p>이 보기를 쓰는 서식은 u_{draft.name.trim()} 를 더 찾지 못합니다.</p>
      <p className="column-hint">자료는 그대로 두고 보기의 구성만 지웁니다. 되돌릴 수 없습니다.</p>
      <div className="column-controls"><button className="action" disabled={busy} onClick={() => setDeleting(false)}>취소</button>
        <button className="action danger" disabled={busy} onClick={() => void remove()}>보기 지우기</button></div>
    </Modal> : <Modal label="만들던 보기 버리기 확인" onClose={() => setDeleting(false)}>
      {/* 저장 전이라 지울 보기도, 그것을 찾지 못할 서식도 없다 — 버리는 것은 이 화면에서 만들던 이름과 열뿐이다. */}
      <h2>만들던 보기를 버릴까요?</h2>
      <p>저장하지 않은 {draft.name.trim() ? `‘${draft.name.trim()}’ 의 ` : ""}이름과 열 구성을 버리고 {views[0] ? `‘${views[0].name}’ 보기로 돌아갑니다.` : "돌아갑니다."}</p>
      <div className="column-controls"><button className="action" onClick={() => setDeleting(false)}>계속 만들기</button>
        <button className="action danger" onClick={() => void remove()}>버리기</button></div>
    </Modal>)}
  </div></div>;
}

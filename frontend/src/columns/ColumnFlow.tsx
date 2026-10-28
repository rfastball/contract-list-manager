import { lazy, Suspense, useEffect, useMemo, useRef, useState } from "react";
import { call } from "../bridge";
import { Modal } from "../Modal";
import type { ColumnContext, ColumnTrace, EntityType, SqlCheck } from "../types";
import type { Flow, FlowEnv, FormulaV2, LinkKind, Op, Shape, Step } from "../flow/ast";
import { docWord, rowColumns, shapeLabel, wordOf } from "../flow/ast";
import { checkFlow } from "../flow/check";
import { defaultStep } from "../flow/verbs";
import { FlowSyntaxError, parseFlow, printArgs, printFlow, printSource, verbWord } from "../flow/syntax";
import { presets } from "../flow";
import type { TraceValue } from "../flow/evaluate";

/** CodeMirror 는 흐름식·SQL 탭을 열 때만 불러온다. */
const FlowCodeEditor = lazy(() => import("../code/editors").then(m => ({ default: m.FlowCodeEditor })));
const SqlCodeEditor = lazy(() => import("../code/editors").then(m => ({ default: m.SqlCodeEditor })));
const loading = <p className="column-hint" role="status">편집기를 여는 중…</p>;
import { StepSettings } from "./StepSettings";
import { VerbPicker } from "./VerbPicker";
import { Icon } from "./icons";

type Tab = "steps" | "code" | "sql";
export type FlowCodeDraft = { text: string; problem: string };
type Props = {
  /** 가공열의 자료 종류. 맞춤 뷰(<code>userView</code>)면 없다 — 바깥 행이 같은 조달 건의 세 문서다. */
  entity?: EntityType;
  formula: FormulaV2 | null;
  context: ColumnContext | null;
  env: FlowEnv | null;
  readOnly: boolean;
  onChange: (f: FormulaV2 | null) => void;
  onSuggestName: (name: string) => void;
  /**
   * 맞춤 뷰의 필드를 고친다(docs/user-views.md §3·§8). 보표는 traceUserViewColumn 이 내고, 「SQL로 꺼내기」의 SQL 도
   * 그 보표가 실어 온다 — 바깥 행이 세 문서라 면 뷰의 가공열 컴파일로는 읽을 수 없다. SQL 필드의 검사도 그 보표로 한다.
   * 글 꼴은 <code>공고.공고명</code> 이다(env.view).
   */
  userView?: boolean;
  /** 맞춤 뷰 보표의 미리보기 자료 열쇠. 미리보기가 보는 미리보기 자료가 처음(미리보기 자료 A)이다. */
  sampleKeys?: string[];
  /** 원문은 탭·열 편집기를 닫아도 초안과 함께 남아야 한다. 저장하는 식과 분리한다. */
  codeDraft: FlowCodeDraft | null;
  onCodeDraftChange: (draft: FlowCodeDraft | null) => void;
};

const sampleName = (i: number) => String.fromCharCode(65 + i);
const dropWord: Partial<Record<Op, string>> = { distinct: "겹쳐 빠짐", filter: "걸러짐", take: "잘림" };
const flowOf = (f: FormulaV2 | null) => f?.source === "flow" ? f.flow : null;

/** `=` 가공열 편집 — 값 흐름 보표(§6.4). 행은 단계, 열은 미리보기 자료 문서. 칸은 그 단계까지의 값. */
export function ColumnFlow({ entity, formula, context, env, readOnly, onChange, onSuggestName, userView = false, sampleKeys, codeDraft, onCodeDraftChange }: Props) {
  const [tab, setTab] = useState<Tab>(formula?.source === "sql" ? "sql" : "steps");
  const [trace, setTrace] = useState<ColumnTrace | null>(null);
  const [tracedFor, setTracedFor] = useState("");
  const [traceError, setTraceError] = useState("");
  const [tracing, setTracing] = useState(false);
  const [sel, setSel] = useState(0);
  const [picking, setPicking] = useState(false);
  const [sample, setSample] = useState(0);
  const [escaping, setEscaping] = useState(false);
  const [compiled, setCompiled] = useState<{ packed: string; sql: string; error?: string } | null>(null);
  const rows = useRef<(HTMLTableRowElement | null)[]>([]);
  const settings = useRef<HTMLElement>(null);
  const addButton = useRef<HTMLButtonElement>(null);
  const flow = flowOf(formula);
  const packed = formula ? JSON.stringify(formula) : "";
  const checked = useMemo(() => flow && env ? checkFlow(flow, env) : null, [flow, env]);
  const links = useMemo(() => (context?.available?.linked ?? []).map(l => l.kind), [context]);
  const mode = { view: userView };

  useEffect(() => { if (formula?.source === "sql") setTab("sql"); }, [formula?.source]);
  // 보표는 흐름식만 돈다. SQL 열은 SqlEdit 가 검증(validateComputedSql)으로 미리보기 자료를 돌린다.
  const tracedFlow = formula?.source === "flow" ? packed : "";
  // 열쇠 목록을 글 하나로 — 의존 비교가 값으로 되게. 조달 건 열쇠에는 「|」 가 들어 있어 JSON 으로 싼다.
  const keys = userView && sampleKeys?.length ? JSON.stringify(sampleKeys) : "";
  useEffect(() => {
    let active = true;
    const packed = tracedFlow;
    setTraceError("");
    if (!packed) { setTrace(null); setTracing(false); return; }
    setTracing(true);
    const timer = setTimeout(() => {
      const traced = userView ? call<ColumnTrace>("traceUserViewColumn", packed, keys ? JSON.parse(keys) as string[] : null) : call<ColumnTrace>("traceComputedColumn", entity, packed);
      void traced.then(r => { if (active) { setTrace(r); setTracedFor(packed); } })
        .catch((e: Error) => { if (active) setTraceError(e.message); })
        .finally(() => { if (active) setTracing(false); });
    }, 200);
    return () => { active = false; clearTimeout(timer); };
  }, [entity, tracedFlow, userView, keys]);
  // 「SQL로 꺼내기」가 낼 SQL — SQL 탭을 볼 때만 다리에 묻는다.
  const compilable = tab === "sql" && formula?.source === "flow" && !!checked?.ok ? packed : "";
  useEffect(() => {
    if (!compilable || userView) return;
    let active = true;
    const timer = setTimeout(() => {
      void call<string>("compileComputedColumn", entity, compilable).then(sql => { if (active) setCompiled({ packed: compilable, sql }); })
        .catch((e: Error) => { if (active) setCompiled({ packed: compilable, sql: "", error: e.message }); });
    }, 200);
    return () => { active = false; clearTimeout(timer); };
  }, [entity, compilable, userView]);
  const ejected = userView ? (compilable && trace && tracedFor === compilable ? { packed: compilable, sql: trace.sql } as { packed: string; sql: string; error?: string } : null)
    : compiled && compiled.packed === compilable ? compiled : null;
  useEffect(() => { if (flow && sel > flow.steps.length) setSel(flow.steps.length); }, [flow, sel]);

  const putFlow = (next: Flow) => onChange({ version: 2, id: formula?.id ?? "", source: "flow", flow: next });
  const setFlow = (next: Flow) => {
    // 단계 편집은 정상 원문만 갱신한다. 미완성 원문을 덮어써서 잃지 않는다.
    if (!codeDraft?.problem) onCodeDraftChange(null);
    putFlow(next);
  };
  const focusRow = (k: number) => requestAnimationFrame(() => rows.current[k]?.focus());
  const moveStep = (k: number, delta: number) => {
    if (!flow || k < 1) return;
    const to = k + delta;
    if (to < 1 || to > flow.steps.length) return;
    const steps = [...flow.steps];
    steps.splice(to - 1, 0, ...steps.splice(k - 1, 1));
    setFlow({ ...flow, steps }); setSel(to); focusRow(to);
  };
  const removeStep = (k: number) => {
    if (!flow || k < 1) return;
    setFlow({ ...flow, steps: flow.steps.filter((_, i) => i !== k - 1) });
    const next = Math.min(k, flow.steps.length - 1); setSel(next); focusRow(next);
  };
  const addStep = (op: Op) => {
    if (!flow || !checked || !env) return;
    const at = Math.min(sel, flow.steps.length);
    const step: Step = defaultStep(op, checked.fields[at] ?? [], rowColumns(env));
    const steps = [...flow.steps];
    steps.splice(at, 0, step);
    setFlow({ ...flow, steps }); setPicking(false); setSel(at + 1); focusRow(at + 1);
  };

  const tabs: [Tab, string][] = [["steps", "단계"], ["code", "흐름식"], ["sql", "SQL"]];
  return <div className="flow-editor">
    <div className="flow-tabs" role="tablist" aria-label="편집 방식">
      {tabs.map(([t, label]) => <button key={t} type="button" role="tab" id={`flow-tab-${t}`} aria-controls={`flow-panel-${t}`} aria-selected={tab === t}
        disabled={formula?.source === "sql" && t !== "sql"} onClick={() => setTab(t)}>{label}</button>)}
      <span className="flow-tabs-status" role="status">{tracing ? "미리보기 자료 계산 중…" : ""}</span>
    </div>
    {formula?.source === "sql" && <p className="column-context flow-sql-note">SQL로 직접 쓰는 열입니다. 단계나 흐름식으로는 되돌릴 수 없습니다.</p>}
    {tab !== "code" && codeDraft?.problem && <p className="flow-error-line" role="alert">흐름식에 저장하지 않은 문구가 있습니다. 흐름식 탭에서 고치세요. {codeDraft.problem}</p>}
    {traceError && <p className="flow-error-line">미리보기 자료 값을 계산하지 못했습니다. {traceError}</p>}

    {tab === "steps" && <div className="flow-panel" role="tabpanel" id="flow-panel-steps" aria-labelledby="flow-tab-steps">
      {!flow ? (readOnly ? <p className="column-hint">식이 없습니다.</p> : env ? <QuickStart env={env} links={links} onFlow={(f, name) => { setFlow(f); setSel(f.steps.length); if (name) onSuggestName(name); }} /> : <p role="status">시작 데이터를 불러오는 중…</p>)
        : <>
          {!readOnly && <div className="trace-add">
            <button ref={addButton} type="button" className="action" aria-label="단계 추가" aria-describedby="trace-add-position" aria-haspopup="dialog" aria-expanded={picking}
              disabled={!checked?.shapes[Math.min(sel, flow.steps.length)]} onClick={() => setPicking(p => !p)}>
              <Icon name="plus" />단계 추가</button>
            <span id="trace-add-position" className="column-hint" role="status">추가 위치: {sel === 0 ? "시작 데이터" : `${sel}단계`} 뒤</span>
            {picking && checked?.shapes[Math.min(sel, flow.steps.length)] && <VerbPicker shape={checked.shapes[Math.min(sel, flow.steps.length)]!}
              onPick={addStep} onClose={() => { setPicking(false); addButton.current?.focus(); }} />}
          </div>}
          <label className="trace-sample-picker">미리보기 자료<select value={sample} onChange={e => setSample(Number(e.target.value))}>
            {(trace?.samples ?? []).map((s, i) => <option key={i} value={i}>미리보기 자료 {sampleName(i)} · {s.title || s.base}</option>)}</select></label>
          <div className="trace-scroll">
            <table className="trace" role="grid" aria-label="값 흐름" aria-rowcount={flow.steps.length + 2}>
              <thead><tr>
                <th scope="col" className="trace-step-head">단계</th>
                {(trace?.samples ?? []).map((s, i) => <th key={i} scope="col" data-sample={i} className={i === sample ? "is-picked" : undefined}>
                  <span className="trace-sample-name">미리보기 자료 {sampleName(i)}</span>
                  <small title={s.title || s.base}>{s.title || s.base}</small><small className="trace-sample-count">{s.rows ?? context?.sources[i]?.totalRows ?? 0}품목</small>
                </th>)}
              </tr></thead>
              <tbody onKeyDown={e => {
                if (e.target instanceof HTMLElement && e.target.tagName !== "TR") return;
                const n = flow.steps.length;
                if (e.altKey && (e.key === "ArrowUp" || e.key === "ArrowDown")) { e.preventDefault(); if (!readOnly) moveStep(sel, e.key === "ArrowUp" ? -1 : 1); return; }
                if (e.key === "ArrowUp" || e.key === "ArrowDown") { e.preventDefault(); const k = Math.max(0, Math.min(n, sel + (e.key === "ArrowUp" ? -1 : 1))); setSel(k); focusRow(k); return; }
                if (e.key === "Delete" && !readOnly && sel > 0) { e.preventDefault(); removeStep(sel); return; }
                if (e.key === "Enter") { e.preventDefault(); settings.current?.querySelector<HTMLElement>("input, select, button:not(:disabled)")?.focus(); }
              }}>
                {[null, ...flow.steps].map((step, k) => {
                  const issue = checked?.errors.find(x => x.index === k)?.message ?? trace?.steps[k]?.error;
                  const firstBad = checked?.errors.find(x => !x.final)?.index;
                  const blocked = firstBad !== undefined && k > firstBad;
                  const shape = checked?.shapes[k] ?? null;
                  const word = step ? verbWord(step) : printSource(flow.from, mode);
                  const args = step ? printArgs(step, mode) : "";
                  const cells = trace && trace.steps.length === flow.steps.length + 1 ? trace.steps[k] : null;
                  return <tr key={k} ref={el => { rows.current[k] = el; }} tabIndex={k === sel ? 0 : -1} aria-selected={k === sel}
                    className={[k === sel ? "is-selected" : "", issue ? "has-error" : "", blocked ? "is-blocked" : ""].filter(Boolean).join(" ") || undefined}
                    onClick={() => setSel(k)} onFocus={() => setSel(k)}>
                    <th scope="row">
                      <div className="trace-step"><span className="code trace-verb">{word}</span>{args && <span className="code trace-args">{args}</span>}</div>
                      <div className="trace-meta">{k === 0 ? "시작 데이터" : `${k}단계`}{shape ? ` · ${shapeLabel[shape]}` : ""}</div>
                      {issue && !blocked && <p className="trace-error"><Icon name="alert" size={14} />{issue}</p>}
                    </th>
                    {(trace?.samples ?? []).map((_, i) => <td key={i} data-sample={i} className={i === sample ? "is-picked" : undefined}>
                      {blocked ? <span className="trace-blocked">앞 단계를 고치면 계산합니다</span>
                        : <TraceCell v={cells?.values[i]} shape={shape} op={step?.op} />}
                    </td>)}
                  </tr>;
                })}
              </tbody>
              {trace && <tfoot><tr className="trace-final">
                <th scope="row">열에 찍힐 값</th>
                {trace.final.map((v, i) => <td key={i} data-sample={i} className={i === sample ? "is-picked" : undefined}>{checked && !checked.ok
                  ? <span className="trace-blocked">식을 고치면 계산합니다</span> : v || <span className="blank-mark">빈 값</span>}</td>)}
              </tr></tfoot>}
            </table>
          </div>
          <p className="column-hint trace-keys">↑↓ 단계 고르기 · Alt+↑↓ 옮기기 · Delete 지우기 · Enter 설정</p>
          {env && <StepSettings index={Math.min(sel, flow.steps.length)} flow={flow} env={env} panelRef={settings}
            fields={checked?.fields[Math.min(sel, flow.steps.length) - 1] ?? []} input={sel > 0 ? checked?.shapes[sel - 1] ?? null : null}
            sampleRows={sel > 0 ? rowsOf(trace?.steps[sel - 1]?.values[sample]) : undefined} sampleTitle={trace?.samples[sample] ? sampleName(sample) : undefined}
            readOnly={readOnly} entityLinks={links} onChange={setFlow}
            onMove={d => moveStep(sel, d)} onRemove={() => removeStep(sel)} />}
        </>}
    </div>}

    {tab === "code" && <div className="flow-panel" role="tabpanel" id="flow-panel-code" aria-labelledby="flow-tab-code">
      {env && <FlowCode flow={flow} env={env} readOnly={readOnly} onFlow={putFlow} draft={codeDraft} onDraftChange={onCodeDraftChange} />}
    </div>}

    {tab === "sql" && <div className="flow-panel" role="tabpanel" id="flow-panel-sql" aria-labelledby="flow-tab-sql">
      {formula?.source === "sql" ? <SqlEdit entity={entity} formula={formula} context={context} readOnly={readOnly} onChange={onChange} userView={userView} sampleKeys={keys} />
        : <>
          <p className="column-hint">단계에서 자동 생성한 읽기 전용 SQL입니다. 단계나 흐름식을 바꾸면 이 쿼리도 갱신됩니다. 직접 편집으로 전환하면 단계·흐름식 편집을 더 이상 사용할 수 없습니다. {userView
            ? <>바깥 행의 문서는 <span className="code">접수</span>·<span className="code">공고</span>·<span className="code">계약</span>으로 부릅니다.</>
            : <>바깥 행은 <span className="code">현재</span>로 부릅니다.</>}</p>
          <Suspense fallback={loading}><SqlCodeEditor label="컴파일된 SQL" readOnly value={!flow ? "-- 식이 없습니다"
            : !checked?.ok ? "-- 식에 오류가 있어 SQL을 만들지 않았습니다" : ejected ? (ejected.error ? `-- ${ejected.error}` : ejected.sql) : "-- 계산 중…"} /></Suspense>
          {!readOnly && <div className="column-controls flow-sql-actions">
            <button type="button" className="action" disabled={!checked?.ok || !ejected?.sql || !!codeDraft?.problem} onClick={() => setEscaping(true)}>SQL로 꺼내 직접 편집…</button>
            {!checked?.ok && flow && <span className="column-hint">식의 오류를 먼저 고치세요.</span>}
          </div>}
        </>}
    </div>}

    {escaping && ejected?.sql && <Modal label="SQL로 꺼내기 확인" onClose={() => setEscaping(false)}>
      <h2>SQL로 꺼내 직접 편집할까요?</h2>
      <p>지금 식을 SQL 원문으로 바꿔 직접 고칠 수 있게 합니다. <strong>바꾼 뒤에는 단계나 흐름식으로 되돌릴 수 없습니다.</strong></p>
      <p className="column-hint">SQL은 공개 뷰(v_공고 등)만 읽을 수 있고, 저장할 때 미리보기 자료로 실제 실행해 검사합니다.</p>
      <div className="column-controls"><button className="action" onClick={() => setEscaping(false)}>취소</button>
        <button className="action primary" onClick={() => { onChange({ version: 2, id: formula?.id ?? "", source: "sql", sql: ejected.sql }); setEscaping(false); setTab("sql"); }}>SQL로 꺼내기</button></div>
    </Modal>}
  </div>;
}

const rowsOf = (v: TraceValue | undefined) => v?.kind === "list" ? v.rows : v?.kind === "value" && v.row ? [v.row] : undefined;

function TraceCell({ v, shape, op }: { v?: TraceValue; shape: Shape | null; op?: Op }) {
  if (!v) return <span className="trace-pending">—</span>;
  if (v.kind === "value") return v.blank || !v.text ? <span className="blank-mark">빈 값</span> : <span className="trace-text">{v.text}</span>;
  const unit = shape === "rows" ? "행" : "개";
  return <div className="trace-list">
    <span className="trace-count">{v.count}{unit}</span>
    {!!v.items.length && <span className="trace-chips">{v.items.map((x, i) => <span key={i} className="value-chip">{x || <span className="blank-mark">빈 값</span>}</span>)}
      {v.count > v.items.length && <span className="trace-more">외 {v.count - v.items.length}</span>}</span>}
    {!!v.dropped && <span className="trace-note">{v.dropped}{unit} {(op && dropWord[op]) ?? "빠짐"}</span>}
    {!!v.blank && <span className="trace-note is-warn"><Icon name="alert" size={14} />빈 값 {v.blank}</span>}
  </div>;
}

function QuickStart({ env, links, onFlow }: { env: FlowEnv; links: LinkKind[]; onFlow: (f: Flow, name?: string) => void }) {
  const [field, setField] = useState(env.base[0] ?? "");
  const [pickField, setPickField] = useState(false);
  // 맞춤 뷰에는 「이 행」 이 없다 — 문서 하나에서 시작한다. 품목 묶음은 필드 목록의 문서마다 있다.
  if (env.view) return <section className="quick-start" aria-label="빠른 시작">
    <h3>어느 문서에서 시작할까요?</h3>
    <div className="quick-sources">
      {links.map(k => <button key={k} type="button" className="action" onClick={() => onFlow({ from: { kind: "linked", link: k }, steps: [] })}>{docWord[k]}</button>)}
      <button type="button" className="action" onClick={() => onFlow({ from: { kind: "text", text: "" }, steps: [] })}>글자</button>
    </div>
    <p className="column-hint">문서를 고르면 그 문서 한 건에서 시작합니다. 다음 단계에서 꺼낼 열을 고르세요.</p>
  </section>;
  return <section className="quick-start" aria-label="빠른 시작">
    <h3>어디에서 시작할까요?</h3>
    <div className="quick-sources">
      <button type="button" className="action" onClick={() => onFlow({ from: { kind: "items" }, steps: [] })}>품목</button>
      <button type="button" className="action" aria-pressed={pickField} disabled={!env.base.length} onClick={() => setPickField(v => !v)}>이 행의 열</button>
      {links.map(k => <button key={k} type="button" className="action" onClick={() => onFlow({ from: { kind: "linked", link: k }, steps: [] })}>{wordOf(env, k)}</button>)}
    </div>
    {pickField && <div className="quick-field">
      <label>시작할 열<select value={field} onChange={e => setField(e.target.value)}>{env.base.map(f => <option key={f}>{f}</option>)}</select></label>
      <button type="button" className="action" disabled={!field} onClick={() => onFlow({ from: { kind: "field", field }, steps: [] })}>이 열에서 시작</button>
    </div>}
    <h3>자주 쓰는 묶음</h3>
    <p className="column-hint">고르면 단계가 채워집니다. 채워진 단계는 하나씩 고칠 수 있습니다.</p>
    <ul className="quick-presets">
      {presets(env).map(p => <li key={p.id}>
        <button type="button" className="action" disabled={!p.flow} title={p.unavailable} onClick={() => p.flow && onFlow(p.flow, p.name)}>{p.label}</button>
        {p.flow ? <span className="code quick-preview">{printFlow(p.flow).replace(/\n\s*/g, " ").split(" |> ").flatMap((part, i) => [...(i ? [" "] : []), <span key={i} className="step">{i ? "|> " : ""}{part}</span>])}</span>
          : <span className="quick-unavailable">{p.unavailable}</span>}
      </li>)}
    </ul>
  </section>;
}

function FlowCode({ flow, env, readOnly, onFlow, draft, onDraftChange }: { flow: Flow | null; env: FlowEnv; readOnly: boolean; onFlow: (f: Flow) => void;
  draft: FlowCodeDraft | null; onDraftChange: (draft: FlowCodeDraft) => void }) {
  const mode = { view: !!env.view };
  const text = draft?.text ?? (flow ? printFlow(flow, mode) : "");
  const problem = text.trim() ? draft?.problem ?? "" : "";
  const empty = !text.trim() && !!flow;
  return <>
    <p className="column-hint" id="flow-code-hint">고치는 대로 단계에 반영되고, 읽을 수 없는 곳에는 밑줄이 그어집니다. Ctrl+Space로 동사와 열을 완성합니다.</p>
    <Suspense fallback={loading}><FlowCodeEditor env={env} value={text} readOnly={readOnly} describedBy="flow-code-hint" placeholder={env.view ? "공고.품목 |> 꺼내기 세부품명 |> 잇기 \", \"" : "품목 |> 꺼내기 세부품명 |> 잇기 \", \""}
      onChange={value => {
        if (!value.trim()) { onDraftChange({ text: value, problem: flow ? "식이 비어 있습니다" : "" }); return; }
        try {
          const next = parseFlow(value, mode).flow;
          onDraftChange({ text: value, problem: "" }); onFlow(next);
        } catch (e) { onDraftChange({ text: value, problem: e instanceof FlowSyntaxError ? e.message : (e as Error).message }); }
      }} /></Suspense>
    <p className={problem || empty ? "flow-error-line" : "column-hint"} aria-live="polite">{problem ? `이 식을 읽을 수 없습니다: ${problem}. 단계에는 마지막으로 읽은 식이 남아 있습니다.`
      : empty ? "식이 비어 있습니다. 단계에는 마지막으로 읽은 식이 남아 있습니다." : flow ? `단계 ${flow.steps.length}개로 반영했습니다.` : ""}</p>
  </>;
}

/**
 * 꺼낸 SQL 편집(§6.4). 밑줄은 편집기 안의 빠른 모양 검사이고, 진짜 검증(authorizer·준비·미리보기 자료 실행)은 고칠 때마다
 * 다리의 validateComputedSql 이 한다 — 그 결과가 미리보기 자료 표와 오류 줄이다. 뷰가 도는지(순환)는 저장할 때 잡힌다.
 */
/** 맞춤 뷰 SQL 필드의 바깥 행 — 같은 조달 건의 세 문서(docs/user-views.md §3). */
const viewOuter: Record<string, string> = { 접수: "v_접수", 공고: "v_공고", 계약: "v_계약" };

/** 맞춤 뷰의 SQL 필드는 보표(traceUserViewColumn)가 검사한다 — 시작 데이터 한 줄의 오류가 거절, 끝 값이 미리보기 자료마다 찍힐 값이다. */
async function checkViewSql(formula: FormulaV2, keys: string): Promise<SqlCheck> {
  const trace = await call<ColumnTrace>("traceUserViewColumn", JSON.stringify(formula), keys ? JSON.parse(keys) as string[] : null);
  const error = trace.steps[0]?.error ?? null;
  return { ok: !error, error, samples: trace.samples, final: trace.final, ...(trace.note ? { note: trace.note } : {}) };
}

function SqlEdit({ entity, formula, context, readOnly, onChange, userView = false, sampleKeys = "" }: {
  entity?: EntityType; formula: Extract<FormulaV2, { source: "sql" }>; context: ColumnContext | null; readOnly: boolean; onChange: (f: FormulaV2) => void;
  userView?: boolean; sampleKeys?: string;
}) {
  const [issues, setIssues] = useState(0);
  const [check, setCheck] = useState<{ sql: string; result?: SqlCheck; error?: string } | null>(null);
  const view = context?.available?.view ?? "";
  const schema = useMemo(() => context?.available?.schema ?? {}, [context]);
  useEffect(() => {
    let active = true;
    const sql = formula.sql;
    const timer = setTimeout(() => {
      const checked = userView ? checkViewSql(formula, sampleKeys) : call<SqlCheck>("validateComputedSql", entity, sql);
      void checked.then(result => { if (active) setCheck({ sql, result }); })
        .catch((e: Error) => { if (active) setCheck({ sql, error: e.message }); });
    }, 300);
    return () => { active = false; clearTimeout(timer); };
  }, [entity, formula.sql, userView, sampleKeys]); // eslint-disable-line react-hooks/exhaustive-deps
  const current = check && check.sql === formula.sql ? check : null;
  const refused = current?.error ?? (current?.result && !current.result.ok ? current.result.error : null);
  return <>
    <p className="column-hint" id="sql-hint">값 하나를 돌려주는 SELECT 문을 씁니다. {userView
      ? "바깥 행의 문서는 접수·공고·계약으로 부르고, 공개 뷰만 읽을 수 있습니다."
      : <>바깥 행은 <span className="code">현재</span>({view})로 부르고, 공개 뷰만 읽을 수 있습니다.</>} Ctrl+Space로 뷰·열·함수를 완성합니다.</p>
    <Suspense fallback={loading}><SqlCodeEditor label="SQL" value={formula.sql} readOnly={readOnly} schema={schema} view={view} onIssues={setIssues} describedBy="sql-hint"
      outer={userView ? viewOuter : undefined} onChange={sql => onChange({ ...formula, sql })} /></Suspense>
    {issues > 0 && <p className="flow-error-line">고칠 곳이 {issues}군데 있습니다. 밑줄에 마우스를 올리면 이유가 보입니다.</p>}
    <p className={refused ? "flow-error-line" : "column-hint"} role="status">{!current ? "SQL을 검사하는 중…"
      : refused ? `저장할 수 없습니다: ${refused}` : "검사를 통과했습니다. 저장할 때 뷰에 넣어 한 번 더 확인합니다."}</p>
    {current?.result?.ok && <div className="sql-results">
      <table className="trace"><caption>미리보기 자료 결과</caption>
        <thead><tr><th scope="col">미리보기 자료</th><th scope="col">열에 찍힐 값</th></tr></thead>
        <tbody>{current.result.samples.map((s, i) => <tr key={i}><th scope="row">미리보기 자료 {sampleName(i)} <small>{s.title || s.base}</small></th>
          <td>{current.result!.final[i] || <span className="blank-mark">빈 값</span>}</td></tr>)}</tbody></table>
      {current.result.note && <p className="column-hint">{current.result.note}</p>}
    </div>}
  </>;
}

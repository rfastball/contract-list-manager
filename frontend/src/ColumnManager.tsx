import { useEffect, useMemo, useRef, useState } from "react";
import { call } from "./bridge";
import { Modal } from "./Modal";
import type { ColumnContext, ColumnFormula, EntityType, FreezePlan, UserColumn } from "./types";
import type { FlowEnv, FormulaV2 } from "./flow/ast";
import { shapeLabel } from "./flow/ast";
import { checkFlow } from "./flow/check";
import { toV2 } from "./flow/migrate";
import { printSource, verbWord } from "./flow/syntax";
import { ColumnFlow, type FlowCodeDraft } from "./columns/ColumnFlow";
import { Icon } from "./columns/icons";
import "../css/columns.css";

type Props = {
  columns: UserColumn[]; initialEntity: EntityType; initialField?: string;
  fromIntegrated?: boolean; readOnly?: boolean; onClose: () => void;
  navigationRequest?: { id: number; proceed: () => void };
  /** 저장하지 않은 편집이 있는지. 창을 닫을 때 앱이 물을 수 있게 바깥으로 올린다. */
  onDirtyChange?: (dirty: boolean) => void;
  onAddColumn: (entity: EntityType, name: string, kind: string, choices: string) => Promise<void>;
  onUpdateColumn: (entity: EntityType, name: string, newName: string, kind: string, choices: string) => Promise<void>;
  onRemoveColumn: (entity: EntityType, name: string) => Promise<void>;
  onMoveColumn: (entity: EntityType, name: string, delta: number) => Promise<void>;
  onSaveComputed: (entity: EntityType, name: string, formula: ColumnFormula, previousName?: string) => Promise<void>;
  /** 값으로 굳히기. expected 는 확인 창에 보인 칸 수 — 그 사이 자료가 바뀌었으면 다리가 쓰지 않는다. */
  onFreezeColumn: (entity: EntityType, name: string, expected: number) => Promise<void>;
};

type Mode = ":" | "=";
type InputKind = "text" | "choice" | "date" | "number";
type Draft = {
  entity: EntityType; name: string;
  /** 저장된 열의 이름. 새 열이면 없다. */
  previousName?: string;
  /** 저장된 열의 성질. 전환 확인이 이것과 견준다. */
  savedMode?: Mode;
  mode: Mode; kind: InputKind; choices: string[];
  formula: FormulaV2 | null;
};

const labels: Record<EntityType, string> = { request: "접수", notice: "공고", contract: "계약" };
const entities: EntityType[] = ["request", "notice", "contract"];
const kindLabels: Record<InputKind, string> = { text: "글", choice: "고르기", date: "날짜", number: "숫자" };
const titleColumns: Record<EntityType, string> = { contract: "계약건명", notice: "공고명", request: "요청명" };

function formulaOf(column: UserColumn): FormulaV2 | null {
  if (!column.formula) return null;
  try { return toV2(column.formula); } catch { return null; }
}

/** 목록 줄의 꼬리. 입력열은 꼴, 가공열은 원천과 마지막 동사. 깨진 식은 그것부터 말한다. */
function tail(column: UserColumn): string {
  if (column.kind !== "computed") return column.kind === "choice" ? `고르기 ${column.choices.length}` : kindLabels[column.kind];
  if (column.broken) return "깨진 식";
  const f = formulaOf(column);
  if (!f) return "식 없음";
  if (f.source === "sql") return "SQL";
  const last = f.flow.steps.at(-1);
  return last ? `…${verbWord(last)}` : printSource(f.flow.from);
}

function draftFor(entity: EntityType, column: UserColumn): Draft {
  const computed = column.kind === "computed";
  return {
    entity, name: column.fieldName, previousName: column.fieldName, savedMode: computed ? "=" : ":",
    mode: computed ? "=" : ":", kind: computed ? "text" : column.kind as InputKind, choices: computed ? [] : [...column.choices],
    // 값으로 굳힌 입력열은 식을 비활성으로 들고 있다 — 다시 = 로 바꾸면 그 식이 돌아온다.
    formula: formulaOf(column),
  };
}
const fresh = (entity: EntityType, mode: Mode, name: string): Draft => ({ entity, name, mode, kind: "text", choices: [], formula: null });

const envOf = (c: ColumnContext | null): FlowEnv | null => c?.available ? {
  base: c.available.base, items: c.available.items,
  linked: Object.fromEntries(c.available.linked.map(l => [l.kind, { fields: l.fields, items: l.items, ...(l.many ? { many: true } : {}) }])),
} : null;

/**
 * 열 관리 — 열은 이름 · 꼴 · (있으면) 식이다(§3). <code>이름 : 꼴</code> 은 사람이 채우고
 * <code>이름 = 식</code> 은 식이 채운다. 연산자 기호가 목록·정의줄·검색·표 머리글에서 같은 뜻으로 선다.
 * 열 정의는 하나씩 저장하고, 목록 탐색과 표 복귀는 같은 초안 보호를 거친다.
 */
export function ColumnManager(props: Props) {
  const { columns, initialEntity, initialField, fromIntegrated, readOnly = false, onClose, navigationRequest, onDirtyChange } = props;
  const initial = columns.find(c => c.entityType === initialEntity && c.fieldName === initialField);
  const [draft, setDraft] = useState<Draft | null>(() => initial ? draftFor(initialEntity, initial) : null);
  const [baseline, setBaseline] = useState(() => initial ? JSON.stringify(draftFor(initialEntity, initial)) : "");
  const [entity, setEntity] = useState<EntityType>(initialEntity);
  const [query, setQuery] = useState("");
  const [newName, setNewName] = useState("");
  const [context, setContext] = useState<ColumnContext | null>(null);
  const [contextEntity, setContextEntity] = useState<EntityType | null>(null);
  const [contextError, setContextError] = useState("");
  const [contextRetry, setContextRetry] = useState(0);
  const [sampleInput, setSampleInput] = useState("");
  const [choiceText, setChoiceText] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [pending, setPending] = useState<{ run: () => void } | null>(null);
  const [removing, setRemoving] = useState(false);
  const [converting, setConverting] = useState<Mode | null>(null);
  const [plan, setPlan] = useState<FreezePlan | null>(null);
  const [planError, setPlanError] = useState("");
  const [menu, setMenu] = useState(false);
  // 편집기를 연 차례 — 다른 열(또는 새 열)을 열면 바뀌어 가공열 편집기와 그 안의 흐름식 문구를 새로 세운다.
  // 저장으로 이름이 바뀌어도 같은 열이라 그대로 둔다.
  const [opened, setOpened] = useState(0);
  const [codeDraft, setCodeDraft] = useState<FlowCodeDraft | null>(null);
  const nameInput = useRef<HTMLInputElement>(null);
  const search = useRef<HTMLInputElement>(null);
  const menuRoot = useRef<HTMLDivElement>(null);
  const initialResolved = useRef(!initialField || !!initial);
  const requestSeen = useRef<number | undefined>(undefined);

  const codeProblem = codeDraft?.problem ?? "";
  const dirty = !!draft && (JSON.stringify(draft) !== baseline || !!codeProblem);
  useEffect(() => { onDirtyChange?.(dirty); }, [dirty, onDirtyChange]);
  const target = draft?.entity ?? entity;
  const data = contextEntity === target ? context : null;
  const env = useMemo(() => envOf(data), [data]);
  const selected = draft?.previousName ? columns.find(c => c.entityType === draft.entity && c.fieldName === draft.previousName) ?? null : null;
  const siblings = columns.filter(c => c.entityType === target);
  const position = siblings.findIndex(c => c.fieldName === draft?.previousName);
  const q = query.trim();
  const only: Mode | null = q.startsWith("=") ? "=" : q.startsWith(":") ? ":" : null;
  const needle = (only ? q.slice(1) : q).trim().toLocaleLowerCase();
  const visible = siblings.filter(c => (!only || (only === "=") === (c.kind === "computed")) && c.fieldName.toLocaleLowerCase().includes(needle));
  const flow = draft?.mode === "=" && draft.formula?.source === "flow" ? draft.formula.flow : null;
  const checked = useMemo(() => flow && env ? checkFlow(flow, env) : null, [flow, env]);
  const errorCount = draft?.mode === "=" ? checked?.errors.length ?? 0 : 0;

  function open(next: Draft | null) {
    initialResolved.current = true;
    setOpened(n => n + 1);
    setCodeDraft(null);
    setDraft(next); setBaseline(next ? JSON.stringify(next) : ""); setError(""); setRemoving(false); setNotice(""); setSampleInput(""); setChoiceText(""); setMenu(false);
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
  function create(mode: Mode, name: string) {
    if (readOnly) return;
    navigate(() => { open(fresh(entity, mode, name.trim())); setNewName(""); });
  }

  useEffect(() => { (draft ? nameInput.current : search.current)?.focus(); }, [draft?.previousName, !!draft]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    if (initialResolved.current || !initialField) return;
    const column = columns.find(c => c.entityType === initialEntity && c.fieldName === initialField);
    if (column) open(draftFor(initialEntity, column));
  }, [columns, initialEntity, initialField]);
  useEffect(() => {
    if (!navigationRequest || requestSeen.current === navigationRequest.id || busy) return;
    requestSeen.current = navigationRequest.id;
    navigate(navigationRequest.proceed);
  }, [navigationRequest, busy]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    let active = true;
    setContextError("");
    void call<ColumnContext>("columnContext", target).then(result => { if (active) { setContext(result); setContextEntity(target); } })
      .catch((e: Error) => { if (active) setContextError(e.message); });
    return () => { active = false; };
  }, [target, contextRetry, columns]);
  useEffect(() => {
    if (!menu) return;
    const away = (e: PointerEvent) => { if (!menuRoot.current?.contains(e.target as Node)) setMenu(false); };
    document.addEventListener("pointerdown", away);
    return () => document.removeEventListener("pointerdown", away);
  }, [menu]);

  async function save(): Promise<boolean> {
    if (!draft || busy || readOnly) return false;
    const name = draft.name.trim();
    if (!name) { setError("열 이름을 입력하세요."); return false; }
    if (/[:=]/.test(name)) { setError("열 이름에는 :와 =를 쓸 수 없습니다."); return false; }
    if (draft.mode === "=") {
      if (codeProblem) { setError(`흐름식을 읽을 수 없어 저장하지 않았습니다 — ${codeProblem}`); return false; }
      if (!draft.formula) { setError("어디에서 시작할지 고르거나 자주 쓰는 묶음을 고르세요."); return false; }
      if (checked && !checked.ok) { setError(`식에 오류가 있어 저장할 수 없습니다. ${checked.errors[0].index === 0 ? "시작 데이터" : `${checked.errors[0].index}단계`}: ${checked.errors[0].message}`); return false; }
    }
    setBusy(true); setError("");
    try {
      if (draft.mode === "=") await props.onSaveComputed(draft.entity, name, draft.formula!, draft.previousName);
      else if (draft.previousName) await props.onUpdateColumn(draft.entity, draft.previousName, name, draft.kind, draft.choices.join(", "));
      else await props.onAddColumn(draft.entity, name, draft.kind, draft.choices.join(", "));
      const next: Draft = { ...draft, name, previousName: name, savedMode: draft.mode };
      setDraft(next); setBaseline(JSON.stringify(next)); setCodeDraft(null); setQuery("");
      setNotice(`‘${name}’ 열을 저장했습니다.`); return true;
    } catch (e) { setError((e as Error).message); return false; }
    finally { setBusy(false); }
  }
  async function move(delta: number) {
    if (!selected || busy) return;
    setMenu(false); setBusy(true); setError("");
    try { await props.onMoveColumn(selected.entityType, selected.fieldName, delta); setNotice("열 순서를 바꿨습니다."); }
    catch (e) { setError((e as Error).message); }
    finally { setBusy(false); }
  }
  async function remove() {
    if (!selected || busy) return;
    setBusy(true); setError("");
    try { await props.onRemoveColumn(selected.entityType, selected.fieldName); open(null); setNotice(`‘${selected.fieldName}’ 열을 표에서 뺐습니다.`); }
    catch (e) { setError((e as Error).message); setRemoving(false); }
    finally { setBusy(false); }
  }
  /**
   * 연산자 칸을 눌러 성질을 바꾼다(§3.1). 저장된 열을 바꿀 때만 묻는다 — 칸 수는 다리가 먼저 센다.
   * <code>: → =</code> 는 저장할 때 바뀌고(입력값은 숨는다), <code>= → :</code> 는 확인하는 순간 값을 쓴다(굳히기).
   */
  function flip() {
    if (!draft || readOnly || busy) return;
    if (codeProblem) { setError(`흐름식을 먼저 고치세요. ${codeProblem}`); return; }
    const to: Mode = draft.mode === ":" ? "=" : ":";
    if (!draft.savedMode || !draft.previousName || to === draft.savedMode) { change({ mode: to }); return; }
    setConverting(to);
    countPlan(draft.entity, draft.previousName);
  }
  /** 바꿀 칸을 다리에게 센다. 세지 못했으면 확인 창의 「다시 확인」 이 다시 부른다. */
  function countPlan(entity: string, name: string) {
    setPlan(null); setPlanError("");
    void call<FreezePlan>("planFreezeColumn", entity, name).then(setPlan)
      .catch((e: Error) => setPlanError(e.message));
  }
  async function freeze() {
    if (!draft?.previousName || !plan || busy || readOnly) return;
    setBusy(true); setError("");
    try {
      await props.onFreezeColumn(draft.entity, draft.previousName, plan.cells);
      const next: Draft = { ...draft, mode: ":", savedMode: ":", kind: "text", choices: [] };
      setDraft(next); setBaseline(JSON.stringify(next)); setConverting(null);
      setNotice(`‘${draft.previousName}’ 열을 값으로 굳혔습니다(${plan.cells.toLocaleString("ko-KR")}칸).`);
    } catch (e) { setPlanError((e as Error).message); }
    finally { setBusy(false); }
  }
  function addChoices(text: string) {
    if (!draft) return;
    const add = text.split(/[,|]/).map(c => c.trim()).filter(c => c && !draft.choices.includes(c));
    if (add.length) change({ choices: [...draft.choices, ...add] });
    setChoiceText("");
  }

  const contextStatus = contextError ? <div role="alert" className="column-context-error">{contextError}<button className="action" onClick={() => setContextRetry(n => n + 1)}>자료 다시 불러오기</button></div>
    : !data ? <p role="status" className="column-hint">{labels[target]}의 열과 미리보기 자료를 불러오는 중…</p> : null;

  const resultShape = !draft || draft.mode === ":" ? ""
    : draft.formula?.source === "sql" ? "글" : checked?.final ? (checked.final === "value" ? "글" : shapeLabel[checked.final]) : "";
  const summary = !draft ? "" : draft.mode === ":" ? (draft.kind === "choice" ? `고르기(${draft.choices.join(", ")})` : kindLabels[draft.kind])
    : !draft.formula ? "가져올 데이터를 고르세요" : draft.formula.source === "sql" ? "SQL" : [printSource(draft.formula.flow.from), ...draft.formula.flow.steps.map(verbWord)].join(" |> ");

  const inputEditor = draft && draft.mode === ":" && <div className="input-editor">
    <fieldset className="column-form" disabled={busy || readOnly}>
      <legend className="sr-only">입력열 설정</legend>
      <div className="field-row"><span className="field-label" id="cm-kind">값 종류</span>
        <div className="seg" role="group" aria-labelledby="cm-kind">
          {(Object.keys(kindLabels) as InputKind[]).map(k => <button key={k} type="button" aria-pressed={draft.kind === k} onClick={() => { change({ kind: k }); setSampleInput(""); }}>{kindLabels[k]}</button>)}
        </div></div>
      {draft.kind === "choice" && <div className="field-row"><span className="field-label" id="cm-choices">후보</span>
        <div className="choice-box" aria-labelledby="cm-choices">
          {draft.choices.map(c => <span key={c} className="choice-chip">{c}<button type="button" aria-label={`${c} 빼기`} onClick={() => change({ choices: draft.choices.filter(x => x !== c) })}><Icon name="remove" size={14} /></button></span>)}
          <input aria-label="선택 항목" className="choice-input" value={choiceText} placeholder={draft.choices.length ? "후보 더하기" : "검토 전, 검토 중, 완료"}
            onChange={e => { if (/[,|]/.test(e.target.value)) addChoices(e.target.value); else setChoiceText(e.target.value); }}
            onKeyDown={e => {
              if (e.key === "Enter") { e.preventDefault(); addChoices(choiceText); }
              else if (e.key === "Backspace" && !choiceText && draft.choices.length) change({ choices: draft.choices.slice(0, -1) });
            }} onBlur={() => choiceText && addChoices(choiceText)} />
        </div></div>}
      {draft.kind === "choice" && <p className="column-hint">쉼표나 Enter로 후보를 나눕니다. 후보에서 빼도 이미 입력한 값은 남습니다.</p>}
      {(draft.kind === "date" || draft.kind === "number") && <p className="column-hint">{draft.kind === "date" ? "2026/09/20처럼 적습니다." : "164,872,340처럼 적습니다."} 적은 그대로 문서에 찍힙니다.</p>}
    </fieldset>
    <section className="column-result" aria-label="목록 미리보기">
      <h3>목록 미리보기</h3>
      {contextStatus}
      {data && <>
        <p className="column-hint">{draft.previousName ? "이미 입력한 값은 그대로 유지됩니다." : "새 열은 사용자 열 맨 뒤에 붙습니다. 저장한 뒤 목록의 칸에 직접 입력하세요."}</p>
        <div className="column-data-scroll"><table><thead><tr><th>{data.columns[0]}</th><th>{titleColumns[target]}</th>
          <th className="column-result-new">{draft.name || "새 열 이름"}</th></tr></thead>
          <tbody>{data.rows.slice(0, 3).map((r, i) => <tr key={i}><td>{r[data.columns[0]]}</td><td>{r[titleColumns[target]] || "—"}</td>
            <td className="column-result-new">{draft.previousName && draft.savedMode === ":" ? r[draft.previousName] || <span className="blank-mark">빈 값</span> : sampleInput || <span className="blank-mark">저장 후 입력</span>}</td></tr>)}</tbody></table></div>
        {!data.rows.length && <p className="column-hint">아직 수집된 자료가 없습니다. 열을 먼저 만들어 둘 수 있습니다.</p>}
        {!readOnly && <label className="sample-input">입력해 보기{draft.kind === "choice"
          ? <select aria-label="입력해 보기" value={sampleInput} onChange={e => setSampleInput(e.target.value)}><option value="">골라 보세요</option>{draft.choices.map(c => <option key={c}>{c}</option>)}</select>
          : <input aria-label="입력해 보기" className="text" value={sampleInput} placeholder="예: 홍길동" onChange={e => setSampleInput(e.target.value)} />}
          <small>미리보기에만 쓰고 저장하지 않습니다.</small></label>}
        <p className="column-hint">저장한 뒤 목록에서 칸을 두 번 누르거나 F2를 눌러 입력합니다. 여러 행에 한꺼번에 넣으려면 「같은 값 입력」을 쓰세요.</p>
      </>}
    </section>
  </div>;

  return <section className="column-manager" aria-label="열 관리">
    <header className="cm-head">
      <button className="action quiet cm-back" disabled={busy} onClick={() => navigate(onClose)}><Icon name="back" />목록으로 돌아가기</button>
      <h1 className="cm-title">열 관리</h1>
      <div className="seg cm-entities" role="group" aria-label="자료 종류">
        {entities.map(k => <button key={k} type="button" aria-pressed={entity === k} disabled={busy}
          onClick={() => { if (k !== entity || draft) navigate(() => { setEntity(k); open(null); }); }}>{labels[k]}</button>)}
      </div>
      <p className="column-notice" role="status">{busy ? "처리 중…" : notice}</p>
    </header>
    {fromIntegrated && <p className="column-context">통합에는 계약의 사용자 열만 표시됩니다. 공고·접수 열은 해당 목록에서 확인하세요.</p>}
    {readOnly && <p className="column-context">열람 중이라 열 정의를 바꿀 수 없습니다.</p>}

    <div className={`cm-body${draft ? " is-editing" : ""}`}>
      <nav className="cm-list" aria-label="열 목록">
        <label className="cm-search"><span className="sr-only">열 이름 검색</span>
          <input ref={search} className="text" type="search" aria-label="열 이름 검색" placeholder="열 찾기 (= 가공열, : 입력열)" value={query} onChange={e => setQuery(e.target.value)} /></label>
        <p className="cm-count">{labels[entity]} 사용자 열 <span>{visible.length}{visible.length !== siblings.length ? ` / ${siblings.length}` : ""}</span></p>
        <ul className="cm-rows">
          {visible.map(c => {
            const isComputed = c.kind === "computed";
            const current = selected === c;
            return <li key={c.fieldName}><button type="button" className="cm-row" disabled={busy} aria-current={current ? "true" : undefined}
              aria-label={`${c.fieldName} · ${isComputed ? "식" : "입력"} · ${tail(c)}`}
              onClick={() => { if (!current) navigate(() => open(draftFor(c.entityType, c))); }}>
              <span className={`cm-op${isComputed ? " is-formula" : ""}`} aria-hidden="true">{isComputed ? "=" : ":"}</span>
              <span className="cm-name">{c.fieldName}</span>
              <span className={`cm-tail code${c.broken ? " is-broken" : ""}`} title={c.broken ?? undefined}>{c.broken && <Icon name="alert" size={14} />}{tail(c)}</span>
            </button></li>;
          })}
        </ul>
        {!visible.length && <div className="column-empty">
          {q ? <><p>「{q}」에 맞는 열이 없습니다.</p><button className="action" onClick={() => setQuery("")}>검색 지우기</button></>
            : <p>{labels[entity]}에 사용자 열이 없습니다.{readOnly ? "" : " 아래에서 새 열을 만드세요."}</p>}
        </div>}
        {!readOnly && <div className="cm-new">
          <span className="cm-op" aria-hidden="true"><Icon name="plus" size={14} /></span>
          <input className="cm-new-input" aria-label="새 열 이름" placeholder="새 열 이름 뒤에 : 또는 = 입력" value={newName} maxLength={21} disabled={busy}
            onChange={e => {
              const v = e.target.value;
              const m = /^(.*?)\s*([:=：＝])$/.exec(v);
              if (m) create(m[2] === "=" || m[2] === "＝" ? "=" : ":", m[1]);
              else setNewName(v);
            }}
            onKeyDown={e => { if (e.key === "Enter" && newName.trim()) { e.preventDefault(); create(":", newName); } }} />
          <span className="cm-new-buttons">
            <button type="button" className="chip" aria-label="입력열 만들기" title="사람이 값을 채우는 열" disabled={busy} onClick={() => create(":", newName)}><span className="code">:</span> 입력</button>
            <button type="button" className="chip" aria-label="가공열 만들기" title="식이 값을 채우는 열" disabled={busy} onClick={() => create("=", newName)}><span className="code">=</span> 가공</button>
          </span>
        </div>}
      </nav>

      <div className="cm-main">
        {!draft ? <div className="cm-empty">
          <h2>열을 고르거나 새로 만드세요</h2>
          <dl className="cm-syntax">
            <div><dt className="code">담당자 : 글</dt><dd>사람이 값을 채우는 입력열</dd></div>
            <div><dt className="code">품명 모음 = 품목 |&gt; …</dt><dd>품목·이 행의 열·연결 문서에서 값을 계산하는 가공열</dd></div>
          </dl>
          {!readOnly && <div className="cm-empty-actions">
            <button type="button" className="action" disabled={busy} onClick={() => create(":", "")}><span className="code">:</span> 입력열 만들기</button>
            <button type="button" className="action" disabled={busy} onClick={() => create("=", "")}><span className="code">=</span> 가공열 만들기</button>
          </div>}
          <details className="column-existing"><summary><Icon name="chevron" />기본 열과 첫 문서의 값 보기</summary>
            <p className="column-hint">{labels[target]} 목록의 기본 열과 사용자 열입니다. 값은 목록 화면에서 고치세요.</p>
            {contextStatus}
            {data && <div className="column-data-scroll"><table><thead><tr><th>현재 열 이름</th><th>첫 문서의 값</th></tr></thead><tbody>{data.columns.map(c => <tr key={c}><th scope="row">{c}</th><td>{data.rows[0]?.[c] || "(빈 값)"}</td></tr>)}</tbody></table></div>}
          </details>
        </div> : <section className="cm-editor" aria-label="열 편집">
          <div className="cm-definition">
            <fieldset className="cm-definition-line" disabled={busy || readOnly}>
              <legend className="sr-only">정의</legend>
              <label className="cm-def-name-label">열 이름
                <input ref={nameInput} aria-label="열 이름" className="cm-def-name" maxLength={20} value={draft.name} placeholder="열 이름"
                  size={Math.max(6, Math.min(20, draft.name.length + 1))} onChange={e => change({ name: e.target.value })} />
              </label>
              <button type="button" className={`cm-def-op${draft.mode === "=" ? " is-formula" : ""}`} onClick={flip}
                aria-label={draft.mode === "=" ? "가공열. 입력열로 바꾸기" : "입력열. 가공열로 바꾸기"}
                title={draft.mode === "=" ? "식이 채우는 가공열입니다. 누르면 입력열로 바꿉니다." : "사람이 채우는 입력열입니다. 누르면 가공열로 바꿉니다."}>{draft.mode}</button>
              <span className={`cm-def-summary${draft.mode === "=" && !draft.formula ? " is-empty" : " code"}`} title={summary}>{summary}</span>
            </fieldset>
            <div className="cm-def-state">
              {draft.mode === "=" && resultShape && <span className="cm-def-shape" title="결과 값 종류">{resultShape}</span>}
              {errorCount > 0 && <span className="cm-state is-error"><Icon name="alert" size={14} />오류 {errorCount}개</span>}
              {selected?.broken && !dirty && <span className="cm-state is-error" title={selected.broken}><Icon name="alert" size={14} />깨진 식</span>}
              <span className={`cm-state${dirty ? " is-dirty" : ""}`}>{dirty ? "변경됨" : draft.previousName ? "저장됨" : "저장 전"}</span>
              {selected && !readOnly && <div className="cm-menu" ref={menuRoot}>
                <button type="button" className="icon-button" aria-label="열 메뉴" aria-haspopup="menu" aria-expanded={menu} disabled={busy} onClick={() => setMenu(m => !m)}><Icon name="more" /></button>
                {menu && <div className="cm-menu-list" role="menu" aria-label="열 메뉴" onKeyDown={e => { if (e.key === "Escape") { e.preventDefault(); setMenu(false); } }}>
                  <button type="button" role="menuitem" disabled={dirty || position <= 0} onClick={() => void move(-1)}><Icon name="up" />위로 옮기기</button>
                  <button type="button" role="menuitem" disabled={dirty || position < 0 || position === siblings.length - 1} onClick={() => void move(1)}><Icon name="down" />아래로 옮기기</button>
                  <button type="button" role="menuitem" className="danger" disabled={dirty} onClick={() => { setMenu(false); setRemoving(true); }}><Icon name="trash" />열 제거…</button>
                  {dirty && <p className="column-hint">저장하거나 버린 뒤에 옮기거나 지울 수 있습니다.</p>}
                </div>}
              </div>}
            </div>
          </div>
          {selected?.broken && selected.kind === "computed" && <p className="cm-broken" role="alert"><Icon name="alert" size={14} />
            <span>저장된 식을 계산할 수 없어 이 열은 모든 행이 빈 값으로 나가고 있습니다. 이유: {selected.broken} 식을 고쳐 저장하면 다시 계산합니다.</span></p>}
          {draft.previousName && draft.name.trim() !== draft.previousName && <p className="column-context">이 열을 이름으로 읽는 문서 양식은 새 이름으로 다시 연결해야 합니다.</p>}
          {draft.entity === "contract" && !draft.previousName && <p className="column-hint cm-note">계약 열은 통합 목록에도 표시됩니다.</p>}
          <div className="cm-scroll">
            {inputEditor}
            {draft.mode === "=" && <>
              {contextStatus}
              <ColumnFlow key={opened} entity={draft.entity} formula={draft.formula} codeDraft={codeDraft} onCodeDraftChange={setCodeDraft} context={data} env={env} readOnly={readOnly || busy}
                onChange={formula => change({ formula })} onSuggestName={name => { if (!draft.name.trim()) change({ name }); }} />
            </>}
          </div>
          <footer className="column-save">
            {error && <p className="warn" role="alert">{error}</p>}
            <div className="column-controls">
              <span className="column-hint">{readOnly ? "열람 중" : dirty ? "저장하면 목록에 반영됩니다." : draft.previousName ? "저장된 열입니다." : "저장 전에는 목록이 바뀌지 않습니다."}</span>
              <button className="action" disabled={busy} onClick={() => navigate(() => open(null))}>닫기</button>
              {!readOnly && <button className="action primary" disabled={busy || !!codeProblem || (!dirty && !!draft.previousName)} onClick={() => void save()}>저장</button>}
            </div>
          </footer>
        </section>}
      </div>
    </div>

    {pending && <Modal label="저장하지 않은 열 변경" onClose={() => { if (!busy) setPending(null); }}>
      <h2>저장하지 않은 변경이 있습니다</h2><p>{draft?.name.trim() ? `‘${draft.name.trim()}’ 열의 ` : ""}변경을 저장하고 이동할까요?</p>
      {codeProblem && <p className="column-hint">지금은 저장할 수 없습니다. 흐름식을 고치세요. {codeProblem}</p>}
      {error && <p className="warn" role="alert">{error}</p>}
      <div className="column-controls"><button className="action" disabled={busy} onClick={() => setPending(null)}>계속 편집</button>
        <button className="action" disabled={busy} onClick={() => { const next = pending.run; setPending(null); open(null); next(); }}>변경 버리기</button>
        <button className="action primary" disabled={busy || !!codeProblem} onClick={async () => { const next = pending.run; if (await save()) { setPending(null); next(); } }}>저장 후 이동</button></div>
    </Modal>}
    {removing && selected && <Modal label="열 제거 확인" onClose={() => { if (!busy) setRemoving(false); }}>
      <h2>‘{selected.fieldName}’ 열을 표에서 뺄까요?</h2>
      <p>{selected.kind === "computed" ? "원본 자료는 그대로 남습니다. 이 열에 연결한 문서 양식은 더 이상 값을 받지 못하고, 같은 이름으로 다시 만들어도 식별자가 달라 다시 연결해야 합니다." : "입력한 값은 지우지 않습니다. 같은 이름으로 다시 만들면 그 값이 그대로 돌아옵니다."}</p>
      {!!selected.viewReaders && <p className="cm-count-line">보기 <strong>{selected.viewReaders}곳</strong>이 이 열을 씁니다. 지우면 그 열은 식이 깨져 빈 값으로 나갑니다.</p>}
      <div className="column-controls"><button className="action" disabled={busy} onClick={() => setRemoving(false)}>취소</button><button className="action danger" disabled={busy} onClick={() => void remove()}>열 제거</button></div>
    </Modal>}
    {converting && draft && <Modal label="열 성질 바꾸기 확인" onClose={() => { if (!busy) setConverting(null); }}>
      {converting === "=" ? <>
        <h2>‘{draft.previousName}’ 열을 가공열(=)로 바꿀까요?</h2>
        <p>입력한 값은 <strong>지우지 않고 숨깁니다</strong>. 그동안은 식이 칸을 채우고, 나중에 값으로 굳히면 숨긴 값은 그때 계산된 값으로 바뀝니다.</p>
        {plan ? <p className="cm-count-line">숨길 입력값 <strong>{plan.hidden.toLocaleString("ko-KR")}칸</strong> · 저장할 때 적용됩니다.</p>
          : !planError && <p className="column-hint" role="status">입력값을 세는 중…</p>}
      </> : <>
        <h2>‘{draft.previousName}’ 열의 값을 굳혀 입력열(:)로 바꿀까요?</h2>
        <p>문서마다 최신 차수에서 계산된 값을 입력값으로 <strong>바로 씁니다</strong>. 식은 꺼 둔 채 남겨 두어 다시 켤 수 있습니다.</p>
        {plan ? <ul className="cm-count-list">
          <li>입력값을 쓸 문서: {plan.series.toLocaleString("ko-KR")}건 중 <strong>{plan.cells.toLocaleString("ko-KR")}건</strong> (계산값이 빈 문서는 비워 둡니다)</li>
          <li>덮어쓰거나 지울 숨은 입력값: <strong>{plan.hidden.toLocaleString("ko-KR")}칸</strong> (식에 가려 있던 예전 값)</li>
        </ul> : !planError && <p className="column-hint" role="status">칸을 세는 중…</p>}
        {dirty && <p className="column-hint">저장하지 않은 식 변경이 있습니다. 먼저 저장하거나 버려야 굳힐 수 있습니다.</p>}
      </>}
      {planError && <p className="warn" role="alert">{planError}</p>}
      <div className="column-controls"><button className="action" disabled={busy} onClick={() => setConverting(null)}>취소</button>
        {planError && !plan && draft.previousName &&
          <button className="action" disabled={busy} onClick={() => countPlan(draft.entity, draft.previousName!)}>다시 확인</button>}
        {converting === "="
          ? <button className="action primary" disabled={!plan} onClick={() => { change({ mode: "=" }); setConverting(null); }}>가공열로 바꾸기</button>
          : <button className="action primary" disabled={!plan || busy || dirty} onClick={() => void freeze()}>값으로 굳히기</button>}</div>
    </Modal>}
  </section>;
}

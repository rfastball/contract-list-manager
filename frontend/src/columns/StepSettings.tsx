import { useRef } from "react";
import type { Branch, Clause, Compare, Flow, FlowEnv, LinkKind, Shape, Source, Step } from "../flow/ast";
import { rowColumns, shapeLabel, wordOf } from "../flow/ast";
import { verbByOp, type Arg } from "../flow/verbs";
import { maxBranches } from "../flow/check";
import { unitWord, verbWord } from "../flow/syntax";
import type { Row } from "../types";
import { Icon } from "./icons";

const compares: Compare[] = ["=", "≠", ">", "<", "≥", "≤", "포함", "비어있음", "안비어있음"];

type Props = {
  index: number;
  flow: Flow;
  env: FlowEnv;
  /** 이 단계에 들어오는 행의 필드. */
  fields: string[];
  input: Shape | null;
  /** 서식 단계에서 눌러 끼울 미리보기 자료 품목(고른 미리보기 자료의 들어오는 행). */
  sampleRows?: Row[];
  sampleTitle?: string;
  readOnly: boolean;
  entityLinks: LinkKind[];
  onChange: (flow: Flow) => void;
  onMove: (delta: number) => void;
  onRemove: () => void;
  panelRef?: React.Ref<HTMLElement>;
};

/** 고른 단계의 설정. 시작 데이터(0)이면 시작 데이터를, 아니면 동사의 인자를 고친다. */
export function StepSettings(props: Props) {
  const { index, flow, readOnly } = props;
  const heading = index === 0 ? "시작 데이터" : verbWord(flow.steps[index - 1]);
  const verb = index ? verbByOp.get(flow.steps[index - 1].op) : null;
  return <section className="step-settings" aria-label="단계 설정" ref={props.panelRef} tabIndex={-1}>
    <header className="step-settings-head">
      <h3><span className={index ? "code" : undefined}>{heading}</span> 설정</h3>
      {verb && <p className="column-hint">{verb.desc}{props.input ? ` (들어오는 값: ${shapeLabel[props.input]})` : ""}</p>}
      {index > 0 && !readOnly && <div className="step-settings-tools">
        <button type="button" className="icon-button" aria-label="단계 위로 옮기기" title="위로 (Alt+↑)" disabled={index <= 1} onClick={() => props.onMove(-1)}><Icon name="up" /></button>
        <button type="button" className="icon-button" aria-label="단계 아래로 옮기기" title="아래로 (Alt+↓)" disabled={index >= flow.steps.length} onClick={() => props.onMove(1)}><Icon name="down" /></button>
        <button type="button" className="icon-button danger" aria-label="단계 지우기" title="지우기 (Delete)" onClick={props.onRemove}><Icon name="trash" /></button>
      </div>}
    </header>
    <fieldset className="step-settings-body" disabled={readOnly}>
      <legend className="sr-only">{heading} 설정</legend>
      {index === 0 ? <SourceSettings {...props} /> : <ArgSettings {...props} step={flow.steps[index - 1]} />}
    </fieldset>
  </section>;
}

function SourceSettings({ flow, env, entityLinks, onChange }: Props) {
  const from = flow.from;
  const set = (next: Source) => onChange({ ...flow, from: next });
  const kind = from.kind;
  return <div className="step-form">
    <div className="seg" role="group" aria-label="시작 데이터 종류">
      {(env.view ? [["linked", "문서"], ["text", "글자"], ["number", "수"]] as const
        : [["items", "품목"], ["field", "이 행의 열"], ["linked", "연결 문서"], ["text", "글자"], ["number", "수"]] as const).map(([k, label]) =>
        <button key={k} type="button" aria-pressed={kind === k} disabled={k === "linked" && !entityLinks.length}
          onClick={() => {
            if (k === kind) return;
            if (k === "items") set({ kind: "items" });
            if (k === "field") set({ kind: "field", field: env.base[0] ?? "" });
            if (k === "linked") set({ kind: "linked", link: entityLinks[0] });
            if (k === "text") set({ kind: "text", text: "" });
            if (k === "number") set({ kind: "number", value: 0 });
          }}>{label}</button>)}
    </div>
    {from.kind === "field" && <label>열<select value={from.field} onChange={e => set({ kind: "field", field: e.target.value })}>
      {!env.base.includes(from.field) && <option value={from.field}>{from.field || "고르세요"}</option>}
      {env.base.map(f => <option key={f}>{f}</option>)}</select></label>}
    {from.kind === "linked" && <>
      <label>{env.view ? "문서" : "연결 문서"}<select value={from.link} onChange={e => set({ kind: "linked", link: e.target.value as LinkKind })}>
        {entityLinks.map(k => <option key={k} value={k}>{wordOf(env, k)}</option>)}</select></label>
      <label>읽을 것<select value={from.items ? "\u0000items" : from.field ?? "\u0000row"} onChange={e => {
        const v = e.target.value;
        set(v === "\u0000items" ? { kind: "linked", link: from.link, items: true } : v === "\u0000row" ? { kind: "linked", link: from.link } : { kind: "linked", link: from.link, field: v });
      }}>
        <option value={"\u0000row"}>문서 자체(한 행)</option><option value={"\u0000items"}>그 문서의 품목</option>
        {(env.linked[from.link]?.fields ?? []).map(f => <option key={f} value={f}>{f}</option>)}
      </select></label>
      <p className="column-hint">{env.view ? "같은 조달 건의 그 문서(최신 차수)를 읽습니다. 그 문서가 없는 건이면 빈 값입니다."
        : "사람이 확정한 연결 문서의 최신 차수를 읽습니다. 연결이 없으면 빈 값이고, 그 문서의 사용자 열은 읽을 수 없습니다."}</p>
    </>}
    {from.kind === "text" && <label>글자<input className="text" value={from.text} onChange={e => set({ kind: "text", text: e.target.value })} /></label>}
    {from.kind === "number" && <label>수<input className="text" type="number" value={from.value} onChange={e => set({ kind: "number", value: Number(e.target.value) })} /></label>}
  </div>;
}

function ArgSettings(props: Props & { step: Step }) {
  const { step, flow, index, onChange, fields, env, input } = props;
  const verb = verbByOp.get(step.op)!;
  const format = useRef<HTMLInputElement>(null);
  const put = (patch: Partial<Step>) => {
    const steps = [...flow.steps];
    const next = { ...step, ...patch } as Step;
    for (const k of Object.keys(patch) as (keyof Step)[]) if (patch[k] === undefined) delete next[k];
    steps[index - 1] = next;
    onChange({ ...flow, steps });
  };
  const rowish = input === "rows" || input === "row";
  const insert = (field: string) => {
    const el = format.current, text = step.text ?? "";
    const at = el?.selectionStart ?? text.length, end = el?.selectionEnd ?? at;
    put({ text: text.slice(0, at) + `{${field}}` + text.slice(end) });
    requestAnimationFrame(() => { el?.focus(); el?.setSelectionRange(at + field.length + 2, at + field.length + 2); });
  };
  if (!verb.args.length) return <p className="column-hint">설정할 것이 없습니다.</p>;
  return <div className="step-form">
    {verb.args.map((a, i) => <ArgControl key={i} a={a} step={step} put={put} fields={fields} base={rowColumns(env)} baseLabel={env.view ? "문서의 열" : "이 행의 열"}
      rowish={rowish} formatRef={step.op === "format" ? format : undefined} />)}
    {step.op === "format" && <SamplePicker rows={props.sampleRows} fields={fields} title={props.sampleTitle} onPick={insert} />}
  </div>;
}

function SamplePicker({ rows, fields, title, onPick }: { rows?: Row[]; fields: string[]; title?: string; onPick: (f: string) => void }) {
  if (!fields.length) return null;
  return <div className="format-sample">
    <p className="column-hint">아래 표의 값이나 열 이름을 누르면 서식의 커서 자리에 <span className="code">{"{필드}"}</span>가 들어갑니다{title ? ` (미리보기 자료 ${title}의 품목)` : ""}.</p>
    <div className="format-sample-scroll"><table>
      <thead><tr>{fields.map(f => <th key={f}><button type="button" className="field-token" onClick={() => onPick(f)}>{f}</button></th>)}</tr></thead>
      <tbody>{(rows ?? []).slice(0, 5).map((r, i) => <tr key={i}>{fields.map(f => <td key={f}>
        <button type="button" className="field-value" aria-label={`${r[f] || "빈 값"} (${f}) — {${f}} 끼우기`} onClick={() => onPick(f)}>{r[f] || <span className="blank-mark">빈 값</span>}</button>
      </td>)}</tr>)}</tbody>
    </table></div>
  </div>;
}

type ControlProps = {
  a: Arg; step: Step; put: (p: Partial<Step>) => void; fields: string[];
  /** 단계가 고를 수 있는 행의 열 — 가공열은 이 행의 열, 맞춤 뷰는 「공고.공고명」 꼴의 문서의 열. */
  base: string[]; baseLabel: string;
  rowish: boolean; formatRef?: React.RefObject<HTMLInputElement | null>;
};

function FieldSelect({ value, options, onChange, label, optional }: { value?: string; options: string[]; onChange: (v: string | undefined) => void; label: string; optional?: boolean }) {
  return <label>{label}<select value={value ?? ""} onChange={e => onChange(e.target.value === "" && optional ? undefined : e.target.value)}>
    {(optional || !value) && <option value="">{optional ? "(없음)" : "고르세요"}</option>}
    {value && !options.includes(value) && <option value={value}>{value} (없는 열)</option>}
    {options.map(f => <option key={f} value={f}>{f}</option>)}
  </select></label>;
}

function ArgControl({ a, step, put, fields, base, baseLabel, rowish, formatRef }: ControlProps) {
  switch (a.k) {
    case "cond": return <CondEditor cond={step.cond ?? []} fields={rowish ? fields : []} outer={rowish ? [] : base} onChange={cond => put({ cond })} />;
    case "field":
      if (a.optional && !rowish) return null;
      return <FieldSelect label={a.label} value={step.field} options={a.scope === "row" ? fields : base} optional={a.optional} onChange={field => put({ field })} />;
    case "fields":
      if (!rowish) return <p className="column-hint">같은 값은 하나만 남깁니다.</p>;
      return <fieldset className="chip-field"><legend>{a.label} <span className="column-hint">고르지 않으면 모든 열</span></legend>
        <div className="chips">{fields.map(f => {
          const at = step.fields?.indexOf(f) ?? -1;
          return <button key={f} type="button" className="chip" aria-pressed={at >= 0}
            onClick={() => { const list = step.fields ?? []; const next = at >= 0 ? list.filter(x => x !== f) : [...list, f]; put({ fields: next.length ? next : undefined }); }}>
            {at >= 0 && <span className="chip-order">{at + 1}</span>}{f}</button>;
        })}</div></fieldset>;
    case "dir": return <div className="seg" role="group" aria-label="방향">
      <button type="button" aria-pressed={step.dir !== "desc"} onClick={() => put({ dir: undefined })}>오름</button>
      <button type="button" aria-pressed={step.dir === "desc"} onClick={() => put({ dir: "desc" })}>내림</button></div>;
    case "str": return <label>{a.label}<input ref={formatRef} className="text code" value={step[a.slot] ?? ""} onChange={e => put({ [a.slot]: e.target.value })} /></label>;
    case "int": return <label>{a.label}<input className="text" type="number" min={a.slot === "length" ? 0 : 1} value={step[a.slot] ?? ""}
      onChange={e => put({ [a.slot]: e.target.value === "" ? (a.optional ? undefined : 1) : Math.trunc(Number(e.target.value)) })} /></label>;
    case "unit": return <div className="seg" role="group" aria-label="단위">{(["day", "month", "year"] as const).map(u =>
      <button key={u} type="button" aria-pressed={(step.unit ?? "day") === u} onClick={() => put({ unit: u })}>{unitWord[u]}</button>)}</div>;
    case "pairs": return <PairsEditor step={step} put={put} />;
    case "branches": return <BranchesEditor step={step} put={put} outer={base} />;
    case "numOrField": {
      const isField = step.field !== undefined;
      return <div className="step-pair">
        <div className="seg" role="group" aria-label={a.label}>
          <button type="button" aria-pressed={!isField} onClick={() => put({ field: undefined, value: step.value ?? 1 })}>수</button>
          <button type="button" aria-pressed={isField} onClick={() => put({ value: undefined, field: base[0] ?? "" })}>{baseLabel}</button></div>
        {isField ? <FieldSelect label={a.label} value={step.field} options={base} onChange={field => put({ field: field ?? "" })} />
          : <label>{a.label}<input className="text" type="number" value={step.value ?? 0} onChange={e => put({ value: Number(e.target.value) })} /></label>}
      </div>;
    }
    case "strOrField": {
      const isField = step.field !== undefined;
      return <div className="step-pair">
        <div className="seg" role="group" aria-label={a.label}>
          <button type="button" aria-pressed={!isField} onClick={() => put({ field: undefined, text: step.text ?? "" })}>글</button>
          <button type="button" aria-pressed={isField} onClick={() => put({ text: undefined, field: base[0] ?? "" })}>{baseLabel}</button></div>
        {isField ? <FieldSelect label={a.label} value={step.field} options={base} onChange={field => put({ field: field ?? "" })} />
          : <label>{a.label}<input className="text code" value={step.text ?? ""} onChange={e => put({ text: e.target.value })} /></label>}
      </div>;
    }
    case "dayWord": return null;
  }
}

/**
 * 조건 마디들. 행을 견주면 마디마다 그 행의 필드를 고른다(<code>fields</code>). 값 하나를 견주면 「지금 값」(빈 필드)이 기본이고,
 * 「비면」 이 대신 읽는 바깥의 열(<code>outer</code>)을 골라 견줄 수도 있다.
 */
function CondEditor({ cond, fields, outer, onChange, label = "조건" }: { cond: Clause[]; fields: string[]; outer: string[]; onChange: (c: Clause[]) => void; label?: string }) {
  const set = (i: number, patch: Partial<Clause>) => onChange(cond.map((c, j) => {
    if (j !== i) return c;
    const next = { ...c, ...patch };
    if (next.cmp === "비어있음" || next.cmp === "안비어있음") delete next.value; else next.value ??= "";
    return next;
  }));
  return <fieldset className="cond"><legend>{label}</legend>
    {cond.map((c, i) => <div className="cond-row" key={i}>
      {i > 0 && <select aria-label="잇는 말" value={c.join ?? "and"} onChange={e => set(i, { join: e.target.value as Clause["join"] })}>
        <option value="and">그리고</option><option value="or">또는</option></select>}
      {fields.length ? <select aria-label="견줄 열" value={c.field} onChange={e => set(i, { field: e.target.value })}>
        {!fields.includes(c.field) && <option value={c.field}>{c.field || "열"}</option>}
        {fields.map(f => <option key={f}>{f}</option>)}</select>
        : <select aria-label="견줄 값" value={c.field} onChange={e => set(i, { field: e.target.value })}>
          <option value="">지금 값</option>
          {c.field && !outer.includes(c.field) && <option value={c.field}>{c.field} (없는 열)</option>}
          {outer.map(f => <option key={f} value={f}>{f}</option>)}</select>}
      <select aria-label="비교" value={c.cmp} onChange={e => set(i, { cmp: e.target.value as Compare })}>{compares.map(x => <option key={x}>{x}</option>)}</select>
      {c.cmp !== "비어있음" && c.cmp !== "안비어있음" && <input aria-label="비교할 값" className="text" value={String(c.value ?? "")} onChange={e => set(i, { value: e.target.value })} />}
      {cond.length > 1 && <button type="button" className="icon-button" aria-label={`조건 ${i + 1} 지우기`} onClick={() => onChange(cond.filter((_, j) => j !== i).map((x, j) => j === 0 ? { field: x.field, cmp: x.cmp, ...(x.value !== undefined ? { value: x.value } : {}) } : x))}><Icon name="remove" /></button>}
    </div>)}
    <button type="button" className="action quiet" onClick={() => onChange([...cond, { join: "and", field: fields[0] ?? "", cmp: "=", value: "" }])}><Icon name="plus" /> 조건 더하기</button>
  </fieldset>;
}

function PairsEditor({ step, put }: { step: Step; put: (p: Partial<Step>) => void }) {
  const pairs = step.pairs ?? [];
  const set = (i: number, k: 0 | 1, v: string) => put({ pairs: pairs.map((p, j) => j === i ? (k ? [p[0], v] : [v, p[1]]) as [string, string] : p) });
  return <fieldset className="cond"><legend>대응</legend>
    {pairs.map((p, i) => <div className="cond-row" key={i}>
      <input aria-label={`대응 ${i + 1} 원래 값`} className="text code" value={p[0]} onChange={e => set(i, 0, e.target.value)} />
      <Icon name="arrow" />
      <input aria-label={`대응 ${i + 1} 바꿀 값`} className="text code" value={p[1]} onChange={e => set(i, 1, e.target.value)} />
      {pairs.length > 1 && <button type="button" className="icon-button" aria-label={`대응 ${i + 1} 지우기`} onClick={() => put({ pairs: pairs.filter((_, j) => j !== i) })}><Icon name="remove" /></button>}
    </div>)}
    <button type="button" className="action quiet" onClick={() => put({ pairs: [...pairs, ["", ""]] })}><Icon name="plus" /> 대응 더하기</button>
    <label>그외<input className="text code" placeholder="비우면 원래 값 그대로" value={step.otherwise ?? ""} onChange={e => put({ otherwise: e.target.value === "" ? undefined : e.target.value })} /></label>
  </fieldset>;
}

/**
 * 「경우」 의 갈래들. 갈래마다 값 조건(지금 값 또는 바깥의 열)과 맞을 때의 글이고, 위에서부터 견주어 처음 맞는 것이 이긴다 —
 * 그래서 차례를 옮길 수 있다. 맞는 것이 없으면 그외(비우면 빈 값).
 */
function BranchesEditor({ step, put, outer }: { step: Step; put: (p: Partial<Step>) => void; outer: string[] }) {
  const branches = step.branches ?? [];
  const set = (next: Branch[]) => put({ branches: next });
  const patch = (i: number, p: Partial<Branch>) => set(branches.map((b, j) => j === i ? { ...b, ...p } : b));
  const move = (i: number, d: number) => { const next = [...branches]; [next[i], next[i + d]] = [next[i + d], next[i]]; set(next); };
  return <fieldset className="cond"><legend>경우 <span className="column-hint">위에서부터 견주어 처음 맞는 경우의 글을 씁니다</span></legend>
    {branches.map((b, i) => <div className="branch" role="group" aria-label={`경우 ${i + 1}`} key={i}>
      <CondEditor cond={b.cond} fields={[]} outer={outer} label={`경우 ${i + 1} 조건`} onChange={cond => patch(i, { cond })} />
      <div className="cond-row">
        <Icon name="arrow" />
        <input aria-label={`경우 ${i + 1} 글`} className="text code" value={b.then} onChange={e => patch(i, { then: e.target.value })} />
        <button type="button" className="icon-button" aria-label={`경우 ${i + 1} 위로`} disabled={i === 0} onClick={() => move(i, -1)}><Icon name="up" /></button>
        <button type="button" className="icon-button" aria-label={`경우 ${i + 1} 아래로`} disabled={i === branches.length - 1} onClick={() => move(i, 1)}><Icon name="down" /></button>
        {branches.length > 1 && <button type="button" className="icon-button" aria-label={`경우 ${i + 1} 지우기`} onClick={() => set(branches.filter((_, j) => j !== i))}><Icon name="remove" /></button>}
      </div>
    </div>)}
    <button type="button" className="action quiet" disabled={branches.length >= maxBranches}
      onClick={() => set([...branches, { cond: [{ field: "", cmp: "안비어있음" }], then: "" }])}><Icon name="plus" /> 경우 더하기</button>
    <label>그외<input className="text code" placeholder="비우면 빈 값" value={step.otherwise ?? ""} onChange={e => put({ otherwise: e.target.value === "" ? undefined : e.target.value })} /></label>
  </fieldset>;
}

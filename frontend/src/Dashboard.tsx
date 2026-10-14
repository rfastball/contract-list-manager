import { Fragment, useMemo, useRef, useState } from "react";
import type { Kind } from "./Nara";
import type { DashboardDelivery, DashboardDue, DashboardReport, DashboardRow, DashboardStage } from "./types";
import { useFindShortcut } from "./useFindShortcut";
import "../css/dashboard.css";

const money = (value: number | null) => value === null ? "금액 미확인" : `${value.toLocaleString("ko-KR")}원`;
const shownDate = (value: string) => value ? value.replace("T", " ").slice(0, 16) : "미확인";
const latestDate = (row: DashboardRow) => [row.requestDate, row.postedAt, ...row.contracts.map(contract => contract.date)]
  .filter(Boolean).sort((a, b) => b.replace(/\D/g, "").localeCompare(a.replace(/\D/g, "")))[0] ?? "";
/** 네 단계 어디에도 넣지 않은 건. 행의 「자료 확인 필요」 표시에 이유가 있다. */
const UNSTAGED = { label: "단계 미정" };
const PAGE_SIZE = 40;
type DeliveryFilter = "all" | "overdue" | "today" | "week" | "month" | "missing" | "range";
type DeliveryState = { filter: DeliveryFilter; query: string; includeTerminated: boolean; from: string; to: string; page: number; expanded: string | null };
export type DashboardState = { mode: "stages" | "deliveries"; stage: DashboardStage | "all" | "unstaged"; query: string; sort: string; page: number; delivery: DeliveryState };
export const initialDashboardState: DashboardState = { mode: "stages", stage: "all", query: "", sort: "recent", page: 0,
  delivery: { filter: "all", query: "", includeTerminated: false, from: "", to: "", page: 0, expanded: null } };

export function Dashboard({ report, state, onStateChange, onOpen, onRefresh, onSetup, onError }: {
  report: DashboardReport;
  state: DashboardState;
  onStateChange: (state: DashboardState) => void;
  onOpen: (kind: Kind, number: string) => void;
  onRefresh: () => Promise<void>;
  onSetup: () => void;
  onError: (message: string) => void;
}) {
  const { stage, query, sort, page } = state;
  const change = (patch: Partial<DashboardState>) => onStateChange({ ...state, page: 0, ...patch });
  const setStage = (stage: DashboardState["stage"]) => change({ stage });
  const [busy, setBusy] = useState(false);
  const search = useRef<HTMLInputElement>(null);
  useFindShortcut(search);
  const needle = query.trim().toLocaleLowerCase();
  const rows = useMemo(() => report.rows.filter(row => (stage === "all" || row.stage === (stage === "unstaged" ? null : stage))
    && (!needle || [row.title, row.agency, row.requestNumber, ...row.noticeNumbers, ...row.contracts.flatMap(contract => [contract.number, contract.title])].join(" ").toLocaleLowerCase().includes(needle)))
    .sort((a, b) => sort === "amount" ? (a.contractAmount === null ? (b.contractAmount === null ? 0 : 1) : b.contractAmount === null ? -1 : b.contractAmount - a.contractAmount)
      : sort === "title" ? a.title.localeCompare(b.title, "ko")
        : latestDate(b).replace(/\D/g, "").localeCompare(latestDate(a).replace(/\D/g, ""))), [report.rows, stage, needle, sort]);
  const lastPage = Math.max(0, Math.ceil(rows.length / PAGE_SIZE) - 1);
  const currentPage = Math.min(page, lastPage);
  const shown = rows.slice(currentPage * PAGE_SIZE, (currentPage + 1) * PAGE_SIZE);
  const activeStage = stage === "unstaged" ? UNSTAGED
    : report.stages.find(item => item.key === stage);
  const reset = () => change({ stage: "all", query: "" });
  const refresh = async () => {
    setBusy(true);
    try { await onRefresh(); } catch (error) { onError((error as Error).message); } finally { setBusy(false); }
  };
  return <div className="dashboard-brief">
    <section className="db-scope" aria-label="저장 자료 집계">
      <div className="db-scope-heading"><strong>조달 {report.totalCases.toLocaleString("ko-KR")}건</strong>
        <span><time dateTime={report.asOf}>{shownDate(report.asOf)}</time> 기준</span>
        <button className="action quiet" disabled={busy} onClick={() => void refresh()}>{busy ? "새로 읽는 중…" : "자료 새로고침"}</button>
      </div>
      <div className="db-facts" aria-label="전체 자료 수">
        <span>접수 <b>{report.documents.requests.toLocaleString("ko-KR")}</b></span>
        <span>공고 <b>{report.documents.notices.toLocaleString("ko-KR")}</b></span>
        <span>계약 <b>{report.documents.contracts.toLocaleString("ko-KR")}</b></span>
        <span>수집 계약금액(해지 포함) <b>{report.contractsWithAmount > 0 ? money(report.contractAmount) : "금액 미확인"}</b>
          <small>금액 있는 계약 {report.contractsWithAmount.toLocaleString("ko-KR")} / {report.documents.contracts.toLocaleString("ko-KR")}건</small></span>
      </div>
      <details className="db-count-note"><summary>집계 기준</summary><p>조달 흐름은 연결된 접수·공고·계약을 한 건으로 묶습니다. 여러 계약도 한 건에 포함하며, 각 문서는 최신 차수로 셉니다. 문서가 없다는 것은 DB 미수집 상태이며 실제 업무 상태와 다를 수 있습니다.</p>
        <p>저장된 전 차수: 접수 {report.documents.requestRevisions} · 공고 {report.documents.noticeRevisions} · 계약 {report.documents.contractRevisions}. 계약금액은 확인된 값만 더하며, 0원과 미확인을 구분합니다.</p>
      </details>
    </section>
    <div className="db-mode" role="group" aria-label="브리핑 자료 선택"><button className="chip" aria-pressed={state.mode === "stages"} onClick={() => onStateChange({ ...state, mode: "stages" })}>진행 단계</button><button className="chip" aria-pressed={state.mode === "deliveries"} onClick={() => onStateChange({ ...state, mode: "deliveries" })}>납품기한</button></div>
    {state.mode === "deliveries" ? <DeliveryLookup deliveries={report.deliveries} state={state.delivery} onChange={delivery => onStateChange({ ...state, delivery })} onOpen={onOpen} /> : <>
    <section className="db-distribution" aria-labelledby="db-stage-title">
      <div className="db-section-heading"><h2 id="db-stage-title">단계별 현황</h2></div>
      <div className="db-stage-bar" aria-hidden="true">{report.stages.filter(item => item.count > 0).map(item => <span key={item.key} className={`db-stage-${item.key}`} style={{ flexGrow: item.count }} title={`${item.label} ${item.count}건`} />)}
        {report.dataCheckCount > 0 && <span className="db-stage-null" style={{ flexGrow: report.dataCheckCount }} title={`${UNSTAGED.label} ${report.dataCheckCount}건`} />}</div>
      <div className="db-stage-filters" role="group" aria-label="조달 단계 필터">
        <button className="db-stage-filter" aria-pressed={stage === "all"} onClick={() => setStage("all")}><span>전체</span><b>{report.totalCases.toLocaleString("ko-KR")}</b></button>
        {report.stages.map(item => <button key={item.key} className={`db-stage-filter db-stage-${item.key}`} aria-pressed={stage === item.key} onClick={() => setStage(item.key)}>
          <span>{item.label}</span><b>{item.count.toLocaleString("ko-KR")}</b>
        </button>)}
        <button className="db-stage-filter db-stage-null" aria-pressed={stage === "unstaged"} onClick={() => setStage("unstaged")}><span>{UNSTAGED.label}</span><b>{report.dataCheckCount.toLocaleString("ko-KR")}</b></button>
      </div>
    </section>
    <section className="db-results" aria-labelledby="db-results-title">
      <div className="db-section-heading"><h2 id="db-results-title">{activeStage?.label ?? "전체 조달 자료"}</h2>
        <span role="status">{rows.length.toLocaleString("ko-KR")}건 / 전체 {report.totalCases.toLocaleString("ko-KR")}건</span>
        {(stage !== "all" || query) && <button className="quiet-link" onClick={reset}>조건 초기화</button>}
      </div>
      <div className="db-find">
        <input ref={search} type="search" className="grid-find" aria-label="조달 자료 검색" placeholder="건명·기관·문서 번호 검색 (Ctrl+F)" value={query} onChange={event => change({ query: event.target.value })} />
        <label className="db-sort">정렬 <select className="text" value={sort} onChange={event => change({ sort: event.target.value })}><option value="recent">최근 문서 날짜순</option><option value="title">건명순</option><option value="amount">계약금액순</option></select></label>
      </div>
      {report.totalCases === 0 ? <div className="empty-state"><h3>저장된 조달 자료가 없습니다</h3><p>나라장터에서 자료를 수집하거나 JSON 파일을 가져오세요.</p><button className="action" onClick={onSetup}>나라장터 열기</button></div>
        : rows.length === 0 ? <div className="empty-state"><h3>이 조건에 맞는 자료가 없습니다</h3><p>단계와 검색어를 함께 적용했습니다.</p><button className="action" onClick={reset}>조건 초기화</button></div>
          : <div className="db-table-scroll"><table className="db-table" aria-label="단계별 조달 자료"><thead><tr><th>단계</th><th>건명 · 기관</th><th>문서 열기</th><th>최근 문서 날짜</th><th>수집 계약금액<br />(해지 포함)</th></tr></thead>
            <tbody>{shown.map(row => <tr key={row.key} aria-label={row.title}>
              <td><span className={`db-stage-label db-stage-${row.stage}`}>{report.stages.find(item => item.key === row.stage)?.label ?? UNSTAGED.label}</span></td>
              <td className="db-title"><strong>{row.title || "건명 미확인"}</strong><small>{row.agency || "기관 미확인"}</small>{row.issues.length > 0 && <span className="db-row-issue" title={row.issues.join("\n")}>자료 확인 필요 {row.issues.length}</span>}</td>
              <td className="db-document-links">{row.requestNumber && <button className="quiet-link" onClick={() => onOpen("접수", row.requestNumber)}>접수 {row.requestNumber}</button>}
                {row.noticeNumbers.map(number => <button key={number} className="quiet-link" onClick={() => onOpen("공고", number)}>공고 {number}{number !== row.noticeNumber && row.noticeNumber ? " (이전 공고)" : ""}</button>)}
                {row.contracts.map(contract => <button key={contract.number} className="quiet-link" onClick={() => onOpen("계약", contract.number)}>계약 {contract.number}</button>)}</td>
              <td className="db-date">{shownDate(latestDate(row))}</td><td className="db-money">{money(row.contractAmount)}</td>
            </tr>)}</tbody></table></div>}
      {rows.length > PAGE_SIZE && <nav className="db-pagination" aria-label="조달 자료 페이지"><button className="action" disabled={currentPage === 0} onClick={() => onStateChange({ ...state, page: currentPage - 1 })}>이전</button><span>{currentPage + 1} / {lastPage + 1}페이지</span><button className="action" disabled={currentPage === lastPage} onClick={() => onStateChange({ ...state, page: currentPage + 1 })}>다음</button></nav>}
    </section></>}
  </div>;
}

const deliveryFilters: [DeliveryFilter, string][] = [["all", "전체"], ["overdue", "기한 경과"], ["today", "오늘"], ["week", "7일 이내"], ["month", "30일 이내"], ["missing", "기한 미확인"], ["range", "기간 지정"]];
const effectiveItemDue = (item: DashboardDelivery["items"][number], contract: DashboardDelivery) => !item.due.raw.trim() && contract.due.date ? contract.due : item.due;
const itemText = (item: DashboardDelivery["items"][number]) => [item.name, item.specification, item.agency].join(" ").toLocaleLowerCase();
const contractText = (contract: DashboardDelivery) => [contract.number, contract.title, contract.party, contract.agency].join(" ").toLocaleLowerCase();
const lookupPoints = (contract: DashboardDelivery, state: DeliveryState) => {
  const needle = state.query.trim().toLocaleLowerCase();
  const itemOnly = !!needle && !contractText(contract).includes(needle);
  return [...(itemOnly ? [] : [{ due: contract.due, source: "계약 기한" }]), ...contract.items.filter(item => !itemOnly || itemText(item).includes(needle))
    .map(item => ({ due: effectiveItemDue(item, contract), source: item.name || `품목 ${item.lineNo}` }))];
};
const missingDue = (contract: DashboardDelivery) => contract.items.length ? contract.items.some(item => effectiveItemDue(item, contract).date === null) : contract.due.date === null;
const matchesDue = (due: DashboardDue, state: DeliveryState) => due.date !== null && (state.filter === "overdue" ? due.daysRemaining !== null && due.daysRemaining < 0
  : state.filter === "today" ? due.daysRemaining === 0
    : state.filter === "week" || state.filter === "month" ? due.daysRemaining !== null && due.daysRemaining >= 0 && due.daysRemaining <= (state.filter === "week" ? 7 : 30)
      : state.filter === "range" ? !!state.from && !!state.to && due.date >= state.from && due.date <= state.to : true);
const deliveryMatches = (contract: DashboardDelivery, state: DeliveryState) => state.filter === "all" || (state.filter === "missing" ? (state.query.trim() && !contractText(contract).includes(state.query.trim().toLocaleLowerCase())
  ? lookupPoints(contract, state).some(point => point.due.date === null) : missingDue(contract)) : lookupPoints(contract, state).some(point => matchesDue(point.due, state)));
const matchedPoint = (contract: DashboardDelivery, state: DeliveryState) => lookupPoints(contract, state).filter(point => point.due.date !== null && (state.filter === "all" || state.filter === "missing" || matchesDue(point.due, state)))
  .sort((a, b) => a.due.date!.localeCompare(b.due.date!))[0];
const matchedDue = (contract: DashboardDelivery, state: DeliveryState) => matchedPoint(contract, state)?.due;
const dueTone = (due?: DashboardDue) => due?.daysRemaining !== null && due?.daysRemaining !== undefined ? due.daysRemaining < 0 ? "db-due-overdue" : due.daysRemaining <= 7 ? "db-due-soon" : "" : "";
const remaining = (due?: DashboardDue) => due?.daysRemaining === null || due?.daysRemaining === undefined ? "미확인" : due.daysRemaining < 0 ? `${Math.abs(due.daysRemaining)}일 지남` : due.daysRemaining === 0 ? "오늘" : `${due.daysRemaining}일 남음`;

function DeliveryLookup({ deliveries, state, onChange, onOpen }: { deliveries: DashboardDelivery[]; state: DeliveryState; onChange: (state: DeliveryState) => void; onOpen: (kind: Kind, number: string) => void }) {
  const change = (patch: Partial<DeliveryState>) => onChange({ ...state, page: 0, expanded: null, ...patch });
  const reset = () => change({ filter: "all", query: "", includeTerminated: false, from: "", to: "" });
  const search = useRef<HTMLInputElement>(null);
  useFindShortcut(search);
  const base = deliveries.filter(contract => state.includeTerminated || !contract.terminated);
  const needle = state.query.trim().toLocaleLowerCase();
  const rows = base.filter(contract => deliveryMatches(contract, state) && (!needle || [contract.number, contract.title, contract.party, contract.agency,
    ...contract.items.flatMap(item => [item.name, item.specification, item.agency])].join(" ").toLocaleLowerCase().includes(needle)))
    .sort((a, b) => (matchedDue(a, state)?.date ?? "9999").localeCompare(matchedDue(b, state)?.date ?? "9999") || a.number.localeCompare(b.number));
  const lastPage = Math.max(0, Math.ceil(rows.length / PAGE_SIZE) - 1), page = Math.min(state.page, lastPage);
  const shown = rows.slice(page * PAGE_SIZE, (page + 1) * PAGE_SIZE);
  const rangeMissing = state.filter === "range" && (!state.from || !state.to);
  const rangeInvalid = state.filter === "range" && state.from > state.to && !rangeMissing;
  return <section className="db-results db-deliveries" aria-labelledby="db-delivery-title">
    <div className="db-section-heading"><h2 id="db-delivery-title">납품기한 조회</h2><span role="status">{rows.length}건 / {state.includeTerminated ? "전체" : "해지 제외"} {base.length}건</span>
      {(state.filter !== "all" || state.query || state.includeTerminated) && <button className="quiet-link" onClick={reset}>기한 조건 초기화</button>}</div>
    <div className="db-stage-filters db-delivery-filters" role="group" aria-label="납품기한 필터">{deliveryFilters.map(([filter, label]) => <button key={filter} className="db-stage-filter" aria-pressed={state.filter === filter} onClick={() => change({ filter })}>
      <span>{label}</span>{filter !== "range" && <b>{base.filter(contract => deliveryMatches(contract, { ...state, filter, query: "" })).length}</b>}</button>)}</div>
    {state.filter === "range" && <div className="db-range"><label>시작일 <input className="text" type="date" value={state.from} onChange={event => change({ from: event.target.value })} /></label><span>~</span><label>종료일 <input className="text" type="date" value={state.to} onChange={event => change({ to: event.target.value })} /></label>{rangeInvalid && <span role="alert">종료일은 시작일 이후로 정하세요.</span>}</div>}
    <div className="db-find"><input ref={search} className="grid-find" type="search" aria-label="납품기한 자료 검색" placeholder="계약 건명·번호·거래처·기관·품목 검색 (Ctrl+F)" value={state.query} onChange={event => change({ query: event.target.value })} />
      <label className="db-issues-filter"><input type="checkbox" checked={state.includeTerminated} onChange={event => change({ includeTerminated: event.target.checked })} />해지 계약 포함</label><span className="db-order-note">해당 기한 빠른 순</span></div>
    <p className="db-stage-reason">{state.filter === "week" || state.filter === "month" ? "오늘부터 선택한 일수까지이며 지난 기한은 제외합니다. " : state.filter === "range" ? "시작일과 종료일을 포함합니다. " : ""}계약·품목 중 해당 기한이 있는 계약을 한 번씩 표시합니다. 품목으로만 검색되면 그 품목의 기한을 조회합니다. 미확인 품목이 있으면 기한 조회와 미확인 조회에 함께 나타날 수 있습니다.</p>
    {rows.length === 0 ? <div className="empty-state"><h3>{rangeMissing ? "조회할 기간을 선택하세요" : deliveries.length === 0 ? "저장된 계약 자료가 없습니다" : "이 조건에 맞는 납품기한 자료가 없습니다"}</h3><p>기한 조건이나 검색어를 바꿔 다시 찾으세요.</p><button className="action" onClick={reset}>기한 조건 초기화</button></div>
      : <div className="db-table-scroll"><table className="db-table db-delivery-table" aria-label="납품기한 계약 자료"><thead><tr><th>계약 · 거래처 · 기관</th><th>해당 납품기한</th><th>남은 기간</th><th>품목 · 확인 사항</th><th>문서 열기</th><th>품목별 기한</th></tr></thead>
        <tbody>{shown.map(contract => { const due = matchedDue(contract, state); return <Fragment key={contract.number}><tr aria-label={contract.title || contract.number}>
          <td className="db-title"><strong>{contract.title || "건명 미확인"}</strong><small>{contract.party || "거래처 미확인"} · {contract.agency || "기관 미확인"}</small>{(contract.terminated || contract.draft) && <span className="db-row-issue">{[contract.terminated ? "해지" : "", contract.draft ? "계약 초안" : ""].filter(Boolean).join(" · ")}</span>}</td>
          <td className={`db-date ${dueTone(due)}`}>{due?.date ?? "기한 미확인"}{due && <small className="db-due-source">{matchedPoint(contract, state)?.source}</small>}</td><td className={`db-date ${dueTone(due)}`}>{remaining(due)}</td>
          <td>{contract.items.length}개 품목{missingDue(contract) && <small className="db-row-issue">{lookupPoints(contract, { ...state, query: "" }).some(point => point.due.date !== null) ? "일부 기한 미확인" : "기한 미확인"}</small>}{contract.issues.length > 0 && <small className="db-row-issue">자료 확인 필요 {contract.issues.length}</small>}</td>
          <td className="db-document-links"><button className="quiet-link" onClick={() => onOpen("계약", contract.number)}>{contract.number}</button></td>
          <td><button className="action quiet db-detail-toggle" aria-expanded={state.expanded === contract.number} onClick={() => onChange({ ...state, expanded: state.expanded === contract.number ? null : contract.number })}>{state.expanded === contract.number ? "기한 접기" : "기한 펼치기"}</button></td>
        </tr>{state.expanded === contract.number && <tr className="db-detail"><td colSpan={6}><div><h3>{contract.title || contract.number} 납품기한</h3><p>계약일 {shownDate(contract.contractDate)} · 계약 납품기한 {contract.due.raw || "미등록"}{contract.due.date && ` (${contract.due.date})`}</p>
          {contract.issues.length > 0 && <ul className="db-issues">{contract.issues.map((issue, index) => <li key={index}>{issue}</li>)}</ul>}
          {contract.items.length > 0 ? <table className="db-item-table" aria-label={`${contract.number} 품목별 납품기한`}><thead><tr><th>품목</th><th>규격 · 수량</th><th>기관</th><th>기한 원문</th><th>조회 기한</th><th>남은 기간</th></tr></thead><tbody>{contract.items.map((item, index) => { const itemDue = effectiveItemDue(item, contract); const inherited = !item.due.raw.trim() && contract.due.date; return <tr key={`${item.lineNo}-${index}`}><td>{item.lineNo}. {item.name || "품명 미확인"}</td><td>{item.specification} · {item.quantity} {item.unit}</td><td>{item.agency || "미확인"}</td><td>{item.due.raw || "미등록"}</td><td className={dueTone(itemDue)}>{itemDue.date ?? "미확인"}{inherited && <small>계약 기한 적용</small>}</td><td className={dueTone(itemDue)}>{remaining(itemDue)}</td></tr>; })}</tbody></table> : <p className="db-stage-reason">수집된 품목이 없습니다. 계약 문서의 기한으로 조회합니다.</p>}
        </div></td></tr>}</Fragment>; })}</tbody></table></div>}
    {rows.length > PAGE_SIZE && <nav className="db-pagination" aria-label="납품기한 자료 페이지"><button className="action" disabled={page === 0} onClick={() => onChange({ ...state, page: page - 1, expanded: null })}>이전</button><span>{page + 1} / {lastPage + 1}페이지</span><button className="action" disabled={page === lastPage} onClick={() => onChange({ ...state, page: page + 1, expanded: null })}>다음</button></nav>}
  </section>;
}

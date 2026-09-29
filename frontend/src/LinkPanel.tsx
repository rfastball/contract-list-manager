import { useEffect, useMemo, useRef, useState } from "react";
import type { Candidate, LinkContract, LinkFacet, LinkWork, NoticeChoice } from "./types";

type Props = {
  initialKey?: string | null;
  work: LinkWork;
  onConfirm: (c: Candidate) => void;
  onReject: (c: Candidate) => void;
  onUnlink: (contractKey: string) => void;
  onCompare: (contractKey: string, noticeKey: string) => Promise<LinkFacet[]>;
  /**
   * 왼쪽에 서는 것을 화면에서 뭐라 부를지. 기본은 <b>계약</b>이다.
   *
   * <p>접수 ↔ 공고 도 같은 얼개라 이 화면을 그대로 쓴다. 안의 이름(<code>contracts</code>·
   * <code>contractKey</code>·<code>facets[].contract</code>)은 그대로 두고 <b>낱말만</b>
   * 바꾼다 — 부르는 쪽이 접수 응답을 이 꼴로 옮겨 담아 넘기므로, 여기서 갈래를 알 필요가 없다.</p>
   */
  leftLabel?: string;
};

/** 왼쪽 목록을 무엇으로 좁혀 볼 것인가. 기본은 아직 할 일 쪽이다. */
type Filter = "미연결" | "연결됨" | "전체";

const FILTERS: Filter[] = ["미연결", "연결됨", "전체"];

/** 한 번에 그리는 찾기 결과. 더 있으면 수만 적는다 — 스무 줄을 넘기면 눈으로 고르는 자리가 아니다. */
const 찾기결과 = 50;

/**
 * 공고와 계약을 맞대는 화면.
 *
 * <p>얼개는 회계의 <b>계정 대사</b> 화면에서 가져왔다 — 왼쪽에 아직 맞추지 못한 것을 죽 세우고,
 * 하나를 고르면 오른쪽에 짝 후보를 근거와 함께 편다. 30년 쓰인 얼개라 새로 고민할 것이 없다.</p>
 *
 * <p>후보를 <b>점수 한 줄로 요약하지 않고 항목마다 나란히 놓는다.</b> 무엇이 같고 무엇이 다른지
 * 눈으로 보아야 3초에 판단할 수 있고, 기계가 왜 자동으로 연결하지 못했는지도 그 표에서 드러난다.</p>
 *
 * <p><b>후보가 하나도 없는 계약도 왼쪽에 남는다.</b> 후보 목록만 그리면 모두 물리친 계약이
 * 화면에서 사라져, 공고와 연결되지 않은 채로 남은 줄도 모르게 된다.</p>
 *
 * <p><b>이어진 계약도 남는다.</b> 대사 도구들이 한결같이 쓰는 얼개다 — 맞춰진 것을 화면의 한
 * 구획으로 두고 끊기를 그쪽 동작으로 둔다. 잘못 이어진 것을 되돌리려면 그 줄이 서 있어야 한다.
 * 다만 기본은 <b>미연결</b>이라 손볼 것부터 눈에 든다.</p>
 *
 * <p>추천이 못 찾은 짝은 <b>직접 찾기</b>로 간다. 추천은 그대로 두고 밖으로 나가는 길만 낸 것이라,
 * 손수 고른 공고도 후보와 똑같이 견주기 표를 편 뒤에 잇는다.</p>
 */
export function LinkPanel({
  work, onConfirm, onReject, onUnlink, onCompare, leftLabel = "계약", initialKey = null,
}: Props) {
  const [picked, setPicked] = useState<string | null>(initialKey);
  const [filter, setFilter] = useState<Filter>(initialKey ? "전체" : "미연결");
  const [query, setQuery] = useState("");
  const panel = useRef<HTMLDivElement>(null);
  useEffect(() => { if (initialKey) panel.current?.focus(); }, [initialKey]);

  /** 「직접 찾기」에서 고른 공고와 그 견주기 표. 잇기 전에 눈으로 볼 재료다. */
  const [chosen, setChosen] = useState<{ key: string; title: string; facets: LinkFacet[] } | null>(null);

  /** 지금 이어진 짝의 견주기 표. 후보에서 뺀 짝이라 따로 물어 와야 한다. */
  const [linked, setLinked] = useState<LinkFacet[]>([]);

  const counts = useMemo(() => {
    const 연결됨 = work.contracts.filter((c) => c.noticeKey !== null).length;
    return { 미연결: work.contracts.length - 연결됨, 연결됨, 전체: work.contracts.length };
  }, [work.contracts]);

  const shown = useMemo(
    () => work.contracts.filter((c) =>
      filter === "전체" ? true : filter === "연결됨" ? c.noticeKey !== null : c.noticeKey === null),
    [work.contracts, filter],
  );

  const here = useMemo(
    () => initialKey !== null && initialKey === picked && !work.contracts.some((c) => c.key === picked)
      ? undefined : shown.find((c) => c.key === picked) ?? shown[0],
    [shown, picked, initialKey, work.contracts],
  );

  // 물리치거나 이어서 목록이 줄면 고른 자리가 사라질 수 있다. 거르개를 바꿀 때도 같다.
  useEffect(() => {
    if (here && here.key !== picked) setPicked(here.key);
  }, [here, picked]);

  // 다른 계약으로 옮기면 앞서 고른 공고와 치던 말은 뜻을 잃는다.
  useEffect(() => {
    setChosen(null);
    setQuery("");
  }, [here?.key]);

  // 이어진 짝은 후보 목록에 없으므로 견주기 표를 따로 받는다.
  useEffect(() => {
    let 살아있다 = true;
    if (!here?.noticeKey) { setLinked([]); return; }

    onCompare(here.key, here.noticeKey)
      .then((facets) => { if (살아있다) setLinked(facets); })
      .catch(() => { if (살아있다) setLinked([]); });

    return () => { 살아있다 = false; };
  }, [here?.key, here?.noticeKey, onCompare]);

  const candidates = useMemo(
    () => (here ? work.candidates.filter((c) => c.contractKey === here.key) : []),
    [work.candidates, here],
  );

  const found = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return [];

    return work.notices.filter(
      (n) => n.key.toLowerCase().includes(needle) || n.title.toLowerCase().includes(needle));
  }, [work.notices, query]);

  if (work.contracts.length === 0)
    return <div className="empty-state">아직 {leftLabel} 자료가 없습니다. 문서를 가져오세요.</div>;

  const move = (step: number) => {
    const at = shown.findIndex((c) => c.key === here?.key);
    const next = shown[Math.min(Math.max(at + step, 0), shown.length - 1)];
    if (next) setPicked(next.key);
  };

  /** 손수 고른 공고. 후보와 같은 표를 편 뒤에 잇게 한다. */
  const choose = async (notice: NoticeChoice) => {
    if (!here) return;

    setChosen({ key: notice.key, title: notice.title, facets: [] });
    try {
      const facets = await onCompare(here.key, notice.key);
      setChosen({ key: notice.key, title: notice.title, facets });
    } catch {
      setChosen(null);
    }
  };

  /** 손수 고른 짝을 후보와 같은 모양으로 넘긴다 — 잇는 길은 하나여야 한다. */
  const 손수잇기 = (contract: LinkContract, notice: { key: string; title: string; facets: LinkFacet[] }) =>
    onConfirm({
      contractKey: contract.key,
      contractTitle: contract.title,
      noticeKey: notice.key,
      noticeTitle: notice.title,
      confidence: 1,
      reason: "직접 선택",
      noticeContractCount: 0,
      titleMatched: false,
      blocker: null,
      facets: notice.facets,
    });

  return (
    <div
      className="reconcile"
      ref={panel}
      tabIndex={0}
      onKeyDown={(e) => {
        if (e.key === "ArrowDown") { e.preventDefault(); move(1); }
        else if (e.key === "ArrowUp") { e.preventDefault(); move(-1); }
      }}
    >
      <aside className="reconcile-list">
        <div className="reconcile-head" role="group" aria-label={`${leftLabel} 필터`}>
          {FILTERS.map((f) => (
            <button key={f} className="chip" aria-pressed={filter === f} onClick={() => setFilter(f)}>
              {f} {counts[f]}
            </button>
          ))}
        </div>

        {shown.length === 0 ? (
          <div className="empty-state">
            {filter === "미연결"
              ? `연결되지 않은 ${leftLabel} 자료가 없습니다.`
              : `연결된 ${leftLabel} 자료가 없습니다.`}
          </div>
        ) : (
          <ul role="listbox" aria-label={`${leftLabel} 목록`}>
            {/* 이어진 계약도 서므로 이름을 「연결되지 않은 계약」으로 둘 수 없다. 무엇으로 좁혔는지는
                바로 위 거르개가 aria-pressed 로 이미 알리니, 이름은 흔들리지 않는 쪽이 낫다. */}
            {shown.map((c) => (
              <li
                key={c.key}
                role="option"
                aria-selected={c.key === here?.key}
                onClick={() => setPicked(c.key)}
              >
                <div className="title">{c.title || "(건명 없음)"}</div>
                <div className="sub">
                  {c.key}
                  {c.noticeKey !== null && ` ← ${c.noticeKey}`}
                  {c.candidateCount > 0
                    ? ` · 후보 ${c.candidateCount}`
                    : c.noticeKey === null ? " · 후보 없음" : ""}
                </div>
              </li>
            ))}
          </ul>
        )}
      </aside>

      <section className="reconcile-detail">
        {here?.noticeKey && (
          <div className="card current">
            <div className="cand-head">
              <strong>연결된 공고</strong>
              <span>
                <strong>{here.noticeKey}</strong> {here.noticeTitle}
              </span>
              <span className="why">
                {here.decidedBy === "auto" ? "자동 연결" : "직접 확정"}
              </span>
              <button className="action" onClick={() => onUnlink(here.key)}>연결 해제</button>
            </div>

            <Facets facets={linked} leftLabel={leftLabel} />
          </div>
        )}

        {candidates.length === 0
          ? !here?.noticeKey && (
              <div className="empty-state">
                {initialKey !== null && initialKey === picked && !here ? `선택한 ${leftLabel} ${initialKey} 자료가 목록에 없습니다. 목록에서 다른 자료를 선택하세요.`
                  : `${leftLabel}에 연결할 공고 후보가 없습니다. 공고를 가져오거나 아래에서 직접 검색하세요.`}
              </div>
            )
          : candidates.map((c) => (
              <div className="card" key={c.noticeKey}>
                <div className="cand-head">
                  <span className={`score ${c.confidence >= 0.9 ? "high" : "mid"}`}>
                    {Math.round(c.confidence * 100)}%
                  </span>
                  <span>
                    <strong>{c.noticeKey}</strong> {c.noticeTitle}
                  </span>
                  <span className="why">
                    {c.titleMatched ? "건명 일치" : "약한 근거"}
                    {c.noticeContractCount > 0 && ` · 이미 ${c.noticeContractCount}건 연결됨`}
                  </span>
                  <button className="action primary" onClick={() => onConfirm(c)}>연결</button>
                  <button className="action" onClick={() => onReject(c)}>후보 제외</button>
                </div>

                {c.blocker && <p className="blocker">자동으로 연결하지 않았습니다. {c.blocker}</p>}

                <Facets facets={c.facets} leftLabel={leftLabel} />
              </div>
            ))}

        {here && (
          <section className="reconcile-search">
            <h3>직접 찾기</h3>

            <input
              className="grid-find"
              type="search"
              placeholder="공고번호·건명으로 찾기"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => { if (e.key === "Escape") setQuery(""); }}
            />

            {query.trim() && (found.length === 0 ? (
              <p className="hint">검색어와 일치하는 공고가 없습니다.</p>
            ) : (
              <>
                <ul role="listbox" aria-label="직접 찾은 공고">
                  {found.slice(0, 찾기결과).map((n) => (
                    <li
                      key={n.key}
                      role="option"
                      aria-selected={n.key === chosen?.key}
                      onClick={() => choose(n)}
                    >
                      <div className="title">{n.title || "(건명 없음)"}</div>
                      <div className="sub">
                        {n.key}
                        {n.postedAt && ` · ${n.postedAt}`}
                        {n.linked > 0 && ` · 이미 ${n.linked}건 연결됨`}
                      </div>
                    </li>
                  ))}
                </ul>

                {found.length > 찾기결과 && (
                  <p className="hint">검색 결과가 {found.length - 찾기결과}건 더 있습니다. 공고번호나 건명을 더 입력하세요.</p>
                )}
              </>
            ))}

            {chosen && (
              <div className="card">
                <div className="cand-head">
                  <span>
                    <strong>{chosen.key}</strong> {chosen.title}
                  </span>
                  <button className="action primary" onClick={() => 손수잇기(here, chosen)}>연결</button>
                </div>

                <Facets facets={chosen.facets} leftLabel={leftLabel} />
              </div>
            )}
          </section>
        )}
      </section>
    </div>
  );
}

/** 나란히 견주기. 같은 줄과 다른 줄을 색으로 갈라 3초에 판단하게 한다. */
function Facets({ facets, leftLabel }: { facets: LinkFacet[]; leftLabel: string }) {
  if (facets.length === 0) return null;

  return (
    <table className="facets">
      <thead>
        <tr>
          <th />
          <th>{leftLabel}</th>
          <th>공고</th>
        </tr>
      </thead>
      <tbody>
        {facets.map((f) => (
          <tr key={f.name} className={f.agrees === true ? "agree" : f.agrees === false ? "differ" : "unknown"}>
            <th>{f.name}</th>
            <td>{f.contract || "—"}</td>
            <td>{f.notice || "—"}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

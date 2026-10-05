import {
  useCallback, useEffect, useMemo, useRef, useState, type CSSProperties, type Ref,
} from "react";
import type {
  Outline as Tree, OutlineChain, OutlineContract, OutlineNoticeGroup, OutlineRequest,
} from "./types";
import { useFindShortcut } from "./useFindShortcut";

type Props = {
  tree: Tree;
  /** 자료가 하나도 없을 때 가는 자리 — 자료가 들어오는 나라장터 화면이다. */
  onSetup?: () => void;
  onOpen?: (kind: "접수" | "공고" | "계약", number: string, revisions: boolean) => void;
  onLink?: (kind: "접수" | "계약", key: string) => void;
};

/** 한 번에 펴 두는 조달요구번호. 여덟 개가 다 서면 접수 칸이 다른 두 칸을 밀어낸다. */
const 요구번호보임 = 3;

/**
 * PageUp/PageDown 이 한 번에 넘기는 사슬 수.
 *
 * <p>표의 열다섯 줄을 그대로 가져오면 안 된다 — 사슬 한 장은 세 줄짜리 칸에 카드 여백까지
 * 얹혀 표 한 줄의 서너 배 높이라, 한 화면에 드는 것이 대여섯 장이다. 열다섯으로 두면 한 번
 * 눌러 세 화면을 지나쳐 <b>어디로 갔는지 알 수 없다</b>. 한 번에 한 화면쯤으로 맞춘다.</p>
 */
const 한번에넘길사슬 = 5;

/** 커서가 설 수 있는 자리 하나. 한 사슬 안에서 <b>보이는 차례대로</b> 늘어선다. */
type 초점칸 = {
  /**
   * 이 칸의 id. <b>사슬 키가 아니라 그 칸의 본번호</b>로 짓는다 — 사슬 키는 있는 것의 본번호를
   * 이어 만든 이름이라(<code>Outline.cs</code> 의 <code>Key</code>) 공고 하나가 뒤늦게 들어오는
   * 것만으로 바뀐다. 그 이름으로 id 를 지으면 <code>aria-activedescendant</code> 가 가리키던
   * 자리가 다시 그린 뒤 없는 id 가 되어, 낭독기가 커서를 놓친다. 아직 안 들어온 칸에는
   * 본번호가 없으니 그때만 사슬 키를 빌린다.
   */
  id: string;
  /** 이 칸이 여닫는 접이. 없으면 Space 를 눌러도 아무 일도 하지 않는다. */
  fold: { id: string; fallback: boolean } | null;
};

/**
 * 한 사슬의 초점 칸을 <b>그리는 차례 그대로</b> 늘어놓는다 — 접수 · 공고 · 계약1 · 계약2 …
 *
 * <p>없는 칸도 자리를 지키므로 접수·공고는 언제나 한 칸씩이고, 계약만 건수를 따라 늘어난다.
 * 그리는 쪽과 이 차례가 어긋나면 커서가 눈에 보이는 것과 다른 칸을 짚는다.</p>
 */
function 초점칸들(chain: OutlineChain): 초점칸[] {
  const id = (kind: string, base: string | undefined) => `칸-${kind}-${base ?? chain.key}`;

  const 앞 = [
    {
      id: id("접수", chain.request?.key),
      // 「외 N건」이 실제로 설 때만 접이가 있다. 세 개짜리 접수에는 펼 것이 없다.
      fold: chain.request !== null && chain.request.requestNumbers.length > 요구번호보임
        ? { id: `${chain.request.key}#요구번호`, fallback: false }
        : null,
    },
    // 공고 칸에는 아직 접이가 없다 — 차수 스택이 서는 날 이 자리가 채워진다.
    { id: id("공고", chain.notice?.groupBase), fold: null },
  ];

  if (chain.contracts.length === 0) return [...앞, { id: id("계약", undefined), fold: null }];

  return [
    ...앞,
    ...chain.contracts.map((c) => ({
      id: id("계약", c.key),
      fold: c.items.length > 0 ? { id: c.key, fallback: false } : null,
    })),
  ];
}

/**
 * 쌓인 것을 <b>생긴 모양대로</b> 보여 준다 — 접수 → 공고 → 계약이 한 사슬로 나란히 선다.
 *
 * <p>왜 표가 아닌가. 표는 <b>줄 하나가 한 가지 것</b>일 때만 읽힌다. 그런데 이 자료는
 * 계약 하나에 명세가 여러 줄이라, 평평하게 펴면 위쪽 값이 아래 줄마다 되풀이된다. 품목 표에서
 * 계약번호가 몇 번씩 찍히던 것이 그 되풀이인데, 보는 사람에게는 <b>같은 자료가 겹쳐 들어간
 * 것처럼</b> 보인다. 겹친 것이 아니라 펼 수 없는 것을 편 것이라, 펴지 않고 그대로 낸다.</p>
 *
 * <p><b>없는 칸도 자리를 지킨다.</b> 조달의 기본 흐름은 접수 하나에 공고 하나, 공고 하나에
 * 계약 하나다. 그러니 빠진 것을 지워 버리면 <b>무엇이 아직 안 들어왔는지</b>가 화면에서
 * 사라진다. 그 자리에 <code>(아직 없음)</code> 을 세워 둔다.</p>
 *
 * <p><b>계약은 둘 이상일 때만 갈라진다.</b> 하나뿐인데 목록처럼 그리면, 흔한 1:1:1 이 여럿
 * 달린 것처럼 보인다.</p>
 *
 * <p><b>차수와 명세는 접는다.</b> 변경공고 세 번이 세 줄로 서면 건수가 부풀어 보인다.
 * 찾는 중에는 걸린 것을 펴 준다 — 접힌 채로 걸리면 못 찾은 것과 같다.</p>
 *
 * <p><b>손은 표와 한 벌이다.</b> 옆의 표 보기가 엑셀 키를 쥐고 있으므로, 같은 자료의 두 눈이
 * 서로 다른 손을 요구하면 탭을 옮길 때마다 다시 배워야 한다. 그릇 하나가 초점을 쥐고 커서는
 * DOM 초점이 아니라 <code>cur</code> 클래스로 선다 — 칸마다 tabindex 를 돌리면 Tab 한 번에
 * 한 칸씩이라 넷째 사슬에 닿으려고 앞의 모든 칸을 밟아야 한다. DOM 초점이 움직이지 않으므로
 * 낭독기에게는 <code>aria-activedescendant</code> 가 커서를 알린다. 그것이 없으면 커서가 선
 * 자리를 말할 길이 아예 없다.</p>
 *
 * <p>Enter는 선택한 자료의 기존 표를 열고 번호로 좁혀 준다.</p>
 */
export function Outline({ tree, onSetup, onOpen, onLink }: Props) {
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState<Record<string, boolean>>({});
  const [cursor, setCursor] = useState({ row: 0, col: 0 });

  const board = useRef<HTMLDivElement>(null);
  const search = useRef<HTMLInputElement>(null);
  const here = useRef<HTMLElement>(null);

  useFindShortcut(search);

  const needle = query.trim().toLowerCase();
  const searching = needle.length > 0;

  const found = useMemo(() => filter(tree, needle), [tree, needle]);
  const 칸표 = useMemo(() => found.chains.map(초점칸들), [found.chains]);

  const isOpen = (id: string, fallback: boolean) => open[id] ?? (searching || fallback);
  const toggle = (id: string, fallback: boolean) =>
    setOpen((o) => ({ ...o, [id]: !(o[id] ?? (searching || fallback)) }));

  // 걸러내면 사슬이 줄어 커서가 <b>없는 줄</b>을 가리킬 수 있다 — 표가 rows.length 를 보고
  // 커서를 안으로 당기는 것과 같은 자리다. 걸린 것이 하나도 없으면 커서도 없다.
  useEffect(() => {
    setCursor((c) => {
      const row = Math.min(c.row, Math.max(칸표.length - 1, 0));
      return { row, col: Math.min(c.col, 끝칸(칸표, row)) };
    });
  }, [칸표]);

  useEffect(() => {
    here.current?.scrollIntoView({ block: "nearest", inline: "nearest" });
  }, [cursor]);

  const 지금 = 칸표[cursor.row]?.[cursor.col];
  const chain = found.chains[cursor.row];
  const kind = cursor.col === 0 ? "접수" : cursor.col === 1 ? "공고" : "계약";
  const selected = cursor.col === 0 ? chain?.request : cursor.col === 1
    ? chain?.notice?.current : chain?.contracts[cursor.col - 2];

  const move = useCallback((dRow: number, dCol: number) => {
    setCursor((c) => {
      const row = Math.min(Math.max(c.row + dRow, 0), Math.max(칸표.length - 1, 0));
      // 줄을 옮길 때 칸 자리는 지키되 그 사슬의 칸 수를 넘으면 마지막 칸으로 물린다 —
      // 계약이 둘인 줄의 오른쪽 끝에 서 있다가 세 칸짜리 줄로 내려가는 자리다.
      return { row, col: Math.min(Math.max(c.col + dCol, 0), 끝칸(칸표, row)) };
    });
  }, [칸표]);

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.target !== e.currentTarget) return;
    if (found.chains.length === 0) return;
    switch (e.key) {
      case "ArrowUp": move(-1, 0); break;
      case "ArrowDown": move(1, 0); break;
      case "ArrowLeft": move(0, -1); break;
      case "ArrowRight": move(0, 1); break;
      case "PageUp": move(-한번에넘길사슬, 0); break;
      case "PageDown": move(한번에넘길사슬, 0); break;
      case "Home":
        if (e.ctrlKey) setCursor({ row: 0, col: 0 });
        else setCursor((c) => ({ ...c, col: 0 }));
        break;
      case "End": {
        const 끝줄 = Math.max(칸표.length - 1, 0);
        if (e.ctrlKey) setCursor({ row: 끝줄, col: 끝칸(칸표, 끝줄) });
        else setCursor((c) => ({ ...c, col: 끝칸(칸표, c.row) }));
        break;
      }
      case " ":
        // 접이가 없는 칸에서는 아무 일도 하지 않는다. 공고 칸이 그렇다.
        if (지금?.fold) toggle(지금.fold.id, 지금.fold.fallback);
        break;
      case "Enter":
        if (selected) onOpen?.(kind, selected.number, false);
        break;
      default:
        // 그 밖의 키는 흘려보낸다. Ctrl+F 는 창 전체에서 useFindShortcut 이 듣는다.
        return;
    }

    e.preventDefault();
  };

  return (
    <div className="outline-wrap">
      <div className="grid-toolbar">
        <input
          ref={search}
          className="grid-find"
          type="search"
          aria-label="조달 흐름 검색"
          placeholder="찾기 (Ctrl+F) — 요청명·공고명·계약건명·번호·상대자"
          value={query}
          onChange={(e) => { setQuery(e.target.value); setOpen({}); setCursor({ row: 0, col: 0 }); }}
          // 지우기만 하고 초점을 두고 오면 손이 찾기 칸에 갇힌다 — 화살표가 아무 데도 닿지
          // 않아 격자를 다시 눌러 짚어야 한다. 표의 찾기 칸과 같은 몸짓으로 돌려준다.
          onKeyDown={(e) => { if (e.key === "Escape") { setQuery(""); board.current?.focus(); } }}
        />
        <span className="result-count" role="status">{found.chains.length}건<span> / 전체 {tree.chains.length}건</span></span>
        <span className="spacer" />
        <button className="action" onClick={() => setOpen(Object.fromEntries(칸표.flatMap((row) => row.flatMap((cell) => cell.fold ? [[cell.fold.id, false]] : []))))}>모두 접기</button>
      </div>
      <div className="outline-actions">
        <span id="outline-help">방향키 이동 · Space 펼치기 · Enter 표에서 보기</span>
        {selected && <span className="number">선택: {selected.number}</span>}
        {onOpen && <button className="chip" disabled={!selected} onClick={() => selected && onOpen(kind, selected.number, false)}>표에서 보기</button>}
        {onOpen && <button className="chip" disabled={!selected || kind === "접수"} onClick={() => selected && onOpen(kind, selected.key, true)}>전체 차수 보기</button>}
        {onLink && <button className="chip" disabled={!selected || kind === "공고"} onClick={() => selected && kind !== "공고" && onLink(kind, selected.key)}>선택 건 연결 검토</button>}
      </div>

      {/* aria-colcount·aria-colindex 는 일부러 붙이지 않는다. 계약이 둘인 사슬은 칸이 셋이
          아니라 넷이라, 열 번호를 적으면 줄마다 다른 것을 뜻하는 번호가 된다 — 낭독기가
          「3 중 3」이라 읽어 준 것이 옆줄에서는 거짓이 되므로, 틀린 번호보다 없는 편이 낫다. */}
      <div
        className="outline"
        ref={board}
        role="grid"
        aria-label="구조 보기 — 접수 · 공고 · 계약"
        aria-describedby="outline-help"
        aria-activedescendant={지금?.id}
        tabIndex={0}
        onKeyDown={onKeyDown}
      >
        {found.chains.length === 0 ? (
          <div className="empty-state">
            <h2>{tree.chains.length === 0 ? "등록된 조달 자료가 없습니다" : "검색 결과가 없습니다"}</h2>
            <p>{tree.chains.length > 0
              ? "다른 검색어로 찾아보세요."
              : "왼쪽 메뉴의 나라장터에서 브라우저 확장을 설치하거나 JSON 파일을 가져오세요."}</p>
            {tree.chains.length === 0
              ? onSetup && <button className="action primary" onClick={onSetup}>나라장터 열기</button>
              : <button className="action" onClick={() => { setQuery(""); search.current?.focus(); }}>검색 지우기</button>}
          </div>
        ) : (
          found.chains.map((chain, ri) => (
            <ChainCard
              key={chain.key}
              chain={chain}
              cells={칸표[ri]}
              cur={ri === cursor.row ? cursor.col : -1}
              here={here}
              isOpen={isOpen}
              toggle={toggle}
              onSelect={(col) => setCursor({ row: ri, col })}
            />
          ))
        )}
      </div>
    </div>
  );
}

/** 그 사슬의 마지막 칸 자리. 사슬이 하나도 없어도 0 아래로 내려가지 않는다. */
function 끝칸(칸표: 초점칸[][], row: number): number {
  return Math.max((칸표[row]?.length ?? 1) - 1, 0);
}

// ── 사슬 한 장 ────────────────────────────────────────

type Folds = {
  isOpen: (id: string, fallback: boolean) => boolean;
  toggle: (id: string, fallback: boolean) => void;
};

/** 칸 하나가 격자에서 쓰는 것. 네 종류의 칸이 모두 같은 것을 붙이므로 한자리에서 만든다. */
type Chrome = { id: string; cur: boolean; cellRef?: Ref<HTMLElement> };

function 칸속성(chrome: Chrome, className: string) {
  return {
    id: chrome.id,
    role: "gridcell" as const,
    ref: chrome.cellRef,
    className: chrome.cur ? `${className} cur` : className,
  };
}

function ChainCard({
  chain, cells, cur, here, isOpen, toggle, onSelect,
}: { chain: OutlineChain; cells: 초점칸[]; cur: number; here: Ref<HTMLElement>; onSelect: (col: number) => void } & Folds) {
  const { request, notice, contracts } = chain;

  const chrome = (i: number): Chrome =>
    ({ id: cells[i].id, cur: cur === i, cellRef: cur === i ? here : undefined });

  return (
    <article className="outline-node" role="row" aria-label={이름(chain)} onClick={(e) => {
      const cell = (e.target as HTMLElement).closest('[role="gridcell"]');
      const col = cells.findIndex((c) => c.id === cell?.id);
      if (col >= 0) onSelect(col);
      if (!(e.target as HTMLElement).closest('button, summary')) e.currentTarget.closest<HTMLElement>('[role="grid"]')?.focus();
    }}>
      <RequestCell request={request} chrome={chrome(0)} isOpen={isOpen} toggle={toggle} />
      <Arrow />
      <NoticeCell group={notice} chrome={chrome(1)} />
      <Arrow />

      {/* 계약이 둘 이상일 때만 갈라진다. 하나면 다른 두 칸과 같은 무게로 나란히 선다.
          이 그릇은 자리잡기일 뿐이라 격자에서는 지운다 — 남겨 두면 gridcell 이 row 에 바로
          매달리지 않아, 낭독기가 계약 칸을 그 줄의 칸으로 세지 않는다. */}
      <div role="presentation" style={contracts.length > 1 ? 갈래 : 칸자리}>
        {contracts.length > 1 && <span style={갈래셈}>계약 {contracts.length}건</span>}

        {contracts.length === 0 ? (
          <EmptyCell kind="계약" chrome={chrome(2)} />
        ) : (
          contracts.map((c, i) => (
            <ContractCell
              key={c.key}
              contract={c}
              chrome={chrome(2 + i)}
              open={isOpen(c.key, false)}
              onToggle={() => toggle(c.key, false)}
            />
          ))
        )}
      </div>
    </article>
  );
}

function Arrow() {
  return <svg className="flow-arrow" viewBox="0 0 24 24" aria-hidden="true"><path d="M4 12h15m-6-6 6 6-6 6" /></svg>;
}

/**
 * 아직 들어오지 않은 칸. 자리가 사라지면 무엇이 빠졌는지 보이지 않는다.
 *
 * <p>흐리게 두지 않는다 — 대비까지 함께 떨어져 무엇이 비었는지 읽으려면 눈을 붙여야 했다.
 * 바닥을 비우고 테두리를 점선으로 바꾸는 일은 app.css 가 한다.</p>
 */
function EmptyCell({ kind, chrome }: { kind: string; chrome: Chrome }) {
  return (
    <section {...칸속성(chrome, "outline-cell empty")} style={칸}>
      <div style={줄}>
        <span className="kind">{kind}</span>
      </div>
      <div className="title" style={감싼제목}>(아직 없음)</div>
    </section>
  );
}

function RequestCell({
  request, chrome, isOpen, toggle,
}: { request: OutlineRequest | null; chrome: Chrome } & Folds) {
  if (!request) return <EmptyCell kind="접수" chrome={chrome} />;

  const id = `${request.key}#요구번호`;
  const 다펼침 = isOpen(id, false);
  const 남은 = request.requestNumbers.length - 요구번호보임;
  const 보일것 = 다펼침 ? request.requestNumbers : request.requestNumbers.slice(0, 요구번호보임);

  return (
    <section {...칸속성(chrome, "outline-cell")} style={칸}>
      <div style={줄}>
        <span className="kind">접수</span>
        <span className="number">{request.number}</span>
        <Revisions of={request.revisions} />
      </div>

      <div className="title" style={감싼제목}>{request.title || "(요청명 없음)"}</div>

      <div style={줄}>
        <span className="meta">{request.receivedOn}</span>
        <span className="meta">{request.demandAgency}</span>
        <span className="money">{request.goodsAmount}</span>
      </div>

      {/* 조달요구번호는 여덟 개까지 간다. 몇 개만 펴고 나머지는 셈으로 접는다. */}
      {request.requestNumbers.length > 0 && (
        <div style={번호줄}>
          {보일것.map((n) => (
            <span key={n} className="number">{n}</span>
          ))}

          {남은 > 0 && (
            // Tab 차례에서 뺀다. 두면 사슬마다 단추가 하나씩 서서, 넷째 사슬의 것을 누르려고
            // 앞의 모든 단추를 밟아야 한다 — 격자 안의 위젯은 격자의 키(Space)로 닿는다.
            <button
              className="chip"
              tabIndex={-1}
              aria-expanded={다펼침}
              onClick={() => toggle(id, false)}
            >
              {다펼침 ? "접기" : `외 ${남은}건`}
            </button>
          )}
        </div>
      )}
    </section>
  );
}

/**
 * 가운데 칸. 받는 것은 공고 하나가 아니라 <b>건</b>이지만, 그리는 것은 <b>현행 하나</b>다 —
 * 취소되어 물러난 공고까지 나란히 세우면 지나간 것과 살아 있는 것이 같은 무게로 보인다.
 *
 * <p>차수 이름표는 그 건의 <b>장수 전체</b>를 센다. 현행의 차수만 세면 취소·재공고로 갈린
 * 본번호의 장이 화면에서 통째로 없던 일이 되어, 다섯 장이 쌓인 건이 한 장짜리로 보인다.</p>
 */
function NoticeCell({ group, chrome }: { group: OutlineNoticeGroup | null; chrome: Chrome }) {
  if (!group) return <EmptyCell kind="공고" chrome={chrome} />;

  const notice = group.current;

  // 장을 <b>공고번호째로</b> 늘어놓는다. 차수만 적으면 본번호가 갈린 건에서 같은 숫자가 두 번
  // 나와(`000 · 000 · 001`) 무엇이 무엇인지 알 수 없다 — 갈린 것을 한 칸에 모아 놓고 그
  // 갈렸다는 사실만 지우는 셈이다.
  const 장 = [notice, ...group.superseded]
    .filter((n) => n !== null)
    .flatMap((n) => n.revisions.map((seq) => `${n.key}-${seq}`));

  // 현행이 정해지지 않은 건. 두 본번호가 서로의 최신 차수를 가리켜 살아남은 것이 없는
  // 꼴이라, 그래도 공고는 들어와 있다. 빈 칸으로 떨어뜨리면 <b>아직 안 들어온 것</b>과
  // 구별이 가지 않아, 넣은 사람이 넣지 않은 줄 안다 — 자리는 지키고 무엇이 없는지 적는다.
  if (!notice) {
    return (
      <section {...칸속성(chrome, "outline-cell")} style={칸}>
        <div style={줄}>
          <span className="kind">공고</span>
          <span className="number">(현행 확인 필요)</span>
          <Revisions of={장} />
        </div>
      </section>
    );
  }

  return (
    <section {...칸속성(chrome, "outline-cell")} style={칸}>
      <div style={줄}>
        <span className="kind">공고</span>
        <span className="number">{notice.number}</span>
        <Revisions of={장} />
      </div>

      <div className="title" style={감싼제목}>{notice.title || "(공고명 없음)"}</div>

      <div style={줄}>
        <span className="meta">{notice.postedAt}</span>
        <span className="meta">{notice.agency}</span>
        <span className="money">{notice.estimatedPrice}</span>
      </div>
    </section>
  );
}

function ContractCell({
  contract, chrome, open, onToggle,
}: { contract: OutlineContract; chrome: Chrome; open: boolean; onToggle: () => void }) {
  return (
    <section {...칸속성(chrome, "outline-cell")} style={칸}>
      <div style={줄}>
        <span className="kind">계약</span>
        <span className="number">{contract.number}</span>
        <Revisions of={contract.revisions} />
      </div>

      <div className="title" style={감싼제목}>{contract.title || "(계약건명 없음)"}</div>

      <div style={줄}>
        <span className="meta">{contract.counterparty}</span>
        <span className="meta">{contract.contractedOn}</span>
        <span className="money">{contract.amount}</span>
      </div>

      {contract.items.length > 0 && (
        <>
          {/* 요구번호 접이와 같은 까닭으로 Tab 차례에서 뺀다 — Space 가 이 자리를 대신한다. */}
          <button
            className="chip"
            tabIndex={-1}
            aria-expanded={open}
            onClick={onToggle}
            style={접이}
          >
            {open ? "▾" : "▸"} 품목 {contract.items.length}
          </button>

          {open && (
            <table className="outline-items">
              <thead>
                <tr>
                  <th>순번</th><th>품명</th><th>규격</th>
                  <th>수량</th><th>단위</th><th>단가</th><th>금액</th>
                </tr>
              </thead>
              <tbody>
                {contract.items.map((i) => (
                  <tr key={i.lineNo}>
                    <td className="num">{i.lineNo}</td>
                    <td>{i.name}</td>
                    <td>{i.specification}</td>
                    <td className="num">{i.quantity}</td>
                    <td>{i.unit}</td>
                    <td className="num">{i.unitPrice}</td>
                    <td className="num">{i.amount}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </>
      )}
    </section>
  );
}

/** 쌓인 차수. 하나뿐이면 굳이 말하지 않는다 — 모든 줄에 붙으면 아무 말도 아니게 된다. */
function Revisions({ of }: { of: string[] }) {
  if (of.length <= 1) return null;
  return <details className="revisions"><summary>문서 {of.length}개</summary><span>차수 {of.join(" · ")}</span></details>;
}

/** 사슬을 부르는 이름. 앞칸부터 이름이 있는 것을 쓴다 — 사람이 그렇게 부른다. */
function 이름(chain: OutlineChain): string {
  return chain.request?.title || chain.notice?.current?.title
    || chain.contracts[0]?.title || chain.key;
}

/**
 * 걸리는 것만 남긴다.
 *
 * <p>접수나 공고가 걸리면 그 사슬의 계약을 <b>모두</b> 남긴다 — 앞칸을 찾은 사람이 보고 싶은
 * 것은 거기서 나온 계약 전부다. 계약만 걸리면 그 계약만 남기되 앞칸은 그대로 두어 어디에
 * 달린 계약인지 보이게 한다.</p>
 */
function filter(tree: Tree, needle: string): Tree {
  if (!needle) return tree;

  const 걸림 = (values: string[]) => values.some((v) => v.toLowerCase().includes(needle));

  const hitsContract = (c: OutlineContract) =>
    걸림([c.number, c.title, c.counterparty, c.demandAgency, ...c.items.map((i) => i.name)]);

  // 건 안의 <b>장 전부</b>를 훑는다. 현행만 보면 취소된 원공고의 번호로는 아무것도 걸리지
  // 않는데, 사람 손에 들려 있는 종이가 바로 그 번호일 때가 많다.
  const hitsNotice = (group: OutlineNoticeGroup) =>
    [group.current, ...group.superseded].some(
      (n) => n !== null && 걸림([n.number, n.title, n.agency]));

  const hitsHead = (chain: OutlineChain) =>
    (chain.notice !== null && hitsNotice(chain.notice)) ||
    (chain.request !== null &&
      걸림([chain.request.number, chain.request.title, chain.request.demandAgency,
            ...chain.request.requestNumbers]));

  const chains = tree.chains
    .map((chain) => (hitsHead(chain) ? chain : { ...chain, contracts: chain.contracts.filter(hitsContract) }))
    .filter((chain) => hitsHead(chain) || chain.contracts.length > 0);

  return { chains };
}

// ── 자리잡기 ──────────────────────────────────────────
// 살갗(바닥·테두리·낱말·알약)은 app.css 의 구조 보기 규칙이 쥔다 — .kind·.number·.title·
// .meta·.money·.revisions 는 .outline-cell 아래에서만 사니 칸마다 그 이름을 얹는다.
// 여기서 하는 일은 세 칸을 가로로 세우는 것뿐이다.

const 칸: CSSProperties = {
  display: "flex", flexDirection: "column", alignItems: "stretch", gap: "0.25rem",
  flex: "1 1 0", minWidth: 0,
};

/** 계약이 하나이거나 없을 때. 칸 하나가 그대로 제자리를 차지한다. */
const 칸자리: CSSProperties = { display: "flex", flexDirection: "column", flex: "1 1 0", minWidth: 0 };

/** 계약이 둘 이상일 때만. 위아래로 갈라져 펼쳐진다. */
const 갈래: CSSProperties = { ...칸자리, gap: "0.25rem" };

const 줄: CSSProperties = { display: "flex", flexWrap: "wrap", alignItems: "baseline", gap: "0.25rem 0.5rem", minWidth: 0 };

const 번호줄: CSSProperties = {
  display: "flex", flexWrap: "wrap", alignItems: "baseline", gap: "0.25rem 0.5rem",
};

/** 칸이 좁아 한 줄에 안 들어간다. 말줄임 대신 접어 넘긴다 — 건명은 끝까지 읽어야 한다. */
const 감싼제목: CSSProperties = { whiteSpace: "normal", overflow: "visible", textOverflow: "clip" };

const 접이: CSSProperties = { alignSelf: "flex-start", margin: 0 };

/** 갈래 밖에 서므로 .outline-cell 의 규칙이 닿지 않는다. 같은 무게로 손수 맞춘다. */
const 갈래셈: CSSProperties = {
  color: "var(--a-muted)", fontSize: "var(--fs-caption)", padding: "0 0.25rem",
};

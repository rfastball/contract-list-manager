import { useEffect, useRef, useState, type ReactNode } from "react";
import { call } from "./bridge";
import { ErpTools } from "./ErpTools";
import { 검토대기, 탭설명 } from "./Mirror";
import type { CaptureDay, CaptureEntry, ExtensionContact, ExtensionLive, ExtensionLogEntry, ExtensionStatus, MirrorState, MirrorTab } from "./types";

/**
 * 나라장터 — 자료가 들어오는 <b>한 원천</b>과 그것이 갈라지는 세 갈래(접수·공고·계약).
 *
 * <p><b>앱이 모르는 것은 그리지 않는다.</b> 앱은 브라우저를 들여다볼 수 없다. 아는 것은 셋이다 — 확장이 쥔 포트로
 * 뜬 호스트가 지금 살아 있는지(상시 연결, ADR-035), 붙고 끊긴 기록, 성공한 저장의 기록(<code>captures</code>).
 * 그래서 붙어 있을 때만 「연결됨」 을 말하고 통로를 실선으로 긋는다. <b>끊긴 까닭은 말하지 않는다</b> — 브라우저를
 * 닫았는지 확장을 껐는지 앱은 모른다.</p>
 */

export type Kind = "접수" | "공고" | "계약";
export const KINDS: Kind[] = ["접수", "공고", "계약"];

/** 갈래까지의 거리에 맞춘 흐름 시간. 같은 속도로 흐르도록 먼 갈래일수록 길다. */
const FLOW_MS: Record<Kind, number> = { 접수: 700, 공고: 850, 계약: 1000 };
/** 통로를 한 번 지나는 시간. 두 마디라 반씩 나눠 미끄러진다. */
const CONDUIT_MS = 1300;

const isKind = (k: string): k is Kind => (KINDS as string[]).includes(k);

/** 움직임을 줄여 달라고 했는가. 그러면 흐름·마디·펼침 없이 +N 만 선다. */
const 움직임줄임 = () =>
  typeof window.matchMedia === "function" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;

// ── 때 ─────────────────────────────────────────────────
// 기록의 때는 모두 로컬 ISO(yyyy-MM-ddTHH:mm:ss)로 온다. 글자를 잘라 쓰고 Date 로 옮기지 않는다 —
// 옮기면 시험 기계의 시간대가 끼어든다.

const 두자리 = (n: number) => String(n).padStart(2, "0");
const 날짜 = (d: Date) => `${d.getFullYear()}-${두자리(d.getMonth() + 1)}-${두자리(d.getDate())}`;
const 오늘인가 = (at: string, now: Date) => at.slice(0, 10) === 날짜(now);

/** 오늘이면 <code>HH:MM</code>, 아니면 <code>MM/DD</code>. 한 줄이 늘지 않게 짧게. */
export const 짧은때 = (at: string, now = new Date()) =>
  오늘인가(at, now) ? at.slice(11, 16) : `${at.slice(5, 7)}/${at.slice(8, 10)}`;

/** 오늘이면 <code>오늘 HH:MM</code>, 아니면 <code>MM/DD HH:MM</code>. */
export const 긴때 = (at: string, now = new Date()) =>
  오늘인가(at, now) ? `오늘 ${at.slice(11, 16)}` : `${at.slice(5, 7)}/${at.slice(8, 10)} ${at.slice(11, 16)}`;

export const 브라우저이름 = (browser: string) => (browser === "기타" ? "기타 브라우저" : browser);

/** 붙은 때. 오늘이면 <code>HH:MM</code>, 아니면 <code>MM/DD HH:MM</code> — 「…부터」 앞에 선다. */
export const 부터때 = (at: string, now = new Date()) =>
  오늘인가(at, now) ? at.slice(11, 16) : `${at.slice(5, 7)}/${at.slice(8, 10)} ${at.slice(11, 16)}`;

/**
 * 받침 — 브라우저 이름·판 번호 뒤의 조사를 고른다. 없으면 빈 문자열, ㄹ 이면 「ㄹ」, 그 밖은 「ㅇ」.
 * Chrome(크롬)·0(영)·3(삼)·6(육) 은 받침이 있고, 1(일)·7(칠)·8(팔) 은 ㄹ 이다.
 */
export const 받침 = (word: string) => {
  const last = word.trim().slice(-1);
  if (/[0-9]/.test(last)) return "036".includes(last) ? "ㅇ" : "178".includes(last) ? "ㄹ" : "";
  if (/[가-힣]/.test(last)) {
    const code = (last.charCodeAt(0) - 0xac00) % 28;
    return code === 0 ? "" : code === 8 ? "ㄹ" : "ㅇ";
  }
  return word === "Chrome" ? "ㅇ" : "";
};
export const 이가 = (w: string) => w + (받침(w) ? "이" : "가");
export const 을를 = (w: string) => w + (받침(w) ? "을" : "를");
/** 「으로/로」 — ㄹ 받침 뒤에는 「로」. */
export const 으로 = (w: string) => w + (받침(w) && 받침(w) !== "ㄹ" ? "으로" : "로");

// ── 원천의 상태 ────────────────────────────────────────

/**
 * <code>live</code> — 지금 붙어 있고 판이 앱과 같다. <code>stale</code> — 붙어 있으나 판이 다르다.
 * <code>offline</code> — 붙은 적은 있으나 지금 붙은 것이 없다. <code>waiting</code> — 준비했으나 아직 한 번도 붙지 않았다.
 */
export type SourceKind = "unknown" | "readonly" | "setup" | "waiting" | "stale" | "live" | "offline";

export type SourceState = {
  kind: SourceKind;
  /** 사이드바 원천 밑의 한 줄. 짧고 바뀌지 않는다 — 들어온 순간은 흐름과 +N 이 말한다. */
  text: string;
  /** 가장 늦게 인사한 브라우저. 인사 기록이 없으면 null. */
  latest: ExtensionContact | null;
  /** 지금 붙어 있는 브라우저. 통로·보드는 이것으로 말한다 — 옛 판이면 옛 판인 쪽. */
  attached: ExtensionLive | null;
  /** 지금 붙은 때 — 붙어 있는 것 중 가장 이른 것. */
  since: string | null;
  /** 마지막으로 붙어 있던 때와 브라우저 — 인사 흔적과 오늘의 기록 중 가장 늦은 것. 끊긴 동안 「마지막 연결」 이 이것이다. */
  lastSeen: { browser: string; at: string } | null;
  /** 옛 판인 쪽의 판 — 붙은 확장의 판이 다르면 그것, 아니면 디스크에 풀어 둔 판. */
  staleVersion: string | null;
};

/** 브라우저마다 마지막 인사 하나가 남는다. 그중 가장 늦은 것. */
export function 마지막인사(status: ExtensionStatus | null | undefined): ExtensionContact | null {
  if (!status || status.contacts.length === 0) return null;
  return status.contacts.reduce((a, b) => (b.at > a.at ? b : a));
}

/** 붙어 있는 브라우저들의 이름 — 같은 브라우저가 둘이면(프로필 여럿) 한 번만. */
export const 붙은이름 = (live: ExtensionLive[]) =>
  [...new Set(live.map((l) => 브라우저이름(l.browser)))].join(" · ");

/**
 * 원천이 지금 어떤가. 붙어 있는지는 상시 연결이 말한다(<code>live</code>) — 인사 흔적만으로는 「연결됨」 이라 하지 않는다.
 *
 * <p>상태를 읽지 못했으면(개발 실행·시험 홈은 다리가 거절한다) 준비 전으로 본다. 준비를 누르면 거기서 까닭이 선다.</p>
 */
export function 원천상태(status: ExtensionStatus | null | undefined, readOnly: boolean): SourceState {
  const latest = 마지막인사(status);
  const live = status?.live ?? [];
  const oldLive = status ? live.find((l) => l.version !== status.embeddedVersion) : undefined;
  const diskStale = !!status && status.diskVersion !== null && status.diskVersion !== status.embeddedVersion;
  const stale = live.length > 0 && (!!oldLive || diskStale);
  const staleVersion = !stale ? null : oldLive ? oldLive.version : status!.diskVersion;
  const attached = oldLive ?? live[0] ?? null;
  const since = live.length > 0 ? live.map((l) => l.connectedAt).reduce((a, b) => (b < a ? b : a)) : null;
  const seen = [
    ...(status?.contacts ?? []).map((c) => ({ browser: c.browser, at: c.at })),
    ...(status?.log ?? []).map((e) => ({ browser: e.browser, at: e.at })),
  ];
  const lastSeen = seen.length > 0 ? seen.reduce((a, b) => (b.at > a.at ? b : a)) : null;
  const base = { latest, attached, since, lastSeen, staleVersion };

  if (readOnly) return { ...base, kind: "readonly", text: "열람 창 · 수집 안 함" };
  if (status === undefined) return { ...base, kind: "unknown", text: "" };
  if (!status?.prepared) return { ...base, kind: "setup", text: "확장 설치 필요" };
  if (stale) return { ...base, kind: "stale", text: "확장이 옛 판" };
  if (live.length > 0) return { ...base, kind: "live", text: `${붙은이름(live)} 연결됨` };
  if (!lastSeen) return { ...base, kind: "waiting", text: "확장 연결 기다림" };
  return { ...base, kind: "offline", text: "연결 안 됨" };
}

/** 붙어 있는가 — 판이 같든 다르든. 줄기·통로가 실선이 된다. */
const 붙음 = (kind: SourceKind) => kind === "live" || kind === "stale";

/** 원천 아이콘·알약의 결. 붙어 있고 판이 같으면 ok, 손이 더 가야 하면 warn, 끊겼거나 열람 창이면 흐리게. */
const 결 = (kind: SourceKind) =>
  kind === "live" ? "ok" : kind === "setup" || kind === "waiting" || kind === "stale" ? "warn" : "neutral";

// ── 들어옴 ─────────────────────────────────────────────

export type Arrivals = {
  /** 창이 떠 있는 동안 들어온 것 중 그 갈래를 아직 열어 보지 않은 수. */
  counts: Record<Kind, number>;
  /** 원천에서 갈래로 미끄러지는 흐름. <code>n</code> 은 같은 갈래로 다시 흐를 때 그림을 새로 세우는 열쇠다. */
  flow: { kind: Kind; n: number } | null;
  /** 마디가 차오른 갈래. */
  flash: Kind | null;
  /** 막 들어와 펼쳐지는 줄. */
  fresh: ReadonlySet<string>;
  /** 통로를 지나는 차례. 0 이면 지나는 것이 없다. */
  pulse: number;
};

/**
 * 새로 들어온 수집을 가려내 +N 과 움직임으로 바꾼다.
 *
 * <p>처음 읽은 기록은 이미 본 것으로 친다 — 창을 켤 때 오늘 것이 모두 「새로」 로 서면 아무것도 말하지 않는 것과 같다.
 * 그 갈래를 보고 있는 동안 들어온 것은 세지 않고, 그 갈래를 열면 0 이 된다.</p>
 *
 * <p>수는 잃지 않는다. 움직임은 새 수집이 오면 처음부터 다시 그리지만, 수를 올리는 약속은 거두지 않는다.</p>
 */
export function use들어옴(captures: CaptureDay | null, viewing: Kind | null): Arrivals {
  const seen = useRef<Set<string> | null>(null);
  const viewingNow = useRef(viewing);
  viewingNow.current = viewing;
  const [counts, setCounts] = useState<Record<Kind, number>>({ 접수: 0, 공고: 0, 계약: 0 });
  const [flow, setFlow] = useState<Arrivals["flow"]>(null);
  const [flash, setFlash] = useState<Kind | null>(null);
  const [fresh, setFresh] = useState<ReadonlySet<string>>(new Set());
  const [pulse, setPulse] = useState(0);
  const motion = useRef<ReturnType<typeof setTimeout>[]>([]);
  const bumps = useRef<ReturnType<typeof setTimeout>[]>([]);

  useEffect(() => () => { [...motion.current, ...bumps.current].forEach(clearTimeout); }, []);

  useEffect(() => {
    if (viewing) setCounts((c) => (c[viewing] === 0 ? c : { ...c, [viewing]: 0 }));
  }, [viewing]);

  useEffect(() => {
    if (!captures) return;
    const ids = captures.entries.map((e) => e.captureId);
    if (captures.last) ids.push(captures.last.captureId);
    if (seen.current === null) { seen.current = new Set(ids); return; }

    const known = seen.current;
    const arrived = captures.entries.filter((e) => !known.has(e.captureId) && isKind(e.kind));
    ids.forEach((id) => known.add(id));
    if (arrived.length === 0) return;

    const bump = () => setCounts((c) => {
      const next = { ...c };
      for (const e of arrived) if (e.kind !== viewingNow.current) next[e.kind]++;
      return next;
    });

    if (움직임줄임()) { bump(); return; }

    // 가장 늦은 것의 갈래로 흐른다. 새 수집이 오면 움직임은 처음부터 다시 그린다.
    const kind = arrived[0].kind;
    const dur = FLOW_MS[kind];
    motion.current.forEach(clearTimeout);
    const later = (fn: () => void, ms: number, into = motion.current) => { into.push(setTimeout(fn, ms)); };
    setFresh(new Set(arrived.map((e) => e.captureId)));
    setFlow({ kind, n: Date.now() });
    setFlash(null);
    setPulse((p) => p + 1);
    // 흐름이 마디에 스며들 무렵 마디가 차오르고 수가 오른다.
    later(() => setFlash(kind), Math.round(dur * 0.82));
    later(bump, Math.round(dur * 0.82), bumps.current);
    later(() => setFlow(null), dur + 60);
    later(() => setPulse(0), CONDUIT_MS);
    // 천천히 제자리로.
    later(() => { setFlash(null); setFresh(new Set()); }, dur + 2000);
  }, [captures]);

  return { counts, flow, flash, fresh, pulse };
}

// ── 사이드바의 가지 ────────────────────────────────────

const ROOT_H = 56, KID_H = 40, TRUNK = 10, NODE_X = 27, BEND = 7;

type TreeProps = {
  /** 고른 탭. 공고 연결을 보고 있으면 null. */
  selected: string | null;
  /** 키보드로 서 있는 탭(tabIndex 0). */
  focusable: string;
  onSelect: (tab: "나라장터" | Kind) => void;
  source: SourceState;
  /** 갈래마다 전체 건수. 아직 모르면 null. */
  totals: Record<Kind, number> | null;
  arrivals: Arrivals;
  /** Chrome 에서 보고 있는 수집할 수 있는 화면 — 그 종류의 갈래 옆에 눈을 세운다. */
  seeing?: { kind: Kind; label: string } | null;
  onSee?: () => void;
};

/**
 * 나라장터 한 원천에서 줄기가 내려와 갈래마다 꺾여 마디에 닿는다. 줄기·꺾임·마디는 꾸밈이라 읽히지 않는다.
 *
 * <p>탭 목록 안에 선다 — 방향키 차례는 App 이 쥔다. 들어오는 흐름은 경로 하나를 따라 처음부터 끝까지 미끄러진다:
 * 칸마다 켜고 끄지 않는다.</p>
 */
export function SourceTree({ selected, focusable, onSelect, source, totals, arrivals, seeing, onSee }: TreeProps) {
  const tone = 결(source.kind);
  const { flow, flash, counts } = arrivals;
  const treeH = ROOT_H + KID_H * KINDS.length;
  const idx = flow ? KINDS.indexOf(flow.kind) : -1;
  const cy = ROOT_H + KID_H * Math.max(idx, 0) + KID_H / 2;
  const d = `M${TRUNK} ${ROOT_H / 2 + 10} L${TRUNK} ${cy - BEND} Q${TRUNK} ${cy} ${TRUNK + BEND} ${cy} L${NODE_X} ${cy}`;

  return (
    <div className={`source-tree${source.kind === "readonly" ? " readonly" : 붙음(source.kind) ? " connected" : ""}`}>
      {flow && (
        <svg key={flow.n} className="source-flow" aria-hidden="true" viewBox={`0 0 40 ${treeH}`} style={{ height: treeH }}>
          <path d={d} pathLength={100} style={{ animationDuration: `${FLOW_MS[flow.kind]}ms` }} />
        </svg>
      )}

      <div className="source-root">
        <span className="tree-deco" aria-hidden="true">
          <span className="tree-line root-down" />
          <span className={`source-icon ${tone}`}>
            <svg viewBox="0 0 24 24"><rect x="3" y="4" width="18" height="16" rx="2" /><path d="M3 9h18" /></svg>
          </span>
        </span>
        <button id="tab-나라장터" role="tab" className="tab source-tab"
          aria-selected={selected === "나라장터"} aria-controls="workspace"
          tabIndex={focusable === "나라장터" ? 0 : -1}
          onClick={() => onSelect("나라장터")}>
          <span className="source-name"><b>나라장터</b>
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="m9 6 6 6-6 6" /></svg>
          </span>
          <span className="source-status" role="status">{source.text}</span>
        </button>
      </div>

      {KINDS.map((k, i) => {
        const n = counts[k];
        const total = totals?.[k];
        const label = total === undefined ? k : `${k} ${total.toLocaleString("ko-KR")}건`;
        return (
          <div className="source-branch" key={k}>
            <span className="tree-deco" aria-hidden="true">
              <span className="tree-line up" />
              {i < KINDS.length - 1 && <span className="tree-line down" />}
              <span className="tree-elbow" />
              <span className={`tree-node${flash === k ? " flash" : selected === k ? " current" : ""}`} />
            </span>
            <button id={`tab-${k}`} role="tab" className="tab branch-tab"
              aria-selected={selected === k} aria-controls="workspace"
              aria-label={n > 0 ? `${label}, 새로 들어온 ${n}건` : label}
              tabIndex={focusable === k ? 0 : -1}
              onClick={() => onSelect(k)}>
              <span>{k}</span>
              <span className="branch-meta">
                {n > 0 && <span className="branch-new" aria-hidden="true">+{n}</span>}
                {total !== undefined && <span className="branch-count" aria-hidden="true">{total.toLocaleString("ko-KR")}</span>}
              </span>
            </button>
            {seeing?.kind === k && (
              <button type="button" className="branch-see" aria-label={seeing.label} title={seeing.label} onClick={onSee}>
                <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12z" /><circle cx="12" cy="12" r="2.8" /></svg>
              </button>
            )}
          </div>
        );
      })}
    </div>
  );
}

// ── 나라장터 화면 ──────────────────────────────────────

type Props = {
  readOnly: boolean;
  /** 확장 상태. 아직 못 읽었으면 undefined, 읽지 못했으면 null. */
  status: ExtensionStatus | null | undefined;
  source: SourceState;
  captures: CaptureDay | null;
  arrivals: Arrivals;
  /** 지금 열린 파일의 이름(작업자료면 수집이 쌓이는 자리). */
  workfileName: string;
  /** 지금 열린 파일의 역할 이름. */
  roleName: string;
  /** 확장 상태를 곧바로 다시 읽는다(준비한 직후). 평소에는 App 이 2초마다 읽는다 — 사이드바도 쓰므로. */
  onStatusReload: () => void;
  /** 그 종류 탭으로 옮겨 번호로 검색한다. */
  onOpen: (kind: Kind, number: string) => void;
  /** JSON 검토로 자료가 바뀌었다. */
  onErpChanged: () => Promise<void>;
  /** 화면 밖으로 알릴 실패. */
  onError: (message: string) => void;
  /** Chrome 의 나라장터 탭(지금 보는 화면, ADR-036). 붙어 있지 않으면 null. */
  mirror: MirrorState | null;
  /** 그 탭을 지금 보는 화면에 비춘다. */
  onShowTab: (id: string) => void;
};

type View = "들어온 자료" | "확장 상태";

const 열기대상 = (browser: string | undefined) => (browser === "Edge" ? "edge" : "chrome");
const 관리이름 = (browser: string | undefined) => (browser === "Edge" ? "Edge 확장 관리 열기" : "Chrome 확장 관리 열기");

/**
 * 나라장터 화면. 「들어온 자료」 와 「Chrome 확장 상태」 를 낮은 위계의 전환으로 오간다 — 같은 문을 두 눈으로 본다.
 *
 * <p>확장 준비·관리 열기는 설정에 있던 그 길(<code>prepareExtension</code>·<code>openExtensionSetup</code>)을 그대로 쓴다.</p>
 */
export function NaraPanel(props: Props) {
  const { readOnly, status, source, captures, arrivals, workfileName, onStatusReload, onError } = props;
  const [view, setView] = useState<View>("들어온 자료");
  const [advanced, setAdvanced] = useState(false);
  const [focusJson, setFocusJson] = useState(0);
  const [preparedNow, setPreparedNow] = useState<{ folder: string; version: string }>();

  const setupShown = !readOnly && (source.kind === "setup" || source.kind === "waiting");

  /** 「JSON 파일로 가져오기…」 — 고급을 펴고 JSON 고르는 자리로 초점을 옮긴다. */
  const openJson = () => { setView("들어온 자료"); setAdvanced(true); setFocusJson((n) => n + 1); };

  const openManager = (browser: string | undefined) =>
    void call("openExtensionSetup", 열기대상(browser)).catch((e: Error) => onError(e.message));

  const advancedBlock = (
    <Advanced open={advanced} onToggle={() => setAdvanced((v) => !v)} readOnly={readOnly}
      focusRequest={focusJson} onErpChanged={props.onErpChanged} />
  );

  return (
    <>
      <header className="top">
        <div className="page-heading">
          <h1>나라장터</h1>
          <p>브라우저 확장이 나라장터의 접수·공고·계약 화면을 이 작업자료로 가져옵니다.</p>
        </div>
        {!setupShown && (
          <div className="top-group">
            <button className="action" onClick={openJson} disabled={readOnly}>JSON 파일로 가져오기…</button>
          </div>
        )}
      </header>

      <div className="view-bar">
        <div className="shape nara-views" role="group" aria-label="나라장터 보기">
          {(["들어온 자료", "확장 상태"] as View[]).map((v) => (
            <button key={v} className="chip" aria-pressed={view === v} onClick={() => setView(v)}>
              {v === "확장 상태" ? "Chrome 확장 상태" : v}
            </button>
          ))}
        </div>
      </div>

      <div className="nara">
        {view === "들어온 자료" ? (
          <>
            {readOnly && <ReadOnlyNote />}
            {!readOnly && (붙음(source.kind) || source.kind === "offline") && status && (
              <Conduit status={status} source={source} pulse={arrivals.pulse} workfileName={workfileName}
                onManage={openManager} />
            )}
            {setupShown && (
              <Setup status={status} source={source} prepared={preparedNow}
                onPrepared={(p) => { setPreparedNow(p); onStatusReload(); }} onJson={openJson} />
            )}
            {setupShown && !(captures && captures.entries.length > 0)
              ? <div className="nara-body">{advancedBlock}</div>
              : (
                <div className="nara-body">
                  <CaptureList readOnly={readOnly} captures={captures} fresh={arrivals.fresh} onOpen={props.onOpen}>
                    {advancedBlock}
                  </CaptureList>
                </div>
              )}
          </>
        ) : (
          <Board {...props} onManage={openManager} onShowList={() => setView("들어온 자료")} />
        )}
      </div>
    </>
  );
}

function Advanced({ open, onToggle, readOnly, focusRequest, onErpChanged }: {
  open: boolean; onToggle: () => void; readOnly: boolean; focusRequest: number; onErpChanged: () => Promise<void>;
}) {
  return (
    <div className="nara-advanced">
      <button type="button" className="nara-disclosure" aria-expanded={open} aria-controls="nara-advanced" onClick={onToggle}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="m9 6 6 6-6 6" /></svg>
        고급 · 수집 규칙과 지난 수집 다시 보기
      </button>
      {open && (
        <div id="nara-advanced" className="nara-advanced-body">
          {/* 수집은 작업자료에만 쌓인다. 열람 창은 매핑도 JSON 검토도 건드리지 않는다. */}
          {readOnly
            ? <p className="hint">열람 중에는 수집 도구를 쓰지 않습니다. 작업자료 창에서 하세요.</p>
            : <ErpTools startOpen focusRequest={focusRequest} onChanged={onErpChanged} />}
        </div>
      )}
    </div>
  );
}

function ReadOnlyNote() {
  return (
    <section className="nara-readonly" aria-label="수집 통로">
      <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="5" y="11" width="14" height="10" rx="2" /><path d="M8 11V8a4 4 0 0 1 8 0v3" /></svg>
      <p>
        <b>이 창에는 수집한 자료가 저장되지 않습니다.</b>{" "}
        <span className="soft">브라우저 확장은 작업자료 창에만 저장합니다. 아래는 이 파일에 남은 수집 기록입니다.</span>
      </p>
    </section>
  );
}

const 브라우저그림 = <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="3" y="4" width="18" height="16" rx="2" /><path d="M3 9h18M6.5 6.5h.01M9 6.5h.01" /></svg>;
const 확장그림 = <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M9 4h6v4h-6zM7 8h10v5a5 5 0 0 1-10 0zM12 18v3" /></svg>;
const 문서그림 = <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 3h10l4 4v14H5zM14 3v5h5M8 12h8M8 16h5" /></svg>;

/**
 * 브라우저 → 확장 → 작업자료. 붙어 있으면 실선, 끊겼으면 점선이다. 새 수집이 들어오면 — 붙어 있을 때만 — 줄을 따라
 * 한 번 미끄러진다.
 *
 * <p>세 마디는 같은 폭의 칸에 아이콘을 가운데 세우고 이름을 그 아래에 둔다. 줄은 아이콘의 가운데 높이에서 한 아이콘의
 * 가장자리부터 다음 아이콘의 가장자리까지 끊김 없이 닿는다 — 글 옆에 줄을 두면 글의 빈자리에서 시작해 다음 아이콘에
 * 못 미쳐 끊어져 보였다.</p>
 */
function Conduit({ status, source, pulse, workfileName, onManage }: {
  status: ExtensionStatus; source: SourceState; pulse: number; workfileName: string;
  onManage: (browser: string | undefined) => void;
}) {
  const connected = 붙음(source.kind);
  const live = source.kind === "live";
  const stale = source.kind === "stale";
  const browser = (connected ? source.attached?.browser : source.lastSeen?.browser) ?? source.latest?.browser ?? "Chrome";
  const name = 브라우저이름(browser);
  const version = source.attached?.version ?? source.latest?.version ?? status.diskVersion ?? status.embeddedVersion;
  const since = source.since ? 부터때(source.since) : "";
  const line = (delay: boolean) => (
    <div className={`conduit-line${delay ? " second" : " first"}`} aria-hidden="true">
      <span className={`conduit-dash${connected ? " solid" : ""}`} />
      {live && pulse > 0 && <span key={pulse} className={`conduit-glide${delay ? " later" : ""}`}><span /><span /></span>}
    </div>
  );

  return (
    <section className="nara-conduit" aria-label="수집 통로">
      <div className={`conduit-row${connected ? " on" : ""}`}>
        <div className="conduit-end">
          <span className={`conduit-icon${connected ? " on" : ""}`}>{브라우저그림}</span>
          <div className="conduit-label">
            <div className="conduit-title">{connected ? name : `${name} · 연결 안 됨`}</div>
            <div className="soft">{connected
              ? `${since}부터 연결됨`
              : source.lastSeen ? `마지막 연결 ${긴때(source.lastSeen.at)}` : ""}</div>
          </div>
        </div>
        {line(false)}
        <div className="conduit-end">
          <span className={`conduit-icon${live ? " on" : ""}`}>{확장그림}</span>
          <div className="conduit-label">
            <div className="conduit-title">확장 {version}</div>
            {stale
              ? <div className="warn-strong">옛 판 · 새 판은 {status.embeddedVersion}</div>
              : <div className="soft">단축키 Alt+Shift+S</div>}
          </div>
        </div>
        {line(true)}
        <div className="conduit-end">
          <span className="conduit-icon file">{문서그림}</span>
          <div className="conduit-label">
            <div className="conduit-title num conduit-file" title={workfileName}>{workfileName}</div>
            <div className="soft">지금 열린 작업자료</div>
          </div>
        </div>
      </div>

      <div className="conduit-status">
        <span className="conduit-state">
          <span className={`dot${live ? " ok" : stale ? " warn" : " hollow"}`} aria-hidden="true" />
          {live ? `연결됨 · ${since}부터` : stale ? "연결됨 · 확장이 옛 판" : "연결 안 됨"}
        </span>
        <span className="soft">{live
          ? "나라장터에서 저장하면 아래 목록과 왼쪽 메뉴에 바로 나타납니다."
          : stale
            ? "옛 판으로도 저장은 되지만, 새 수집 규칙이 적용되지 않을 수 있습니다."
            : `${이가(name)} 꺼져 있거나 확장이 꺼져 있습니다. ${을를(name)} 켜면 자동으로 다시 연결됩니다.`}</span>
      </div>

      {stale && (
        <div className="nara-stale" role="alert">
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 3 2.5 20h19zM12 10v4M12 17h.01" /></svg>
          <p><b className="warn-strong">{name}의 확장이 옛 판({source.staleVersion})입니다.</b>{" "}
            나라장터 화면을 열면 새 판({status.embeddedVersion})으로 자동으로 바뀝니다. 바뀌지 않으면 확장 관리에서 새로고침하세요.</p>
          <button className="action" onClick={() => onManage(browser)}>{관리이름(browser)}</button>
        </div>
      )}
    </section>
  );
}

const 마침 = <svg viewBox="0 0 24 24" role="img" aria-label="마침"><path d="m5 12.5 4.5 4.5L19 7.5" /></svg>;

/**
 * 처음 설치 세 단계. 준비하면 1단계가 마치고 2단계에 폴더와 확장 관리 여는 길이 선다. 3단계는 누를 것이 없다 —
 * 브라우저가 처음 인사하면 이 화면이 저절로 통로로 넘어간다.
 */
function Setup({ status, source, prepared, onPrepared, onJson }: {
  status: ExtensionStatus | null | undefined; source: SourceState;
  prepared: { folder: string; version: string } | undefined;
  onPrepared: (p: { folder: string; version: string }) => void;
  onJson: () => void;
}) {
  const [copied, setCopied] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [justPrepared, setJustPrepared] = useState(false);
  const folderInput = useRef<HTMLInputElement>(null);

  // 상태를 읽지 못한 창(개발 실행)에서도 방금 준비했으면 다음 단계로 넘긴다.
  const ready = source.kind === "waiting" || prepared !== undefined;
  const folder = prepared?.folder ?? status?.folder ?? "";
  const version = prepared?.version ?? status?.diskVersion ?? status?.embeddedVersion ?? "";

  async function work(action: () => Promise<void>) {
    setBusy(true); setError("");
    try { await action(); }
    catch (e) { setError(e instanceof Error ? e.message : String(e)); }
    finally { setBusy(false); }
  }

  const prepare = () => work(async () => {
    setCopied(false); setJustPrepared(false);
    const result = await call<{ folder: string; version: string }>("prepareExtension");
    setJustPrepared(true);
    onPrepared(result);
    // 되면 폴더 선택 창에 바로 붙여 넣는다. 안 되면 아래 칸에서 손으로 복사한다.
    try { await navigator.clipboard.writeText(result.folder); setCopied(true); } catch { /* 칸에 맡긴다 */ }
  });

  const copy = async () => {
    try { await navigator.clipboard.writeText(folder); setCopied(true); }
    catch { folderInput.current?.select(); }
  };

  const marker = (state: "done" | "active" | "todo", n: number) =>
    <span className={`step-marker ${state}`}>{state === "done" ? 마침 : n}</span>;

  return (
    <section className="nara-setup" aria-label="확장 설치">
      <div className="nara-setup-in">
        <h2>Chrome 확장 설치</h2>
        <p className="soft lead">나라장터 자료는 확장으로 들어옵니다. 한 번 설치하면 앱을 새 판으로 바꿔도 확장이 함께 바뀝니다.</p>
        <ol className="steps">
          <li>
            {marker(ready ? "done" : "active", 1)}
            <div>
              <h3>확장 준비</h3>
              <p className="soft">앱에 들어 있는 확장을 폴더에 꺼내고, 이 앱과의 연결을 등록합니다. 관리자 권한은 필요 없습니다.</p>
              {ready
                ? <p className="ok-strong small">준비했습니다 · 확장 {version}</p>
                : <button className="action primary step-act" onClick={() => void prepare()} disabled={busy}>{busy ? "준비 중…" : "확장 준비"}</button>}
              {justPrepared && <p role="status" className="hint">확장 {version}을 준비했습니다. 브라우저에서 추가를 완료하세요.</p>}
            </div>
          </li>
          <li>
            {marker(ready ? "active" : "todo", 2)}
            <div>
              <h3>브라우저에 추가</h3>
              <p className="soft">확장 관리에서 <b>개발자 모드</b>를 켜고 <b>압축해제된 확장 프로그램을 로드합니다</b>를 눌러 아래 폴더를 고릅니다.</p>
              {ready && (
                <div className="step-tools">
                  <label className="step-folder"><span>확장 폴더</span>
                    <span className="row">
                      <input ref={folderInput} className="text" readOnly value={folder} onFocus={(e) => e.currentTarget.select()} />
                      <button className="action" type="button" onClick={() => void copy()}>경로 복사</button>
                    </span>
                  </label>
                  {copied && <p className="hint">폴더 경로를 복사했습니다 — 폴더 선택 창 주소칸에 Ctrl+V</p>}
                  <div className="row">
                    {([["chrome", "Chrome 확장 관리 열기"], ["edge", "Edge 확장 관리 열기"]] as const).map(([target, label]) =>
                      <button className="action" type="button" key={target} disabled={busy}
                        onClick={() => void work(async () => { await call("openExtensionSetup", target); })}>{label}</button>)}
                  </div>
                </div>
              )}
            </div>
          </li>
          <li>
            {marker("todo", 3)}
            <div>
              <h3>연결 확인</h3>
              <p className="soft">추가를 마치면 이 화면이 자동으로 바뀝니다.</p>
              {ready && <p role="status" className="waiting"><span className="dot hollow" aria-hidden="true" />Chrome 연결을 기다리는 중</p>}
            </div>
          </li>
        </ol>
        {error && <p role="alert" className="nara-error">{error}</p>}
        <p className="soft small foot">
          기관 정책으로 개발자 모드를 켤 수 없으면 정보화 담당에게 문의하세요. 확장 없이 가져와야 하면{" "}
          <button type="button" className="text-link" onClick={onJson}>JSON 파일로 가져오기</button>를 쓸 수 있습니다.
        </p>
      </div>
    </section>
  );
}

const 지금 = (at: string) => at.slice(11, 16);

/** 들어온 자료. 오늘 것 전부, 늦은 것부터. 새 줄은 펼쳐 들어온다. */
function CaptureList({ readOnly, captures, fresh, onOpen, children }: {
  readOnly: boolean; captures: CaptureDay | null; fresh: ReadonlySet<string>;
  onOpen: (kind: Kind, number: string) => void; children: ReactNode;
}) {
  const entries = captures?.entries ?? [];
  const last = captures?.last;
  return (
    <section className="nara-list" aria-label="들어온 자료">
      <div className="nara-list-head">
        <h2>{readOnly ? "이 파일에 남은 수집 기록" : "들어온 자료"}</h2>
        <span className="soft num">오늘 {entries.length}건</span>
      </div>
      <div className="nara-table-wrap">
        <div role="table" aria-label="들어온 자료" className="nara-table">
          <div role="row" className="nara-row head">
            <span role="columnheader">시각</span><span role="columnheader">종류</span>
            <span role="columnheader">번호-차수</span><span role="columnheader">건명</span>
            <span role="columnheader">결과</span><span role="columnheader"><span className="sr-only">이동</span></span>
          </div>
          {entries.map((e) => <CaptureRow key={e.captureId} entry={e} fresh={fresh.has(e.captureId)} onOpen={onOpen} />)}
          {captures && entries.length === 0 && (
            <div role="row" className="nara-row empty">
              <span role="cell">
                오늘 들어온 자료가 없습니다.
                {last && ` 마지막 수집 ${긴때(last.at)} · ${last.kind} ${last.number}`}
              </span>
            </div>
          )}
          {!captures && <div role="row" className="nara-row empty"><span role="cell">읽는 중…</span></div>}
        </div>
      </div>
      <p className="nara-foot">{readOnly
        ? "열람 중이라 기록을 보기만 합니다."
        : "저장된 자료만 보입니다. 검토 중이거나 저장 결과를 확인하지 못한 자료는 브라우저에 남아 있습니다."}</p>
      {children}
    </section>
  );
}

function CaptureRow({ entry, fresh, onOpen }: { entry: CaptureEntry; fresh: boolean; onOpen: (kind: Kind, number: string) => void }) {
  const kind = entry.kind;
  return (
    <div role="row" className={`nara-row${fresh ? " fresh" : ""}`}>
      <span role="cell" className="soft num">{지금(entry.at)}</span>
      <span role="cell"><span className="kind-pill">{kind}</span></span>
      <span role="cell" className="num nowrap">{entry.number}</span>
      <span role="cell" className="clip" title={entry.title}>{entry.title}</span>
      <span role="cell" className={entry.changed ? "ok-strong" : "soft"}>{entry.result}</span>
      <span role="cell" className="end">
        {isKind(kind) && (
          <button type="button" className="text-link plain" aria-label={`${kind} ${entry.number} 보기`}
            onClick={() => onOpen(kind, entry.number)}>보기</button>
        )}
      </span>
    </div>
  );
}

/**
 * 확장이 알린 오류 코드의 뜻. <b>표는 여기 하나다</b> — 확장은 코드만 보내고(자료는 싣지 않는다) 뜻은 앱이 안다.
 * 모르는 코드(새 확장)는 코드를 그대로 보인다.
 */
export const 오류뜻: Record<string, string> = {
  unconfirmed: "저장 결과를 확인하지 못했습니다",
  connection: "저장 대상에 연결하지 못했습니다",
  setup: "저장 대상 준비가 필요했습니다",
  rejected: "저장 대상이 요청을 받지 않았습니다",
  response: "저장 대상의 응답을 확인하지 못했습니다",
};
export const 오류말 = (code: string) => 오류뜻[code] ?? `확장이 알린 오류(${code})`;

/** 연결 기록 한 줄의 말. 끊긴 까닭은 붙이지 않는다 — 앱은 모른다. */
function 기록말(e: ExtensionLogEntry, embedded: string, named: boolean) {
  const what = e.event === "connected" ? "연결됨"
    : e.event === "disconnected" ? "연결 끊김"
    : `확장이 ${으로(e.version)} 바뀜`;
  const why = [
    named ? 브라우저이름(e.browser) : "",
    e.event === "connected" && e.version !== embedded ? `확장 ${e.version} · 앱은 ${embedded}` : "",
    e.event === "updated" && e.version === embedded ? "앱의 새 판에 맞춰 자동으로" : "",
  ].filter(Boolean).join(" · ");
  return { what, why };
}

/** Chrome 확장 상태 — 아는 것만. 값은 모두 한 줄이라 바뀌어도 칸이 흔들리지 않는다. */
function Board({ readOnly, status, source, captures, workfileName, roleName, mirror, onShowTab, onError, onManage, onShowList }: Props & {
  onManage: (browser: string | undefined) => void; onShowList: () => void;
}) {
  const latest = source.latest;
  const attached = source.attached;
  const tone = 결(source.kind);
  const contacts = [...(status?.contacts ?? [])].sort((a, b) => (a.at < b.at ? 1 : -1));
  const names = contacts.map((c) => 브라우저이름(c.browser));
  const title = `${names.length > 0 ? names.join(" · ") : "Chrome"} 확장`;
  const browser = attached?.browser ?? source.lastSeen?.browser ?? latest?.browser;
  const name = browser ? 브라우저이름(browser) : "Chrome";
  const stale = source.kind === "stale";
  const connected = 붙음(source.kind);
  const since = source.since ? 부터때(source.since) : "";
  const lastSeen = source.lastSeen ? 긴때(source.lastSeen.at) : "";
  const embedded = status?.embeddedVersion ?? "";
  const log = [...(status?.log ?? [])].reverse();
  const named = new Set(log.map((e) => e.browser)).size > 1;
  const updated = log.find((e) => e.event === "updated");

  const pill = {
    unknown: "확인 중", readonly: "열람 창", setup: "설치 전", waiting: "연결 기다림",
    live: "연결됨", stale: "옛 판", offline: "연결 안 됨",
  }[source.kind];
  const sub = {
    unknown: "확장 상태를 읽는 중입니다",
    readonly: "수집한 자료는 작업자료 창에 저장됩니다",
    setup: "아직 브라우저에 확장을 설치하지 않았습니다",
    waiting: "확장을 준비했고 브라우저의 첫 연결을 기다립니다",
    live: `${status ? 붙은이름(status.live) : name} · 확장 ${attached?.version ?? ""} · ${since}부터 연결됨`,
    stale: `${since}부터 연결됨 · 새 판으로 바꿔야 함`,
    offline: lastSeen ? `${name} · 마지막 연결 ${lastSeen}` : "",
  }[source.kind];
  const shownVersion = attached?.version ?? latest?.version ?? status?.diskVersion ?? null;
  const lastEntry = captures?.last;
  // 열린 탭은 붙어 있을 때만 안다. 끊긴 동안은 수를 지어내지 않는다.
  const tabsKnown = connected && !readOnly && mirror !== null && mirror.reporting;
  const tabs = tabsKnown ? mirror!.tabs : [];
  const waiting = tabsKnown ? 검토대기(mirror).length : null;
  const goTab = (t: MirrorTab) =>
    void call("mirrorCommand", String(t.pid), String(t.tabId), "focusTab").catch((e: Error) => onError(e.message));

  // 오류와 재시도. 오늘 것은 기록이 말하고, 지금 상태는 붙어 있을 때만 안다.
  const errors = status?.errors ?? [];
  const lastToday = errors.length > 0 ? errors[errors.length - 1] : null;
  const errorNow = !connected ? `${이가(name)} 연결되어 있지 않아 지금 상태는 알 수 없습니다.`
    : lastToday ? `오늘 오류 ${errors.length}건 · 마지막 ${lastToday.at.slice(11, 16)} ${오류말(lastToday.code)}`
    : "오늘은 오류가 없습니다.";
  // 그 아래 한 줄 — 붙어 있으면 오늘 전의 마지막 오류, 끊겼으면 알고 있는 마지막 오류.
  const past = connected ? status?.pastError ?? null : lastToday ?? status?.pastError ?? null;
  const errorPast = past
    ? `마지막 오류: ${긴때(past.at)} ${오류말(past.code)}${past.recovered ? " — 같은 요청을 다시 보내 해결했습니다" : ""}`
    : connected ? "그 전 7일 동안은 오류가 없었습니다." : "";

  // 확장 설정. 붙어 있고 확장이 알렸을 때만 바꿀 수 있다 — 브라우저가 여럿이면 앞 탭의 브라우저(없으면 처음 붙은 것).
  const settings = connected && !readOnly ? mirror?.settings ?? null : null;
  const [modeSent, setModeSent] = useState<"always" | "button" | null>(null);
  useEffect(() => { setModeSent(null); }, [settings?.panelMode]);
  const mode = settings ? modeSent ?? settings.panelMode : null;
  const setting = (command: "setPanelMode" | "openShortcuts", value?: "always" | "button") => {
    if (!settings) return;
    if (value) setModeSent(value);
    void call("extensionCommand", String(settings.pid), command, ...(value ? [value] : []))
      .catch((e: Error) => { setModeSent(null); onError(e.message); });
  };
  const settingsName = settings ? 브라우저이름(settings.browser) : name;
  const settingsFoot = !connected ? `${이가(name)} 연결되어 있을 때 바꿀 수 있습니다.`
    : !settings ? "확장이 아직 설정을 보내지 않았습니다. 잠시 뒤 다시 확인하세요."
    : `바꾸면 ${settingsName}의 확장에 바로 적용됩니다.`;

  return (
    <div className="nara-board">
      <section className="board-summary" aria-label="연결 요약">
        <div className="summary-id">
          <span className={`board-icon ${tone}`}>{브라우저그림}</span>
          <div className="summary-text">
            <div className="summary-title"><h2>{title}</h2><span className={`pill ${tone}`}>{pill}</span></div>
            <div className="soft clip num">{sub}</div>
          </div>
        </div>
        <dl className="summary-nums">
          <div><dt>확장 판</dt><dd className={stale ? "warn-strong" : ""}>{shownVersion ?? "—"}</dd></div>
          <div><dt>오늘 저장</dt><dd>{captures?.saved ?? 0}</dd></div>
          <div><dt>검토 대기</dt><dd className={waiting ? "warn-strong" : ""}>{waiting ?? "—"}</dd></div>
          <div><dt>오늘 오류</dt><dd className={errors.length ? "warn-strong" : ""}>{errors.length}</dd></div>
        </dl>
        <button className="action" onClick={() => onManage(browser)} disabled={readOnly}>{관리이름(browser)}</button>
      </section>

      <div className="board-cols">
        <div className="board-main">
          <section className="board-card" aria-label="열린 나라장터 탭">
            <div className="board-card-head"><h2>열린 나라장터 탭</h2>{tabsKnown && <span className="soft small">{tabs.length}개</span>}</div>
            {!tabsKnown
              ? <p className="soft">{readOnly ? "열람 창에서는 열린 탭을 보지 않습니다."
                : connected ? "확장이 아직 열린 탭을 보내지 않았습니다."
                : `${이가(name)} 연결되어 있지 않아 열린 탭을 알 수 없습니다.`}</p>
              : tabs.length === 0 ? <p className="soft">{name}에 열린 나라장터 탭이 없습니다.</p>
              : (
                <div role="table" aria-label="열린 나라장터 탭" className="tab-table">
                  {tabs.map((t) => {
                    const text = 탭설명(t);
                    const tone = t.shot?.status === "검토 대기" ? "warn-strong" : t.shot?.status === "저장됨" ? "ok-strong"
                      : t.shot ? "primary-strong" : "soft";
                    return (
                      <div role="row" key={t.id} className="tab-row">
                        <span role="cell" className="nowrap">{t.screen || "나라장터"}</span>
                        <span role="cell" className="num nowrap">{t.shot?.number ?? ""}</span>
                        <span role="cell" className={`clip ${tone}`}>{text}</span>
                        <span role="cell">
                          {t.shot
                            ? <button type="button" className="text-link plain" onClick={() => onShowTab(t.id)}>지금 보는 화면에서 보기</button>
                            : <button type="button" className="text-link plain" onClick={() => goTab(t)}>이 탭으로 가기</button>}
                        </span>
                      </div>
                    );
                  })}
                </div>
              )}
          </section>

          <section className="board-card" aria-label="연결 기록">
            <h2>연결 기록 · 오늘</h2>
            {log.length === 0
              ? <p className="soft">오늘은 연결 기록이 없습니다.</p>
              : (
                <ol className="log-list">
                  {log.map((e, i) => {
                    const { what, why } = 기록말(e, embedded, named);
                    return (
                      <li key={`${e.at}-${e.event}-${e.browser}-${i}`}>
                        <span className="soft num">{e.at.slice(11, 16)}</span>
                        <span className={`log-dot ${e.event}`} aria-hidden="true" />
                        <span className="clip"><b>{what}</b>{why && <> <span className="soft">{why}</span></>}</span>
                      </li>
                    );
                  })}
                </ol>
              )}
          </section>

          <section className="board-card" aria-label="오류와 재시도">
            <h2>오류와 재시도</h2>
            <p className="clip">{errorNow}</p>
            {errorPast && <p className="soft small clip num board-past">{errorPast}</p>}
          </section>

          <section className="board-card" aria-label="최근 연결">
            <h2>최근 연결</h2>
            {contacts.length === 0
              ? <p className="soft">아직 브라우저가 연결된 적이 없습니다.</p>
              : (
                <ol className="contact-list">
                  {contacts.map((c) => (
                    <li key={c.browser}>
                      <span className="soft num">{긴때(c.at)}</span>
                      <b>{브라우저이름(c.browser)}</b>
                      <span className={c.version === embedded ? "soft num" : "warn-strong num"}>
                        확장 {c.version}{c.version === embedded ? "" : " · 옛 판"}
                      </span>
                    </li>
                  ))}
                </ol>
              )}
          </section>
        </div>

        <aside className="board-side" aria-label="확장 정보">
          <section className="board-card">
            <h2>확장</h2>
            <dl className="board-dl">
              <dt>브라우저의 판</dt>
              <dd className={stale ? "warn-strong" : ""}>{connected && attached
                ? `${attached.version} · ${attached.version === embedded ? "앱과 같음" : "앱과 다름"}`
                : latest ? `${latest.version} · 마지막 연결 때` : "—"}</dd>
              <dt>앱에 포함된 판</dt><dd>{status?.embeddedVersion ?? "—"}</dd>
              {updated && <><dt>판 바뀐 때</dt><dd>{긴때(updated.at)}{updated.version === embedded ? " · 자동" : ""}</dd></>}
              <dt>확장 폴더</dt><dd title={status?.folder}>{status?.folder || "—"}</dd>
              <dt>저장 대상</dt><dd>{readOnly ? "작업자료 창의 작업자료" : `${workfileName} · ${roleName}`}</dd>
            </dl>
          </section>

          <section className="board-card" aria-label="설정">
            <h2>설정</h2>
            <fieldset className="board-settings" disabled={!settings}>
              <div>
                <div id="board-mode-label" className="soft tiny">나라장터 화면의 수집기</div>
                <div role="group" aria-labelledby="board-mode-label" className="board-seg">
                  <button type="button" aria-pressed={mode === "always"} onClick={() => setting("setPanelMode", "always")}>항상 띄우기</button>
                  <button type="button" aria-pressed={mode === "button"} onClick={() => setting("setPanelMode", "button")}>툴바 버튼으로만</button>
                </div>
              </div>
              <div className="board-setting-row">
                <div><div className="soft tiny">현재 화면 바로 저장</div><kbd>{settings ? settings.shortcut || "단축키 없음" : "—"}</kbd></div>
                <button type="button" className="action quiet" onClick={() => setting("openShortcuts")}>단축키 바꾸기…</button>
              </div>
              <div className="clip"><span className="soft tiny">사이트 접근</span> · {settings
                ? settings.siteAccess ? "www.g2b.go.kr 허용됨" : "www.g2b.go.kr 허용 안 됨" : "알 수 없음"}</div>
            </fieldset>
            <p className="soft tiny board-foot">{settingsFoot}</p>
          </section>

          <section className="board-card">
            <h2>오늘 들어온 자료</h2>
            <dl className="today-nums">
              <div><dt>접수</dt><dd>{captures?.requests ?? 0}</dd></div>
              <div><dt>공고</dt><dd>{captures?.notices ?? 0}</dd></div>
              <div><dt>계약</dt><dd>{captures?.contracts ?? 0}</dd></div>
            </dl>
            <p className="clip"><span className="soft">마지막</span>{" "}
              <span className="num">{lastEntry ? `${긴때(lastEntry.at)} · ${lastEntry.kind} ${lastEntry.number}` : "—"}</span></p>
            <button type="button" className="text-link" onClick={onShowList}>들어온 자료에서 보기</button>
          </section>
        </aside>
      </div>
    </div>
  );
}

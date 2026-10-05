import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { call, isHosted } from "./bridge";
import { ConfirmDelete } from "./ConfirmDelete";
import { Grid } from "./Grid";
import { LinkPanel } from "./LinkPanel";
import { Dock } from "./Mirror";
import { KINDS, NaraPanel, SourceTree, use들어옴, 원천상태, 브라우저이름, type Kind } from "./Nara";
import { Outline } from "./Outline";
import { SettingsPanel } from "./SettingsPanel";
import { Status } from "./Status";
import type {
  CaptureDay, Candidate, DataLocation, ExtensionStatus, DeletionPlan, DeletionScope, EntityType,
  LinkFacet, LinkWork, MergeResult, MirrorState, Outline as Tree, PlanImportResult, PlanPick, RelinkResult,
  RequestLinkFacet, RequestLinkWork, Session, Settings, Sheet, StatusReport, SubmitResult, WindowPrefs,
  Summary, SwitchResult, UserColumn, WorkfileKind, WorkfilePlan,
} from "./types";

/**
 * 통합이 맨 앞이다 — 접수·공고·계약을 한 줄에 모아 낸 것이라서다. 그다음은 <b>나라장터</b> 한 원천에서 갈라지는
 * 세 갈래(접수 → 공고 → 계약)이고, 계획은 그 앞칸이 아니라 <b>분모</b>라 갈래 뒤에 선다.
 *
 * <p>현황은 맨 뒤다. 아직 지표가 가상이라 <b>앱의 얼굴로 세우지 않는다</b> — 기본 탭은
 * 통합 그대로다.</p>
 */
type Tab = "통합" | "나라장터" | "계획" | "접수" | "공고" | "계약" | "현황";

/** 시트가 있는 탭. 현황은 표가 아니라 센 것이고, 나라장터는 자료가 들어오는 문이라 여기 들지 않는다. */
type SheetTab = Exclude<Tab, "현황" | "나라장터">;

const isKind = (t: Tab): t is Kind => (KINDS as string[]).includes(t);

/**
 * 같은 자료를 보는 눈. 쌓인 모양대로(구조), 내보낼 모양대로(표), 차수를 편 것(차수).
 *
 * <p><b>탭이 아니라 눈인 까닭.</b> 차수를 편 표는 다른 자료가 아니라 <b>같은 자료를 접지 않고
 * 본 것</b>이다 — 탭으로 세우면 같은 건이 두 탭에 나뉘어 어느 쪽이 진짜인지 묻게 된다.</p>
 *
 * <p><code>구조</code> 는 통합에만 있다. 공고·계약 탭에서는 <code>표</code> 가 곧 「최신」이라,
 * 그 둘은 <b>최신 | 차수</b> 두 알약으로 그린다.</p>
 */
type Shape = "구조" | "표" | "차수";

/**
 * 잇기 화면의 갈래. 얼개가 같아 화면은 하나를 쓰고, 다리와 낱말만 갈린다.
 *
 * <p>둘 다 오른쪽이 공고다 — 접수도 계약도 공고 하나에 매달린다.</p>
 */
type Side = "계약" | "접수";

const SIDES: Side[] = ["계약", "접수"];

/**
 * 시트 하나의 얼개.
 *
 * <p><b>키가 둘인 까닭.</b> <code>keyColumn</code> 은 <b>고칠 값이 매달리는 자리</b>다 —
 * 통합에서 손댈 수 있는 것은 계약 쪽 열뿐이라 계약번호가 그 자리다. 그런데 통합은 이제
 * 계약이 아직 없는 줄도 낸다(접수만, 접수+공고). 그런 줄은 계약번호가 비어 있어 가리킬 이름이
 * 없으므로, <code>identity</code> 가 <b>앞에서부터 차 있는 것</b>을 골라 그 줄의 이름으로 쓴다.</p>
 *
 * <p>둘을 한 열로 합치지 않는다. 합치면 공고번호로 계약 열을 고치라는 말이 되어, 덮개가
 * 공고에 걸리고 <b>어느 뷰도 읽지 않는 자리로 조용히 사라진다</b>.</p>
 */
type SheetSpec = {
  view: string;
  /**
   * 차수를 편 뷰. 없으면 그 탭에는 「차수」 눈이 없다 — 계획은 엑셀에서 온 것이라 차수가
   * 아예 없고, 접수는 아직 차수가 갈리는 문서를 받지 않는다.
   *
   * <p><b>읽기 전용이다</b>(다리의 <code>Views.ReadOnly</code>). 여기서 따로 잠그지 않는 것은
   * 다리가 이미 <code>editable</code>·<code>correctable</code> 을 빈 채로 보내기 때문이다 —
   * 화면이 한 번 더 판정하면 목록이 두 벌이 되어 갈릴 자리가 생긴다.</p>
   */
  차수뷰?: string;
  keyColumn: string;
  identity: string[];
  /**
   * 사람이 세운 열을 부르는 이름. <b>계획에는 없어 null 이다</b> — 계획은 접수·공고·계약과
   * 나란한 개체가 아니라 <code>user_column</code> 의 <code>entity_type</code> 에 자리가 없다.
   *
   * <p><code>EntityType</code> 유니온에 <code>"plan"</code> 을 더하지 않는 까닭이 그것이다.
   * 더하면 "계획에도 사람 열을 세울 수 있다" 는 말이 되는데, 담을 표가 없다.</p>
   */
  entityType: EntityType | null;
};

const SHEETS: Record<SheetTab, SheetSpec> = {
  // 통합은 줄 하나가 조달 건이다 — 접수만 온 것도, 공고까지만 온 것도 표에 선다.
  통합: {
    view: "v_통합",
    // 통합차수는 줄 하나가 공고 문서 한 장이다 — 취소된 원공고와 재공고가 나란히 서고,
    // 변경공고는 앞차수와 함께 선다. 접수·계약 열은 그 줄들에 되풀이된다.
    차수뷰: "v_통합차수",
    keyColumn: "계약번호",
    identity: ["계약번호", "입찰공고번호", "접수번호"],
    entityType: "contract",
  },
  // 계획은 분모다. 줄 하나가 조달요구번호 하나이고, 엑셀에서 와서 읽기 전용이다.
  계획: {
    view: "v_계획",
    keyColumn: "조달요구번호",
    identity: ["조달요구번호"],
    entityType: null,
  },
  접수: {
    view: "v_접수",
    keyColumn: "접수번호",
    identity: ["접수번호"],
    entityType: "request",
  },
  공고: {
    view: "v_공고",
    차수뷰: "v_공고차수",
    keyColumn: "입찰공고번호",
    identity: ["입찰공고번호"],
    entityType: "notice",
  },
  계약: {
    view: "v_계약",
    차수뷰: "v_계약차수",
    keyColumn: "계약번호",
    identity: ["계약번호"],
    entityType: "contract",
  },
};

/**
 * 차례를 여기 못 박는다 — 현황·나라장터는 시트가 없어 SHEETS 의 키에서 나오지 않는다. 방향키도 이 차례를 따른다:
 * 사이드바에 보이는 차례(통합 · 나라장터 · 접수 · 공고 · 계약 · 계획 · 현황)와 같아야 한다.
 */
const TABS: Tab[] = ["통합", "나라장터", "접수", "공고", "계약", "계획", "현황"];

const PAGE: Record<Tab, { title: string; description: string }> = {
  통합: { title: "조달 흐름", description: "조달 건마다 접수부터 계약까지 이어서 봅니다." },
  나라장터: { title: "나라장터", description: "브라우저 확장이 나라장터의 접수·공고·계약 화면을 이 작업자료로 가져옵니다." },
  계획: { title: "조달 계획", description: "계획 엑셀을 기준으로 조달요구별 진행 단계를 확인합니다." },
  접수: { title: "접수 목록", description: "접수된 조달요구와 품목, 예산을 확인하고 정리합니다." },
  공고: { title: "공고 목록", description: "공고 내용과 관련공고를 확인하고 변경 차수를 살펴봅니다." },
  계약: { title: "계약 목록", description: "계약 내용과 금액을 확인하고 진행상태·메모를 기록합니다." },
  현황: { title: "진행 현황", description: "시험용 지표입니다. 항목을 누르면 해당 계획 목록으로 이동합니다." },
};

/**
 * 거르개를 푸는 <b>한 자리</b>. 꼴은 <code>"열이름=값"</code> 하나뿐이다.
 *
 * <p>지표마다 다른 길을 내지 않는다 — 뒤의 지표 배열에 줄을 더하는 것만으로 화면이
 * 따라와야 하는데, 여기가 지표를 알기 시작하면 그 성질이 곧바로 무너진다.</p>
 */
const 거르개풀기 = (packed: string) => {
  const at = packed.indexOf("=");
  return at < 0 ? null : { 열: packed.slice(0, at), 값: packed.slice(at + 1) };
};

/**
 * 접수 쪽 응답을 잇기 화면이 아는 꼴로 옮겨 담는다.
 *
 * <p>둘은 같은 일이라 화면을 둘로 늘리지 않는다. 다리가 내는 이름만 다르므로
 * (<code>requests</code>·<code>requestKey</code>·<code>facets[].request</code>)
 * <b>여기 한 자리에서</b> 갈아 끼우고, 잇기 화면은 갈래를 알지 못한 채 그대로 산다.</p>
 */
const 계약꼴로 = (w: RequestLinkWork): LinkWork => ({
  contracts: w.requests,
  candidates: w.candidates.map((c) => ({
    contractKey: c.requestKey,
    contractTitle: c.requestTitle,
    noticeKey: c.noticeKey,
    noticeTitle: c.noticeTitle,
    confidence: c.confidence,
    reason: c.reason,
    noticeContractCount: c.noticeLinkedCount,
    titleMatched: c.titleMatched,
    blocker: c.blocker,
    facets: c.facets.map(견줌꼴로),
  })),
  notices: w.notices,
});

const 견줌꼴로 = (f: RequestLinkFacet): LinkFacet =>
  ({ name: f.name, contract: f.request, notice: f.notice, agrees: f.agrees });

export function App() {
  const [tab, setTab] = useState<Tab>("통합");
  const [shape, setShape] = useState<Shape>("구조");
  const bodyKey = `${tab}:${shape}`;
  const activeBody = useRef({ tab, shape, key: bodyKey });
  activeBody.current = { tab, shape, key: bodyKey };
  const bodyRequest = useRef(0);
  const [loadedBody, setLoadedBody] = useState("");
  const [bodyError, setBodyError] = useState<{ key: string; message: string } | null>(null);
  const [gridQuery, setGridQuery] = useState("");
  const [linkTarget, setLinkTarget] = useState<string | null>(null);
  const [side, setSide] = useState<Side>("계약");
  const activeSide = useRef(side);
  activeSide.current = side;
  const [linkError, setLinkError] = useState<string | null>(null);

  /** 공고 연결은 다른 탭과 성격이 다르다 — 쌓인 것을 보는 자리가 아니라 사람이 판정하는 자리다. */
  const [linking, setLinking] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);

  const [summary, setSummary] = useState<Summary | null>(null);
  const [sheet, setSheet] = useState<Sheet | null>(null);
  /** 현황 탭이 그릴 것. 무엇이 올지는 뒤가 정하므로 화면은 담아 두기만 한다. */
  const [statusReport, setStatusReport] = useState<StatusReport | null>(null);
  /** 계획 탭에 걸린 거르개. 현황에서 눌러 걸고, 계획 탭에서 걷는다. */
  const [filter, setFilter] = useState<{ 열: string; 값: string } | null>(null);
  const [columns, setColumns] = useState<UserColumn[]>([]);
  /** 접수·공고·계약 것을 모두. 열 고치는 자리가 셋 다 쓴다. */
  const [allColumns, setAllColumns] = useState<UserColumn[]>([]);
  const [tree, setTree] = useState<Tree | null>(null);
  const [work, setWork] = useState<LinkWork | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  /** 지우려고 세어 둔 것. 계열 전체와 차수 하나를 함께 받아 사람이 고르게 한다. */
  const [doomed, setDoomed] = useState<{ plan: DeletionPlan; one: DeletionPlan | null } | null>(null);
  const [location, setLocation] = useState<DataLocation | null>(null);
  /** 창의 몸가짐. 열람 창은 다리가 거절하므로 null 로 남고, 설정에 그 절이 서지 않는다. */
  const [windowPrefs, setWindowPrefs] = useState<WindowPrefs | null>(null);
  /**
   * 이 창이 무엇을 열고 있는가. 열람 창이면 다리가 고치는 요청을 모두 거절하므로, 화면은 그것을 미리
   * 알고 편집 자리를 잠근다 — 눌러 본 뒤에 거절을 받으면 사람은 고친 줄 알았다가 되돌려진다.
   *
   * <p>읽기 전에는 작업자료 창으로 본다. 잠그는 것은 넷째 겹일 뿐이고, 다리와 SQLite 가 이미 막는다.</p>
   */
  const [session, setSession] = useState<Session | null>(null);
  const readOnly = session?.readOnly === true;
  /** 잇기 목록을 읽은 차례. 갈래를 바꿀 때 늦게 닿는 옛 답을 가려낸다. */
  const 잇기차례 = useRef(0);
  const [saving, setSaving] = useState(false);
  /** 오늘 들어온 수집. 사이드바의 +N 과 나라장터 화면이 함께 쓴다. */
  const [captures, setCaptures] = useState<CaptureDay | null>(null);
  /** 확장이 어디까지 깔렸는지. 아직 못 읽었으면 undefined, 읽지 못했으면(개발 실행·시험 홈) null. */
  const [extension, setExtension] = useState<ExtensionStatus | null | undefined>(undefined);
  /** Chrome 의 나라장터 탭과 그 투영(지금 보는 화면, ADR-036). 붙어 있지 않으면 null. */
  const [mirror, setMirror] = useState<MirrorState | null>(null);
  /** 보드의 「지금 보는 화면에서 보기」 가 기둥에 거는 요청. */
  const [dockRequest, setDockRequest] = useState<{ id: string; n: number; follow?: boolean } | null>(null);
  /** 지금 보는 화면을 펼쳐 작업 영역을 차지하는가. 왼쪽 탐색의 어느 항목을 눌러도 좁아진다. */
  const [dockWide, setDockWide] = useState(false);
  const [toastHost, setToastHost] = useState<Element | null>(null);
  const [toast, setToast] = useState<{ text: string; bad?: boolean } | null>(null);

  useEffect(() => { setToastHost(document.querySelector("dialog[open]")); }, [settingsOpen, doomed]);

  /** 경로처럼 읽어야 하는 알림은 오래 세운다 — 2.5초에 지나가면 어디에 떨어졌는지 못 본다. */
  const say = useCallback((text: string, bad?: boolean, ms?: number) => {
    setToast({ text, bad });
    setTimeout(() => setToast(null), ms ?? (bad ? 6000 : 2500));
  }, []);

  const loadSummary = useCallback(async () => {
    setSummary(await call<Summary>("status"));
  }, []);

  /** 수집 기록은 곁의 정보라 실패해도 알리지 않는다 — 다음 확인에서 다시 읽는다. */
  const loadCaptures = useCallback(
    () => call<CaptureDay>("captures").then(setCaptures, () => {}), []);

  /**
   * 개발 실행·시험 홈에서는 다리가 거절한다. 그때는 준비 전으로 보이고, 준비를 누르면 까닭이 선다.
   * 2초마다 읽으므로 바뀐 것이 없으면 상태를 갈지 않는다 — 사이드바와 나라장터 화면이 까닭 없이 다시 그려지지 않게.
   */
  const extensionSeen = useRef<string | null>(null);
  const loadExtension = useCallback(() => call<ExtensionStatus>("extensionStatus").then(
    (next) => {
      const text = JSON.stringify(next);
      if (text !== extensionSeen.current) { extensionSeen.current = text; setExtension(next); }
    },
    () => { if (extensionSeen.current !== "null") { extensionSeen.current = "null"; setExtension(null); } }), []);

  const loadBody = useCallback(async (which = activeBody.current.tab, how = activeBody.current.shape) => {
    const key = `${which}:${how}`;
    if (key !== activeBody.current.key) return;
    const request = ++bodyRequest.current;
    const current = () => request === bodyRequest.current && key === activeBody.current.key;
    setBodyError(null);
    try {
      // 나라장터는 표가 아니다 — 그 화면이 쓰는 것(수집 기록·확장 상태)은 App 이 따로 쥔다.
      if (which === "나라장터") {
        if (current()) setLoadedBody(key);
        return;
      }

      if (which === "현황") {
        const next = await call<StatusReport>("현황");
        if (current()) { setStatusReport(next); setLoadedBody(key); }
        return;
      }

      if (which === "통합" && how === "구조") {
        const next = await call<Tree>("outline");
        if (current()) { setTree(next); setLoadedBody(key); }
        return;
      }

      const spec = SHEETS[which];

      // 「차수」 눈이 없는 탭(계획·접수)에서는 접힌 것을 그대로 낸다. 눈은 탭을 옮겨도 남으므로,
      // 여기서 물러서 주지 않으면 공고에서 차수를 펴 둔 채 접수로 가는 순간 표가 통째로 빈다.
      const view = how === "차수" && spec.차수뷰 !== undefined ? spec.차수뷰 : spec.view;

      // 계획에는 사람이 세운 열이 없다 — 부를 이름이 없으므로 다리를 부르지 않는다.
      const [data, cols] = await Promise.all([
        call<Sheet>("sheet", view),
        spec.entityType === null
          ? Promise.resolve<UserColumn[]>([])
          : call<UserColumn[]>("userColumns", spec.entityType),
      ]);
      if (current()) {
        setSheet(data);
        setColumns(cols);
        setLoadedBody(key);
      }
    } catch (error) {
      if (current()) setBodyError({ key, message: (error as Error).message });
      throw error;
    }
  }, []);

  /**
   * 잇기 목록을 읽는다.
   *
   * <p><b>늦게 닿은 옛 답을 버린다.</b> 갈래를 바꾸는 사이에 앞 갈래의 답이 돌아오면, 낱말은
   * 접수인데 목록은 계약인 화면이 선다 — 그 자리에서 「연결」을 누르면 사람은 접수를 이었다고
   * 믿고 기계는 계약을 잇는다. 차례를 세어 마지막 것만 화면에 올린다.</p>
   */
  const loadLink = useCallback(async () => {
    const 내차례 = ++잇기차례.current;
    const requestedSide = activeSide.current;
    setLinkError(null);
    try {
      const next = requestedSide === "접수"
        ? 계약꼴로(await call<RequestLinkWork>("requestLinkCandidates"))
        : await call<LinkWork>("linkCandidates");
      if (내차례 === 잇기차례.current && requestedSide === activeSide.current) setWork(next);
    } catch (error) {
      if (내차례 === 잇기차례.current && requestedSide === activeSide.current) setLinkError((error as Error).message);
      throw error;
    }
  }, []);

  useEffect(() => {
    if (!isHosted) return;
    void loadSummary().catch((e: Error) => say(e.message, true));
    void call<Settings>("settings").then(setSettings).catch((e: Error) => say(e.message, true));
    // 어느 작업자료인지는 DB 밖 홈의 config.json 에서 오므로 설정과 따로 읽는다.
    void call<DataLocation>("dataLocation").then(setLocation).catch(() => setLocation(null));
    void call<WindowPrefs>("windowPrefs").then(setWindowPrefs).catch(() => setWindowPrefs(null));
    void call<Session>("session").then(setSession).catch((e: Error) => say(e.message, true));
    void call<UserColumn[]>("userColumns").then(setAllColumns).catch((e: Error) => say(e.message, true));
    void loadCaptures();
    void loadExtension();
  }, [loadSummary, say, loadCaptures, loadExtension]);

  // 확장이 지금 붙어 있는지는 앱 어디서나 보인다(사이드바의 원천). 그래서 화면과 상관없이 2초마다 다시 읽는다 —
  // 브라우저를 켜거나 끄면, 확장을 처음 추가하면, 저절로 바뀐다.
  useEffect(() => {
    if (!isHosted) return;
    const timer = setInterval(() => void loadExtension(), 2000);
    return () => clearInterval(timer);
  }, [loadExtension]);

  useEffect(() => {
    if (!isHosted) return;
    setLoadedBody("");
    void loadBody(tab, shape).catch((e: Error) => say(e.message, true));
    return () => { bodyRequest.current++; };
  }, [tab, shape, loadBody, say]);

  useEffect(() => {
    if (!isHosted || !linking) return;
    setWork(null);
    void loadLink().catch((e: Error) => say(e.message, true));
    return () => { 잇기차례.current++; };
  }, [side, linking, loadLink, say]);

  /** 자료가 바뀐 뒤 화면 전체를 다시 읽는다 — 요약도 본문도 연결 큐도 함께 움직인다. */
  const refresh = useCallback(async () => {
    await loadSummary();
    await loadBody();
    if (linking) await loadLink();
    // 수집 기록과 확장 상태는 곁의 정보라 실패해도 던지지 않는다 — 본문 갱신의 성패를 가르지 않는다.
    await loadCaptures();
    await loadExtension();
  }, [loadSummary, loadBody, loadLink, loadCaptures, loadExtension, tab, shape, linking]);

  const dataVersion = useRef<number | null>(null);
  useEffect(() => {
    if (!isHosted) return;
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try {
        const version = await call<number>("dataVersion");
        if (stopped) return;
        if (version !== dataVersion.current) {
          await refresh();
          if (!stopped) dataVersion.current = version;
        }
      } catch (e) {
        if (!stopped) say((e as Error).message, true);
      } finally {
        // 느린 조회를 겹쳐 실행하지 않고, 실패한 갱신은 다음 확인에서 다시 시도한다.
        if (!stopped) timer = setTimeout(() => void poll(), 1000);
      }
    };
    void poll();
    return () => { stopped = true; clearTimeout(timer); };
  }, [refresh, say]);

  // 확장 수집 뒤 메인 창으로 돌아오면 같은 DB의 최신 내용을 읽는다.
  useEffect(() => {
    const onFocus = () => { void refresh().catch((e: Error) => say(e.message, true)); };
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [refresh]);

  /**
   * 쌓인 것을 엑셀 한 권으로 뽑는다. CLI 의 `pclm export` 와 같은 파일이다.
   *
   * 자리는 앱이 묻는다 — 이 파일은 **그 시점의 사진**이라 여기에 손으로 적은 것은 DB 로
   * 돌아오지 않는다. 어디에 떨어졌는지 알림에 경로를 그대로 적는 까닭도 그것이다.
   * 고르기를 그만두면 `null` 이 오고, 화면은 아무 말 없이 그대로 있는다.
   */
  const exportExcel = async () => {
    setSaving(true);
    try {
      const path = await call<string | null>("export");
      if (path !== null) say(`내보냈습니다  ${path}`, false, 8000);
    } catch (e) {
      say((e as Error).message, true);
    } finally {
      setSaving(false);
    }
  };

  /**
   * 내 자료 한 벌을 제출본 파일로 뜬다. CLI 의 `pclm submit` 과 같은 파일이다.
   *
   * 자리는 앱이 묻는다 — 메일로 보낼 파일이라 어디에 떨어졌는지 모르면 그다음 손이
   * 이어지지 않는다. 이름이 비었는지도 **앱이 판정한다**: 화면이 들고 있는 설정은 저장 전
   * 입력값과 어긋날 수 있어, 여기서 재면 적어 둔 이름을 두고도 비었다는 말이 나온다.
   */
  const submit = async () => {
    try {
      const r = await call<SubmitResult | null>("submit");
      if (r === null) return;                      // 그만두면 아무 말도 하지 않는다

      say(r.nameMissing
        ? `제출본을 만들었습니다  ${r.path} — 이름이 비어 있어 파일 이름에 넣지 못했습니다`
        : `제출본을 만들었습니다  ${r.path}`, false, 8000);
    } catch (e) {
      say((e as Error).message, true);
    }
  };

  /**
   * 받은 제출본을 모아 한 벌로 짓는다. CLI 의 `pclm merge` 와 같은 길이다.
   *
   * 알림에는 셈만 서고, 겹친 것이 무엇이었는지는 취합본 옆에 떨어진 글(txt)에 있다 —
   * 한 건씩 읽어야 하는 것이라 지나가는 알림에 실을 수 없다.
   */
  const mergeSubmissions = async () => {
    try {
      const r = await call<MergeResult | null>("merge");
      if (r === null) return;

      say(`제출본 ${r.submissions}개 · 조달요구 ${r.plans} · 접수 ${r.requests} · 공고 ${r.notices}`
        + ` · 계약 ${r.contracts}, 충돌 ${r.conflicts}건, 제외 ${r.rejected}개`
        + `  ${r.path}  ${r.reportPath}`, false, 12000);
    } catch (e) {
      say((e as Error).message, true);
    }
  };

  const edit = async (rowKey: string, field: string, value: string) => {
    try {
      const keyAtSave = bodyKey;
      await call("setField", rowKey, field, value);
      if (activeBody.current.key !== keyAtSave) return;

      // 넣은 값만 제자리에서 갈아 끼운다. 표를 통째로 다시 읽으면 스크롤과 커서가 날아가고,
      // 수백 행에서는 그때마다 화면이 멎는다.
      const key = tab === "현황" || tab === "나라장터" ? "" : SHEETS[tab].keyColumn;
      setSheet((s) =>
        s ? { ...s, rows: s.rows.map((r) => (r[key] === rowKey ? { ...r, [field]: value } : r)) } : s,
      );
    } catch (e) {
      say((e as Error).message, true);
      throw e; // 표가 제 값을 그대로 두게 — 넣히지 않은 값이 화면에 남으면 안 된다
    }
  };

  /**
   * 파서가 읽은 값을 고친다. 사람이 세운 열과 길이 갈리는 것은 담기는 표가 달라서다.
   *
   * 고친 표시가 새로 붙어야 하므로 **표를 다시 읽는다** — 값만 갈아 끼우면 고쳤다는 것이
   * 화면에 남지 않아, 무엇이 문서에서 읽은 값인지 알 수 없게 된다.
   */
  const correct = async (rowKey: string, field: string, value: string) => {
    try {
      await call("setOverride", rowKey, field, value);
      await loadBody();
    } catch (e) {
      say((e as Error).message, true);
      throw e; // 표가 제 값을 그대로 두게
    }
  };

  const revert = async (rowKey: string, field: string) => {
    try {
      await call("clearOverride", rowKey, field);
      await loadBody();
      say("원래 값으로 되돌렸습니다.");
    } catch (e) {
      say((e as Error).message, true);
      throw e;
    }
  };

  /** 세어 보이기부터. 되돌릴 수 없는 일이라 사람이 숫자를 보고 눌러야 한다. */
  const askDelete = async (rowKey: string) => {
    try {
      const plan = await call<DeletionPlan>("deletionPlan", rowKey, "series");

      setDoomed({
        plan,
        // 차수가 하나뿐이면 고를 것이 없다 — 그것을 지우는 것이 곧 계열을 지우는 것이다.
        one: plan.revisions.length > 1
          ? await call<DeletionPlan>("deletionPlan", rowKey, "seq")
          : null,
      });
    } catch (e) {
      say((e as Error).message, true);
    }
  };

  const remove = async (rowKey: string, scope: DeletionScope) => {
    try {
      await call("deleteEntity", rowKey, scope);
      setDoomed(null);
      say(`삭제했습니다  ${rowKey}`);
      // 줄이 통째로 사라지고 요약과 연결 큐도 함께 움직인다.
      await refresh();
    } catch (e) {
      say((e as Error).message, true);
    }
  };

  const confirmLink = async (c: Candidate) => {
    try {
      await call(side === "접수" ? "confirmRequestLink" : "confirmLink", c.contractKey, c.noticeKey);
      say(`연결했습니다  ${c.contractKey} ← ${c.noticeKey}`);
      await Promise.all([loadSummary(), loadLink(), loadBody()]);
    } catch (e) {
      say((e as Error).message, true);
    }
  };

  // 물리친 짝은 기록으로 남아 다시 후보에 오르지 않는다. 그래야 목록이 실제로 줄어든다.
  const rejectLink = async (c: Candidate) => {
    try {
      await call(side === "접수" ? "rejectRequestLink" : "rejectLink", c.contractKey, c.noticeKey);
      say(`연결 후보에서 제외했습니다  ${c.contractKey} ↮ ${c.noticeKey}`);
      await Promise.all([loadSummary(), loadLink()]);
    } catch (e) {
      say((e as Error).message, true);
    }
  };

  /**
   * 링크를 푼다. 물리치기와 달리 **"아니다" 라는 판정은 남기지 않는다** — 다시 가져올 때
   * 기계가 같은 짝을 다시 이을 수 있다. 통합 뷰의 공고 열이 비므로 몸통도 함께 다시 읽는다.
   */
  const unlink = async (contractKey: string) => {
    try {
      await call(side === "접수" ? "unlinkRequest" : "unlink", contractKey);
      say(`연결을 해제했습니다  ${contractKey}`);
      await Promise.all([loadSummary(), loadLink(), loadBody()]);
    } catch (e) {
      say((e as Error).message, true);
    }
  };

  /**
   * ERP 명시 참조로 다시 잇는다. 있던 연결은 풀지도 바꾸지도 않는다 — 기계가 추천만으로
   * 잇는 길은 없다(ADR-029).
   */
  const relink = async () => {
    try {
      const { linked } = await call<RelinkResult>("relinkExplicit");
      say(`명시 참조 연결 ${linked}건 · 기존 연결 유지`);
      await refresh();
    } catch (e) {
      say((e as Error).message, true);
    }
  };

  /**
   * 추천에 없는 짝을 나란히 견준다. 실패를 삼키면 화면이 빈 표를 그대로 세운다.
   *
   * <p><b>참조가 흔들리면 안 된다.</b> 잇기 화면이 이것을 효과의 의존성으로 물고 있어,
   * 그릴 때마다 새 함수가 되면 토스트가 뜨고 지는 것만으로도 다리를 헛되이 다시 부른다.</p>
   */
  const compareLink = useCallback(
    async (contractKey: string, noticeKey: string) => {
      try {
        if (side === "접수") {
          const facets = await call<RequestLinkFacet[]>("compareRequestLink", contractKey, noticeKey);
          return facets.map(견줌꼴로);
        }

        return await call<LinkFacet[]>("compareLink", contractKey, noticeKey);
      } catch (e) {
        say((e as Error).message, true);
        throw e;
      }
    },
    [say, side],
  );

  /** 계획 엑셀을 고른다. 고른 자리는 다리가 설정에 적어 두고, 여기엔 이름과 경로가 온다. */
  const pickPlanExcel = async () => {
    try {
      return await call<PlanPick | null>("pickPlanExcel");
    } catch (e) {
      say((e as Error).message, true);
      return null;
    }
  };

  /**
   * 골라 둔 계획 엑셀을 넣는다. 결과는 설정 창이 그 자리에서 적는다 — 없는 필수 열이
   * 있으면 한 줄도 들어가지 않았다는 것을 사람이 봐야 한다.
   */
  const importPlan = async () => {
    try {
      const result = await call<PlanImportResult>("importPlan");
      // 계획이 늘었으면 계획 탭도 함께 움직인다.
      if (result.ok) await loadBody();
      return result;
    } catch (e) {
      say((e as Error).message, true);
      throw e;
    }
  };

  /**
   * 열을 고치는 일은 모두 이 길로 지난다.
   *
   * 열이 바뀌면 계약면 뷰가 다시 지어지므로 <b>표를 다시 읽어야 한다</b> — 다시 읽지 않으면
   * 화면이 옛 열을 붙들고 있어, 방금 세운 열이 설정 창에만 있고 표에는 없는 것처럼 보인다.
   */
  const columnAction = async (work: () => Promise<UserColumn[]>) => {
    try {
      setAllColumns(await work());
      await loadBody();
    } catch (e) {
      say((e as Error).message, true);
    }
  };

  /** 화면은 쉼표로 받고 다리는 세로줄로 받는다 — 저장 형식이 세로줄이라 그쪽에 맞춘다. */
  const packChoices = (choices: string) =>
    choices.split(",").map((c) => c.trim()).filter((c) => c.length > 0).join("|");

  const addColumn = (entityType: string, name: string, kind: string, choices: string) =>
    columnAction(() => call<UserColumn[]>("addColumn", entityType, name, kind, packChoices(choices)));

  const updateColumn = (
    entityType: string, fieldName: string, newName: string, kind: string, choices: string,
  ) =>
    columnAction(() =>
      call<UserColumn[]>("updateColumn", entityType, fieldName, newName, kind, packChoices(choices)));

  const removeColumn = (entityType: string, fieldName: string) =>
    columnAction(() => call<UserColumn[]>("removeColumn", entityType, fieldName));

  const moveColumn = (entityType: string, fieldName: string, delta: number) =>
    columnAction(() => call<UserColumn[]>("moveColumn", entityType, fieldName, String(delta)));

  /**
   * 다른 자료를 <b>새 창으로</b> 열어 본다. 이 창은 그대로다 — 열람은 별도 프로세스라 이 창의 자료에
   * 닿을 길이 없다(ADR-031). 고르기를 그만두면 null 이 오고 아무 말도 하지 않는다.
   */
  const openOther = async () => {
    try {
      const r = await call<{ path: string } | null>("openOther");
      if (r !== null) say(`새 창에서 열어 봅니다  ${r.path}`, false, 6000);
    } catch (e) {
      say((e as Error).message, true);
    }
  };

  /**
   * 무엇으로 바꿀지 고르게 한다. 다리는 아무것도 바꾸지 않고 고른 것과 그 뜻을 돌려준다 — 설정이 그것을 보이고
   * 확인을 받는다. 쓸 수 없는 자리면 여기서 까닭을 알리고 확인 창을 띄우지 않는다.
   */
  const pickWorkfile = async (kind: WorkfileKind) => {
    try {
      return await call<WorkfilePlan | null>("pickWorkfile", kind);
    } catch (e) {
      say((e as Error).message, true);
      return null;
    }
  };

  /**
   * 확인받은 대로 바꾼다. 다리는 답을 보낸 뒤 창을 다시 띄우므로 이 알림은 잠깐 보이고 사라진다 — 그래도 남기는
   * 까닭은 창이 다시 뜨기 전 그 사이에 사람이 무엇이 일어났는지 알게 하려는 것이다.
   */
  const switchWorkfile = async (plan: WorkfilePlan) => {
    const method = { move: "moveWorkfile", use: "switchWorkfile", snapshot: "switchWorkfile", new: "newWorkfile" }[plan.action];
    const args = plan.action === "snapshot" ? [plan.source, plan.path] : [plan.path];
    try {
      const r = await call<SwitchResult>(method, ...args);
      say(`작업자료를 바꿨습니다 — 창을 다시 띄웁니다  ${r.path}`, false, 8000);
    } catch (e) {
      say((e as Error).message, true);
    }
  };

  /** 지금 시점을 백업으로 뜬다. 어디에 떨어졌는지 알림에 경로를 그대로 적는다. 그만두면 아무 말도 하지 않는다. */
  const backupWorkfile = async () => {
    try {
      const r = await call<{ path: string } | null>("backupWorkfile");
      if (r !== null) say(`백업을 만들었습니다  ${r.path}`, false, 8000);
    } catch (e) {
      say((e as Error).message, true);
    }
  };

  /** 창의 몸가짐은 누르는 그 자리에서 선다 — 저장 단추를 타지 않는다. 실패하면 화면의 칸도 되돌린다. */
  const saveWindowPrefs = async (closeToTray: boolean, autostart: boolean) => {
    try {
      setWindowPrefs(await call<WindowPrefs>("saveWindowPrefs", String(closeToTray), String(autostart)));
    } catch (e) {
      say((e as Error).message, true);
      void call<WindowPrefs>("windowPrefs").then(setWindowPrefs).catch(() => {});
    }
  };

  const saveSettings = async (submitterName: string) => {
    try {
      setSettings(await call<Settings>("saveSettings", submitterName));
      say("설정을 저장했습니다.");
      setSettingsOpen(false);
    } catch (e) {
      say((e as Error).message, true);
    }
  };

  /**
   * 현황에서 올라온 거르개를 걸고 계획 탭으로 간다.
   *
   * <p><b>푸는 자리는 여기 하나다.</b> 지표마다 다른 길을 내면 뒤에 지표를 더하는 일이
   * 화면을 고치는 일이 되고, 갈아끼울 수 있게 하려던 것이 그대로 무너진다.</p>
   */
  const 걸기 = (packed: string) => {
    const parsed = 거르개풀기(packed);
    if (!parsed) return;

    setFilter(parsed);
    setLinking(false);
    setTab("계획");
  };

  /**
   * 계획 탭에 걸린 거르개를 여기서 씌운다. <b>Grid 는 건드리지 않는다</b> — 표는 받은 줄을
   * 그리는 것이 전부이고, 무엇을 보일지는 바깥이 정한다.
   */
  const 거른시트 =
    tab === "계획" && filter && sheet
      ? { ...sheet, rows: sheet.rows.filter((r) => (r[filter.열] ?? "") === filter.값) }
      : sheet;

  /**
   * 열람 중이면 고칠 수 있는 칸이 없다. 다리가 이미 빈 채로 보내지만 여기서도 비운다 — 다리가 답하기 전의
   * 낡은 표나 가짜 다리가 칸을 열어 두어도, 넣는 순간 거절당할 칸을 고칠 수 있는 것처럼 그리지 않는다.
   */
  const 보이는시트 = readOnly && 거른시트
    ? { ...거른시트, editable: [], correctable: [], overrides: {} }
    : 거른시트;

  /** 지금 보고 있는 갈래. 그 갈래로 들어온 것은 이미 본 것이라 +N 에 세지 않는다. */
  const arrivals = use들어옴(captures, !linking && isKind(tab) ? tab : null);
  const source = 원천상태(extension, readOnly);

  /**
   * 지금 보는 화면 — 확장이 붙어 있는 동안 1초마다 다시 읽는다(dataVersion 과 같은 결). 바뀐 것이 없으면 다시 그리지
   * 않는다. 열람 창은 읽지 않는다: 비출 작업자료가 아니다.
   */
  const mirrorOn = isHosted && !readOnly && (source.kind === "live" || source.kind === "stale");
  const mirrorSeen = useRef<string | null>(null);
  useEffect(() => {
    if (!mirrorOn) { mirrorSeen.current = null; setMirror(null); return; }
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try {
        const next = await call<MirrorState>("mirror");
        if (stopped) return;
        const text = JSON.stringify(next);
        if (text !== mirrorSeen.current) { mirrorSeen.current = text; setMirror(next); }
      } catch { /* 곁의 정보라 알리지 않는다 — 다음 확인에서 다시 읽는다. */ }
      finally { if (!stopped) timer = setTimeout(() => void poll(), 1000); }
    };
    void poll();
    return () => { stopped = true; clearTimeout(timer); };
  }, [mirrorOn]);

  /** 탭을 고른다. 검색과 공고 연결은 탭을 따라오지 않는다. 펼친 지금 보는 화면은 좁아진다. */
  const pick = (t: Tab) => { setGridQuery(""); setLinking(false); setTab(t); setDockWide(false); };

  /** 펼침은 붙어 있을 때만 — 비출 것이 없으면 작업 영역을 돌려준다. */
  const wideOn = dockWide && mirrorOn;

  /**
   * 사이드바 갈래의 눈 — Chrome 의 앞 탭이 수집할 수 있는 화면이면 그 종류의 갈래 옆에 선다. 누르면 따라가기를 켜고
   * 앞 탭을 비춘다.
   */
  const frontTab = mirrorOn ? mirror?.tabs.find((t) => t.id === mirror.front && t.shot) ?? null : null;
  const seeing = frontTab?.shot ? {
    kind: frontTab.shot.kind,
    label: `${브라우저이름(frontTab.browser)}에서 보고 있는 ${frontTab.shot.kind} ${frontTab.shot.number} — 지금 보는 화면에서 보기`,
  } : null;
  const see = () => { if (frontTab) setDockRequest((r) => ({ id: frontTab.id, n: (r?.n ?? 0) + 1, follow: true })); };

  /** 들어온 수집의 「보기」 — 그 종류 탭으로 옮겨 표 검색을 그 번호로 건다. */
  const openCapture = (kind: Kind, number: string) => { setGridQuery(number); setLinking(false); setTab(kind); };

  const plainTab = (t: Tab) => (
    <button key={t} id={`tab-${t}`} role="tab" className="tab"
      aria-selected={!linking && tab === t} aria-controls="workspace"
      tabIndex={tab === t ? 0 : -1}
      onClick={() => pick(t)}>
      {t}
    </button>
  );

  if (!isHosted) return <div className="empty-state">이 화면은 앱 안에서 씁니다.</div>;

  // 배지는 둘의 합이다. 갈래마다 따로 세우면 접수 큐가 갈래를 열어 보기 전에는 안 보인다.
  const unlinked = (summary?.unlinkedContracts ?? 0) + (summary?.unlinkedRequests ?? 0);
  const page = linking
    ? { title: "공고 연결", description: "접수·계약과 공고를 비교하고, 같은 조달 건인지 확인해 연결합니다." }
    : PAGE[tab];

  return (
    <div className={`app${wideOn ? " dock-wide" : ""}`}>
      {/* 셋째 기둥은 DOM 의 끝에 있다. 키보드로는 목록 전체를 지나야 닿으므로 맨 앞에 건너뛰는 길을 둔다. */}
      <a className="skip-link" href="#dock-title" onClick={(e) => {
        const title = document.getElementById("dock-title");
        if (!title) return;
        e.preventDefault();
        title.setAttribute("tabindex", "-1");
        title.focus();
      }}>지금 보는 화면으로 건너뛰기</a>
      <aside className="sidebar" aria-label="작업 탐색">
        <div className="brand">
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 3h10l4 4v14H5zM14 3v5h5M8 12h8M8 16h5" /></svg>
          <strong>계약 목록</strong>
        </div>
        <p className="nav-label">자료 탐색</p>
        {/* 나라장터 한 원천에서 접수·공고·계약 세 갈래로. 방향키는 보이는 차례(TABS)를 그대로 따른다. */}
        <nav className="tabs" role="tablist" aria-label="자료 종류" aria-orientation="vertical"
          onKeyDown={(e) => {
            const index = TABS.indexOf(tab);
            const next = e.key === "ArrowDown" || e.key === "ArrowRight" ? (index + 1) % TABS.length
              : e.key === "ArrowUp" || e.key === "ArrowLeft" ? (index + TABS.length - 1) % TABS.length
              : e.key === "Home" ? 0 : e.key === "End" ? TABS.length - 1 : -1;
            if (next < 0) return;
            e.preventDefault();
            pick(TABS[next]);
            document.getElementById(`tab-${TABS[next]}`)?.focus();
          }}>
          {plainTab("통합")}
          <SourceTree
            selected={linking ? null : tab}
            focusable={tab}
            onSelect={pick}
            source={source}
            totals={summary ? { 접수: summary.requestBases, 공고: summary.noticeBases, 계약: summary.contractBases } : null}
            arrivals={arrivals}
            seeing={seeing}
            onSee={see}
          />
          {plainTab("계획")}
          {plainTab("현황")}
        </nav>
        <p className="nav-label">자료 검토</p>
        <button className="nav-link" aria-pressed={linking} onClick={() => { setLinkTarget(null); setLinking((v) => !v); setDockWide(false); }}>
          공고 연결
          {unlinked > 0 && <span className="badge" aria-label={`연결되지 않은 계약·접수 ${unlinked}건`}>{unlinked}</span>}
        </button>
        <div className="sidebar-bottom">
          <button className="action quiet" onClick={() => { setDockWide(false); setSettingsOpen(true); }}>설정</button>
        </div>
      </aside>
      <main className="workspace" id="workspace" aria-label={page.title} hidden={wideOn}>
      {/* 열람 창은 그것을 내내 말한다. 창 제목만으로는 어느 창이 내 작업자료인지 헷갈린다. */}
      {readOnly && session && (
        <div className="readonly-banner" role="status">
          <b>열람 중</b> · {session.roleName} · <span className="path">{session.path}</span>
          <span> — 읽기 전용이라 이 창에서는 고칠 수 없습니다.</span>
        </div>
      )}
      {tab === "나라장터" && !linking ? (
        <NaraPanel
          readOnly={readOnly}
          status={extension}
          source={source}
          captures={captures}
          arrivals={arrivals}
          workfileName={(location?.path ?? session?.path ?? "").split(/[\\/]/).pop() ?? ""}
          roleName={session?.roleName ?? "작업자료"}
          onStatusReload={loadExtension}
          onOpen={openCapture}
          onErpChanged={refresh}
          onError={(message) => say(message, true)}
          mirror={mirror}
          onShowTab={(id) => setDockRequest((r) => ({ id, n: (r?.n ?? 0) + 1 }))}
        />
      ) : <>
      <header className="top">
        <div className="page-heading"><h1>{page.title}</h1><p>{page.description}</p></div>
        {/* 자료는 확장·ERP JSON 으로 들어온다(ADR-028). 여기는 내보내는 자리뿐이다. */}
        <div className="top-group">
          <button className="action" onClick={() => void exportExcel()} disabled={saving}>
            {saving ? "내보내는 중…" : "엑셀로 내보내기"}
          </button>
        </div>

      </header>
      <div className="view-bar">
        {summary && (
          <div className="stat" aria-label="전체 자료 수">
            <span>접수 <b>{summary.requestBases}</b></span>
            <span>공고 <b>{summary.noticeBases}</b></span>
            <span>계약 <b>{summary.contractBases}</b></span>
          </div>
        )}

        {/* 무엇으로 걸렸는지 보이고, 걷는 길이 그 옆에 있다. */}
        {tab === "계획" && filter && !linking && (
          <div className="shape" role="group" aria-label="필터">
            <span className="filter-tag">{filter.열} = {filter.값}</span>
            <button className="chip" onClick={() => setFilter(null)}>필터 해제</button>
          </div>
        )}

        {tab === "통합" && !linking && (
          <div className="shape" role="group" aria-label="통합 보기 방식">
            {(["구조", "표", "차수"] as Shape[]).map((s) => (
              <button
                key={s}
                className="chip"
                aria-pressed={shape === s}
                onClick={() => setShape(s)}
              >
                {s === "차수" ? "전체 차수" : s}
              </button>
            ))}
          </div>
        )}

        {/*
          공고·계약에는 「구조」가 없어 표가 곧 최신이다. 그래서 알약은 둘이지만 고르는 것은
          통합과 <b>같은 눈</b>이라, 한쪽에서 펴 두면 탭을 옮겨도 펴진 채로 따라온다.

          「최신」을 눌러도 펴져 있지 않았으면 아무것도 바꾸지 않는다 — 「표」로 고쳐 버리면
          통합으로 돌아갔을 때 보고 있던 「구조」가 눌러 본 적 없이 사라진다.
        */}
        {(tab === "공고" || tab === "계약") && !linking && (
          <div className="shape" role="group" aria-label={`${tab} 보기 방식`}>
            <button
              className="chip"
              aria-pressed={shape !== "차수"}
              onClick={() => { if (shape === "차수") setShape("표"); }}
            >
              최신
            </button>
            <button
              className="chip"
              aria-pressed={shape === "차수"}
              onClick={() => setShape("차수")}
            >
              전체 차수
            </button>
          </div>
        )}

        {/* 잇기의 두 갈래. 같은 눈으로 다른 짝을 보는 자리라 탭이 아니라 낮은 위계의 알약이다. */}
        {linking && (
          <>
            <div className="shape" role="group" aria-label="연결 갈래">
              {SIDES.map((s) => (
                <button
                  key={s}
                  className="chip"
                  aria-pressed={side === s}
                  onClick={() => { if (side !== s) { setLinkTarget(null); setWork(null); setSide(s); } }}
                >
                  {s} ↔ 공고
                </button>
              ))}
            </div>

            {/* 갈래를 고르는 것이 아니라 한 번 돌리는 일이라 그 묶음 밖에 선다. 잇는 일이라 열람 중에는 없다. */}
            {!readOnly && <button className="chip" onClick={() => void relink()}>명시 참조로 다시 잇기</button>}
          </>
        )}
        {tab === "계획" && !linking && <button className="action quiet plan-setup" onClick={() => setSettingsOpen(true)}>계획 엑셀 가져오기 설정</button>}
      </div>

      {linking ? (
        linkError ? <div className="empty-state" role="alert"><h2>연결 자료를 읽지 못했습니다</h2><p>{linkError}</p>
          <button className="action" onClick={() => void loadLink().catch(() => {})}>다시 읽기</button></div> : work ? (
          <LinkPanel
            key={`${side}:${linkTarget ?? ""}`}
            initialKey={linkTarget}
            work={work}
            leftLabel={side}
            readOnly={readOnly}
            onConfirm={confirmLink}
            onReject={rejectLink}
            onUnlink={unlink}
            onCompare={compareLink}
          />
        ) : <div className="empty-state">읽는 중…</div>
      ) : bodyError?.key === bodyKey ? (
        <div className="empty-state" role="alert"><h2>자료를 읽지 못했습니다</h2><p>{bodyError.message}</p>
          <button className="action" onClick={() => void loadBody().catch(() => {})}>다시 읽기</button>
        </div>
      ) : loadedBody !== bodyKey ? <div className="empty-state" role="status">읽는 중…</div> : tab === "현황" ? (
        statusReport
          ? <Status report={statusReport} onFilter={걸기} />
          : <div className="empty-state">읽는 중…</div>
      ) : tab === "통합" && shape === "구조" ? (
        tree ? <Outline tree={tree} onSetup={() => pick("나라장터")}
          onOpen={(kind, number, revisions) => { setGridQuery(number); setTab(kind); setShape(revisions ? "차수" : "표"); }}
          onLink={(kind, key) => { setLinkTarget(key); setWork(null); setSide(kind); setLinking(true); }} /> : <div className="empty-state">읽는 중…</div>
      ) : 보이는시트?.rows.length === 0 ? (
        <div className="empty-state">
          <h2>{tab === "계획" && filter ? "이 조건에 맞는 계획이 없습니다" : `아직 ${tab} 자료가 없습니다`}</h2>
          <p>{tab === "계획"
            ? "계획 엑셀을 가져오면 조달요구별 진행 단계를 확인할 수 있습니다."
            : "왼쪽 메뉴의 나라장터에서 브라우저 확장을 설치하거나 JSON 파일을 가져오세요."}</p>
          {tab === "계획" && filter
            ? <button className="action" onClick={() => setFilter(null)}>조건 지우기</button>
            : tab === "계획"
              ? <button className="action primary" onClick={() => setSettingsOpen(true)}>계획 엑셀 설정</button>
              : <button className="action primary" onClick={() => pick("나라장터")}>나라장터 열기</button>}
        </div>
      ) : 보이는시트 && tab !== "나라장터" ? (
        <Grid
          key={`${bodyKey}:${gridQuery}`}
          initialQuery={gridQuery}
          sheet={보이는시트}
          columns={columns}
          keyColumn={SHEETS[tab].keyColumn}
          identityColumns={SHEETS[tab].identity}
          onEdit={edit}
          onCorrect={correct}
          onRevert={revert}
          // 계획 행은 지우는 개체가 아니다 — 손잡이를 넘기지 않으면 단추가 서지 않는다. 열람 중에도 그렇다.
          onDelete={tab === "계획" || readOnly ? undefined : askDelete}
        />
      ) : (
        <div className="empty-state">읽는 중…</div>
      )}
      </>}
      <footer className="workspace-footer">
        <span>{import.meta.env.DEV && !window.chrome?.webview ? "미리보기 · 예시 자료" : "계약 목록"}</span>
        <span className="footer-shortcut">Ctrl+F 검색</span>
      </footer>
      </main>

      <Dock source={source} readOnly={readOnly} mirror={mirror} request={dockRequest} wide={wideOn} onWide={setDockWide}
        onShowList={() => pick("나라장터")}
        onImported={() => void refresh().catch(() => {})}
        onError={(message) => say(message, true)} />

      {settingsOpen && settings && (
        <SettingsPanel
          settings={settings}
          columns={allColumns}
          onAddColumn={addColumn}
          onUpdateColumn={updateColumn}
          onRemoveColumn={removeColumn}
          onMoveColumn={moveColumn}
          onOpenNara={() => { setSettingsOpen(false); pick("나라장터"); }}
          onPickPlanExcel={pickPlanExcel}
          onImportPlan={importPlan}
          onSave={saveSettings}
          onSubmit={submit}
          onMerge={mergeSubmissions}
          location={location}
          readOnly={readOnly}
          windowPrefs={windowPrefs}
          onSaveWindowPrefs={saveWindowPrefs}
          onOpenOther={() => void openOther()}
          onPickWorkfile={pickWorkfile}
          onSwitchWorkfile={switchWorkfile}
          onBackup={backupWorkfile}
          onRevealData={() => void call("revealDataFolder").catch((e: Error) => say(e.message, true))}
          onClose={() => setSettingsOpen(false)}
        />
      )}

      {doomed && (
        <ConfirmDelete
          plan={doomed.plan}
          one={doomed.one}
          onDelete={(scope) => remove(doomed.plan.display, scope)}
          onClose={() => setDoomed(null)}
        />
      )}

      {toast && createPortal(<div role={toast.bad ? "alert" : "status"} className={`toast${toast.bad ? " bad" : ""}`}>{toast.text}</div>, toastHost ?? document.body)}
    </div>
  );
}

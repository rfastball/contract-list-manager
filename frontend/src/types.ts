/** 뷰 한 줄. 값은 모두 문서에 그대로 찍힐 문자열이다(규약 3절). */
export type Row = Record<string, string>;

export type Sheet = {
  name: string;
  columns: string[];
  /** 사람이 세운 열(진행상태·메모). 값은 user_field 에 담긴다. */
  editable: string[];
  /**
   * 파서가 읽은 값을 손으로 고칠 수 있는 열. 값은 덮개로 얹히고 파서 표는 건드리지 않는다.
   *
   * 통합 시트에서는 계약 쪽 열만 온다 — 공고 열은 이어진 다른 레코드의 것이라
   * 이 줄의 키로 주소가 잡히지 않는다. 공고 탭에서 고치면 여기에도 그대로 비친다.
   */
  correctable: string[];
  /** 이미 고쳐 둔 칸. 줄 키 → 열 이름 → **고치기 전에 그 자리에 있던 값**. */
  overrides: Record<string, Record<string, string>>;
  rows: Row[];
};

/** 개체 셋. 흐름은 접수 → 공고 → 계약이고 보통 1:1:1 이다. */
export type EntityType = "contract" | "notice" | "request";

export type UserColumn = {
  entityType: EntityType;
  fieldName: string;
  kind: "text" | "choice" | "date" | "number";
  choices: string[];
};

export type Summary = {
  requestRows: number;
  requestBases: number;
  noticeRows: number;
  noticeBases: number;
  contractRows: number;
  contractBases: number;
  counterparties: number;
  unlinkedContracts: number;
  /** 공고에 이어지지 않은 접수. 머리의 배지가 계약 것과 합쳐 낸다. */
  unlinkedRequests: number;
};

/** 판과 라이선스. 고지 둘은 exe 안에 박힌 전문이다. */
export type About = { version: string; license: string; notices: string };

/**
 * 창의 몸가짐. 자료가 아니라 이 컴퓨터의 것이라 작업자료 밖(홈의 window.json)에 적힌다 — 작업자료를 바꾸거나
 * 제출본을 보내도 따라가지 않는다. 열람 창은 다루지 않는다.
 */
export type WindowPrefs = {
  /** 닫기(X)를 눌러도 끝내지 않고 알림 영역에 둔다. */
  closeToTray: boolean;
  /** Windows 에 로그인하면 켠다. 참은 레지스트리 하나다 — 작업 관리자에서 끈 것도 꺼짐으로 온다. */
  autostart: boolean;
  /** 자동 실행을 바꿀 수 있는 자리인가. 시험 홈(--home)·압축 파일 안에서 켠 창은 아니다. */
  autostartAvailable: boolean;
  /** 바꿀 수 없는 까닭. 바꿀 수 있으면 null. 다리가 짓는다. */
  autostartReason: string | null;
};

/**
 * 지금 연 작업자료와 그것을 가리키는 홈. <b>DB 안이 아니라 밖(홈의 config.json)에 적힌다</b> — 어느 파일을
 * 열지를 파일 안에 두면 그 값을 읽으려고 파일을 먼저 열어야 한다. 그래서 Settings 와 따로 온다.
 */
/**
 * 이 창이 무엇을 열고 있는가. <code>readOnly</code> 면 남의 것·제출본·취합본을 <b>열어 본</b> 창이라
 * 다리가 고치는 요청을 모두 거절한다 — 화면은 그것을 미리 알고 편집 자리를 잠근다.
 */
export type Session = {
  readOnly: boolean;
  /** 파일의 역할. <code>work</code>·<code>submission</code>·<code>merged</code>·<code>backup</code>·<code>retired</code>. */
  role: string;
  /** 사람에게 보일 역할 이름. 다리가 짓는다 — 화면이 따로 옮기면 두 벌이 된다. */
  roleName: string;
  /** 보고 있는 파일. 열람이면 원본이다(임시 사본이 아니다). */
  path: string;
};

export type DataLocation = {
  /** 작업자료(.pclm) 의 전체 경로. */
  path: string;
  folder: string;
  sizeBytes: number;
  /** 홈 폴더. 쪽지·백업·확장 연결이 여기 있다. */
  home: string;
  /** 작업자료를 가리키는 쪽지. 밖에서 읽는 쪽도 이것을 본다. */
  configPath: string;
};

/** 무엇으로 바꿀지 고르는 갈래. 바꾸기는 고른 파일의 역할에 따라 그 자리를 쓰거나(use) 사본을 뜬다(snapshot). */
export type WorkfileKind = "move" | "switch" | "new";

/**
 * 고른 것과 그 뜻(<code>pickWorkfile</code>). <b>아직 아무것도 바뀌지 않았다</b> — 화면이 이것을 보이고 확인을 받은
 * 뒤에야 바꾸기를 부른다.
 */
export type WorkfilePlan = {
  /** <code>move</code> 옮기기 · <code>use</code> 다른 작업자료 쓰기 · <code>snapshot</code> 사본에서 새로 · <code>new</code> 새 계약자료. */
  action: "move" | "use" | "snapshot" | "new";
  /** 바꾼 뒤 작업자료가 될 파일. */
  path: string;
  /** 옮기기면 지금 자리, 사본에서 새로면 원본. 그 밖에는 null. */
  source: string | null;
  /** 사본에서 새로일 때 원본의 역할 이름(제출본·취합본·백업…). */
  sourceRoleName: string | null;
  /** 지금 작업자료. */
  current: string;
};

/** 바꾸기의 답. 다리는 이 답을 보낸 뒤 창을 다시 띄운다. */
export type SwitchResult = { restart: boolean; path: string };

/**
 * 확장이 어디까지 깔렸는지. 앱은 브라우저를 볼 수 없어, 확장이 인사할 때 남긴 흔적으로 안다.
 * `at` 은 ISO 8601 로컬 시각.
 */
export type ExtensionStatus = {
  prepared: boolean;
  embeddedVersion: string;
  diskVersion: string | null;
  contacts: ExtensionContact[];
  /** 확장을 풀어 둘 자리. 아직 준비하지 않았어도 온다 — 브라우저에서 사람이 고르는 폴더다. */
  folder: string;
  /**
   * 지금 붙어 있는 브라우저(상시 연결, ADR-035). 확장이 쥔 포트로 뜬 호스트가 살아 있는 것만 온다.
   * `connectedAt` 은 포트가 열린 때(로컬 ISO).
   */
  live: ExtensionLive[];
  /** 오늘의 연결 기록(붙음·끊김·판 바뀜), 일어난 차례대로. 끊긴 까닭은 없다 — 앱도 호스트도 모른다. */
  log: ExtensionLogEntry[];
  /** 오늘 확장의 수집 흐름이 알린 오류, 일어난 차례대로. 확장은 코드만 보낸다. */
  errors: ExtensionProblem[];
  /** 오늘 전의 마지막 오류(기록이 남는 이레 안). */
  pastError: ExtensionProblem | null;
};

/** 오류 하나. <code>recovered</code> 는 그 뒤 같은 요청을 다시 보내 풀었는가. */
export type ExtensionProblem = { at: string; browser: string; code: string; recovered: boolean };

/** 확장이 호스트에 인사할 때 남긴 흔적. 브라우저마다 마지막 하나만 남는다. */
export type ExtensionContact = { browser: string; version: string; at: string };

export type ExtensionLive = { browser: string; version: string; connectedAt: string };

/** 붙음·끊김·판 바뀜. 판 바뀜이면 `previous` 가 그 앞의 판이고, 아니면 빈 문자열. */
export type ExtensionLogEntry = {
  at: string;
  event: "connected" | "disconnected" | "updated";
  browser: string;
  version: string;
  previous: string;
};

// ── 지금 보는 화면(ADR-036) ─────────────────────────────
// 확장이 보고한 나라장터 탭과, 수집할 수 있는 탭마다 작업자료에 비춘 것(mirror). 값은 계약면에 찍힐 표기 그대로다.

/** 칸 하나. <code>override</code> 는 사람이 고친 칸의 아래 값이 이번 화면으로 바뀌는 것이다. */
export type MirrorField = {
  column: string;
  kind: "id" | "same" | "changed" | "new" | "override";
  /** 계약면 표기. 덮개 칸이면 화면의 값(덮개 아래). */
  value: string;
  /** 견준 차수의 표기 — <code>changed</code> 에만. */
  old: string | null;
  /** 변환 전 화면의 원값이 표기와 다르면(날짜·금액) 그것. */
  raw: string | null;
  /** 사람이 고친 값 — <code>override</code> 에만. */
  human: string | null;
  /** 가져올 때 「수집 원값으로 복원」 을 고르는 자리 — <code>override</code> 에만. */
  choiceId: string | null;
  /** 이 열을 채우는 화면의 자리(수집 규칙의 source 키, 표면 <code>table:대상</code>). 열 정의에서 끌어낸 것이라 없을 수 있다. */
  sources: string[];
  /** 그 자리에 화면이 적어 둔 이름표. */
  from: string;
};

/** 확장이 읽어 보낸 화면의 이름표와 보이는 글 한 줄(「화면 그대로」). */
export type ScreenRow = {
  /** 가장 가까운 앞의 구역 제목. 없으면 빈 글. */
  group: string;
  label: string;
  text: string;
  /** 수집 규칙의 source 키. 품목 줄은 <code>table:대상:n</code>, 수집 규칙에 없는 화면은 빈 글. */
  source: string;
};

/** 확장의 설정 — 「Chrome 확장 상태」 의 설정이 보이고 바꾼다. 앞 탭의 브라우저(없으면 처음 붙은 것)의 것이다. */
export type ExtensionSettings = {
  pid: number;
  browser: string;
  panelMode: "always" | "button";
  /** 현재 화면 바로 저장의 단축키. 없으면 빈 글. */
  shortcut: string;
  siteAccess: boolean;
};

export type MirrorItem = {
  line: string; name: string; spec: string; quantity: string; unit: string; price: string; amount: string;
  /** 견준 차수의 같은 순번 — 수량·단가·금액이 다를 때만. */
  before: { quantity: string; unit: string; price: string; amount: string } | null;
  /** 견준 차수에 없던 순번. */
  added: boolean;
  /** 화면의 품목 줄(<code>table:대상:n</code>). 이번 화면에 없던 줄이면 빈 글. */
  source: string;
};

/** 접수 → 공고 → 계약 의 한 칸. 지금 저장된 연결로만 읽는다. */
export type MirrorPlace = {
  kind: "접수" | "공고" | "계약";
  state: "here" | "linked" | "ref" | "missing";
  /** 번호-차수. <code>ref</code> 면 참조한 본번호. */
  number: string;
  /** <code>ref</code> 일 때 그 본번호가 작업자료에 있는가(있는데 이어지지 않았다). */
  collected: boolean;
  /** 이 칸을 잇는 화면의 자리 — 그 종류를 참조하는 번호 칸. */
  source: string | null;
};

export type MirrorRound = { seq: string; amount: string; savedOn: string; state: "stored" | "current" | "ghost" };

export type MirrorStatus = "새 자료" | "새 차수" | "검토 대기" | "바뀐 칸" | "저장됨";

export type MirrorShot = {
  entityType: EntityType;
  kind: "접수" | "공고" | "계약";
  number: string;
  base: string;
  seq: string;
  title: string;
  status: MirrorStatus;
  /** 견준 차수와 다른 칸 수 + 다른 품목 행 수. */
  changes: number;
  compareSeq: string | null;
  latestSeq: string | null;
  view: string;
  viewColumns: number;
  fields: MirrorField[];
  /** 이 화면에서 읽지 않아 빈 열의 수. */
  rest: number;
  items: MirrorItem[];
  itemChanges: number;
  itemRows: number;
  itemsAllRead: boolean;
  place: MirrorPlace[];
  rounds: MirrorRound[];
  baseToken: string;
  /** 창에서 가져올 수 없는 까닭. Chrome 의 수집기에서 고른다. */
  blocked: string | null;
};

/** Chrome 의 나라장터 탭 하나. <code>id</code> 는 호스트 프로세스와 탭 번호를 이은 것이다. */
export type MirrorTab = {
  id: string;
  pid: number;
  tabId: number;
  browser: string;
  title: string;
  /** 나라장터 머리 제목. */
  screen: string;
  /** 화면의 메뉴 번호(숫자 다섯, ADR-037). 모르거나 옛 확장이면 빈 문자열. */
  menu: string;
  state: "reading" | "supported" | "unsupported" | "error";
  /** 로컬 <code>yyyy-MM-ddTHH:mm:ss</code>. 아직 읽지 않았으면 빈 문자열. */
  readAt: string;
  message: string;
  /** Chrome 에서 앞에 있는 탭. */
  front: boolean;
  shot: MirrorShot | null;
  /** 투영하지 못한 까닭. */
  error: string | null;
  /** 화면 그대로 — 이름표와 보이는 글. 옛 확장은 보내지 않아 빈 목록이다. */
  screenRows: ScreenRow[];
};

/** 수집하는 화면 하나(ADR-037) — 매핑에서 화면을 단 프로필. <code>kind</code> 는 접수·공고·계약. */
export type SupportedScreen = { code: string; entityType: string; kind: string; name: string };

export type MirrorState = {
  readOnly: boolean;
  /** 지금 붙은 확장이 탭을 알렸는가. 옛 판은 알리지 않는다. */
  reporting: boolean;
  front: string | null;
  tabs: MirrorTab[];
  /** 확장의 설정. 옛 확장이거나 아직 알리지 않았으면 null. */
  settings: ExtensionSettings | null;
  /** 수집하는 화면 — 「수집 안 함」 의 안내가 이것을 그대로 보인다. 열람 창은 빈 목록. */
  screens: SupportedScreen[];
};

/** 창의 가져오기. 낡았으면 쓰지 않고 그 탭을 다시 읽힌다. */
export type ImportResult =
  | { status: "stored"; entity: string; changed: boolean; at: string }
  | { status: "stale"; message: string };

// ── 수집 기록 ──────────────────────────────────────────
// 성공한 저장만 남는다. 검토 중이거나 결과를 확인하지 못한 것은 브라우저에 있다.

/** 수집 한 번. 값은 화면에 그대로 찍을 문자열이다. */
export type CaptureEntry = {
  captureId: string;
  /** 로컬 <code>yyyy-MM-ddTHH:mm:ss</code>. */
  at: string;
  entityType: EntityType;
  /** 접수·공고·계약. */
  kind: "접수" | "공고" | "계약";
  /** 번호-차수. 표의 번호와 같은 표기라 그대로 검색에 건다. */
  number: string;
  title: string;
  changed: boolean;
  /** <code>저장</code> · <code>변경 없음</code>. */
  result: string;
  /** <code>live</code> 확장 · <code>file</code> JSON 파일. */
  scope: string;
};

/** 오늘(로컬 자정부터) 들어온 수집과 마지막 하나(<code>captures</code>). */
export type CaptureDay = {
  today: string;
  /** 늦은 것부터. */
  entries: CaptureEntry[];
  requests: number;
  notices: number;
  contracts: number;
  /** 오늘 것 중 무엇을 바꾼 수. */
  saved: number;
  /** 가장 늦은 수집. 오늘이 아니어도 온다. */
  last: CaptureEntry | null;
};

export type Settings = {
  /** 골라 둔 계획 엑셀의 전체 경로. 한 번도 고른 적이 없으면 빈 문자열. */
  planPath: string;
  /** 제출본 파일 이름에만 쓴다. 자료의 임자를 정하는 값이 아니다(ADR-023). */
  submitterName: string;
};

/** 제출본 한 벌을 뜬 결과. 그만두면 null 이 온다. */
export type SubmitResult = { path: string; nameMissing: boolean };

/** 취합 한 번의 결과. 자세한 것은 취합본 옆 txt 에 있다. */
export type MergeResult = {
  path: string; reportPath: string;
  submissions: number; plans: number; requests: number; notices: number; contracts: number;
  conflicts: number; rejected: number;
};

// ── 지우기 ─────────────────────────────────────────────
// 되돌릴 수 없는 일이라, 묻기 전에 무엇이 사라지는지 세어 보인다.

/** 계열 전체인가 이 차수 하나인가. */
export type DeletionScope = "series" | "seq";

export type DeletionPlan = {
  entityType: EntityType;
  display: string;
  /** 이 셈이 겨눈 차수. 범위가 계열이어도 사람이 고른 줄의 차수다. */
  seq: string;
  title: string;
  /** 이 본번호로 쌓인 차수 전부. 낮은 것부터. */
  revisions: string[];
  /** 계열까지 걷히는가. 걷히면 사람이 적은 값과 링크도 함께 간다. */
  seriesGoes: boolean;
  childRows: number;
  userFields: number;
  overrides: number;
  linked: boolean;
};

// ── 구조 보기 ──────────────────────────────────────────
// 접수 → 공고 → 계약. 보통 1:1:1 이라 사슬 하나가 한 벌이고, 평평한 표로 펴면 위쪽 값이
// 아래 줄마다 되풀이돼서 자료가 겹쳐 들어간 것처럼 보인다.

export type OutlineItem = {
  lineNo: number;
  name: string;
  specification: string;
  quantity: string;
  unit: string;
  unitPrice: string;
  amount: string;
};

export type OutlineContract = {
  /** 차수를 벗은 키. 사람이 적은 값이 여기 매달린다. */
  key: string;
  /** 차수까지 붙은 계약번호. 화면에 보이는 것은 이쪽이다. */
  number: string;
  title: string;
  contractedOn: string;
  amount: string;
  counterparty: string;
  demandAgency: string;
  /** 쌓인 차수. 본문에는 최신 하나만 서고 나머지는 이름표로 모인다. */
  revisions: string[];
  items: OutlineItem[];
};

export type OutlineNotice = {
  key: string;
  number: string;
  title: string;
  postedAt: string;
  agency: string;
  estimatedPrice: string;
  revisions: string[];
};

/**
 * 사슬의 가운데 칸. 공고 하나가 아니라 <b>건 하나</b>다 — 취소되고 재채번된 공고는 차수가
 * 아니라 <b>본번호</b>가 갈리므로, 공고마다 칸을 세우면 접수도 계약도 하나인 조달 건이 둘로
 * 서고 그중 죽은 쪽이 접수와 계약을 안는다.
 */
export type OutlineNoticeGroup = {
  /**
   * 건의 이름. <b>그리지 않는다</b> — 그 건이 아는 본번호 중 가장 이른 것이라 들어온 적 없는
   * 조상일 수 있고, 보여 주면 사람이 없는 공고를 찾으러 간다. 그래도 받아 두는 것은 옆판과
   * 시험이 <b>건을 짚을 자리</b>가 필요해서다.
   */
  groupBase: string;
  /**
   * 살아 있는 공고 한 장. <b>null 일 수 있다</b> — 두 본번호가 서로의 최신 차수를 가리키면
   * 대체되지 않은 것이 하나도 남지 않는다. 그때도 공고는 들어와 있으므로, 안 들어온 칸으로
   * 떨어뜨리지 않는다.
   */
  current: OutlineNotice | null;
  /** 대체된 공고. <b>늦은 것부터</b> 온다 — 방금 지나간 것이 맨 앞이다. */
  superseded: OutlineNotice[];
};

/** 사슬의 첫 칸. 조달요구번호가 여덟 개까지 가므로 낱개로 온다 — 화면이 몇 개만 펴고 접는다. */
export type OutlineRequest = {
  key: string;
  number: string;
  title: string;
  receivedOn: string;
  goodsAmount: string;
  budgetAmount: string;
  demandAgency: string;
  requestNumbers: string[];
  revisions: string[];
};

/**
 * 한 벌. <b>접수 → 공고 → 계약</b>이 나란히 서고, 없는 칸은 null 로 온다.
 *
 * <p>세 칸이 모두 서는 것이 기본이고 1:1:1 이다. 계약이 둘 이상일 때만 끝 칸이 갈라진다.
 * 어디에도 매달리지 못한 계약은 <code>request</code>·<code>notice</code> 가 모두 null 인
 * 사슬로 온다 — 따로 내지 않으면 화면에서 통째로 사라진다.</p>
 */
export type OutlineChain = {
  key: string;
  request: OutlineRequest | null;
  notice: OutlineNoticeGroup | null;
  contracts: OutlineContract[];
};

/** 쌓인 것을 생긴 모양대로. */
export type Outline = {
  chains: OutlineChain[];
};

// ── 공고 연결 ──────────────────────────────────────────

/** 두 쪽을 나란히 놓고 견준 항목 하나. */
export type LinkFacet = {
  name: string;
  contract: string;
  notice: string;
  /** null 이면 한쪽 값이 없어 판정할 수 없다는 뜻이다. */
  agrees: boolean | null;
};

export type Candidate = {
  contractKey: string;
  contractTitle: string;
  noticeKey: string;
  noticeTitle: string;
  confidence: number;
  reason: string;
  noticeContractCount: number;
  /** 건명이 정확히 일치하는가. 아니면 약한 근거로 올라온 후보다. */
  titleMatched: boolean;
  /** 잇기 전에 확인할 점. 없으면 판정을 통과한 유일한 후보다. */
  blocker: string | null;
  facets: LinkFacet[];
};

/**
 * 잇기 화면에 세울 계약 한 줄. 후보가 없는 것도, **이미 이어진 것도** 온다 —
 * 잘못 이어진 것을 끊으려면 그 줄이 화면에 서 있어야 한다.
 */
export type LinkContract = {
  key: string;
  title: string;
  candidateCount: number;
  /** 지금 이어져 있는 공고. null 이면 아직 이어지지 않았다. */
  noticeKey: string | null;
  noticeTitle: string;
  /**
   * 누가 이었는가 — `human` 이면 사람, `explicit` 이면 ERP 명시 참조, `auto` 는 옛 판의 기계
   * 연결이 남은 것이다(ADR-029 뒤로 새로 생기지 않는다). 이어지지 않았으면 null.
   */
  decidedBy: string | null;
};

/** 사람이 손수 고를 공고 하나. 추천이 찾지 못한 짝을 「직접 찾기」에서 고르는 자리에 쓴다. */
export type NoticeChoice = {
  key: string;
  title: string;
  postedAt: string;
  /** 이 공고에 이미 붙은 계약 수. 한 공고에 계약이 여럿일 수 있어 막지는 않는다. */
  linked: number;
};

/** 공고 연결 화면이 한 번에 받아 가는 것. */
export type LinkWork = {
  contracts: LinkContract[];
  candidates: Candidate[];
  notices: NoticeChoice[];
};

// ── 접수 ↔ 공고 잇기 ───────────────────────────────────
// 계약 쪽과 같은 얼개인데 왼쪽에 서는 것이 접수다. 다리가 내는 이름이 달라 타입도 따로
// 두고, 화면에 넘기기 전에 App 이 위의 꼴로 옮겨 담는다 — 잇기 화면은 하나여야 한다.

/** 접수 쪽 견줌. 왼쪽 값의 이름만 다르다. */
export type RequestLinkFacet = {
  name: string;
  request: string;
  notice: string;
  agrees: boolean | null;
};

export type RequestCandidate = {
  requestKey: string;
  requestTitle: string;
  noticeKey: string;
  noticeTitle: string;
  confidence: number;
  reason: string;
  /** 이 공고에 이미 붙은 접수 수. 접수:공고 는 1:1 이라 0 이 아니면 막힌다. */
  noticeLinkedCount: number;
  titleMatched: boolean;
  blocker: string | null;
  facets: RequestLinkFacet[];
};

export type LinkRequest = {
  key: string;
  title: string;
  candidateCount: number;
  noticeKey: string | null;
  noticeTitle: string;
  decidedBy: string | null;
};

export type RequestLinkWork = {
  requests: LinkRequest[];
  candidates: RequestCandidate[];
  notices: NoticeChoice[];
};

/** ERP 명시 참조로 새로 이은 수. 두 갈래(접수·계약)를 합친 것이다 — 있던 연결은 세지 않는다. */
export type RelinkResult = { linked: number };

// ── 계획 ───────────────────────────────────────────────
// 접수·공고·계약과 달리 <b>엑셀에서 온다</b>. 서식은 표본에 고정되어 있어(ADR-023 개정)
// 짚을 것이 없고, 남은 결정은 <b>어느 파일인가</b> 하나뿐이다 — 그 자리가 설정에 남는다.

/** 고른 계획 엑셀. 이름은 보여 주려고, 경로는 설정에 적힌 것과 같아 그대로 적어 두려고 온다. */
export type PlanPick = {
  name: string;
  path: string;
};

/** 계획 엑셀 한 권을 읽은 결과. 머리글이 어긋난 것은 고장이 아니라 파일을 다시 보면 되는 일이다. */
export type PlanImportResult = {
  rows: number;
  created: number;
  updated: number;
  /** 조달요구번호가 빈 줄. 계획의 자연키라 담을 자리가 없다. */
  skipped: number;
  /** 없던 필수 **머리글 이름**. 하나라도 있으면 **한 줄도 들어가지 않았다**. */
  missingRequired: string[];
  /** 실제로 잡힌 머리글. 어느 열이 들어갔는지 사람이 눈으로 확인하는 자리다. */
  found: string[];
  ok: boolean;
};

// ── 현황 ───────────────────────────────────────────────
// 무엇을 보일지는 뒤(`Status.Metrics`)가 정한다. 화면은 **오는 대로** 그린다 — 지표를 더하거나
// 빼는 일이 그 배열만 고치는 일이어야 하므로, 여기에 지표의 이름이 나타나서는 안 된다.

/** 센 결과 한 칸. `거르개` 는 눌렀을 때 계획 탭이 걸 조건이고 꼴은 `"열이름=값"` 하나뿐이다. */
export type Cell = { 이름: string; 수: number; 거르개: string | null };

/** 같은 묶음의 칸들. 묶음은 지표 배열에 처음 나온 차례로 선다. */
export type Group = { 이름: string; cells: Cell[] };

export type StatusReport = { groups: Group[] };

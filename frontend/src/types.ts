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

/**
 * 자료가 쌓이는 자리. <b>DB 안이 아니라 밖(config.json)에 적힌다</b> — 어느 DB 를 열지를
 * DB 안에 두면 그 값을 읽으려고 DB 를 먼저 열어야 한다. 그래서 Settings 와 따로 온다.
 */
/** 판과 라이선스. 고지 둘은 exe 안에 박힌 전문이다. */
export type About = { version: string; license: string; notices: string };

export type DataLocation = {
  path: string;
  folder: string;
  sizeBytes: number;
  /** 기본 자리인가. 아니면 밖에서 읽는 쪽이 못 따라올 수 있다. */
  isDefault: boolean;
  configPath: string;
};

/**
 * 확장이 어디까지 깔렸는지. 앱은 브라우저를 볼 수 없어, 확장이 인사할 때 남긴 흔적으로 안다.
 * `at` 은 ISO 8601 로컬 시각.
 */
export type ExtensionStatus = {
  prepared: boolean;
  embeddedVersion: string;
  diskVersion: string | null;
  contacts: { browser: string; version: string; at: string }[];
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
  /** 자동으로 이어지지 않은 까닭. */
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
  /** 누가 이었는가 — `auto` 면 기계, `human` 이면 사람. 이어지지 않았으면 null. */
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

/**
 * 자동으로 다시 이은 결과. 두 갈래(접수·계약)를 합친 수다.
 *
 * `released` 는 규칙판이 올라 **다시 본** 옛 자동 링크 수다 — 사람이 확정한 것은 여기 들지
 * 않는다. 그래서 `linked` 가 `released` 보다 적을 수 있고, 그것은 고장이 아니라
 * 새 규칙이 더 깐깐해졌다는 뜻이다.
 */
export type RelinkResult = { linked: number; released: number };

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

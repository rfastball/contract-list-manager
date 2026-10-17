import mappingJson from "../../src/Pclm.Core/Erp/mapping.json?raw";
/**
 * 개발용 가짜 다리.
 *
 * 창(WebView2) 없이 브라우저에서 화면을 손볼 수 있게 계약면 뷰를 흉내 낸다.
 * `import.meta.env.DEV` 안에서만 동적으로 불러오므로 **배포 묶음에는 들어가지 않는다**.
 *
 * 열 이름과 순서는 `tests/Pclm.Core.Tests/contract/views.txt` 와 같게 둔다 —
 * 여기가 어긋나면 진짜 앱에서만 깨지는 화면을 만들게 된다.
 */

import type {
  BatchChange, BatchDeletionPlan, CaptureDay, CaptureEntry, DashboardReport, DashboardRow, DashboardStage, DeletionPlan, ExtensionStatus, EntityType, ImportResult, MergeResult,
  ExtensionSettings, MirrorField, MirrorItem, MirrorShot, MirrorState, MirrorTab, ScreenRow, SupportedScreen,
  Outline, OutlineContract, OutlineNotice, OutlineNoticeGroup, OutlineRequest,
  PlanImportResult, PlanPick,
  Row, Session, Settings, Sheet, StatusReport, SubmitResult, Summary,
  SwitchResult, UserColumn, WindowPrefs, WorkfilePlan, ColumnAvailable, ColumnContext, ColumnTrace, FreezePlan, SqlCheck,
  UserView, UserViewColumn, UserViewContext, UserViewPreview, UserViewSample,
} from "./types";
import type { Flow, FlowEnv, FormulaV2 } from "./flow/ast";
import type { ViewPackage, ViewPackagePick, ViewPackagePreview } from "./views/packageTypes";
// 서식용 예시 보기 — 진짜 다리가 exe 에서 꺼내는 그 파일 한 벌을 그대로 읽는다.
import 예시보기 from "../../src/Pclm.Core/Storage/Presets/builtin-views.json?raw";
import { clausesOf, docWord, qualified } from "./flow/ast";
import { checkFlow } from "./flow/check";
import { evaluate, type EvalInput } from "./flow/evaluate";
import { toV2 } from "./flow/migrate";
import { 받침 } from "./flow/josa";
import { checkSqlMock, compileFlow } from "./flow/sql";

/**
 * 가짜 다리의 갈림 스위치. <code>열람</code> 이면 열람 창처럼 답한다 — 띠가 서고 고치는 요청이 거절된다.
 * 기본은 작업자료 창이고, 미리보기에서는 주소에 <code>?열람</code> 을 붙여 켠다.
 *
 * <p>시험은 이 객체의 값을 바꾼다. 응답을 가로채는 것으로는 모자라다 — 화면이 처음 뜰 때 동시에 나가는
 * 요청은 시험의 엿듣개를 거치지 않고 이 모듈의 본래 <code>invoke</code> 로 가는 일이 있다.</p>
 */
export const 스위치 = {
  열람: typeof location !== "undefined" && new URLSearchParams(location.search).has("열람"),
  /**
   * 확장이 어디까지 깔렸는가. 미리보기에서는 주소에 <code>?확장=설치 전</code> 처럼 붙여 바꾼다.
   * <code>없음</code> 이면 진짜 다리가 개발 실행·시험 홈에서 하듯 상태 읽기를 거절한다.
   */
  확장: ((typeof location !== "undefined" && new URLSearchParams(location.search).get("확장")) || "연결") as
    "없음" | "설치 전" | "기다림" | "옛 판" | "연결" | "끊김",
  /**
   * 확장 상태를 통째로 갈아 끼운다. 시험이 판·인사 기록을 제 손으로 짓는 자리다 — 응답을 가로채면 처음 뜰 때
   * 동시에 나가는 요청이 엿듣개를 비껴가 위의 꼴이 먼저 서는 일이 있다.
   */
  확장상태: null as ExtensionStatus | null,
  /**
   * 내 보기의 열과 미리보기 자료(userViewContext)를 이 까닭으로 거절한다. 보기는 처음 뜰 때 이것을 보기 목록과 동시에
   * 부르므로 응답을 가로채는 엿듣개를 비껴간다 — 시험은 여기서 실패를 짓는다.
   */
  뷰맥락오류: null as string | null,
  /** 설정 읽기(settings)를 이 횟수만큼 거절한다. 처음 뜰 때 동시에 나가는 부름이라 엿듣개를 비껴간다 — 시험은 여기서 실패를 짓는다. */
  설정오류: 0,
};

/** 지금 로컬 날짜의 그 시각을 다리가 내는 꼴(<code>yyyy-MM-ddTHH:mm:ss</code>)로. */
export const 오늘의 = (hhmm: string) => {
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}T${hhmm}:00`;
};

/**
 * 가짜 다리의 수집 기록. 늦은 것부터 둔다. 번호와 건명은 아래 표본(지어낸 것)과 같아 「보기」 가 그 줄을 찾는다.
 *
 * <p>시험은 <code>줄</code> 맨 앞에 넣고 <code>판</code> 을 올려 새 수집이 들어온 것처럼 꾸민다 —
 * <code>dataVersion</code> 이 이 판을 낸다.</p>
 */
export const 수집 = {
  판: 0,
  줄: [] as CaptureEntry[],
};

const 수집줄 = (at: string, kind: CaptureEntry["kind"], number: string, title: string, changed: boolean): CaptureEntry => ({
  captureId: `${kind}-${number}-${at}`,
  at,
  entityType: kind === "접수" ? "request" : kind === "공고" ? "notice" : "contract",
  kind, number, title, changed,
  result: changed ? "저장" : "변경 없음",
  scope: "live",
});
export { 수집줄 };

/** 진짜 다리의 Bridge.Writes 와 같은 목록. 열람이면 이것들을 거절한다. */
const 고치는요청 = new Set([
  "addColumn", "updateColumn", "removeColumn", "moveColumn",
  "setField", "setOverride", "clearOverride", "deleteEntity",
  "batchEdit", "batchDeletionPlan", "batchDelete", "saveComputedColumn", "freezeColumn",
  "saveSettings", "pickPlanExcel", "importPlan", "submit", "merge",
  "prepareExtension", "openExtensionSetup", "erpTools",
  "pickWorkfile", "moveWorkfile", "switchWorkfile", "newWorkfile", "backupWorkfile",
  "importShot", "mirrorCommand",
  "windowPrefs", "saveWindowPrefs",
  "saveUserView", "deleteUserView", "pickViewPackage", "builtinViewPackage", "previewViewPackage", "importViewPackage",
]);

/** 압축 안에서 켠 창이 자동 실행용으로 exe 를 복사해 둘 자리(지어낸 사람). 미리보기는 주소에 <code>?압축</code> 을 붙여 본다. */
export const 복사자리 = "C:\\Users\\홍길동\\AppData\\Local\\Programs\\계약목록\\계약목록.exe";

/** 가짜 다리의 창 몸가짐. 진짜 다리는 홈의 window.json 에 적는다. 시험이 처음 꼴로 되돌린다. */
export const 창 = {
  몸가짐: { closeToTray: false, autostart: false, autostartAvailable: true, autostartReason: null,
    autostartCopyTarget: typeof location !== "undefined" && new URLSearchParams(location.search).has("압축") ? 복사자리 : null } as WindowPrefs,
};

/** 가짜 다리의 확장. 판과 폴더는 지어낸 것이다. */
const 확장판 = "0.9.0";
const 확장폴더 = "C:\\Users\\홍길동\\AppData\\Local\\Pclm\\extension";

/** 가짜 다리의 지금 작업자료. 경로는 지어낸 것이다. */
const 작업자료 = "C:\\Users\\홍길동\\AppData\\Local\\Pclm\\계약자료.pclm";

const 통합열 = [
  "계약번호", "계약본번호", "차수", "계약건명", "계약일자", "계약방법", "계약구분",
  "품명", "수량", "단위", "계약금액", "계약보증금률(%)", "계약보증금(원)", "수수료", "지체상금률", "하자보수보증금률",
  "하자담보책임기간", "계약기간", "납품기한", "인도조건", "납품장소", "분할납품", "지급방법",
  "수요기관", "검사기관", "검수기관", "계약상대자", "대표자", "사업자등록번호",
  "상대자주소", "상대자전화", "상대자팩스",
  "진행상태", "메모",
  "입찰공고번호", "공고명", "공고종류", "게시일시", "입찰방식", "낙찰방법", "낙찰하한율",
  "사업예산", "배정예산", "추정가격", "기초금액", "개찰일시", "입찰개시일시", "입찰마감일시",
  "등록마감일시", "공고담당자", "사전규격등록번호",
];

const 공고열 = [
  "입찰공고번호", "공고본번호", "차수", "공고명", "공고종류", "게시일시", "입찰방식",
  "낙찰방법", "낙찰하한율", "계약방법", "계약구분", "사업예산", "배정예산", "추정가격",
  "기초금액", "분할납품", "하자담보기간", "공고기관", "공고담당자", "집행관", "지역제한",
  "사전규격등록번호", "개찰일시", "입찰개시일시", "입찰마감일시", "등록마감일시",
  "수요기관", "세부품명", "세부품명번호", "수량", "단위", "납품기한", "인도조건",
  "담당부서", "담당자", "담당자전화", "담당자팩스",
  // 관련공고는 적힌 그대로이고, 뒤의 둘은 그것을 타고 지어진 파생값이다 —
  // 「공고건」은 갈린 본번호를 묶은 이름, 「현행공고」는 그 건에서 지금 서 있는 한 장.
  "관련공고", "공고건", "현행공고",
  "검토여부", "메모",
];

const 계약열 = [
  "계약번호", "계약본번호", "차수", "계약건명", "계약일자", "계약방법", "계약구분",
  "품명", "수량", "단위", "계약금액", "계약보증금률(%)", "계약보증금(원)", "수수료", "지체상금률", "하자보수보증금률",
  "하자담보책임기간", "계약기간", "납품기한", "인도조건", "납품장소", "분할납품", "지급방법",
  "수요기관", "검사기관", "검수기관", "계약상대자", "대표자", "사업자등록번호",
  "상대자주소", "상대자전화", "상대자팩스",
  "진행상태", "메모",
];

const 접수열 = [
  "접수번호", "접수본번호", "차수", "조달요구번호", "품목수",
  "요청명", "접수일자", "업무구분", "계약법구분", "계약방법", "계약유형",
  "낙찰방법", "회계구분", "지급방법",
  "품대", "수수료", "부가가치세액", "예산금액",
  "외국산여부", "요청구분", "공개여부", "전담관", "담당과", "담당자",
  "선급금선고지", "선급금지급가능", "기타사항",
  "수요기관", "수요기관코드", "기관담당자", "기관담당자전화", "기관담당자팩스",
  "세부품명", "세부품명번호", "수량", "단위", "인도조건", "납품기한", "재고번호",
  "확인여부", "메모",
];

/**
 * 계획은 사람 열이 없다 — <code>plan</code> 은 접수·공고·계약과 나란한 개체가 아니라
 * <code>user_column</code> 의 <code>entity_type</code> 에 자리가 없다. 읽기 전용이다.
 */
const 계획열 = [
  "조달요구번호", "재고번호", "품명", "화폐구분", "단위", "지시수량", "요청부대부서명",
  "요청부대담당자", "요청부대사용자전화번호", "계약부서", "담당자", "연락처",
  "단계", "접수번호", "접수일자", "입찰공고번호", "게시일시", "개찰일시",
  "계약건수", "계약번호", "계약일자", "계약금액",
];

/**
 * 통합이 접수 뷰에서 붙여 오는 아홉 열. `수수료` 는 계약 쪽에 이미 있어 `접수수수료` 로 낸다 —
 * 같은 이름이 둘이면 SQLite 가 조용히 뒤엣것을 `수수료:1` 로 바꾼다.
 */
const 통합의접수열 = [
  "접수번호", "조달요구번호", "요청명", "접수일자", "품대", "접수수수료", "예산금액",
  "기관담당자", "기관담당자전화",
];

/** v_통합 이 내는 열. 계약·공고 몸통(통합열) 끝에 접수 아홉 열이 붙는다. */
const 통합표열 = [...통합열, ...통합의접수열];

/** 레코드를 가리키는 열. 고치면 값이 바뀌는 것이 아니라 레코드가 옮겨간다. Views.KeyColumns 와 같다. */
const 키열 = [
  "입찰공고번호", "공고본번호", "계약번호", "계약본번호",
  "접수번호", "접수본번호", "차수", "순번",
  // 건과 그 건의 현행 공고. 파생값이라 덮개를 씌워도 화면의 글자만 바뀐다.
  "공고건", "현행공고",
];

/** 통합이 공고·접수 뷰에서 붙여 오는 열. 이 줄의 키로 주소가 잡히지 않아 통합에서는 잠근다. */
const 통합의붙은열 = 통합표열.slice(통합표열.indexOf("입찰공고번호"));

/** 파서가 읽은 값을 고칠 수 있는 열 — 키 열도 사람 열도 아닌 나머지. Bridge.ReadSheet 와 같은 셈이다. */
const 고칠수있는열 = (columns: string[], editable: string[], 통합 = false) =>
  columns.filter(
    (c) => !키열.includes(c) && !editable.includes(c) && !(통합 && 통합의붙은열.includes(c)),
  );

const 씨앗 = [
  { 건명: "2026년 가람 수질측정기 조달", 품명: "수질검사장치", 기관: "가람군수지원단", 금액: 164_872_340 },
  { 건명: "26년 한별 공압식승강판 20톤 구매", 품명: "승강판", 기관: "한별군수지원단", 금액: 197_488_630 },
  { 건명: "26년 가람 이온분리-성분측정기 구매", 품명: "성분측정기", 기관: "가람군수지원단", 금액: 318_643_000 },
  { 건명: "26년 가람 절단기 구매", 품명: "플라즈마절단기", 기관: "가람군수지원단", 금액: 29_512_000 },
  { 건명: "26년 누림 항공유 분석장비 구매", 품명: "유분석기", 기관: "누림군수지원단", 금액: 88_140_000 },
];

const 상태 = ["준비", "투찰", "낙찰", "계약", "납품", "검수", "정산", "종료"];

/**
 * 접수 표본. 조달요구번호가 <b>여덟 개</b>인 것을 하나 둔다 — 접수 칸이 그것을 몇 개만
 * 펴고 나머지를 접는지 여기서 본다. 이름과 번호는 모두 지어낸 것이다.
 */
const 접수씨앗 = [
  {
    base: "MPKPLA26910286",
    요청명: "26년한별윈치8종구매",
    접수일자: "2026/06/09",
    품대: "181,725,200",
    수수료: "1,683,420",
    부가가치세액: "16,520,473",
    예산금액: "183,408,620",
    기관: "한별군수지원단",
    기관코드: "6410015",
    담당자: "홍길동",
    전화: "055-555-4123",
    팩스: "055-555-4100",
    품명: "수동윈치",
    품명번호: "25101901",
    재고번호: "9950000072851",
    수량: "209",
    요구번호: [
      "MPKPLA26910286", "MPKPLA26910287", "MPKPLA26910288", "MPKPLA26910289",
      "MPKPLA26910291", "MPKPLA26910292", "MPKPLA26910294", "MPKPLA26910295",
    ],
  },
  {
    base: "MPKPLA26910301",
    요청명: "26년가람수질측정기구매",
    접수일자: "2026/06/16",
    품대: "164,872,340",
    수수료: "1,527,710",
    부가가치세액: "14,988,394",
    예산금액: "209,480,290",
    기관: "가람군수지원단",
    기관코드: "6210022",
    담당자: "김철수",
    전화: "042-555-2624",
    팩스: "042-555-2600",
    품명: "수질검사장치",
    품명번호: "42115802",
    재고번호: "6640375019876",
    수량: "1",
    요구번호: ["MPKPLA26910301"],
  },
  {
    base: "MPKPLA26910412",
    요청명: "26년가람절단기구매",
    접수일자: "2026/06/24",
    품대: "29,512,000",
    수수료: "412,300",
    부가가치세액: "2,683,000",
    예산금액: "35,709,520",
    기관: "가람군수지원단",
    기관코드: "6210022",
    담당자: "이영희",
    전화: "042-555-2711",
    팩스: "042-555-2700",
    품명: "플라즈마절단기",
    품명번호: "29077509",
    재고번호: "9431000050017",
    수량: "12",
    요구번호: ["MPKPLA26910412", "MPKPLA26910413", "MPKPLA26910415"],
  },
];

const 돈 = (n: number) => n.toLocaleString("en-US");

/** 빈 열까지 포함해 한 줄을 채운다 — 진짜 뷰는 NULL 이 아니라 빈 문자열을 낸다. */
function 줄(columns: string[], values: Record<string, string>): Row {
  const row: Row = {};
  for (const c of columns) row[c] = values[c] ?? "";
  return row;
}

/** 계획 담당자. 계획 엑셀이 행마다 적는 것이라 계획이 곧 배정표다. 모두 지어낸 이름이다. */
const 계획담당 = ["홍길동", "김철수", "이영희", "박민수"];

/**
 * 계획 한 줄. <b>단계는 있는 것을 적는 것이지 판정이 아니다</b> — 어디까지 왔는지만 적는다.
 * 넷째 줄마다 계약이 둘이라 계약 세 칸을 비운다(하나로 정할 수 없는 것을 하나인 양 내지 않는다).
 */
function 계획행(i: number): Row {
  const 번호 = `MPKPLA269103${String(i + 10).padStart(2, "0")}`;
  const 단계 = ["미착수", "접수", "공고", "계약"][Math.min(Math.floor(i / 6), 3)];

  const 접수 = 단계 !== "미착수";
  const 공고 = 단계 === "공고" || 단계 === "계약";
  const 계약 = 단계 === "계약";
  const 둘로낙찰 = 계약 && i % 4 === 3;

  return 줄(계획열, {
    조달요구번호: 번호,
    재고번호: `2590-01-${String(200 + i).padStart(3, "0")}-${String(1000 + i * 7)}`,
    품명: ["수동윈치", "플라즈마절단기", "공압식승강판", "수질검사장치"][i % 4],
    화폐구분: "KRW",
    단위: "대",
    지시수량: String(3 + (i % 9)),
    요청부대부서명: `${["한별", "가람", "누림"][i % 3]}군수지원단 보급창`,
    요청부대담당자: 계획담당[(i + 1) % 계획담당.length],
    요청부대사용자전화번호: `051-000-${String(1000 + i).padStart(4, "0")}`,
    계약부서: `계약${1 + (i % 3)}과`,
    담당자: 계획담당[i % 계획담당.length],
    연락처: `02-000-${String(2000 + i).padStart(4, "0")}`,
    단계,
    접수번호: 접수 ? `${번호}-000` : "",
    접수일자: 접수 ? `2026/07/${String(3 + (i % 20)).padStart(2, "0")}` : "",
    입찰공고번호: 공고 ? `R26BK090170${String(50 + i)}-000` : "",
    게시일시: 공고 ? `2026/08/${String(4 + (i % 20)).padStart(2, "0")}` : "",
    개찰일시: 공고 ? `2026/08/${String(14 + (i % 10)).padStart(2, "0")} 11:00:00` : "",
    계약건수: 계약 ? (둘로낙찰 ? "2" : "1") : "0",
    계약번호: 계약 && !둘로낙찰 ? `R26TA091105${String(10 + i)}00` : "",
    계약일자: 계약 && !둘로낙찰 ? `2026/08/${String(20 + (i % 9)).padStart(2, "0")}` : "",
    계약금액: 계약 && !둘로낙찰 ? 돈(16_400_000 + i * 3_010_000) : "",
  });
}

/**
 * 통합 한 줄. 계약 시트와 통합 시트가 같은 값에서 나오고 <b>열 목록만 다르다</b> — 붙는 열이
 * 다를 뿐 계약 쪽 몸통은 한 벌이라, 두 벌로 적으면 한쪽이 조용히 늙는다.
 *
 * <p>접수는 1:1 이라 <b>한 줄에만</b> 매단다. 여러 계약에 같은 접수를 붙이면 화면 감이
 * 실제와 달라진다.</p>
 */
function 통합행(i: number, columns: string[] = 통합열): Row {
  const s = 씨앗[i % 씨앗.length];
  const n = String(i + 1).padStart(2, "0");
  const r = i === 0 ? 접수씨앗[1] : null;

  return 줄(columns, {
    접수번호: r ? `${r.base}-000` : "",
    조달요구번호: r ? r.요구번호.join(", ") : "",
    요청명: r?.요청명 ?? "",
    접수일자: r?.접수일자 ?? "",
    품대: r?.품대 ?? "",
    접수수수료: r?.수수료 ?? "",
    예산금액: r?.예산금액 ?? "",
    기관담당자: r?.담당자 ?? "",
    기관담당자전화: r?.전화 ?? "",
    계약번호: `R26TA091105${n}00`,
    계약본번호: `R26TA091105${n}`,
    차수: "00",
    계약건명: s.건명,
    계약일자: `2026/08/${String((i % 27) + 1).padStart(2, "0")}`,
    계약방법: "제한경쟁",
    계약구분: "총액계약",
    품명: s.품명,
    수량: "1",
    단위: "대",
    계약금액: 돈(s.금액),
    "계약보증금률(%)": "10",
    "계약보증금(원)": 돈(s.금액 * 0.1),
    지체상금률: "0.075",
    하자보수보증금률: "3",
    하자담보책임기간: "3년",
    납품기한: "2026/09/20",
    인도조건: "지정장소도착도",
    납품장소: "수요처 지정 창고",
    분할납품: "가능",
    지급방법: "정부구매카드",
    수요기관: s.기관,
    검사기관: s.기관,
    검수기관: s.기관,
    계약상대자: "㈜한국분석기기",
    대표자: "홍길동",
    사업자등록번호: "123-45-67890",
    입찰공고번호: `R26BK090170${n}-000`,
    공고명: s.건명,
    공고종류: "실공고(등록공고)",
    게시일시: "2026/07/02 16:59:28",
    입찰방식: "전자입찰",
    낙찰방법: "적격심사제",
    낙찰하한율: "84.245",
    사업예산: 돈(Math.round(s.금액 * 1.21)),
    배정예산: 돈(Math.round(s.금액 * 1.23)),
    추정가격: 돈(Math.round(s.금액 * 1.1)),
    기초금액: 돈(Math.round(s.금액 * 1.19)),
    개찰일시: "2026/07/11 11:00:00",
    입찰마감일시: "2026/07/10 18:00:00",
    공고담당자: "홍길동",
    진행상태: 상태[i % 상태.length],
    메모: i % 5 === 0 ? "선금 신청함" : "",
  });
}

/**
 * 계약이 아직 없는 통합 줄.
 *
 * <p>통합의 줄 하나는 계약이 아니라 <b>조달 건</b>이다 — 접수만 온 것도, 공고까지만 온 것도
 * 표에 선다. 그래야 구조 보기와 표 보기가 같은 것을 센다.</p>
 */
function 생애주기행(i: number, 공고까지: boolean): Row {
  const r = 접수씨앗[i];
  const s = 씨앗[i % 씨앗.length];
  const n = String(i + 60).padStart(2, "0");

  return 줄(통합표열, {
    접수번호: `${r.base}-000`,
    조달요구번호: r.요구번호.join(", "),
    요청명: r.요청명,
    접수일자: r.접수일자,
    품대: r.품대,
    접수수수료: r.수수료,
    예산금액: r.예산금액,
    기관담당자: r.담당자,
    기관담당자전화: r.전화,

    ...(공고까지
      ? {
          입찰공고번호: `R26BK090170${n}-000`,
          공고명: r.요청명,
          공고종류: "실공고(등록공고)",
          게시일시: "2026/07/02 16:59:28",
          입찰방식: "전자입찰",
          낙찰방법: "적격심사제",
          낙찰하한율: "84.245",
          사업예산: r.품대,
          배정예산: r.예산금액,
          추정가격: 돈(Math.round(s.금액 * 1.1)),
          기초금액: 돈(Math.round(s.금액 * 1.19)),
          개찰일시: "2026/07/11 11:00:00",
          공고담당자: "홍길동",
        }
      : {}),
  });
}

/** 접수 한 줄. 끝 일곱 열은 v_공고 의 규율대로 <b>칸마다 모든 줄이 같을 때만</b> 채운다. */
function 접수행(i: number): Row {
  const r = 접수씨앗[i];
  const 하나 = r.요구번호.length === 1;

  return 줄(접수열, {
    접수번호: `${r.base}-000`,
    접수본번호: r.base,
    차수: "000",
    조달요구번호: r.요구번호.join(", "),
    품목수: String(r.요구번호.length),
    요청명: r.요청명,
    접수일자: r.접수일자,
    업무구분: "군수품",
    계약법구분: "국가계약법",
    계약방법: "제한경쟁",
    계약유형: "총액계약",
    낙찰방법: "적격심사제",
    회계구분: "일반회계",
    지급방법: "정부구매카드",
    품대: r.품대,
    수수료: r.수수료,
    부가가치세액: r.부가가치세액,
    예산금액: r.예산금액,
    외국산여부: "부",
    요청구분: "일반요청",
    공개여부: "공개",
    전담관: "박전담",
    담당과: "장비물자과",
    담당자: "최담당",
    선급금선고지: "부",
    선급금지급가능: "여",
    수요기관: r.기관,
    수요기관코드: r.기관코드,
    기관담당자: r.담당자,
    기관담당자전화: r.전화,
    기관담당자팩스: r.팩스,
    // 품목이 여럿이면 세부품명이 갈리므로 비운다. 수량은 갈리지 않고 합해진다.
    세부품명: 하나 ? r.품명 : "",
    세부품명번호: 하나 ? r.품명번호 : "",
    수량: r.수량,
    단위: "대",
    인도조건: "지정장소도착도",
    납품기한: "2026/10/27",
    재고번호: 하나 ? r.재고번호 : "",
    확인여부: ["확인", "미확인", "확인"][i],
    메모: i === 0 ? "요구번호 8건 · 윈치 8종" : "",
  });
}

/** 갈린 건의 이름과, 그 건에서 지금 서 있는 한 장. 아래 두 곳이 함께 본다. */
const 갈린건 = "R26BK09011054";
const 갈린건의현행 = "R26BK09012082-000";

/**
 * 취소 후 재공고로 <b>본번호가 갈린</b> 한 건. 차수 눈이 무엇을 보이려는 것인지가 여기 다 있다 —
 * 최신만 내는 표에는 마지막 한 줄만 서고, 차수를 편 표에서만 셋이 나란히 선다.
 *
 * <p>번호 꼴만 실측을 흉내 냈고 값은 모두 지어낸 것이다.</p>
 */
const 갈린공고: Row[] = [
  { 번호: `${갈린건}-000`, 본: 갈린건, 차: "000", 종류: "실공고(등록공고)", 관련: "" },
  { 번호: `${갈린건}-001`, 본: 갈린건, 차: "001", 종류: "취소공고", 관련: `${갈린건}-000` },
  { 번호: 갈린건의현행, 본: "R26BK09012082", 차: "000", 종류: "재공고", 관련: `${갈린건}-001` },
].map((n) =>
  줄(공고열, {
    입찰공고번호: n.번호,
    공고본번호: n.본,
    차수: n.차,
    공고명: "26년 가람 조립대 구매",
    공고종류: n.종류,
    게시일시: "2026/07/09 10:00:00",
    계약방법: "제한경쟁",
    수요기관: "가람군수지원단",
    세부품명: "작업대",
    세부품명번호: "4110160101",
    수량: "12",
    단위: "대",
    관련공고: n.관련,
    공고건: 갈린건,
    현행공고: 갈린건의현행,
  }),
);

const sheets: Record<string, Sheet> = {
  "v_공고": {
    name: "v_공고",
    columns: 공고열,
    editable: ["검토여부", "메모"],
    correctable: 고칠수있는열(공고열, ["검토여부", "메모"]),
    overrides: {},
    // 갈린 건에서는 <b>현행 한 장만</b> 선다 — 취소된 원공고와 취소공고는 대체되어 빠진다.
    // 그 셋이 나란히 서는 것을 보려면 「차수」 눈으로 옮긴다(v_공고차수).
    rows: [...씨앗.map((s, i) =>
      줄(공고열, {
        입찰공고번호: `R26BK090170${String(i + 1).padStart(2, "0")}-000`,
        공고본번호: `R26BK090170${String(i + 1).padStart(2, "0")}`,
        차수: "000",
        공고명: s.건명,
        공고종류: "실공고(등록공고)",
        게시일시: "2026/07/02 16:59:28",
        입찰방식: "전자입찰",
        낙찰방법: "적격심사제",
        낙찰하한율: "84.245",
        계약방법: "제한경쟁",
        계약구분: "총액계약",
        사업예산: 돈(Math.round(s.금액 * 1.21)),
        배정예산: 돈(Math.round(s.금액 * 1.23)),
        추정가격: 돈(Math.round(s.금액 * 1.1)),
        기초금액: 돈(Math.round(s.금액 * 1.19)),
        분할납품: "가능",
        하자담보기간: "3년",
        공고기관: "국방부조달본부",
        공고담당자: "홍길동",
        개찰일시: "2026/07/11 11:00:00",
        수요기관: s.기관,
        세부품명: s.품명,
        세부품명번호: `42115802${String(i + 1).padStart(2, "0")}`,
        수량: "3",
        단위: "대",
        // 날짜가 박힌 공고는 그 날을, 일수만 있는 공고는 계약일부터 센 말을 낸다.
        납품기한: i % 3 === 0 ? "2026/09/26" : `계약 후 ${[60, 90, 300][i % 3]}일 이내`,
        인도조건: "현장설치도",
        담당부서: "의무과",
        담당자: "홍길동",
        담당자전화: "042-555-2624",
        담당자팩스: "042-555-2600",
        // 갈리지 않은 건은 제 본번호가 곧 건 이름이고, 제가 그 건의 현행이다.
        공고건: `R26BK090170${String(i + 1).padStart(2, "0")}`,
        현행공고: `R26BK090170${String(i + 1).padStart(2, "0")}-000`,
        검토여부: ["미검토", "검토중", "참여", "불참"][i % 4],
      }),
    ), 갈린공고[2]],
  },
  // 통합 탭이 보는 것. 손으로 고친 칸을 하나 세워 둔다 — 표시와 되돌리기를 볼 자리다.
  "v_통합": {
    name: "v_통합",
    columns: 통합표열,
    editable: ["진행상태", "메모"],
    correctable: 고칠수있는열(통합표열, ["진행상태", "메모"], true),
    overrides: { "R26TA0911050100": { 계약금액: "164,872,340" } },
    // 계약이 아직 없는 두 줄을 끝에 세운다 — 접수+공고 하나, 접수만 하나.
    // 화면이 그런 줄을 어떻게 가리키고 어떻게 잠그는지 보는 자리다.
    rows: [
      ...Array.from({ length: 42 }, (_, i) => 통합행(i, 통합표열)),
      생애주기행(0, true),
      생애주기행(2, false),
    ],
  },
  // 계획. 분모라 아직 아무것도 오지 않은 줄(미착수)이 대부분이고, 뒤로 갈수록 차 있다.
  // 이름·번호는 모두 지어낸 것이다.
  "v_계획": {
    name: "v_계획",
    columns: 계획열,
    // 읽기 전용이다. 진짜 다리도 계획 뷰에는 빈 목록을 낸다(Bridge.ReadSheet).
    editable: [],
    correctable: [],
    overrides: {},
    rows: Array.from({ length: 24 }, (_, i) => 계획행(i)),
  },
  "v_접수": {
    name: "v_접수",
    columns: 접수열,
    editable: ["확인여부", "메모"],
    correctable: 고칠수있는열(접수열, ["확인여부", "메모"]),
    overrides: {},
    rows: 접수씨앗.map((_, i) => 접수행(i)),
  },
  "v_계약": {
    name: "v_계약",
    columns: 계약열,
    editable: ["진행상태", "메모"],
    correctable: 고칠수있는열(계약열, ["진행상태", "메모"]),
    overrides: { "R26TA0911050100": { 계약금액: "164,872,340" } },
    rows: Array.from({ length: 42 }, (_, i) => {
      const 통합 = 통합행(i);
      return 줄(계약열, Object.fromEntries(계약열.map((c) => [c, 통합[c] ?? ""])));
    }),
  },
};

// ── 손으로 채우는 열 ──────────────────────────────────
// 진짜 앱은 열이 바뀌면 뷰를 다시 짓는다. 여기서는 그 자리에 사람 열을 다시 얹는다.

type Entity = EntityType;

const userColumns: Record<Entity, UserColumn[]> = {
  contract: [
    { entityType: "contract", fieldName: "진행상태", kind: "text", choices: [] },
    { entityType: "contract", fieldName: "메모", kind: "text", choices: [] },
  ],
  notice: [
    { entityType: "notice", fieldName: "검토여부", kind: "text", choices: [] },
    { entityType: "notice", fieldName: "메모", kind: "text", choices: [] },
    // 앞 판(v1)에 저장된 가공열. 읽을 때 v2 로 옮겨 여는지 보는 자리다.
    { entityType: "notice", fieldName: "품명 모음", kind: "computed", choices: [], formula: { version: 1, id: "c0ffee00-0000-4000-8000-000000000001", steps: [
      { op: "pick", fields: ["세부품명"] }, { op: "distinct" }, { op: "format", text: "{세부품명}" }, { op: "join", text: ", " },
    ] } },
  ],
  request: [
    { entityType: "request", fieldName: "확인여부", kind: "text", choices: [] },
    { entityType: "request", fieldName: "메모", kind: "text", choices: [] },
  ],
};

const 모든열 = () => [...userColumns.contract, ...userColumns.notice, ...userColumns.request].map(열내기);

/** 다리가 내는 열 — 그 열을 쓰는 맞춤 뷰 수(viewReaders)가 붙는다. 진짜 다리(Bridge.UserColumns)와 같다. */
const 열내기 = (c: UserColumn): UserColumn => ({ ...c, viewReaders: 뷰가읽는수(c.entityType, c.fieldName) });

/**
 * v_통합차수 가 내는 열. v_통합 끝에 <b>둘만 더한 것</b>이다 — 실제 뷰도 그렇고, 계약면 시험이
 * 그 성질을 붙들고 있다.
 */
const 통합차수열 = [...통합표열, "공고건", "현행공고"];

// ── 차수를 편 표 셋 ─────────────────────────────────────────────
//
// 본 뷰와 <b>열이 한 글자도 다르지 않다</b>. 실제 뷰도 열 한 벌을 나눠 쓰고 계약면 시험이 그
// 성질을 붙들고 있으니, 여기서도 본 뷰의 열 목록을 그대로 가리킨다 — 베껴 적으면 갈린다.
//
// 셋 다 <b>읽기 전용</b>이라 다리처럼 빈 목록을 낸다(Views.ReadOnly). 그래서 아래 열맞추기의
// 시트 목록에도 들지 않는다 — 거기 넣으면 사람 열을 세울 때 editable 이 채워져, 고칠 수 없어야
// 할 표가 조용히 열린다.

sheets["v_공고차수"] = {
  name: "v_공고차수",
  columns: 공고열,
  editable: [],
  correctable: [],
  overrides: {},
  // 최신 표의 마지막 줄이 갈린 건의 현행이다. 그 자리를 셋으로 펴 놓는다.
  rows: [...sheets["v_공고"].rows.slice(0, -1), ...갈린공고],
};

sheets["v_계약차수"] = {
  name: "v_계약차수",
  columns: 계약열,
  editable: [],
  correctable: [],
  overrides: {},
  // 지어낸 이 자료에는 변경계약이 없어 최신 표와 줄이 같다. 없는 것을 지어 넣지 않는다 —
  // 차수 뷰가 늘 더 많은 줄을 낸다고 믿게 만드는 편이 빈 것보다 나쁘다.
  rows: sheets["v_계약"].rows,
};

sheets["v_통합차수"] = {
  name: "v_통합차수",
  columns: 통합차수열,
  editable: [],
  correctable: [],
  overrides: {},
  rows: [
    ...sheets["v_통합"].rows.map((r) =>
      줄(통합차수열, {
        ...r,
        // 갈리지 않은 건이라 공고번호에서 차수만 떼면 그것이 곧 건 이름이다.
        공고건: r.입찰공고번호 ? r.입찰공고번호.slice(0, -4) : "",
        현행공고: r.입찰공고번호,
      }),
    ),
    // 갈린 건은 공고 셋이 나란히 서고 계약·접수 열은 빈 채로 남는다 — 이어진 것이 없어서다.
    ...갈린공고.map((n) =>
      줄(통합차수열, {
        입찰공고번호: n.입찰공고번호,
        공고명: n.공고명,
        공고종류: n.공고종류,
        게시일시: n.게시일시,
        공고건: n.공고건,
        현행공고: n.현행공고,
      }),
    ),
  ],
};

/** 사람 열을 뺀 기계 열과, 사람 열이 끼어드는 자리. 열이 바뀔 때 이 위에 다시 얹는다. */
const 기계열: Record<string, { columns: string[]; at: number }> = {};

for (const [name, sheet] of Object.entries(sheets)) {
  const editable = new Set(sheet.editable);
  const at = sheet.columns.findIndex((c) => editable.has(c));

  기계열[name] = {
    columns: sheet.columns.filter((c) => !editable.has(c)),
    at: at < 0 ? sheet.columns.length : at,
  };
}

/** 그 개체의 사람 열이 서는 시트들. 통합은 계약 쪽이라 계약 시트와 함께 움직인다. */
const 시트 = (entityType: Entity) => ({
  contract: ["v_통합", "v_계약"],
  notice: ["v_공고"],
  request: ["v_접수"],
}[entityType]);

const computedFields: Record<Entity, string[]> = {
  contract: ["품명", "물품분류번호", "물품식별번호", "규격", "수요기관", "단위", "수량", "단가", "금액", "인도조건"],
  notice: ["세부품명", "세부품명번호", "물품식별번호", "규격", "수요기관", "단위", "수량", "추정단가", "납품장소", "인도조건"],
  request: ["세부품명", "세부품명번호", "물품식별번호", "규격", "단위", "수량", "조달요구번호", "단가", "금액", "납품장소", "인도조건"],
};
/**
 * 표본 문서 셋의 품목. 이름·번호·값은 모두 지어낸 것이다. 가는 꼴을 일부러 섞었다 —
 * A 는 겹친 책상 한 줄(고유가 「1행 겹쳐 빠짐」을 보일 자리), B 는 번호가 빈 품목, C 는 넉 줄.
 */
type 품목씨앗 = { 이름: string; 번호: string; 식별: string; 규격: string; 수량: number; 단가: number };
export const 품목표본: 품목씨앗[][] = [
  [
    { 이름: "책상", 번호: "001", 식별: "1000000001", 규격: "A", 수량: 2, 단가: 150_000 },
    { 이름: "책상", 번호: "001", 식별: "1000000001", 규격: "A", 수량: 3, 단가: 150_000 },
    { 이름: "의자", 번호: "002", 식별: "1000000002", 규격: "B", 수량: 5, 단가: 80_000 },
  ],
  [
    { 이름: "의자", 번호: "", 식별: "", 규격: "B", 수량: 4, 단가: 80_000 },
  ],
  [
    { 이름: "모니터", 번호: "003", 식별: "1000000003", 규격: "27형", 수량: 2, 단가: 310_000 },
    { 이름: "키보드", 번호: "004", 식별: "1000000004", 규격: "유선", 수량: 10, 단가: 25_000 },
    { 이름: "마우스", 번호: "005", 식별: "1000000005", 규격: "유선", 수량: 10, 단가: 12_000 },
    { 이름: "책상", 번호: "001", 식별: "1000000001", 규격: "A", 수량: 1, 단가: 150_000 },
  ],
];

/** 표본 품목을 그 자료 종류의 품목 필드로 편다. */
export function 품목행(entity: Entity, set: number): Row[] {
  return 품목표본[set % 품목표본.length].map(s => {
    const 값: Row = {
      품명: s.이름, 세부품명: s.이름, 물품분류번호: s.번호, 세부품명번호: s.번호, 물품식별번호: s.식별, 규격: s.규격,
      수요기관: "가람군수지원단", 단위: "개", 수량: String(s.수량), 단가: 돈(s.단가), 추정단가: 돈(s.단가),
      금액: 돈(s.수량 * s.단가), 조달요구번호: "MPKPLA26900001", 납품장소: "수요처 지정 창고", 인도조건: "지정장소도착도",
    };
    return Object.fromEntries(computedFields[entity].map(f => [f, 값[f] ?? ""]));
  });
}

/** 계약면의 품목 뷰. views.txt 와 같은 열이다. SQL 자동완성의 재료. */
const 품목뷰열: Record<string, string[]> = {
  v_공고품목: ["입찰공고번호", "공고명", "순번", "수요기관", "세부품명", "세부품명번호", "물품식별번호", "규격", "수량", "단위", "추정단가", "납품기한", "납품장소", "인도조건", "담당부서", "담당자", "담당자전화", "담당자팩스"],
  v_계약품목: ["계약번호", "계약건명", "순번", "품명", "규격", "물품분류번호", "물품식별번호", "수량", "단위", "단가", "금액", "수요기관", "납품기한", "인도조건"],
  v_접수품목: ["접수번호", "요청명", "순번", "조달요구번호", "국방조달계획년도", "변경차수", "세부품명", "세부품명번호", "물품식별번호", "규격", "검사형태", "취소여부", "수량", "단위", "단가", "금액", "면수", "인도조건", "납품일수", "납품기한", "납품장소", "재고번호", "규격번호", "지출요청번호", "입찰공고번호", "공고순번"],
};
const 종류뷰: Record<Entity, string> = { contract: "v_계약", notice: "v_공고", request: "v_접수" };

/** 원천으로 쓸 수 있는 것. 연결 문서의 사용자 열은 넣지 않는다(§4.1). */
function 쓸것(entity: Entity): ColumnAvailable {
  const others = (["notice", "request", "contract"] as Entity[]).filter(k => k !== entity);
  const schema: Record<string, string[]> = {
    v_통합: 기계열["v_통합"].columns, v_통합차수: 통합차수열,
    v_접수: 기계열["v_접수"].columns, v_공고: 기계열["v_공고"].columns, v_계약: 기계열["v_계약"].columns,
    v_공고차수: 기계열["v_공고"].columns, v_계약차수: 기계열["v_계약"].columns, ...품목뷰열,
  };
  return {
    view: 종류뷰[entity], base: 기계열[종류뷰[entity]].columns, items: computedFields[entity],
    // 공고·접수에서 연결계약은 여러 건이다(§11.1-2) — 진짜 다리의 Flow.EnvFor 와 같다.
    linked: others.map(kind => ({ kind, fields: 기계열[종류뷰[kind]].columns, items: computedFields[kind], ...(kind === "contract" ? { many: true } : {}) })),
    schema,
  };
}
const 재료 = (entity: Entity): FlowEnv => {
  const a = 쓸것(entity);
  return { base: a.base, items: a.items, linked: Object.fromEntries(a.linked.map(l => [l.kind, { fields: l.fields, items: l.items, ...(l.many ? { many: true } : {}) }])) };
};

/** 이 줄과 사람이 이은 문서. 통합 줄을 거쳐 찾는다 — 진짜 앱은 확정된 연결(ADR-004·021)을 따른다. */
function 연결(entity: Entity, row: Row, set: number): EvalInput["linked"] {
  const 키 = { contract: "계약번호", notice: "입찰공고번호", request: "접수번호" } as const;
  const 통합 = sheets["v_통합"].rows.find(r => r[키[entity]] && r[키[entity]] === row[키[entity]]);
  const out: EvalInput["linked"] = {};
  for (const kind of ["notice", "request", "contract"] as Entity[]) {
    if (kind === entity) continue;
    const key = 통합?.[키[kind]];
    const found = key ? sheets[종류뷰[kind]].rows.find(r => r[키[kind]] === key) : undefined;
    out[kind] = found ? { row: found, items: 품목행(kind, set) } : null;
  }
  return out;
}

function columnContextExample(entity: Entity): ColumnContext {
  const spec = 어느것.find(s => s.entityType === entity);
  if (!spec) throw new Error("가공열의 자료 종류가 올바르지 않습니다.");
  const sheet = sheets[spec.view];
  const rows = [...sheet.rows].sort((a, b) => 줄키(a).localeCompare(줄키(b))).slice(0, 5);
  return { columns: sheet.columns, rows, fields: computedFields[entity], available: 쓸것(entity), sources: rows.map((row, i) => {
    const key = 줄키(row);
    const items = 품목행(entity, i);
    return { base: key.slice(0, -(spec.차수길이 + (entity === "contract" ? 0 : 1))), revision: key.slice(-spec.차수길이),
      title: row[spec.제목열] ?? "", rows: items, totalRows: items.length };
  }) };
}

/** 단계마다의 값. 표본 문서 셋을 TS 평가기로 돈다 — 진짜 앱은 같은 식을 SQL 로 돈다(§9). */
type 표본줄 = { row: Row; set: number; sample: ColumnTrace["samples"][number] };
function 추적(entity: Entity, formula: FormulaV2, env: FlowEnv = 재료(entity), link: typeof 연결 = 연결, picks?: 표본줄[]): ColumnTrace {
  const context = columnContextExample(entity);
  const chosen = picks ?? context.rows.slice(0, 3).map((row, i) => {
    const { base, revision, title } = context.sources[i];
    return { row, set: i, sample: { base, revision, title } };
  });
  const samples = chosen.map(p => p.sample);
  if (formula.source === "sql") {
    const issues = checkSqlMock(formula.sql);
    return { samples, sql: formula.sql, final: samples.map(() => ""), note: "시안의 목업은 SQL 을 돌리지 않습니다. 앱에서는 표본에서 실제로 돌려 봅니다.",
      steps: [{ shape: "value", values: samples.map(() => ({ kind: "value", text: "", blank: true })), ...(issues.length ? { error: issues[0].message } : {}) }] };
  }
  const checked = checkFlow(formula.flow, env);
  const runs = chosen.map(({ row, set }) => evaluate(formula.flow, env, { row, items: 품목행(entity, set), linked: link(entity, row, set) }));
  const firstError = checked.errors.find(e => !e.final)?.index ?? runs.find(r => r.error)?.error?.index ?? Infinity;
  const steps = Array.from({ length: formula.flow.steps.length + 1 }, (_, k) => {
    const issue = checked.errors.find(e => e.index === k) ?? runs.find(r => r.error?.index === k)?.error;
    return {
      shape: checked.shapes[k],
      values: runs.map(r => r.values[k] ?? { kind: "value" as const, text: "" }),
      ...(issue ? { error: issue.message } : {}),
      ...(k > firstError ? { blocked: true } : {}),
    };
  });
  return { samples, steps, final: runs.map(r => r.final), sql: compileFlow(formula.flow, env, entity) };
}

// ── 맞춤 뷰(docs/user-views.md) ─────────────────────────────────
// 목업 다리. 정의를 들고 위의 TS 평가기로 미리보기·보표를 낸다 — 진짜 다리(Store 의 맞춤 뷰)와 같은 이름과 꼴이다.
// 한 줄은 조달 건(사슬)이다. 통합 표의 줄이 곧 사슬이고, 덧연결 하나와 통합에 없는 공고·접수가 제 줄로 선다.

const 번호열 = { contract: "계약번호", notice: "입찰공고번호", request: "접수번호" } as const;

/**
 * 한 공고에 계약이 둘인 건(분할 낙찰). 통합 표는 계약마다 한 줄이라 거기 적힌 연결은 1:1 뿐이다 —
 * 「계약 여럿」 과 모으기가 찾을 건을 여기 하나 더 잇는다. 번호는 지어낸 것이다.
 */
const 덧연결: { notice: string; contract: string }[] = [{ notice: "R26BK09017003-000", contract: "R26TA0911050800" }];

/** 조달 건 하나. 문서가 없는 자리는 null 이다. set 은 그 건의 품목 표본 차례다. */
type 사슬 = { key: string; request: Row | null; notice: Row | null; contract: Row | null; set: number };
/** 맞춤 뷰의 한 줄 — 계약마다 한 줄이면 사슬 하나, 모으기면 그 건의 사슬 모두(계약번호 차례). */
type 뷰줄 = { key: string; chains: 사슬[] };

const 문서들: Entity[] = ["request", "notice", "contract"];
const 사슬열쇠 = (c: Omit<사슬, "key" | "set">) =>
  `${c.request?.접수번호 ?? ""}|${c.notice?.입찰공고번호 ?? ""}|${c.contract?.계약번호 ?? ""}`;

/** 그 문서의 면 뷰 한 줄. 통합 줄이 가리키는데 면 뷰에 없으면(지어낸 자료의 빈 곳) 통합 줄을 그 문서로 읽는다. */
function 문서(kind: Entity, key: string | undefined, fallback?: Row): Row | null {
  if (!key) return null;
  return sheets[종류뷰[kind]].rows.find(r => r[번호열[kind]] === key) ?? fallback ?? null;
}

/** 모든 조달 건 — 진짜 다리의 Views.사슬 과 같은 규칙(공고가 선 건 · 공고 없는 접수 · 매달리지 못한 계약)을 지어낸 자료로 편다. */
function 사슬들(): 사슬[] {
  const out: Omit<사슬, "key" | "set">[] = sheets["v_통합"].rows.map(r => ({
    request: 문서("request", r.접수번호, r), notice: 문서("notice", r.입찰공고번호, r), contract: 문서("contract", r.계약번호, r),
  }));
  for (const d of 덧연결) {
    const notice = 문서("notice", d.notice);
    out.push({ request: out.find(c => c.notice === notice)?.request ?? null, notice, contract: 문서("contract", d.contract) });
  }
  for (const kind of ["notice", "request"] as const)
    for (const row of sheets[종류뷰[kind]].rows)
      if (!out.some(c => c[kind]?.[번호열[kind]] === row[번호열[kind]]))
        out.push({ request: kind === "request" ? row : null, notice: kind === "notice" ? row : null, contract: null });
  return out.map((c, set) => ({ ...c, key: 사슬열쇠(c), set }));
}

/** 접수·공고의 건 — 공고가 있는 사슬은 (접수, 공고)로 묶는다. 공고가 없으면 사슬 하나가 제 건이다. */
const 건열쇠 = (c: 사슬) => c.notice ? `${c.request?.접수번호 ?? ""}|${c.notice.입찰공고번호}|` : c.key;
const 계약번호차례 = (a: 사슬, b: 사슬) => (a.contract?.계약번호 ?? "").localeCompare(b.contract?.계약번호 ?? "");

/** 흐름식이 읽는 문서 종류 — 원천 linked 와 단계가 읽는 행의 열(공고.공고명). 진짜 다리의 UserViewBuilder.Kinds 와 같다. */
const 행열단계 = new Set(["ifEmpty", "add", "sub", "mul", "div", "dateDiff"]);
function 읽는종류(formula: unknown): Set<Entity> {
  const out = new Set<Entity>();
  let f: FormulaV2;
  try { f = toV2({ id: "", ...(formula as object) }); } catch { return out; }
  if (f.source !== "flow") return out;
  if (f.flow.from.kind === "linked") out.add(f.flow.from.link);
  for (const s of f.flow.steps) {
    const q = s.field !== undefined && 행열단계.has(s.op) ? qualified(s.field) : null;
    if (q) out.add(q.link);
    for (const c of clausesOf(s)) { const k = qualified(c.field); if (k) out.add(k.link); }
  }
  return out;
}

/** 그 뷰의 줄 — 거르기(담긴 흐름식이 읽는 종류가 하나도 없는 건은 빠진다)와 모으기 뒤. */
function 뷰줄들(columns: { formula: unknown }[], fold: boolean): 뷰줄[] {
  const kinds = new Set(columns.flatMap(c => [...읽는종류(c.formula)]));
  const rows: 뷰줄[] = [];
  const groups = new Map<string, 뷰줄>();
  for (const c of 사슬들()) {
    const key = fold && c.notice ? 건열쇠(c) : c.key;
    const at = groups.get(key);
    if (at) { at.chains.push(c); continue; }
    const row = { key, chains: [c] };
    groups.set(key, row);
    rows.push(row);
  }
  for (const r of rows) {
    r.chains.sort(계약번호차례);
    // 계약이 하나뿐인 건은 계약마다 한 줄과 같은 열쇠다(진짜 다리도 그 계약을 들고 간다).
    if (r.chains.length === 1) r.key = r.chains[0].key;
  }
  return !kinds.size ? rows : rows.filter(r => [...kinds].some(k => r.chains.some(c => !!c[k])));
}

/** 계약이 여럿인 건 수. */
const 계약여럿 = () => 뷰줄들([], true).filter(r => r.chains.filter(c => c.contract).length >= 2).length;

/** 접수·공고·계약의 사람 열·가공열(§3). 맞춤 뷰는 면 뷰 위층이라 읽을 수 있다. */
const 자기열 = (kind: Entity) => userColumns[kind].map(c => c.fieldName).filter(n => !기계열[종류뷰[kind]].columns.includes(n));
const 문서열 = (kind: Entity) => [...기계열[종류뷰[kind]].columns, ...자기열(kind)];
const 뷰재료 = (): FlowEnv => ({
  base: [], items: [], view: true,
  linked: Object.fromEntries(문서들.map(k => [k, { fields: 문서열(k), items: computedFields[k] }])),
});

function 뷰맥락(): UserViewContext {
  const available = 쓸것("notice");
  return {
    columns: [], rows: [], fields: [], sources: [],
    available: { ...available, view: "", base: [], items: [], linked: 문서들.map(kind => ({ kind, fields: 문서열(kind), items: computedFields[kind] })) },
    total: 사슬들().length, manyContracts: 계약여럿(),
  };
}

/** 사슬 하나를 평가기 입력으로 — 행의 열은 「공고.공고명」 꼴로 펴고, 문서마다 그 품목을 단다. */
function 사슬입력(c: 사슬): EvalInput {
  const row: Row = {};
  const linked: EvalInput["linked"] = {};
  for (const kind of 문서들) {
    const doc = c[kind];
    for (const f of 문서열(kind)) row[`${docWord[kind]}.${f}`] = doc?.[f] ?? "";
    linked[kind] = doc ? { row: doc, items: 품목행(kind, c.set) } : null;
  }
  return { row, items: [], linked };
}

/** 줄 하나에 찍힐 값. 모으기에서 계약을 읽는 필드는 계약마다 셈해 계약번호 차례로 ", " 로 잇는다. */
function 줄값(flow: Flow, env: FlowEnv, r: 뷰줄, fold: boolean): string {
  const each = (c: 사슬) => evaluate(flow, env, 사슬입력(c)).final;
  const many = r.chains.filter(c => c.contract);
  if (fold && many.length >= 2 && 읽는종류({ version: 2, source: "flow", flow }).has("contract")) return many.map(each).join(", ");
  return each(r.chains[0]);
}

const 보이는번호 = (r: 뷰줄) => {
  const c = r.chains[0], one = r.chains.length === 1;
  return c.notice?.입찰공고번호 || c.request?.접수번호 || (one ? c.contract?.계약번호 ?? "" : "");
};
const 보이는제목 = (r: 뷰줄) => {
  const c = r.chains[0];
  return c.notice?.공고명 || c.request?.요청명 || c.contract?.계약건명 || "";
};
const 표본꼴 = (r: 뷰줄): UserViewSample => ({ key: r.key, number: 보이는번호(r), title: 보이는제목(r) });

type 뷰가장자리 = "manyContracts" | "unlinked";
type 뷰꼴 = { name: string; definition: { version: 2; contracts?: string; columns: { id?: string; name: string; formula: unknown }[] } };
const 모으기 = (view: 뷰꼴) => {
  const c = view.definition?.contracts ?? "split";
  if (c !== "split" && c !== "fold") throw new Error(`계약이 여럿인 건의 처리가 올바르지 않습니다: ${c}`);
  return c === "fold";
};

function 뷰표본(view: 뷰꼴, query: string, edge: 뷰가장자리 | null): UserViewSample[] {
  if (edge && edge !== "manyContracts" && edge !== "unlinked") throw new Error(`모르는 표본 가장자리입니다: ${edge}`);
  const q = query.trim().toLocaleLowerCase();
  const fold = 모으기(view);
  const all = 뷰줄들(view.definition.columns, fold);
  return all.filter(r => {
    const s = 표본꼴(r);
    if (q && !s.number.toLocaleLowerCase().includes(q) && !s.title.toLocaleLowerCase().includes(q)) return false;
    const c = r.chains[0];
    const contracts = fold ? r.chains.filter(x => x.contract).length
      : 사슬들().filter(x => c.notice && x.notice === c.notice && x.contract).length;
    if (edge === "manyContracts" && contracts < 2) return false;
    if (edge === "unlinked" && c.request && c.notice && (fold ? contracts > 0 : !!c.contract)) return false;
    return true;
  }).map(표본꼴);
}

/** 필드마다 그 뷰의 줄을 돌아 이 표본의 값과 빈 값 수를 낸다. 진짜 다리는 같은 SELECT 를 읽기 전용 연결에서 돌린다. */
function 뷰미리보기(view: 뷰꼴, sampleKey: string): UserViewPreview {
  const fold = 모으기(view);
  const rows = 뷰줄들(view.definition.columns, fold);
  const env = 뷰재료();
  const at = rows.findIndex(r => r.key === sampleKey);
  return {
    sample: at < 0 ? null : 표본꼴(rows[at]),
    total: rows.length,
    columns: view.definition.columns.map(c => {
      const id = c.id ?? "";
      const f = toV2({ id: "", ...(c.formula as object) });
      // 목업은 SQL 을 돌리지 않는다 — 값은 비워 두고 깨진 것으로 치지 않는다. 앱에서는 표본에서 실제로 돌린다.
      if (f.source === "sql") return { id, value: "", blank: 0 };
      const checked = checkFlow(f.flow, env);
      if (!checked.ok) return { id, value: "", blank: 0, error: checked.errors[0].message };
      const values = rows.map(r => 줄값(f.flow, env, r, fold));
      return { id, value: at < 0 ? "" : values[at], blank: values.filter(v => v === "").length };
    }),
  };
}

/**
 * 필드 하나의 보표 — 표본은 준 열쇠(미리보기가 보는 표본이 처음)이고, 없으면 처음 조달 건 셋이다. 단계는 계약 한 건을 본다 —
 * 모은 건이면 계약번호가 가장 앞선 계약이다. 열에 찍힐 값은 뷰가 낼 값(모았으면 이어 붙인 것)이다.
 */
function 뷰추적(formula: FormulaV2, keys: string[] | null | undefined): ColumnTrace {
  const env = 뷰재료();
  const split = 뷰줄들([], false), folded = 뷰줄들([], true);
  const picks = (keys?.length ? keys : split.slice(0, 3).map(r => r.key)).flatMap(key => {
    const inSplit = split.find(r => r.key === key);
    const row = inSplit ?? folded.find(r => r.key === key);
    return row ? [{ row, fold: !inSplit }] : [];
  });
  const items = formula.source === "flow" && formula.flow.from.kind === "linked" ? formula.flow.from.link : null;
  const samples = picks.map(({ row }) => ({ base: 보이는번호(row), revision: "", title: 보이는제목(row),
    rows: items && row.chains[0][items] ? 품목행(items, row.chains[0].set).length : 0 }));
  if (formula.source === "sql") {
    const issues = checkSqlMock(formula.sql);
    return { samples, sql: formula.sql, final: samples.map(() => ""), note: "시안의 목업은 SQL 을 돌리지 않습니다. 앱에서는 표본에서 실제로 돌려 봅니다.",
      steps: [{ shape: "value", values: samples.map(() => ({ kind: "value", text: "", blank: true })), ...(issues.length ? { error: issues[0].message } : {}) }] };
  }
  const checked = checkFlow(formula.flow, env);
  const runs = picks.map(({ row }) => evaluate(formula.flow, env, 사슬입력(row.chains[0])));
  const firstError = checked.errors.find(e => !e.final)?.index ?? runs.find(r => r.error)?.error?.index ?? Infinity;
  const steps = Array.from({ length: formula.flow.steps.length + 1 }, (_, k) => {
    const issue = checked.errors.find(e => e.index === k) ?? runs.find(r => r.error?.index === k)?.error;
    return {
      shape: checked.shapes[k],
      values: runs.map(r => r.values[k] ?? { kind: "value" as const, text: "" }),
      ...(issue ? { error: issue.message } : {}),
      ...(k > firstError ? { blocked: true } : {}),
    };
  });
  const final = checked.ok ? picks.map(({ row, fold }) => 줄값(formula.flow, env, row, fold)) : picks.map(() => "");
  return { samples, steps, final, sql: compileFlow(formula.flow, env, "notice") };
}

const 뷰열 = (id: string, name: string, flow: Flow): UserViewColumn => ({ id, name, formula: { version: 2, id: "", source: "flow", flow } });
const 문서필드 = (link: Entity, field: string): Flow => ({ from: { kind: "linked", link, field }, steps: [] });
const 뷰씨앗 = (): UserView[] => [
  { id: "uv-0001", name: "공고문서", broken: {}, definition: { version: 2, contracts: "fold", columns: [
    뷰열("c-0001", "접수번호", 문서필드("request", "접수번호")),
    뷰열("c-0002", "공고번호", 문서필드("notice", "입찰공고번호")),
    뷰열("c-0003", "계약번호", 문서필드("contract", "계약번호")),
    뷰열("c-0004", "공고명", 문서필드("notice", "공고명")),
    뷰열("c-0005", "대표품명", { from: { kind: "linked", link: "notice", items: true }, steps: [{ op: "get", field: "세부품명" }, { op: "distinct" }, { op: "abbrev", n: 1, text: "{앞} 외 {나머지}종" }] }),
    뷰열("c-0006", "진행", 문서필드("notice", "검토여부")),
  ] } },
  // 깨진 필드 하나 — 계약의 사람 열 이름이 바뀐 뒤 그 열을 읽던 식(§5). 목록과 줄에서 그 까닭이 보이는 자리다.
  { id: "uv-0002", name: "계약서류", broken: {}, definition: { version: 2, contracts: "split", columns: [
    뷰열("c-0101", "계약번호", 문서필드("contract", "계약번호")),
    뷰열("c-0102", "계약건명", 문서필드("contract", "계약건명")),
    뷰열("c-0103", "계약일자", 문서필드("contract", "계약일자")),
    뷰열("c-0104", "계약금액", 문서필드("contract", "계약금액")),
    뷰열("c-0105", "계약상대자", 문서필드("contract", "계약상대자")),
    뷰열("c-0106", "공고번호", 문서필드("notice", "입찰공고번호")),
    뷰열("c-0107", "접수번호", 문서필드("request", "접수번호")),
    뷰열("c-0108", "품목 수", { from: { kind: "linked", link: "contract", items: true }, steps: [{ op: "count" }] }),
    뷰열("c-0109", "진행", 문서필드("contract", "진행단계")),
  ] } },
  // 서식용 예시 — 진짜 작업자료가 처음부터 들고 서는 그 보기(exe 에 실린 같은 파일 한 벌).
  ...(JSON.parse(예시보기) as ViewPackage).views.map((v, i): UserView => ({
    id: `uv-01${String(i).padStart(2, "0")}`, name: v.name, broken: {}, definition: structuredClone(v.definition) })),
];

/**
 * 흐름식에서 kind 문서의 열 field 를 읽는 자리를 찾고, to 가 있으면 고친다 — 원천 linked(kind) field, 단계가 읽는 「공고.f」,
 * 원천이 그 문서 한 행일 때 첫 단계가 짚는 필드. 진짜 다리의 UserViewBuilder.Touch 와 같다.
 */
function 열자리(formula: unknown, kind: Entity, field: string, to?: string): boolean {
  const f = formula as { source?: string; flow?: { from?: { kind?: string; link?: string; field?: string; items?: boolean }; steps?: { op?: string; field?: string; text?: string; cond?: { field: string }[] }[] } } | null;
  if (f?.source !== "flow" || !f.flow) return false;
  let found = false;
  const from = f.flow.from, steps = f.flow.steps ?? [];
  const linked = from?.kind === "linked" && from.link === kind;
  if (linked && from!.field === field) { found = true; if (to !== undefined) from!.field = to; }
  const word = `${docWord[kind]}.`;
  for (const s of steps) {
    if (s.op && 행열단계.has(s.op) && s.field === word + field) { found = true; if (to !== undefined) s.field = word + to; }
    // 값 하나를 견주는 조건의 마디도 그 자리를 읽는다(만약 공고.f = …).
    for (const c of clausesOf(s)) if (c.field === word + field) { found = true; if (to !== undefined) c.field = word + to; }
  }
  const first = steps[0];
  if (linked && from!.field === undefined && !from!.items && first) {
    if (first.op === "get" && first.field === field) { found = true; if (to !== undefined) first.field = to; }
    if (first.op === "if") for (const c of first.cond ?? []) if (c.field === field) { found = true; if (to !== undefined) c.field = to; }
    if (first.op === "format" && (first.text ?? "").includes(`{${field}}`)) { found = true; if (to !== undefined) first.text = first.text!.split(`{${field}}`).join(`{${to}}`); }
  }
  return found;
}
/** 그 종류의 사람 열을 쓰는 맞춤 뷰 수 — 열을 지우는 확인 문구가 센다. */
function 뷰가읽는수(kind: Entity, name: string): number {
  return 사용자뷰.filter(v => v.definition.columns.some(c => 열자리(c.formula, kind, name))).length;
}
/** 사람 열 이름이 바뀌면 그 열을 읽던 맞춤 뷰 필드를 새 이름으로 고쳐 쓴다. */
function 뷰열이름바꾸기(kind: Entity, from: string, to: string) {
  for (const v of 사용자뷰) for (const c of v.definition.columns) 열자리(c.formula, kind, from, to);
}

/** 가짜 다리의 맞춤 뷰. 시험이 사용자뷰초기화() 로 처음 꼴로 되돌린다. */
const 사용자뷰: UserView[] = 뷰씨앗();
/** Test seam: real browser mode downloads/opens JSON; tests can supply a chosen fixture. */
export const 보기구성파일 = { picked: null as { name: string; json: string } | null, exported: null as string | null };

function 구성내보내기(ids: string[]): ViewPackage {
  const chosen = 사용자뷰.filter(v => ids.includes(v.id));
  if (!chosen.length || new Set(ids).size !== chosen.length) throw new Error("내보낼 보기를 하나 이상 고르세요.");
  const all = Object.values(userColumns).flat();
  const needed = new Set<UserColumn>();
  const formulas = chosen.flatMap(v => v.definition.columns.map(c => c.formula));
  let sql = false;
  for (let i = 0; i < formulas.length; i++) {
    const f = formulas[i];
    if (f.source === "sql") sql = true;
    for (const c of all) if (!needed.has(c) && (f.source === "sql" || 열자리(f, c.entityType, c.fieldName))) {
      needed.add(c);
      if (c.kind === "computed" && c.formula) formulas.push(toV2(c.formula));
    }
  }
  return { format: "pclm-view-config", version: 1, includesAllColumnsForSql: sql,
    views: structuredClone(chosen.map(v => ({ name: v.name, definition: v.definition }))),
    columns: structuredClone(all.filter(c => needed.has(c)).map(c => ({ entityType: c.entityType, fieldName: c.fieldName, kind: c.kind, choices: c.choices,
      ...(c.kind === "computed" && c.formula ? { formula: toV2(c.formula) } : {}) }))) };
}

function 구성미리보기(json: string, applyToken?: string): ViewPackagePreview {
  if (json.length > 2_000_000) throw new Error("보기 구성 파일은 2MB까지 가져올 수 있습니다.");
  const p = JSON.parse(json) as ViewPackage;
  if (p.format !== "pclm-view-config" || p.version !== 1 || !Array.isArray(p.views) || !p.views.length || !Array.isArray(p.columns)) throw new Error("지원하는 보기 구성 파일이 아닙니다(구성 판 1).");
  const oldColumns = structuredClone(userColumns), oldViews = structuredClone(사용자뷰);
  const preview: ViewPackagePreview = { views: [], columns: [], errors: [], includesAllColumnsForSql: p.includesAllColumnsForSql, token: "" };
  const normalized = (c: Pick<UserColumn, "kind" | "choices" | "formula">) => JSON.stringify({ kind: c.kind, choices: c.kind === "choice" ? c.choices : [],
    formula: c.kind === "computed" && c.formula ? { ...toV2(c.formula), id: "" } : null });
  try {
    const seen = new Set<string>();
    for (const c of p.columns) {
      try {
        if (!(["notice", "contract", "request"] as string[]).includes(c.entityType) || !c.fieldName?.trim() || c.fieldName.length > 20 || /['"\\\x00-\x1f]/.test(c.fieldName) || !Array.isArray(c.choices)) throw new Error("맞춤 열 형식이 올바르지 않습니다.");
        const key = `${c.entityType}.${c.fieldName}`;
        if (seen.has(key)) throw new Error("구성 안에서 열이 겹칩니다.");
        seen.add(key);
        if (c.kind === "computed") {
          if (!c.formula) throw new Error("가공식이 없습니다.");
          const f = toV2(c.formula);
          const problem = f.source === "sql" ? checkSqlMock(f.sql)[0] : checkFlow(f.flow, 재료(c.entityType)).errors[0];
          if (problem) throw new Error(problem.message);
        } else if (!["text", "choice", "date", "number"].includes(c.kind) || c.formula) throw new Error("입력열 형식이 올바르지 않습니다.");
        const old = userColumns[c.entityType].find(x => x.fieldName.toLowerCase() === c.fieldName.toLowerCase());
        if (old && (old.fieldName !== c.fieldName || normalized(old) !== normalized(c))) {
          preview.columns.push({ entityType: c.entityType, fieldName: c.fieldName, action: "conflict" });
          const kind = { request: "접수", notice: "공고", contract: "계약" }[c.entityType as "request" | "notice" | "contract"];
          preview.errors.push(`내 ${kind} 열 「${old.fieldName}」 ${받침(old.fieldName) ? "과" : "와"} 정의가 다릅니다. 열 관리에서 내 열 이름을 바꾸면 이 구성을 가져올 수 있습니다. 보낸 사람에게 열 이름을 바꿔 다시 내보내 달라고 해도 됩니다.`);
          continue;
        }
        if (!old && 쓰이는가(c.entityType, c.fieldName)) throw new Error("표가 이미 쓰는 이름입니다.");
        preview.columns.push({ entityType: c.entityType, fieldName: c.fieldName, action: old ? "reuse" : "add" });
        if (!old) userColumns[c.entityType].push(structuredClone({ ...c,
          ...(c.kind === "computed" && c.formula ? { formula: { ...toV2(c.formula), id: crypto.randomUUID() } } : {}) }));
      } catch (e) { preview.errors.push(`${c.entityType}.${c.fieldName}: ${(e as Error).message}`); }
    }
    for (const v of p.views) {
      try {
        if (v.definition.version !== 2 || !["split", "fold"].includes(v.definition.contracts) || !v.name?.trim() || v.name.length > 40) throw new Error("보기 정의 형식이 올바르지 않습니다.");
        let name = v.name.trim();
        for (let i = 1; 사용자뷰.some(x => x.name.toLowerCase() === name.toLowerCase()); i++) {
          const suffix = i === 1 ? " (복사본)" : ` (복사본 ${i})`;
          name = v.name.slice(0, 40 - suffix.length) + suffix;
        }
        뷰저장({ ...v, name }, null);
        preview.views.push({ name: v.name, importName: name });
      } catch (e) { preview.errors.push(`${v.name}: ${(e as Error).message}`); }
    }
    // Token binds the displayed plan; the backend additionally validates real SQLite execution.
    preview.token = JSON.stringify({ p, views: preview.views, columns: preview.columns, errors: preview.errors });
    if (applyToken !== undefined) {
      if (preview.errors.length) throw new Error(preview.errors.join("\n"));
      if (preview.token !== applyToken) throw new Error("미리보기 뒤 구성이 바뀌었습니다. 파일을 다시 골라 확인하세요.");
      for (const e of ["notice", "contract", "request"] as Entity[]) 열맞추기(e);
      return preview;
    }
    return preview;
  } finally {
    if (applyToken === undefined || preview.errors.length || preview.token !== applyToken) {
      for (const e of ["notice", "contract", "request"] as Entity[]) userColumns[e].splice(0, userColumns[e].length, ...oldColumns[e]);
      사용자뷰.splice(0, 사용자뷰.length, ...oldViews);
    }
  }
}

async function 구성파일고르기(): Promise<ViewPackagePick | null> {
  let picked = 보기구성파일.picked;
  if (!picked) picked = await new Promise<{ name: string; json: string } | null>((resolve, reject) => {
    const input = document.createElement("input"); input.type = "file"; input.accept = ".json,application/json";
    input.oncancel = () => resolve(null);
    input.onchange = () => { const file = input.files?.[0]; if (!file) resolve(null); else file.text().then(json => resolve({ name: file.name, json })).catch(reject); };
    input.click();
  });
  return picked ? { ...picked, preview: 구성미리보기(picked.json) } : null;
}
export function 사용자뷰초기화() { 사용자뷰.splice(0, 사용자뷰.length, ...뷰씨앗()); }

function 뷰목록(): UserView[] {
  const env = 뷰재료();
  return 사용자뷰.map(v => {
    const broken: Record<string, string> = {};
    for (const c of v.definition.columns) {
      if (c.formula.source !== "flow") continue;
      const problem = checkFlow(c.formula.flow, env).errors[0];
      if (problem) broken[c.id] = problem.message;
    }
    return structuredClone({ ...v, broken });
  });
}

function 뷰저장(view: 뷰꼴, previousId: string | null): UserView[] {
  const name = view.name.trim();
  if (!name) throw new Error("뷰 이름이 비었습니다.");
  const fold = 모으기(view);
  if (!view.definition.columns.length) throw new Error("필드를 하나 이상 추가하세요.");
  if (사용자뷰.some(v => v.name.toLowerCase() === name.toLowerCase() && v.id !== previousId)) throw new Error(`‘${name}’ 맞춤 뷰가 이미 있습니다.`);
  const env = 뷰재료();
  const seen = new Set<string>();
  const columns = view.definition.columns.map((c): UserViewColumn => {
    const col = c.name.trim();
    if (!col) throw new Error("이름이 빈 필드가 있습니다.");
    if (seen.has(col.toLowerCase())) throw new Error(`필드 이름이 겹칩니다: ${col}`);
    seen.add(col.toLowerCase());
    const f = toV2({ id: "", ...(c.formula as object) });
    const problem = f.source === "flow" ? checkFlow(f.flow, env).errors[0] : checkSqlMock(f.sql)[0];
    if (problem) throw new Error(`‘${col}’ 필드를 세울 수 없습니다: ${problem.message}`);
    return { id: c.id || `c-${crypto.randomUUID()}`, name: col, formula: f };
  });
  const saved: UserView = { id: previousId ?? `uv-${crypto.randomUUID()}`, name, broken: {},
    definition: { version: 2, contracts: fold ? "fold" : "split", columns } };
  const at = 사용자뷰.findIndex(v => v.id === previousId);
  if (at >= 0) 사용자뷰.splice(at, 1, saved);
  else 사용자뷰.push(saved);
  return 뷰목록();
}

/** 표에 찍을 값. 가공열은 값을 저장하지 않는다 — 읽을 때마다 식이 낸다. */
function 가공값(c: UserColumn, entity: Entity, row: Row, i: number): string {
  if (!c.formula) return "";
  const f = toV2(c.formula);
  if (f.source === "sql") return "";
  return evaluate(f.flow, 재료(entity), { row, items: 품목행(entity, i), linked: 연결(entity, row, i) }).final;
}

function 열맞추기(entityType: Entity) {
  const names = userColumns[entityType].map((c) => c.fieldName);

  for (const key of 시트(entityType)) {
    const sheet = sheets[key];
    const base = 기계열[key];

    sheet.columns = [...base.columns.slice(0, base.at), ...names, ...base.columns.slice(base.at)];
    sheet.editable = userColumns[entityType].filter(c => c.kind !== "computed").map(c => c.fieldName);

    sheet.rows.forEach((row, i) => {
      for (const c of userColumns[entityType]) {
        // 굳힌 입력열은 식을 비활성으로 들고 있다 — 값은 굳힐 때 적힌 그대로다.
        if (c.kind === "computed" && c.formula) row[c.fieldName] = 가공값(c, entityType, row, i);
        else row[c.fieldName] ??= "";
      }
    });
  }
}

// 처음부터 가공열이 서 있는 공고 표 — 위의 v1 열을 얹는다.
열맞추기("notice");

/** 표가 이미 쓰는 이름인가. 진짜 앱은 뷰의 열을 읽어 견주는데, 여기서는 기계 열로 견준다. */
function 쓰이는가(entityType: Entity, name: string) {
  return 시트(entityType).some((key) => 기계열[key].columns.includes(name));
}

function 열고치기(entityType: Entity, work: () => void): Promise<unknown> {
  try {
    work();
    열맞추기(entityType);
    return Promise.resolve(모든열());
  } catch (e) {
    return Promise.reject(e as Error);
  }
}

function 후보(packed: string | null): string[] {
  return packed ? packed.split("|").filter((c) => c.length > 0) : [];
}

let settings: Settings = {
  submitterName: "홍길동",
  planPath: "C:\\Users\\사람\\Documents\\2026년 조달계획.xlsx",
};

/**
 * 구조 보기용. 사슬 여섯으로 <b>빠질 수 있는 자리를 모두</b> 한 번씩 세운다 —
 * 세 칸이 다 찬 것, 계약만 아직 없는 것, 접수 없이 공고에 계약이 둘 달린 것,
 * 접수만 들어온 것, 어디에도 매달리지 못한 계약, 그리고 <b>취소 뒤 재공고로 본번호가
 * 갈린 건</b>. 마지막 것이 없으면 가운데 칸이 공고가 아니라 건이라는 사실이 가짜 다리로는
 * 한 번도 드러나지 않는다.
 */
/**
 * 구조 보기에서 본 <b>갈린 건</b>. 표가 낸 것(<code>갈린공고</code>)과 같은 건이라 이름과
 * 현행을 같은 상수에서 받는다 — 두 눈이 서로 다른 건을 세면 가짜 다리 안에서만 앞뒤가 맞고,
 * 진짜 다리로 갈아 끼우는 날 어긋난다.
 *
 * <p>본문에 서는 것은 살아 있는 재공고이고, 취소된 원공고는 대체된 것으로 물러난다.</p>
 */
const 갈린건칸: OutlineNoticeGroup = {
  // 건의 이름은 가장 이른 본번호다. 화면은 이것을 그리지 않는다.
  groupBase: 갈린건,
  current: {
    key: "R26BK09012082",
    number: 갈린건의현행,
    title: "26년 가람 조립대 구매",
    postedAt: "2026/07/25 09:30:00",
    agency: "국방부조달본부",
    estimatedPrice: "38,720,000",
    revisions: ["000"],
  },
  superseded: [
    {
      key: 갈린건,
      number: `${갈린건}-001`,
      title: "26년 가람 조립대 구매",
      postedAt: "2026/07/09 10:00:00",
      agency: "국방부조달본부",
      estimatedPrice: "38,720,000",
      revisions: ["000", "001"],
    },
  ],
};

const 갈린건접수: OutlineRequest = {
  key: "MPKPLA26910450", // gitleaks:allow (목업 데이터)
  number: "MPKPLA26910450-000",
  title: "26년가람조립대구매",
  receivedOn: "2026/06/24",
  goodsAmount: "38,720,000",
  budgetAmount: "39,281,400",
  demandAgency: "가람군수지원단",
  requestNumbers: ["MPKPLA26910450"],
  revisions: ["000"],
};

const 갈린건계약: OutlineContract = {
  key: "R26TA09080469",
  number: "R26TA0908046900",
  title: "26년 가람 조립대 조달",
  contractedOn: "2026/07/30",
  amount: "37,400,000",
  counterparty: "㈜한국분석기기",
  demandAgency: "가람군수지원단",
  revisions: ["00"],
  items: [],
};

const outline: Outline = {
  chains: [
    { key: "MPKPLA26910301", request: 접수(1), notice: 공고건(0), contracts: [계약(0, 0)] },
    { key: "MPKPLA26910286", request: 접수(0), notice: 공고건(1), contracts: [] },
    { key: "R26BK09017003", request: null, notice: 공고건(2), contracts: [계약(2, 0), 계약(2, 1)] },
    { key: "MPKPLA26910412", request: 접수(2), notice: null, contracts: [] },
    { key: "R26TA09110804", request: null, notice: null, contracts: [계약(4, 0)] },
    {
      key: `MPKPLA26910450/${갈린건}`,
      request: 갈린건접수,
      notice: 갈린건칸,
      contracts: [갈린건계약],
    },
  ],
};

/** 갈리지 않은 건. 공고 하나가 그대로 제 건이라 대체된 것이 없다. */
function 공고건(i: number): OutlineNoticeGroup {
  const notice = 공고칸(i);
  return { groupBase: notice.key, current: notice, superseded: [] };
}

function 접수(i: number): OutlineRequest {
  const r = 접수씨앗[i];

  return {
    key: r.base,
    number: `${r.base}-000`,
    title: r.요청명,
    receivedOn: r.접수일자,
    goodsAmount: r.품대,
    budgetAmount: r.예산금액,
    demandAgency: r.기관,
    requestNumbers: r.요구번호,
    revisions: ["000"],
  };
}

function 공고칸(i: number): OutlineNotice {
  const s = 씨앗[i % 씨앗.length];
  const n = String(i + 1).padStart(2, "0");

  return {
    key: `R26BK090170${n}`,
    number: `R26BK090170${n}-00${i === 3 ? "1" : "0"}`,
    title: s.건명,
    postedAt: "2026/07/02 16:59:28",
    agency: "국방부조달본부",
    estimatedPrice: 돈(Math.round(s.금액 * 1.1)),
    revisions: i === 3 ? ["000", "001"] : ["000"],
  };
}

function 계약(i: number, k: number): OutlineContract {
  const s = 씨앗[i % 씨앗.length];
  const n = `R26TA0911${String(90 + i * 2 + k).padStart(2, "0")}${String(i).padStart(2, "0")}`;

  return {
    key: n,
    number: `${n}0${k}`,
    title: `${s.건명}${k > 0 ? " (2차분)" : ""}`,
    contractedOn: `2026/08/${String((i % 27) + 1).padStart(2, "0")}`,
    amount: 돈(Math.round(s.금액 / (k + 1))),
    counterparty: "㈜한국분석기기",
    demandAgency: s.기관,
    revisions: k > 0 ? ["00", "01"] : ["00"],
    items: [1, 2].map((line) => ({
      lineNo: line,
      name: s.품명,
      specification: "별첨 규격서에 따름",
      quantity: String(line),
      unit: "대",
      unitPrice: 돈(Math.round(s.금액 / 2)),
      amount: 돈(Math.round(s.금액 / 2) * line),
    })),
  };
}

const summary: Summary = {
  requestRows: 3, requestBases: 3,
  noticeRows: 5, noticeBases: 5, contractRows: 42, contractBases: 42,
  counterparties: 3,
  unlinkedContracts: 3, unlinkedRequests: 1,
};

/** 실제 미리보기 표의 문서를 같은 분모로 묶는다. 실제 분류는 Core Reports.Dashboard가 소유한다. */
function dashboardFixture(): DashboardReport {
  const labels: [DashboardStage, string][] = [["requestOnly", "공고 대기"], ["open", "공고중"], ["awaitingContract", "계약 대기"], ["contracted", "계약 완료"]];
  const requests = sheets["v_접수"].rows, notices = sheets["v_공고"].rows, contracts = sheets["v_계약"].rows;
  const linked = sheets["v_통합"].rows;
  const usedRequests = new Set<string>(), usedContracts = new Set<string>();
  const make = (key: string, request: Row | undefined, notice: Row | undefined, captured: Row[]): DashboardRow => {
    if (request) usedRequests.add(request.접수번호);
    captured.forEach(contract => usedContracts.add(contract.계약번호));
    const items = captured.map(contract => ({ number: contract.계약번호, title: contract.계약건명, date: contract.계약일자,
      amount: contract.계약금액 ? Number(contract.계약금액.replaceAll(",", "")) : null, draft: !contract.계약일자, terminated: false }));
    const known = items.flatMap(contract => contract.amount === null ? [] : [contract.amount]);
    const stage: DashboardStage = items.some(contract => contract.draft) ? "awaitingContract" : items.length ? "contracted" : notice ? "awaitingContract" : "requestOnly";
    return { key, title: request?.요청명 || notice?.공고명 || items[0]?.title || "건명 미확인", stage,
      requestNumber: request?.접수번호 ?? "", noticeNumber: notice?.입찰공고번호 ?? "", noticeNumbers: notice ? [notice.입찰공고번호] : [],
      noticeKind: notice?.공고종류 ?? "", requestDate: request?.접수일자 ?? "", postedAt: notice?.게시일시 ?? "",
      bidStartsAt: "", bidClosesAt: notice?.입찰마감일시 ?? "", openingAt: notice?.개찰일시 ?? "",
      agency: request?.수요기관 || notice?.수요기관 || captured[0]?.수요기관 || "", contracts: items,
      contractAmount: known.length ? known.reduce((sum, amount) => sum + amount, 0) : null,
      issues: request ? [] : ["연결된 접수 문서가 DB에 없습니다."], draftContracts: items.filter(contract => contract.draft).length, noticeCancelled: false, terminatedContracts: 0 };
  };
  const rows = notices.map(notice => {
    const relations = linked.filter(row => row.입찰공고번호 === notice.입찰공고번호);
    const request = requests.find(request => relations.some(row => row.접수번호 === request.접수번호));
    return make(notice.입찰공고번호, request, notice, contracts.filter(contract => relations.some(row => row.계약번호 === contract.계약번호)));
  });
  for (const request of requests) if (!usedRequests.has(request.접수번호)) rows.push(make(request.접수번호, request, undefined, []));
  for (const contract of contracts) if (!usedContracts.has(contract.계약번호)) {
    const row = make(contract.계약번호, undefined, undefined, [contract]);
    if (linked.some(source => source.계약번호 === contract.계약번호 && source.입찰공고번호)) row.issues.push("계약에 적힌 공고 문서가 DB에 없습니다.");
    rows.push(row);
  }
  const due = (raw: string) => {
    const date = /^\d{4}[/-]\d{2}[/-]\d{2}$/.test(raw) ? raw.replaceAll("/", "-") : null;
    return { raw, date, daysRemaining: date ? Math.round((Date.parse(date) - Date.parse("2026-10-09")) / 86400000) : null };
  };
  const deliveries = contracts.map((contract, index) => ({ number: contract.계약번호, title: contract.계약건명, contractDate: contract.계약일자,
    party: contract.계약상대자, agency: contract.수요기관, terminated: false, draft: !contract.계약일자, due: due(contract.납품기한 ?? ""),
    items: 품목행("contract", index).map((item, line) => ({ lineNo: line + 1, name: item.품명, specification: item.규격, agency: item.수요기관,
      quantity: item.수량, unit: item.단위, due: due(item.납품기한 ?? "") })), issues: [] }));
  return { dataCheckCount: rows.filter(row => row.stage === null).length, deliveries, asOf: "2026-10-09T09:00:00+09:00", totalCases: rows.length, rows,
    stages: labels.map(([key, label]) => ({ key, label, count: rows.filter(row => row.stage === key).length })),
    documents: { requests: requests.length, notices: notices.length, contracts: contracts.length,
      requestRevisions: requests.length, noticeRevisions: sheets["v_공고차수"].rows.length, contractRevisions: sheets["v_계약차수"].rows.length },
    contractAmount: rows.reduce((sum, row) => sum + (row.contractAmount ?? 0), 0),
    contractsWithAmount: rows.flatMap(row => row.contracts).filter(contract => contract.amount !== null).length };
}

/**
 * 줄 하나를 가리키는 키. 시트마다 어느 열이 키인지 다르다.
 *
 * <p>차례가 뜻을 갖는다 — 통합에는 접수번호도 실려 있지만 그 줄을 먼저 가리키는
 * 것은 계약번호다. 계약이 아직 없는 줄에서만 공고번호가, 그것도 없으면 접수번호가
 * 그 줄의 이름이 된다(App.tsx 의 <code>identity</code> 와 같은 차례다).</p>
 */
const 줄키 = (row: Row) =>
  [row["계약번호"], row["입찰공고번호"], row["접수번호"]].find(Boolean) ?? "";

/**
 * 그 키가 어느 개체의 것인가. <b>번호 모양으로 가리지 않는다</b> — 접수번호도 공고번호처럼
 * 붙임표를 달아, 모양으로 가리면 접수를 공고로 읽는다. 쌓인 줄에서 찾아 정한다.
 */
const 어느것: { entityType: Entity; view: string; 제목열: string; 차수길이: number }[] = [
  { entityType: "contract", view: "v_계약", 제목열: "계약건명", 차수길이: 2 },
  { entityType: "notice", view: "v_공고", 제목열: "공고명", 차수길이: 3 },
  { entityType: "request", view: "v_접수", 제목열: "요청명", 차수길이: 3 },
];

/** 지우면 무엇이 사라지는지. 진짜 다리는 DB 를 세지만 여기서는 그럴듯한 수를 낸다. */
function 계획(key: string, whole: boolean): DeletionPlan {
  const 갈래 = 어느것.find((k) => sheets[k.view].rows.some((r) => 줄키(r) === key));
  if (!갈래) throw new Error(`그런 접수·공고·계약이 없습니다: ${key}`);

  const sheet = sheets[갈래.view];
  const row = sheet.rows.find((r) => 줄키(r) === key)!;
  const 차수 = key.slice(-갈래.차수길이);

  // 두 번째 줄만 차수가 쌓인 것으로 꾸며 둔다 — 범위 고르기를 볼 자리다.
  const revisions = sheet.rows.indexOf(row) === 1
    ? (갈래.차수길이 === 3 ? ["000", "001"] : ["00", "01"])
    : [차수];

  const seriesGoes = whole || revisions.length <= 1;
  const 공고 = 갈래.entityType === "notice";

  return {
    entityType: 갈래.entityType,
    display: key,
    seq: 차수,
    title: row[갈래.제목열] ?? "",
    revisions,
    seriesGoes,
    childRows: 공고 ? 3 : 2,
    userFields: seriesGoes ? 2 : 0,
    overrides: Object.keys(sheet.overrides[key] ?? {}).length,
    linked: seriesGoes,
  };
}

// 예시 자료의 삭제 계획. 실제 합집합 영향과 원자성은 Core의 synthetic DB 테스트가 검증한다.
function batchPlan(keys: string[], whole: boolean): BatchDeletionPlan {
  const targets = [...new Set(keys)].map(key => 계획(key, whole));
  return { targets, fingerprint: JSON.stringify([targets, keys.map(key => Object.values(sheets).flatMap(s => s.rows.filter(r => 줄키(r) === key)))]),
    documentCount: targets.length, seriesCount: targets.filter(t => t.seriesGoes).length,
    childRows: targets.reduce((n, t) => n + t.childRows, 0), userFields: targets.reduce((n, t) => n + t.userFields, 0),
    overrides: targets.reduce((n, t) => n + t.overrides, 0), links: targets.filter(t => t.linked).length };
}

/**
 * 지어낸 계획 엑셀의 머리글. 서식이 표본에 고정된 뒤로(ADR-023 개정) 진짜 다리도 이 열두 개를
 * 이름 그대로 잡는다 — 여기서는 <b>표본 그대로인 파일</b>을 골랐다고 본다.
 */
const 계획머리글 = [
  "조달요구번호", "재고번호", "품명", "화폐구분", "단위", "지시수량",
  "요청부대부서명", "요청부대담당자", "요청부대사용자전화번호",
  "계약부서", "담당자", "연락처",
];

/**
 * 현황. <b>지표는 뒤가 정한다</b> — 여기 있는 것도 가상이고, 화면은 오는 대로 그린다.
 * 수는 위의 계획 표본과 맞춘다(24줄 · 미착수 6 · 접수 6 · 공고 6 · 계약 6).
 */
const 현황: StatusReport = {
  groups: [
    {
      이름: "쌓인 것",
      cells: [
        { 이름: "계획", 수: 24, 거르개: null },
        { 이름: "접수", 수: 18, 거르개: null },
        { 이름: "공고", 수: 12, 거르개: null },
        { 이름: "계약", 수: 7, 거르개: null },
      ],
    },
    {
      이름: "단계",
      cells: [
        { 이름: "미착수", 수: 6, 거르개: "단계=미착수" },
        { 이름: "접수", 수: 6, 거르개: "단계=접수" },
        { 이름: "공고", 수: 6, 거르개: "단계=공고" },
        { 이름: "계약", 수: 6, 거르개: "단계=계약" },
      ],
    },
  ],
};

/** 미리보기의 오늘 수집. 표본의 번호·건명을 그대로 쓴다 — 모두 지어낸 것이다. */

// ── 지금 보는 화면(ADR-036) ─────────────────────────────
// Chrome 에 열린 나라장터 탭 셋 — 새 차수인 계약, 사람이 고친 칸이 화면과 다르고 화면이 비우거나 빠뜨린 것이 있는 계약,
// 수집 규칙에 없는 화면.
// 번호·건명·금액은 모두 지어낸 것이다. 시험은 <code>지금화면</code> 의 값을 바꾼다.

const 칸 = (column: string, kind: MirrorField["kind"], value: string, extra: Partial<MirrorField> = {}): MirrorField =>
  ({ column, kind, value, old: null, raw: null, human: null, choiceId: null, sources: [], from: "", ...extra });

/** 칸을 채우는 화면의 자리와 그 이름표. */
const 자리 = (source: string, from: string): Partial<MirrorField> => ({ sources: [source], from });

const 품목 = (line: string, name: string, quantity: string, unit: string, price: string, amount: string,
  before: MirrorItem["before"] = null): MirrorItem =>
  ({ line, name, spec: "", quantity, unit, price, amount, before, added: false, source: `table:contract_item:${line}` });

const 화면줄 = (group: string, label: string, text: string, source = ""): ScreenRow => ({ group, label, text, source });

const 새차수화면 = (): MirrorShot => ({
  entityType: "contract", kind: "계약", number: "R26TA0000010101", base: "R26TA00000101", seq: "01", title: "시험 음향설비 개선",
  status: "새 차수", changes: 3, compareSeq: "00", latestSeq: "00", view: "v_계약", viewColumns: 30,
  fields: [
    칸("계약번호", "id", "R26TA0000010101", 자리("ctrtNoOrd", "계약번호")), 칸("계약본번호", "id", "R26TA00000101", 자리("ctrtNoOrd", "계약번호")),
    칸("차수", "id", "01", 자리("ctrtNoOrd", "계약번호")),
    칸("계약건명", "same", "시험 음향설비 개선", 자리("ctrtNm", "계약명")),
    칸("계약일자", "changed", "2026/10/02", { old: "2026/09/14", raw: "2026-10-02", ...자리("ctrtDt", "계약일자") }),
    칸("계약방법", "same", "제한경쟁", 자리("ctrtMthdCd", "계약방법")),
    // 품목에서 셈하는 총액 — 화면의 한 칸에서 오지 않아 잇는 자리가 없다.
    칸("계약금액", "changed", "34,400,000", { old: "30,400,000" }),
    칸("수요기관", "same", "가온시험기관", 자리("dmstUntyGrpNm", "기관명")),
    칸("계약상대자", "same", "주식회사 시험음향", { sources: ["table:erp_partner"], from: "" }), 칸("대표자", "same", "홍길동"), 칸("사업자등록번호", "same", "123-45-67890"),
  ],
  rest: 19,
  items: [
    품목("1", "디지털 믹서", "1", "식", "18,400,000", "18,400,000"),
    품목("2", "무선마이크 세트", "8", "세트", "2,000,000", "16,000,000", { quantity: "6", unit: "세트", price: "2,000,000", amount: "12,000,000" }),
  ],
  itemChanges: 1, itemRows: 2, itemsAllRead: true,
  place: [
    { kind: "접수", state: "missing", number: "", collected: false, source: null },
    { kind: "공고", state: "ref", number: "R26BK00000101", collected: false, source: "bidPbancNo" },
    { kind: "계약", state: "here", number: "R26TA0000010101", collected: true, source: null },
  ],
  rounds: [
    { seq: "00", amount: "30,400,000", savedOn: "09/16", state: "stored" },
    { seq: "01", amount: "34,400,000", savedOn: "", state: "ghost" },
  ],
  baseToken: "mock-새차수", missing: [], keptCells: 0, keptIn: "", totalsWithDefaults: true, totalsWithoutMissing: true,
});

const 검토화면 = (): MirrorShot => ({
  entityType: "contract", kind: "계약", number: "R26TA0000010201", base: "R26TA00000102", seq: "01", title: "시험동 배기설비 보수",
  status: "검토 대기", changes: 0, compareSeq: "01", latestSeq: "01", view: "v_계약", viewColumns: 30,
  fields: [
    칸("계약번호", "id", "R26TA0000010201"), 칸("계약본번호", "id", "R26TA00000102"), 칸("차수", "id", "01"),
    칸("계약건명", "same", "시험동 배기설비 보수"),
    칸("납품장소", "override", "시험동 지하 기계실", { human: "시험동 B1 기계실", choiceId: "override/납품장소" }),
    칸("계약상대자", "override", "주식회사 바른시험", { human: "(주)바른시험", choiceId: "override/계약상대자" }),
    칸("대표자", "same", "홍길동"),
    // 화면이 비워 보낸 칸 — 있던 값을 그대로 두는 것이 기본이다.
    칸("납품기한", "blank", "2026/11/30", { choiceId: "contract/R26TA00000102-01/delivery_due" }),
  ],
  rest: 22,
  // 2행은 작업자료에만 있고 이번 화면에는 없다 — 그대로 두므로 품목 전체에는 남는다.
  items: [품목("1", "배기팬 교체", "1", "식", "31,200,000", "31,200,000"), { ...품목("2", "배기 덕트 보강", "1", "식", "2,400,000", "2,400,000"), source: "" }],
  itemChanges: 0, itemRows: 1, itemsAllRead: true,
  place: [
    { kind: "접수", state: "missing", number: "", collected: false, source: null },
    { kind: "공고", state: "linked", number: "R26BK00000102-000", collected: true, source: "bidPbancNo" },
    { kind: "계약", state: "here", number: "R26TA0000010201", collected: true, source: null },
  ],
  rounds: [
    { seq: "00", amount: "29,800,000", savedOn: "09/24", state: "stored" },
    { seq: "01", amount: "31,200,000", savedOn: "09/30", state: "current" },
  ],
  baseToken: "mock-검토",
  missing: [{ table: "contract_item", tableName: "품목", line: 2, label: "배기 덕트 보강", amount: "2,400,000", choiceId: "remove/contract_item/2" }],
  keptCells: 1, keptIn: "품목",
  // 머리 총액은 화면의 품목에서 셈한다 — 빠진 행을 남기면 맞지 않고, 지우면 맞는다.
  totalsWithDefaults: false, totalsWithoutMissing: true,
});

const 탭 = (tabId: number, screen: string, state: MirrorTab["state"], shot: MirrorShot | null, front = false,
  screenRows: ScreenRow[] = [], menu = ""): MirrorTab => ({
  id: `4242:${tabId}`, pid: 4242, tabId, browser: "Chrome", title: "나라장터", screen, menu, state,
  readAt: 오늘의("09:20"), message: "", front, shot, error: null, screenRows, export: null,
});

/** 화면 그대로 — 확장이 읽어 보낸 이름표와 보이는 글. 화면의 서식 그대로다(날짜의 줄표, 금액의 쉼표 없음). */
const 새차수줄 = (): ScreenRow[] => [
  화면줄("계약 기본정보", "계약번호", "R26TA00000101-01", "ctrtNoOrd"),
  화면줄("계약 기본정보", "계약명", "시험 음향설비 개선", "ctrtNm"),
  화면줄("계약 기본정보", "계약방법", "제한경쟁", "ctrtMthdCd"),
  화면줄("계약 기본정보", "계약일자", "2026-10-02", "ctrtDt"),
  화면줄("계약 기본정보", "공고번호", "R26BK00000101", "bidPbancNo"),
  화면줄("수요기관", "기관명", "가온시험기관", "dmstUntyGrpNm"),
  화면줄("품목내역", "1 디지털 믹서", "1식 · 18400000", "table:contract_item:1"),
  화면줄("품목내역", "2 무선마이크 세트", "8세트 · 16000000", "table:contract_item:2"),
];

const 개찰줄 = (): ScreenRow[] => [
  화면줄("", "공고번호", "R26BK00000101-000"),
  화면줄("", "공고명", "시험 음향설비 개선"),
  화면줄("", "개찰일시", "2026-09-08 14:00"),
];

/** 가짜 다리의 Chrome 탭. 시험은 <code>탭</code> 을 갈아 끼우고, <code>낡음</code> 이면 가져오기가 낡은 baseToken 으로 답한다. */
export const 지금화면 = {
  탭: [] as MirrorTab[],
  /** 확장이 알린 설정. null 이면 옛 확장처럼 알리지 않는다. */
  설정: { pid: 4242, browser: "Chrome", siteAccess: true } as ExtensionSettings | null,
  앞: "4242:1" as string | null,
  낡음: false,
  /** 가져오려는 탭의 화면이 보고에 없다(닫힘 · 끊김). */
  사라짐: false,
  /** 앱이 보낸 명령. */
  명령: [] as { tabId: number; command: string }[],
  /** 가져온 요청. <code>picks</code> 는 사람이 고른 자리 — 복원할 덮개, 비울 칸, 지울 행. */
  가져옴: [] as { tabId: number; baseToken: string; picks: string[]; captureId: string }[],
  /** 수집하는 화면 — 진짜 다리는 매핑에서 화면을 단 프로필을 낸다(Mirror.SupportedScreens). */
  수집화면: [] as SupportedScreen[],
};

/** 기본 매핑에서 화면을 단 프로필(src/Pclm.Core/Erp/mapping.json 의 screen) 그대로. */
export const 기본수집화면 = (): SupportedScreen[] => [
  { code: "01117", entityType: "request", kind: "접수", name: "조달요구 접수(목록형)" },
  { code: "01112", entityType: "request", kind: "접수", name: "조달요구 접수 상세(일반용역)" },
  { code: "01179", entityType: "notice", kind: "공고", name: "입찰공고 상세" },
  { code: "01181", entityType: "notice", kind: "공고", name: "입찰공고 상세(일반용역)" },
  { code: "01196", entityType: "notice", kind: "공고", name: "입찰공고상세조회(통합)" },
  { code: "01579", entityType: "contract", kind: "계약", name: "계약 상세" },
  { code: "01608", entityType: "contract", kind: "계약", name: "물품·일반용역 변경계약서 작성" },
];

export const 기본탭 = (): MirrorTab[] => [
  탭(1, "계약 상세", "supported", 새차수화면(), true, 새차수줄(), "01579"),
  탭(2, "계약 상세", "supported", 검토화면(), false, [], "01579"),
  탭(3, "개찰 결과", "unsupported", null, false, 개찰줄(), "01175"),
];
지금화면.탭 = 기본탭();
지금화면.수집화면 = 기본수집화면();

function 기본수집(): CaptureEntry[] {
  return [
    수집줄(오늘의("09:12"), "계약", "R26TA0911050100", 씨앗[0].건명, true),
    수집줄(오늘의("08:57"), "공고", "R26BK09017002-000", 씨앗[1].건명, false),
    수집줄(오늘의("08:41"), "접수", `${접수씨앗[0].base}-000`, 접수씨앗[0].요청명, true),
  ];
}

/** 진짜 다리(CaptureLog.Read)와 같은 셈 — 오늘 것만, 종류별 수, 바꾼 수, 마지막 하나. */
function 수집날(줄: CaptureEntry[]): CaptureDay {
  const today = 오늘의("00:00").slice(0, 10);
  const entries = 줄.filter((e) => e.at.startsWith(today));
  return {
    today,
    entries,
    requests: entries.filter((e) => e.kind === "접수").length,
    notices: entries.filter((e) => e.kind === "공고").length,
    contracts: entries.filter((e) => e.kind === "계약").length,
    saved: entries.filter((e) => e.changed).length,
    last: 줄[0] ?? null,
  };
}

/** 화면이 부르는 것을 그대로 받는다. 이름과 인자는 Bridge.cs 의 switch 와 같아야 한다. */
export function invoke(method: string, args: unknown[]): Promise<unknown> {
  if (method === "dataVersion") return Promise.resolve(수집.판);
  const wait = <T>(value: T) => new Promise<T>((r) => setTimeout(() => r(value), 60));

  const 열람 = 스위치.열람;
  if (열람 && 고치는요청.has(method))
    return Promise.reject(new Error("열람 중인 자료라 고칠 수 없습니다 — 제출본 「제출_홍길동_20260901.pclm」 는 이 창에서 읽기만 합니다."));

  switch (method) {
    case "session":
      return wait<Session>(열람
        ? { readOnly: true, role: "submission", roleName: "제출본", path: "C:\\Users\\홍길동\\Documents\\제출_홍길동_20260901.pclm" }
        : { readOnly: false, role: "work", roleName: "작업자료", path: "C:\\Users\\홍길동\\AppData\\Local\\Pclm\\계약자료.pclm" });

    // 진짜 다리는 고르기 창을 열고, 고른 것이 쓸 수 있는 자리인지 본 뒤 그 뜻을 돌려준다. 아무것도 바꾸지 않는다.
    // 바꾸기는 제출본을 고른 꼴(사본에서 새로)을 낸다 — 확인 창이 가장 많은 것을 말해야 하는 갈래다.
    case "pickWorkfile": {
      const 문서 = "C:\\Users\\홍길동\\Documents";
      const plans: Record<string, WorkfilePlan> = {
        move: { action: "move", path: "D:\\계약\\계약자료.pclm", source: 작업자료, sourceRoleName: null, current: 작업자료 },
        switch: {
          action: "snapshot", path: `${문서}\\계약자료.pclm`,
          source: `${문서}\\제출_홍길동_20260901.pclm`, sourceRoleName: "제출본", current: 작업자료,
        },
        new: { action: "new", path: `${문서}\\계약자료.pclm`, source: null, sourceRoleName: null, current: 작업자료 },
      };
      const plan = plans[String(args[0])];
      return plan ? wait(plan) : Promise.reject(new Error(`모르는 바꾸기입니다: ${String(args[0])}`));
    }

    // 진짜 다리는 바꾼 뒤 답을 보내고 창을 다시 띄운다. 여기서는 답의 꼴만 낸다.
    case "moveWorkfile":
    case "switchWorkfile":
    case "newWorkfile":
      return wait<SwitchResult>({ restart: true, path: String(args[args.length - 1]) });

    case "backupWorkfile": {
      const 오늘 = new Date().toISOString().slice(0, 10).replace(/-/g, "");
      return wait({ path: `C:\\Users\\홍길동\\Documents\\계약자료_백업_${오늘}.pclm` });
    }

    // 진짜 다리는 고르기 창을 열고 새 창(열람 프로세스)을 띄운다. 여기서는 고른 꼴만 낸다.
    case "openOther":
      return wait({ path: "C:\\Users\\홍길동\\Documents\\제출_홍길동_20260901.pclm" });

    // 진짜 다리는 확장을 홈에 풀고 브라우저 연결을 등록한다. 여기서는 준비한 것으로 치고 꼴만 낸다.
    case "prepareExtension":
      스위치.확장 = "기다림";
      return wait({ folder: 확장폴더, version: 확장판 });
    // 진짜 다리는 브라우저의 확장 관리를 연다. 여기서는 연 것으로 친다.
    case "openExtensionSetup":
      return wait(null);
    case "extensionStatus": {
      if (스위치.확장상태) return wait(스위치.확장상태);
      const 꼴 = 스위치.확장;
      if (꼴 === "없음") return Promise.reject(new Error("내장 확장 연결은 업무 홈에서만 준비합니다."));
      const status: ExtensionStatus = {
        prepared: 꼴 !== "설치 전",
        embeddedVersion: 확장판,
        diskVersion: 꼴 === "설치 전" ? null : 확장판,
        contacts: 꼴 === "연결" || 꼴 === "끊김" ? [{ browser: "Chrome", version: 확장판, at: 오늘의("09:12") }]
          : 꼴 === "옛 판" ? [{ browser: "Chrome", version: "0.8.0", at: 오늘의("09:12") }]
          : [],
        folder: 확장폴더,
        // 지금 붙어 있는 것(상시 연결). 끊긴 꼴은 오늘 18:02 에 끊겼다.
        live: 꼴 === "연결" ? [{ browser: "Chrome", version: 확장판, connectedAt: 오늘의("09:12") }]
          : 꼴 === "옛 판" ? [{ browser: "Chrome", version: "0.8.0", connectedAt: 오늘의("09:12") }]
          : [],
        log: 꼴 === "연결" || 꼴 === "끊김" ? [
          { at: 오늘의("08:30"), event: "updated", browser: "Chrome", version: 확장판, previous: "0.8.0" },
          { at: 오늘의("08:31"), event: "connected", browser: "Chrome", version: 확장판, previous: "" },
          { at: 오늘의("08:57"), event: "disconnected", browser: "Chrome", version: 확장판, previous: "" },
          { at: 오늘의("09:12"), event: "connected", browser: "Chrome", version: 확장판, previous: "" },
          ...(꼴 === "끊김" ? [{ at: 오늘의("18:02"), event: "disconnected" as const, browser: "Chrome", version: 확장판, previous: "" }] : []),
        ] : 꼴 === "옛 판" ? [
          { at: 오늘의("09:12"), event: "connected", browser: "Chrome", version: "0.8.0", previous: "" },
        ] : [],
      };
      return wait(status);
    }
    case "captures":
      return wait(수집날(수집.줄.length > 0 ? 수집.줄 : 기본수집()));
    // 진짜 다리는 홈의 탭 보고를 읽어 탭마다 작업자료에 비춘다(열람 창은 읽지 않는다). 여기서는 지어 둔 탭을 낸다.
    case "mirror":
      if (열람) return wait<MirrorState>({ readOnly: true, reporting: false, front: null, tabs: [], settings: null, screens: [] });
      return wait<MirrorState>({ readOnly: false, reporting: true, front: 지금화면.앞,
        tabs: 지금화면.탭.map((t) => ({ ...t, front: t.id === 지금화면.앞 })), settings: 지금화면.설정, screens: 지금화면.수집화면 });
    case "mirrorCommand":
      지금화면.명령.push({ tabId: Number(args[1]), command: String(args[2]) });
      return wait({ sent: true });
    // 진짜 다리는 수집의 저장과 같은 절차로 쓴다. 여기서는 그 탭을 「저장됨」 으로 바꾸고 수집 기록을 한 줄 더한다.
    case "importShot": {
      const tabId = Number(args[1]);
      const picks = JSON.parse(String(args[3])) as string[];
      지금화면.가져옴.push({ tabId, baseToken: String(args[2]), picks, captureId: String(args[4]) });
      if (지금화면.사라짐) return wait<ImportResult>({ status: "gone", message: "이 탭이 닫혔거나 브라우저 연결이 끊겨 가져오지 않았습니다. 나라장터에서 화면을 다시 여세요." });
      if (지금화면.낡음) return wait<ImportResult>({ status: "stale", message: "화면이나 작업자료가 바뀌었습니다. 다시 읽습니다." });
      const now = new Date();
      const p2 = (n: number) => String(n).padStart(2, "0");
      const hhmm = `${p2(now.getHours())}:${p2(now.getMinutes())}`;
      const tab = 지금화면.탭.find((t) => t.tabId === tabId);
      const shot = tab?.shot;
      if (!tab || !shot) return Promise.reject(new Error("화면이 바뀌었습니다."));
      지금화면.탭 = 지금화면.탭.map((t) => t !== tab ? t : {
        ...t, readAt: 오늘의(hhmm),
        shot: { ...shot, status: "저장됨", changes: 0, itemChanges: 0, compareSeq: shot.seq, latestSeq: shot.seq,
          fields: shot.fields.map((f) => f.kind === "changed" || f.kind === "new" ? { ...f, kind: "same", old: null }
            : f.kind === "override" ? { ...f, kind: "same", value: picks.includes(f.choiceId ?? "") ? f.value : f.human ?? "", human: null, choiceId: null }
            : f.kind === "blank" ? { ...f, kind: "same", value: picks.includes(f.choiceId ?? "") ? "" : f.value, choiceId: null }
            : f),
          items: shot.items.filter((i) => !shot.missing.some((m) => m.table.endsWith("_item") && String(m.line) === i.line && picks.includes(m.choiceId)))
            .map((i) => ({ ...i, before: null, added: false })),
          missing: shot.missing.filter((m) => !picks.includes(m.choiceId)), keptCells: 0, keptIn: "",
          rounds: shot.rounds.map((r) => r.state === "ghost" ? { ...r, state: "current", savedOn: `${p2(now.getMonth() + 1)}/${p2(now.getDate())}` } : r) },
      });
      수집.줄 = [수집줄(오늘의(hhmm), shot.kind, shot.number, shot.title, true), ...(수집.줄.length > 0 ? 수집.줄 : 기본수집())];
      수집.판++;
      return wait<ImportResult>({ status: "stored", entity: shot.number, changed: true, at: hhmm });
    }
    case "erpTools":
      if (args[0] === "list") return wait({ active: JSON.parse(mappingJson), defaults: JSON.parse(mappingJson), versions: [] });
      return Promise.reject(new Error("개발 미리보기입니다. 매핑 검증·자료 저장은 메인 프로그램에서 실행하세요."));
    case "status":
      summary.noticeBases = sheets["v_공고"].rows.length;
      summary.noticeRows = sheets["v_공고차수"].rows.length;
      return wait(summary);

    // 열람이면 진짜 다리처럼 고칠 수 있는 열을 비워 보낸다.
    case "sheet": {
      const view = 뷰목록().find(v => `u_${v.name}` === args[0]);
      if (view) {
        const fold = 모으기(view), env = 뷰재료();
        const rows = 뷰줄들(view.definition.columns, fold).map(r => Object.fromEntries(view.definition.columns.map(c => {
          const f = toV2(c.formula);
          return [c.name, f.source === "flow" && checkFlow(f.flow, env).ok ? 줄값(f.flow, env, r, fold) : ""];
        })));
        return wait({ name: `u_${view.name}`, columns: view.definition.columns.map(c => c.name), rows,
          editable: [], correctable: [], overrides: {} });
      }
      const sheet = sheets[args[0] as string];
      if (!sheet) throw new Error(`내보이지 않는 표입니다: ${args[0]}`);
      return wait(열람 ? { ...sheet, editable: [], correctable: [], overrides: {} } : sheet);
    }

    case "columnContext":
      return wait(columnContextExample(args[0] as Entity));
    case "traceComputedColumn":
      try {
        return wait(추적(args[0] as Entity, toV2(JSON.parse(args[1] as string))));
      } catch (e) { return Promise.reject(e as Error); }
    case "compileComputedColumn": {
      const [entityType, json] = args as [Entity, string];
      try {
        const f = toV2(JSON.parse(json));
        return wait(f.source === "sql" ? f.sql : compileFlow(f.flow, 재료(entityType), entityType));
      } catch (e) { return Promise.reject(e as Error); }
    }
    // 목업은 SQL 을 돌리지 않는다 — 모양만 보고(checkSqlMock), 표본 값은 비워 둔다.
    case "validateComputedSql": {
      const [entityType, sql] = args as [Entity, string];
      const samples = columnContextExample(entityType).sources.slice(0, 3).map(({ base, revision, title }) => ({ base, revision, title }));
      const issue = checkSqlMock(sql)[0];
      const result: SqlCheck = { ok: !issue, error: issue?.message ?? null, samples, final: samples.map(() => ""),
        note: "미리보기 자료에서는 SQL을 돌리지 않습니다. 앱에서는 표본 문서에서 실제로 돌려 봅니다." };
      return wait(result);
    }
    case "planFreezeColumn": {
      const [entityType, name] = args as [Entity, string];
      const column = userColumns[entityType].find(c => c.fieldName === name);
      if (!column) return Promise.reject(new Error(`그런 열이 없습니다: ${name}`));
      const rows = sheets[종류뷰[entityType]].rows;
      const filled = rows.filter(r => (r[name] ?? "") !== "").length;
      const plan: FreezePlan = column.kind === "computed"
        ? { kind: column.kind, cells: filled, hidden: 0, series: rows.length }
        : { kind: column.kind, cells: 0, hidden: filled, series: 0 };
      return wait(plan);
    }
    case "freezeColumn": {
      const [entityType, name, expected] = args as [Entity, string, string];
      return 열고치기(entityType, () => {
        const at = userColumns[entityType].findIndex(c => c.fieldName === name);
        const column = userColumns[entityType][at];
        if (!column || column.kind !== "computed") throw new Error("가공열만 값으로 굳힐 수 있습니다.");
        const filled = sheets[종류뷰[entityType]].rows.filter(r => (r[name] ?? "") !== "").length;
        if (expected !== undefined && Number(expected) !== filled) throw new Error("센 뒤 자료가 바뀌었습니다. 다시 세어 확인하세요. 쓴 값은 없습니다.");
        userColumns[entityType][at] = { ...column, kind: "text", choices: [] };
      });
    }
    case "saveComputedColumn": {
      const [entityType, name, json, previousName] = args as [Entity, string, string, string | null];
      return 열고치기(entityType, () => {
        const old = userColumns[entityType].find(c => c.fieldName === previousName);
        if (!name.trim()) throw new Error("열 이름이 비었습니다.");
        if (userColumns[entityType].some(c => c.fieldName === name && c !== old) || 쓰이는가(entityType, name))
          throw new Error(`이미 있는 열입니다: ${name}`);
        const f = toV2(JSON.parse(json));
        const problem = f.source === "flow" ? checkFlow(f.flow, 재료(entityType)).errors[0] : checkSqlMock(f.sql)[0];
        if (problem) throw new Error(problem.message);
        f.id = old?.formula?.id || crypto.randomUUID();
        const column: UserColumn = { entityType, fieldName: name, kind: "computed", choices: [], formula: f };
        if (old) userColumns[entityType].splice(userColumns[entityType].indexOf(old), 1, column);
        else userColumns[entityType].push(column);
        if (old && previousName && previousName !== name) 뷰열이름바꾸기(entityType, previousName, name);
      });
    }
    // 맞춤 뷰(docs/user-views.md §8). 목업은 정의를 들고 TS 평가기로 미리보기·보표를 낸다. 기준 인자는 없다.
    case "userViews":
      return wait(뷰목록());
    case "exportViewPackage": {
      try {
        const json = JSON.stringify(구성내보내기(JSON.parse(args[0] as string)), null, 2);
        보기구성파일.exported = json;
        const name = "보기구성.json", url = URL.createObjectURL(new Blob([json], { type: "application/json" }));
        const a = document.createElement("a"); a.href = url; a.download = name; a.click(); setTimeout(() => URL.revokeObjectURL(url), 0);
        return wait({ download: true, name });
      } catch (e) { return Promise.reject(e as Error); }
    }
    case "pickViewPackage": return 구성파일고르기();
    case "builtinViewPackage":
      try { return wait({ name: "예시 보기", json: 예시보기, preview: 구성미리보기(예시보기) }); } catch (e) { return Promise.reject(e as Error); }
    case "previewViewPackage":
      try { return wait(구성미리보기(args[0] as string)); } catch (e) { return Promise.reject(e as Error); }
    case "importViewPackage":
      try { 구성미리보기(args[0] as string, args[1] as string); return wait(뷰목록()); } catch (e) { return Promise.reject(e as Error); }
    case "userViewContext":
      return 스위치.뷰맥락오류 ? Promise.reject(new Error(스위치.뷰맥락오류)) : wait(뷰맥락());
    case "traceUserViewColumn":
      try { return wait(뷰추적(toV2(JSON.parse(args[0] as string)), args[1] as string[] | null | undefined)); }
      catch (e) { return Promise.reject(e as Error); }
    case "userViewSamples":
      try { return wait(뷰표본(JSON.parse(args[0] as string) as 뷰꼴, (args[1] as string | null) ?? "", (args[2] as 뷰가장자리 | null) ?? null)); }
      catch (e) { return Promise.reject(e as Error); }
    case "previewUserView":
      try { return wait(뷰미리보기(JSON.parse(args[0] as string) as 뷰꼴, String(args[1] ?? ""))); }
      catch (e) { return Promise.reject(e as Error); }
    case "saveUserView":
      try { return wait(뷰저장(JSON.parse(args[0] as string) as 뷰꼴, (args[1] as string | null) ?? null)); }
      catch (e) { return Promise.reject(e as Error); }
    case "deleteUserView": {
      const at = 사용자뷰.findIndex(v => v.id === args[0]);
      if (at < 0) return Promise.reject(new Error("그런 맞춤 뷰가 없습니다."));
      사용자뷰.splice(at, 1);
      return wait(뷰목록());
    }
    case "userColumns": {
      const which = args[0] as Entity | undefined;
      return wait(which ? userColumns[which].map(열내기) : 모든열());
    }

    case "addColumn": {
      const [entityType, fieldName, kind, packed] = args as [Entity, string, string, string];

      return 열고치기(entityType, () => {
        const name = fieldName.trim();
        if (!name) throw new Error("열 이름이 비었습니다.");
        if (userColumns[entityType].some((c) => c.fieldName === name))
          throw new Error(`이미 있는 열입니다: ${name}`);
        if (쓰이는가(entityType, name)) throw new Error(`표가 이미 쓰는 이름입니다: ${name}`);

        userColumns[entityType].push({ entityType, fieldName: name, kind: kind as UserColumn["kind"], choices: 후보(packed) });
      });
    }

    case "updateColumn": {
      const [entityType, fieldName, newName, kind, packed] =
        args as [Entity, string, string, string, string];

      return 열고치기(entityType, () => {
        const name = newName.trim();
        const at = userColumns[entityType].findIndex((c) => c.fieldName === fieldName);
        if (at < 0) throw new Error(`그런 열이 없습니다: ${fieldName}`);
        if (!name) throw new Error("열 이름이 비었습니다.");
        if (name !== fieldName && userColumns[entityType].some((c) => c.fieldName === name))
          throw new Error(`이미 있는 열입니다: ${name}`);
        if (name !== fieldName && 쓰이는가(entityType, name))
          throw new Error(`표가 이미 쓰는 이름입니다: ${name}`);

        userColumns[entityType][at] = {
          entityType, fieldName: name, kind: kind as UserColumn["kind"],
          choices: kind === "choice" ? 후보(packed) : [],
        };

        // 이름이 바뀌면 적어 둔 값도, 그 열을 읽던 맞춤 뷰의 식도 따라간다(docs/user-views.md §5).
        if (name !== fieldName) 뷰열이름바꾸기(entityType, fieldName, name);
        if (name !== fieldName)
          for (const key of 시트(entityType))
            for (const row of sheets[key].rows) {
              row[name] = row[fieldName] ?? "";
              delete row[fieldName];
            }
      });
    }

    case "removeColumn": {
      const [entityType, fieldName] = args as [Entity, string];

      return 열고치기(entityType, () => {
        const at = userColumns[entityType].findIndex((c) => c.fieldName === fieldName);
        if (at < 0) throw new Error(`그런 열이 없습니다: ${fieldName}`);
        userColumns[entityType].splice(at, 1);
      });
    }

    case "moveColumn": {
      const [entityType, fieldName, delta] = args as [Entity, string, string];

      return 열고치기(entityType, () => {
        const list = userColumns[entityType];
        const at = list.findIndex((c) => c.fieldName === fieldName);
        if (at < 0) throw new Error(`그런 열이 없습니다: ${fieldName}`);

        const to = Math.min(Math.max(at + Number(delta), 0), list.length - 1);
        list.splice(to, 0, ...list.splice(at, 1));
      });
    }

    case "batchEdit": {
      const [view, json] = args as [string, string];
      const changes = JSON.parse(json) as BatchChange[];
      const current = sheets[view];
      if (!current || current.editable.length + current.correctable.length === 0) return Promise.reject(new Error("읽기 전용 표입니다."));
      for (const c of changes) {
        const r = current.rows.find(row => 줄키(row) === c.rowKey);
        if (!r || !(current.editable.includes(c.field) || current.correctable.includes(c.field))) return Promise.reject(new Error("수정할 수 없는 칸입니다."));
        if ((r[c.field] ?? "") !== c.original) return Promise.reject(new Error("자료가 변경되었습니다. 다시 읽고 검토하세요. 저장한 값은 없습니다."));
        if (c.field === "메모" && c.value === "실패") return Promise.reject(new Error("일부러 낸 오류 — 저장한 값은 없습니다."));
      }
      for (const c of changes) for (const sheet of Object.values(sheets)) {
        if (!sheet.columns.includes(c.field)) continue;
        for (const r of sheet.rows.filter(r => 줄키(r) === c.rowKey)) {
          if (c.operation === "revert") {
            const original = sheet.overrides[c.rowKey]?.[c.field];
            if (original !== undefined) { r[c.field] = original; delete sheet.overrides[c.rowKey][c.field]; }
          } else {
            if (current.correctable.includes(c.field)) (sheet.overrides[c.rowKey] ??= {})[c.field] ??= r[c.field] ?? "";
            r[c.field] = c.value;
          }
        }
      }
      return wait({ changed: changes.length });
    }
    case "batchDeletionPlan": {
      return wait(batchPlan(JSON.parse(String(args[0])), args[1] === "series"));
    }
    case "batchDelete": {
      const keys = JSON.parse(String(args[0])) as string[];
      const plan = batchPlan(keys, args[1] === "series");
      if (args[2] !== plan.fingerprint) return Promise.reject(new Error("삭제 영향이 변경되었습니다. 다시 검토하세요."));
      return Promise.all(keys.map(key => invoke("deleteEntity", [key, args[1]]))).then(() => plan);
    }
    case "setField": {
      const [key, field, value] = args as [string, string, string];
      // 되돌리기를 볼 자리. 담당은 이제 기본 열이 아니라 남아 있는 열로 바꾼다.
      if (field === "메모" && value === "실패")
        return Promise.reject(new Error("일부러 낸 오류 — 되돌리기를 볼 자리"));

      for (const sheet of Object.values(sheets))
        for (const row of sheet.rows) if (줄키(row) === key) row[field] = value;

      return wait(null);
    }

    case "setOverride": {
      const [key, column, value] = args as [string, string, string];

      if (키열.includes(column))
        return Promise.reject(new Error(`레코드를 가리키는 열이라 고칠 수 없습니다: ${column}`));

      for (const sheet of Object.values(sheets)) {
        if (!sheet.correctable.includes(column)) continue;

        for (const row of sheet.rows)
          if (줄키(row) === key) {
            // 원래 값은 처음 고칠 때만 담는다 — 두 번째부터 다시 적으면 안내가 거짓이 된다.
            const cells = (sheet.overrides[key] ??= {});
            cells[column] ??= row[column] ?? "";
            row[column] = value;
          }
      }

      return wait(null);
    }

    case "clearOverride": {
      const [key, column] = args as [string, string];

      for (const sheet of Object.values(sheets)) {
        const was = sheet.overrides[key]?.[column];
        if (was === undefined) continue;

        for (const row of sheet.rows) if (줄키(row) === key) row[column] = was;

        delete sheet.overrides[key][column];
        if (Object.keys(sheet.overrides[key]).length === 0) delete sheet.overrides[key];
      }

      return wait(null);
    }

    case "deletionPlan": {
      const [key, scope] = args as [string, string];
      return wait(계획(key, scope === "series"));
    }

    case "deleteEntity": {
      const [key] = args as [string, string];

      for (const sheet of Object.values(sheets)) {
        sheet.rows = sheet.rows.filter((row) => 줄키(row) !== key);
        delete sheet.overrides[key];
      }


      summary.contractBases = sheets["v_계약"].rows.length;
      summary.contractRows = summary.contractBases;
      summary.requestBases = sheets["v_접수"].rows.length;
      summary.requestRows = summary.requestBases;

      return wait(null);
    }

    case "outline":
      return wait(outline);

    case "dashboard":
      return wait(dashboardFixture());

    case "settings":
      if (스위치.설정오류 > 0) { 스위치.설정오류--; return Promise.reject(new Error("설정 파일이 잠겨 있습니다.")); }
      return wait(settings);

    case "saveSettings": {
      const [submitter] = args as [string | undefined];
      settings = {
        ...settings,
        // 이름이 오지 않으면 그대로 둔다 — 진짜 다리도 그렇게 한다.
        submitterName: submitter ?? settings.submitterName,
      };
      return wait(settings);
    }

    case "dataLocation":
      return wait({
        path: "C:\\Users\\홍길동\\AppData\\Local\\Pclm\\계약자료.pclm",
        folder: "C:\\Users\\홍길동\\AppData\\Local\\Pclm",
        sizeBytes: 192512,
        home: "C:\\Users\\홍길동\\AppData\\Local\\Pclm",
        configPath: "C:\\Users\\홍길동\\AppData\\Local\\Pclm\\config.json",
      });
    // 진짜 다리는 exe 안에 박힌 전문을 준다. 여기서는 꼴만.
    case "about":
      return wait({
        version: "0.5.0",
        license: "MIT License\n\nCopyright (c) 2026 JM\n\n(개발 미리보기 — 전문은 exe 안에 있습니다)",
        notices: "계약 목록 — 제3자 고지\n\n== nuget ClosedXML 0.105.1 — MIT\n== npm react 19.2.8 — MIT",
      });
    case "windowPrefs":
      return wait(창.몸가짐);

    // 진짜 다리는 깃발을 쥐었다가 창을 닫을 때 보고, 「정말 닫기」 에서 창을 내린다. 미리보기에는 내릴 창이 없다.
    case "setUnsaved":
      if (args[0] !== "true" && args[0] !== "false") return Promise.reject(new Error(`켜짐·꺼짐이 아닙니다: ${String(args[0])}`));
      return wait(null);
    case "closeWindow":
      return wait(null);

    // 진짜 다리처럼 "true"·"false" 만 받는다.
    case "saveWindowPrefs": {
      const [closeToTray, autostart] = args as [string, string];
      for (const v of [closeToTray, autostart])
        if (v !== "true" && v !== "false") return Promise.reject(new Error(`켜짐·꺼짐이 아닙니다: ${v}`));
      // 진짜 다리처럼 걸 수 없는 자리에서 자동 실행을 바꾸려 하면 아무것도 적지 않고 까닭을 낸다.
      if ((autostart === "true") !== 창.몸가짐.autostart && !창.몸가짐.autostartAvailable)
        return Promise.reject(new Error(창.몸가짐.autostartReason ?? "자동 실행을 걸 수 없습니다."));
      창.몸가짐 = { ...창.몸가짐, closeToTray: closeToTray === "true", autostart: autostart === "true" };
      return wait(창.몸가짐);
    }

    case "revealDataFolder":
      return wait({ folder: "C:\\Users\\홍길동\\AppData\\Local\\Pclm" });

    // 이어도 계약은 목록에 남는다 — 잘못 이었으면 그 줄에서 끊어야 한다.
    case "exportSheet":
    case "export": {
      // 진짜 다리는 고르기 창을 열고 실제로 쓴 경로를 돌려준다. 그만두면 null 이다.
      const 오늘 = new Date().toISOString().slice(0, 10).replace(/-/g, "");
      return wait(`C:\\Users\\사람\\Documents\\관리대장_${오늘}.xlsx`);
    }

    // 진짜 다리는 자리를 묻고 그 자리에 제출본을 뜬다. 이름이 비었는지도 앱이 판정한다 —
    // 화면이 들고 있는 설정은 저장 전 입력값과 어긋날 수 있어 화면이 잴 수 없다.
    case "submit": {
      const 오늘 = new Date().toISOString().slice(0, 10).replace(/-/g, "");
      return wait<SubmitResult>({
        path: `C:\\Users\\사람\\Documents\\제출_홍길동_${오늘}.pclm`,
        nameMissing: false,
      });
    }

    // 진짜 다리는 제출본이 든 폴더를 묻고, 그 폴더에 취합본과 결과 글(txt)을 실제로
    // 떨어뜨린다. 여기서는 지어낸 셈만 낸다.
    case "merge": {
      const 오늘 = new Date().toISOString().slice(0, 10).replace(/-/g, "");
      const 폴더 = "C:\\Users\\사람\\Documents\\받은제출본";
      return wait<MergeResult>({
        path: `${폴더}\\취합_${오늘}.pclm`,
        reportPath: `${폴더}\\취합_${오늘}.txt`,
        submissions: 3,
        plans: 24,
        requests: 5,
        notices: 5,
        contracts: 4,
        conflicts: 2,
        rejected: 1,
      });
    }

    case "현황":
      return wait(현황);

    // 진짜 다리는 고르기 창을 열고 그 자리를 설정에 적어 둔다. 여기서도 그렇게 흉내 낸다.
    case "pickPlanExcel": {
      const path = "C:\\Users\\사람\\Documents\\2026년 조달계획.xlsx";
      settings = { ...settings, planPath: path };
      return wait<PlanPick>({ name: "2026년 조달계획.xlsx", path });
    }

    // 필수 머리글이 없으면 한 줄도 들어가지 않는다 — 그 갈래도 화면에서 볼 수 있게 낸다.
    case "importPlan": {
      const 빠진 = ["조달요구번호", "담당자"].filter((h) => !계획머리글.includes(h));

      if (빠진.length > 0)
        return wait<PlanImportResult>({
          rows: 0, created: 0, updated: 0, skipped: 0,
          missingRequired: 빠진, found: [], ok: false,
        });

      return wait<PlanImportResult>({
        rows: 24, created: 20, updated: 3, skipped: 1,
        missingRequired: [], found: [...계획머리글], ok: true,
      });
    }

    default:
      return Promise.reject(new Error(`모르는 요청입니다: ${method}`));
  }
}

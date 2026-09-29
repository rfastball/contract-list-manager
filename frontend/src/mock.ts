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
  Candidate, DeletionPlan, EntityType, LinkContract, LinkFacet, LinkRequest, MergeResult,
  NoticeChoice, Outline, OutlineContract, OutlineNotice, OutlineNoticeGroup, OutlineRequest,
  PlanImportResult, PlanPick,
  RequestCandidate, RequestLinkFacet, Row, Settings, Sheet, StatusReport, SubmitResult, Summary,
  UserColumn,
} from "./types";

const 통합열 = [
  "계약번호", "계약본번호", "차수", "계약건명", "계약일자", "계약방법", "계약구분",
  "품명", "수량", "단위", "계약금액", "수수료", "지체상금률", "하자보수보증금률",
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
  // 계약방법·계약구분을 줄여 이은 것. 어느 한쪽이라도 줄임표에 없으면 빈칸이다.
  "입찰방법",
  "검토여부", "메모",
];

const 계약열 = [
  "계약번호", "계약본번호", "차수", "계약건명", "계약일자", "계약방법", "계약구분",
  "품명", "수량", "단위", "계약금액", "수수료", "지체상금률", "하자보수보증금률",
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

/** v_통합_v1 에 접수 아홉 열을 더한 것. v1 은 계약면 약속이라 손대지 않고 나란히 세운다. */
const 통합열_v2 = [...통합열, ...통합의접수열];

/** 레코드를 가리키는 열. 고치면 값이 바뀌는 것이 아니라 레코드가 옮겨간다. Views.KeyColumns 와 같다. */
const 키열 = [
  "입찰공고번호", "공고본번호", "계약번호", "계약본번호",
  "접수번호", "접수본번호", "차수", "순번",
  // 건과 그 건의 현행 공고. 파생값이라 덮개를 씌워도 화면의 글자만 바뀐다.
  "공고건", "현행공고",
];

/** 통합이 공고·접수 뷰에서 붙여 오는 열. 이 줄의 키로 주소가 잡히지 않아 통합에서는 잠근다. */
const 통합의붙은열 = 통합열_v2.slice(통합열_v2.indexOf("입찰공고번호"));

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
 * 통합 한 줄. v1 과 v2 가 같은 값에서 나오고 <b>열 목록만 다르다</b> — 붙는 열이 다를 뿐
 * 계약 쪽 몸통은 한 벌이라, 두 벌로 적으면 한쪽이 조용히 늙는다.
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
    지체상금률: "0.075",
    하자보수보증금률: "3",
    하자담보책임기간: "36개월",
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
 * <p>v2 의 줄 하나는 계약이 아니라 <b>조달 건</b>이다 — 접수만 온 것도, 공고까지만 온 것도
 * 표에 선다. 그래야 구조 보기와 표 보기가 같은 것을 센다. v1 은 계약을 줄기로 세운 옛 시트라
 * 계약이 있는 것만 낸다.</p>
 */
function 생애주기행(i: number, 공고까지: boolean): Row {
  const r = 접수씨앗[i];
  const s = 씨앗[i % 씨앗.length];
  const n = String(i + 60).padStart(2, "0");

  return 줄(통합열_v2, {
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

/** 접수 한 줄. 끝 일곱 열은 v_공고_v1 의 규율대로 <b>세부품명이 한 가지일 때만</b> 채운다. */
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
  "v_통합_v1": {
    name: "v_통합_v1",
    columns: 통합열,
    editable: ["진행상태", "메모"],
    correctable: 고칠수있는열(통합열, ["진행상태", "메모"], true),
    // 손으로 고친 칸을 하나 세워 둔다 — 표시와 되돌리기를 볼 자리다.
    overrides: { "R26TA0911050100": { 계약금액: "164,872,340" } },
    rows: Array.from({ length: 42 }, (_, i) => 통합행(i)),
  },
  "v_공고_v1": {
    name: "v_공고_v1",
    columns: 공고열,
    editable: ["검토여부", "메모"],
    correctable: 고칠수있는열(공고열, ["검토여부", "메모"]),
    overrides: {},
    // 갈린 건에서는 <b>현행 한 장만</b> 선다 — 취소된 원공고와 취소공고는 대체되어 빠진다.
    // 그 셋이 나란히 서는 것을 보려면 「차수」 눈으로 옮긴다(v_공고차수_v1).
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
        하자담보기간: "36",
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
        입찰방법: "제한(총액)",
        검토여부: ["미검토", "검토중", "참여", "불참"][i % 4],
      }),
    ), 갈린공고[2]],
  },
  // 통합 탭이 보는 것은 이쪽이다. v1 은 계약면 약속이라 남겨 두고 나란히 세운다.
  "v_통합_v2": {
    name: "v_통합_v2",
    columns: 통합열_v2,
    editable: ["진행상태", "메모"],
    correctable: 고칠수있는열(통합열_v2, ["진행상태", "메모"], true),
    overrides: { "R26TA0911050100": { 계약금액: "164,872,340" } },
    // 계약이 아직 없는 두 줄을 끝에 세운다 — 접수+공고 하나, 접수만 하나.
    // 화면이 그런 줄을 어떻게 가리키고 어떻게 잠그는지 보는 자리다.
    rows: [
      ...Array.from({ length: 42 }, (_, i) => 통합행(i, 통합열_v2)),
      생애주기행(0, true),
      생애주기행(2, false),
    ],
  },
  // 계획. 분모라 아직 아무것도 오지 않은 줄(미착수)이 대부분이고, 뒤로 갈수록 차 있다.
  // 이름·번호는 모두 지어낸 것이다.
  "v_계획_v1": {
    name: "v_계획_v1",
    columns: 계획열,
    // 읽기 전용이다. 진짜 다리도 계획 뷰에는 빈 목록을 낸다(Bridge.ReadSheet).
    editable: [],
    correctable: [],
    overrides: {},
    rows: Array.from({ length: 24 }, (_, i) => 계획행(i)),
  },
  "v_접수_v1": {
    name: "v_접수_v1",
    columns: 접수열,
    editable: ["확인여부", "메모"],
    correctable: 고칠수있는열(접수열, ["확인여부", "메모"]),
    overrides: {},
    rows: 접수씨앗.map((_, i) => 접수행(i)),
  },
  "v_계약_v1": {
    name: "v_계약_v1",
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
  ],
  request: [
    { entityType: "request", fieldName: "확인여부", kind: "text", choices: [] },
    { entityType: "request", fieldName: "메모", kind: "text", choices: [] },
  ],
};

const 모든열 = () => [...userColumns.contract, ...userColumns.notice, ...userColumns.request];

/**
 * 통합 v3 이 내는 열. v2 끝에 <b>둘만 더한 것</b>이다 — 이름을 바꾼 것이 아니라 더한 것이라
 * 저쪽에 저장된 필드 연결이 끊기지 않는다.
 */
const 통합열_v3 = [...통합열_v2, "공고건", "현행공고"];

// ── 차수를 편 표 셋 ─────────────────────────────────────────────
//
// 본 뷰와 <b>열이 한 글자도 다르지 않다</b>. 실제 뷰도 열 한 벌을 나눠 쓰고 계약면 시험이 그
// 성질을 붙들고 있으니, 여기서도 본 뷰의 열 목록을 그대로 가리킨다 — 베껴 적으면 갈린다.
//
// 셋 다 <b>읽기 전용</b>이라 다리처럼 빈 목록을 낸다(Views.ReadOnly). 그래서 아래 열맞추기의
// 시트 목록에도 들지 않는다 — 거기 넣으면 사람 열을 세울 때 editable 이 채워져, 고칠 수 없어야
// 할 표가 조용히 열린다.

sheets["v_공고차수_v1"] = {
  name: "v_공고차수_v1",
  columns: 공고열,
  editable: [],
  correctable: [],
  overrides: {},
  // 최신 표의 마지막 줄이 갈린 건의 현행이다. 그 자리를 셋으로 펴 놓는다.
  rows: [...sheets["v_공고_v1"].rows.slice(0, -1), ...갈린공고],
};

sheets["v_계약차수_v1"] = {
  name: "v_계약차수_v1",
  columns: 계약열,
  editable: [],
  correctable: [],
  overrides: {},
  // 지어낸 이 자료에는 변경계약이 없어 최신 표와 줄이 같다. 없는 것을 지어 넣지 않는다 —
  // 차수 뷰가 늘 더 많은 줄을 낸다고 믿게 만드는 편이 빈 것보다 나쁘다.
  rows: sheets["v_계약_v1"].rows,
};

sheets["v_통합_v3"] = {
  name: "v_통합_v3",
  columns: 통합열_v3,
  editable: [],
  correctable: [],
  overrides: {},
  rows: [
    ...sheets["v_통합_v2"].rows.map((r) =>
      줄(통합열_v3, {
        ...r,
        // 갈리지 않은 건이라 공고번호에서 차수만 떼면 그것이 곧 건 이름이다.
        공고건: r.입찰공고번호 ? r.입찰공고번호.slice(0, -4) : "",
        현행공고: r.입찰공고번호,
      }),
    ),
    // 갈린 건은 공고 셋이 나란히 서고 계약·접수 열은 빈 채로 남는다 — 이어진 것이 없어서다.
    ...갈린공고.map((n) =>
      줄(통합열_v3, {
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

/** 그 개체의 사람 열이 서는 시트들. 통합은 계약 쪽이라 v1·v2 가 함께 움직인다. */
const 시트 = (entityType: Entity) => ({
  contract: ["v_통합_v1", "v_통합_v2", "v_계약_v1"],
  notice: ["v_공고_v1"],
  request: ["v_접수_v1"],
}[entityType]);

function 열맞추기(entityType: Entity) {
  const names = userColumns[entityType].map((c) => c.fieldName);

  for (const key of 시트(entityType)) {
    const sheet = sheets[key];
    const base = 기계열[key];

    sheet.columns = [...base.columns.slice(0, base.at), ...names, ...base.columns.slice(base.at)];
    sheet.editable = names;

    for (const row of sheet.rows) for (const n of names) row[n] ??= "";
  }
}

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

function facets(같은건명: boolean, 같은기관: boolean): LinkFacet[] {
  return [
    { name: "건명", contract: "26년 가람 절단기 구매", notice: "26년 가람 절단기 구매", agrees: 같은건명 },
    { name: "세부품명", contract: "플라즈마절단기", notice: "플라즈마절단기", agrees: true },
    { name: "물품식별번호", contract: "29077509", notice: "29077509", agrees: true },
    { name: "수요기관", contract: "가람군수지원단", notice: 같은기관 ? "가람군수지원단" : "한별군수지원단", agrees: 같은기관 },
    { name: "계약일 / 게시일", contract: "", notice: "2026-07-03", agrees: null },
  ];
}

let candidates: Candidate[] = [
  {
    contractKey: "R26TA0999000100", contractTitle: "26년 가람 절단기 구매",
    noticeKey: "R26BK09017030-001", noticeTitle: "26년 가람 절단기 구매",
    confidence: 0.9, reason: "건명 일치", noticeContractCount: 0,
    titleMatched: true, blocker: "같은 이름 공고가 2건입니다", facets: facets(true, true),
  },
  {
    contractKey: "R26TA0999000100", contractTitle: "26년 가람 절단기 구매",
    noticeKey: "R26BK09017099-000", noticeTitle: "26년 가람 절단기 구매",
    confidence: 0.9, reason: "건명 일치", noticeContractCount: 1,
    titleMatched: true, blocker: "같은 이름 공고가 2건입니다", facets: facets(true, false),
  },
  {
    contractKey: "R26TA0999000200", contractTitle: "26년 누림 항공유 분석장비 구매",
    noticeKey: "R26BK09017050-000", noticeTitle: "26년 누림 유분석장비 구매",
    confidence: 0.62, reason: "물품식별번호 일치 · 건명 유사 40%", noticeContractCount: 0,
    titleMatched: false, blocker: null, facets: facets(false, true),
  },
];

/** 이어진 줄을 둘 섞어 둔다 — 끊고 다시 잇는 길이 화면에 서는지 여기서 본다. */
let 계약줄: LinkContract[] = [
  { key: "R26TA0999000100", title: "26년 가람 절단기 구매", candidateCount: 2,
    noticeKey: null, noticeTitle: "", decidedBy: null },
  { key: "R26TA0999000200", title: "26년 누림 항공유 분석장비 구매", candidateCount: 1,
    noticeKey: null, noticeTitle: "", decidedBy: null },
  { key: "R26TA0999000300", title: "26년 한별 소나 부품 구매", candidateCount: 0,
    noticeKey: null, noticeTitle: "", decidedBy: null },
  { key: "R26TA0999000400", title: "26년 한별 공압식승강판 20톤 구매", candidateCount: 0,
    noticeKey: "R26BK09017040-000", noticeTitle: "26년 한별 공압식승강판 20톤 구매", decidedBy: "auto" },
  { key: "R26TA0999000500", title: "26년 가람 수질측정기 조달", candidateCount: 0,
    noticeKey: "R26BK09017060-000", noticeTitle: "26년 가람 수질측정기 조달", decidedBy: "human" },
];

/** 「직접 찾기」가 훑을 자리. 추천에 오르지 않은 공고도 섞어 둔다. */
const noticeChoices: NoticeChoice[] = [
  { key: "R26BK09017030-001", title: "26년 가람 절단기 구매", postedAt: "2026-07-02", linked: 0 },
  { key: "R26BK09017099-000", title: "26년 가람 절단기 구매", postedAt: "2026-07-01", linked: 1 },
  { key: "R26BK09017050-000", title: "26년 누림 유분석장비 구매", postedAt: "2026-06-30", linked: 0 },
  { key: "R26BK09017040-000", title: "26년 한별 공압식승강판 20톤 구매", postedAt: "2026-06-29", linked: 1 },
  { key: "R26BK09017060-000", title: "26년 가람 수질측정기 조달", postedAt: "2026-06-28", linked: 1 },
  { key: "R26BK09017070-000", title: "26년 누림 항공기 부품 구매", postedAt: "2026-06-27", linked: 0 },
];

/** 아직 이어지지 않은 줄이 몇인가. 머리의 배지가 이 수를 그대로 받는다. */
const 미연결수 = () => 계약줄.filter((c) => c.noticeKey === null).length;

// ── 접수 ↔ 공고 ──────────────────────────────────────
// 계약 쪽과 얼개는 같고 판정만 다르다 — 건명이 아니라 품목 줄 수로 좁히고, 수량·단가
// 다중집합이 완전히 같을 때만 통과한다. 통과 후보가 둘이면 기계가 잇지 않고 사람에게 넘긴다.

/** 접수 쪽 견줌. 세부품명번호는 <b>어긋나도 막지 않는다</b> — 협의·요청 누락으로 다를 수 있다. */
function 접수견줌(요청명: string, 공고명: string): RequestLinkFacet[] {
  return [
    { name: "건명", request: 요청명, notice: 공고명, agrees: 요청명 === 공고명 },
    { name: "품목수", request: "3", notice: "3", agrees: true },
    { name: "수량·단가", request: "3줄 전부 일치", notice: "3줄 전부 일치", agrees: true },
    { name: "세부품명", request: "플라즈마절단기", notice: "플라즈마절단기", agrees: true },
    { name: "세부품명번호", request: "29077509", notice: "29077511", agrees: false },
    { name: "품대 / 사업금액", request: "29,512,000", notice: "29,512,000", agrees: true },
    { name: "예산금액 / 배정예산", request: "35,709,520", notice: "36,299,760", agrees: false },
    { name: "수요기관", request: "가람군수지원단", notice: "가람군수지원단", agrees: true },
    { name: "접수일자 / 게시일시", request: "2026/06/24", notice: "2026/07/02", agrees: true },
  ];
}

let 접수줄: LinkRequest[] = [
  { key: "MPKPLA26910286", title: "26년한별윈치8종구매", candidateCount: 0,
    noticeKey: "R26BK09018047-000", noticeTitle: "26년한별윈치8종구매", decidedBy: "auto" },
  { key: "MPKPLA26910301", title: "26년가람수질측정기구매", candidateCount: 0,
    noticeKey: "R26BK09017060-000", noticeTitle: "26년 가람 수질측정기 조달", decidedBy: "human" },
  { key: "MPKPLA26910412", title: "26년가람절단기구매", candidateCount: 2,
    noticeKey: null, noticeTitle: "", decidedBy: null },
];

let requestCandidates: RequestCandidate[] = [
  {
    requestKey: "MPKPLA26910412", requestTitle: "26년가람절단기구매",
    noticeKey: "R26BK09017030-001", noticeTitle: "26년 가람 절단기 구매",
    confidence: 0.95, reason: "품목 3줄 · 수량·단가 전부 일치", noticeLinkedCount: 0,
    // 줄 수가 같은 공고는 둘이지만 뒤엣것은 이미 임자가 있다 — 통과한 후보가 하나뿐이라
    // 막을 까닭이 없다. 유일성을 통과 후보 사이에서 세는 규칙이 그대로 비치는 자리다.
    titleMatched: false, blocker: null,
    facets: 접수견줌("26년가람절단기구매", "26년 가람 절단기 구매"),
  },
  {
    requestKey: "MPKPLA26910412", requestTitle: "26년가람절단기구매",
    noticeKey: "R26BK09017099-000", noticeTitle: "26년 가람 절단기 구매",
    confidence: 0.95, reason: "품목 3줄 · 수량·단가 전부 일치", noticeLinkedCount: 1,
    titleMatched: false, blocker: "그 공고에 이미 다른 접수가 붙어 있습니다",
    facets: 접수견줌("26년가람절단기구매", "26년 가람 절단기 구매"),
  },
];

/** 접수 쪽 「직접 찾기」가 훑을 자리. 계약 쪽이 훑는 것에 접수가 붙은 공고를 더한다. */
const 접수쪽공고: NoticeChoice[] = [
  { key: "R26BK09018047-000", title: "26년한별윈치8종구매", postedAt: "2026-07-18", linked: 1 },
  ...noticeChoices,
];

const 미연결접수수 = () => 접수줄.filter((r) => r.noticeKey === null).length;

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
  key: "MPKPLA26910450",
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
  { entityType: "contract", view: "v_계약_v1", 제목열: "계약건명", 차수길이: 2 },
  { entityType: "notice", view: "v_공고_v1", 제목열: "공고명", 차수길이: 3 },
  { entityType: "request", view: "v_접수_v1", 제목열: "요청명", 차수길이: 3 },
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

/** 화면이 부르는 것을 그대로 받는다. 이름과 인자는 Bridge.cs 의 switch 와 같아야 한다. */
export function invoke(method: string, args: unknown[]): Promise<unknown> {
  const wait = <T>(value: T) => new Promise<T>((r) => setTimeout(() => r(value), 60));

  switch (method) {
    case "prepareExtension":
    case "openExtensionSetup":
    case "extensionStatus":
      return Promise.reject(new Error("확장 설치 준비는 계약목록.exe에서 실행하세요."));
    case "erpTools":
      if (args[0] === "list") return wait({ active: JSON.parse(mappingJson), defaults: JSON.parse(mappingJson), versions: [] });
      return Promise.reject(new Error("개발 미리보기입니다. 매핑 검증·자료 저장은 메인 프로그램에서 실행하세요."));
    case "status":
      return wait(summary);

    case "sheet":
      return wait(sheets[args[0] as string]);

    case "userColumns": {
      const which = args[0] as Entity | undefined;
      return wait(which ? userColumns[which] : 모든열());
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

        // 이름이 바뀌면 적어 둔 값도 따라간다.
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

      계약줄 = 계약줄.filter((c) => c.key !== key.slice(0, -2));
      접수줄 = 접수줄.filter((r) => r.key !== key.slice(0, -4));

      summary.contractBases = sheets["v_계약_v1"].rows.length;
      summary.contractRows = summary.contractBases;
      summary.requestBases = sheets["v_접수_v1"].rows.length;
      summary.requestRows = summary.requestBases;
      summary.unlinkedContracts = 미연결수();
      summary.unlinkedRequests = 미연결접수수();

      return wait(null);
    }

    case "outline":
      return wait(outline);

    case "settings":
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

    case "linkCandidates":
      return wait({ contracts: 계약줄, candidates, notices: noticeChoices });

    // 진짜 다리는 기계가 이은 것만 풀고 다시 판정한다. 여기서는 그 결과의 꼴만 낸다.
    case "relink":
      return wait({ linked: 1, released: 0 });

    case "dataLocation":
      return wait({
        path: "C:\\Users\\홍길동\\AppData\\Local\\Pclm\\pclm.db",
        folder: "C:\\Users\\홍길동\\AppData\\Local\\Pclm",
        sizeBytes: 192512,
        isDefault: true,
        configPath: "C:\\Users\\홍길동\\AppData\\Roaming\\Pclm\\config.json",
      });
    case "moveDataLocation":
      return wait({ folder: "D:\\계약목록_자료" });
    // 진짜 다리는 exe 안에 박힌 전문을 준다. 여기서는 꼴만.
    case "about":
      return wait({
        version: "0.5.0",
        license: "MIT License\n\nCopyright (c) 2026 JM\n\n(개발 미리보기 — 전문은 exe 안에 있습니다)",
        notices: "계약 목록 — 제3자 고지\n\n== nuget ClosedXML 0.105.1 — MIT\n== npm react 19.2.8 — MIT",
      });
    case "revealDataFolder":
      return wait({ folder: "C:\\Users\\홍길동\\AppData\\Local\\Pclm" });

    // 이어도 계약은 목록에 남는다 — 잘못 이었으면 그 줄에서 끊어야 한다.
    case "confirmLink": {
      const [contractKey, noticeKey] = args as [string, string];
      const 공고 = noticeChoices.find((n) => n.key === noticeKey);

      candidates = candidates.filter((c) => c.contractKey !== contractKey);
      계약줄 = 계약줄.map((c) => c.key === contractKey
        ? { ...c, candidateCount: 0, noticeKey, noticeTitle: 공고?.title ?? "", decidedBy: "human" }
        : c);

      summary.unlinkedContracts = 미연결수();
      return wait(null);
    }

    case "rejectLink": {
      const [contractKey, noticeKey] = args as [string, string];
      candidates = candidates.filter(
        (c) => !(c.contractKey === contractKey && c.noticeKey === noticeKey),
      );

      // 물리쳐도 계약은 목록에 남는다 — 이어지지 않은 채로 남았다는 사실이 사라지면 안 된다.
      계약줄 = 계약줄.map((c) =>
        c.key === contractKey
          ? { ...c, candidateCount: candidates.filter((x) => x.contractKey === contractKey).length }
          : c,
      );

      return wait(null);
    }

    // 풀기만 한다. 거부와 달리 "아니다" 라는 판정은 남기지 않는다.
    case "unlink": {
      const [contractKey] = args as [string];
      계약줄 = 계약줄.map((c) =>
        c.key === contractKey ? { ...c, noticeKey: null, noticeTitle: "", decidedBy: null } : c);

      summary.unlinkedContracts = 미연결수();
      return wait(null);
    }

    // 추천에 없는 짝도 나란히 견준다. 진짜 다리는 DB 를 읽지만 여기서는 건명만 맞대어 본다.
    case "compareLink": {
      const [contractKey, noticeKey] = args as [string, string];
      const 계약 = 계약줄.find((c) => c.key === contractKey);
      const 공고 = noticeChoices.find((n) => n.key === noticeKey);

      if (!계약 || !공고)
        return Promise.reject(new Error(`견줄 짝을 찾지 못했습니다: ${contractKey} ← ${noticeKey}`));

      return wait<LinkFacet[]>([
        { name: "건명", contract: 계약.title, notice: 공고.title, agrees: 계약.title === 공고.title },
        { name: "세부품명", contract: "플라즈마절단기", notice: "플라즈마절단기", agrees: true },
        { name: "물품식별번호", contract: "29077509", notice: "29077509", agrees: true },
        { name: "수요기관", contract: "가람군수지원단", notice: "가람군수지원단", agrees: true },
        { name: "계약일 / 게시일", contract: "2026-07-22", notice: 공고.postedAt, agrees: true },
      ]);
    }

    case "requestLinkCandidates":
      return wait({ requests: 접수줄, candidates: requestCandidates, notices: 접수쪽공고 });

    case "confirmRequestLink": {
      const [requestKey, noticeKey] = args as [string, string];
      const 공고 = 접수쪽공고.find((n) => n.key === noticeKey);

      requestCandidates = requestCandidates.filter((c) => c.requestKey !== requestKey);
      접수줄 = 접수줄.map((r) => r.key === requestKey
        ? { ...r, candidateCount: 0, noticeKey, noticeTitle: 공고?.title ?? "", decidedBy: "human" }
        : r);

      summary.unlinkedRequests = 미연결접수수();
      return wait(null);
    }

    case "rejectRequestLink": {
      const [requestKey, noticeKey] = args as [string, string];
      requestCandidates = requestCandidates.filter(
        (c) => !(c.requestKey === requestKey && c.noticeKey === noticeKey),
      );

      접수줄 = 접수줄.map((r) =>
        r.key === requestKey
          ? { ...r, candidateCount: requestCandidates.filter((x) => x.requestKey === requestKey).length }
          : r,
      );

      return wait(null);
    }

    case "unlinkRequest": {
      const [requestKey] = args as [string];
      접수줄 = 접수줄.map((r) =>
        r.key === requestKey ? { ...r, noticeKey: null, noticeTitle: "", decidedBy: null } : r);

      summary.unlinkedRequests = 미연결접수수();
      return wait(null);
    }

    case "compareRequestLink": {
      const [requestKey, noticeKey] = args as [string, string];
      const 접수 = 접수줄.find((r) => r.key === requestKey);
      const 공고 = 접수쪽공고.find((n) => n.key === noticeKey);

      if (!접수 || !공고)
        return Promise.reject(new Error(`견줄 짝을 찾지 못했습니다: ${requestKey} ← ${noticeKey}`));

      return wait<RequestLinkFacet[]>(접수견줌(접수.title, 공고.title));
    }

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

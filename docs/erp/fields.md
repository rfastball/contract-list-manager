# 수집 필드 목록

필드 하나(프로필 · 출처 · ID)마다 **지금 상태 하나**를 적은 한 벌이다(ADR-043). 담는 기준은 쓰임이 아니라 앎이다 —
뜻이 확정된 필드는 담고, 담지 않는 것은 개인정보·이미 담은 값의 다른 출처·사람이 뜻이 없다고 확인한 값뿐이다.
이 표와 기본 매핑(`src/Pclm.Core/Erp/mapping.json`)이 어긋나면 `tests/Pclm.Core.Tests/FieldRegistryTests.cs` 가 실패한다.

어떻게 이 상태에 이르렀는지는 연혁으로 남은 [전수 매핑표](json-field-mapping.md)·[확인 질문](json-field-questions.md)·
[옛 일괄 제외](json-unused-candidates.md)에 있다. 거기 적힌 「미사용」·「보류」 는 지금 상태가 아니다.
사람이 화면에서 뜻을 확인하면 그 커밋에서 이 표·매핑·면 뷰·규약 문서를 함께 고친다.

## 상태

| 상태 | 뜻 |
|---|---|
| `수집` | 뜻이 확정되어 담는다. 확정은 사람이 뜻을 확인했거나, 같은 원문의 명칭 칸(코드 옆 `…Nm`)·화면 도움말로 뜻이 직접 드러난 것이다. 지금 매핑에 있는 필드는 모두 이것이다(2026-09-27 승인을 거친 매핑). 확정인데 아직 매핑에 없는 행은 시험의 `NotYetMapped` 에만 있다 — 담으면 거기서 지운다. |
| `제외: 개인정보` | 주민등록번호·계좌번호. 알아도 들어올 자리가 없다(ADR-005). |
| `제외: 다른 출처` | 담는(또는 담을) 값과 같은 뜻의 다른 칸 — 같은 내용의 중복 표, 선택 품목의 사본, 명칭 칸이 따로 담기는 코드. |
| `제외: 뜻 없음` | 사람이 업무상 뜻이 없다고 확인한 값. |
| `후보` | 뜻이 ID·표 맥락의 짐작이거나 미해석이다. 「쓰지 않음」 은 이유가 아니므로 옛 미사용·일괄 제외도 여기로 돌아왔다. 짐작해 확정하지 않는다. |

뜻은 ID 단위로 정해진다(ADR-043 결정 6) — 같은 문자열의 ID 는 어느 화면·표에서도 같은 뜻이라 `수집`·`제외: 뜻 없음`·`후보` 는
그 ID 의 모든 행에 같고, 자리마다 갈리는 것은 그 자리 값이 사본인지(`제외: 다른 출처`)뿐이다. 어긋나면 시험이 잡는다.

## 읽는 법

- 사람의 확인은 검토지(필드마다 답을 고르는 화면)로 받아 이 표에 반영한다. 그날 바뀐 행의 근거에 `사용자 확인 날짜(필드 확정 검토)`
  로 남고, 같은 ID 원칙으로 따라 바뀐 행은 `같은 ID 원칙(날짜) — <프로필 · 출처> 에서 확정` 으로 남는다.

- **프로필**은 `mapping.json` 의 profile id 다. 전수조사 코퍼스(JSON 15개)는 01117·01179·01196·01579 네 화면의 것이라 그 넷의
  프로필에 전수 행을 둔다. 같은 꼴을 읽는 다른 프로필(01112·01181·01608·공개 공고 머리글)에는 그 프로필의 매핑에 있거나 그 화면의
  코퍼스에서 관찰을 적은 필드만 둔다 — 화면이 다르면 같은 출처 이름이어도 같은 필드가 있다는 근거가 없다.
- **출처**는 `pointInfo` 또는 매핑이 쓰는 표 출처 문자열 그대로다(와일드카드 `*_grdFile` 포함). 머리 필드가 `tables.<표>.<행>.<ID>`
  로 표 칸을 읽으면 출처는 그 표다.
- 식별 필드(`baseField`·`seqField`·표의 `keys`)도 매핑이 읽어 문서 번호·행 키로 담으므로 `수집` 이다. `secondarySource`(날짜에
  붙는 시각)도 같다.
- **근거**는 `사용자 확인 날짜(기록 위치)` · `원문 명칭 <Nm 필드>` · `화면 도움말`, 후보는 짐작의 근거(`ID·표 맥락`·`빈 열만 확인`·
  `구조·이름 분류`·`미해석`)다. 「옛 …」 꼬리는 ADR-043 전에 쓰임을 이유로 뺐던 기록이다. 「09-27 매핑 · 개별 확인 기록 없음」 은
  그날 승인한 매핑에 들었으나 필드별 확인 기록을 문서에서 찾지 못한 것이다.
- 값·건명·기관·금액·날짜는 적지 않는다. 뜻 칸에는 코드→명칭 쌍만 적는다.

## `g2b-request-v1` — 조달요구 접수(목록형) (01117)

수집 47 · 제외: 다른 출처 54 · 제외: 뜻 없음 10 · 후보 17

| 프로필 | 출처 | ID | 뜻 | 상태 | 근거 |
|---|---|---|---|---|---|
| `g2b-request-v1` | `pointInfo` | `(빈 ID)` | 빈 ID(출처 복원 불가) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-request-v1` | `pointInfo` | `advfGivePsbltyRat` | 선급금 지급가능 비율 | 후보 | ID·표 맥락 · 옛 제외(Q04 「사용하지 않는 값」) |
| `g2b-request-v1` | `pointInfo` | `advfGivePsbltyYn` | 선급금 지급가능 여부 | 후보 | ID·표 맥락 · 옛 제외(Q04 「사용하지 않는 값」) |
| `g2b-request-v1` | `pointInfo` | `advfPinfrmPsbltyRat` | 선급금 선고지 가능 비율(%) | 수집 | 사용자 확인 2026-09-27(질문 Q04 부분 확정) |
| `g2b-request-v1` | `pointInfo` | `advfPinfrmYn` | 선급금 선고지 여부 | 수집 | 사용자 확인 2026-09-27(일괄 답변 7 / R08) |
| `g2b-request-v1` | `pointInfo` | `albmNopg` | 면수 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본 |
| `g2b-request-v1` | `pointInfo` | `antcPrcmFee` | 예상 조달수수료 | 수집 | 사용자 확인 2026-09-27(질문 Q01 일괄 승인) |
| `g2b-request-v1` | `pointInfo` | `areaCd` | 화면/영역 코드(업무 의미 미확정) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-request-v1` | `pointInfo` | `bgtAmt` | 예산금액 | 수집 | 사용자 확인 2026-09-27(질문 Q01 일괄 승인) |
| `g2b-request-v1` | `pointInfo` | `clsfCrtrBidYn` | 분류별 입찰 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-request-v1` | `pointInfo` | `ctrtDmndAmt` | 계약요청 품목금액 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본 |
| `g2b-request-v1` | `pointInfo` | `ctrtDmndBizNm` | 계약요청 사업명(요청명) | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-v1` | `pointInfo` | `ctrtDmndQty` | 계약요청 품목수량 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본 |
| `g2b-request-v1` | `pointInfo` | `ctrtDmndUprc` | 계약요청 품목단가 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본 |
| `g2b-request-v1` | `pointInfo` | `ctrtTyCd` | 계약구분(계120001 → 총액계약) | 수집 | 사용자 확인 2026-09-27(일괄 답변 10 / R11) |
| `g2b-request-v1` | `pointInfo` | `depth1` | 화면 경로 단계 후보 | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-request-v1` | `pointInfo` | `depth2` | 화면 경로 단계 후보 | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-request-v1` | `pointInfo` | `devyCndtCd` | 인도조건 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토) · 머리 칸은 화면에서 고른 행의 사본 |
| `g2b-request-v1` | `pointInfo` | `dlvgdsPlacNm` | 납품장소 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본 |
| `g2b-request-v1` | `pointInfo` | `dlvgdsTermNody` | 납품일수 | 제외: 다른 출처 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridView 에서 확정 |
| `g2b-request-v1` | `pointInfo` | `dmndInstBizPicNm` | 수요기관 업무담당자명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-v1` | `pointInfo` | `dmndInstBizPicTlphNo` | 수요기관 업무담당자 전화번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-v1` | `pointInfo` | `dmndInstPicFaxNo` | 수요기관 담당자 팩스번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-v1` | `pointInfo` | `dmstAcntgCd` | 수요기관 회계구분 | 제외: 다른 출처 | 원문 명칭 dmstAcntgNm 을 수집 |
| `g2b-request-v1` | `pointInfo` | `dmstAcntgNm` | 수요기관 회계구분 | 수집 | 원문 명칭 dmstAcntgNm(dmstAcntgCd 옆) |
| `g2b-request-v1` | `pointInfo` | `dmstNm` | 수요기관명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-v1` | `pointInfo` | `dmstUntyGrpNo` | 수요기관 통합기관번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-v1` | `pointInfo` | `drpmSbpySeCd` | 지급방법(직130001 → 직불, 직130002 → 대지급) | 수집 | 사용자 확인 2026-09-27(일괄 답변 10 / R11) |
| `g2b-request-v1` | `pointInfo` | `dtlsPrnm` | 세부품명 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본 |
| `g2b-request-v1` | `pointInfo` | `dtlsPrnmNo` | 세부품명번호 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본 |
| `g2b-request-v1` | `pointInfo` | `egnrEvlTrgtYn` | 기술평가 대상 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-request-v1` | `pointInfo` | `engDmstNm` | 영문 수요기관명 | 후보 | ID·표 맥락 |
| `g2b-request-v1` | `pointInfo` | `fprdYn` | 외국산 허용 여부 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-v1` | `pointInfo` | `itemIdnfNm` | 물품식별명/규격 표시 후보 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본 |
| `g2b-request-v1` | `pointInfo` | `itemIdnfNo` | 물품식별번호 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본 |
| `g2b-request-v1` | `pointInfo` | `lcrtTyCd` | 계약법 구분(계040001 → 국가계약법, 계040002 → 지방계약법) | 수집 | 사용자 확인 2026-09-27(일괄 답변 10 / R11) |
| `g2b-request-v1` | `pointInfo` | `ndfsItemSpecNo` | 규격번호 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본 |
| `g2b-request-v1` | `pointInfo` | `ndfsIvtrNo` | 재고번호 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본 |
| `g2b-request-v1` | `pointInfo` | `ndfsPrcmCtrtPlanYear` | 국방 조달계획연도 후보 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본 |
| `g2b-request-v1` | `pointInfo` | `ndfsPrcmDmndNo` | 국방 조달요구번호 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본 |
| `g2b-request-v1` | `pointInfo` | `newLotmSeCd` | 신규·장기 구분 후보 | 후보 | ID·표 맥락 |
| `g2b-request-v1` | `pointInfo` | `pbancKndCd` | 공고종류 | 수집 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · pointInfo 에서 확정 |
| `g2b-request-v1` | `pointInfo` | `prcmBsneSeNm` | 조달업무구분명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-v1` | `pointInfo` | `prspPrce` | 추정가격 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-v1` | `pointInfo` | `qtyUntNm` | 수량단위명 | 제외: 다른 출처 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridView 에서 확정 |
| `g2b-request-v1` | `pointInfo` | `rcptBraOrgnCd` | 접수 지청·조직코드 후보 | 후보 | ID·표 맥락 |
| `g2b-request-v1` | `pointInfo` | `rlsYn` | 공개 여부 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-v1` | `pointInfo` | `rtrcnYn` | 취소 여부 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본 |
| `g2b-request-v1` | `pointInfo` | `scsbdMthdCd` | 낙찰방법 | 수집 | 사용자 확인 2026-09-27(일괄 답변 10 / R11) |
| `g2b-request-v1` | `pointInfo` | `slCprcmBsneSeCd` | 조달업무구분 후보 | 후보 | ID·표 맥락 |
| `g2b-request-v1` | `pointInfo` | `stdCtrtMthdCd` | 계약방법 | 수집 | 사용자 확인 2026-09-27(일괄 답변 10 / R11) |
| `g2b-request-v1` | `pointInfo` | `totlPymtAmt` | 품대 | 수집 | 사용자 확인 2026-09-27(질문 Q01 일괄 승인) |
| `g2b-request-v1` | `pointInfo` | `tpSrngTrgtYn` | 심사대상 여부(약어 미확정) | 후보 | ID·표 맥락 |
| `g2b-request-v1` | `pointInfo` | `wrtYmd` | 접수일 | 수집 | 사용자 확인 2026-09-27(질문 Q07 부분 확정) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `CHK` | 행 선택 상태 | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `albmNopg` | 면수 | 제외: 다른 출처 | mf_wfm_container_gridView 와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `ctrtDmndAmt` | 계약요청 품목금액 | 제외: 다른 출처 | mf_wfm_container_gridView 와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `ctrtDmndQty` | 계약요청 품목수량 | 제외: 다른 출처 | mf_wfm_container_gridView 와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `ctrtDmndRcptItemSqno` | 계약요청 접수 품목순번 | 제외: 다른 출처 | mf_wfm_container_gridView 와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `ctrtDmndRcptNo` | 계약요청 접수번호 | 제외: 다른 출처 | mf_wfm_container_gridView 와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `ctrtDmndRcptOrd` | 계약요청 접수차수 | 제외: 다른 출처 | mf_wfm_container_gridView 와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `ctrtDmndUprc` | 계약요청 품목단가 | 제외: 다른 출처 | mf_wfm_container_gridView 와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `devyCndtCd` | 인도조건 | 제외: 다른 출처 | mf_wfm_container_gridView 와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `devyCndtNm` | 인도조건 | 제외: 다른 출처 | mf_wfm_container_gridView 와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `dlvgdsPlacNm` | 납품장소 | 제외: 다른 출처 | mf_wfm_container_gridView 와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `dlvgdsTermDt` | 납품기한 날짜 | 제외: 다른 출처 | mf_wfm_container_gridView 와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `dlvgdsTermNody` | 납품일수 | 제외: 다른 출처 | mf_wfm_container_gridView 와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `dtlsPrnm` | 세부품명 | 제외: 다른 출처 | mf_wfm_container_gridView 와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `dtlsPrnmNo` | 세부품명번호 | 제외: 다른 출처 | mf_wfm_container_gridView 와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `extlPbfnAsetDmndSqnoVal` | 외부 재정 자산요청 순번 후보 | 제외: 다른 출처 | mf_wfm_container_gridView 와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `inspTyCd` | 검사형태 | 제외: 다른 출처 | mf_wfm_container_gridView 와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `itemCfnm` | 물품명 후보(정확한 용어 미확정) | 제외: 다른 출처 | mf_wfm_container_gridView 와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `itemClsfNo` | 물품분류번호 | 제외: 다른 출처 | mf_wfm_container_gridView 와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `itemIdnfNm` | 물품식별명/규격 표시 후보 | 제외: 다른 출처 | mf_wfm_container_gridView 와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `itemIdnfNo` | 물품식별번호 | 제외: 다른 출처 | mf_wfm_container_gridView 와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `itrlPrcmAsetNoVal` | 내부 조달 자산번호 후보 | 제외: 다른 출처 | mf_wfm_container_gridView 와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `itrlPrcmItemIdnfNm` | 내부 조달 물품식별명 후보 | 제외: 다른 출처 | mf_wfm_container_gridView 와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `itrlPrcmItemIdnfNoVal` | 내부 조달 물품식별번호 후보 | 제외: 다른 출처 | mf_wfm_container_gridView 와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `ndfsItemDlvgdsMtruCdVal` | 국방 품목 납품 관련 구분 코드/명(약어 미확정) | 제외: 다른 출처 | mf_wfm_container_gridView 와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `ndfsItemSpecNo` | 규격번호 | 제외: 다른 출처 | mf_wfm_container_gridView 와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `ndfsIvtrNo` | 재고번호 | 제외: 다른 출처 | mf_wfm_container_gridView 와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `ndfsPrcmCtrtPlanYear` | 국방 조달계획연도 후보 | 제외: 다른 출처 | mf_wfm_container_gridView 와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `ndfsPrcmDmndNo` | 국방 조달요구번호 | 제외: 다른 출처 | mf_wfm_container_gridView 와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `ndfsPrcmItemDlvgdsMtruNm` | 국방 품목 납품 관련 구분 코드/명(약어 미확정) | 제외: 다른 출처 | mf_wfm_container_gridView 와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `ndfsPrcmPlanChgOrd` | 국방 조달계획 변경차수 | 제외: 다른 출처 | mf_wfm_container_gridView 와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `ndfsPrcmPlanDlvgdsSqno` | 국방 조달계획 납품순번 | 제외: 다른 출처 | mf_wfm_container_gridView 와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `ndpfExpndDmndNoVal` | 국방 지출요청번호 후보 | 제외: 다른 출처 | mf_wfm_container_gridView 와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `no` | 표시/출력/내부 순번(업무 키로 사용 금지) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `qtyUntNm` | 수량단위명 | 제외: 다른 출처 | mf_wfm_container_gridView 와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-request-v1` | `mf_wfm_container_gridViewExcel` | `rtrcnYn` | 취소 여부 | 제외: 다른 출처 | mf_wfm_container_gridView 와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `CHK` | 행 선택 상태 | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `albmNopg` | 면수 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `ctrtDmndAmt` | 계약요청 품목금액 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `ctrtDmndQty` | 계약요청 품목수량 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `ctrtDmndRcptItemSqno` | 계약요청 접수 품목순번 | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `ctrtDmndRcptNo` | 계약요청 접수번호 | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `ctrtDmndRcptOrd` | 계약요청 접수차수 | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `ctrtDmndUprc` | 계약요청 품목단가 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `devyCndtCd` | 인도조건 | 수집 | 사용자 확인 2026-09-27(일괄 답변 6 / R07) |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `devyCndtNm` | 인도조건 | 제외: 다른 출처 | 사용자 확인 2026-09-27(일괄 답변 6 / R07) — 같은 뜻 devyCndtCd 를 수집 |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `dlvgdsPlacNm` | 납품장소 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `dlvgdsTermDt` | 납품기한 날짜 | 수집 | 사용자 확인 2026-09-27(일괄 답변 5 / R06) |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `dlvgdsTermNody` | 납품일수 | 수집 | 사용자 확인 2026-09-27(일괄 답변 5 / R06) |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `dtlsPrnm` | 세부품명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `dtlsPrnmNo` | 세부품명번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `extlPbfnAsetDmndSqnoVal` | 외부 재정 자산요청 순번 후보 | 후보 | 빈 열만 확인 |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `inspTyCd` | 검사형태 | 후보 | ID·표 맥락 |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `itemCfnm` | 물품명 후보(정확한 용어 미확정) | 후보 | ID·표 맥락 |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `itemClsfNo` | 물품분류번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-contract-v1 의 답) |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `itemIdnfNm` | 물품식별명/규격 표시 후보 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `itemIdnfNo` | 물품식별번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `itrlPrcmAsetNoVal` | 내부 조달 자산번호 후보 | 후보 | 빈 열만 확인 |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `itrlPrcmItemIdnfNm` | 내부 조달 물품식별명 후보 | 후보 | 빈 열만 확인 |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `itrlPrcmItemIdnfNoVal` | 내부 조달 물품식별번호 후보 | 후보 | 빈 열만 확인 |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `ndfsItemDlvgdsMtruCdVal` | 국방 납품부대 코드 | 제외: 다른 출처 | 원문 명칭 ndfsPrcmItemDlvgdsMtruNm 을 수집(답변 3 명칭 우선) |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `ndfsItemSpecNo` | 규격번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `ndfsIvtrNo` | 재고번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `ndfsPrcmCtrtPlanYear` | 국방 조달계획연도 후보 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) 메모: '국방 조달계획연도' |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `ndfsPrcmDmndNo` | 국방 조달요구번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `ndfsPrcmItemDlvgdsMtruNm` | 국방 납품부대명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `ndfsPrcmPlanChgOrd` | 국방 조달계획 변경차수 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `ndfsPrcmPlanDlvgdsSqno` | 국방 조달계획 납품순번 | 후보 | ID·표 맥락 |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `ndpfExpndDmndNoVal` | 국방 지출요청번호 후보 | 후보 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `no` | 표시/출력/내부 순번(업무 키로 사용 금지) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `qtyUntNm` | 수량단위명 | 수집 | 사용자 확인 2026-09-27(일괄 답변 6 / R07) |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-request-v1` | `mf_wfm_container_gridView` | `rtrcnYn` | 취소 여부 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |

## `g2b-request-b-v1` — 조달요구 접수 상세(일반용역) (01112)

수집 48 · 제외: 다른 출처 17 · 제외: 뜻 없음 30 · 후보 2

| 프로필 | 출처 | ID | 뜻 | 상태 | 근거 |
|---|---|---|---|---|---|
| `g2b-request-b-v1` | `mf_wfm_container_gvDmndItem` | `devyCndtNm` | 인도조건 명칭 | 제외: 다른 출처 | 일반용역 코퍼스 2026-10-15 같은 행 코드·명칭 쌍 — 같은 뜻 devyCndtCd 를 수집(코드 사전) |
| `g2b-request-b-v1` | `mf_wfm_container_gvDmndItem` | `ndfsPrcmDmndNo` | 국방 조달요구번호 | 수집 | 사용자 확인 2026-10-15(같은 ID 원칙 — 일반용역에서는 늘 비어 있음) |
| `g2b-request-b-v1` | `mf_wfm_container_gvDmndItem` | `ndfsPrcmCtrtPlanYear` | 국방 조달계획연도 후보 | 수집 | 사용자 확인 2026-10-15(같은 ID 원칙 — 일반용역에서는 늘 비어 있음) |
| `g2b-request-b-v1` | `mf_wfm_container_gvDmndItem` | `ndfsPrcmPlanChgOrd` | 국방 조달계획 변경차수 | 수집 | 사용자 확인 2026-10-15(같은 ID 원칙 — 일반용역에서는 늘 비어 있음) |
| `g2b-request-b-v1` | `mf_wfm_container_gvDmndItem` | `dtlsPrnmNo` | 세부품명번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-b-v1` | `mf_wfm_container_gvDmndItem` | `itemIdnfNo` | 물품식별번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-b-v1` | `mf_wfm_container_gvDmndItem` | `dtlsPrnm` | 세부품명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-b-v1` | `mf_wfm_container_gvDmndItem` | `rtrcnYn` | 취소 여부 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-b-v1` | `mf_wfm_container_gvDmndItem` | `itemIdnfNm` | 물품식별명/규격 표시 후보 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-b-v1` | `mf_wfm_container_gvDmndItem` | `ctrtDmndUprc` | 계약요청 품목단가 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-b-v1` | `mf_wfm_container_gvDmndItem` | `ctrtDmndQty` | 계약요청 품목수량 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-b-v1` | `mf_wfm_container_gvDmndItem` | `ctrtDmndAmt` | 계약요청 품목금액 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-b-v1` | `mf_wfm_container_gvDmndItem` | `qtyUntNm` | 수량단위명 | 수집 | 사용자 확인 2026-09-27(일괄 답변 6 / R07) |
| `g2b-request-b-v1` | `mf_wfm_container_gvDmndItem` | `albmNopg` | 면수 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-b-v1` | `mf_wfm_container_gvDmndItem` | `devyCndtCd` | 인도조건 | 수집 | 사용자 확인 2026-09-27(일괄 답변 6 / R07) |
| `g2b-request-b-v1` | `mf_wfm_container_gvDmndItem` | `dlvgdsTermNody` | 납품일수 | 수집 | 사용자 확인 2026-09-27(일괄 답변 5 / R06) |
| `g2b-request-b-v1` | `mf_wfm_container_gvDmndItem` | `dlvgdsTermDt` | 납품기한 날짜 | 수집 | 사용자 확인 2026-09-27(일괄 답변 5 / R06) |
| `g2b-request-b-v1` | `mf_wfm_container_gvDmndItem` | `dlvgdsPlacNm` | 납품장소 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-b-v1` | `mf_wfm_container_gvDmndItem` | `ndfsIvtrNo` | 재고번호 | 수집 | 사용자 확인 2026-10-15(같은 ID 원칙 — 일반용역에서는 늘 비어 있음) |
| `g2b-request-b-v1` | `mf_wfm_container_gvDmndItem` | `ndfsItemSpecNo` | 규격번호 | 수집 | 사용자 확인 2026-10-15(같은 ID 원칙 — 일반용역에서는 늘 비어 있음) |
| `g2b-request-b-v1` | `mf_wfm_container_gvDmndItem` | `ndpfExpndDmndNoVal` | 국방 지출요청번호 후보 | 후보 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답 「모름」) |
| `g2b-request-b-v1` | `mf_wfm_container_gvDmndItem` | `ctrtDmndRcptNo` | 계약요청 접수번호 | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-request-b-v1` | `mf_wfm_container_gvDmndItem` | `ctrtDmndRcptOrd` | 계약요청 접수차수 | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-request-b-v1` | `mf_wfm_container_gvDmndItem` | `ctrtDmndRcptItemSqno` | 계약요청 접수 품목순번 | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-request-b-v1` | `pointInfo` | `devyCndtCd` | 인도조건 코드 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본 |
| `g2b-request-b-v1` | `pointInfo` | `devyCndtNm` | 인도조건 명칭 | 후보 | 일반용역 코퍼스 2026-10-15 같은 행 코드·명칭 쌍 · 접수 전체 값인지 선택 품목 값인지 미확인 |
| `g2b-request-b-v1` | `pointInfo` | `ctrtDmndBizNm` | 계약요청 사업명(요청명) | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-b-v1` | `pointInfo` | `wrtYmd` | 접수일 | 수집 | 사용자 확인 2026-09-27(질문 Q07 부분 확정) |
| `g2b-request-b-v1` | `pointInfo` | `prcmBsneSeNm` | 조달업무구분명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-b-v1` | `pointInfo` | `lcrtTyCd` | 계약법 구분(계040001 → 국가계약법, 계040002 → 지방계약법) | 수집 | 사용자 확인 2026-09-27(일괄 답변 10 / R11) |
| `g2b-request-b-v1` | `pointInfo` | `stdCtrtMthdCd` | 계약방법 | 수집 | 사용자 확인 2026-09-27(일괄 답변 10 / R11) |
| `g2b-request-b-v1` | `pointInfo` | `ctrtTyCd` | 계약구분(계120001 → 총액계약) | 수집 | 사용자 확인 2026-09-27(일괄 답변 10 / R11) |
| `g2b-request-b-v1` | `pointInfo` | `scsbdMthdCd` | 낙찰방법 | 수집 | 사용자 확인 2026-09-27(일괄 답변 10 / R11) |
| `g2b-request-b-v1` | `pointInfo` | `dmstAcntgNm` | 수요기관 회계구분 | 수집 | 원문 명칭 dmstAcntgNm(dmstAcntgCd 옆) |
| `g2b-request-b-v1` | `pointInfo` | `drpmSbpySeCd` | 지급방법(직130001 → 직불, 직130002 → 대지급) | 수집 | 사용자 확인 2026-09-27(일괄 답변 10 / R11) |
| `g2b-request-b-v1` | `pointInfo` | `totlPymtAmt` | 품대 | 수집 | 사용자 확인 2026-09-27(질문 Q01 일괄 승인) |
| `g2b-request-b-v1` | `pointInfo` | `antcPrcmFee` | 예상 조달수수료 | 수집 | 사용자 확인 2026-09-27(질문 Q01 일괄 승인) |
| `g2b-request-b-v1` | `pointInfo` | `bgtAmt` | 예산금액 | 수집 | 사용자 확인 2026-09-27(질문 Q01 일괄 승인) |
| `g2b-request-b-v1` | `pointInfo` | `fprdYn` | 외국산 허용 여부 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-b-v1` | `pointInfo` | `rlsYn` | 공개 여부 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-b-v1` | `pointInfo` | `advfPinfrmYn` | 선급금 선고지 여부 | 수집 | 사용자 확인 2026-09-27(일괄 답변 7 / R08) |
| `g2b-request-b-v1` | `pointInfo` | `advfPinfrmPsbltyRat` | 선급금 선고지 가능 비율(%) | 수집 | 사용자 확인 2026-09-27(질문 Q04 부분 확정) |
| `g2b-request-b-v1` | `pointInfo` | `dmstNm` | 수요기관명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-b-v1` | `pointInfo` | `dmstUntyGrpNo` | 수요기관 통합기관번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-b-v1` | `pointInfo` | `dmndInstBizPicNm` | 수요기관 업무담당자명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-b-v1` | `pointInfo` | `dmndInstBizPicTlphNo` | 수요기관 업무담당자 전화번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-b-v1` | `pointInfo` | `dmndInstPicFaxNo` | 수요기관 담당자 팩스번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-request-b-v1` | `pointInfo` | `ctrtDmndRcptNo` | 계약요청 접수번호 | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-request-b-v1` | `pointInfo` | `ctrtDmndRcptOrd` | 계약요청 접수차수 | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-request-b-v1` | `pointInfo` | `albmNopg` | 면수 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-request-b-v1` | `pointInfo` | `areaCd` | 화면/영역 코드(업무 의미 미확정) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · pointInfo 에서 확인 |
| `g2b-request-b-v1` | `pointInfo` | `ctrtDmndAmt` | 계약요청 품목금액 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-request-b-v1` | `pointInfo` | `ctrtDmndQty` | 계약요청 품목수량 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-request-b-v1` | `pointInfo` | `ctrtDmndUprc` | 계약요청 품목단가 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-request-b-v1` | `pointInfo` | `depth1` | 화면 경로 단계 후보 | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · pointInfo 에서 확인 |
| `g2b-request-b-v1` | `pointInfo` | `depth2` | 화면 경로 단계 후보 | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · pointInfo 에서 확인 |
| `g2b-request-b-v1` | `pointInfo` | `dlvgdsPlacNm` | 납품장소 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-request-b-v1` | `pointInfo` | `dlvgdsTermDt` | 납품기한 날짜 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-request-b-v1` | `pointInfo` | `dlvgdsTermNody` | 납품일수 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-request-b-v1` | `pointInfo` | `dtlsPrnm` | 세부품명 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-request-b-v1` | `pointInfo` | `dtlsPrnmNo` | 세부품명번호 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-request-b-v1` | `pointInfo` | `itemIdnfNm` | 물품식별명/규격 표시 후보 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-request-b-v1` | `pointInfo` | `pbancKndCd` | 공고종류 | 수집 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · pointInfo 에서 확정 |
| `g2b-request-b-v1` | `pointInfo` | `prspPrce` | 추정가격 | 수집 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · pointInfo 에서 확정 |
| `g2b-request-b-v1` | `pointInfo` | `rtrcnYn` | 취소 여부 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-request-b-v1` | `*_grdFile` | `CHK` | 행 선택 상태 | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-request-b-v1` | `*_grdFile` | `atchFileNm` | 서버 내부 파일경로/저장명/이미지 URL(수집 제외) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-request-b-v1` | `*_grdFile` | `atflPathNm` | 서버 내부 파일경로/저장명/이미지 URL(수집 제외) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-request-b-v1` | `*_grdFile` | `befRowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-request-b-v1` | `*_grdFile` | `chnrId` | 시스템 등록/변경/입력자(수집 제외) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-request-b-v1` | `*_grdFile` | `chnrIpar` | 등록/변경 IP 성격 값(수집 제외) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-request-b-v1` | `*_grdFile` | `colNm` | 내부 테이블/컬럼명 | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-request-b-v1` | `*_grdFile` | `imgUrl` | 서버 내부 파일경로/저장명/이미지 URL(수집 제외) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-request-b-v1` | `*_grdFile` | `kbrdrNm` | 시스템 등록/변경/입력자(수집 제외) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-request-b-v1` | `*_grdFile` | `nIndex` | 표시/출력/내부 순번(업무 키로 사용 금지) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-request-b-v1` | `*_grdFile` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-request-b-v1` | `*_grdFile` | `rstrId` | 시스템 등록/변경/입력자(수집 제외) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-request-b-v1` | `*_grdFile` | `rstrIpar` | 등록/변경 IP 성격 값(수집 제외) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-request-b-v1` | `*_grdFile` | `scrnIndtOdr` | 화면 표시순서 | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-request-b-v1` | `*_grdFile` | `sessUserNo` | 세션 사용자번호(수집 제외) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-request-b-v1` | `*_grdFile` | `tblNm` | 내부 테이블/컬럼명 | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-request-b-v1` | `mf_wfm_container_gvDmndItem` | `CHK` | 행 선택 상태 | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-request-b-v1` | `mf_wfm_container_gvDmndItem` | `itemClsfNo` | 물품분류번호 | 수집 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridView 에서 확정 |
| `g2b-request-b-v1` | `mf_wfm_container_gvDmndItem` | `no` | 표시/출력/내부 순번(업무 키로 사용 금지) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-request-b-v1` | `mf_wfm_container_gvDmndItem` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-request-b-v1` | `mf_wfm_container_wfBaseInfo_gvCtrtDmndDmstPic` | `CHK` | 행 선택 상태 | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-request-b-v1` | `mf_wfm_container_wfBaseInfo_gvCtrtDmndDmstPic` | `ctrtDmndRcptNo` | 계약요청 접수번호 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-request-b-v1` | `mf_wfm_container_wfBaseInfo_gvCtrtDmndDmstPic` | `ctrtDmndRcptOrd` | 계약요청 접수차수 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-request-b-v1` | `mf_wfm_container_wfBaseInfo_gvCtrtDmndDmstPic` | `no` | 표시/출력/내부 순번(업무 키로 사용 금지) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-request-b-v1` | `mf_wfm_container_wfBaseInfo_gvCtrtDmndDmstPic` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-request-b-v1` | `mf_wfm_container_wfBaseInfo_gvOderPlan` | `CHK` | 행 선택 상태 | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-request-b-v1` | `mf_wfm_container_wfBaseInfo_gvOderPlan` | `ctrtDmndRcptNo` | 계약요청 접수번호 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-request-b-v1` | `mf_wfm_container_wfBaseInfo_gvOderPlan` | `ctrtDmndRcptOrd` | 계약요청 접수차수 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-request-b-v1` | `mf_wfm_container_wfBaseInfo_gvOderPlan` | `no` | 표시/출력/내부 순번(업무 키로 사용 금지) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-request-b-v1` | `mf_wfm_container_wfBaseInfo_gvOderPlan` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-request-b-v1` | `mf_wfm_container_wfRfpDtl_grdPrpsDmndInfoView` | `kbrdrId` | 시스템 등록/변경/입력자(수집 제외) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_grdEtpsLst 에서 확인 |
| `g2b-request-b-v1` | `mf_wfm_container_wfRfpDtl_grdPrpsDmndInfoView` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |

## `g2b-notice-a-v1` — 입찰공고 상세 (01179)

수집 76 · 제외: 다른 출처 20 · 제외: 뜻 없음 31 · 후보 120

| 프로필 | 출처 | ID | 뜻 | 상태 | 근거 |
|---|---|---|---|---|---|
| `g2b-notice-a-v1` | `pointInfo` | `(빈 ID)` | 빈 ID(출처 복원 불가) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-a-v1` | `pointInfo` | `aagrSbmsnElctHdwrSeCd` | 서류 전자/수기 제출구분(서류 종류 확인) | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `alotBgtAmt` | 배정예산 | 수집 | 사용자 확인 2026-09-27(질문 Q01 일괄 승인) |
| `g2b-notice-a-v1` | `pointInfo` | `areaCd` | 화면/영역 코드(업무 의미 미확정) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-a-v1` | `pointInfo` | `atcsCd` | 법령 조항 후보 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `atmtCntt01` | 화면 도움말 HTML(사업 특기사항 아님) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-a-v1` | `pointInfo` | `atrzDt` | 결재일시 후보(게시일시와 구별) | 후보 | ID·표 맥락 · 옛 미사용(Q07) |
| `g2b-notice-a-v1` | `pointInfo` | `atrzSttsNm` | 결재상태명 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `bdngAmtRangLmtYn` | 투찰금액 범위제한 여부 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `bfSpecRcptYn` | 사전규격 관련 여부(접수/공개 여부 확인) | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `bfSpecRegNo` | 사전규격 등록번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-v1` | `pointInfo` | `bfssPicNm` | 사업 담당자명 후보 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `bidBlffNm` | 입찰 집행관명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-v1` | `pointInfo` | `bidClsfNo` | 입찰분류번호 | 제외: 다른 출처 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List 에서 확정 |
| `g2b-notice-a-v1` | `pointInfo` | `bidDepoPayTermDay` | 입찰보증금 납부기한일 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `bidDepoPayTermTime` | 입찰보증금 납부기한시각 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `bidMthdCd` | 입찰방식(입180002 → 전자입찰) | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-v1` | `pointInfo` | `bidPbancItemSqno` | 입찰공고 품목순번 | 제외: 다른 출처 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List 에서 확정 |
| `g2b-notice-a-v1` | `pointInfo` | `bidPbancNm` | 입찰공고명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-v1` | `pointInfo` | `bidPbancNo` | 입찰공고번호(본번호) | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-notice-a-v1` | `pointInfo` | `bidPbancOrd` | 입찰공고차수 | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-notice-a-v1` | `pointInfo` | `bidPrceEvlRt` | 입찰가격 평가비율 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `bidPrcpLmtYn` | 입찰참가 제한 여부 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `bidQlfcRegDay` | 입찰참가자격 등록 마감일 | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R13) |
| `g2b-notice-a-v1` | `pointInfo` | `bidQlfcRegTime` | 입찰참가자격 등록 마감시각 | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R13) |
| `g2b-notice-a-v1` | `pointInfo` | `bizAmt` | 사업금액(추정가격+부가세) | 수집 | 사용자 확인 2026-09-27(질문 Q01 일괄 승인) |
| `g2b-notice-a-v1` | `pointInfo` | `bizAmtFeeSumAmt` | 사업금액·수수료 합계 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-v1` | `pointInfo` | `blndEvlAplcnYn` | 블라인드 평가 적용 여부 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `bsneMixtCtrtYn` | 업무 혼합계약 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `clsfCrtrBidYn` | 분류별 입찰 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `coexCllbEvlAplcnYn` | 공동·협업 평가 적용 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `coexStlmTrgtYn` | 상생결제 대상 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `cornPbancRsn` | 재공고 사유 후보 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `ctrlPrcmExcpYn` | 중앙조달 예외 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `ctrtDmndRcptNo` | 계약요청 접수번호 | 제외: 다른 출처 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List 에서 확정 |
| `g2b-notice-a-v1` | `pointInfo` | `ctrtTyCd` | 계약구분(계120001 → 총액계약) | 수집 | 사용자 확인 2026-09-27(일괄 답변 10 / R11) |
| `g2b-notice-a-v1` | `pointInfo` | `depth1` | 화면 경로 단계 후보 | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-a-v1` | `pointInfo` | `depth2` | 화면 경로 단계 후보 | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-a-v1` | `pointInfo` | `depth3` | 화면 경로 단계 후보 | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-a-v1` | `pointInfo` | `devyCndtCd` | 인도조건 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본 |
| `g2b-notice-a-v1` | `pointInfo` | `dfrlScrAplcnYn` | 차등점수 적용 여부 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `dfrlScrItvlScr` | 차등점수 간격점수 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `dgtnBizYn` | 정보화사업 여부 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `dlvgdsPlacNm` | 납품장소 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본 |
| `g2b-notice-a-v1` | `pointInfo` | `dlvgdsTermCn` | 납품기한 내용 | 수집 | 사용자 확인 2026-09-27(일괄 답변 5 / R06) |
| `g2b-notice-a-v1` | `pointInfo` | `dlvgdsTermDt` | 납품기한 날짜 | 제외: 다른 출처 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List 에서 확정 |
| `g2b-notice-a-v1` | `pointInfo` | `dlvgdsTermNody` | 납품일수 | 제외: 다른 출처 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List 에서 확정 |
| `g2b-notice-a-v1` | `pointInfo` | `dmstUntyGrpNm` | 수요기관명 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-notice-a-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본 |
| `g2b-notice-a-v1` | `pointInfo` | `dmstcOvrsSeCd` | 국내/국제 입찰구분 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-v1` | `pointInfo` | `drawPlrlPnprTnum` | 추첨 복수예비가격 개수 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `dtfrBizYn` | 용어 미확정 — 약어만으로 번역하지 않음 | 후보 | 미해석 |
| `g2b-notice-a-v1` | `pointInfo` | `dtlsPrnmNm` | 세부품명 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-notice-a-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본 |
| `g2b-notice-a-v1` | `pointInfo` | `dtlsPrnmNo` | 세부품명번호 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본 |
| `g2b-notice-a-v1` | `pointInfo` | `egnrEvlTrgtYn` | 기술평가 대상 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `emrgPbancRsnCd` | 긴급공고 사유 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `emrgPbancYn` | 긴급공고 여부 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `esdacYn` | 용어 미확정 — 약어만으로 번역하지 않음 | 후보 | 미해석 |
| `g2b-notice-a-v1` | `pointInfo` | `evlDocSbmsnBgngDay` | 평가서류 제출 시작일 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `evlDocSbmsnBgngTime` | 평가서류 제출 시작시각 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `evlDocSbmsnDdlnDay` | 평가서류 제출 마감일 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `evlDocSbmsnDdlnTime` | 평가서류 제출 마감시각 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `evlDocSbmsnElctHdwrSeCd` | 서류 전자/수기 제출구분(서류 종류 확인) | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `evlMaagSeCd` | 평가 관리구분 후보 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `evlSysUtztnYn` | 평가시스템 이용 여부 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `fctrRspbMhct` | 하자담보책임기간(개월수) | 수집 | 사용자 확인 2026-09-27(질문 Q03 용어·단위 승인) |
| `g2b-notice-a-v1` | `pointInfo` | `fctrRspbNbyr` | 하자담보책임기간(연수) | 수집 | 사용자 확인 2026-09-27(질문 Q03 용어·단위 승인) |
| `g2b-notice-a-v1` | `pointInfo` | `fctrRspbPrdAplcnYn` | 하자담보책임기간 적용 여부 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `fildCnddYn` | 현장설명 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `fncrNm` | 재무관 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) · 사용자 결정 2026-10-16: 「채권자」 가 아니라 재무관으로 담는다(값은 사람 이름) |
| `g2b-notice-a-v1` | `pointInfo` | `fprdYn` | 외국산 허용 여부 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-v1` | `pointInfo` | `ftalPrcmCtrtEntFormSeCd` | 조달 계약체결 형태구분(약어 미확정) | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `grpNgtnCtrtBizClsfCd` | 단체 수의계약 업무분류 후보 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `igiInspInstSeCd` | 검수기관 구분 후보 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `infoLeakPrhbTrgtYn` | 정보누출금지 대상 여부 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `inspInstSeCd` | 검사기관 구분 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `itemClsfBdngLmtAtrbCd` | 물품분류 투찰제한 속성 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `itemClsfNm` | 물품분류명 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `itemClsfNo` | 물품분류번호 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-contract-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본(품목표 grdAliasDmTtl01List 의 한 행과 같은 값, 코퍼스 대조 2026-10-15) |
| `g2b-notice-a-v1` | `pointInfo` | `itemDtlSpecCn` | 품목 상세규격 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-notice-a-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본 |
| `g2b-notice-a-v1` | `pointInfo` | `itemIdnfNm` | 물품식별명/규격 표시 후보 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본(품목표 grdAliasDmTtl01List 의 한 행과 같은 값, 코퍼스 대조 2026-10-15) |
| `g2b-notice-a-v1` | `pointInfo` | `itemIdnfNo` | 물품식별번호 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본 |
| `g2b-notice-a-v1` | `pointInfo` | `itemIvtrNoVal` | 재고번호 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `jbizProdYn` | 중소기업제품 관련 여부(정확한 라벨 미확정) | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `lcnsLmtYn` | 업종·면허 제한 여부 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `lcrtTyCd` | 계약법 구분(계040001 → 국가계약법, 계040002 → 지방계약법) | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-v1` | `pointInfo` | `lotmCtnuCtrtYn` | 장기계속계약 여부 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `lrcoPtcpLmtExcpYn` | 대기업 참여제한 예외 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `mixtCtrtAffcClclnYn` | 혼합계약 사후정산 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `mnftrItemLmtYn` | 제조물품 제한 여부 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-v1` | `pointInfo` | `ndfsItemSpecNo` | 규격번호 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본(품목표 grdAliasDmTtl01List 의 한 행과 같은 값, 코퍼스 대조 2026-10-15) |
| `g2b-notice-a-v1` | `pointInfo` | `ndfsPrcmDmndNo` | 국방 조달요구번호 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본(품목표 grdAliasDmTtl01List 의 한 행과 같은 값, 코퍼스 대조 2026-10-15) |
| `g2b-notice-a-v1` | `pointInfo` | `ngtnCtrtBizClsfCd` | 수의계약 업무분류 후보 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `onbsPlacCd` | 개찰장소 코드 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `onbsPrnmntDay` | 개찰 예정일 | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R13) |
| `g2b-notice-a-v1` | `pointInfo` | `onbsPrnmntTime` | 개찰 예정시각 | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R13) |
| `g2b-notice-a-v1` | `pointInfo` | `osinspYn` | 외부검사 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `pbancChgRsn` | 공고 변경사유 | 수집 | 사용자 확인 2026-09-27(일괄 답변 3 / R03) |
| `g2b-notice-a-v1` | `pointInfo` | `pbancInstUntyGrpNm` | 공고기관명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-v1` | `pointInfo` | `pbancKndCd` | 공고종류 | 수집 | 사용자 확인 2026-09-27(일괄 답변 3 / R03) |
| `g2b-notice-a-v1` | `pointInfo` | `pbancPicNm` | 공고 담당자명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-v1` | `pointInfo` | `pbancSttsCd` | 공고상태 | 수집 | 사용자 확인 2026-09-27(일괄 답변 3 / R03) |
| `g2b-notice-a-v1` | `pointInfo` | `pmcsRtnmEvlTrgtYn` | 평가대상 여부(약어 미확정) | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `pmcsRtnmEvlcrtAmt` | 평가기준 금액(약어 미확정) | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `pnprDcsnMthoCd` | 예정가격 결정방법 | 수집 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-service-v1 · pointInfo 에서 확정 |
| `g2b-notice-a-v1` | `pointInfo` | `pnprPgmUseYn` | 예정가격 프로그램 사용 여부 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `pnprRwrtYn` | 예정가격 재작성 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `prchsDtlItemQty` | 구매대상 품목수량 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-notice-a-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본 |
| `g2b-notice-a-v1` | `pointInfo` | `prchsDtlItemUntVal` | 구매대상 품목 단위 | 제외: 다른 출처 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List 에서 확정 |
| `g2b-notice-a-v1` | `pointInfo` | `prchsDtlItemUprc` | 구매대상 품목 추정단가 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-notice-a-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본 |
| `g2b-notice-a-v1` | `pointInfo` | `presTlphNo` | 대표 전화번호 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `prfmncLmtYn` | 실적제한 여부 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-v1` | `pointInfo` | `prsntnTmmn` | 발표시간(분) 후보 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `prsntnYn` | 발표 여부 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `prspPrce` | 추정가격 | 수집 | 사용자 확인 2026-09-27(질문 Q01 일괄 승인) |
| `g2b-notice-a-v1` | `pointInfo` | `prttnDlvgdsPsbltyYn` | 분할납품 가능 여부 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-v1` | `pointInfo` | `qestRspnsTmmn` | 질의응답시간(분) 후보 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `rbidPrmsYn` | 재입찰 허용 여부 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-v1` | `pointInfo` | `rgnLmtYn` | 지역제한 여부/표시 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-notice-a-service-v1 의 답) |
| `g2b-notice-a-v1` | `pointInfo` | `rschDvlpItemYn` | 연구개발물품 여부 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-v1` | `pointInfo` | `samePrceBidAtmtDrawYn` | 동가 입찰 자동추첨 여부 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `sbmsnOdrDocNmLst` | 제출서류명 목록 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `sckrAplcnYn` | 용어 미확정 — 약어만으로 번역하지 않음 | 후보 | 미해석 |
| `g2b-notice-a-v1` | `pointInfo` | `scsbdLwlmRat` | 낙찰하한율 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-v1` | `pointInfo` | `scsbdMthdCd` | 낙찰방법 | 수집 | 사용자 확인 2026-09-27(일괄 답변 10 / R11) |
| `g2b-notice-a-v1` | `pointInfo` | `scsbdMthdDtlsCrtrNo` | 낙찰 심사/세부기준 번호 후보 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `scsbdSrngCrtrNo` | 낙찰 심사/세부기준 번호 후보 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `scsbdSrngDtlsCrtrSqno` | 낙찰 심사 세부기준 순번 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `skdwCndtCtrtYn` | 조건부 계약 여부(조건 약어 미확정) | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `slCprcmBsneSeCd` | 조달업무구분 후보 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `slprRcptBgngDay` | 입찰서 접수 시작일 | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R13) |
| `g2b-notice-a-v1` | `pointInfo` | `slprRcptBgngTime` | 입찰서 접수 시작시각 | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R13) |
| `g2b-notice-a-v1` | `pointInfo` | `slprRcptDdlnDay` | 입찰서 접수 마감일 | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R13) |
| `g2b-notice-a-v1` | `pointInfo` | `slprRcptDdlnTime` | 입찰서 접수 마감시각 | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R13) |
| `g2b-notice-a-v1` | `pointInfo` | `smrlCmptItemYn` | 중소기업간 경쟁제품 여부 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `specRlsDt` | 사전규격 공개일시 후보 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `stdCtrtMthdCd` | 계약방법 | 수집 | 사용자 확인 2026-09-27(일괄 답변 10 / R11) |
| `g2b-notice-a-v1` | `pointInfo` | `stdPbadmsReqNody` | 표준 행정 소요일수 후보 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `techAbltEvlRt` | 기술능력 평가비율 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `totlPnprTnum` | 총 예비가격 개수 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `tpSrngTrgtYn` | 심사대상 여부(약어 미확정) | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `vatAmt` | 부가가치세액 | 수집 | 사용자 확인 2026-09-27(질문 Q01 일괄 승인) |
| `g2b-notice-a-v1` | `pointInfo` | `wtoBidRsnCd` | 국제입찰 사유 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `pointInfo` | `pbancPstgDt` | 공고 게시일시 | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R13) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs1_body_wframe6_grdAliasDmTtl06List` | `CHK` | 행 선택 상태 | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs1_body_wframe6_grdAliasDmTtl06List` | `deptNm` | 부서명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs1_body_wframe6_grdAliasDmTtl06List` | `dmstPicId` | 수요기관 담당자 ID/명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs1_body_wframe6_grdAliasDmTtl06List` | `dmstUntyGrpNm` | 수요기관명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs1_body_wframe6_grdAliasDmTtl06List` | `dmstUntyGrpNo` | 수요기관 통합기관번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs1_body_wframe6_grdAliasDmTtl06List` | `eml` | 이메일 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs1_body_wframe6_grdAliasDmTtl06List` | `evlPicYn` | 평가담당자 여부 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs1_body_wframe6_grdAliasDmTtl06List` | `faxNo` | 팩스번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs1_body_wframe6_grdAliasDmTtl06List` | `no` | 표시/출력/내부 순번(업무 키로 사용 금지) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs1_body_wframe6_grdAliasDmTtl06List` | `picNm` | 담당자명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs1_body_wframe6_grdAliasDmTtl06List` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs1_body_wframe6_grdAliasDmTtl06List` | `tlphNo` | 전화번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs2_body_wframe7_grdAliasDmTtl07LeftList` | `CHK` | 행 선택 상태 | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs2_body_wframe7_grdAliasDmTtl07LeftList` | `bidLmtSeCd` | 입찰제한 구분 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs2_body_wframe7_grdAliasDmTtl07LeftList` | `bidLmtUntyCd` | 입찰참가 제한 업종코드 | 제외: 다른 출처 | 원문 명칭 bidLmtUntyNm 을 수집(답변 3 명칭 우선) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs2_body_wframe7_grdAliasDmTtl07LeftList` | `bidLmtUntyNm` | 입찰참가 제한 업종명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs2_body_wframe7_grdAliasDmTtl07LeftList` | `cnscAbltEvlAmt` | 시공능력 평가액 후보 | 후보 | 빈 열만 확인 |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs2_body_wframe7_grdAliasDmTtl07LeftList` | `lmtGupSqno` | 제한그룹 순번/명 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs2_body_wframe7_grdAliasDmTtl07LeftList` | `lmtGupSqnoNm` | 제한그룹 순번/명 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs2_body_wframe7_grdAliasDmTtl07LeftList` | `lmtKndCd` | 제한종류 코드 | 후보 | 빈 열만 확인 |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs2_body_wframe7_grdAliasDmTtl07LeftList` | `lmtSqno` | 제한 순번 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs2_body_wframe7_grdAliasDmTtl07LeftList` | `no` | 표시/출력/내부 순번(업무 키로 사용 금지) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs2_body_wframe7_grdAliasDmTtl07LeftList` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-a-v1` | `*_grdFile` | `CHK` | 행 선택 상태 | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-a-v1` | `*_grdFile` | `atchFileDscr` | 첨부파일 설명 | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R14) |
| `g2b-notice-a-v1` | `*_grdFile` | `atchFileKndCd` | 첨부파일 종류코드(문서유형명은 미확정) | 후보 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-v1` | `*_grdFile` | `atchFileNm` | 서버 내부 파일경로/저장명/이미지 URL(수집 제외) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-a-v1` | `*_grdFile` | `atchFileSqno` | 첨부파일순번 | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R14) |
| `g2b-notice-a-v1` | `*_grdFile` | `atflPathNm` | 서버 내부 파일경로/저장명/이미지 URL(수집 제외) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-a-v1` | `*_grdFile` | `atflRmrk` | 첨부파일 비고 | 후보 | 빈 열만 확인 |
| `g2b-notice-a-v1` | `*_grdFile` | `befRowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-a-v1` | `*_grdFile` | `bsneClsfCd` | 내부 업무분류코드 | 후보 | 구조·이름 분류 |
| `g2b-notice-a-v1` | `*_grdFile` | `chngDttm` | 등록/변경/입력일시(업무일자와 구별) | 후보 | 빈 열만 확인 |
| `g2b-notice-a-v1` | `*_grdFile` | `chnrId` | 시스템 등록/변경/입력자(수집 제외) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-a-v1` | `*_grdFile` | `chnrIpar` | 등록/변경 IP 성격 값(수집 제외) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-a-v1` | `*_grdFile` | `colNm` | 내부 테이블/컬럼명 | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-a-v1` | `*_grdFile` | `delYn` | 삭제 여부 | 후보 | 빈 열만 확인 |
| `g2b-notice-a-v1` | `*_grdFile` | `dwnldPrmsYn` | 다운로드 허용 여부 | 후보 | 빈 열만 확인 |
| `g2b-notice-a-v1` | `*_grdFile` | `fileExtnNm` | 파일 확장자 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `*_grdFile` | `fileSz` | 파일 크기(단위 확인) | 후보 | ID·표 맥락 · R14 「파일 크기 후보」, 단위 미확정 |
| `g2b-notice-a-v1` | `*_grdFile` | `imgUrl` | 서버 내부 파일경로/저장명/이미지 URL(수집 제외) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-a-v1` | `*_grdFile` | `inptDt` | 등록/변경/입력일시(업무일자와 구별) | 후보 | 구조·이름 분류 |
| `g2b-notice-a-v1` | `*_grdFile` | `kbrdrNm` | 시스템 등록/변경/입력자(수집 제외) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-a-v1` | `*_grdFile` | `nIndex` | 표시/출력/내부 순번(업무 키로 사용 금지) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-a-v1` | `*_grdFile` | `orgnlAtchFileNm` | 원본 첨부파일명 | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R14) |
| `g2b-notice-a-v1` | `*_grdFile` | `rgstDttm` | 등록/변경/입력일시(업무일자와 구별) | 후보 | 빈 열만 확인 |
| `g2b-notice-a-v1` | `*_grdFile` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-a-v1` | `*_grdFile` | `rstrId` | 시스템 등록/변경/입력자(수집 제외) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-a-v1` | `*_grdFile` | `rstrIpar` | 등록/변경 IP 성격 값(수집 제외) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-a-v1` | `*_grdFile` | `scrnIndtOdr` | 화면 표시순서 | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-a-v1` | `*_grdFile` | `sessUserNo` | 세션 사용자번호(수집 제외) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-a-v1` | `*_grdFile` | `tblNm` | 내부 테이블/컬럼명 | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-a-v1` | `*_grdFile` | `untyAtchFileNo` | 통합 첨부파일번호(원문 참조) | 후보 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `CHK` | 행 선택 상태 | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `bidClsfNo` | 입찰분류번호 | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `bidPbancItemSqno` | 입찰공고 품목순번 | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `ctrtDmndRcptItemSqno` | 계약요청 접수 품목순번 | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `ctrtDmndRcptNo` | 계약요청 접수번호 | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `ctrtDmndRcptOrd` | 계약요청 접수차수 | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `devyCndtCd` | 인도조건 | 수집 | 사용자 확인 2026-09-27(일괄 답변 6 / R07) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `devyCndtCdExcel` | 인도조건 · Excel용 보조값 | 후보 | 빈 열만 확인 |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `dlvgdsPlacNm` | 납품장소 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `dlvgdsTermDt` | 납품기한 날짜 | 수집 | 사용자 확인 2026-09-27(일괄 답변 5 / R06) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `dlvgdsTermDtExcel` | 납품기한 날짜 · Excel용 보조값 | 후보 | 빈 열만 확인 |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `dlvgdsTermNody` | 납품일수 | 수집 | 사용자 확인 2026-09-27(일괄 답변 5 / R06) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `dlvgdsTermNodyExcel` | 납품일수 · Excel용 보조값 | 후보 | 빈 열만 확인 |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `dmstUntyGrpNm` | 수요기관명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `dmstUntyGrpNo` | 수요기관 통합기관번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `dtlsPrnmNm` | 세부품명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `dtlsPrnmNo` | 세부품명번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `inspTyCd` | 검사형태 | 후보 | 빈 열만 확인 |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `inspTyCdExcel` | 검사형태 · Excel용 보조값 | 후보 | 빈 열만 확인 |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `iritemClsfNm` | 기관별 물품분류번호/명 후보 | 후보 | 빈 열만 확인 |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `iritemClsfNo` | 기관별 물품분류번호/명 후보 | 후보 | 빈 열만 확인 |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `iritemClsfNoUseYn` | 기관별 물품분류번호 사용 여부 후보 | 후보 | 빈 열만 확인 |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `iritemClsfStepCdVal` | 기관별 물품분류 단계코드 후보 | 후보 | 빈 열만 확인 |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `itemCertTyCdLst` | 품목인증 유형코드 목록 | 후보 | 빈 열만 확인 |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `itemCertTyCdLstExcel` | 품목인증 유형코드 목록 · Excel용 보조값 | 후보 | 빈 열만 확인 |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `itemClsfNm` | 물품분류명 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `itemClsfNo` | 물품분류번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-contract-v1 의 답) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `itemDtlSpecCn` | 품목 상세규격 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `itemIdnfNm` | 물품식별명/규격 표시 후보 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `itemIdnfNo` | 물품식별번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `itemIvtrNoVal` | 재고번호 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `itnistUntyGrpNo` | 내부기관 통합기관번호 후보 | 후보 | 빈 열만 확인 |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `itrlPrcmItemIdnfNm` | 내부 조달 물품식별명 후보 | 후보 | 빈 열만 확인 |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `itrlPrcmItemIdnfNoVal` | 내부 조달 물품식별번호 후보 | 후보 | 빈 열만 확인 |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `ndfsItemSpecNo` | 규격번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `ndfsPrcmDmndNo` | 국방 조달요구번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `no` | 표시/출력/내부 순번(업무 키로 사용 금지) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `prchsDtlItemQty` | 구매대상 품목수량 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `prchsDtlItemQtyExcel` | 구매대상 품목수량 · Excel용 보조값 | 후보 | 빈 열만 확인 |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `prchsDtlItemUntVal` | 구매대상 품목 단위 | 수집 | 사용자 확인 2026-09-27(일괄 답변 6 / R07) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `prchsDtlItemUntValExcel` | 구매대상 품목 단위 · Excel용 보조값 | 후보 | 빈 열만 확인 |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `prchsDtlItemUprc` | 구매대상 품목 추정단가 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `prchsDtlItemUprcExcel` | 구매대상 품목 추정단가 · Excel용 보조값 | 후보 | 빈 열만 확인 |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `prodCertAsctCd` | 제품인증 관련코드 후보 | 후보 | 빈 열만 확인 |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `rowAmtSum` | 행 금액 합계 후보 | 후보 | ID·표 맥락 |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `rsrvDtlsPrnmNo` | 예약 세부품명번호 후보 | 후보 | 빈 열만 확인 |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `specAtrbNm` | 규격 속성명/내용 | 후보 | 빈 열만 확인 |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `specKndCd` | 규격종류 | 후보 | 빈 열만 확인 |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `specKndCdExcel` | 규격종류 · Excel용 보조값 | 후보 | 빈 열만 확인 |
| `g2b-notice-a-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `specNoVal` | 규격번호 | 후보 | 빈 열만 확인 |

## `g2b-notice-a-service-v1` — 입찰공고 상세(일반용역) (01181)

수집 78 · 제외: 다른 출처 79 · 제외: 뜻 없음 41 · 후보 7

| 프로필 | 출처 | ID | 뜻 | 상태 | 근거 |
|---|---|---|---|---|---|
| `g2b-notice-a-service-v1` | `pointInfo` | `scsbdMthdNm` | 낙찰방법 명칭 | 제외: 다른 출처 | 일반용역 코퍼스 2026-10-15 같은 행 코드·명칭 쌍 — 같은 뜻 scsbdMthdCd 를 수집(코드 사전) |
| `g2b-notice-a-service-v1` | `pointInfo` | `dmstcOvrsSeNm` | 국내/국제 입찰구분 명칭 | 제외: 다른 출처 | 일반용역 코퍼스 2026-10-15 같은 행 코드·명칭 쌍 — 같은 뜻 dmstcOvrsSeCd 를 수집(코드 사전) |
| `g2b-notice-a-service-v1` | `pointInfo` | `pnprDcsnMthoNm` | 예정가격 결정방법 명칭 | 수집 | 원문 명칭 pnprDcsnMthoNm(pnprDcsnMthoCd 옆) — 코퍼스의 모든 문서에서 코드 사전의 명칭과 같다(일반용역 코퍼스 2026-10-15, 답변 3 명칭 우선) |
| `g2b-notice-a-service-v1` | `pointInfo` | `bidPbancNm` | 입찰공고명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-service-v1` | `pointInfo` | `pbancKndCd` | 공고종류 | 수집 | 사용자 확인 2026-09-27(일괄 답변 3 / R03) |
| `g2b-notice-a-service-v1` | `pointInfo` | `pbancPstgDt` | 공고 게시일시 | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R13) |
| `g2b-notice-a-service-v1` | `pointInfo` | `scsbdMthdCd` | 낙찰방법 | 수집 | 사용자 확인 2026-09-27(일괄 답변 10 / R11) |
| `g2b-notice-a-service-v1` | `pointInfo` | `scsbdLwlmRat` | 낙찰하한율 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-service-v1` | `pointInfo` | `stdCtrtMthdCd` | 계약방법 | 제외: 다른 출처 | 원문 명칭 stdCtrtMthdNm 을 수집 — 코퍼스의 모든 문서에서 코드(사전)의 명칭과 같다(코퍼스 대조 2026-10-15, 답변 3 명칭 우선) |
| `g2b-notice-a-service-v1` | `pointInfo` | `ctrtTyCd` | 계약구분(계120001 → 총액계약) | 제외: 다른 출처 | 원문 명칭 ctrtTyNm 을 수집 — 코퍼스의 모든 문서에서 코드(사전)의 명칭과 같다(코퍼스 대조 2026-10-15, 답변 3 명칭 우선) |
| `g2b-notice-a-service-v1` | `pointInfo` | `dmstcOvrsSeCd` | 국내/국제 입찰구분 | 수집 | 원문 명칭 dmstcOvrsSeNm(일반용역 코퍼스 2026-10-15) |
| `g2b-notice-a-service-v1` | `pointInfo` | `fncrNm` | 재무관 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) · 사용자 결정 2026-10-16: 「채권자」 가 아니라 재무관으로 담는다(값은 사람 이름) |
| `g2b-notice-a-service-v1` | `pointInfo` | `rbidPrmsYn` | 재입찰 허용 여부 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-service-v1` | `pointInfo` | `prttnDlvgdsPsbltyYn` | 분할납품 가능 여부 | 제외: 다른 출처 | 원문 명칭 prttnDlvgdsPsbltyYnNm 을 수집 — 코퍼스의 모든 문서에서 코드(사전)의 명칭과 같다(코퍼스 대조 2026-10-15, 답변 3 명칭 우선) |
| `g2b-notice-a-service-v1` | `pointInfo` | `fctrRspbMhct` | 하자담보책임기간(개월수) | 수집 | 사용자 확인 2026-09-27(질문 Q03 용어·단위 승인) |
| `g2b-notice-a-service-v1` | `pointInfo` | `pnprDcsnMthoCd` | 예정가격 결정방법 | 제외: 다른 출처 | 원문 명칭 pnprDcsnMthoNm 을 수집(일반용역 코퍼스 2026-10-15, 답변 3 명칭 우선) |
| `g2b-notice-a-service-v1` | `pointInfo` | `bizAmt` | 사업금액(추정가격+부가세) | 수집 | 사용자 확인 2026-09-27(질문 Q01 일괄 승인) |
| `g2b-notice-a-service-v1` | `pointInfo` | `alotBgtAmt` | 배정예산 | 수집 | 사용자 확인 2026-09-27(질문 Q01 일괄 승인) |
| `g2b-notice-a-service-v1` | `pointInfo` | `prspPrce` | 추정가격 | 수집 | 사용자 확인 2026-09-27(질문 Q01 일괄 승인) |
| `g2b-notice-a-service-v1` | `pointInfo` | `vatAmt` | 부가가치세액 | 수집 | 사용자 확인 2026-09-27(질문 Q01 일괄 승인) |
| `g2b-notice-a-service-v1` | `pointInfo` | `pbancInstUntyGrpNm` | 공고기관명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-service-v1` | `pointInfo` | `pbancPicNm` | 공고 담당자명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-service-v1` | `pointInfo` | `bidBlffNm` | 입찰 집행관명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-service-v1` | `pointInfo` | `bfSpecRegNo` | 사전규격 등록번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-service-v1` | `pointInfo` | `rgnLmtYn` | 지역제한 여부/표시 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-service-v1` | `pointInfo` | `prfmncLmtYn` | 실적제한 여부 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-service-v1` | `pointInfo` | `mnftrItemLmtYn` | 제조물품 제한 여부 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-service-v1` | `pointInfo` | `fprdYn` | 외국산 허용 여부 | 제외: 다른 출처 | 원문 명칭 fprdYnNm 을 수집 — 코퍼스의 모든 문서에서 코드(사전)의 명칭과 같다(코퍼스 대조 2026-10-15, 답변 3 명칭 우선) |
| `g2b-notice-a-service-v1` | `pointInfo` | `rschDvlpItemYn` | 연구개발물품 여부 | 제외: 다른 출처 | 원문 명칭 rschDvlpItemYnNm 을 수집 — 일반용역 코퍼스에는 이 코드 칸이 없고 명칭 칸만 있다(코퍼스 대조 2026-10-15, 답변 3 명칭 우선) |
| `g2b-notice-a-service-v1` | `pointInfo` | `pbancSttsCd` | 공고상태 | 수집 | 사용자 확인 2026-09-27(일괄 답변 3 / R03) |
| `g2b-notice-a-service-v1` | `pointInfo` | `dlvgdsTermCn` | 납품기한 내용 | 수집 | 사용자 확인 2026-09-27(일괄 답변 5 / R06) |
| `g2b-notice-a-service-v1` | `pointInfo` | `fctrRspbNbyr` | 하자담보책임기간(연수) | 수집 | 사용자 확인 2026-09-27(질문 Q03 용어·단위 승인) |
| `g2b-notice-a-service-v1` | `pointInfo` | `slprRcptBgngDay` | 입찰서 접수 시작일 | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R13) |
| `g2b-notice-a-service-v1` | `pointInfo` | `slprRcptBgngTime` | 입찰서 접수 시작시각 | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R13) |
| `g2b-notice-a-service-v1` | `pointInfo` | `slprRcptDdlnDay` | 입찰서 접수 마감일 | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R13) |
| `g2b-notice-a-service-v1` | `pointInfo` | `slprRcptDdlnTime` | 입찰서 접수 마감시각 | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R13) |
| `g2b-notice-a-service-v1` | `pointInfo` | `onbsPrnmntDay` | 개찰 예정일 | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R13) |
| `g2b-notice-a-service-v1` | `pointInfo` | `onbsPrnmntTime` | 개찰 예정시각 | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R13) |
| `g2b-notice-a-service-v1` | `pointInfo` | `bidQlfcRegDay` | 입찰참가자격 등록 마감일 | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R13) |
| `g2b-notice-a-service-v1` | `pointInfo` | `bidQlfcRegTime` | 입찰참가자격 등록 마감시각 | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R13) |
| `g2b-notice-a-service-v1` | `pointInfo` | `pbancChgRsn` | 공고 변경사유 | 수집 | 사용자 확인 2026-09-27(일괄 답변 3 / R03) |
| `g2b-notice-a-service-v1` | `pointInfo` | `bidPbancNo` | 입찰공고번호(본번호) | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-notice-a-service-v1` | `pointInfo` | `bidPbancOrd` | 입찰공고차수 | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `dmstUntyGrpNm` | 수요기관명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `dtlsPrnmNm` | 세부품명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `dtlsPrnmNo` | 세부품명번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `itemIdnfNo` | 물품식별번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `itemDtlSpecCn` | 품목 상세규격 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `prchsDtlItemQty` | 구매대상 품목수량 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `prchsDtlItemUntVal` | 구매대상 품목 단위 | 수집 | 사용자 확인 2026-09-27(일괄 답변 6 / R07) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `prchsDtlItemUprc` | 구매대상 품목 추정단가 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `dlvgdsTermNody` | 납품일수 | 수집 | 사용자 확인 2026-09-27(일괄 답변 5 / R06) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `dlvgdsTermDt` | 납품기한 날짜 | 수집 | 사용자 확인 2026-09-27(일괄 답변 5 / R06) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `dlvgdsPlacNm` | 납품장소 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `devyCndtCd` | 인도조건 | 수집 | 사용자 확인 2026-09-27(일괄 답변 6 / R07) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `ctrtDmndRcptNo` | 계약요청 접수번호 | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `ctrtDmndRcptOrd` | 계약요청 접수차수 | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `ctrtDmndRcptItemSqno` | 계약요청 접수 품목순번 | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `bidClsfNo` | 입찰분류번호 | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `bidPbancItemSqno` | 입찰공고 품목순번 | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs1_body_wframe6_grdAliasDmTtl06List` | `dmstUntyGrpNm` | 수요기관명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs1_body_wframe6_grdAliasDmTtl06List` | `deptNm` | 부서명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs1_body_wframe6_grdAliasDmTtl06List` | `picNm` | 담당자명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs1_body_wframe6_grdAliasDmTtl06List` | `faxNo` | 팩스번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs1_body_wframe6_grdAliasDmTtl06List` | `tlphNo` | 전화번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs1_body_wframe6_grdAliasDmTtl06List` | `dmstUntyGrpNo` | 수요기관 통합기관번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs1_body_wframe6_grdAliasDmTtl06List` | `dmstPicId` | 수요기관 담당자 ID/명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-a-service-v1` | `*_grdFile` | `atchFileDscr` | 첨부파일 설명 | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R14) |
| `g2b-notice-a-service-v1` | `*_grdFile` | `orgnlAtchFileNm` | 원본 첨부파일명 | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R14) |
| `g2b-notice-a-service-v1` | `*_grdFile` | `atchFileSqno` | 첨부파일순번 | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R14) |
| `g2b-notice-a-service-v1` | `pointInfo` | `areaCd` | 화면/영역 코드(업무 의미 미확정) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · pointInfo 에서 확인 |
| `g2b-notice-a-service-v1` | `pointInfo` | `atmtCntt01` | 화면 도움말 HTML(사업 특기사항 아님) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · pointInfo 에서 확인 |
| `g2b-notice-a-service-v1` | `pointInfo` | `bidClsfNo` | 입찰분류번호 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `pointInfo` | `bidMthdCd` | 입찰방식(입180002 → 전자입찰) | 제외: 다른 출처 | 원문 명칭 bidMthdNm 을 수집 — 코퍼스의 모든 문서에서 코드 사전의 명칭과 같다(일반용역 코퍼스 2026-10-15, 답변 3 명칭 우선) |
| `g2b-notice-a-service-v1` | `pointInfo` | `bidMthdNm` | 입찰방식 명칭 | 수집 | 같은 ID 원칙(2026-10-15) — g2b-notice-b-v1 · pointInfo 에서 확정 |
| `g2b-notice-a-service-v1` | `pointInfo` | `bidPbancItemSqno` | 입찰공고 품목순번 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `pointInfo` | `bidPbancText` | 입찰공고번호·차수 결합 표시 | 제외: 다른 출처 | 입찰공고번호·차수(bidPbancNo·bidPbancOrd)를 이어 적은 사본(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `pointInfo` | `bizAmtFeeSumAmt` | 사업금액·수수료 합계 | 수집 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · pointInfo 에서 확정 |
| `g2b-notice-a-service-v1` | `pointInfo` | `ctrtDmndRcptNo` | 계약요청 접수번호 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `pointInfo` | `ctrtTyNm` | 계약구분 명칭 | 수집 | 같은 ID 원칙(2026-10-15) — g2b-notice-b-v1 · pointInfo 에서 확정 |
| `g2b-notice-a-service-v1` | `pointInfo` | `depth1` | 화면 경로 단계 후보 | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · pointInfo 에서 확인 |
| `g2b-notice-a-service-v1` | `pointInfo` | `depth2` | 화면 경로 단계 후보 | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · pointInfo 에서 확인 |
| `g2b-notice-a-service-v1` | `pointInfo` | `depth3` | 화면 경로 단계 후보 | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · pointInfo 에서 확인 |
| `g2b-notice-a-service-v1` | `pointInfo` | `devyCndtCd` | 인도조건 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `pointInfo` | `dlvgdsPlacNm` | 납품장소 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `pointInfo` | `dlvgdsTermDt` | 납품기한 날짜 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `pointInfo` | `dlvgdsTermNody` | 납품일수 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `pointInfo` | `dmstUntyGrpNm` | 수요기관명 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `pointInfo` | `dtlsPrnmNm` | 세부품명 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `pointInfo` | `dtlsPrnmNo` | 세부품명번호 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `pointInfo` | `fprdYnNm` | 외국산 허용 여부 | 수집 | 같은 ID 원칙(2026-10-15) — g2b-notice-b-v1 · pointInfo 에서 확정 |
| `g2b-notice-a-service-v1` | `pointInfo` | `itemClsfNo` | 물품분류번호 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `pointInfo` | `itemDtlSpecCn` | 품목 상세규격 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `pointInfo` | `lcrtTyCd` | 계약법 구분(계040001 → 국가계약법, 계040002 → 지방계약법) | 수집 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · pointInfo 에서 확정 |
| `g2b-notice-a-service-v1` | `pointInfo` | `mnftrItemLmtYnNm` | 제조물품 제한 여부 | 제외: 다른 출처 | 같은 뜻의 코드 칸 mnftrItemLmtYn 를 수집 — 명칭이 지금 담는 값과 글자가 달라 바꾸지 않았다(명칭 전환 후보, 일반용역 코퍼스 2026-10-15) |
| `g2b-notice-a-service-v1` | `pointInfo` | `pbancKndNm` | 공고종류 | 제외: 다른 출처 | 같은 뜻의 코드 칸 pbancKndCd 를 수집 — 명칭이 지금 담는 값과 글자가 달라 바꾸지 않았다(명칭 전환 후보, 일반용역 코퍼스 2026-10-15) |
| `g2b-notice-a-service-v1` | `pointInfo` | `prchsDtlItemQty` | 구매대상 품목수량 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `pointInfo` | `prchsDtlItemUntVal` | 구매대상 품목 단위 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `pointInfo` | `prchsDtlItemUprc` | 구매대상 품목 추정단가 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `pointInfo` | `prttnDlvgdsPsbltyYnNm` | 분할납품 가능 여부 | 수집 | 같은 ID 원칙(2026-10-15) — g2b-notice-b-v1 · pointInfo 에서 확정 |
| `g2b-notice-a-service-v1` | `pointInfo` | `rgnLmtYnNm` | 지역제한 여부/표시 | 제외: 다른 출처 | 같은 뜻의 코드 칸 rgnLmtYn 를 수집 — 명칭이 지금 담는 값과 글자가 달라 바꾸지 않았다(명칭 전환 후보, 일반용역 코퍼스 2026-10-15) |
| `g2b-notice-a-service-v1` | `pointInfo` | `rschDvlpItemYnNm` | 연구개발물품 여부 | 수집 | 같은 ID 원칙(2026-10-15) — g2b-notice-b-v1 · pointInfo 에서 확정 |
| `g2b-notice-a-service-v1` | `pointInfo` | `scsbdMthdDtlsNm` | 낙찰방법 세부기준 표시 | 수집 | 같은 ID 원칙(2026-10-15) — g2b-notice-b-v1 · pointInfo 에서 확정 |
| `g2b-notice-a-service-v1` | `pointInfo` | `stdCtrtMthdNm` | 계약방법 | 수집 | 같은 ID 원칙(2026-10-15) — g2b-notice-b-v1 · pointInfo 에서 확정 |
| `g2b-notice-a-service-v1` | `*_grdFile` | `CHK` | 행 선택 상태 | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-notice-a-service-v1` | `*_grdFile` | `atchFileNm` | 서버 내부 파일경로/저장명/이미지 URL(수집 제외) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-notice-a-service-v1` | `*_grdFile` | `atflPathNm` | 서버 내부 파일경로/저장명/이미지 URL(수집 제외) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-notice-a-service-v1` | `*_grdFile` | `befRowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-notice-a-service-v1` | `*_grdFile` | `chnrId` | 시스템 등록/변경/입력자(수집 제외) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-notice-a-service-v1` | `*_grdFile` | `chnrIpar` | 등록/변경 IP 성격 값(수집 제외) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-notice-a-service-v1` | `*_grdFile` | `colNm` | 내부 테이블/컬럼명 | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-notice-a-service-v1` | `*_grdFile` | `imgUrl` | 서버 내부 파일경로/저장명/이미지 URL(수집 제외) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-notice-a-service-v1` | `*_grdFile` | `kbrdrNm` | 시스템 등록/변경/입력자(수집 제외) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-notice-a-service-v1` | `*_grdFile` | `nIndex` | 표시/출력/내부 순번(업무 키로 사용 금지) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-notice-a-service-v1` | `*_grdFile` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-notice-a-service-v1` | `*_grdFile` | `rstrId` | 시스템 등록/변경/입력자(수집 제외) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-notice-a-service-v1` | `*_grdFile` | `rstrIpar` | 등록/변경 IP 성격 값(수집 제외) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-notice-a-service-v1` | `*_grdFile` | `scrnIndtOdr` | 화면 표시순서 | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-notice-a-service-v1` | `*_grdFile` | `sessUserNo` | 세션 사용자번호(수집 제외) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-notice-a-service-v1` | `*_grdFile` | `tblNm` | 내부 테이블/컬럼명 | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_grdLcnsLmt` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_grdPrpsDmndInfoView` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridDmstPic` | `deptNm` | 부서명 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridDmstPic` | `dmstPicId` | 수요기관 담당자 ID/명 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridDmstPic` | `dmstUntyGrpNm` | 수요기관명 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridDmstPic` | `faxNo` | 팩스번호 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridDmstPic` | `picNm` | 담당자명 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridDmstPic` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridDmstPic` | `tlphNo` | 전화번호 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridDmstPicEvl` | `deptNm` | 부서명 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridDmstPicEvl` | `dmstPicId` | 수요기관 담당자 ID/명 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridDmstPicEvl` | `dmstUntyGrpNm` | 수요기관명 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridDmstPicEvl` | `faxNo` | 팩스번호 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridDmstPicEvl` | `picNm` | 담당자명 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridDmstPicEvl` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridDmstPicEvl` | `tlphNo` | 전화번호 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridList1` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridView3` | `bidClsfNo` | 입찰분류번호 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridView3` | `bidPbancItemSqno` | 입찰공고 품목순번 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridView3` | `devyCndtCd` | 인도조건 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridView3` | `dlvgdsPlacNm` | 납품장소 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridView3` | `dlvgdsTermDt` | 납품기한 날짜 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridView3` | `dlvgdsTermNody` | 납품일수 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridView3` | `dmstUntyGrpNm` | 수요기관명 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridView3` | `dmstUntyGrpNo` | 수요기관 통합기관번호 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridView3` | `dtlsPrnmNm` | 세부품명 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridView3` | `dtlsPrnmNo` | 세부품명번호 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridView3` | `itemClsfNo` | 물품분류번호 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridView3` | `itemDtlSpecCn` | 품목 상세규격 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridView3` | `itemIdnfNm` | 물품식별명/규격 표시 후보 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridView3` | `itemIdnfNo` | 물품식별번호 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridView3` | `prchsDtlItemQty` | 구매대상 품목수량 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridView3` | `prchsDtlItemUntVal` | 구매대상 품목 단위 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridView3` | `prchsDtlItemUprc` | 구매대상 품목 추정단가 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridView3` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridView6` | `bidClsfNo` | 입찰분류번호 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridView6` | `bidPbancItemSqno` | 입찰공고 품목순번 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridView6` | `devyCndtCd` | 인도조건 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridView6` | `dlvgdsPlacNm` | 납품장소 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridView6` | `dlvgdsTermDt` | 납품기한 날짜 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridView6` | `dlvgdsTermNody` | 납품일수 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridView6` | `dmstUntyGrpNm` | 수요기관명 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridView6` | `dmstUntyGrpNo` | 수요기관 통합기관번호 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridView6` | `dtlsPrnmNm` | 세부품명 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridView6` | `dtlsPrnmNo` | 세부품명번호 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridView6` | `itemClsfNo` | 물품분류번호 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridView6` | `itemDtlSpecCn` | 품목 상세규격 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridView6` | `itemIdnfNm` | 물품식별명/규격 표시 후보 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridView6` | `itemIdnfNo` | 물품식별번호 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridView6` | `prchsDtlItemQty` | 구매대상 품목수량 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridView6` | `prchsDtlItemUntVal` | 구매대상 품목 단위 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridView6` | `prchsDtlItemUprc` | 구매대상 품목 추정단가 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridView6` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridViewPgst` | `bidClsfNo` | 입찰분류번호 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridViewPgst` | `bidPbancNm` | 입찰공고명 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridViewPgst` | `bidPbancNo` | 입찰공고번호(본번호) | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridViewPgst` | `bidPbancOrd` | 입찰공고차수 | 제외: 다른 출처 | 공고 상세 안에 뜬 통합 조회 팝업(PNPE027) — 같은 공고의 본 표·머리와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridViewPgst` | `bidPgstNm` | 입찰진행상태 명칭(입160010 → 개찰완료) | 수집 | 같은 ID 원칙(2026-10-15) — g2b-notice-b-v1 · mf_wfm_container_mainWframe_gridViewPgst 에서 확정 |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridViewPgst` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-notice-a-service-v1` | `mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridViewPgst` | `totCnt` | 해당 표 전체건수 후보(품목 전체건수와 구별) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-b-v1 · mf_wfm_container_mainWframe_gridViewPgst 에서 확인 |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs1_body_wframe6_grdAliasDmTtl06List` | `CHK` | 행 선택 상태 | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs1_body_wframe6_grdAliasDmTtl06List` | `no` | 표시/출력/내부 순번(업무 키로 사용 금지) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs1_body_wframe6_grdAliasDmTtl06List` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs2_body_wframe7_grdAliasDmTtl07LeftList` | `CHK` | 행 선택 상태 | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs2_body_wframe7_grdAliasDmTtl07LeftList` | `bidLmtSeCd` | 입찰제한 구분 | 후보 | 같은 꼴 — g2b-notice-a-v1 · 같은 표의 상태를 따른다(2026-10-15 일반용역 코퍼스 관찰) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs2_body_wframe7_grdAliasDmTtl07LeftList` | `bidLmtUntyCd` | 입찰참가 제한 업종코드 | 제외: 다른 출처 | 같은 꼴 — g2b-notice-a-v1 · 같은 표의 상태를 따른다(2026-10-15 일반용역 코퍼스 관찰) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs2_body_wframe7_grdAliasDmTtl07LeftList` | `bidLmtUntyNm` | 입찰참가 제한 업종명 | 수집 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · mf_wfm_container_tabCont_contents_itemTabs2_body_wframe7_grdAliasDmTtl07LeftList 에서 확정 |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs2_body_wframe7_grdAliasDmTtl07LeftList` | `cnscAbltEvlAmt` | 시공능력 평가액 후보 | 후보 | 같은 꼴 — g2b-notice-a-v1 · 같은 표의 상태를 따른다(2026-10-15 일반용역 코퍼스 관찰) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs2_body_wframe7_grdAliasDmTtl07LeftList` | `lmtGupSqno` | 제한그룹 순번/명 | 후보 | 같은 꼴 — g2b-notice-a-v1 · 같은 표의 상태를 따른다(2026-10-15 일반용역 코퍼스 관찰) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs2_body_wframe7_grdAliasDmTtl07LeftList` | `lmtGupSqnoNm` | 제한그룹 순번/명 | 후보 | 같은 꼴 — g2b-notice-a-v1 · 같은 표의 상태를 따른다(2026-10-15 일반용역 코퍼스 관찰) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs2_body_wframe7_grdAliasDmTtl07LeftList` | `lmtKndCd` | 제한종류 코드 | 후보 | 같은 꼴 — g2b-notice-a-v1 · 같은 표의 상태를 따른다(2026-10-15 일반용역 코퍼스 관찰) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs2_body_wframe7_grdAliasDmTtl07LeftList` | `lmtSqno` | 제한 순번 | 후보 | 같은 꼴 — g2b-notice-a-v1 · 같은 표의 상태를 따른다(2026-10-15 일반용역 코퍼스 관찰) |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs2_body_wframe7_grdAliasDmTtl07LeftList` | `no` | 표시/출력/내부 순번(업무 키로 사용 금지) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs2_body_wframe7_grdAliasDmTtl07LeftList` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs4_body_wframe2_grdPrpsDmndInfoView` | `kbrdrId` | 시스템 등록/변경/입력자(수집 제외) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_grdEtpsLst 에서 확인 |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs4_body_wframe2_grdPrpsDmndInfoView` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `CHK` | 행 선택 상태 | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `dmstUntyGrpNo` | 수요기관 통합기관번호 | 수집 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · pointInfo 에서 확정 |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `itemClsfNo` | 물품분류번호 | 수집 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridView 에서 확정 |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `itemIdnfNm` | 물품식별명/규격 표시 후보 | 수집 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridView 에서 확정 · 이 화면에서는 코퍼스에서 늘 비어 있음 |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `ndfsItemSpecNo` | 규격번호 | 수집 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridView 에서 확정 · 이 화면에서는 코퍼스에서 늘 비어 있음 |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `ndfsPrcmDmndNo` | 국방 조달요구번호 | 수집 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridView 에서 확정 · 이 화면에서는 코퍼스에서 늘 비어 있음 |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `no` | 표시/출력/내부 순번(업무 키로 사용 금지) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-notice-a-service-v1` | `mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-notice-a-service-v1` | `pointInfo` | `grnteCrdrNm` | 보증 채권자 후보 — 값은 기관장 직함 | 후보 | 사용자 미정 2026-10-16 |

## `g2b-notice-b-v1` — 입찰공고상세조회(통합) (01196)

수집 63 · 제외: 다른 출처 41 · 제외: 뜻 없음 34 · 후보 45

| 프로필 | 출처 | ID | 뜻 | 상태 | 근거 |
|---|---|---|---|---|---|
| `g2b-notice-b-v1` | `pointInfo` | `(빈 ID)` | 빈 ID(출처 복원 불가) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-b-v1` | `pointInfo` | `alotBgtAmt` | 배정예산 | 수집 | 사용자 확인 2026-09-27(질문 Q01 일괄 승인) |
| `g2b-notice-b-v1` | `pointInfo` | `areaCd` | 화면/영역 코드(업무 의미 미확정) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-b-v1` | `pointInfo` | `atmtCntt01` | 화면 도움말 HTML(사업 특기사항 아님) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-b-v1` | `pointInfo` | `bdngAmtRangLmtYnNm` | 투찰금액 범위제한 여부 | 후보 | ID·표 맥락 |
| `g2b-notice-b-v1` | `pointInfo` | `bfSpecRegNo` | 사전규격 등록번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-b-v1` | `pointInfo` | `bidBlffNm` | 입찰 집행관명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-b-v1` | `pointInfo` | `bidMthdNm` | 입찰방식 명칭 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-b-v1` | `pointInfo` | `bidPbancNm` | 입찰공고명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-b-v1` | `pointInfo` | `bidPbancText` | 입찰공고번호·차수 결합 표시 | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-notice-b-v1` | `pointInfo` | `bidPrcpLmtYnNm` | 입찰참가 제한 여부 | 후보 | ID·표 맥락 |
| `g2b-notice-b-v1` | `pointInfo` | `bizAmt` | 사업금액(추정가격+부가세) | 수집 | 사용자 확인 2026-09-27(질문 Q01 일괄 승인) |
| `g2b-notice-b-v1` | `pointInfo` | `ctrtTyNm` | 계약구분 명칭 | 수집 | 사용자 확인 2026-09-27(일괄 답변 10 / R11 · 답변 3 명칭 우선) |
| `g2b-notice-b-v1` | `pointInfo` | `depth1` | 화면 경로 단계 후보 | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-b-v1` | `pointInfo` | `depth2` | 화면 경로 단계 후보 | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-b-v1` | `pointInfo` | `depth3` | 화면 경로 단계 후보 | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-b-v1` | `pointInfo` | `dgtnBizYnNm` | 정보화사업 여부 | 후보 | ID·표 맥락 |
| `g2b-notice-b-v1` | `pointInfo` | `dmstcOvrsSeNm` | 국내/국제 입찰구분 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-b-v1` | `pointInfo` | `fctrRspbText` | 하자담보책임기간 표시 | 수집 | 사용자 확인 2026-09-27(일괄 답변 7 / R08) |
| `g2b-notice-b-v1` | `pointInfo` | `fprdYnNm` | 외국산 허용 여부 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-b-v1` | `pointInfo` | `grnteCrdrNm` | 보증 채권자 후보 — 값은 기관장 직함 | 후보 | 사용자 미정 2026-10-16 |
| `g2b-notice-b-v1` | `pointInfo` | `igiInspInstSeNm` | 검수기관 구분 후보 | 후보 | ID·표 맥락 |
| `g2b-notice-b-v1` | `pointInfo` | `infoLeakPrhbTrgtYnNm` | 정보누출금지 대상 여부 | 후보 | ID·표 맥락 |
| `g2b-notice-b-v1` | `pointInfo` | `inspInstSeNm` | 검사기관 구분 | 후보 | ID·표 맥락 |
| `g2b-notice-b-v1` | `pointInfo` | `itemClsfBdngLmtAtrbNm` | 물품분류 투찰제한 속성 | 후보 | ID·표 맥락 |
| `g2b-notice-b-v1` | `pointInfo` | `jintCtrtCmnMthoNm` | 공동계약 구성방식 표시명 | 후보 | ID·표 맥락 |
| `g2b-notice-b-v1` | `pointInfo` | `lcnsLmtYnNm` | 업종·면허 제한 여부 | 후보 | ID·표 맥락 |
| `g2b-notice-b-v1` | `pointInfo` | `lotmCtnuCtrtYnNm` | 장기계속계약 여부 | 후보 | ID·표 맥락 |
| `g2b-notice-b-v1` | `pointInfo` | `lrcoPtcpLmtExcpYnNm` | 대기업 참여제한 예외 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-notice-b-v1` | `pointInfo` | `mnftrItemLmtCn` | 제조물품 제한 내용 | 후보 | ID·표 맥락 |
| `g2b-notice-b-v1` | `pointInfo` | `mnftrItemLmtYnNm` | 제조물품 제한 여부 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-b-v1` | `pointInfo` | `pbancInstUntyGrpNm` | 공고기관명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-b-v1` | `pointInfo` | `pbancKndNm` | 공고종류 | 수집 | 사용자 확인 2026-09-27(일괄 답변 3 / R03) |
| `g2b-notice-b-v1` | `pointInfo` | `pbancPstgDt` | 공고 게시일시 | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R13) |
| `g2b-notice-b-v1` | `pointInfo` | `pnprDcsnMthoNm` | 예정가격 결정방법 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-b-v1` | `pointInfo` | `prfmncLmtYnNm` | 실적제한 여부 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-b-v1` | `pointInfo` | `prspPrce` | 추정가격 | 수집 | 사용자 확인 2026-09-27(질문 Q01 일괄 승인) |
| `g2b-notice-b-v1` | `pointInfo` | `prttnDlvgdsPsbltyYnNm` | 분할납품 가능 여부 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-b-v1` | `pointInfo` | `qttyAlotSeNm` | 물량배정 구분 | 후보 | ID·표 맥락 |
| `g2b-notice-b-v1` | `pointInfo` | `rbidPrmsYnNm` | 재입찰 허용 여부 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-b-v1` | `pointInfo` | `rgnLmtYnNm` | 지역제한 여부/표시 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-b-v1` | `pointInfo` | `rschDvlpItemYnNm` | 연구개발물품 여부 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-b-v1` | `pointInfo` | `samePrceBidAtmtDrawYnNm` | 동가 입찰 자동추첨 여부 | 후보 | ID·표 맥락 |
| `g2b-notice-b-v1` | `pointInfo` | `scsbdLwlmRat` | 낙찰하한율 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-b-v1` | `pointInfo` | `scsbdMthdDtlsNm` | 낙찰방법 세부기준 표시 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-b-v1` | `pointInfo` | `scsbdMthdNm` | 낙찰방법 | 수집 | 사용자 확인 2026-09-27(일괄 답변 10 / R11 · 답변 3 명칭 우선) |
| `g2b-notice-b-v1` | `pointInfo` | `smrlCmptItemYnNm` | 중소기업간 경쟁제품 여부 | 후보 | ID·표 맥락 |
| `g2b-notice-b-v1` | `pointInfo` | `stdCtrtMthdNm` | 계약방법 | 수집 | 사용자 확인 2026-09-27(일괄 답변 10 / R11 · 답변 3 명칭 우선) |
| `g2b-notice-b-v1` | `pointInfo` | `wtoBidRsnNm` | 국제입찰 사유 | 후보 | ID·표 맥락 |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridList1` | `endDt` | 시작/종료 일시(일정표) | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R13) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridList1` | `ord` | 일정 순번 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) 답 「모름」 · 표의 행 키라 매핑이 읽는다(읽는 법 — 식별 필드) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridList1` | `placNm` | 장소(일정표) | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridList1` | `prgNm` | 진행방법(일정표) | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R13) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridList1` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridList1` | `startDt` | 시작/종료 일시(일정표) | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R13) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridList1` | `subject` | 진행명(일정표) | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R13) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView4` | `bidClsfNo` | 입찰분류번호 | 제외: 다른 출처 | 같은 ID 원칙(2026-10-15) — g2b-notice-b-v1 · mf_wfm_container_mainWframe_gridView3 에서 확정 |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView4` | `bidPbancNo` | 입찰공고번호(본번호) | 제외: 다른 출처 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · pointInfo 에서 확정 · 공고 머리의 번호·차수(bidPbancText)와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView4` | `bidPbancOrd` | 입찰공고차수 | 제외: 다른 출처 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · pointInfo 에서 확정 · 공고 머리의 번호·차수(bidPbancText)와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView4` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView4` | `usefAmt` | 용도별/분류별 금액(정확한 용어 미확정) | 후보 | ID·표 맥락 · Q01 에서 물었으나 답 없음 |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView5` | `baseAmt` | 기초금액 | 수집 | 사용자 확인 2026-09-27(질문 Q01 일괄 승인) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView5` | `bidClsfNo` | 입찰분류번호 | 제외: 다른 출처 | 같은 ID 원칙(2026-10-15) — g2b-notice-b-v1 · mf_wfm_container_mainWframe_gridView3 에서 확정 |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView5` | `bidPbancNo` | 입찰공고번호(본번호) | 제외: 다른 출처 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · pointInfo 에서 확정 · 공고 머리의 번호·차수(bidPbancText)와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView5` | `bidPbancOrd` | 입찰공고차수 | 제외: 다른 출처 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · pointInfo 에서 확정 · 공고 머리의 번호·차수(bidPbancText)와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView5` | `inptDt` | 등록/변경/입력일시(업무일자와 구별) | 후보 | 구조·이름 분류 |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView5` | `rmrkCn` | 비고 내용 | 후보 | 빈 열만 확인 |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView5` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridDmstPic` | `deptNm` | 부서명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridDmstPic` | `dmstPicId` | 수요기관 담당자 ID/명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridDmstPic` | `dmstPicNm` | 수요기관 담당자 ID/명 | 후보 | 빈 열만 확인 |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridDmstPic` | `dmstUntyGrpNm` | 수요기관명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridDmstPic` | `eml` | 이메일 | 후보 | ID·표 맥락 |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridDmstPic` | `evlPicYn` | 평가담당자 여부 | 후보 | ID·표 맥락 |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridDmstPic` | `faxNo` | 팩스번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridDmstPic` | `picNm` | 담당자명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridDmstPic` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridDmstPic` | `tlphNo` | 전화번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-notice-a-v1 의 답) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridDmstPicEvl` | `deptNm` | 부서명 | 제외: 다른 출처 | 코퍼스 대조(2026-10-15) — 모든 행이 gridDmstPic 의 한 행과 담는 여섯 칸(deptNm·dmstPicId·dmstUntyGrpNm·faxNo·picNm·tlphNo)이 같고 평가담당자 여부(evlPicYn)만 갈린다: 수요기관 담당자 표의 사본 |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridDmstPicEvl` | `dmstPicId` | 수요기관 담당자 ID/명 | 제외: 다른 출처 | 코퍼스 대조(2026-10-15) — 모든 행이 gridDmstPic 의 한 행과 담는 여섯 칸(deptNm·dmstPicId·dmstUntyGrpNm·faxNo·picNm·tlphNo)이 같고 평가담당자 여부(evlPicYn)만 갈린다: 수요기관 담당자 표의 사본 |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridDmstPicEvl` | `dmstPicNm` | 수요기관 담당자 ID/명 | 후보 | 빈 열만 확인 |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridDmstPicEvl` | `dmstUntyGrpNm` | 수요기관명 | 제외: 다른 출처 | 코퍼스 대조(2026-10-15) — 모든 행이 gridDmstPic 의 한 행과 담는 여섯 칸(deptNm·dmstPicId·dmstUntyGrpNm·faxNo·picNm·tlphNo)이 같고 평가담당자 여부(evlPicYn)만 갈린다: 수요기관 담당자 표의 사본 |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridDmstPicEvl` | `eml` | 이메일 | 후보 | ID·표 맥락 |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridDmstPicEvl` | `evlPicYn` | 평가담당자 여부 | 후보 | ID·표 맥락 |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridDmstPicEvl` | `faxNo` | 팩스번호 | 제외: 다른 출처 | 코퍼스 대조(2026-10-15) — 모든 행이 gridDmstPic 의 한 행과 담는 여섯 칸(deptNm·dmstPicId·dmstUntyGrpNm·faxNo·picNm·tlphNo)이 같고 평가담당자 여부(evlPicYn)만 갈린다: 수요기관 담당자 표의 사본 |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridDmstPicEvl` | `picNm` | 담당자명 | 제외: 다른 출처 | 코퍼스 대조(2026-10-15) — 모든 행이 gridDmstPic 의 한 행과 담는 여섯 칸(deptNm·dmstPicId·dmstUntyGrpNm·faxNo·picNm·tlphNo)이 같고 평가담당자 여부(evlPicYn)만 갈린다: 수요기관 담당자 표의 사본 |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridDmstPicEvl` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridDmstPicEvl` | `tlphNo` | 전화번호 | 제외: 다른 출처 | 코퍼스 대조(2026-10-15) — 모든 행이 gridDmstPic 의 한 행과 담는 여섯 칸(deptNm·dmstPicId·dmstUntyGrpNm·faxNo·picNm·tlphNo)이 같고 평가담당자 여부(evlPicYn)만 갈린다: 수요기관 담당자 표의 사본 |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView3` | `bidClsfNo` | 입찰분류번호 | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView3` | `bidPbancItemSqno` | 입찰공고 품목순번 | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView3` | `devyCndtCd` | 인도조건 | 수집 | 사용자 확인 2026-09-27(일괄 답변 6 / R07) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView3` | `devyCndtNm` | 인도조건 | 제외: 다른 출처 | 사용자 확인 2026-09-27(일괄 답변 6 / R07) — 같은 뜻 devyCndtCd 를 수집 |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView3` | `dlvgdsPlacNm` | 납품장소 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView3` | `dlvgdsTermDt` | 납품기한 날짜 | 수집 | 사용자 확인 2026-09-27(일괄 답변 5 / R06) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView3` | `dlvgdsTermNody` | 납품일수 | 수집 | 사용자 확인 2026-09-27(일괄 답변 5 / R06) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView3` | `dmstUntyGrpNm` | 수요기관명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView3` | `dmstUntyGrpNo` | 수요기관 통합기관번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView3` | `dtlsPrnmNm` | 세부품명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView3` | `dtlsPrnmNo` | 세부품명번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView3` | `itemClsfNm` | 물품분류명 | 후보 | ID·표 맥락 |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView3` | `itemClsfNo` | 물품분류번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-contract-v1 의 답) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView3` | `itemDtlSpecCn` | 품목 상세규격 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView3` | `itemIdnfNm` | 물품식별명/규격 표시 후보 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView3` | `itemIdnfNo` | 물품식별번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView3` | `prchsDtlItemQty` | 구매대상 품목수량 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView3` | `prchsDtlItemUntVal` | 구매대상 품목 단위 | 수집 | 사용자 확인 2026-09-27(일괄 답변 6 / R07) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView3` | `prchsDtlItemUntValNm` | 구매대상 품목 단위명(단위 Val 과 같은 값) | 제외: 다른 출처 | 사용자 확인 2026-09-27(일괄 답변 6 / R07) — 같은 값 prchsDtlItemUntVal 을 수집 |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView3` | `prchsDtlItemUprc` | 구매대상 품목 추정단가 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView3` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView3` | `ctrtDmndRcptNo` | 계약요청 접수번호 | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView3` | `ctrtDmndRcptOrd` | 계약요청 접수차수 | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView3` | `ctrtDmndRcptItemSqno` | 계약요청 접수 품목순번 | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView6` | `bidClsfNo` | 입찰분류번호 | 제외: 다른 출처 | gridView3 과 공통 열이 같다(전수조사 「중복 수집원 확인」) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView6` | `bidPbancItemSqno` | 입찰공고 품목순번 | 제외: 다른 출처 | gridView3 과 공통 열이 같다(전수조사 「중복 수집원 확인」) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView6` | `devyCndtCd` | 인도조건 | 제외: 다른 출처 | gridView3 과 공통 열이 같다(전수조사 「중복 수집원 확인」) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView6` | `devyCndtNm` | 인도조건 | 제외: 다른 출처 | gridView3 과 공통 열이 같다(전수조사 「중복 수집원 확인」) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView6` | `dlvgdsPlacNm` | 납품장소 | 제외: 다른 출처 | gridView3 과 공통 열이 같다(전수조사 「중복 수집원 확인」) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView6` | `dlvgdsTermDt` | 납품기한 날짜 | 제외: 다른 출처 | gridView3 과 공통 열이 같다(전수조사 「중복 수집원 확인」) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView6` | `dlvgdsTermNody` | 납품일수 | 제외: 다른 출처 | gridView3 과 공통 열이 같다(전수조사 「중복 수집원 확인」) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView6` | `dmstUntyGrpNm` | 수요기관명 | 제외: 다른 출처 | gridView3 과 공통 열이 같다(전수조사 「중복 수집원 확인」) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView6` | `dmstUntyGrpNo` | 수요기관 통합기관번호 | 제외: 다른 출처 | gridView3 과 공통 열이 같다(전수조사 「중복 수집원 확인」) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView6` | `dtlsPrnmNm` | 세부품명 | 제외: 다른 출처 | gridView3 과 공통 열이 같다(전수조사 「중복 수집원 확인」) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView6` | `dtlsPrnmNo` | 세부품명번호 | 제외: 다른 출처 | gridView3 과 공통 열이 같다(전수조사 「중복 수집원 확인」) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView6` | `iritemClsfNm` | 기관별 물품분류번호/명 후보 | 후보 | 빈 열만 확인 |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView6` | `iritemClsfNo` | 기관별 물품분류번호/명 후보 | 후보 | 빈 열만 확인 |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView6` | `iritemClsfNoUseYn` | 기관별 물품분류번호 사용 여부 후보 | 후보 | 빈 열만 확인 |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView6` | `itemClsfNm` | 물품분류명 | 제외: 다른 출처 | gridView3 과 공통 열이 같다(전수조사 「중복 수집원 확인」) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView6` | `itemClsfNo` | 물품분류번호 | 제외: 다른 출처 | gridView3 과 공통 열이 같다(전수조사 「중복 수집원 확인」) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView6` | `itemDtlSpecCn` | 품목 상세규격 | 제외: 다른 출처 | gridView3 과 공통 열이 같다(전수조사 「중복 수집원 확인」) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView6` | `itemIdnfNm` | 물품식별명/규격 표시 후보 | 제외: 다른 출처 | gridView3 과 공통 열이 같다(전수조사 「중복 수집원 확인」) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView6` | `itemIdnfNo` | 물품식별번호 | 제외: 다른 출처 | gridView3 과 공통 열이 같다(전수조사 「중복 수집원 확인」) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView6` | `prchsDtlItemQty` | 구매대상 품목수량 | 제외: 다른 출처 | gridView3 과 공통 열이 같다(전수조사 「중복 수집원 확인」) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView6` | `prchsDtlItemUntVal` | 구매대상 품목 단위 | 제외: 다른 출처 | gridView3 과 공통 열이 같다(전수조사 「중복 수집원 확인」) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView6` | `prchsDtlItemUntValNm` | 구매대상 품목 단위 | 제외: 다른 출처 | gridView3 과 공통 열이 같다(전수조사 「중복 수집원 확인」) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView6` | `prchsDtlItemUprc` | 구매대상 품목 추정단가 | 제외: 다른 출처 | gridView3 과 공통 열이 같다(전수조사 「중복 수집원 확인」) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridView6` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridViewPgst` | `bidClsfNo` | 입찰분류번호 | 제외: 다른 출처 | 같은 ID 원칙(2026-10-15) — g2b-notice-b-v1 · mf_wfm_container_mainWframe_gridView3 에서 확정 |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridViewPgst` | `bidClsfYn` | 입찰분류 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridViewPgst` | `bidPbancNm` | 입찰공고명 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-notice-a-v1 의 답) · 머리의 공고명(bidPbancNm)과 같은 값 — 한 문서는 괄호를 HTML 문자 참조로 적었다(코퍼스 대조 2026-10-15) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridViewPgst` | `bidPbancNo` | 입찰공고번호(본번호) | 제외: 다른 출처 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · pointInfo 에서 확정 · 공고 머리의 번호·차수(bidPbancText)와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridViewPgst` | `bidPbancNoOrd` | 입찰공고번호·차수 결합 표시 | 후보 | ID·표 맥락 |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridViewPgst` | `bidPbancOrd` | 입찰공고차수 | 제외: 다른 출처 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · pointInfo 에서 확정 · 공고 머리의 번호·차수(bidPbancText)와 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridViewPgst` | `bidPgstCd` | 입찰진행상태 코드 | 제외: 다른 출처 | 원문 명칭 bidPgstNm 을 수집(답변 3 명칭 우선) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridViewPgst` | `bidPgstNm` | 입찰진행상태 명칭(입160010 → 개찰완료) | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridViewPgst` | `bidPrgrsOrd` | 입찰진행차수 | 후보 | ID·표 맥락 |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridViewPgst` | `onbsPrnmntDt` | 개찰 예정일시 | 후보 | ID·표 맥락 |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridViewPgst` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_gridViewPgst` | `totCnt` | 해당 표 전체건수 후보(품목 전체건수와 구별) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-b-v1` | `*_grdFile` | `CHK` | 행 선택 상태 | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-b-v1` | `*_grdFile` | `atchFileDscr` | 첨부파일 설명 | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R14) |
| `g2b-notice-b-v1` | `*_grdFile` | `atchFileKndCd` | 첨부파일 종류코드(문서유형명은 미확정) | 후보 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-b-v1` | `*_grdFile` | `atchFileNm` | 서버 내부 파일경로/저장명/이미지 URL(수집 제외) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-b-v1` | `*_grdFile` | `atchFileSqno` | 첨부파일순번 | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R14) |
| `g2b-notice-b-v1` | `*_grdFile` | `atflPathNm` | 서버 내부 파일경로/저장명/이미지 URL(수집 제외) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-b-v1` | `*_grdFile` | `atflRmrk` | 첨부파일 비고 | 후보 | 빈 열만 확인 |
| `g2b-notice-b-v1` | `*_grdFile` | `befRowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-b-v1` | `*_grdFile` | `bsneClsfCd` | 내부 업무분류코드 | 후보 | 구조·이름 분류 |
| `g2b-notice-b-v1` | `*_grdFile` | `chngDttm` | 등록/변경/입력일시(업무일자와 구별) | 후보 | 빈 열만 확인 |
| `g2b-notice-b-v1` | `*_grdFile` | `chnrId` | 시스템 등록/변경/입력자(수집 제외) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-b-v1` | `*_grdFile` | `chnrIpar` | 등록/변경 IP 성격 값(수집 제외) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-b-v1` | `*_grdFile` | `colNm` | 내부 테이블/컬럼명 | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-b-v1` | `*_grdFile` | `delYn` | 삭제 여부 | 후보 | 빈 열만 확인 |
| `g2b-notice-b-v1` | `*_grdFile` | `dwnldPrmsYn` | 다운로드 허용 여부 | 후보 | 빈 열만 확인 |
| `g2b-notice-b-v1` | `*_grdFile` | `fileExtnNm` | 파일 확장자 | 후보 | ID·표 맥락 |
| `g2b-notice-b-v1` | `*_grdFile` | `fileSz` | 파일 크기(단위 확인) | 후보 | ID·표 맥락 · R14 「파일 크기 후보」, 단위 미확정 |
| `g2b-notice-b-v1` | `*_grdFile` | `imgUrl` | 서버 내부 파일경로/저장명/이미지 URL(수집 제외) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-b-v1` | `*_grdFile` | `inptDt` | 등록/변경/입력일시(업무일자와 구별) | 후보 | 구조·이름 분류 |
| `g2b-notice-b-v1` | `*_grdFile` | `kbrdrNm` | 시스템 등록/변경/입력자(수집 제외) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-b-v1` | `*_grdFile` | `nIndex` | 표시/출력/내부 순번(업무 키로 사용 금지) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-b-v1` | `*_grdFile` | `orgnlAtchFileNm` | 원본 첨부파일명 | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R14) |
| `g2b-notice-b-v1` | `*_grdFile` | `rgstDttm` | 등록/변경/입력일시(업무일자와 구별) | 후보 | 빈 열만 확인 |
| `g2b-notice-b-v1` | `*_grdFile` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-b-v1` | `*_grdFile` | `rstrId` | 시스템 등록/변경/입력자(수집 제외) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-b-v1` | `*_grdFile` | `rstrIpar` | 등록/변경 IP 성격 값(수집 제외) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-b-v1` | `*_grdFile` | `scrnIndtOdr` | 화면 표시순서 | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-b-v1` | `*_grdFile` | `sessUserNo` | 세션 사용자번호(수집 제외) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-b-v1` | `*_grdFile` | `tblNm` | 내부 테이블/컬럼명 | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-notice-b-v1` | `*_grdFile` | `untyAtchFileNo` | 통합 첨부파일번호(원문 참조) | 후보 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_grdEtspUntyGrp` | `bidPbancNo` | 입찰공고번호(본번호) | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_grdEtspUntyGrp` | `bidPbancOrd` | 입찰공고차수 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_grdEtspUntyGrp` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_grdLcnsLmt` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-notice-b-v1` | `mf_wfm_container_mainWframe_grdPrpsDmndInfoView` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |

## `g2b-contract-v1` — 계약 상세 (01579)

수집 67 · 제외: 개인정보 7 · 제외: 다른 출처 216 · 제외: 뜻 없음 40 · 후보 335

| 프로필 | 출처 | ID | 뜻 | 상태 | 근거 |
|---|---|---|---|---|---|
| `g2b-contract-v1` | `pointInfo` | `(빈 ID)` | 빈 ID(출처 복원 불가) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `pointInfo` | `advcPinfrmPsbltyRat` | 선급금 선고지 비율(%) | 수집 | 사용자 확인 2026-09-27(질문 Q04 부분 확정) |
| `g2b-contract-v1` | `pointInfo` | `advcPinfrmYn` | 선급금 선고지 여부 | 수집 | 사용자 확인 2026-09-27(일괄 답변 7 / R08) |
| `g2b-contract-v1` | `pointInfo` | `affcClclnTrgtYn` | 사후정산 대상 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `alotAmt` | 배정금액 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `aplcnCrtrPrce` | 적용기준가격 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `areaCd` | 화면/영역 코드(업무 의미 미확정) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `pointInfo` | `atcsNm` | 국가(지방)계약법시행령 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-v1` | `pointInfo` | `atmtCntt01` | 화면 도움말 HTML(사업 특기사항 아님) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `pointInfo` | `bderCnt` | 입찰자 수 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `bidPbancNo` | 입찰공고번호(본번호) | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-contract-v1` | `pointInfo` | `bondPchaAmt` | 채권 매입금액 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `bondSeCd` | 채권구분 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `bsamCmptProdYn` | 중소기업간 경쟁제품 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `bulkDrftOdn1BdcnCn` | 일괄기안 부가조건 내용 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `bulkDrftOdn2BdcnCn` | 일괄기안 부가조건 내용 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `bzmnRegNo` | 사업자등록번호 | 제외: 다른 출처 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_grdEtpsLst 에서 확정 |
| `g2b-contract-v1` | `pointInfo` | `cgntTxam` | 인지세 부과액(물품제조계약) | 수집 | 사용자 확인 2026-09-27(질문 Q02 부분 확정) |
| `g2b-contract-v1` | `pointInfo` | `chckrId` | 검수자 ID/명 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `chckrNm` | 검수자 ID/명 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `chgRsn` | 계약 변경사유 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `cltuItemYn` | 문화상품 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `ctentPicId` | 계약상대자 담당자 ID/명 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `ctentPicNm` | 계약상대자 담당자 ID/명 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `ctentSeCd` | 계약상대자 구분 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `ctentTlphNo` | 계약상대자 전화번호 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `ctentUntyGrpNm` | 계약상대자명 | 제외: 다른 출처 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_grdEtpsLst 에서 확정 |
| `g2b-contract-v1` | `pointInfo` | `ctentUntyGrpNo` | 계약상대자 통합기관번호 | 제외: 다른 출처 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_grdEtpsLst 에서 확정 |
| `g2b-contract-v1` | `pointInfo` | `ctrtAmt` | 선택 품목의 계약금액 | 제외: 다른 출처 | 사용자 확인 2026-09-27(질문 Q02 부분 확정) — 선택 품목 값, 품목표에서 수집 |
| `g2b-contract-v1` | `pointInfo` | `ctrtAprvYmd` | 계약 승인일자 | 후보 | ID·표 맥락 · 옛 미사용 보류(R01) |
| `g2b-contract-v1` | `pointInfo` | `ctrtBrcoOrgnCd` | 계약 지청·조직코드 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `ctrtChgSeCd` | 계약변경구분 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `ctrtChrsIdntYn` | 계약특성 확인 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `ctrtChrsSeCd` | 계약특성 | 수집 | 사용자 확인 2026-10-15(화면에서 확인한 명칭 — 계약특성) |
| `g2b-contract-v1` | `pointInfo` | `ctrtDmndRcptNo` | 계약요청 접수번호 | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-contract-v1` | `pointInfo` | `ctrtDpdrNm` | 계약 조직/직책명(개별 약어 미확정) | 후보 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-v1` | `pointInfo` | `ctrtDt` | 현재 차수 계약일 | 수집 | 사용자 확인 2026-09-27(일괄 답변 1 / R01) |
| `g2b-contract-v1` | `pointInfo` | `ctrtDvofNm` | 계약 조직/직책명(개별 약어 미확정) | 후보 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-v1` | `pointInfo` | `ctrtDvtkSeNm` | 계약관 직책명 | 후보 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-v1` | `pointInfo` | `ctrtFee` | 수수료 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-v1` | `pointInfo` | `ctrtGrnteAmt` | 계약 전체의 계약보증금액 | 수집 | 사용자 확인 2026-09-27(질문 Q02 부분 확정) |
| `g2b-contract-v1` | `pointInfo` | `ctrtGtnrt` | 계약보증금률 | 수집 | 사용자 확인 2026-09-27(질문 Q03 용어·단위 승인) |
| `g2b-contract-v1` | `pointInfo` | `ctrtItemNm` | 계약 품목명 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-contract-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본 |
| `g2b-contract-v1` | `pointInfo` | `ctrtMthdCd` | 계약방법 | 수집 | 사용자 확인 2026-09-27(일괄 답변 10 / R11) |
| `g2b-contract-v1` | `pointInfo` | `ctrtNm` | 계약명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-v1` | `pointInfo` | `ctrtNoOrd` | 계약번호·변경차수 결합 표시 | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-contract-v1` | `pointInfo` | `ctrtPicNm` | 계약담당자 | 수집 | 사용자 확인 2026-09-27(일괄 답변 11 / R12) |
| `g2b-contract-v1` | `pointInfo` | `ctrtPlanYear` | 국방 조달계획연도 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `ctrtPrdCn` | 납품기한 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-v1` | `pointInfo` | `ctrtQty` | 선택 품목의 계약수량 | 제외: 다른 출처 | 사용자 확인 2026-09-27(일괄 답변 2 / R02) — 선택 품목 값, 품목표에서 수집 |
| `g2b-contract-v1` | `pointInfo` | `ctrtSharAmt` | 현재 계약차수의 업체 지분금액 | 제외: 다른 출처 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_grdEtpsLst 에서 확정 |
| `g2b-contract-v1` | `pointInfo` | `ctrtSharRat` | 현재 계약차수의 업체 지분율(%) | 제외: 다른 출처 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_grdEtpsLst 에서 확정 |
| `g2b-contract-v1` | `pointInfo` | `ctrtSlcfNm` | 계약 조직/직책명(개별 약어 미확정) | 후보 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-v1` | `pointInfo` | `ctrtTyCd` | 계약구분(계120001 → 총액계약) | 수집 | 사용자 확인 2026-09-27(일괄 답변 10 / R11) |
| `g2b-contract-v1` | `pointInfo` | `ctrtUntVal` | 계약 품목단위 값/코드 | 제외: 다른 출처 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis 에서 확정 |
| `g2b-contract-v1` | `pointInfo` | `ctrtUprc` | 선택 품목의 계약단가 | 제외: 다른 출처 | 사용자 확인 2026-09-27(일괄 답변 2 / R02) — 선택 품목 값, 품목표에서 수집 |
| `g2b-contract-v1` | `pointInfo` | `ctwtSbmsnMthdCd` | 계약보증서 제출방법 | 수집 | 사용자 확인 2026-09-27(질문 Q03 용어·단위 승인) |
| `g2b-contract-v1` | `pointInfo` | `dcmtDepoEsntlPayTrgtYn` | 하자보수보증금 필수납부 대상 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `dcmtGtnrt` | 하자보수보증금률 | 수집 | 사용자 확인 2026-09-27(질문 Q03 용어·단위 승인) |
| `g2b-contract-v1` | `pointInfo` | `depth1` | 화면 경로 단계 후보 | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `pointInfo` | `depth2` | 화면 경로 단계 후보 | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `pointInfo` | `depth3` | 화면 경로 단계 후보 | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `pointInfo` | `devyCndtCd` | 인도조건 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본 |
| `g2b-contract-v1` | `pointInfo` | `devyCndtNm` | 인도조건 | 후보 | 원문 명칭 · 계약 전체 값인지 선택 품목 값인지 미확인 |
| `g2b-contract-v1` | `pointInfo` | `dgtnBizYn` | 정보화사업 여부 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `dlvgdsPlacNm` | 납품장소 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본(품목표 grdCtrtLis 의 한 행과 같은 값, 코퍼스 대조 2026-10-15) |
| `g2b-contract-v1` | `pointInfo` | `dlvgdsSqno` | 납품순번 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `dlvgdsTermNody` | 납품일수 | 제외: 다른 출처 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridView 에서 확정 · 머리 칸은 화면에서 고른 행의 사본(품목표 grdCtrtLis 의 한 행과 같은 값, 코퍼스 대조 2026-10-15) |
| `g2b-contract-v1` | `pointInfo` | `dlvgdsTermYmd` | 납품기한 날짜 | 제외: 다른 출처 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis 에서 확정 |
| `g2b-contract-v1` | `pointInfo` | `dmstAcmlSpdtAmt` | 수요기관 누적 인지세액 후보 | 후보 | ID·표 맥락 · 옛 일괄 제외 B1 |
| `g2b-contract-v1` | `pointInfo` | `dmstPicId` | 수요기관 담당자 ID/명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-notice-a-v1 의 답) |
| `g2b-contract-v1` | `pointInfo` | `dmstPicNm` | 수요기관 담당자 ID/명 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `dmstSpdtBrdnRt` | 수요기관 인지세 부담률 | 후보 | ID·표 맥락 · 옛 일괄 제외 B1 |
| `g2b-contract-v1` | `pointInfo` | `dmstSpdtJintPayYn` | 수요기관 인지세 공동납부 여부 | 후보 | ID·표 맥락 · 옛 일괄 제외 B1 |
| `g2b-contract-v1` | `pointInfo` | `dmstTlphNo` | 수요기관 전화번호 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `dmstUntyGrpNm` | 수요기관명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-v1` | `pointInfo` | `drpmSbpySeCd` | 지급방법(직130001 → 직불, 직130002 → 대지급) | 수집 | 사용자 확인 2026-09-27(일괄 답변 10 / R11) |
| `g2b-contract-v1` | `pointInfo` | `etpsCrtrBndlCtrtYn` | 업체 기준 묶음계약 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `etpsDlayNody` | 업체 지연일수/사유 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `etpsDlayRsn` | 업체 지연일수/사유 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `etpsPicId` | 업체 담당자 ID/명 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `etpsPicNm` | 업체 담당자 ID/명 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `extlPbfnYn` | 외부 재정시스템 사용 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `fctrRspbMtnb` | 하자담보책임기간(개월수) | 수집 | 사용자 확인 2026-09-27(질문 Q03 용어·단위 승인) |
| `g2b-contract-v1` | `pointInfo` | `fctrRspbNbyr` | 하자담보책임기간(연수) | 수집 | 사용자 확인 2026-09-27(질문 Q03 용어·단위 승인) |
| `g2b-contract-v1` | `pointInfo` | `feePayExmtTyCd` | 수수료 납부·면제 유형 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `flawEctnBudlGrnteYn` | 하자이행 일괄보증 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `fprdYn` | 외국산 허용 여부 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) |
| `g2b-contract-v1` | `pointInfo` | `frstCtrtDt` | 최초계약일 | 수집 | 사용자 확인 2026-09-27(일괄 답변 1 / R01) |
| `g2b-contract-v1` | `pointInfo` | `hefcEtamYn` | 고효율 기자재 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `igiInspInstSeCd` | 검수기관 구분 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `igiInstUntyGrpNm` | 검수기관 번호/명 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `igiInstUntyGrpNo` | 검수기관 번호/명 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `implBlmt` | 이행 잔액 후보 | 후보 | ID·표 맥락 · 옛 일괄 제외 B2 |
| `g2b-contract-v1` | `pointInfo` | `inscCtrtTrgtYnIdntYn` | 보험계약 대상 여부 확인 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `inspInstSeCd` | 검사기관 구분 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `inspInstUntyGrpNm` | 검사기관 번호/명 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `inspInstUntyGrpNo` | 검사기관 번호/명 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `inspNody` | 검사일수 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `inspTyCd` | 검사형태 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `insrId` | 검사자 ID/명 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `insrNm` | 검사자 ID/명 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `instDlayNody` | 기관 지연일수/사유 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `instDlayRsn` | 기관 지연일수/사유 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `invtProdCtrtYn` | 혁신제품 계약 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `itcemSbmsnYn` | 서약서/각서 제출 여부(문서종류 확인) | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `itemClsfNo` | 물품분류번호 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-contract-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본 |
| `g2b-contract-v1` | `pointInfo` | `itemIdnfNo` | 물품식별번호 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본 |
| `g2b-contract-v1` | `pointInfo` | `itemIvtrNoVal` | 재고번호 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `itemSpecVal` | 품목 상세규격 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-contract-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본 |
| `g2b-contract-v1` | `pointInfo` | `jintSyddCmnMthoCd` | 공동수급 구성방식 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `kimgrpIncsYn` | 특정 제품군 여부/포함 여부(약어 미확정) | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `kimgrpYn` | 특정 제품군 여부/포함 여부(약어 미확정) | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `laseYn` | 리스 여부 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `lcrtTyCd` | 계약법 구분(계040001 → 국가계약법, 계040002 → 지방계약법) | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-v1` | `pointInfo` | `lotmPrttnSbpyYn` | 장기 분할 지급 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `lotmYn` | 장기계약 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `lqdmRat` | 지체상금률(일당 %) | 수집 | 사용자 확인 2026-09-27(질문 Q03 용어·단위 승인) |
| `g2b-contract-v1` | `pointInfo` | `lvyBoaSeCd` | 징수·부담 구분 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `lwrdAtcsNm` | 하위 법령 조항 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `ndfmRgstTrgtYn` | 국방 등록대상 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `ndfsPrcmDmndNo` | 국방 조달요구번호 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본(품목표 grdCtrtLis 의 한 행과 같은 값, 코퍼스 대조 2026-10-15) |
| `g2b-contract-v1` | `pointInfo` | `newLotmSeCd` | 신규·장기 구분 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `nwEqpmCnsuOperBizYn` | 신규 장비 구축·운영 사업 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `odn1PlorNtnNm` | 원산지 국가(차수/순번 의미 확인) | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-contract-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본 |
| `g2b-contract-v1` | `pointInfo` | `ofdcCn` | 공문 내용 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `olCtrtYn` | 온라인계약 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `oncMaxDlreqAmt` | 1회 최대 납품요구금액 후보 | 후보 | ID·표 맥락 · 옛 일괄 제외 B2 |
| `g2b-contract-v1` | `pointInfo` | `oncMaxDlreqamtRt` | 1회 최대 납품요구금액 비율 후보 | 후보 | ID·표 맥락 · 옛 일괄 제외 B2 |
| `g2b-contract-v1` | `pointInfo` | `optnItmltDsgnYn` | 선택 품목 지정 여부(약어 미확정) | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `ovrlCtrtYn` | 총괄계약 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `pnpr` | 예정가격 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `prcmBsneAreaCd` | 조달업무영역 코드 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `prflcAplcnCrtrCd` | 물가변동적용기준(물130001 → 지수조정율, 물130002 → 품목조정율) | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-v1` | `pointInfo` | `procsCtrtWtotAgreYn` | 계약 진행 동의 여부(정확한 용어 미확정) | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `prttnDlvgdsPsbltyYn` | 분할납품 가능 여부 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-v1` | `pointInfo` | `prvctImplYn` | 수의계약 사유 관련 여부(약어 미확정) | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `pvctMmrdSbmsnYn` | 서약서/각서 제출 여부(문서종류 확인) | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `recordCountPerPage` | 페이지당 표시건수 | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `pointInfo` | `rlsYn` | 공개 여부 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) |
| `g2b-contract-v1` | `pointInfo` | `rprsCtentUntyGrpNm` | 대표 계약상대자명 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `rprsvNm` | 대표자명 | 제외: 다른 출처 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-contract-change-v1 의 답) · 머리 칸은 화면에서 고른 행의 사본 |
| `g2b-contract-v1` | `pointInfo` | `rprsvUnptYnIdntYn` | 부정당 제재/조세포탈/대표자 제재 확인 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `rsreRat` | 비율(약어 미확정) | 후보 | ID·표 맥락 · 옛 일괄 제외 B2 |
| `g2b-contract-v1` | `pointInfo` | `sbrtSysUseYn` | 하도급 시스템 사용 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `schlMlsvFdmtYn` | 학교급식 식재료 여부 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `sftyInspYn` | 안전검사 여부 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `spdtGuidMatrIdntYn` | 인지세 안내사항 확인 여부 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `spdtTxtnTrgtYn` | 인지세 과세대상 여부 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `specNoVal` | 규격번호 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `stdPbadmsReqNodyNm` | 표준 행정 소요일수 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `swSprtnOderFeeDscntYn` | 소프트웨어 분리발주 수수료 할인 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `taxevYnIdntYn` | 부정당 제재/조세포탈/대표자 제재 확인 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `techEvlAgncYn` | 기술평가 대행 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `tlbkpngCtrtAmt` | 총부기 계약금액 후보 | 후보 | ID·표 맥락 · 옛 일괄 제외 B2 |
| `g2b-contract-v1` | `pointInfo` | `unptRsttHistIdntYn` | 부정당 제재/조세포탈/대표자 제재 확인 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `usrDocFtreCd` | 사용자 문서형태 코드 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `vatIncsYn` | 부가세 포함 여부 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `vatRat` | 부가세율 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `pointInfo` | `wholSharAmt` | 전체지분금액(전체 범위 확인) | 후보 | ID·표 맥락 · 옛 미사용 보류(Q06) |
| `g2b-contract-v1` | `pointInfo` | `wholSharRat` | 전체지분율(전체 범위 확인) | 후보 | ID·표 맥락 · 옛 미사용 보류(Q06) |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `addCgntTxam` | 추가 인지세액 후보 | 후보 | ID·표 맥락 · 옛 일괄 제외 A1 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `agncFee` | 대리점 수수료 후보 | 후보 | ID·표 맥락 · 옛 일괄 제외 B3 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `avtsmtBankAddr` | 통지은행 정보(현재 MVP 제외) | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `avtsmtBankNm` | 통지은행 정보(현재 MVP 제외) | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `avtsmtBankSfcdVal` | 통지은행 정보(현재 MVP 제외) | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `bzmnRegNo` | 사업자등록번호 | 수집 | 사용자 확인 2026-09-27(decisions §6 업체 식별·지분) |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `cgntTxam` | 인지세 부과액(물품제조계약) | 제외: 다른 출처 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · pointInfo 에서 확정 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `cncdIntpCd` | 공동이행 관련 코드 후보 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `ctentIntpNm` | 계약상대자 이행명 후보 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `ctentSeCd` | 계약상대자 구분 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `ctentUntyGrpNm` | 계약상대자명 | 수집 | 사용자 확인 2026-09-27(decisions §6 업체 식별·지분) |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `ctentUntyGrpNo` | 계약상대자 통합기관번호 | 수집 | 사용자 확인 2026-09-27(decisions §6 업체 식별·지분) |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `ctrtChgOrd` | 계약변경차수 | 제외: 다른 출처 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis 에서 확정 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `ctrtCrnyCd` | 계약/적용 통화코드 | 후보 | 빈 열만 확인 · 옛 일괄 제외 B3 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `ctrtGrnteAmt` | 계약 전체의 계약보증금액 | 제외: 다른 출처 | 사용자 확인 2026-10-15(같은 ID 원칙 — 뜻은 머리 칸의 계약보증금액과 같다) · 머리 칸(pointInfo)을 수집 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `ctrtNo` | 계약번호(본번호) | 제외: 다른 출처 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis 에서 확정 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `ctrtSharAmt` | 현재 계약차수의 업체 지분금액 | 수집 | 사용자 확인 2026-09-27(질문 Q06 부분 확정) |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `ctrtSharRat` | 현재 계약차수의 업체 지분율(%) | 수집 | 사용자 확인 2026-09-27(질문 Q06 부분 확정) |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `ctwtIsrNm` | 보증서 발급자명 후보 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `ctwtStokNoVal` | 보증 증권번호 후보 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `dolrCtrtAmt` | 달러 계약금액 | 후보 | ID·표 맥락 · 옛 일괄 제외 B3 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `drctEctnIdrpIdntYn` | 용어 미확정 — 약어만으로 번역하지 않음 | 후보 | 미해석 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `dtlCtknSeCd` | 세부 계약종류 구분 후보 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `dutyObsrIdrpIdntYn` | 용어 미확정 — 약어만으로 번역하지 않음 | 후보 | 미해석 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `eltsbAgreYn` | 전자제출 동의 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `ernnEtpsAddr` | 해외 관련업체 연락처/주소(정확한 역할 미확정) | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `ernnEtpsCtrtEctnGrnteAmt` | 관련 해외업체 계약이행보증금액(업체 역할 미확정) | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `ernnEtpsCtrtEctnGtnrt` | 관련 해외업체 계약이행보증금률(업체 역할 미확정) | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `ernnEtpsFaxNo` | 해외 관련업체 연락처/주소(정확한 역할 미확정) | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `ernnEtpsTlphNo` | 해외 관련업체 연락처/주소(정확한 역할 미확정) | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `ernnEtpsUntyGrpNo` | 해외 관련업체 통합기관번호(정확한 역할 미확정) | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `etpsAddr` | 업체 주소 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `etpsFaxNo` | 업체 팩스번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `etpsPicId` | 업체 담당자 ID/명 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `etpsPicNm` | 업체 담당자 ID/명 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `etpsTlphNo` | 업체 전화번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `frgtWbilSeCd` | 화물운송장 구분 | 후보 | 빈 열만 확인 · 옛 일괄 제외 B3 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `giveAcntBankCd` | 지급계좌 은행코드/계좌번호(수집 제외) | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `giveActno` | 지급계좌 은행코드/계좌번호(수집 제외) | 제외: 개인정보 | ADR-043 개인정보(계좌번호) |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `giveMmrdIdntYn` | 지급각서 확인 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `idrpIdntYn` | 용어 미확정 — 약어만으로 번역하지 않음 | 후보 | 미해석 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `incmCgntAddPchaAmt` | 수입인지 추가 구매금액 후보 | 후보 | ID·표 맥락 · 옛 일괄 제외 A1 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `incmCgntPchaAmt` | 수입인지 구매금액 후보 | 후보 | ID·표 맥락 · 옛 제외(Q05 미사용) |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `inptDt` | 등록/변경/입력일시(업무일자와 구별) | 후보 | 구조·이름 분류 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `itcemUntyAtchFileNo` | 문서유형별 통합 첨부번호(문서 약어 확인) | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `itgtCtratIdntYn` | 용어 미확정 — 약어만으로 번역하지 않음 | 후보 | 미해석 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `jintShreSeCd` | 공동 분담구분 후보 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `jintSyddCmnMthoCd` | 공동수급 구성방식 코드 | 제외: 다른 출처 | 원문 명칭 jintSyddCmnMthoNm 을 수집(답변 3 명칭 우선) |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `jintSyddCmnMthoNm` | 공동수급 구성방식 명칭(공500004 → 단독계약) | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `jintSyddRsnCd` | 공동수급 사유코드 후보 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `kbrdrId` | 시스템 등록/변경/입력자(수집 제외) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `ltcrEstblMthdCd` | 신용장 개설방법 | 후보 | 빈 열만 확인 · 옛 일괄 제외 B3 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `ltcrVldYmd` | 신용장 유효일자 | 후보 | 빈 열만 확인 · 옛 일괄 제외 B3 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `nextRowYn` | 다음 행 여부(전체 수집 증거로 사용 금지) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `nflcNm` | 용어 미확정 — 약어만으로 번역하지 않음 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `ocfmEdocNo` | 문서 전자문서번호(문서 약어 확인) | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `ocfmUntyAtchFileNo` | 문서유형별 통합 첨부번호(문서 약어 확인) | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `outNum` | 표시/출력/내부 순번(업무 키로 사용 금지) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `prchsRsltnNo` | 구매결의번호 후보 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `prttnShpgPrmsYn` | 분할선적 허용 여부 | 후보 | ID·표 맥락 · 옛 일괄 제외 B3 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `ptcpIntpCd` | 참여이행 관련 코드 후보 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `ptnrIntpCd` | 분담이행 관련 코드 후보 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `pvctCntltUntyAtchFileNo` | 문서유형별 통합 첨부번호(문서 약어 확인) | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `pvctMmrdUntyAtchFileNo` | 문서유형별 통합 첨부번호(문서 약어 확인) | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `rcryCtrtAmt` | 원화 계약금액 후보 | 후보 | ID·표 맥락 · 옛 일괄 제외 B3 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `rowNum` | 표시/출력/내부 순번(업무 키로 사용 금지) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `rprsvNm` | 대표자명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-contract-change-v1 의 답) |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `rspdntId` | 응답자 ID | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `rspnsDt` | 응답일시 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `rspnsTyCd` | 응답유형 코드 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `sgctClclnMthdCd` | 대금 정산방법(약어 미확정) | 후보 | 빈 열만 확인 · 옛 일괄 제외 B3 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `shpgAphrCd` | 선적 항구 코드(차수 범위 확인) | 후보 | 빈 열만 확인 · 옛 일괄 제외 B3 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `shpgPrnmntNody` | 선적 예정일수 | 후보 | ID·표 맥락 · 옛 일괄 제외 B3 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `shpgPrnmntYmd` | 선적 예정일 | 후보 | 빈 열만 확인 · 옛 일괄 제외 B3 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `shpgTrnpMthdCd` | 선적 운송방법 | 후보 | 빈 열만 확인 · 옛 일괄 제외 B3 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `spplEtpsCtrtEctnGrnteAmt` | 공급업체 계약이행보증금액 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `spplEtpsCtrtEctnGtnrt` | 공급업체 계약이행보증금률 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `trmtPsbltyYn` | 환적 가능 여부 후보 | 후보 | ID·표 맥락 · 옛 일괄 제외 B3 |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `vatIncsYn` | 부가세 포함 여부 | 후보 | ID·표 맥락 · 옛 제외(Q05 미사용) |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `vatRat` | 부가세율 | 후보 | ID·표 맥락 · 옛 제외(Q05 미사용) |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `wholSharAmt` | 전체지분금액(전체 범위 확인) | 후보 | ID·표 맥락 · 옛 미사용 보류(Q06) |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `wholSharRat` | 전체지분율(전체 범위 확인) | 후보 | ID·표 맥락 · 옛 미사용 보류(Q06) |
| `g2b-contract-v1` | `mf_wfm_container_grdEtpsLst` | `wtotIdntYn` | 용어 미확정 — 약어만으로 번역하지 않음 | 후보 | 미해석 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `advlTxam` | 종가세액 후보 | 후보 | ID·표 맥락 · 옛 일괄 제외 B4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `aplcnCrnyCd` | 계약/적용 통화코드 | 후보 | 빈 열만 확인 · 옛 일괄 제외 B4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `arvlAphrCd` | 도착 항구코드 | 후보 | 빈 열만 확인 · 옛 일괄 제외 B4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `asetUntCd` | 자산단위코드 | 후보 | 빈 열만 확인 · 옛 일괄 제외 A7 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `asntCoreCpntNm` | 조립국가 대상 주요/핵심부품명 | 후보 | 빈 열만 확인 · 옛 일괄 제외 B5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `asntNtnCd` | 조립국가 코드/명 후보 | 후보 | 빈 열만 확인 · 옛 일괄 제외 B5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `asntNtnNm` | 조립국가 코드/명 후보 | 후보 | 빈 열만 확인 · 옛 일괄 제외 B5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `asntOdn1StpeCpntNm` | 조립국가 대상 주요/핵심부품명 | 후보 | 빈 열만 확인 · 옛 일괄 제외 B5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `asntOdn2StpeCpntNm` | 조립국가 대상 주요/핵심부품명 | 후보 | 빈 열만 확인 · 옛 일괄 제외 B5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `authrtBgngYmd` | 권리 시작/종료일자 | 후보 | 빈 열만 확인 · 옛 일괄 제외 A5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `authrtEndYmd` | 권리 시작/종료일자 | 후보 | 빈 열만 확인 · 옛 일괄 제외 A5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `bgngNvnoVal` | 시작/종료 번호(약어 미확정) | 후보 | 빈 열만 확인 · 옛 일괄 제외 A5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `bgtImplSeCd` | 예산 집행구분 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `bidClsfNo` | 입찰분류번호 | 수집 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List 에서 확정 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `bidPbancItemSqno` | 입찰공고 품목순번 | 수집 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List 에서 확정 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `bsamCmptProdYn` | 중소기업간 경쟁제품 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `clndNoVal` | 지번 후보 | 후보 | 빈 열만 확인 · 옛 일괄 제외 A4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `cmptncBrcoOrgnCd` | 관할 지청·조직코드 후보 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `cmusSwItemSeCd` | 상용 소프트웨어 품목구분 후보 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `coreCpntAsntNtnCd` | 핵심부품 조립국가 코드/명 | 후보 | 빈 열만 확인 · 옛 일괄 제외 B5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `coreCpntAsntNtnNm` | 핵심부품 조립국가 코드/명 | 후보 | 빈 열만 확인 · 옛 일괄 제외 B5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `coreCpntPlorNtnCd` | 핵심부품 원산지 코드/명 | 후보 | 빈 열만 확인 · 옛 일괄 제외 B5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `coreCpntPlorNtnNm` | 핵심부품 원산지 코드/명 | 후보 | 빈 열만 확인 · 옛 일괄 제외 B5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `ctentUntyGrpNo` | 계약상대자 통합기관번호 | 제외: 다른 출처 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_grdEtpsLst 에서 확정 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `ctrtAmt` | 품목별 계약금액 | 수집 | 사용자 확인 2026-09-27(질문 Q02 부분 확정) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `ctrtChgOrd` | 계약변경차수 | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `ctrtCndtCn` | 계약조건 내용 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `ctrtCrnyCd` | 계약/적용 통화코드 | 후보 | 빈 열만 확인 · 옛 일괄 제외 B4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `ctrtDmndRcptNo` | 계약요청 접수번호 | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `ctrtDmndRcptOrd` | 계약요청 접수차수 | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `ctrtGrnteAmt` | 계약 전체의 계약보증금액 | 제외: 다른 출처 | 사용자 확인 2026-10-15(같은 ID 원칙 — 뜻은 머리 칸의 계약보증금액과 같다) · 머리 칸(pointInfo)을 수집 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `ctrtGtnrt` | 계약보증금률 | 제외: 다른 출처 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · pointInfo 에서 확정 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `ctrtItemNm` | 계약 품목명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `ctrtItemSqnm` | 계약 품목순번 표시명 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `ctrtItemSqno` | 계약 품목순번 | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `ctrtNo` | 계약번호(본번호) | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `ctrtPlanYear` | 국방 조달계획연도 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `ctrtQty` | 품목별 계약수량 | 수집 | 사용자 확인 2026-09-27(일괄 답변 2 / R02) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `ctrtSqms` | 계약 면적 후보 | 후보 | ID·표 맥락 · 옛 일괄 제외 A4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `ctrtUntCd` | 계약 품목단위 값/코드 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `ctrtUntVal` | 계약 품목단위 값/코드 | 수집 | 사용자 확인 2026-09-27(일괄 답변 6 / R07) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `ctrtUprc` | 품목별 계약단가 | 수집 | 사용자 확인 2026-09-27(일괄 답변 2 / R02) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `ddcCtcrAddr` | 공제 계약자 개인정보(물품 MVP 제외) | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `ddcCtcrNm` | 공제 계약자 개인정보(물품 MVP 제외) | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `ddcCtcrPrsnNo` | 주민번호 성격 식별자(수집 제외) | 제외: 개인정보 | ADR-043 개인정보(주민등록번호 성격) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `ddcEvlAmt` | 공제 평가금액 후보 | 후보 | ID·표 맥락 · 옛 일괄 제외 A6 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `depsInstNm` | 예치기관명 | 후보 | 빈 열만 확인 · 옛 일괄 제외 A5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `devyCndtCd` | 인도조건 | 수집 | 사용자 확인 2026-09-27(일괄 답변 6 / R07) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `devyCndtNm` | 인도조건 | 제외: 다른 출처 | 사용자 확인 2026-09-27(일괄 답변 6 / R07) — 같은 뜻 devyCndtCd 를 수집 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `dlvgdsPlacNm` | 납품장소 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `dlvgdsSqno` | 납품순번 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `dlvgdsTermChcSeCd` | 납품기한 선택구분 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `dlvgdsTermMtnb` | 납품 개월수 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `dlvgdsTermNody` | 납품일수 | 수집 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridView 에서 확정 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `dlvgdsTermYmd` | 납품기한 날짜 | 수집 | 사용자 확인 2026-09-27(일괄 답변 5 / R06) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `dmstRvwRsltCd` | 수요기관/조달청 검토결과 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `dmstUntyGrpNm` | 수요기관명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `dmstUntyGrpNo` | 수요기관 통합기관번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `dmstcCmmyYn` | 국내 상품 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `dolrCtrtAmt` | 달러 계약금액 | 후보 | ID·표 맥락 · 옛 일괄 제외 B4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `dolrCtrtUprc` | 달러 계약단가 | 후보 | ID·표 맥락 · 옛 일괄 제외 B4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `endNvnoVal` | 시작/종료 번호(약어 미확정) | 후보 | 빈 열만 확인 · 옛 일괄 제외 A5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `engItemNm` | 영문 품목명 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `engItemSpecVal` | 영문 품목규격 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `eusrNm` | 최종사용자명 후보 | 후보 | 빈 열만 확인 · 옛 일괄 제외 B4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `evrtLaseChrg` | 회차별 리스료 후보 | 후보 | ID·표 맥락 · 옛 일괄 제외 A3 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `excnItrt` | 실행 이자율 | 후보 | ID·표 맥락 · 옛 일괄 제외 A3 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `extlPbfnAsetDmndNoVal` | 외부 재정 자산요청번호 후보 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `extlPbfnAsetDmndSqnoVal` | 외부 재정 자산요청 순번 후보 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `extlPbfnBizId` | 외부 재정 사업 ID 후보 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `frcpCtrtGrnteChrg` | 외자계약 보증수수료 | 후보 | ID·표 맥락 · 옛 일괄 제외 B4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `frcpCtrtItcg` | 외자계약 부대비용 | 후보 | ID·표 맥락 · 옛 일괄 제외 B4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `frcpCtrtPrem` | 외자계약 보험료 후보 | 후보 | ID·표 맥락 · 옛 일괄 제외 B4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `frcpCtrtSptdCt` | 외자계약 기술지원비 후보 | 후보 | ID·표 맥락 · 옛 일괄 제외 B4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `frcpCtrtTrngCt` | 외자계약 교육훈련비 후보 | 후보 | ID·표 맥락 · 옛 일괄 제외 B4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `frcpItemDevyCndtCd` | 외자물품 인도조건 | 후보 | 빈 열만 확인 · 옛 일괄 제외 B4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `g2bYn` | 나라장터 관련 여부(정확한 용도 미확정) | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `gvmnItemClsfNo` | 정부 물품분류번호 후보 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `hefcEtamYn` | 고효율 기자재 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `hghrkInstCd` | 상위기관 코드 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `hskCd` | HSK 품목분류코드 | 후보 | 빈 열만 확인 · 옛 일괄 제외 B4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `inptDt` | 등록/변경/입력일시(업무일자와 구별) | 후보 | 구조·이름 분류 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `inscCtrtDdcRat` | 보험계약 공제율 후보 | 후보 | ID·표 맥락 · 옛 일괄 제외 A6 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `inscStokNoVal` | 보험 증권번호 | 후보 | 빈 열만 확인 · 옛 일괄 제외 A6 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `inscStokVldBgngYmd` | 보험증권 유효 시작/종료일자 | 후보 | 빈 열만 확인 · 옛 일괄 제외 A6 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `inscStokVldEndYmd` | 보험증권 유효 시작/종료일자 | 후보 | 빈 열만 확인 · 옛 일괄 제외 A6 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `insdAddr` | 피보험자 개인정보(물품 MVP 제외) | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `insdNm` | 피보험자 개인정보(물품 MVP 제외) | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `insdPrsnNo` | 주민번호 성격 식별자(수집 제외) | 제외: 개인정보 | ADR-043 개인정보(주민등록번호 성격) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `inspInstUntyGrpNm` | 검사기관 번호/명 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `inspInstUntyGrpNo` | 검사기관 번호/명 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `inspNody` | 검사일수 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `inspTyCd` | 검사형태 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `isChecked` | 행 선택 상태 | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `itasRegInstNm` | 무형자산 등록번호/명/기관/일자 후보 | 후보 | 빈 열만 확인 · 옛 일괄 제외 A5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `itasRegNm` | 무형자산 등록번호/명/기관/일자 후보 | 후보 | 빈 열만 확인 · 옛 일괄 제외 A5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `itasRegNoVal` | 무형자산 등록번호/명/기관/일자 후보 | 후보 | 빈 열만 확인 · 옛 일괄 제외 A5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `itasRegYmd` | 무형자산 등록번호/명/기관/일자 후보 | 후보 | 빈 열만 확인 · 옛 일괄 제외 A5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `itemClsfNo` | 물품분류번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `itemDelRsn` | 품목 삭제사유 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `itemDtlClsfNo` | 물품 세부분류번호 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `itemFee` | 품목 수수료 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `itemIdnfNo` | 물품식별번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `itemIvtrNoVal` | 재고번호 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `itemSpecVal` | 품목 상세규격 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `itemWght` | 물품중량 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `itrlPrcmItemIdnfNmVal` | 내부 조달 물품식별명 후보 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `itrlPrcmItemIdnfNoVal` | 내부 조달 물품식별번호 후보 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `itrrTrnpItcg` | 내륙 운송 부대비용 후보 | 후보 | ID·표 맥락 · 옛 일괄 제외 B4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `jdgtEvlAmt` | 감정평가금액 후보 | 후보 | 빈 열만 확인 · 옛 일괄 제외 A4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `kbrdrId` | 시스템 등록/변경/입력자(수집 제외) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `kimgrpYn` | 특정 제품군 여부/포함 여부(약어 미확정) | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `laseCtrtNo` | 리스 계약번호 | 후보 | 빈 열만 확인 · 옛 일괄 제외 A3 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `laseExcnYmd` | 리스 실행일자 | 후보 | 빈 열만 확인 · 옛 일괄 제외 A3 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `laseFee` | 리스 수수료 | 후보 | ID·표 맥락 · 옛 일괄 제외 A3 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `lctnNm` | 소재지명 | 후보 | 빈 열만 확인 · 옛 일괄 제외 A4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `ldcgCd` | 지목코드 후보 | 후보 | 빈 열만 확인 · 옛 일괄 제외 A4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `lgqtDlgdiExclYn` | 장기 납품 관련 제외 여부(약어 미확정) | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `mmLeseChrg` | 월 임대료 | 후보 | ID·표 맥락 · 옛 일괄 제외 A4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `mmMgcs` | 월 관리비 | 후보 | ID·표 맥락 · 옛 일괄 제외 A4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `modlNm` | 모델명 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `mrstDepsNoVal` | 유가증권 예치번호 후보 | 후보 | 빈 열만 확인 · 옛 일괄 제외 A5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `mrstFamtAmt` | 유가증권 액면금액 후보 | 후보 | ID·표 맥락 · 옛 일괄 제외 A5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `mrstFamtIrrt` | 유가증권 액면이율 후보 | 후보 | ID·표 맥락 · 옛 일괄 제외 A5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `mrstMtryYmd` | 유가증권 발행/만기일 후보 | 후보 | 빈 열만 확인 · 옛 일괄 제외 A5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `mrstPblcnYmd` | 유가증권 발행/만기일 후보 | 후보 | 빈 열만 확인 · 옛 일괄 제외 A5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `mrstSharRat` | 유가증권 지분율 후보 | 후보 | ID·표 맥락 · 옛 일괄 제외 A5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `ndfsPrcmDmndNo` | 국방 조달요구번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토, 같은 ID g2b-request-v1 의 답) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `ndpfExpndYn` | 국방 지출 여부 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `nextRowYn` | 다음 행 여부(전체 수집 증거로 사용 금지) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `odn1PlorNtnCd` | 원산지 국가코드 | 제외: 다른 출처 | 원문 명칭 odn1PlorNtnNm 을 수집(답변 3 명칭 우선) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `odn1PlorNtnNm` | 원산지 국가명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `odn1ShpgAphrCd` | 선적 항구 코드(차수 범위 확인) | 후보 | 빈 열만 확인 · 옛 일괄 제외 B4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `odn1StpeCpntAsntNtnCd` | 주요부품 조립국가 코드/명 후보 | 후보 | 빈 열만 확인 · 옛 일괄 제외 B5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `odn1StpeCpntAsntNtnNm` | 주요부품 조립국가 코드/명 후보 | 후보 | 빈 열만 확인 · 옛 일괄 제외 B5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `odn1StpeCpntPlorNtnCd` | 주요부품 원산지 코드/명 후보 | 후보 | 빈 열만 확인 · 옛 일괄 제외 B5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `odn1StpeCpntPlorNtnNm` | 주요부품 원산지 코드/명 후보 | 후보 | 빈 열만 확인 · 옛 일괄 제외 B5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `odn2PlorNtnCd` | 원산지 국가(차수/순번 의미 확인) | 후보 | 빈 열만 확인 · 옛 일괄 제외 B5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `odn2ShpgAphrCd` | 선적 항구 코드(차수 범위 확인) | 후보 | 빈 열만 확인 · 옛 일괄 제외 B4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `odn2StpeCpntAsntNtnCd` | 주요부품 조립국가 코드/명 후보 | 후보 | 빈 열만 확인 · 옛 일괄 제외 B5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `odn2StpeCpntAsntNtnNm` | 주요부품 조립국가 코드/명 후보 | 후보 | 빈 열만 확인 · 옛 일괄 제외 B5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `odn2StpeCpntPlorNtnCd` | 주요부품 원산지 코드/명 후보 | 후보 | 빈 열만 확인 · 옛 일괄 제외 B5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `odn2StpeCpntPlorNtnNm` | 주요부품 원산지 코드/명 후보 | 후보 | 빈 열만 확인 · 옛 일괄 제외 B5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `odn3PlorNtnCd` | 원산지 국가(차수/순번 의미 확인) | 후보 | 빈 열만 확인 · 옛 일괄 제외 B5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `odn3ShpgAphrCd` | 선적 항구 코드(차수 범위 확인) | 후보 | 빈 열만 확인 · 옛 일괄 제외 B4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `orgpAntpSeCd` | 원본/사본 구분 후보 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `outNum` | 표시/출력/내부 순번(업무 키로 사용 금지) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `ovrsFrgh` | 해외 운임 | 후보 | ID·표 맥락 · 옛 일괄 제외 B4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `ownrAddr` | 소유자 개인정보(물품 MVP 제외) | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `ownrNm` | 소유자 개인정보(물품 MVP 제외) | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `ownrPrsnNo` | 주민번호 성격 식별자(수집 제외) | 제외: 개인정보 | ADR-043 개인정보(주민등록번호 성격) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `ownrTlphNo` | 소유자 개인정보(물품 MVP 제외) | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `pblcnInstNm` | 발행기관명 | 후보 | 빈 열만 확인 · 옛 일괄 제외 A5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `pblcnNotm` | 발행회차 후보 | 후보 | ID·표 맥락 · 옛 일괄 제외 A5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `plorCoreCpntNm` | 원산지 대상 주요/핵심부품명 | 후보 | 빈 열만 확인 · 옛 일괄 제외 B5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `plorOdn1StpeCpntNm` | 원산지 대상 주요/핵심부품명 | 후보 | 빈 열만 확인 · 옛 일괄 제외 B5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `plorOdn2StpeCpntNm` | 원산지 대상 주요/핵심부품명 | 후보 | 빈 열만 확인 · 옛 일괄 제외 B5 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `ppsRvwRsltCd` | 수요기관/조달청 검토결과 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `prchsRsltnSqno` | 구매결의순번 후보 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `prptDtlSeCd` | 재산 구분/세부구분/항목 코드 | 후보 | 빈 열만 확인 · 옛 일괄 제외 A7 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `prptItmsCd` | 재산 구분/세부구분/항목 코드 | 후보 | 빈 열만 확인 · 옛 일괄 제외 A7 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `prptSeCd` | 재산 구분/세부구분/항목 코드 | 후보 | 빈 열만 확인 · 옛 일괄 제외 A7 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `prttnShpgPrmsYn` | 분할선적 허용 여부 | 후보 | ID·표 맥락 · 옛 일괄 제외 B4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `prumAmt` | 보험료 금액 후보 | 후보 | ID·표 맥락 · 옛 일괄 제외 B4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `rcryCtrtAmt` | 원화 계약금액 후보 | 후보 | ID·표 맥락 · 옛 일괄 제외 B4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `rcryExrt` | 원화 환율 | 후보 | ID·표 맥락 · 옛 일괄 제외 B4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `rcrySpamAmt` | 물품대금(원화/원통화 범위 확인) | 후보 | ID·표 맥락 · 옛 일괄 제외 B4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `rcryTlgrRdmpExrt` | 원화 전신환 매도율 후보 | 후보 | ID·표 맥락 · 옛 일괄 제외 B4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `rgInstNm` | 등기기관명 후보 | 후보 | 빈 열만 확인 · 옛 일괄 제외 A4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `rgYmd` | 등기일자 후보 | 후보 | 빈 열만 확인 · 옛 일괄 제외 A4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `rgnLgdngCd` | 지역 법정동 코드 | 후보 | 빈 열만 확인 · 옛 일괄 제외 A4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `rgtbNoVal` | 권리증 번호 후보 | 후보 | 빈 열만 확인 · 옛 일괄 제외 A4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `rmrkCn` | 비고 내용 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `rowNum` | 표시/출력/내부 순번(업무 키로 사용 금지) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `shpgTermMtnb` | 선적기한 개월수 | 후보 | ID·표 맥락 · 옛 일괄 제외 B4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `shpgTermYmd` | 선적기한 일자 | 후보 | 빈 열만 확인 · 옛 일괄 제외 B4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `shpgTrnpMthdCd` | 선적 운송방법 | 후보 | 빈 열만 확인 · 옛 일괄 제외 B4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `spamAmt` | 물품대금(원화/원통화 범위 확인) | 후보 | ID·표 맥락 · 옛 일괄 제외 B4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `specAtrbCn` | 규격 속성명/내용 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `specKndCd` | 규격종류 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `specKndNm` | 규격종류 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `specNoVal` | 규격번호 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `spplEtpsBzmnRegNo` | 공급업체 사업자등록번호 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `spplEtpsNm` | 공급업체명 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `spplEtpsUntyGrpNo` | 공급업체 통합기관번호 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `spplRgnDtlsRgnNm` | 공급지역 번호/명/상세명 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `spplRgnNm` | 공급지역 번호/명/상세명 | 후보 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `spplRgnNo` | 공급지역 번호/명/상세명 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `tdolrCtrtAmt` | 전체 달러 계약금액 후보 | 후보 | ID·표 맥락 · 옛 일괄 제외 B4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `totlLaseChrg` | 총 리스료 | 후보 | ID·표 맥락 · 옛 일괄 제외 A3 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `trcrnyCtrtAmt` | 전체 통화 계약금액 후보 | 후보 | ID·표 맥락 · 옛 일괄 제외 B4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `trffAmt` | 관세액 | 후보 | ID·표 맥락 · 옛 일괄 제외 B4 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `uprcClcBassCn` | 단가 산출근거 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` | `usagNm` | 용도명 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `advlTxam` | 종가세액 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `aplcnCrnyCd` | 계약/적용 통화코드 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `arvlAphrCd` | 도착 항구코드 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `asetUntCd` | 자산단위코드 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `asntCoreCpntNm` | 조립국가 대상 주요/핵심부품명 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `asntNtnCd` | 조립국가 코드/명 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `asntNtnNm` | 조립국가 코드/명 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `asntOdn1StpeCpntNm` | 조립국가 대상 주요/핵심부품명 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `asntOdn2StpeCpntNm` | 조립국가 대상 주요/핵심부품명 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `authrtBgngYmd` | 권리 시작/종료일자 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `authrtEndYmd` | 권리 시작/종료일자 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `bgngNvnoVal` | 시작/종료 번호(약어 미확정) | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `bgtImplSeCd` | 예산 집행구분 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `bidClsfNo` | 입찰분류번호 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `bidPbancItemSqno` | 입찰공고 품목순번 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `bsamCmptProdYn` | 중소기업간 경쟁제품 여부 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `clndNoVal` | 지번 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `cmptncBrcoOrgnCd` | 관할 지청·조직코드 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `cmusSwItemSeCd` | 상용 소프트웨어 품목구분 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `coreCpntAsntNtnCd` | 핵심부품 조립국가 코드/명 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `coreCpntAsntNtnNm` | 핵심부품 조립국가 코드/명 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `coreCpntPlorNtnCd` | 핵심부품 원산지 코드/명 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `coreCpntPlorNtnNm` | 핵심부품 원산지 코드/명 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `ctentUntyGrpNo` | 계약상대자 통합기관번호 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `ctrtAmt` | 품목별 계약금액 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `ctrtChgOrd` | 계약변경차수 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `ctrtCndtCn` | 계약조건 내용 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `ctrtCrnyCd` | 계약/적용 통화코드 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `ctrtDmndRcptNo` | 계약요청 접수번호 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `ctrtDmndRcptOrd` | 계약요청 접수차수 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `ctrtGrnteAmt` | 계약보증금액 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `ctrtGtnrt` | 계약보증금률 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `ctrtItemNm` | 계약 품목명 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `ctrtItemSqnm` | 계약 품목순번 표시명 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `ctrtItemSqno` | 계약 품목순번 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `ctrtNo` | 계약번호(본번호) | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `ctrtPlanYear` | 국방 조달계획연도 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `ctrtQty` | 품목별 계약수량 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `ctrtSqms` | 계약 면적 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `ctrtUntCd` | 계약 품목단위 값/코드 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `ctrtUntVal` | 계약 품목단위 값/코드 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `ctrtUprc` | 품목별 계약단가 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `ddcCtcrAddr` | 공제 계약자 개인정보(물품 MVP 제외) | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `ddcCtcrNm` | 공제 계약자 개인정보(물품 MVP 제외) | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `ddcCtcrPrsnNo` | 주민번호 성격 식별자(수집 제외) | 제외: 개인정보 | ADR-043 개인정보(주민등록번호 성격) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `ddcEvlAmt` | 공제 평가금액 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `depsInstNm` | 예치기관명 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `devyCndtCd` | 인도조건 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `devyCndtNm` | 인도조건 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `dlvgdsPlacNm` | 납품장소 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `dlvgdsSqno` | 납품순번 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `dlvgdsTermChcSeCd` | 납품기한 선택구분 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `dlvgdsTermMtnb` | 납품 개월수 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `dlvgdsTermNody` | 납품일수 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `dlvgdsTermYmd` | 납품기한 날짜 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `dmstRvwRsltCd` | 수요기관/조달청 검토결과 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `dmstUntyGrpNm` | 수요기관명 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `dmstUntyGrpNo` | 수요기관 통합기관번호 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `dmstcCmmyYn` | 국내 상품 여부 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `dolrCtrtAmt` | 달러 계약금액 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `dolrCtrtUprc` | 달러 계약단가 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `endNvnoVal` | 시작/종료 번호(약어 미확정) | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `engItemNm` | 영문 품목명 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `engItemSpecVal` | 영문 품목규격 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `eusrNm` | 최종사용자명 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `evrtLaseChrg` | 회차별 리스료 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `excnItrt` | 실행 이자율 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `extlPbfnAsetDmndNoVal` | 외부 재정 자산요청번호 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `extlPbfnAsetDmndSqnoVal` | 외부 재정 자산요청 순번 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `extlPbfnBizId` | 외부 재정 사업 ID 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `frcpCtrtGrnteChrg` | 외자계약 보증수수료 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `frcpCtrtItcg` | 외자계약 부대비용 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `frcpCtrtPrem` | 외자계약 보험료 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `frcpCtrtSptdCt` | 외자계약 기술지원비 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `frcpCtrtTrngCt` | 외자계약 교육훈련비 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `frcpItemDevyCndtCd` | 외자물품 인도조건 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `g2bYn` | 나라장터 관련 여부(정확한 용도 미확정) | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `gvmnItemClsfNo` | 정부 물품분류번호 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `hefcEtamYn` | 고효율 기자재 여부 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `hghrkInstCd` | 상위기관 코드 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `hskCd` | HSK 품목분류코드 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `inptDt` | 등록/변경/입력일시(업무일자와 구별) | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `inscCtrtDdcRat` | 보험계약 공제율 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `inscStokNoVal` | 보험 증권번호 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `inscStokVldBgngYmd` | 보험증권 유효 시작/종료일자 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `inscStokVldEndYmd` | 보험증권 유효 시작/종료일자 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `insdAddr` | 피보험자 개인정보(물품 MVP 제외) | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `insdNm` | 피보험자 개인정보(물품 MVP 제외) | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `insdPrsnNo` | 주민번호 성격 식별자(수집 제외) | 제외: 개인정보 | ADR-043 개인정보(주민등록번호 성격) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `inspInstUntyGrpNm` | 검사기관 번호/명 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `inspInstUntyGrpNo` | 검사기관 번호/명 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `inspNody` | 검사일수 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `inspTyCd` | 검사형태 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `isChecked` | 행 선택 상태 | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `itasRegInstNm` | 무형자산 등록번호/명/기관/일자 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `itasRegNm` | 무형자산 등록번호/명/기관/일자 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `itasRegNoVal` | 무형자산 등록번호/명/기관/일자 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `itasRegYmd` | 무형자산 등록번호/명/기관/일자 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `itemClsfNo` | 물품분류번호 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `itemDelRsn` | 품목 삭제사유 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `itemDtlClsfNo` | 물품 세부분류번호 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `itemFee` | 품목 수수료 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `itemIdnfNo` | 물품식별번호 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `itemIvtrNoVal` | 재고번호 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `itemSpecVal` | 품목 상세규격 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `itemWght` | 물품중량 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `itrlPrcmItemIdnfNmVal` | 내부 조달 물품식별명 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `itrlPrcmItemIdnfNoVal` | 내부 조달 물품식별번호 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `itrrTrnpItcg` | 내륙 운송 부대비용 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `jdgtEvlAmt` | 감정평가금액 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `kbrdrId` | 시스템 등록/변경/입력자(수집 제외) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `kimgrpYn` | 특정 제품군 여부/포함 여부(약어 미확정) | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `laseCtrtNo` | 리스 계약번호 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `laseExcnYmd` | 리스 실행일자 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `laseFee` | 리스 수수료 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `lctnNm` | 소재지명 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `ldcgCd` | 지목코드 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `lgqtDlgdiExclYn` | 장기 납품 관련 제외 여부(약어 미확정) | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `mmLeseChrg` | 월 임대료 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `mmMgcs` | 월 관리비 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `modlNm` | 모델명 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `mrstDepsNoVal` | 유가증권 예치번호 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `mrstFamtAmt` | 유가증권 액면금액 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `mrstFamtIrrt` | 유가증권 액면이율 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `mrstMtryYmd` | 유가증권 발행/만기일 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `mrstPblcnYmd` | 유가증권 발행/만기일 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `mrstSharRat` | 유가증권 지분율 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `ndfsPrcmDmndNo` | 국방 조달요구번호 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `ndpfExpndYn` | 국방 지출 여부 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `nextRowYn` | 다음 행 여부(전체 수집 증거로 사용 금지) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `odn1PlorNtnCd` | 원산지 국가(차수/순번 의미 확인) | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `odn1PlorNtnNm` | 원산지 국가(차수/순번 의미 확인) | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `odn1ShpgAphrCd` | 선적 항구 코드(차수 범위 확인) | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `odn1StpeCpntAsntNtnCd` | 주요부품 조립국가 코드/명 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `odn1StpeCpntAsntNtnNm` | 주요부품 조립국가 코드/명 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `odn1StpeCpntPlorNtnCd` | 주요부품 원산지 코드/명 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `odn1StpeCpntPlorNtnNm` | 주요부품 원산지 코드/명 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `odn2PlorNtnCd` | 원산지 국가(차수/순번 의미 확인) | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `odn2ShpgAphrCd` | 선적 항구 코드(차수 범위 확인) | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `odn2StpeCpntAsntNtnCd` | 주요부품 조립국가 코드/명 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `odn2StpeCpntAsntNtnNm` | 주요부품 조립국가 코드/명 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `odn2StpeCpntPlorNtnCd` | 주요부품 원산지 코드/명 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `odn2StpeCpntPlorNtnNm` | 주요부품 원산지 코드/명 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `odn3PlorNtnCd` | 원산지 국가(차수/순번 의미 확인) | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `odn3ShpgAphrCd` | 선적 항구 코드(차수 범위 확인) | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `orgpAntpSeCd` | 원본/사본 구분 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `outNum` | 표시/출력/내부 순번(업무 키로 사용 금지) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `ovrsFrgh` | 해외 운임 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `ownrAddr` | 소유자 개인정보(물품 MVP 제외) | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `ownrNm` | 소유자 개인정보(물품 MVP 제외) | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `ownrPrsnNo` | 주민번호 성격 식별자(수집 제외) | 제외: 개인정보 | ADR-043 개인정보(주민등록번호 성격) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `ownrTlphNo` | 소유자 개인정보(물품 MVP 제외) | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `pblcnInstNm` | 발행기관명 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `pblcnNotm` | 발행회차 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `plorCoreCpntNm` | 원산지 대상 주요/핵심부품명 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `plorOdn1StpeCpntNm` | 원산지 대상 주요/핵심부품명 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `plorOdn2StpeCpntNm` | 원산지 대상 주요/핵심부품명 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `ppsRvwRsltCd` | 수요기관/조달청 검토결과 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `prchsRsltnSqno` | 구매결의순번 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `prptDtlSeCd` | 재산 구분/세부구분/항목 코드 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `prptItmsCd` | 재산 구분/세부구분/항목 코드 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `prptSeCd` | 재산 구분/세부구분/항목 코드 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `prttnShpgPrmsYn` | 분할선적 허용 여부 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `prumAmt` | 보험료 금액 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `rcryCtrtAmt` | 원화 계약금액 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `rcryExrt` | 원화 환율 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `rcrySpamAmt` | 물품대금(원화/원통화 범위 확인) | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `rcryTlgrRdmpExrt` | 원화 전신환 매도율 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `rgInstNm` | 등기기관명 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `rgYmd` | 등기일자 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `rgnLgdngCd` | 지역 법정동 코드 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `rgtbNoVal` | 권리증 번호 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `rmrkCn` | 비고 내용 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `rowNum` | 표시/출력/내부 순번(업무 키로 사용 금지) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `shpgTermMtnb` | 선적기한 개월수 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `shpgTermYmd` | 선적기한 일자 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `shpgTrnpMthdCd` | 선적 운송방법 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `spamAmt` | 물품대금(원화/원통화 범위 확인) | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `specAtrbCn` | 규격 속성명/내용 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `specKndCd` | 규격종류 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `specKndNm` | 규격종류 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `specNoVal` | 규격번호 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `spplEtpsBzmnRegNo` | 공급업체 사업자등록번호 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `spplEtpsNm` | 공급업체명 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `spplEtpsUntyGrpNo` | 공급업체 통합기관번호 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `spplRgnDtlsRgnNm` | 공급지역 번호/명/상세명 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `spplRgnNm` | 공급지역 번호/명/상세명 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `spplRgnNo` | 공급지역 번호/명/상세명 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `tdolrCtrtAmt` | 전체 달러 계약금액 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `totlLaseChrg` | 총 리스료 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `trcrnyCtrtAmt` | 전체 통화 계약금액 후보 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `trffAmt` | 관세액 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `uprcClcBassCn` | 단가 산출근거 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLisExcel` | `usagNm` | 용도명 | 제외: 다른 출처 | 일반 품목표와 행·필드가 같다(전수조사 「중복 수집원 확인」) |
| `g2b-contract-v1` | `*_grdFile` | `CHK` | 행 선택 상태 | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `*_grdFile` | `atchFileDscr` | 첨부파일 설명 | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R14) |
| `g2b-contract-v1` | `*_grdFile` | `atchFileKndCd` | 첨부파일 종류코드(문서유형명은 미확정) | 후보 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-v1` | `*_grdFile` | `atchFileNm` | 서버 내부 파일경로/저장명/이미지 URL(수집 제외) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `*_grdFile` | `atchFileSqno` | 첨부파일순번 | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R14) |
| `g2b-contract-v1` | `*_grdFile` | `atflPathNm` | 서버 내부 파일경로/저장명/이미지 URL(수집 제외) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `*_grdFile` | `atflRmrk` | 첨부파일 비고 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `*_grdFile` | `befRowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `*_grdFile` | `bsneClsfCd` | 내부 업무분류코드 | 후보 | 구조·이름 분류 |
| `g2b-contract-v1` | `*_grdFile` | `chngDttm` | 등록/변경/입력일시(업무일자와 구별) | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `*_grdFile` | `chnrId` | 시스템 등록/변경/입력자(수집 제외) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `*_grdFile` | `chnrIpar` | 등록/변경 IP 성격 값(수집 제외) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `*_grdFile` | `colNm` | 내부 테이블/컬럼명 | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `*_grdFile` | `delYn` | 삭제 여부 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `*_grdFile` | `dwnldPrmsYn` | 다운로드 허용 여부 | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `*_grdFile` | `fileExtnNm` | 파일 확장자 | 후보 | ID·표 맥락 |
| `g2b-contract-v1` | `*_grdFile` | `fileSz` | 파일 크기(단위 확인) | 후보 | ID·표 맥락 · R14 「파일 크기 후보」, 단위 미확정 |
| `g2b-contract-v1` | `*_grdFile` | `imgUrl` | 서버 내부 파일경로/저장명/이미지 URL(수집 제외) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `*_grdFile` | `inptDt` | 등록/변경/입력일시(업무일자와 구별) | 후보 | 구조·이름 분류 |
| `g2b-contract-v1` | `*_grdFile` | `kbrdrNm` | 시스템 등록/변경/입력자(수집 제외) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `*_grdFile` | `nIndex` | 표시/출력/내부 순번(업무 키로 사용 금지) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `*_grdFile` | `orgnlAtchFileNm` | 원본 첨부파일명 | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R14) |
| `g2b-contract-v1` | `*_grdFile` | `rgstDttm` | 등록/변경/입력일시(업무일자와 구별) | 후보 | 빈 열만 확인 |
| `g2b-contract-v1` | `*_grdFile` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `*_grdFile` | `rstrId` | 시스템 등록/변경/입력자(수집 제외) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `*_grdFile` | `rstrIpar` | 등록/변경 IP 성격 값(수집 제외) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `*_grdFile` | `scrnIndtOdr` | 화면 표시순서 | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `*_grdFile` | `sessUserNo` | 세션 사용자번호(수집 제외) | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `*_grdFile` | `tblNm` | 내부 테이블/컬럼명 | 제외: 뜻 없음 | 사용자 확인 2026-09-27(json-unused-candidates 「기술 필드」 — 업무 뜻 없음) |
| `g2b-contract-v1` | `*_grdFile` | `untyAtchFileNo` | 통합 첨부파일번호(원문 참조) | 후보 | 사용자 확인 2026-10-15(필드 확정 검토) |

## `g2b-contract-change-v1` — 물품·일반용역 변경계약서 작성 (01608)

수집 57 · 제외: 개인정보 8 · 제외: 다른 출처 52 · 제외: 뜻 없음 44 · 후보 4

| 프로필 | 출처 | ID | 뜻 | 상태 | 근거 |
|---|---|---|---|---|---|
| `g2b-contract-change-v1` | `pointInfo` | `ctrtNm` | 계약명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-change-v1` | `pointInfo` | `frstCtrtDt` | 최초계약일 | 수집 | 사용자 확인 2026-09-27(일괄 답변 1 / R01) |
| `g2b-contract-change-v1` | `pointInfo` | `ctrtMthdNm` | 계약방법 명칭 | 수집 | 사용자 확인 2026-09-27(일괄 답변 10 / R11 · 답변 3 명칭 우선) |
| `g2b-contract-change-v1` | `pointInfo` | `atcsNm` | 국가(지방)계약법시행령 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-change-v1` | `pointInfo` | `ctrtTyCd` | 계약구분(계120001 → 총액계약) | 제외: 다른 출처 | 원문 명칭 ctrtTyNm 을 수집 — 코퍼스의 모든 문서에서 코드(사전)의 명칭과 같다(코퍼스 대조 2026-10-15, 답변 3 명칭 우선) |
| `g2b-contract-change-v1` | `pointInfo` | `ctrtChrsSeCd` | 계약특성 | 수집 | 사용자 확인 2026-10-15(화면에서 확인한 명칭 — 계약특성) |
| `g2b-contract-change-v1` | `pointInfo` | `ctrtFee` | 수수료 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-change-v1` | `pointInfo` | `lqdmRat` | 지체상금률(일당 %) | 수집 | 사용자 확인 2026-09-27(질문 Q03 용어·단위 승인) |
| `g2b-contract-change-v1` | `pointInfo` | `dcmtGtnrt` | 하자보수보증금률 | 수집 | 사용자 확인 2026-09-27(질문 Q03 용어·단위 승인) |
| `g2b-contract-change-v1` | `pointInfo` | `ctrtPrdCn` | 납품기한 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-change-v1` | `pointInfo` | `prttnDlvgdsPsbltyYn` | 분할납품 가능 여부 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-change-v1` | `pointInfo` | `drpmSbpySeCd` | 지급방법(직130001 → 직불, 직130002 → 대지급) | 수집 | 사용자 확인 2026-09-27(일괄 답변 10 / R11) |
| `g2b-contract-change-v1` | `pointInfo` | `dmstUntyGrpNm` | 수요기관명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-change-v1` | `pointInfo` | `ctrtGtnrt` | 계약보증금률 | 수집 | 사용자 확인 2026-09-27(질문 Q03 용어·단위 승인) |
| `g2b-contract-change-v1` | `pointInfo` | `ctrtGrnteAmt` | 계약보증금액 | 수집 | 사용자 확인 2026-09-27(질문 Q02 부분 확정) |
| `g2b-contract-change-v1` | `pointInfo` | `ctwtSbmsnMthdCd` | 계약보증서 제출방법 | 수집 | 사용자 확인 2026-09-27(질문 Q03 용어·단위 승인) |
| `g2b-contract-change-v1` | `pointInfo` | `advcPinfrmYn` | 선급금 선고지 여부 | 수집 | 사용자 확인 2026-09-27(일괄 답변 7 / R08) |
| `g2b-contract-change-v1` | `pointInfo` | `advcPinfrmPsbltyRat` | 선급금 선고지 비율(%) | 수집 | 사용자 확인 2026-09-27(질문 Q04 부분 확정) |
| `g2b-contract-change-v1` | `pointInfo` | `fctrRspbNbyr` | 하자담보책임기간(연수) | 수집 | 사용자 확인 2026-09-27(질문 Q03 용어·단위 승인) |
| `g2b-contract-change-v1` | `pointInfo` | `fctrRspbMtnb` | 하자담보책임기간(개월수) | 수집 | 사용자 확인 2026-09-27(질문 Q03 용어·단위 승인) |
| `g2b-contract-change-v1` | `pointInfo` | `bidPbancNo` | 입찰공고번호(본번호) | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-contract-change-v1` | `pointInfo` | `ctrtDvofNm` | 계약 조직/직책명(개별 약어 미확정) | 후보 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-change-v1` | `pointInfo` | `ctrtSlcfNm` | 계약 조직/직책명(개별 약어 미확정) | 후보 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-change-v1` | `pointInfo` | `ctrtDpdrNm` | 계약 조직/직책명(개별 약어 미확정) | 후보 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-change-v1` | `pointInfo` | `ctrtPicNm` | 계약담당자 | 수집 | 사용자 확인 2026-09-27(일괄 답변 11 / R12) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `ctrtNo` | 계약번호(본번호) | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `ctrtChgOrd` | 계약변경차수 | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `itemClsfNo` | 물품분류번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `itemIdnfNo` | 물품식별번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `ctrtItemNm` | 계약 품목명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `itemSpecVal` | 품목 상세규격 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `spplRgnNm` | 공급지역 번호/명/상세명 | 후보 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `ctrtUntVal` | 계약 품목단위 값/코드 | 수집 | 사용자 확인 2026-09-27(일괄 답변 6 / R07) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `devyCndtCd` | 인도조건 | 수집 | 사용자 확인 2026-09-27(일괄 답변 6 / R07) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `ctrtQty` | 품목별 계약수량 | 수집 | 사용자 확인 2026-09-27(일괄 답변 2 / R02) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `ctrtUprc` | 품목별 계약단가 | 수집 | 사용자 확인 2026-09-27(일괄 답변 2 / R02) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `ctrtAmt` | 품목별 계약금액 | 수집 | 사용자 확인 2026-09-27(질문 Q02 부분 확정) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `dmstUntyGrpNm` | 수요기관명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `dlvgdsTermYmd` | 납품기한 날짜 | 수집 | 사용자 확인 2026-09-27(일괄 답변 5 / R06) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `ctrtDmndRcptNo` | 계약요청 접수번호 | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `ctrtDmndRcptOrd` | 계약요청 접수차수 | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `ctrtItemSqno` | 계약 품목순번 | 수집 | 사용자 확인 2026-09-27(일괄 답변 4 / R04 — 명시 키, decisions §6) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCrntEtpsLst` | `ctentUntyGrpNo` | 계약상대자 통합기관번호 | 수집 | 사용자 확인 2026-09-27(decisions §6 업체 식별·지분) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCrntEtpsLst` | `ctentUntyGrpNm` | 계약상대자명 | 수집 | 사용자 확인 2026-09-27(decisions §6 업체 식별·지분) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCrntEtpsLst` | `bzmnRegNo` | 사업자등록번호 | 수집 | 사용자 확인 2026-09-27(decisions §6 업체 식별·지분) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCrntEtpsLst` | `rprsvNm` | 대표자명 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCrntEtpsLst` | `etpsAddr` | 업체 주소 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCrntEtpsLst` | `etpsTlphNo` | 업체 전화번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCrntEtpsLst` | `etpsFaxNo` | 업체 팩스번호 | 수집 | 사용자 확인 2026-10-15(필드 확정 검토) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCrntEtpsLst` | `ctrtSharRat` | 계약지분율 | 수집 | 사용자 확인 2026-09-27(질문 Q06 부분 확정) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCrntEtpsLst` | `ctrtSharAmt` | 계약지분금액 | 수집 | 사용자 확인 2026-09-27(질문 Q06 부분 확정) |
| `g2b-contract-change-v1` | `*_grdFile` | `atchFileDscr` | 첨부파일 설명 | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R14) |
| `g2b-contract-change-v1` | `*_grdFile` | `orgnlAtchFileNm` | 원본 첨부파일명 | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R14) |
| `g2b-contract-change-v1` | `*_grdFile` | `atchFileSqno` | 첨부파일순번 | 수집 | 사용자 확인 2026-09-27(일괄 답변 12 / R14) |
| `g2b-contract-change-v1` | `pointInfo` | `areaCd` | 화면/영역 코드(업무 의미 미확정) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · pointInfo 에서 확인 |
| `g2b-contract-change-v1` | `pointInfo` | `atmtCntt01` | 화면 도움말 HTML(사업 특기사항 아님) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · pointInfo 에서 확인 |
| `g2b-contract-change-v1` | `pointInfo` | `ctentUntyGrpNm` | 계약상대자명 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `pointInfo` | `ctentUntyGrpNo` | 계약상대자 통합기관번호 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `pointInfo` | `ctrtAmt` | 품목별 계약금액 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `pointInfo` | `ctrtItemNm` | 계약 품목명 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `pointInfo` | `ctrtNo` | 계약번호(본번호) | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `pointInfo` | `ctrtQty` | 품목별 계약수량 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `pointInfo` | `ctrtTyNm` | 계약구분 명칭 | 수집 | 같은 ID 원칙(2026-10-15) — g2b-notice-b-v1 · pointInfo 에서 확정 |
| `g2b-contract-change-v1` | `pointInfo` | `ctrtUntVal` | 계약 품목단위 값/코드 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `pointInfo` | `ctrtUprc` | 품목별 계약단가 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `pointInfo` | `depth1` | 화면 경로 단계 후보 | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · pointInfo 에서 확인 |
| `g2b-contract-change-v1` | `pointInfo` | `depth2` | 화면 경로 단계 후보 | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · pointInfo 에서 확인 |
| `g2b-contract-change-v1` | `pointInfo` | `depth3` | 화면 경로 단계 후보 | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · pointInfo 에서 확인 |
| `g2b-contract-change-v1` | `pointInfo` | `devyCndtCd` | 인도조건 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `pointInfo` | `dlvgdsPlacNm` | 납품장소 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `pointInfo` | `dlvgdsTermNody` | 납품일수 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `pointInfo` | `dlvgdsTermYmd` | 납품기한 날짜 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `pointInfo` | `itemClsfNo` | 물품분류번호 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `pointInfo` | `itemIdnfNo` | 물품식별번호 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `pointInfo` | `itemSpecVal` | 품목 상세규격 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `pointInfo` | `odn1PlorNtnNm` | 원산지 국가명 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `pointInfo` | `recordCountPerPage` | 페이지당 표시건수 | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · pointInfo 에서 확인 |
| `g2b-contract-change-v1` | `*_grdFile` | `CHK` | 행 선택 상태 | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-contract-change-v1` | `*_grdFile` | `atchFileNm` | 서버 내부 파일경로/저장명/이미지 URL(수집 제외) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-contract-change-v1` | `*_grdFile` | `atflPathNm` | 서버 내부 파일경로/저장명/이미지 URL(수집 제외) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-contract-change-v1` | `*_grdFile` | `befRowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-contract-change-v1` | `*_grdFile` | `chnrId` | 시스템 등록/변경/입력자(수집 제외) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-contract-change-v1` | `*_grdFile` | `chnrIpar` | 등록/변경 IP 성격 값(수집 제외) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-contract-change-v1` | `*_grdFile` | `colNm` | 내부 테이블/컬럼명 | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-contract-change-v1` | `*_grdFile` | `imgUrl` | 서버 내부 파일경로/저장명/이미지 URL(수집 제외) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-contract-change-v1` | `*_grdFile` | `kbrdrNm` | 시스템 등록/변경/입력자(수집 제외) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-contract-change-v1` | `*_grdFile` | `nIndex` | 표시/출력/내부 순번(업무 키로 사용 금지) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-contract-change-v1` | `*_grdFile` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-contract-change-v1` | `*_grdFile` | `rstrId` | 시스템 등록/변경/입력자(수집 제외) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-contract-change-v1` | `*_grdFile` | `rstrIpar` | 등록/변경 IP 성격 값(수집 제외) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-contract-change-v1` | `*_grdFile` | `scrnIndtOdr` | 화면 표시순서 | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-contract-change-v1` | `*_grdFile` | `sessUserNo` | 세션 사용자번호(수집 제외) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-contract-change-v1` | `*_grdFile` | `tblNm` | 내부 테이블/컬럼명 | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · *_grdFile 에서 확인 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdBeoeEtpsLst` | `giveActno` | 지급계좌 은행코드/계좌번호(수집 제외) | 제외: 개인정보 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_grdEtpsLst 에서 확인 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdBeoeEtpsLst` | `kbrdrId` | 시스템 등록/변경/입력자(수집 제외) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_grdEtpsLst 에서 확인 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdBeoeEtpsLst` | `nextRowYn` | 다음 행 여부(전체 수집 증거로 사용 금지) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_grdEtpsLst 에서 확인 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdBeoeEtpsLst` | `outNum` | 표시/출력/내부 순번(업무 키로 사용 금지) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_grdEtpsLst 에서 확인 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdBeoeEtpsLst` | `rowNum` | 표시/출력/내부 순번(업무 키로 사용 금지) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_grdEtpsLst 에서 확인 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdBeoeEtpsLst` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCrntEtpsLst` | `cgntTxam` | 인지세 부과액(물품제조계약) | 제외: 다른 출처 | 같은 ID 원칙(2026-10-15) — 같은 자리의 g2b-contract-v1 칸이 다른 출처(뜻은 머리 칸과 같다) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCrntEtpsLst` | `ctrtChgOrd` | 계약변경차수 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCrntEtpsLst` | `ctrtGrnteAmt` | 계약보증금액 | 제외: 다른 출처 | 같은 ID 원칙(2026-10-15) — 같은 자리의 g2b-contract-v1 칸이 다른 출처(뜻은 머리 칸과 같다) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCrntEtpsLst` | `ctrtNo` | 계약번호(본번호) | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCrntEtpsLst` | `giveActno` | 지급계좌 은행코드/계좌번호(수집 제외) | 제외: 개인정보 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_grdEtpsLst 에서 확인 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCrntEtpsLst` | `kbrdrId` | 시스템 등록/변경/입력자(수집 제외) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_grdEtpsLst 에서 확인 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCrntEtpsLst` | `nextRowYn` | 다음 행 여부(전체 수집 증거로 사용 금지) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_grdEtpsLst 에서 확인 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCrntEtpsLst` | `outNum` | 표시/출력/내부 순번(업무 키로 사용 금지) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_grdEtpsLst 에서 확인 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCrntEtpsLst` | `rowNum` | 표시/출력/내부 순번(업무 키로 사용 금지) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_grdEtpsLst 에서 확인 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCrntEtpsLst` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `bidClsfNo` | 입찰분류번호 | 수집 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List 에서 확정 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `bidPbancItemSqno` | 입찰공고 품목순번 | 수집 | 같은 ID 원칙(2026-10-15) — g2b-notice-a-v1 · mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List 에서 확정 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `ctentUntyGrpNm` | 계약상대자명 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `ctentUntyGrpNo` | 계약상대자 통합기관번호 | 제외: 다른 출처 | 같은 문서에서 수집하는 칸과 같은 값(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `ctrtGrnteAmt` | 계약보증금액 | 제외: 다른 출처 | 같은 ID 원칙(2026-10-15) — 같은 자리의 g2b-contract-v1 칸이 다른 출처(뜻은 머리 칸과 같다) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `ctrtGtnrt` | 계약보증금률 | 제외: 다른 출처 | 같은 ID 원칙(2026-10-15) — 같은 자리의 g2b-contract-v1 칸이 다른 출처(뜻은 머리 칸과 같다) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `ddcCtcrPrsnNo` | 주민번호 성격 식별자(수집 제외) | 제외: 개인정보 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis 에서 확인 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `dlvgdsPlacNm` | 납품장소 | 수집 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridView 에서 확정 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `dlvgdsTermNody` | 납품일수 | 수집 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridView 에서 확정 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `dmstUntyGrpNo` | 수요기관 통합기관번호 | 수집 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · pointInfo 에서 확정 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `insdPrsnNo` | 주민번호 성격 식별자(수집 제외) | 제외: 개인정보 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis 에서 확인 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `isChecked` | 행 선택 상태 | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis 에서 확인 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `kbrdrId` | 시스템 등록/변경/입력자(수집 제외) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_grdEtpsLst 에서 확인 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `ndfsPrcmDmndNo` | 국방 조달요구번호 | 수집 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridView 에서 확정 · 이 화면에서는 코퍼스에서 늘 비어 있음 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `nextRowYn` | 다음 행 여부(전체 수집 증거로 사용 금지) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_grdEtpsLst 에서 확인 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `odn1PlorNtnNm` | 원산지 국가명 | 수집 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis 에서 확정 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `outNum` | 표시/출력/내부 순번(업무 키로 사용 금지) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_grdEtpsLst 에서 확인 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `ownrPrsnNo` | 주민번호 성격 식별자(수집 제외) | 제외: 개인정보 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis 에서 확인 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `rowNum` | 표시/출력/내부 순번(업무 키로 사용 금지) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_grdEtpsLst 에서 확인 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLis` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLisExcel` | `bidClsfNo` | 입찰분류번호 | 제외: 다른 출처 | mf_wfm_container_grdCtrtLis 와 행·필드가 같다(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLisExcel` | `bidPbancItemSqno` | 입찰공고 품목순번 | 제외: 다른 출처 | mf_wfm_container_grdCtrtLis 와 행·필드가 같다(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLisExcel` | `ctentUntyGrpNm` | 계약상대자명 | 제외: 다른 출처 | mf_wfm_container_grdCtrtLis 와 행·필드가 같다(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLisExcel` | `ctentUntyGrpNo` | 계약상대자 통합기관번호 | 제외: 다른 출처 | mf_wfm_container_grdCtrtLis 와 행·필드가 같다(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLisExcel` | `ctrtAmt` | 품목별 계약금액 | 제외: 다른 출처 | mf_wfm_container_grdCtrtLis 와 행·필드가 같다(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLisExcel` | `ctrtChgOrd` | 계약변경차수 | 제외: 다른 출처 | mf_wfm_container_grdCtrtLis 와 행·필드가 같다(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLisExcel` | `ctrtDmndRcptNo` | 계약요청 접수번호 | 제외: 다른 출처 | mf_wfm_container_grdCtrtLis 와 행·필드가 같다(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLisExcel` | `ctrtDmndRcptOrd` | 계약요청 접수차수 | 제외: 다른 출처 | mf_wfm_container_grdCtrtLis 와 행·필드가 같다(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLisExcel` | `ctrtGrnteAmt` | 계약보증금액 | 제외: 다른 출처 | mf_wfm_container_grdCtrtLis 와 행·필드가 같다(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLisExcel` | `ctrtGtnrt` | 계약보증금률 | 제외: 다른 출처 | mf_wfm_container_grdCtrtLis 와 행·필드가 같다(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLisExcel` | `ctrtItemNm` | 계약 품목명 | 제외: 다른 출처 | mf_wfm_container_grdCtrtLis 와 행·필드가 같다(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLisExcel` | `ctrtItemSqno` | 계약 품목순번 | 제외: 다른 출처 | mf_wfm_container_grdCtrtLis 와 행·필드가 같다(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLisExcel` | `ctrtNo` | 계약번호(본번호) | 제외: 다른 출처 | mf_wfm_container_grdCtrtLis 와 행·필드가 같다(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLisExcel` | `ctrtQty` | 품목별 계약수량 | 제외: 다른 출처 | mf_wfm_container_grdCtrtLis 와 행·필드가 같다(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLisExcel` | `ctrtUntVal` | 계약 품목단위 값/코드 | 제외: 다른 출처 | mf_wfm_container_grdCtrtLis 와 행·필드가 같다(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLisExcel` | `ctrtUprc` | 품목별 계약단가 | 제외: 다른 출처 | mf_wfm_container_grdCtrtLis 와 행·필드가 같다(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLisExcel` | `ddcCtcrPrsnNo` | 주민번호 성격 식별자(수집 제외) | 제외: 개인정보 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis 에서 확인 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLisExcel` | `devyCndtCd` | 인도조건 | 제외: 다른 출처 | mf_wfm_container_grdCtrtLis 와 행·필드가 같다(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLisExcel` | `dlvgdsPlacNm` | 납품장소 | 제외: 다른 출처 | mf_wfm_container_grdCtrtLis 와 행·필드가 같다(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLisExcel` | `dlvgdsTermNody` | 납품일수 | 제외: 다른 출처 | mf_wfm_container_grdCtrtLis 와 행·필드가 같다(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLisExcel` | `dlvgdsTermYmd` | 납품기한 날짜 | 제외: 다른 출처 | mf_wfm_container_grdCtrtLis 와 행·필드가 같다(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLisExcel` | `dmstUntyGrpNm` | 수요기관명 | 제외: 다른 출처 | mf_wfm_container_grdCtrtLis 와 행·필드가 같다(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLisExcel` | `dmstUntyGrpNo` | 수요기관 통합기관번호 | 제외: 다른 출처 | mf_wfm_container_grdCtrtLis 와 행·필드가 같다(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLisExcel` | `insdPrsnNo` | 주민번호 성격 식별자(수집 제외) | 제외: 개인정보 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis 에서 확인 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLisExcel` | `isChecked` | 행 선택 상태 | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis 에서 확인 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLisExcel` | `itemClsfNo` | 물품분류번호 | 제외: 다른 출처 | mf_wfm_container_grdCtrtLis 와 행·필드가 같다(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLisExcel` | `itemIdnfNo` | 물품식별번호 | 제외: 다른 출처 | mf_wfm_container_grdCtrtLis 와 행·필드가 같다(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLisExcel` | `itemSpecVal` | 품목 상세규격 | 제외: 다른 출처 | mf_wfm_container_grdCtrtLis 와 행·필드가 같다(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLisExcel` | `kbrdrId` | 시스템 등록/변경/입력자(수집 제외) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_grdEtpsLst 에서 확인 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLisExcel` | `ndfsPrcmDmndNo` | 국방 조달요구번호 | 제외: 다른 출처 | mf_wfm_container_grdCtrtLis 와 행·필드가 같다(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLisExcel` | `nextRowYn` | 다음 행 여부(전체 수집 증거로 사용 금지) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_grdEtpsLst 에서 확인 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLisExcel` | `odn1PlorNtnNm` | 원산지 국가명 | 제외: 다른 출처 | mf_wfm_container_grdCtrtLis 와 행·필드가 같다(코퍼스 대조 2026-10-15) |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLisExcel` | `outNum` | 표시/출력/내부 순번(업무 키로 사용 금지) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_grdEtpsLst 에서 확인 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLisExcel` | `ownrPrsnNo` | 주민번호 성격 식별자(수집 제외) | 제외: 개인정보 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis 에서 확인 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLisExcel` | `rowNum` | 표시/출력/내부 순번(업무 키로 사용 금지) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-contract-v1 · mf_wfm_container_grdEtpsLst 에서 확인 |
| `g2b-contract-change-v1` | `mf_wfm_container_grdCtrtLisExcel` | `rowStatus` | 현재/이전 행 상태(ERP 저장상태 의미 미확정) | 제외: 뜻 없음 | 같은 ID 원칙(2026-10-15) — g2b-request-v1 · mf_wfm_container_gridViewExcel 에서 확인 |

## `g2b-public-notice-header-v1` — 화면 없음(받지 않는 프로필)

수집 8

| 프로필 | 출처 | ID | 뜻 | 상태 | 근거 |
|---|---|---|---|---|---|
| `g2b-public-notice-header-v1` | `pointInfo` | `title` | 공고명 | 수집 | 09-27 매핑(공개 공고 화면 머리글 값) · 개별 확인 기록 없음 |
| `g2b-public-notice-header-v1` | `pointInfo` | `notice_kind` | 공고종류 | 수집 | 09-27 매핑(공개 공고 화면 머리글 값) · 개별 확인 기록 없음 |
| `g2b-public-notice-header-v1` | `pointInfo` | `posted_at` | 게시일시 | 수집 | 09-27 매핑(공개 공고 화면 머리글 값) · 개별 확인 기록 없음 |
| `g2b-public-notice-header-v1` | `pointInfo` | `award_method` | 낙찰방법 | 수집 | 09-27 매핑(공개 공고 화면 머리글 값) · 개별 확인 기록 없음 |
| `g2b-public-notice-header-v1` | `pointInfo` | `contract_method` | 계약방법 | 수집 | 09-27 매핑(공개 공고 화면 머리글 값) · 개별 확인 기록 없음 |
| `g2b-public-notice-header-v1` | `pointInfo` | `notice_agency` | 공고기관 | 수집 | 09-27 매핑(공개 공고 화면 머리글 값) · 개별 확인 기록 없음 |
| `g2b-public-notice-header-v1` | `pointInfo` | `noticeBase` | 공고 본번호 | 수집 | 09-27 매핑(공개 공고 화면 머리글 값) · 개별 확인 기록 없음 |
| `g2b-public-notice-header-v1` | `pointInfo` | `seq` | 공고 차수 | 수집 | 09-27 매핑(공개 공고 화면 머리글 값) · 개별 확인 기록 없음 |

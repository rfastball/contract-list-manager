# JSON 매핑 확인 질문과 관찰 맥락

2026-09-27. [전체 ID 매핑표](json-field-mapping.md)의 용어 후보를 사용자가 확정하기 위한 목록이다.
구현은 이 확인 뒤에 진행한다. 승인·미사용·해석 보류 상태는 아래 최신 사용자 결정이 우선한다.

답변은 `Q번호 / ID / 공식 용어 / 값의 단위·코드 뜻 / 적용 범위` 형태면 된다.
예: `Q04 / advfPinfrmPsbltyRat / [실제 화면 용어] / % / 접수 전체`.
맞는 후보는 묶어서 승인하고 틀린 항목만 수정해도 된다. 원천에 없는 정보의 추정 생성을 승인하는 절차는 아니다.

진행 방식: 최신 사용자 요청에 따라 남은 항목을 **관찰 증거 → 의미 추론 → 판단 이유 → 제안 처리**로 한꺼번에 제시한다.
아래 ‘일괄 답변 반영’이 현재 결정이며, ‘남은 항목 일괄 검토’와 Q01~Q18은 그 근거·질문 이력이다.
이미 확정·제외한 항목을 재질문하지 않는다. 승인 범위 밖의 추론은 사용자 확정과 구별한다.
단, 미사용 가능성이 높은 항목은 최신 사용자 요청에 따라 [일괄 제외 후보](json-unused-candidates.md)로 묶어 검토한다.
해당 목록의 A1~A7·B1~B5는 사용자 요청으로 모두 제외 확정했다. 제외된 항목의 의미를 다시 질문하지 않는다.
최신 방침: 당장 사용하지 않는 항목은 **분류·용어 후보만 보존하고 미사용 보류**한다.
후보 의미를 확정하려고 하나씩 질문하지 않는다. 질문은 현재 MVP에서 실제 사용할 정보의
정확성·명시 키 연결·값 보존에 필요한 사항에 한정한다. 업체별 계약지분은 유지하되 공동수급
상세는 최신 사용자 결정에 따라 코퍼스 확보까지 미사용으로 전환했다.

## 사용자 확정 기록 (2026-09-27)

- Q01: 사용자가 제시된 금액 구분·매핑을 일괄 확정했다. 답변에 없는 코드사전이나 계산 규칙을 추가하지 않는다.
- Q02 부분 확정: `cgntTxam`은 **물품제조계약에만 있는 인지세 부과액**이다.
  이 시스템의 업무 의미에 관한 사용자 확인이며 일반 법령 판단으로 확대하지 않는다.
  계약 전체의 부과액은 유지하며, 업체표의 동명 열은 아래 사용자 결정에 따라 제외한다.
- Q03: 제시된 용어·단위를 승인했다. `lqdmRat="0.075"`는 **일당 0.075%의 지체상금률**이다.
  계약보증금률·하자보수보증금률, 보증금액·보증서 제출방법, 하자기간 연수·개월수 해석을 승인했다.
  실제로 제시하지 않은 제출방법 코드별 명칭이나 연/개월 선택·합산 동작은 새로 확정한 것으로 보지 않는다.
- Q02 추가 확정: 사용자는 `ctrtAmt`가 품목별로 반복된다면 선택 품목 금액이라는 해석을 승인했다.
  원본 계약 5개의 대표 품목표 26행(3·4·1·2·16행) 모두 `ctrtAmt`를 가지며,
  5개 모두 `pointInfo.ctrtAmt`가 첫 품목 금액과 같음을 재확인했다. 조건이 충족되어
  `ctrtAmt`는 품목별 계약금액, `pointInfo.ctrtAmt`는 선택 품목 값으로 확정한다.
  실제 화면에서 선택행을 바꾸는 동작까지 검증했다는 뜻은 아니다. 계약 총액으로 매핑하지 않는다.
- Q02 추가 확정: `pointInfo.ctrtGrnteAmt`는 계약 전체의 계약보증금액이다.
  품목표의 동명 열은 아래 사용자 결정에 따라 제외한다.
- Q02 보류 결정: 업체표 `grdEtpsLst[].ctrtGrnteAmt`는 사용자 지시에 따라 현재 무의미한
  항목으로 보고 MVP 업무 매핑·계산·표시에서 제외한다. 0을 면제·미납·미입력으로 해석하지 않는다.
  계약 전체의 `pointInfo.ctrtGrnteAmt`에는 이 제외 결정을 적용하지 않는다.
- Q02 제외 결정: 업체표 `grdEtpsLst[].cgntTxam`은 MVP 업무 매핑·계산·표시에서 제외한다.
  계약 전체 인지세 부과액 `pointInfo.cgntTxam`은 유지한다. 업체표 0의 의미는 추정하지 않는다.
- Q02 제외 확정: 품목표 `grdCtrtLis[].ctrtGrnteAmt`는 무의미한 항목으로 확정하여
  MVP 업무 매핑·계산·표시에서 제외한다. 중복 Excel 품목표의 동명 열도 같은 원천 범위다.
  계약 전체의 `pointInfo.ctrtGrnteAmt`는 유지한다.
- Q04 부분 확정: 접수 `advfPinfrmPsbltyRat=70`은 **선급금 선고지 가능 비율 70%**다.
  실제 지급률로 해석하지 않는다. 아래 계약 단계의 실제 선고지 비율과 구별한다.
- Q04 제외 결정: 접수 `advfGivePsbltyYn`과 `advfGivePsbltyRat`는 사용하지 않는 값으로 처리하여
  MVP 업무 매핑·계산·표시에서 제외한다. N·0을 실제 지급불가나 지급한도 0으로 해석하지 않는다.
- Q04 추가 확정: 계약 `advcPinfrmPsbltyRat`는 **실제 선급금 선고지 비율**이다.
  계약은 실제 고지가 생성되는 단계이며, 값 70은 실제 선고지 비율 70%를 뜻한다.
  ID에 Psblty가 남아 있어도 접수 단계의 가능 비율로 표시하지 않는다. 실제 지급 완료 비율과도 구별한다.
- Q04 저장 명칭 지정: `advcPinfrmPsbltyRat`는 **선급금 선고지 비율**로 저장한다.
  사용자 오타 정정을 반영했다. 위 업무 의미는 유지한다.
- Q05 제외 결정: 업체표 `incmCgntPchaAmt`는 우선 미사용 값으로 처리하여 MVP 업무 매핑·계산·표시에서 제외한다.
  원문 0을 실제 인지 구매액 0으로 해석하지 않는다.
- 일괄 제외 확정: [미사용 목록](json-unused-candidates.md) A1~A7·B1~B5의 출처별 122개 필드 전체를 제외한다.
  기술·내부관리 필드도 업무 수집에서 제외하되 수집기 검증에 필요한 것은 내부용으로 구분한다.
  계약 전체 보증금·인지세 부과액·선고지 비율, 업체 지분·공동수급 상세는 유지한다.
- Q05 제외 결정: 업체표 `grdEtpsLst[].vatIncsYn`·`vatRat`는 미사용 값으로 처리하여
  MVP 업무 매핑·계산·표시에서 제외한다. pointInfo의 동명 필드 및 부가세액 `vatAmt`로 확대하지 않는다.
- Q06 부분 확정: 업체표 `ctrtSharRat`는 **현재 계약차수의 해당 업체 지분율(%)**,
  `ctrtSharAmt`는 **현재 계약차수의 해당 업체 지분금액**이다. 업체별 행으로 보존한다.
- Q06 미사용 보류: `wholSharRat`·`wholSharAmt`는 전체지분율·전체지분금액이라는 분류·용어 후보만
  남기고 당장 사용하지 않는다. pointInfo와 업체표 모두 업무 매핑·계산·표시에 사용하지 않으며
  장기계속·총괄 기준이라는 의미는 확정하지 않는다. 사용이 필요해질 때 의미를 다시 확인한다.
- Q07 부분 확정: 접수 `pointInfo.wrtYmd`는 **접수일**이다. 작성일로 해석하지 않으며
  기존 PDF 접수일자와 대응한다. 예시(지어낸 값) `20260404`는 2026-04-04 접수일이다.
- Q07 미사용 결정: 공고 A의 `atrzDt`는 분류·용어 후보만 남기고 업무 매핑·계산·표시에서 제외한다.
  결재일시라는 의미도 확정하지 않으며 공고 게시일시의 대체 출처로 사용하지 않는다.
- 일괄 답변 수신: 아래 표에 승인·정정·보류를 반영했다. 이전 기록과 R01~R16의 제안보다 최신 결정이 우선한다.

### 일괄 답변 반영 (2026-09-27)

사용자의 번호는 직전 대화의 1~12번이며 문서 R번호와 다르므로 대응을 명시한다.
‘승인’은 제시했던 해석과 보류 경계를 함께 승인한 것으로 기록하며, 모른다고 제시한 코드명을 새로 확정하지 않는다.

| 답변 / 근거 | 최신 결정 | 적용 범위·남은 경계 |
|---|---|---|
| 1 / R01 계약 날짜 | 승인. `frstCtrtDt`=최초계약일, `ctrtDt`=현재 차수 계약일 | `ctrtAprvYmd`는 계약 승인일 후보·미사용 보류. 승인 업무 단계는 추가 확정하지 않음 |
| 2 / R02 품목 수량·단가 | 승인. `ctrtQty`=품목별 계약수량, `ctrtUprc`=품목별 계약단가 | pointInfo는 선택 품목 값. 전체 품목표를 사용하며 단가 소수 보존 |
| 3 / R03 공고 종류·상태 | 제시한 대응 승인. **직접 나와 있는 이름을 코드 해석보다 우선 사용** | 같은 문서·같은 의미의 명칭을 사용. 이름이 없을 때 승인된 코드 대응만 사용하고 미해석 코드는 명칭 미확정 유지 |
| 4 / R04~R05 연결·순번 | 표본 E 접수는 **오첨부로 인한 접수번호 오류**. 연결은 명시 키만 사용, 나머지는 미사용 | S14를 표본 E 계약의 대응 접수로 취급하지 않음. 올바른 접수번호를 추정하거나 원본을 고치지 않음. `prchsRsltnSqno`의 추정 별칭, `ctrtItemSqnm` 및 납품순번의 추정 계보 등 부가 해석은 후보만 보존하고 미사용. 명시 식별자·참조 자체는 유지 |
| 5 / R06 납품기한 | **실제 공고 입력 오류**. 메인/품목별 입력란이 분리되어 발생 | 변경 중 잔존값이라는 기존 추측 대신 사용자 설명을 기록. 출처별 원문 보존·불일치 표시. 메인 또는 품목 중 우선값·정정일자는 지정되지 않았으므로 임의 선택하지 않음 |
| 6 / R07 단위·인도조건·납품부대 | 제시한 해석 승인 | 단위 Val은 표시값. 관찰된 인도조건 대응과 납품부대 코드·명칭 적용. 부대코드를 통합기관번호와 동일시하지 않음 |
| 7 / R08 하자기간·선고지 여부 | 승인. 연수·개월수 별도 보존·표시, 접수/계약별 선급금 선고지 여부 | 기존 선고지 비율 의미 유지. 연·개월 선택·합산 동작이나 실제 지급 완료 의미를 추가하지 않음 |
| 8 / R09 공동수급 | **코퍼스 확보 시까지 미사용** | 공고·계약의 공동수급 방식, 역할·분담 관련 필드는 분류·근거만 보존하고 MVP 업무 매핑·계산·표시에서 보류. 별도로 확정한 업체별 `ctrtSharRat/ctrtSharAmt`는 유지 |
| 9 / R10 검사·검수 | **해석 보류** | 제안한 기관 구분·검사형태 코드 대응을 확정하지 않음. 보류를 기존 PDF 검사·검수 정보 삭제로 확대하지 않음 |
| 10 / R11 계약·낙찰·지급 코드 | 제시한 해석과 미확정 처리 승인 | 제한경쟁·총액계약·적격심사제·전자입찰의 제시된 대응 승인. 나머지 코드의 미제시 명칭은 생성하지 않음 |
| 11 / R12 담당자·직책 | 제시한 해석·처리 승인 | 계약관 직책명·계약담당자 대응, 다른 역할의 이름을 합치지 않음. 나머지 미확정 직책은 후보·미사용 유지 |
| 12 / R13~R14 일정·첨부 | 제시한 해석·처리 승인 | 게시일시·일정명·방법·시작/마감 대응. 첨부 종류코드·원본 파일명·설명 구분. 미확정 문서유형명·파일크기 단위는 추정하지 않음 |

R15~R16(직전 답변 13번)은 이번 번호별 승인에 포함되지 않는다. 기존 제외 방침과
JSON/PDF 대조 결과는 유지하되 이번 답변으로 추가 승인되었다고 기록하지 않는다.

## 남은 항목 일괄 검토 (2026-09-27)

JSON 15개를 다시 읽어 조사했다. 표본 S번호·전체 경로는 [매핑표](json-field-mapping.md)에 있다.
이하의 ‘추론·제안’은 답변 전 조사 근거를 보존한 것이다. 현재 사용 여부와 확정 상태는 위 일괄 답변 표를 따른다.
계약 품목은 `grdCtrtLis`, 업체는 `grdEtpsLst` 기준이며 중복 Excel 표를 합산하지 않았다.
근거 수준은 ‘원문 명칭 직접 관찰’, ‘대응 자료와 일치’, ‘반복 패턴 추론’, ‘판단 불가’를 구별한다.
PDF 대조는 기존 `tests/Pclm.Core.Tests/golden/` 의 표본 A 공고 골든(파일명 생략)에 기록된 추출값을 이용했다.
이번에 PDF를 새로 파싱하거나 실제 웹페이지의 선택·변경 동작을 시험한 것은 아니다.

### R01. 계약일·최초계약일·승인일 — 강한 추론, 승인일의 업무 단계는 미확정

날짜는 지어낸 값이다 — 한 폭으로 옮겨 **같음·앞뒤·간격만** 원문 그대로 남겼다.

| 표본 | 계약차수 | frstCtrtDt | ctrtDt | ctrtAprvYmd |
|---|---|---|---|---|
| S06 표본 C | 00 | 2026-07-29 | 2026-07-29 | 2026-07-30 |
| S09 표본 D | 01 | 2026-07-19 | 2026-07-28 | 2026-07-28 |
| S10~S12 표본 E 각 계약 | 00 | 2026-07-30 | 2026-07-30 | 2026-07-30 |

**추론:** `frstCtrtDt`=최초계약일, `ctrtDt`=현재 차수 계약일, `ctrtAprvYmd`=계약 승인일 후보.
00차수에서는 앞 두 날짜가 같고 유일한 01차수에서만 달라지므로, `ctrtDt`를 최초계약일로
통합하는 해석보다 현재 차수 계약일이라는 해석이 강하다. 다만 같은 계약의 00·01 스냅샷을
모두 가진 것은 아니다. 승인일은 S06에서 계약일보다 하루 늦으므로 계약일 대체값으로 쓰지 않는다.
승인이 어떤 업무 행위인지는 증거가 없다. **제안:** 앞 두 날짜를 별도 사용하고 승인일은 후보만 유지·미사용 보류.

### R02. 선택 품목의 수량·단가 — 강한 추론

계약 5개 모두 `pointInfo.ctrtQty/ctrtUprc`가 대표 품목표 첫 행과 수치상 일치한다.
S06·S09·S10·S11·S12 다섯 건 모두 그렇다(수량·단가 원문 생략 — S06의 단가는 소수 셋째 자리까지 있다).
품목표는 각각 3·4·1·2·16행이다.
이미 확정한 선택 품목 `ctrtAmt`와 같은 패턴이므로 **품목별 계약수량·계약단가**로 해석한다.
**제안:** 전체 품목표를 수집하고 pointInfo를 계약 전체 수량·단가로 사용하지 않는다.
단가 소수도 원문대로 보존한다. 실제 UI 선택행 전환 동작은 미검증이다.

### R03. 공고 종류·상태·변경사유·진행상태 — 증거 수준을 나눠 사용

| ID·값 | 제안 의미 | 증거와 한계 |
|---|---|---|
| `pbancKndCd=공440002` | 실공고 | A형 6개에서 일정. S01의 동일 공고 PDF 추출값이 ‘실공고(변경공고)’여서 지지됨 |
| `pbancSttsCd=공400001` | 일반/최초 공고 후보 | S02·S05·S08 일반 표본. 공식 라벨은 직접 없음 |
| `공400002` | 변경공고 | S01 파일명 및 동일 공고 PDF 추출값과 일치 |
| `공400003` | 취소공고 후보 | S15 파일명과 동시 관찰. 공식 라벨 직접 없음 |
| `공400004` | 재공고 후보 | S03 파일명과 동시 관찰. 공식 라벨 직접 없음 |
| `pbancKndNm` | 공고 종류·상태의 결합 표시 | S13 원문 ‘실공고(재공고)’ |
| `bidPgstCd=입160010` / `bidPgstNm` | 개찰완료 | S13 진행상태표 동일 행의 직접 코드·명칭 쌍 |
| `pbancChgRsn` | 공고 변경사유 | S01 ‘공고(입찰)기간 변경’, S02·S03 ‘납품기한 정정’ |
| `cornPbancRsn=유050002` | 재공고/유찰 관련 사유코드 후보 | S03만 관찰. 표시명 없어 코드 뜻은 판단 불가 |

S02의 상태는 일반 후보인데 변경사유 문구가 존재한다. 따라서 **사유 문구 유무로 공고 상태를 결정하지 않는다**.
종류·공고상태·입찰진행상태는 분리하며, 미확정 코드명을 전역 코드사전으로 단정하지 않는다.

### R04. 접수·공고·계약 연결키 — 직접 관찰과 미확정 별칭 분리

접수 `(ctrtDmndRcptNo, ctrtDmndRcptOrd, ctrtDmndRcptItemSqno)`는 S04 3행,
S07 4행, S14 198행 안에서 각각 모두 유일하다. S04↔S05 및 S07↔S08에 같은 복합키가 있어
접수→공고 품목 연결 근거가 된다. 다른 문서에서도 유일하다는 일반 규칙까지 입증한 것은 아니다.

계약 품목에는 `ctrtDmndRcptItemSqno`가 없고 `prchsRsltnSqno`가 있다.
S06·S09에서 값이 각각 1~3·1~4로 맞아 보여도, **다른 ID를 동일한 원천 품목번호로 간주하는 것은 아직 추론**이다.
계약→접수 문서 연결과 계약→접수 개별 품목 연결을 구별한다. 계약이 참조하는 공고 본번호에
차수가 없으면 임의로 000 또는 최신 차수를 보충하지 않는다.

S14는 **접수번호 1개 아래 국방조달요구번호 `ndfsPrcmDmndNo` 17개·198행**이다.
계약 표본 E과 접수번호도 다르다. S10~S12 품목의 접수·공고 참조는 비어 있다.
S13 19행과 계약 1+2+16행이 같아도 행수·제목으로 연결할 수 없다.
**제안:** 이미 확정한 명시 키 연결 정책을 적용하고, `prchsRsltnSqno` 별칭은 의미 확정 전 연결에 쓰지 않는다.

### R05. 품목순번·납품순번 — 반복값이 반례

`ctrtItemSqno`와 `ctrtItemSqnm`은 26행 전부 같고 각 계약 내 1부터 증가한다.
전자는 계약 품목순번, 후자는 표시순번 후보지만 현재 표본으로 둘의 차이는 판단할 수 없다.
반면 S09는 4품목 모두 `dlvgdsSqno=1`이며 접수 S07에서도 요구번호 4개 각각의
`ndfsPrcmPlanDlvgdsSqno=1`이다. **납품순번은 문서 전체 품목 식별자가 아니다.**
두 납품순번이 원천 요구별 같은 계보라는 추론은 가능하지만 이름·값 일치만으로 연결하지 않는다.
공고의 `bidClsfNo`는 관찰 품목에서 모두 1이라 `bidPbancItemSqno`가 분류마다 다시 시작하는지는 알 수 없다.
**제안:** 문서번호·차수·분류번호·각 원문 순번을 분리 보존하고 순번 단독 또는 추정 별칭으로 합치지 않는다.

### R06. 납품기한 — 실제 충돌과 변경계약 반례가 있음

날짜는 R01과 같은 폭으로 옮긴 지어낸 값이다(간격은 원문 그대로).

- S02 표본 B: `dlvgdsTermCn=2026/10/27`, `pointInfo.dlvgdsTermDt=20260927`,
  두 품목의 `dlvgdsTermDt=20260927`, `dlvgdsTermNody=0`이다. 설명과 구조화 날짜가 충돌한다.
- S03 재공고: 설명·날짜가 모두 2026-10-27이며 품목 일수는 빈 문자열이다.
- S05·S08: 설명은 ‘계약 후 90일 이내’이고 일수는 90이다.
- S09 계약: 기한 2026-10-17, 일수 90. 최초계약일 7월 19일로부터 90일이지만
  현재 차수 계약일 7월 28일로부터는 81일이다.
- 계약 26품목 모두 `dlvgdsTermChcSeCd=납140001`이며 날짜와 일수가 함께 존재하고
  `dlvgdsTermMtnb=0`이다. 다른 선택코드 사례가 없어 납140001의 정확한 선택 의미는 미확정이다.

**추론:** `dlvgdsTermDt/Ymd`=납품기한 날짜, `Nody`=일수, `Mtnb`=개월수,
`Cn`=기한 설명. 일수 0은 날짜 방식에서의 비사용값일 가능성이 높지만 즉시납품으로 해석할 수 없다.
**제안:** 날짜·일수·설명을 각각 보존하고 충돌 표시. 현재 계약일+일수로 날짜를 재생성하지 않는다.
S02에서 실제 유효한 기한이 어느 쪽인지는 사용자의 업무 판단이 필요한 사항이다.

### R07. 단위·인도조건·납품부대 — 직접 값과 대응 자료

| ID | 제안 용어·대응 | 판단 이유 |
|---|---|---|
| `prchsDtlItemUntVal`, `qtyUntNm`, `ctrtUntVal` | 해당 품목 수량단위 표시값 | 공고 포·대·개, 접수 m·개, 계약 개. Val도 실제 한글 단위여서 코드라고 볼 수 없음 |
| `prchsDtlItemUntValNm` | 단위명 | S13 19행에서 Val과 모두 ‘개’로 일치 |
| `ctrtUntCd` | 단위코드 후보, 현재 빈값 | 표시값으로부터 코드 생성 금지 |
| `devyCndtCd=인010004` | 납품장소 입고도 | S06·S09 계약 품목의 같은 행 `devyCndtNm` |
| `devyCndtCd=인010003` | 납품장소 하차도 | S10~S13 품목의 같은 행 `devyCndtNm` |
| `devyCndtCd=인010005` | 현장설치도 후보 | S01 동일 공고 PDF 추출 품목의 DeliveryTerms와 대응 |
| `ndfsItemDlvgdsMtruCdVal` / `ndfsPrcmItemDlvgdsMtruNm` | 국방 납품부대 코드 / 납품부대명 후보 | 접수 205행에 숫자 코드와 실제 보급창·보급대대 등의 부대명이 대응. ‘조건 구분’보다는 납품 대상 부대를 나타냄 |

접수의 `devyCndtNm`은 전부 비어 있으므로 접수 자체에서 코드·명칭 쌍을 관찰한 것은 아니다.
납품부대 코드와 수요기관 통합기관번호는 별개 ID로 보존한다.

### R08. 하자기간·선고지 여부 — 기존 확정 의미에 덧붙는 범위

공고 A는 `fctrRspbNbyr=1`, `fctrRspbMhct=0`. 계약은 연수 1 또는 2,
`fctrRspbMtnb=0`. 공고 B는 `fctrRspbText='2 년 0 개월'`이다.
연·개월을 병기하는 기간이라는 근거는 있지만, 두 값이 모두 양수인 사례는 없다.
**제안:** 승인된 연수·개월수 그대로 별도 보존하고 ‘1년 0개월’처럼 표시한다.
선택·합산 규칙까지 새로 확정하지 않는다.

접수 `advfPinfrmYn=Y`와 계약 `advcPinfrmYn=Y`는 각각 이미 확정한 비율 70과 함께 있다.
**제안:** 각각 접수/계약 단계의 ‘선급금 선고지 여부’ 후보로 대응한다.
계약은 고지가 생성되는 단계라는 사용자 설명을 적용하되 지급 완료 여부로 확대하지 않는다.

### R09. 공동수급 — 공고 조건과 실제 업체 구성을 분리

계약 5개 모두 업체표 1행, 지분율 100, `jintSyddCmnMthoCd=공500004`,
`jintSyddCmnMthoNm=단독계약`이다. 이 코드·명칭 쌍은 직접 확인됐다.
하지만 S13 공고의 `jintCtrtCmnMthoNm`은 **‘(전자)공동이행’**이다.
**추론:** 공고의 공동계약 허용/참가 방식과 계약 업체 행의 실제 구성 방식을 나타내는 서로 다른 범위일 가능성이 높다.
정확한 허용·의무 구분은 미확정이다. 특히 공고의 값을 각 계약에 복사해 공동계약으로 확정하지 않는다.

`ctentSeCd=계450001`은 모든 계약에서 같지만 단독업체만 있어 대표사/구성원 식별 코드인지 입증할 수 없다.
`jintShreSeCd`, `jintSyddRsnCd`, `cncdIntpCd`, `ptnrIntpCd`, `ptcpIntpCd`,
`ctentIntpNm`, `dtlCtknSeCd`는 모두 빈값이다. 세부 공식 용어는 근거 부족으로 후보 유지한다.
**제안:** 업체별 행·현재차수 지분·원천의 구성방식 코드/명칭은 포함한다.
실제 공동수급의 역할·분담 코드는 미확정으로 구분하며, 공고의 방식과 혼합하지 않는다.
단독계약 표본만으로 공동수급 지원 전체를 검증했다고 선언할 수는 없다.

### R10. 검사·검수 — 필드 구분은 가능, 코드 해석은 일부만 지지

`inspInstSeCd`=검사기관 구분, `igiInspInstSeCd`=검수기관 구분이라는 후보를 유지한다.
S06·S09 검사기관 구분 `검130001`과 기관명은 수요기관 자신이다(기관명 원문 생략).
S10~S12 검사기관 구분 `검130005`와 기관명은 조달청 조달품질원이다.
S13은 검사기관 구분명 ‘조달청(군수품)’, 검수기관 구분명 ‘수요기관’이다.
따라서 `검130001→수요기관`, `검130005→조달청(군수품)`이라는 추론은 가능하지만
구분 코드·구분명 직접 쌍을 가진 동일 행의 증거는 아니다.

`inspTyCd`는 계약에서 `검040015/검040012/검040010`, 접수에서는 빈값 또는 `1`이다.
S12는 한 계약 안에 `검040012`와 `검040010`이 모두 있어 **품목별 검사형태**로 다뤄야 한다.
**제안:** 검사/검수 기관과 품목별 검사형태를 구분 수집한다. 검사형태 코드별 명칭과
접수 `1`의 대응은 판단 불가로 원문 보존하며, `검130*` 기관 구분과 섞지 않는다.

### R11. 계약방법·낙찰방법 등 코드 — 근거 있는 부분만 제안

| ID·값 | 후보 | 근거·한계 |
|---|---|---|
| `stdCtrtMthdCd=계030003` | 제한경쟁 | S01 동일 공고 PDF 추출값 ContractMethod와 일치 |
| 계약 `ctrtMthdCd=계030003` | 계약방법, 제한경쟁 후보 | 연결된 접수·공고와 코드 동일. std 접두사 유무의 업무 차이는 증거 없음 |
| `ctrtTyCd=계120001` | 총액계약 | S01 동일 공고 PDF 추출값 ContractKind와 일치 |
| `scsbdMthdCd=낙030001` | 적격심사제 | S01 동일 공고 PDF 추출값 AwardMethod와 일치 |
| `scsbdMthdCd=낙030002` | 낙찰방법의 다른 코드, 명칭 미확정 | S02·S03만 존재. 절차만으로 임의 명칭을 붙이지 않음 |
| `bidMthdCd=입180002` | 전자입찰 | S01 동일 공고 PDF 추출값 BidMethod와 일치 |
| `slCprcmBsneSeCd=조070001` | 조달업무 구분 후보 | 접수·공고 A에서 상수. 구분 축을 값 비교로 알 수 없음 |
| `lcrtTyCd=계040001/계040002` | 국가계약법/지방계약법 | 사용자가 준 코드표로 확정 |
| `drpmSbpySeCd=직130001/직130002` | 직불/대지급 | 사용자가 준 코드표로 확정 |
| `feePayExmtTyCd=99` | 수수료 면제유형 후보 | 계약 전부 99. 99를 ‘해당 없음’으로 임의 확정할 수 없음 |
| `ctwtSbmsnMthdCd=계260002` | 보증서 제출방법(용어는 기존 승인) | 계약 전부 동일. 전자/서면 등 코드명 근거 없음 |
| `atcsCd` | 법령 조항 식별코드 후보 | 여러 숫자 구간과 끝 8자리 날짜 형태가 달라짐. 조항 전문을 복원할 자료는 없음 |

**제안:** 명칭을 직접 제공하는 화면은 원문 명칭을 사용한다. 근거가 있는 대응은 승인 후보로,
나머지는 원문 코드+명칭 미확정으로 구분한다. 다른 문서의 표시명을 단순 위치로 가져오지 않는다.

### R12. 기관 담당자·계약 직책 — 이름 중복은 역할의 동일성 증거가 아님

S13 `gridDmstPic`와 `gridDmstPicEvl`은 모두 1행이며 내용 전체가 같고 `evlPicYn=N`이다.
반면 같은 행의 `dmstPicNm`와 `picNm`은 서로 다르다. 따라서 두 필드를 동일인 이름의
별칭으로 합치거나 Evl 표에 있다는 이유로 평가담당자라고 확정할 수 없다.

계약 `ctrtDvtkSeNm`에는 ‘조달물자분임계약관’이라는 **직책명**이 있고,
`ctrtDvofNm/ctrtSlcfNm/ctrtDpdrNm/ctrtPicNm`에는 사람 이름 또는 이름·전화가 있다.
앞 두 사람 필드는 5계약에서 서로 같다. 전부 조직명으로 해석하는 것은 잘못이며,
같은 사람이 두 역할을 겸할 가능성도 있어 역할을 합칠 근거는 아니다.
**제안:** `ctrtPicNm`=계약담당자 후보, `ctrtDvtkSeNm`=계약관 직책명 후보.
나머지 직책과 `bfssPicNm`=사업담당자 후보는 라벨 미확정으로 두고 당장 필요하지 않은 것은 미사용 보류한다.
기존 PDF 업무연락처 범위는 유지하되 원천 역할이 다른 연락처를 덮어쓰지 않는다. 개인 연락처 예시는 이 문서에 싣지 않는다.

### R13. 공고 일정 — 명칭형 화면에서 직접 검증 가능

S13 `pbancPstgDt='2026/07/07 10:54:42'`가 일정표 ‘공고게시’ 행 `startDt`와 정확히 같다.
따라서 **공고 게시일시**라는 대응 근거가 강하다. 일정표는 입찰참가자격등록·공동수급협정서제출·
입찰보증서접수·입찰서제출·개찰을 각각 다른 `subject` 행으로 준다.
`startDt/endDt`는 시작/마감, `prgNm`은 ‘전자’·‘전자입찰’ 등 진행방법 후보다.
**제안:** 원문 일정명·방법·시작·마감을 보존한다. 공고 A의 `*Day/*Time`은 같은 업무 일정별로
짝지으며 하나가 없으면 시각을 생성하지 않는다. 제외한 `atrzDt`로 게시일시를 채우지 않는다.

### R14. 첨부문서 — 설명과 종류코드는 다름

`atchFileDscr`는 빈값, ‘문서명’, ‘PDF파일 솔루션 자동변환’이다.
따라서 이 필드를 문서유형이라고 표시하면 무의미한 설명/변환 메모가 문서종류가 된다.
`atchFileKndCd`는 `첨020114/첨020116/첨020033` 등 다양한 분류 코드여서 문서종류 코드라는 추론이 강하다.
`orgnlAtchFileNm`=원본 첨부파일명, `fileSz`=파일 크기 후보다.
**제안:** 파일명·종류코드·설명을 구분한다. 코드의 공식 문서유형명과 fileSz의 단위는 증거가 없어 추정하지 않는다.
통합첨부번호·파일순번은 원문 참조로 유지하되 변경 버전에서도 유지되는 불변키라고 단정하지 않는다.

### R15. 나머지 정책·기타유형·기술값 — 추가 문답 없이 분류만 유지

사용자 방침대로 이미 제외한 122개 필드와 기타 미사용 결정은 그대로 적용한다.
`tpSrngTrgtYn`, `skdwCndtCtrtYn`, `dtfrBizYn`, `esdacYn`, `sckrAplcnYn`,
`kimgrpYn/IncsYn`, `pmcsRtnmEvlTrgtYn`, `pmcsRtnmEvlcrtAmt`,
`ftalPrcmCtrtEntFormSeCd`, `jbizProdYn` 등의 미해석 정책값은 분류·용어 후보만 유지하고
현재 사용하지 않는다. 원산지 등 이미 유지하기로 한 값과 공동수급 상세는 이 보류로 제외하지 않는다.
pointInfo의 `vatIncsYn/vatRat`는 업체표 제외 결정이 자동 적용되지는 않지만,
당장 계산·표시에 사용하지 않고 후보 보류를 제안한다. `vatAmt`는 기존 승인대로 유지한다.

`rowStatus`, `recordCountPerPage`, `nextRowYn` 등은 업무값이 아닌 수집 진단 정보다.
S12는 표시건수 10인데 16행이 들어 있으므로 표시건수만큼 자르지 않는다.
반대로 현재 배열이 서버 전체라는 증거도 없다. 전부 빈 10개 표는 열 자체를 알 수 없어 가상 필드를 만들지 않는다.
이 항목들 때문에 용어 확인 대화를 더 늘리지 않는다.

### R16. 기존 PDF와 나머지 기본필드 — JSON 완전 대체는 아직 불가

제목·물품명·식별번호·규격·수량·금액·기관·연락처·원산지 등의 나머지 개별 ID는
[전수 매핑표](json-field-mapping.md)의 출처별 후보를 유지한다. 후보를 자동 확정하거나
미사용 보류를 기존 필수 정보에까지 확대하지 않는다. ID 이름이 명확하더라도 원문 출처는 남긴다.

S01과 동일 공고 PDF 추출값에는 **기초금액·게시일시(원문 생략)·관련 공고 3건**이 있으나 해당 JSON은 이를 완전히 제공하지 않는다.
계약 JSON 5개는 같은 계약 PDF와의 직접 대조도 아직 없다.
따라서 ‘JSON이 PDF 정보를 완전히 포함한다’는 결론은 현재 코퍼스에서 성립하지 않는다.
**제안:** JSON 중심, PDF는 누락 정보 보완·원문 확인의 보조 역할. 기존 PDF 수집항목을 버리지 않으며,
보완 연결에도 이미 승인한 명시 키 규칙을 적용한다.

현재 의미 확인의 핵심은 R01 날짜, R03 상태명, R06 충돌 처리, R09 실제 공동수급 역할,
R10~R12의 필요한 코드·담당자 역할이다. 근거가 충분한 나머지 대응은 묶어서 검토할 수 있다.
근거 없는 코드나 불필요한 값은 사용자가 모두 해독해야 할 질문으로 남기지 않는다.

## 우선 확인 순서

| 우선 | 묶음 | 결정할 것 |
|---|---|---|
| 1 | Q01~Q06 | 금액 명칭, 헤더/품목/업체 범위, 비율 단위, 공동수급·지분 |
| 2 | Q07~Q10 | 날짜·공고상태·명시 키·코드와 납품조건 |
| 3 | Q11~Q16 | 담당자 역할, 기타 금액/코드, 모호한 약어, UI 상태, 첨부 |
| 4 | Q17~Q18 | 기타 계약유형·미해석 ID, 나머지 용어 후보 전체 승인/정정 |

## Q01. 사업금액·추정가격·기초금액·품대

공고 A의 S02 표본 B `pointInfo`에서(값은 지어낸 값 — 더하기와 같음은 원문 그대로):

| ID | 값 (지어낸 값) | 용어 후보 |
|---|---:|---|
| prspPrce | 81818182 | 추정가격 |
| vatAmt | 8181818 | 부가세 |
| bizAmt | 90000000 | 사업금액(추정가격+부가세) |
| bizAmtFeeSumAmt | 90889000 | 사업금액·수수료 합계 |
| alotBgtAmt | 90889000 | 배정예산 |

공고 B S13은 `pointInfo.bizAmt`와 `gridView4[].usefAmt`가 같은 값이고
`gridView5[].baseAmt`는 그보다 작은 다른 값이다(원문 생략). 다른 표이며 같은 금액이 아니다.
접수 S04의 `totlPymtAmt`·`prspPrce`(원문 생략)는 연결 공고 S05의
사업금액·추정가격과 각각 같다. 값이 같다는 사실과 공식 명칭은 구분한다.

확인 요청:

- `bizAmt`, `bizAmtFeeSumAmt`, `alotBgtAmt`의 실제 라벨은 위 후보가 맞는가?
- `usefAmt`가 있는 표의 이름과 열 라벨은 무엇인가? 분류별 사업금액인가, 다른 용도 금액인가?
- 접수의 `totlPymtAmt`는 PDF의 **품대**에 대응하는가? 부가세 포함인가?
- `antcPrcmFee`는 예상 수수료인가 확정 수수료인가? `bgtAmt`와 배정예산의 관계는 무엇인가?

확정 답변: **사용자 확정 — Q01 일괄 승인(2026-09-27)**.

## Q02. 같은 ID의 헤더·품목·업체 범위

S06 표본 C 계약의 동일 ID가 출처에 따라 다르다(금액은 지어낸 값 — 같음과 1원 자리 버림은 원문 그대로).

| ID | pointInfo | 계약 품목표 | 업체표 grdEtpsLst |
|---|---:|---|---:|
| ctrtAmt | 7215330 | 첫 행 7215330, 3행 합계 25412607 | 열 없음 |
| ctrtGrnteAmt | 2541270 | 별도 동명 열 존재 | 0 |
| cgntTxam | 20000 | 열 없음 | 0 |
| ctrtSharAmt | 25412600 | 열 없음 | 25412600 |

계약 5개 모두 `pointInfo.ctrtAmt`는 첫 품목의 금액과 같다. 스칼라에 같은 ID가 모였다는
사실만으로 문서 전체 값이라고 볼 수 없다. 원래 component ID/fullRef는 덤프에 없다.

확인 요청:

- `pointInfo.ctrtAmt/ctrtQty/ctrtUprc`는 선택 품목 입력란의 값인가?
- `ctrtGrnteAmt`의 계약 전체·업체별·품목별 동명 필드는 각각 무엇을 의미하는가?
- 업체표의 보증금·인지세 0은 미입력, 해당 없음, 미납, 실제 0 중 무엇인가?

계약 총액의 품목 합산 후 10원 단위 절사 규칙은 이미 합의했다. 이 질문은 이를 재승인받는
것이 아니라 원천 ID의 범위 확인이다. 지분금액을 총액의 별칭으로 사용하지 않는다.

확정 답변: **부분 확정**. `cgntTxam`=물품제조계약에만 있는 인지세 부과액.
`ctrtAmt`=품목별 계약금액, `pointInfo.ctrtAmt`=선택 품목 금액(원본의 반복 출현 확인 후 사용자 조건부 승인 충족).
`pointInfo.ctrtGrnteAmt`=계약 전체의 계약보증금액.
업체표 `grdEtpsLst[].ctrtGrnteAmt`는 현재 무의미한 항목으로 보고 MVP 업무 매핑·계산·표시에서 제외한다.
업체표 `grdEtpsLst[].cgntTxam`도 제외한다. `pointInfo.cgntTxam`은 인지세 부과액으로 유지한다.
품목표 `grdCtrtLis[].ctrtGrnteAmt`도 무의미한 항목으로 확정하여 제외한다.
나머지 출처별 의미·0의 의미는 한 항목씩 확인한다.

## Q03. 보증금률·보증금액·하자·지체상금 단위

계약 `pointInfo`의 `ctrtGtnrt`는 전 표본 `"10"`, `lqdmRat`는 `"0.075"`다.
`dcmtGtnrt`는 S06/S09 `"0"`, S10~S12 `"3"`이다. `ctrtGrnteAmt`는 계약별 금액이 있다.

확인 요청:

- `ctrtGtnrt`=계약보증금률, `dcmtGtnrt`=하자보수보증금률이 맞는가?
- 비율은 모두 % 단위인가? 특히 `lqdmRat=0.075`는 **일당 0.075%**인가, 다른 단위인가?
- `ctwtSbmsnMthdCd`의 실제 라벨과 코드 뜻은 무엇인가?
- `fctrRspbNbyr`와 `fctrRspbMhct`/`fctrRspbMtnb`는 연·개월을 함께 더하는 두 칸인가,
  아니면 선택 방식에 따라 하나만 유효한가?
- 보증금액은 원문 보존 대상으로 삼는다. 금률×금액 재계산이나 반올림 규칙은 적용하지 않는다.

확정 답변: **사용자 확정 — 제시한 용어·단위 승인(2026-09-27)**.
`lqdmRat=0.075`는 일당 0.075%. 제시하지 않은 개별 코드 뜻·연/개월 적용 동작은 추정하지 않는다.

## Q04. 선고지와 지급 가능 비율의 구별

접수 S04/S07/S14는 모두 다음 조합이다.

| ID | 값 | 후보 |
|---|---|---|
| advfPinfrmYn | Y | 선급금 선고지 여부 |
| advfPinfrmPsbltyRat | 70 | 선급금 선고지 가능비율 |
| advfGivePsbltyYn | N | 선급금 지급가능 여부 |
| advfGivePsbltyRat | 0 | 선급금 지급가능 비율 |

계약은 `advcPinfrmYn=Y`, `advcPinfrmPsbltyRat=70`이며 `advfGive*`는 없다.

확인 요청: 선고지 70과 지급가능 0은 어떤 업무 차이인가? 선고지 여부가 Y여도 실제 지급가능이
N일 수 있는 조건은 무엇인가? 접수 `advf*`와 계약 `advc*`는 같은 용어의 화면별 ID인가?
이 값은 실제 지급률·지급액이 아니라 상한/가능비율인가?

확정 답변: **부분 확정**. 접수 `advfPinfrmPsbltyRat=70`은 선급금 선고지 가능 비율 70%이며 실제 지급률이 아니다.
접수 `advfGivePsbltyYn`·`advfGivePsbltyRat`는 사용자 결정으로 사용하지 않는 값으로 처리하여 제외한다.
계약 `advcPinfrmPsbltyRat`는 **실제 선급금 선고지 비율**로 확정했다.
계약에서 실제 고지가 생성되므로 접수의 가능 비율과 구별한다. 실제 지급 완료를 뜻하지 않는다.
저장 명칭은 사용자 지정에 따라 **선급금 선고지 비율**로 한다.

## Q05. 인지세 총액·업체 부담·실제 구매액

S06 `pointInfo.cgntTxam=20000`이지만 업체표는 `cgntTxam`, `incmCgntPchaAmt`,
`incmCgntAddPchaAmt`, `addCgntTxam`이 모두 0이다. S10~S12도 헤더 금액은 있지만 업체표는 0이다.
전 계약의 `dmstSpdtJintPayYn=N`, `dmstSpdtBrdnRt=0`, `dmstAcmlSpdtAmt=0`이다.

확인 요청:

- `pointInfo.cgntTxam`은 물품제조계약의 **인지세 부과액으로 확정**했다. 업체표 `cgntTxam`은 사용자 결정으로 제외하며 재확인하지 않는다.
- `incmCgntPchaAmt`는 우선 미사용으로 제외한다. 남은 항목인 `incmCgntAddPchaAmt`=추가 구매액,
  `addCgntTxam`=추가 세액이라는 구별이 맞는가?
- `dmstAcmlSpdtAmt`의 누적 범위는 현재 계약·변경계약 전체·수요기관 중 무엇인가?
- `vatIncsYn=Y`, `vatRat=10`은 해당 업체 지분금액의 세금 조건인가 계약 전체의 조건인가?

확정 답변: **부분 확정**. 업체표 `cgntTxam`과 `incmCgntPchaAmt`는 제외한다.
나머지 추가 구매액·추가 세액·기관 부담·부가세 범위는 미확정이다.
후속 사용자 일괄 결정: 추가 구매액·추가 세액(A1)과 기관 인지세 부담(B1)은 모두 제외했다.
후속 사용자 결정: 업체표 부가세 포함 여부 `vatIncsYn`·세율 `vatRat`도 미사용으로 제외했다.
업체표의 부가세 적용 범위는 재질문하지 않는다. pointInfo 동명 필드의 활용 여부는 별도 미확정이다.

## Q06. 업체 지분·전체 지분·공동수급 역할

전 계약 업체표는 1행이며 같은 행에서 `jintSyddCmnMthoCd=공500004`,
`jintSyddCmnMthoNm=단독계약`이 대응한다. 이 **한 코드·명칭 쌍만 직접 관찰**했다.

| ID | 관찰 맥락 | 확인할 의미 |
|---|---|---|
| ctrtSharRat / ctrtSharAmt | 업체별, 지분율 100과 계약별 금액 | 확정: 현재 계약차수의 해당 업체 지분율(%)·지분금액 |
| wholSharRat / wholSharAmt | S06 빈값, S09~S12 0 | 미사용 보류. 분류·용어 후보만 유지하고 현재 의미 확인은 생략 |
| ctentSeCd | 전 표본 계450001 | 대표/구성원 여부인가, 다른 계약상대자 구분인가? |
| jintShreSeCd / jintSyddRsnCd | 모두 빈값 | 정확한 공동수급 분담구분/사유 라벨 |
| cncdIntpCd / ptnrIntpCd / ptcpIntpCd | 모두 빈값 | 공동·분담·참여 이행 코드라는 추정이 맞는가? |
| ctentIntpNm / dtlCtknSeCd | 모두 빈값 | 업체별 이행명/세부 계약종류라는 추정이 맞는가? |

확인 요청: 각 ID의 실제 라벨과 대표사·구성원 식별 기준, 공동이행·분담이행·공동분담이행의
코드 및 지분율 적용 방식을 알려 달라. 금액의 현재 차수/누적 범위와 비율의 분모도 필요하다.
단독계약 값만으로 공동수급 코드사전을 만들어내지 않는다.

확정 답변: **부분 확정**. 업체표 `ctrtSharRat`·`ctrtSharAmt`는 현재 계약차수의 업체별 지분율(%)·지분금액이다.
전체지분은 미사용 보류로 전환했다. 공동수급 역할 코드 중 현재 쓰지 않는 항목은 후보만 유지하며,
실제 업체별 지분·구성방식 수집에 필요한 구분은 유지한다.

## Q07. 작성·접수·결재·게시·계약 일자

접수는 `wrtYmd`, 공고 A는 `atrzDt`, 공고 B는 `pbancPstgDt`,
계약은 `frstCtrtDt`, `ctrtDt`, `ctrtAprvYmd`가 있다.

확정: 접수 `wrtYmd`는 **접수일**이다.
미사용 결정: `atrzDt`는 의미 확인을 생략하고 제외한다. 공고 게시일시로 대체하지 않는다.
계약의 최초계약일·현재차수계약일·승인일이라는 구분이 맞는가?
PDF 접수일자에 다른 의미의 날짜를 대입하지 않도록 구별한다.

확정 답변: **부분 확정**. 접수 `wrtYmd`=접수일. 공고 `atrzDt`는 미사용으로 제외한다.
계약 날짜의 의미는 아직 미확정이다.

## Q08. 공고종류·상태·입찰진행상태

공고 A의 `pbancKndCd`는 6개 모두 `공440002`지만 `pbancSttsCd`는 다르다.

| 표본 설명(파일명 기준) | pbancSttsCd |
|---|---|
| S02/S05/S08 일반 공고 표본 | 공400001 |
| S01 변경공고 표본 | 공400002 |
| S15 취소공고 표본 | 공400003 |
| S03 재공고 표본 | 공400004 |

확인 요청: 위 코드별 공식 명칭은 무엇인가? `공440002`는 실공고를 뜻하는가?
공고 B의 `pbancKndNm`은 종류와 상태를 합친 표시인가? `bidPgstCd/Nm`은 이들과 구별되는
입찰진행상태인가? `pbancChgRsn`과 `cornPbancRsn`의 정확한 화면 용어도 확인한다.
파일명과 값의 동시 출현은 확인했지만 공식 코드 정의로 확정하지 않았다.

확정 답변: **대기**.

## Q09. 접수·공고·계약의 명시 키와 순번 단위

S04→S05 및 S07→S08의 `(ctrtDmndRcptNo, ctrtDmndRcptOrd, ctrtDmndRcptItemSqno)`는 일치한다.
반면 계약의 공고 참조는 본번호만 있고 차수가 없으며, S10~S12 품목에는 접수·공고 품목 참조가 비어 있다.
S14는 파일명의 표본 E과 관계없이 S10~S12가 참조하는 접수번호와 다른 접수다.

확인 요청:

- 위 접수 복합키는 해당 접수차수 안의 원천 행을 유일하게 가리키는가?
- `bidPbancItemSqno`는 공고 전체 순번인가 `bidClsfNo` 안의 순번인가?
- `ctrtItemSqno`, `ctrtItemSqnm`, `dlvgdsSqno`의 각각의 단위는 무엇인가?
- `ndfsPrcmPlanDlvgdsSqno`와 계약 `dlvgdsSqno`는 같은 계보의 순번인가?

누락된 차수·품목은 미연결로 둔다. 명시 키만 연결한다는 정책은 이미 확정되어 재질문하지 않는다.

확정 답변: **대기**.

## Q10. 코드·표시명과 납품조건

접수 품목표에는 `devyCndtCd`와 `devyCndtNm`이 함께 있고 공고 A에는 코드만 있다.
단위도 `prchsDtlItemUntVal`, `prchsDtlItemUntValNm`, `ctrtUntVal`, `ctrtUntCd`가 혼재한다.
계약에는 `dlvgdsTermChcSeCd`, 기한 날짜·일수·개월수가 함께 있다.

확인 요청: 단위의 Val 값은 표시명인가 코드인가? 인도조건 코드의 명칭 대응과
`inspTyCd`, 검사·검수기관 구분의 공식 명칭은 무엇인가? 납품일수 0은 기한 날짜 사용,
즉시 납품, 미지정 중 무엇인가? 날짜·일수·개월수의 선택 기준은 어느 ID인가?

확정 답변: **대기**.

## Q11. 기관 담당자와 계약 조직·직책

공고 B의 `gridDmstPic`와 `gridDmstPicEvl`은 표본에서 내용이 같지만 이름은 다르다.
`dmstPicNm`와 `picNm`이 한 행에 함께 있고 `evlPicYn`도 있다.
계약 스칼라에는 `ctrtDvtkSeNm`, `ctrtDvofNm`, `ctrtSlcfNm`, `ctrtDpdrNm`, `ctrtPicNm`이 있다.

확인 요청: 두 담당자 표의 실제 제목과 일반/평가 담당 역할은 무엇인가?
`dmstPicNm`와 `picNm`의 차이, 위 계약 조직·직책 ID의 각각의 공식 용어,
`bfssPicNm`이 사업담당자라는 추정이 맞는지 확인한다. 업무 연락처 외 시스템 로그인 ID는
그 자체로 사람을 연결하는 업무 키로 사용하지 않는다.

확정 답변: **대기**.

## Q12. 추가 계약금액·비율의 이름

**종료: B2 전체 제외 확정.** 아래 질문은 과거 조사 맥락으로만 보존하며 재질문하지 않는다.

`tlbkpngCtrtAmt`, `implBlmt`, `rsreRat`, `oncMaxDlreqamtRt`, `oncMaxDlreqAmt`는
이번 계약 스칼라에서 모두 0이다. 이름만으로 총부기금액·이행잔액·1회 최대 납품요구 한도 등을
추정할 수 있지만 실제 의미를 입증할 유효 사례가 없다. `pnpr`, `aplcnCrtrPrce`, `alotAmt`도 별도 필드다.

확인 요청: 공식 라벨, 각 금액의 현재/총괄 범위, `rsreRat`의 뜻과 단위, 0의 의미는 무엇인가?
금액·조건 MVP에는 포함하되 의미 확인 전 다른 금액의 별칭으로 합치지 않는다.

확정 답변: **대기**.

## Q13. 계약·낙찰·지급·수수료 코드사전

`slCprcmBsneSeCd`, `lcrtTyCd`, `stdCtrtMthdCd`, `ctrtMthdCd`, `ctrtTyCd`,
`scsbdMthdCd`, `drpmSbpySeCd`, `feePayExmtTyCd`, `atcsCd`를 구분했다.
공고 B의 명칭과 공고 A의 코드가 서로 다른 문서에서 관찰되므로 위치만 보고 코드명을 붙이지 않는다.

확인 요청: 실제 화면 라벨과 관찰 코드의 공식 뜻, 같은 용어의 화면별 ID인지 확인한다.
특히 계약유형/계약구분, 지급방법/직불·대지급, 계약방법 두 ID의 구별이 필요하다.

확정 답변: **대기**.

## Q14. 정책·평가 관련 모호한 약어

`tpSrngTrgtYn`, `skdwCndtCtrtYn`, `dtfrBizYn`, `esdacYn`, `sckrAplcnYn`,
`kimgrpYn`, `kimgrpIncsYn`, `pmcsRtnmEvlTrgtYn`, `pmcsRtnmEvlcrtAmt`,
`ftalPrcmCtrtEntFormSeCd`, `jbizProdYn`은 이름만으로 공식 용어를 확정하지 않았다.

확인 요청: 이 ID들의 실제 체크박스/입력란 이름은 무엇인가?
MVP 포함 여부와 무관하게 먼저 뜻을 분류한다. 빈값이나 N이라는 이유로 불필요한 정보라고 단정하지 않는다.
인접 필드·출처와 예시는 아래 부록에 있다.

확정 답변: **대기**.

## Q15. 기술 상태와 전체 수집 증거

공고 A 품목 `rowStatus=U`, 다른 대표 품목표는 R이 관찰된다.
S12의 `recordCountPerPage=10`인데 대표 품목표에는 16행이 있다.
공고 B의 `totCnt`는 품목표가 아니라 입찰진행상태 표에 있다.

확인 요청: U/R은 이 화면에서 수정/조회라는 뜻인가, 단순 내부 행 상태인가?
`nextRowYn`과 표시건수는 어떤 역할인가? 업무항목으로 저장하는 대신 수집 진단용으로 분리한다.
값만 보고 서버 전체 품목을 모두 수집했다고 선언하지 않는다. 이 기술 질문의 미확정은
코퍼스 필드 분류를 중단하는 사유가 아니다.

확정 답변: **대기**.

## Q16. 첨부문서 유형과 표시 단위

접수에는 첨부표가 없고 공고·계약에는 동적 `wq_uuid_*_grdFile` 표가 있다.
`atchFileDscr`, `atchFileKndCd`, `orgnlAtchFileNm`, `fileSz`가 함께 있다.

확인 요청: 문서유형에 해당하는 것은 설명인가 종류코드인가? 코드별 명칭과 파일 크기 단위는 무엇인가?
통합첨부번호·파일순번은 같은 첨부의 변경 버전에서도 유지되는가?
서버 경로·세션 사용자·IP·다운로드 권한을 업무문서 내용으로 저장하지 않는다.

확정 답변: **대기**.

## Q17. 외자·다른 계약유형·미해석 ID

계약 품목표는 물품 외에 부동산·유가증권·보험·리스로 보이는 열까지 포함한다.
주민번호 성격 열은 실제 값이 비어 있지만 수집 제외로 분류했다. 원산지·외자·운송은 별도 분류했고,
전부 빈 열과 0인 숫자 열을 구분했다.

확인 요청: 해당 분류가 맞는지, 현재 물품 업무에서도 반드시 필요한 열이 있는지 확인한다.
원화로 추정한 `rcry*`를 포함해 외자 약어는 공식 사전이 없으므로 후보다.
아래 미해석 ID는 정확한 용어를 별도로 알려 달라. 확인 전 새 저장 필드를 만들지 않는다.

확정 답변: **대기**.

## Q18. 나머지 후보의 일괄 승인·정정과 빈 표

Q01~Q17에 속하지 않는 모든 ID도 전체 매핑표에서 **용어 후보**로 남겨 두었다.
물품명·담당자·일정처럼 이해하기 쉬운 이름도 공식 라벨 대조를 완료했다고 표시하지 않는다.
해당 행의 ID를 지정해 수정하거나 분류별로 후보를 승인할 수 있다.

전 표본 0행인 표는 열 ID가 없으므로 가상의 매핑을 만들지 않았다. 표의 실제 제목·역할만 먼저 확인한다.
일반 표와 Excel 표의 중복은 확인했으며, 개별 `*Excel` 보조열은 원래 열의 별칭인지 별도 값인지 미확정이다.

확정 답변: **대기**.

<!-- GENERATED_CONTEXT_START -->

## 부록 A. 모호한 ID의 출처·주변 맥락

인접 필드는 JSON 객체의 저장 순서에서 앞뒤 두 ID다. 실제 화면 배치나 공식 용어의 증거로 보지 않는다.
아래는 Q14·미해석 ID와 Q18 중 뜻이 특히 불분명한 항목이다. Q01~Q13·Q15~Q17의 나머지 열은 전체 매핑표의 해당 ID에서 값·타입·빈값과 출처를 볼 수 있다.

| 질문 | ID | 후보 | 출처·표본 | 관찰 맥락 | 인접 ID |
|---|---|---|---|---|---|
| Q18 | `areaCd` | 화면/영역 코드(업무 의미 미확정) | 접수 · `pointInfo` · S04,S07,S14 | string; 관찰 3, 빈값 0, 0 0; 원문 생략 | `depth1`, `depth2` |
| Q18 | `areaCd` | 화면/영역 코드(업무 의미 미확정) | 공고 A · 코드형/탭 · `pointInfo` · S01,S02,S03,S05,S08,S15 | string; 관찰 6, 빈값 0, 0 0; 원문 생략 | `depth1`, `depth2` |
| Q18 | `areaCd` | 화면/영역 코드(업무 의미 미확정) | 공고 B · 명칭형/요약 · `pointInfo` · S13 | string; 관찰 1, 빈값 0, 0 0; 원문 생략 | `depth1`, `depth2` |
| Q18 | `areaCd` | 화면/영역 코드(업무 의미 미확정) | 계약 · `pointInfo` · S06,S09,S10,S11,S12 | string; 관찰 5, 빈값 0, 0 0; 원문 생략 | `depth1`, `depth2` |
| Q17 | `drctEctnIdrpIdntYn` | 용어 미확정 — 약어만으로 번역하지 않음 | 계약 · `mf_wfm_container_grdEtpsLst` · S06,S09,S10,S11,S12 | string; 관찰 5, 빈값 0, 0 0; "N", "Y" | `dutyObsrIdrpIdntYn`, `eltsbAgreYn`, `idrpIdntYn`, `wtotIdntYn` |
| Q14 | `dtfrBizYn` | 용어 미확정 — 약어만으로 번역하지 않음 | 공고 A · 코드형/탭 · `pointInfo` · S01,S02,S03,S05,S08,S15 | string; 관찰 6, 빈값 0, 0 0; "N" | `bizAmtFeeSumAmt`, `evlMaagSeCd`, `evlSysUtztnYn`, `igiInspInstSeCd`, `inspInstSeCd`, `vatAmt` |
| Q17 | `dutyObsrIdrpIdntYn` | 용어 미확정 — 약어만으로 번역하지 않음 | 계약 · `mf_wfm_container_grdEtpsLst` · S06,S09,S10,S11,S12 | string; 관찰 5, 빈값 0, 0 0; "N", "Y" | `drctEctnIdrpIdntYn`, `eltsbAgreYn`, `idrpIdntYn`, `ocfmUntyAtchFileNo` |
| Q18 | `ernnEtpsAddr` | 해외 관련업체 연락처/주소(정확한 역할 미확정) | 계약 · `mf_wfm_container_grdEtpsLst` · S06,S09,S10,S11,S12 | string; 관찰 5, 빈값 5, 0 0; 비어 있음 | `ernnEtpsFaxNo`, `ernnEtpsTlphNo`, `giveAcntBankCd`, `giveActno` |
| Q18 | `ernnEtpsCtrtEctnGrnteAmt` | 관련 해외업체 계약이행보증금액(업체 역할 미확정) | 계약 · `mf_wfm_container_grdEtpsLst` · S06,S09,S10,S11,S12 | number; 관찰 5, 빈값 0, 0 5; 0 | `ernnEtpsCtrtEctnGtnrt`, `shpgAphrCd`, `shpgTrnpMthdCd`, `spplEtpsCtrtEctnGrnteAmt` |
| Q18 | `ernnEtpsCtrtEctnGtnrt` | 관련 해외업체 계약이행보증금률(업체 역할 미확정) | 계약 · `mf_wfm_container_grdEtpsLst` · S06,S09,S10,S11,S12 | number; 관찰 5, 빈값 0, 0 5; 0 | `ernnEtpsCtrtEctnGrnteAmt`, `shpgTrnpMthdCd`, `spplEtpsCtrtEctnGrnteAmt`, `spplEtpsCtrtEctnGtnrt` |
| Q18 | `ernnEtpsFaxNo` | 해외 관련업체 연락처/주소(정확한 역할 미확정) | 계약 · `mf_wfm_container_grdEtpsLst` · S06,S09,S10,S11,S12 | string; 관찰 5, 빈값 5, 0 0; 비어 있음 | `ernnEtpsAddr`, `ernnEtpsTlphNo`, `ernnEtpsUntyGrpNo`, `giveAcntBankCd` |
| Q18 | `ernnEtpsTlphNo` | 해외 관련업체 연락처/주소(정확한 역할 미확정) | 계약 · `mf_wfm_container_grdEtpsLst` · S06,S09,S10,S11,S12 | string; 관찰 5, 빈값 5, 0 0; 비어 있음 | `ernnEtpsAddr`, `ernnEtpsFaxNo`, `ernnEtpsUntyGrpNo`, `rprsvNm` |
| Q18 | `ernnEtpsUntyGrpNo` | 해외 관련업체 통합기관번호(정확한 역할 미확정) | 계약 · `mf_wfm_container_grdEtpsLst` · S06,S09,S10,S11,S12 | string; 관찰 5, 빈값 5, 0 0; 비어 있음 | `ctentSeCd`, `ernnEtpsFaxNo`, `ernnEtpsTlphNo`, `rprsvNm` |
| Q14 | `esdacYn` | 용어 미확정 — 약어만으로 번역하지 않음 | 공고 A · 코드형/탭 · `pointInfo` · S01,S02,S03,S05,S08,S15 | string; 관찰 6, 빈값 0, 0 0; "N" | `atmtCntt01`, `bfssPicNm`, `bidClsfNo`, `ctrtDmndRcptNo` |
| Q14 | `ftalPrcmCtrtEntFormSeCd` | 조달 계약체결 형태구분(약어 미확정) | 공고 A · 코드형/탭 · `pointInfo` · S01,S02,S03,S05,S08,S15 | string; 관찰 6, 빈값 0, 0 0; "기170012" | `fprdYn`, `lcnsLmtYn`, `rgnLmtYn`, `smrlCmptItemYn` |
| Q18 | `g2bYn` | 나라장터 관련 여부(정확한 용도 미확정) | 계약 · `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` · S06,S09,S10,S11,S12 | string; 관찰 26, 빈값 0, 0 0; "N" | `hefcEtamYn`, `kimgrpYn`, `specAtrbCn`, `usagNm` |
| Q17 | `idrpIdntYn` | 용어 미확정 — 약어만으로 번역하지 않음 | 계약 · `mf_wfm_container_grdEtpsLst` · S06,S09,S10,S11,S12 | string; 관찰 5, 빈값 0, 0 0; "Y" | `drctEctnIdrpIdntYn`, `dutyObsrIdrpIdntYn`, `itgtCtratIdntYn`, `wtotIdntYn` |
| Q18 | `itemCfnm` | 물품명 후보(정확한 용어 미확정) | 접수 · `mf_wfm_container_gridView` · S04,S07,S14 | string; 관찰 205, 빈값 0, 0 0; 원문 생략 | `ctrtDmndQty`, `ctrtDmndUprc`, `itemClsfNo`, `itemIdnfNm` |
| Q17 | `itgtCtratIdntYn` | 용어 미확정 — 약어만으로 번역하지 않음 | 계약 · `mf_wfm_container_grdEtpsLst` · S06,S09,S10,S11,S12 | string; 관찰 5, 빈값 0, 0 0; "Y" | `giveMmrdIdntYn`, `idrpIdntYn`, `rspnsDt`, `wtotIdntYn` |
| Q14 | `jbizProdYn` | 중소기업제품 관련 여부(정확한 라벨 미확정) | 공고 A · 코드형/탭 · `pointInfo` · S01,S02,S03,S05,S08,S15 | string; 관찰 6, 빈값 0, 0 0; "N" | `fprdYn`, `infoLeakPrhbTrgtYn`, `lrcoPtcpLmtExcpYn`, `smrlCmptItemYn` |
| Q14 | `kimgrpIncsYn` | 특정 제품군 여부/포함 여부(약어 미확정) | 계약 · `pointInfo` · S06,S09,S10,S11,S12 | string; 관찰 5, 빈값 0, 0 0; "N" | `itemClsfNo`, `itemIdnfNo`, `schlMlsvFdmtYn`, `sftyInspYn` |
| Q14 | `kimgrpYn` | 특정 제품군 여부/포함 여부(약어 미확정) | 계약 · `pointInfo` · S06,S09,S10,S11,S12 | string; 관찰 5, 빈값 0, 0 0; "N" | `inspTyCd`, `odn1PlorNtnNm` |
| Q14 | `kimgrpYn` | 특정 제품군 여부/포함 여부(약어 미확정) | 계약 · `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` · S06,S09,S10,S11,S12 | string; 관찰 26, 빈값 0, 0 0; "N" | `g2bYn`, `hefcEtamYn`, `hskCd`, `usagNm` |
| Q18 | `lgqtDlgdiExclYn` | 장기 납품 관련 제외 여부(약어 미확정) | 계약 · `mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis` · S06,S09,S10,S11,S12 | string; 관찰 26, 빈값 26, 0 0; 비어 있음 | `inptDt`, `kbrdrId`, `nextRowYn`, `rowStatus` |
| Q18 | `ndfsItemDlvgdsMtruCdVal` | 국방 품목 납품 관련 구분 코드/명(약어 미확정) | 접수 · `mf_wfm_container_gridView` · S04,S07,S14 | string; 관찰 205, 빈값 0, 0 0; 원문 생략 | `albmNopg`, `ndfsPrcmPlanDlvgdsSqno`, `ndpfExpndDmndNoVal`, `qtyUntNm` |
| Q18 | `ndfsPrcmItemDlvgdsMtruNm` | 국방 품목 납품 관련 구분 코드/명(약어 미확정) | 접수 · `mf_wfm_container_gridView` · S04,S07,S14 | string; 관찰 205, 빈값 0, 0 0; 원문 생략 | `ndfsPrcmPlanDlvgdsSqno`, `ndpfExpndDmndNoVal`, `rowStatus`, `rtrcnYn` |
| Q17 | `nflcNm` | 용어 미확정 — 약어만으로 번역하지 않음 | 계약 · `mf_wfm_container_grdEtpsLst` · S06,S09,S10,S11,S12 | string; 관찰 5, 빈값 5, 0 0; 비어 있음 | `agncFee`, `avtsmtBankNm`, `frgtWbilSeCd`, `trmtPsbltyYn` |
| Q18 | `optnItmltDsgnYn` | 선택 품목 지정 여부(약어 미확정) | 계약 · `pointInfo` · S06,S09,S10,S11,S12 | string; 관찰 5, 빈값 0, 0 0; "N" | `instDlayNody`, `instDlayRsn`, `ofdcCn`, `schlMlsvFdmtYn`, `sftyInspYn` |
| Q14 | `pmcsRtnmEvlTrgtYn` | 평가대상 여부(약어 미확정) | 공고 A · 코드형/탭 · `pointInfo` · S02,S03 | string; 관찰 2, 빈값 0, 0 0; "N" | `dfrlScrAplcnYn`, `dfrlScrItvlScr`, `pmcsRtnmEvlcrtAmt`, `prsntnYn` |
| Q14 | `pmcsRtnmEvlcrtAmt` | 평가기준 금액(약어 미확정) | 공고 A · 코드형/탭 · `pointInfo` · S02,S03 | string; 관찰 2, 빈값 0, 0 2; 원문 생략 | `dfrlScrItvlScr`, `pmcsRtnmEvlTrgtYn`, `prsntnTmmn`, `prsntnYn` |
| Q18 | `procsCtrtWtotAgreYn` | 계약 진행 동의 여부(정확한 용어 미확정) | 계약 · `pointInfo` · S06,S09,S10,S11,S12 | string; 관찰 5, 빈값 0, 0 0; "Y" | `ctrtDpdrNm`, `ctrtPicNm`, `dmstPicId`, `dmstUntyGrpNm` |
| Q18 | `prvctImplYn` | 수의계약 사유 관련 여부(약어 미확정) | 계약 · `pointInfo` · S06,S09,S10,S11,S12 | string; 관찰 5, 빈값 0, 0 0; "N" | `inspInstSeCd`, `lotmYn`, `newLotmSeCd`, `prflcAplcnCrtrCd` |
| Q14 | `sckrAplcnYn` | 용어 미확정 — 약어만으로 번역하지 않음 | 공고 A · 코드형/탭 · `pointInfo` · S01,S02,S03,S05,S08,S15 | string; 관찰 6, 빈값 0, 0 0; "N" | `coexStlmTrgtYn`, `grpNgtnCtrtBizClsfCd`, `ngtnCtrtBizClsfCd`, `stdPbadmsReqNody` |
| Q14 | `skdwCndtCtrtYn` | 조건부 계약 여부(조건 약어 미확정) | 공고 A · 코드형/탭 · `pointInfo` · S01,S02,S03,S05,S08,S15 | string; 관찰 6, 빈값 0, 0 0; "N" | `bidPbancNm`, `bsneMixtCtrtYn`, `mixtCtrtAffcClclnYn`, `rschDvlpItemYn` |
| Q14 | `tpSrngTrgtYn` | 심사대상 여부(약어 미확정) | 접수 · `pointInfo` · S04,S07,S14 | string; 관찰 3, 빈값 0, 0 0; "N" | `clsfCrtrBidYn`, `egnrEvlTrgtYn`, `pbancKndCd`, `scsbdMthdCd` |
| Q14 | `tpSrngTrgtYn` | 심사대상 여부(약어 미확정) | 공고 A · 코드형/탭 · `pointInfo` · S01,S05,S08,S15 | string; 관찰 4, 빈값 0, 0 0; "N" | `atcsCd`, `egnrEvlTrgtYn`, `scsbdSrngCrtrNo`, `scsbdSrngDtlsCrtrSqno` |
| Q17 | `wtotIdntYn` | 용어 미확정 — 약어만으로 번역하지 않음 | 계약 · `mf_wfm_container_grdEtpsLst` · S06,S09,S10,S11,S12 | string; 관찰 5, 빈값 0, 0 0; "Y" | `drctEctnIdrpIdntYn`, `giveMmrdIdntYn`, `idrpIdntYn`, `itgtCtratIdntYn` |

## 부록 B. 열을 확인할 수 없는 빈 표

| 화면형 | 실제 표 ID | 표본 | 확인 요청 |
|---|---|---|---|
| 공고 A · 코드형/탭 | `mf_wfm_container_tabCont_contents_itemTabs2_body_wframe6_grdAliasDmTtl06List` | S01,S02,S03,S05,S08,S15 | 실제 화면 제목·업무 역할 확인. 0행이므로 열 ID와 매핑은 미확정 |
| 공고 A · 코드형/탭 | `mf_wfm_container_tabCont_contents_itemTabs2_body_wframe7_grdAliasDmTtl07RightList` | S01,S02,S03,S05,S08,S15 | 실제 화면 제목·업무 역할 확인. 0행이므로 열 ID와 매핑은 미확정 |
| 공고 A · 코드형/탭 | `mf_wfm_container_tabCont_contents_itemTabs2_body_wframe11_grdBidPrcLmtMfrcFld` | S01,S02,S03,S05,S08,S15 | 실제 화면 제목·업무 역할 확인. 0행이므로 열 ID와 매핑은 미확정 |
| 공고 A · 코드형/탭 | `mf_wfm_container_tabCont_contents_itemTabs2_body_wframe10_grdAliasDmTtl10List` | S01,S02,S03,S05,S08,S15 | 실제 화면 제목·업무 역할 확인. 0행이므로 열 ID와 매핑은 미확정 |
| 계약 | `mf_wfm_container_grdSldrGrnteEtpsLst` | S06,S09,S10,S11,S12 | 실제 화면 제목·업무 역할 확인. 0행이므로 열 ID와 매핑은 미확정 |
| 계약 | `mf_wfm_container_tacCtrt_contents_content2_body_grdKmciKndInfo` | S06,S09,S10,S11,S12 | 실제 화면 제목·업무 역할 확인. 0행이므로 열 ID와 매핑은 미확정 |
| 공고 B · 명칭형/요약 | `mf_wfm_container_mainWframe_grdLcnsLmt` | S13 | 실제 화면 제목·업무 역할 확인. 0행이므로 열 ID와 매핑은 미확정 |
| 공고 B · 명칭형/요약 | `mf_wfm_container_mainWframe_grdCbiShrt` | S13 | 실제 화면 제목·업무 역할 확인. 0행이므로 열 ID와 매핑은 미확정 |
| 공고 B · 명칭형/요약 | `mf_wfm_container_mainWframe_grdPrpsDmndInfoView` | S13 | 실제 화면 제목·업무 역할 확인. 0행이므로 열 ID와 매핑은 미확정 |
| 공고 B · 명칭형/요약 | `mf_wfm_container_mainWframe_grdEtspUntyGrp` | S13 | 실제 화면 제목·업무 역할 확인. 0행이므로 열 ID와 매핑은 미확정 |

## 부록 C. 사용자가 답변할 기록 형식

| 질문 | 출처 / ID | 확정 용어 | 코드·단위·적용 범위 | 결정 |
|---|---|---|---|---|
| Q__ |  |  |  | 대기 |

namespace Pclm.Core.Storage;

/// <summary>
/// 스키마 정본.
///
/// <para><b>DB가 유일한 진실</b>이다. 원본 문서는 담지 않는다 — 자료는 확장 수집·ERP JSON·
/// 계획 엑셀로 들어와 값으로만 쌓인다(ADR-028).</para>
///
/// <para><b>차수는 자연키의 일부</b>다. 변경공고·변경계약은 갱신이 아니라 별개의 행으로 쌓이고,
/// 최신분은 뷰가 <c>MAX(seq)</c> 로 고른다. 이력 테이블이 따로 필요 없다.</para>
///
/// <para><b>금액은 TEXT로 담는다.</b> 단가에 소수점이 실재해서(<c>54,957,446.667</c>)
/// REAL 로 두면 부동소수점 오차가 계약 금액에 섞인다.</para>
///
/// <para><b>판올림은 앞으로만 간다.</b> 빈 파일은 <see cref="Baseline"/> 하나로 v19 를 곧장 짓고,
/// 그 뒤의 변경은 <see cref="Steps"/> 에 v20 부터 한 단계씩 덧붙인다 — 이미 나간 단계는 고치지
/// 않는다. 기준선보다 옛 판(v1~v18)은 정식판 이전의 시험판이라 이 프로그램이 올리지 않는다.
/// 0.7.0 이 그 열아홉 단계를 마지막으로 들고 있던 판이다.</para>
/// </summary>
public static class Schema
{
    /// <summary>스키마 판. 올릴 때마다 <see cref="Steps"/> 에 단계를 하나 더하고 이것도 하나 올린다.</summary>
    public const int Version = 20;

    /// <summary><see cref="Baseline"/> 이 짓는 판. 이보다 옛 판은 받지 않는다.</summary>
    public const int BaselineVersion = 19;

    /// <summary>
    /// 기준선 뒤의 단계. <c>Steps[0]</c> 이 v19 → v20 이다.
    ///
    /// <para>단계를 더할 때는 <b>끝에만</b> 덧붙인다. 앞의 것을 고치면 이미 그 단계를 지난 파일과
    /// 갓 지은 파일의 스키마가 조용히 갈린다 — <c>contract/schema.txt</c> 박제가 그것을 붙든다.
    /// 빈 파일도 기준선 뒤에 이것을 밟으므로 둘은 언제나 같은 길을 지난다.</para>
    /// </summary>
    public static readonly IReadOnlyList<string> Steps = [V20];

    /// <summary>
    /// v19 → v20. <b>파일이 스스로 PCLM 임과 역할을 밝힌다</b>(ADR-031).
    ///
    /// <para>그전에는 일반 SQLite 와 이 프로그램의 자료를 가를 표지가 없었고, 제출본은 작업자료를
    /// 그대로 뜬 것이라 같은 <c>dataset_id</c> 를 달고 나갔다. 취합본은 파일 이름 접두사로만 가렸다.
    /// 이제 <c>application_id</c> 가 SQLite 머리에 PCLM 을 박고, <c>pclm_file.role</c> 이 이 파일이
    /// 작업자료인지 제출본·취합본·백업·옮겨진 옛 자료인지 적는다 — 쓰기를 허락할지는 역할이 정한다.</para>
    ///
    /// <para><c>erp_dataset.environment</c> 는 따라오지 않는다. 확장 저장 대상이 개발용인지 업무용인지는
    /// 자료가 아니라 <b>그 자료를 연 호스트</b>의 사정이라, 파일에 적어 두면 파일을 옮길 때 거짓이 된다.
    /// <c>dataset_id</c> 는 그대로 옮긴다 — 확장이 들고 다니는 미리보기가 판올림 하나에 끊기면 안 된다.</para>
    ///
    /// <para><c>created_at</c> 은 코드의 다른 시각과 같은 꼴(<c>DateTime.ToString("O")</c>, UTC 7자리 소수)로
    /// 적는다. SQLite 의 <c>%f</c> 는 밀리초 셋째 자리까지라 뒤에 0 넷을 붙여 자리를 맞춘다.</para>
    ///
    /// <para><c>application_id</c> 는 <c>user_version</c> 과 같이 머리의 한 칸이라 트랜잭션 안에서 바꿔도
    /// 함께 커밋되고 함께 되돌려진다 — 판올림이 반쯤 실패해 "PCLM 인데 표가 없는" 파일이 남지 않는다.</para>
    /// </summary>
    private const string V20 = """
        CREATE TABLE pclm_file (
            singleton  INTEGER PRIMARY KEY CHECK(singleton = 1),
            dataset_id TEXT NOT NULL,
            role       TEXT NOT NULL CHECK(role IN ('work', 'submission', 'merged', 'backup', 'retired')),
            created_at TEXT NOT NULL
        );

        -- 이름표가 없던 파일은 없지만(기준선이 심는다), 비어 있어도 판올림이 멎지 않게 새로 뽑는다.
        INSERT INTO pclm_file (singleton, dataset_id, role, created_at)
        VALUES (1,
                COALESCE((SELECT dataset_id FROM erp_dataset WHERE singleton = 1), lower(hex(randomblob(16)))),
                'work',
                strftime('%Y-%m-%dT%H:%M:%f0000Z', 'now'));

        DROP TABLE erp_dataset;

        -- 'PCLM' (0x50434C4D). PRAGMA 는 매개변수를 받지 않는다 — 상수다.
        PRAGMA application_id = 1346587725;
        """;

    /// <summary>
    /// 정식판 이전의 시험판으로 지은 판인가. <c>0</c> 은 아직 아무것도 짓지 않은 빈 파일이라 여기
    /// 들지 않는다.
    /// </summary>
    public static bool IsPreBaseline(int version) => version is > 0 and < BaselineVersion;

    /// <summary>
    /// 빈 파일에 v19 를 곧장 짓는다.
    ///
    /// <para><b>표의 정의는 옛 열아홉 단계가 남긴 글자 그대로다.</b> SQLite 는 <c>sqlite_master</c> 에
    /// 정의 원문을 담아 두는데, 갓 지은 파일과 단계를 밟아 올라온 파일이 그 글자까지 같아야 둘을
    /// 같은 판이라 부를 수 있다. 그래서 <c>ALTER TABLE ... ADD COLUMN</c> 이 줄 끝에 이어 붙인 열,
    /// <c>RENAME</c> 이 남긴 따옴표 이름까지 손대지 않았다 — <c>contract/schema.txt</c> 가 한 글자씩
    /// 대조한다. 정의 안의 <c>--</c> 설명도 원문의 일부라 고치지 않는다.</para>
    ///
    /// <para><b>참조를 전부 복합키로 쓰고 모든 자식에 외래키를 건다.</b> 차수 키
    /// <c>(base, seq)</c> 를 이어붙인 문자열로 들고 다니면 외래키를 걸 수 없고, 잇는 규칙이 코드
    /// 여기저기 흩어져 한 군데만 어긋나도 조용히 안 맞는다. 이어붙인 문자열은 <b>보여줄 때만</b>
    /// 뷰에서 만든다.</para>
    ///
    /// <para><b>기계가 읽은 사실은 차수 키에, 사람이 붙인 것은 계열 키에</b> 매단다(ADR-012).
    /// 계열(<c>*_series</c>)은 차수를 벗은 키다. 뷰는 <c>MAX(seq)</c> 만 내므로, 손으로 적은 값과
    /// 링크가 차수에 매달려 있으면 변경계약 한 건에 옛 차수에 남아 오류 없이 빈 칸이 된다.
    /// 계열은 파생 표라 업서트가 <c>INSERT OR IGNORE</c> 로 채운다 — 본표에서 계열로 외래키를
    /// 걸지 않았다.</para>
    ///
    /// <para><b>공고건(<c>notice_group</c>)</b>은 계열 위에 한 칸 더 선 묶음이다. 취소 후 재공고는
    /// 본번호가 갈려 차수로는 영영 이어지지 않는데, 접수도 계약도 하나다. 접수·계약 링크를 건에
    /// 매달아 한 조달 건이 두 줄로 서지 않게 한다. 건의 이름은 <b>가장 이른 본번호</b>다 — 재공고서는
    /// 자기가 대신하는 번호를 적어 두므로 문서가 없는 조상도 이름이 될 수 있고, 최소값으로 두면 당초
    /// 공고가 나중에 들어와도 이름이 흔들리지 않는다. 건은 <see cref="NoticeGroups.Rebuild"/> 가
    /// 관련공고를 타고 짓는다. <c>notice_series.group_base</c> 에 캐스케이드를 걸지 않는 것은
    /// <c>notice_group</c> → <c>notice_series</c> → <c>notice_user_field</c> 로 두 칸 이어져 건을
    /// 다시 짓는 것만으로 사람 메모가 지워지기 때문이다.</para>
    ///
    /// <para><b>관련공고(<c>notice_relation</c>)는 담되 판정하지 않는다</b>(ADR-025). 값이 여럿이라
    /// 차수에 매단 딸린 표다. <c>related</c> 는 적힌 그대로 남고, <c>related_base</c>·
    /// <c>related_seq</c> 는 C# 한 군데(<c>ValueParser.SplitNoticeRef</c>)가 해소해 채운다 —
    /// 해소하지 못하면 NULL 이다.</para>
    ///
    /// <para><b>접수의 키는 접수번호·접수차수다</b>(ADR-027). <c>request_link.notice_group</c> 을
    /// UNIQUE 로 두는 것은 뜻이 있는 선택이다 — 접수 1 : 건 1 을 DB 가 지켜, 한 건에 접수 둘이
    /// 붙으면 <c>v_통합</c> 의 줄이 조용히 두 배가 되는 대신 넣는 자리에서 시끄럽게 실패한다.
    /// <c>request_item_link</c> 는 수량·단가 다중집합이 같을 때 지퍼처럼 짝지은 <b>결과</b>라 링크를
    /// 풀면 함께 걷는다(ADR-021).</para>
    ///
    /// <para><b>링크에는 누가 이었고 무엇을 근거로 삼았는지</b>(<c>decided_by</c>·<c>evidence</c>·
    /// <c>rule_version</c>)를 남긴다. 규칙을 고칠 때 자동 링크만 골라 다시 판정하고 사람이 확정한
    /// 것은 건드리지 않으려면 둘이 갈려 있어야 한다. 사람이 <b>아니라고 판정한 짝</b>은
    /// <c>*_link_rejection</c> 에 따로 둔다 — 링크 표는 키가 한쪽 하나라 짝마다 생기는 거부를 담지
    /// 못하고, 버리면 물리친 후보가 다음에도 맨 위에 다시 뜬다.</para>
    ///
    /// <para><b>덮개(<c>field_override</c>)</b>는 사람이 기계가 읽은 값을 고칠 자리다(ADR-020).
    /// 본표를 직접 고치면 문서에 적힌 값과 사람의 판단이 한 칸에 섞여 가릴 수도 되돌릴 수도 없다.
    /// 사람 값과 달리 <b>차수에 매단다</b> — "이 문서의 이 칸을 이렇게 읽어야 한다" 는 정정이라,
    /// 1차에서 고친 금액이 2차에 얹히면 확인한 적 없는 숫자가 확인된 얼굴로 찍힌다.
    /// <c>column_name</c> 은 표의 열이 아니라 <b>뷰의 한글 열 이름</b>이고, 뷰가 표기까지 마친 값
    /// 바깥에 씌운다. <c>value</c> 는 빈 문자열도 유효한 값이라 되돌리기는 행을 지우는 쪽이 맡는다.
    /// <c>original</c> 은 <b>처음 고칠 때</b> 그 자리에 있던 값이다 — 두 번째 편집에서 다시 적으면
    /// "직전에 내가 적은 것" 이 되므로 넣을 때만 적고, 빈 칸이었으면 빈 문자열이라 NOT NULL 이다.
    /// 세 종류를 한 표에 담아 외래키를 걸지 못하므로 지울 때 손으로 함께 지운다
    /// (<see cref="Store.Delete"/>).</para>
    ///
    /// <para><b>사람이 채우는 열</b>은 정의(<c>user_column</c>)와 값(<c>*_user_field</c>)으로 나눈다 —
    /// 열을 늘려도 스키마를 고칠 필요가 없다. 수집은 값 표에 쓰지 않는다(ADR-007). 처음 세우는
    /// 열은 어디서나 쓰는 둘(진행상태·메모, 공고는 검토여부·메모)뿐이고 후보 목록 없이 <c>text</c>
    /// 다 — 쓰지 않는 열을 미리 세우면 표만 넓어지고, 후보를 정해 두면 그 밖의 상태가 생길 때마다
    /// 화면이 토를 단다. 필요하면 사람이 더하거나 <c>choice</c> 로 바꾼다.</para>
    ///
    /// <para><b>계획(<c>plan</c>)</b>에는 차수가 없다. 지난 계획을 문서로 내놓을 일이 없어 계획변경은
    /// 같은 조달요구번호를 <b>덮어쓰는 것</b>으로 본다. 열은 표본 서식(「조달계획 (양식 표본).xlsx」)의
    /// 머리글 그대로라 대응표가 있을 자리가 없다(ADR-023). 수량만 INTEGER 인 것은
    /// <c>request_item.quantity</c> 와 형을 맞추기 위해서다. <c>source_name</c>·<c>imported_at</c> 은
    /// 같은 번호가 다른 파일에서 다시 올 때 마지막이 어느 파일이었는지 남긴다.</para>
    ///
    /// <para><b>수집(<c>erp_*</c>)</b>. <c>erp_dataset</c> 은 이 DB 의 이름표로, <c>dataset_id</c> 를
    /// 지을 때 한 번 무작위로 뽑는다(v20 에서 <c>pclm_file</c> 로 옮겨 간다 — <see cref="V20"/>). <c>erp_capture</c> 는 수집 영수증이라 같은 수집 ID 로 다시
    /// 물으면 그때 돌려준 것을 그대로 돌려준다. <c>erp_source</c>·<c>erp_row</c> 는 세 종류를 한
    /// 표에 담아 외래키를 걸지 못하므로 방아쇠가 개체와 함께 지운다.</para>
    ///
    /// <para><b>설정(<c>app_setting</c>)</b>을 DB 에 두는 것은 DB 와 설정이 따로 옮겨 다니며
    /// 어긋나지 않게 하려는 것이다. 열을 늘리지 않고 키-값으로 둔다.</para>
    /// </summary>
    public const string Baseline = """
        -- ── 공고 ───────────────────────────────────────────────────────
        CREATE TABLE notice (
            notice_base              TEXT NOT NULL,
            seq                      TEXT NOT NULL,
            title                    TEXT,
            notice_kind              TEXT,
            posted_at                TEXT,
            bid_method               TEXT,
            award_method             TEXT,
            award_criteria           TEXT,
            lowest_bid_ratio         TEXT,
            contract_method          TEXT,
            contract_kind            TEXT,
            international_kind       TEXT,
            creditor                 TEXT,
            rebid_allowed            INTEGER,
            partial_delivery_allowed INTEGER,
            warranty_months          INTEGER,
            price_estimation_method  TEXT,
            project_amount           TEXT,
            allocated_budget         TEXT,
            estimated_price          TEXT,
            base_price               TEXT,
            notice_agency            TEXT,
            notice_officer           TEXT,
            executive_officer        TEXT,
            prior_spec_number        TEXT,
            region_restriction       TEXT,
            performance_restricted   INTEGER,
            manufacturing_kind       TEXT,
            foreign_allowed          INTEGER,
            research_item            INTEGER,
            source_sha256            TEXT,
            updated_at               TEXT NOT NULL, warranty_month_part TEXT, bid_opens_at TEXT, bid_closes_at TEXT, opening_at TEXT, registration_closes_at TEXT, change_reason TEXT, vat TEXT, source_status TEXT, delivery_term_text TEXT, warranty_years TEXT, warranty_text TEXT,
            PRIMARY KEY (notice_base, seq)
        );

        CREATE TABLE "notice_item" (
            notice_base        TEXT NOT NULL,
            seq                TEXT NOT NULL,
            line_no            INTEGER NOT NULL,
            demand_agency      TEXT,
            item_name          TEXT,
            detail_item_number TEXT,
            item_id_number     TEXT,
            specification      TEXT,
            quantity           INTEGER,
            unit               TEXT,
            unit_price         TEXT,
            delivery_days      INTEGER,
            delivery_place     TEXT,
            delivery_terms     TEXT, delivery_due TEXT, ref_request_base TEXT, ref_request_seq TEXT, ref_request_item TEXT, source_class TEXT, source_item TEXT,
            PRIMARY KEY (notice_base, seq, line_no),
            FOREIGN KEY (notice_base, seq) REFERENCES notice(notice_base, seq) ON DELETE CASCADE
        );

        CREATE TABLE "notice_schedule" (
            notice_base TEXT NOT NULL,
            seq         TEXT NOT NULL,
            line_no     INTEGER NOT NULL,
            name        TEXT,
            method      TEXT,
            starts_at   TEXT,
            ends_at     TEXT,
            place       TEXT,
            PRIMARY KEY (notice_base, seq, line_no),
            FOREIGN KEY (notice_base, seq) REFERENCES notice(notice_base, seq) ON DELETE CASCADE
        );

        -- 수요기관 담당자. 기관이 여럿일 수 있어 딸린 표다 — 공고 뷰에는 첫 줄만 붙인다.
        -- 공고를 낸 조달청 담당자(notice.notice_officer)와는 다른 사람이라 합치지 않는다.
        CREATE TABLE notice_officer_contact (
            notice_base   TEXT NOT NULL,
            seq           TEXT NOT NULL,
            line_no       INTEGER NOT NULL,
            demand_agency TEXT,
            department    TEXT,
            officer       TEXT,
            fax           TEXT,
            phone         TEXT,
            PRIMARY KEY (notice_base, seq, line_no),
            FOREIGN KEY (notice_base, seq) REFERENCES notice(notice_base, seq) ON DELETE CASCADE
        );

        -- 관련공고. 담되 판정하지 않는다(ADR-025).
        CREATE TABLE notice_relation (
            notice_base TEXT NOT NULL,
            seq         TEXT NOT NULL,
            line_no     INTEGER NOT NULL,  -- 적힌 차례. 원문은 최신에서 과거 순으로 적는다
            related     TEXT NOT NULL, related_base TEXT, related_seq  TEXT,     -- 관련공고 번호. 꼴을 검사하지 않고 적힌 그대로 담는다
            PRIMARY KEY (notice_base, seq, line_no),
            FOREIGN KEY (notice_base, seq) REFERENCES notice(notice_base, seq) ON DELETE CASCADE
        );

        -- ── 계약 ───────────────────────────────────────────────────────
        -- 계약상대자. 사업자등록번호가 자연키다. 주민등록번호는 담지 않는다(ADR-005).
        CREATE TABLE counterparty (
            business_number TEXT PRIMARY KEY,
            name            TEXT,
            representative  TEXT,
            address         TEXT,
            phone           TEXT,
            fax             TEXT,
            updated_at      TEXT NOT NULL
        );

        CREATE TABLE contract (
            contract_base              TEXT NOT NULL,
            seq                        TEXT NOT NULL,
            title                      TEXT,
            contracted_on              TEXT,
            contract_method            TEXT,
            law_clause                 TEXT,
            goods_or_service           TEXT,
            purchase_management_number TEXT,
            agency_management_number   TEXT,
            contract_kind              TEXT,
            contract_character         TEXT,
            terminated                 INTEGER,
            terminated_on              TEXT,
            item_name                  TEXT,
            quantity                   INTEGER,
            unit                       TEXT,
            amount                     TEXT,
            fee                        TEXT,
            delay_penalty_rate         TEXT,
            warranty_bond_rate         TEXT,
            warranty_period            TEXT,
            contract_period            TEXT,
            delivery_due               TEXT,
            delivery_terms             TEXT,
            delivery_place             TEXT,
            partial_delivery_allowed   INTEGER,
            payment_method             TEXT,
            demand_agency              TEXT,
            inspection_agency          TEXT,
            acceptance_agency          TEXT,
            counterparty_number        TEXT REFERENCES counterparty(business_number),
            source_sha256              TEXT,
            updated_at                 TEXT NOT NULL, first_contracted_on TEXT, guarantee_rate TEXT, guarantee_amount TEXT, guarantee_method TEXT, stamp_tax TEXT, advance_notice TEXT, advance_notice_rate TEXT, warranty_years TEXT, warranty_months TEXT, ref_request_base TEXT, ref_notice_base TEXT, contract_officer TEXT, finance_officer TEXT, expenditure_officer TEXT, staff TEXT,
            PRIMARY KEY (contract_base, seq)
        );

        CREATE TABLE "contract_item" (
            contract_base         TEXT NOT NULL,
            seq                   TEXT NOT NULL,
            line_no               INTEGER NOT NULL,
            classification_number TEXT,
            item_id_number        TEXT,
            item_name             TEXT,
            specification         TEXT,
            region                TEXT,
            unit                  TEXT,
            delivery_terms        TEXT,
            quantity              INTEGER,
            unit_price            TEXT,
            amount                TEXT,
            demand_agency         TEXT,
            delivery_due          TEXT, ref_request_base TEXT, ref_request_seq TEXT, source_item TEXT,
            PRIMARY KEY (contract_base, seq, line_no),
            FOREIGN KEY (contract_base, seq) REFERENCES contract(contract_base, seq) ON DELETE CASCADE
        );

        CREATE TABLE "contract_attachment" (
            contract_base TEXT NOT NULL,
            seq           TEXT NOT NULL,
            line_no       INTEGER NOT NULL,
            document_type TEXT,
            file_name     TEXT,
            PRIMARY KEY (contract_base, seq, line_no),
            FOREIGN KEY (contract_base, seq) REFERENCES contract(contract_base, seq) ON DELETE CASCADE
        );

        -- ── 접수 ───────────────────────────────────────────────────────
        CREATE TABLE request (
            request_base      TEXT NOT NULL,   -- 접수번호 (V18 부터. 그 전에는 대표 요청번호)
            seq               TEXT NOT NULL,   -- 접수차수
            title             TEXT,
            received_on       TEXT,
            business_kind     TEXT,
            contract_law      TEXT,
            contract_method   TEXT,
            contract_kind     TEXT,
            award_method      TEXT,
            accounting_kind   TEXT,
            payment_method    TEXT,
            goods_amount      TEXT,            -- 품대. 공고의 사업금액에 대응한다
            fee               TEXT,
            vat               TEXT,
            budget_amount     TEXT,            -- 예산금액. 공고의 배정예산에 대응한다
            foreign_allowed   INTEGER,
            request_kind      TEXT,
            disclosure        TEXT,
            executive_officer TEXT,
            department        TEXT,
            officer           TEXT,
            advance_notice    INTEGER,
            advance_payment   INTEGER,
            remarks           TEXT,
            demand_agency     TEXT,
            demand_agency_code TEXT,
            agency_officer    TEXT,
            agency_phone      TEXT,
            agency_fax        TEXT,
            source_sha256     TEXT,
            updated_at        TEXT NOT NULL, advance_notice_rate TEXT,
            PRIMARY KEY (request_base, seq)
        );

        CREATE TABLE request_item (
            request_base       TEXT NOT NULL,
            seq                TEXT NOT NULL,
            line_no            INTEGER NOT NULL,
            request_number     TEXT,           -- 요청번호 = 조달요구번호. 이 줄의 정체다
            plan_year          TEXT,
            change_seq         TEXT,
            detail_item_number TEXT,
            item_id_number     TEXT,
            item_name          TEXT,
            cancelled          INTEGER,
            specification      TEXT,
            inspection_kind    TEXT,
            unit_price         TEXT,
            quantity           INTEGER,
            amount             TEXT,
            unit               TEXT,
            pages              INTEGER,
            delivery_terms     TEXT,
            delivery_days      TEXT,           -- "계약후 90일 이내" 처럼 문장이 온다
            delivery_due       TEXT,
            delivery_place     TEXT,
            stock_number       TEXT,
            spec_number        TEXT,
            expense_request_number TEXT, ref_request_base TEXT, ref_request_seq TEXT, ref_request_item TEXT,
            PRIMARY KEY (request_base, seq, line_no),
            FOREIGN KEY (request_base, seq) REFERENCES request(request_base, seq) ON DELETE CASCADE
        );

        -- ── 건과 계열: 차수를 벗은 키. 사람이 붙인 것과 링크가 여기 매달린다 ──
        CREATE TABLE notice_group (group_base TEXT PRIMARY KEY);

        CREATE TABLE "notice_series" (
            notice_base TEXT PRIMARY KEY,
            -- 캐스케이드를 걸지 않는다. 걸면 notice_user_field 까지 두 칸 이어져 사람 메모가 지워진다.
            group_base  TEXT NOT NULL REFERENCES notice_group(group_base)
        );

        CREATE TABLE contract_series (contract_base TEXT PRIMARY KEY);

        CREATE TABLE request_series (request_base TEXT PRIMARY KEY);

        -- ── 사람이 붙인 것. 수집은 여기 쓰지 않는다(ADR-007·ADR-020) ──────
        CREATE TABLE "notice_user_field" (
            notice_base TEXT NOT NULL,
            field_name  TEXT NOT NULL,
            value       TEXT,
            updated_at  TEXT NOT NULL,
            PRIMARY KEY (notice_base, field_name),
            FOREIGN KEY (notice_base) REFERENCES notice_series(notice_base) ON DELETE CASCADE
        );

        CREATE TABLE "contract_user_field" (
            contract_base TEXT NOT NULL,
            field_name    TEXT NOT NULL,
            value         TEXT,
            updated_at    TEXT NOT NULL,
            PRIMARY KEY (contract_base, field_name),
            FOREIGN KEY (contract_base) REFERENCES contract_series(contract_base) ON DELETE CASCADE
        );

        CREATE TABLE request_user_field (
            request_base TEXT NOT NULL,
            field_name   TEXT NOT NULL,
            value        TEXT,
            updated_at   TEXT NOT NULL,
            PRIMARY KEY (request_base, field_name),
            FOREIGN KEY (request_base) REFERENCES request_series(request_base) ON DELETE CASCADE
        );

        -- 열 정의는 참조가 아니라 종류 이름이라 외래키 대신 값을 제한한다.
        CREATE TABLE "user_column" (
            entity_type TEXT NOT NULL CHECK (entity_type IN ('contract', 'notice', 'request')),
            field_name  TEXT NOT NULL,
            kind        TEXT NOT NULL DEFAULT 'text' CHECK (kind IN ('text', 'choice', 'date', 'number')),
            options     TEXT,
            sort_order  INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (entity_type, field_name)
        );

        -- 덮개. 사람 값과 달리 차수에 매단다.
        CREATE TABLE "field_override" (
            entity_type TEXT NOT NULL CHECK (entity_type IN ('notice', 'contract', 'request')),
            base        TEXT NOT NULL,
            seq         TEXT NOT NULL,
            column_name TEXT NOT NULL,
            value       TEXT NOT NULL,
            original    TEXT NOT NULL,
            updated_at  TEXT NOT NULL,
            PRIMARY KEY (entity_type, base, seq, column_name)
        );

        -- ── 링크. 끝점은 계열과 건이다 ───────────────────────────────────
        -- 계약 계열 하나는 건 하나만 가리킨다(건 1 : 계약 N).
        CREATE TABLE "project_link" (
            contract_base TEXT PRIMARY KEY,
            notice_group  TEXT NOT NULL,
            confidence    REAL,
            confirmed_at  TEXT,
            decided_by    TEXT NOT NULL DEFAULT 'human',
            evidence      TEXT,
            rule_version  INTEGER,
            FOREIGN KEY (contract_base) REFERENCES contract_series(contract_base) ON DELETE CASCADE,
            FOREIGN KEY (notice_group)  REFERENCES notice_group(group_base)       ON DELETE CASCADE
        );

        -- 사람이 물리친 짝. 계약 하나에 여럿 달릴 수 있어 project_link 에 담기지 않는다.
        CREATE TABLE "link_rejection" (
            contract_base TEXT NOT NULL,
            notice_group  TEXT NOT NULL,
            rejected_at   TEXT NOT NULL,
            PRIMARY KEY (contract_base, notice_group),
            FOREIGN KEY (contract_base) REFERENCES contract_series(contract_base) ON DELETE CASCADE,
            FOREIGN KEY (notice_group)  REFERENCES notice_group(group_base)       ON DELETE CASCADE
        );

        -- notice_group 을 UNIQUE 로 조인다. 접수 1 : 건 1 을 DB 가 지킨다(ADR-021).
        CREATE TABLE "request_link" (
            request_base TEXT PRIMARY KEY,
            notice_group TEXT NOT NULL UNIQUE,
            confidence   REAL,
            confirmed_at TEXT,
            decided_by   TEXT NOT NULL DEFAULT 'human',
            evidence     TEXT,
            rule_version INTEGER,
            FOREIGN KEY (request_base) REFERENCES request_series(request_base) ON DELETE CASCADE,
            FOREIGN KEY (notice_group) REFERENCES notice_group(group_base)     ON DELETE CASCADE
        );

        CREATE TABLE "request_link_rejection" (
            request_base TEXT NOT NULL,
            notice_group TEXT NOT NULL,
            rejected_at  TEXT NOT NULL,
            PRIMARY KEY (request_base, notice_group),
            FOREIGN KEY (request_base) REFERENCES request_series(request_base) ON DELETE CASCADE,
            FOREIGN KEY (notice_group) REFERENCES notice_group(group_base)     ON DELETE CASCADE
        );

        -- 휴리스틱이 낸 짝. 어느 조달요구가 어느 공고 품목이 되었는가.
        CREATE TABLE request_item_link (
            request_base   TEXT NOT NULL,
            request_seq    TEXT NOT NULL,
            line_no        INTEGER NOT NULL,
            notice_base    TEXT NOT NULL,
            notice_seq     TEXT NOT NULL,
            notice_line_no INTEGER NOT NULL,
            PRIMARY KEY (request_base, request_seq, line_no),
            FOREIGN KEY (request_base, request_seq) REFERENCES request(request_base, seq) ON DELETE CASCADE,
            FOREIGN KEY (notice_base, notice_seq)   REFERENCES notice(notice_base, seq)   ON DELETE CASCADE
        );

        -- ── 계획. 차수 없이 조달요구번호 하나가 자연키다 ───────────────────
        CREATE TABLE "plan" (
            request_number      TEXT PRIMARY KEY,  -- 조달요구번호. 계획의 자연키
            stock_number        TEXT,              -- 재고번호
            item_name           TEXT,              -- 품명
            currency            TEXT,              -- 화폐구분
            unit                TEXT,              -- 단위
            quantity            INTEGER,           -- 지시수량
            requesting_unit     TEXT,              -- 요청부대부서명
            requesting_officer  TEXT,              -- 요청부대담당자
            requesting_phone    TEXT,              -- 요청부대사용자전화번호
            contract_department TEXT,              -- 계약부서
            officer             TEXT,              -- 담당자
            contact             TEXT,              -- 연락처
            source_name         TEXT NOT NULL,
            imported_at         TEXT NOT NULL
        );

        -- ── 수집 ───────────────────────────────────────────────────────
        CREATE TABLE erp_dataset (
            singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
            dataset_id TEXT NOT NULL,
            environment TEXT NOT NULL DEFAULT 'unbound'
        );

        CREATE TABLE erp_capture (
            capture_id TEXT PRIMARY KEY,
            request_hash TEXT NOT NULL,
            content_hash TEXT NOT NULL,
            profile TEXT NOT NULL,
            entity_base TEXT NOT NULL,
            entity_seq TEXT NOT NULL,
            snapshot_json TEXT NOT NULL,
            result_json TEXT NOT NULL,
            captured_at TEXT NOT NULL
        );

        CREATE TABLE erp_mapping (revision TEXT PRIMARY KEY, json TEXT NOT NULL, active INTEGER NOT NULL DEFAULT 0,
            validated INTEGER NOT NULL DEFAULT 0, samples TEXT NOT NULL DEFAULT '[]', created_at TEXT NOT NULL);

        CREATE TABLE erp_source (entity_type TEXT NOT NULL, entity_base TEXT NOT NULL, entity_seq TEXT NOT NULL,
            document_json TEXT NOT NULL, mapping_revision TEXT NOT NULL, updated_at TEXT NOT NULL,
            PRIMARY KEY(entity_type,entity_base,entity_seq));

        CREATE TABLE erp_row (entity_type TEXT NOT NULL, entity_base TEXT NOT NULL, entity_seq TEXT NOT NULL,
            table_name TEXT NOT NULL, source_key TEXT NOT NULL, line_no INTEGER NOT NULL,
            PRIMARY KEY(entity_type,entity_base,entity_seq,table_name,source_key));

        CREATE TABLE erp_notice_attachment (notice_base TEXT NOT NULL, seq TEXT NOT NULL, line_no INTEGER NOT NULL,
            document_type TEXT, file_name TEXT, PRIMARY KEY(notice_base,seq,line_no),
            FOREIGN KEY(notice_base,seq) REFERENCES notice(notice_base,seq) ON DELETE CASCADE);

        CREATE TABLE erp_partner (contract_base TEXT NOT NULL, seq TEXT NOT NULL, line_no INTEGER NOT NULL,
            source_id TEXT, name TEXT, business_number TEXT, representative TEXT, address TEXT, phone TEXT, fax TEXT,
            share_rate TEXT, share_amount TEXT, PRIMARY KEY(contract_base,seq,line_no),
            FOREIGN KEY(contract_base,seq) REFERENCES contract(contract_base,seq) ON DELETE CASCADE);

        -- ── 설정. 설정 하나 늘 때마다 판올림하지 않게 키-값으로 둔다 ────────
        CREATE TABLE app_setting (
            key   TEXT PRIMARY KEY,
            value TEXT
        );

        -- ── 색인 ───────────────────────────────────────────────────────
        CREATE INDEX ix_notice_title    ON notice(title);

        CREATE INDEX ix_contract_title  ON contract(title);

        CREATE INDEX ix_request_item_number ON request_item(request_number);

        CREATE INDEX ix_request_item_link_notice ON request_item_link(notice_base, notice_seq);

        CREATE INDEX ix_project_link_group ON project_link(notice_group);

        -- 뷰의 「현행공고」가 이 열 짝으로 자기를 가리키는 행을 찾는다.
        CREATE INDEX ix_notice_relation_ref ON notice_relation(related_base, related_seq);

        CREATE UNIQUE INDEX erp_mapping_active ON erp_mapping(active) WHERE active=1;

        -- ── 방아쇠. 수집 원천은 개체가 지워질 때 함께 간다 ───────────────
        CREATE TRIGGER erp_request_deleted AFTER DELETE ON request BEGIN
         DELETE FROM erp_row WHERE entity_type='request' AND entity_base=OLD.request_base AND entity_seq=OLD.seq;
         DELETE FROM erp_source WHERE entity_type='request' AND entity_base=OLD.request_base AND entity_seq=OLD.seq;
         END;

        CREATE TRIGGER erp_notice_deleted AFTER DELETE ON notice BEGIN
         DELETE FROM erp_row WHERE entity_type='notice' AND entity_base=OLD.notice_base AND entity_seq=OLD.seq;
         DELETE FROM erp_source WHERE entity_type='notice' AND entity_base=OLD.notice_base AND entity_seq=OLD.seq;
         END;

        CREATE TRIGGER erp_contract_deleted AFTER DELETE ON contract BEGIN
         DELETE FROM erp_row WHERE entity_type='contract' AND entity_base=OLD.contract_base AND entity_seq=OLD.seq;
         DELETE FROM erp_source WHERE entity_type='contract' AND entity_base=OLD.contract_base AND entity_seq=OLD.seq;
         END;

        -- ── 씨앗 ───────────────────────────────────────────────────────
        -- 처음 세우는 사람 열. 마음에 안 들면 지우거나 더하면 된다.
        INSERT INTO user_column (entity_type, field_name, kind, options, sort_order) VALUES
            ('contract', '진행상태', 'text', NULL, 20),
            ('contract', '메모',     'text', NULL, 40),
            ('notice',   '검토여부', 'text', NULL, 10),
            ('notice',   '메모',     'text', NULL, 20);

        -- 이 DB 의 이름표. 지을 때 한 번 뽑는다.
        INSERT INTO erp_dataset (singleton, dataset_id) VALUES (1, lower(hex(randomblob(16))));
        """;
}

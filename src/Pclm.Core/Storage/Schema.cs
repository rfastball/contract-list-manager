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
/// </summary>
public static class Schema
{
    /// <summary>스키마 판. 올릴 때마다 <see cref="Migrations"/> 에 단계를 더한다.</summary>
    public const int Version = 19;

    public static readonly IReadOnlyList<string> Migrations =
        [V1, V2, V3, V4, V5, V6, V7, V8, V9, V10, V11, V12, V13, V14, V15, V16, V17, V18, V19];

    /// <summary>
    /// <b>PDF 입구를 걷는다</b>(ADR-028).
    ///
    /// <para><b>왜.</b> ADR-027 이 접수서 PDF 를 막은 뒤 남은 공고·계약 PDF 도 확장이 같은 자료를
    /// 화면에서 더 정확히 가져오게 되어, 두 입구를 함께 유지할 값이 없어졌다. 자료는 이제 확장
    /// 수집·ERP JSON·계획 엑셀로만 들어온다.</para>
    ///
    /// <para><b>걷는 것.</b> <c>document</c>(가져오기 기록 — 원본 경로·지문·읽지 못한 문서)는 표째
    /// 걷는다. 색인은 표와 함께 간다. PDF 가져오기에만 쓰이던 설정 셋(가져올 폴더·포함 키워드·다시
    /// 가져올 때 덮어쓰기)도 <c>app_setting</c> 에서 지운다. 저장해 둔 수집 매핑에서는 <c>pdf-*</c>
    /// 프로필을 걷는다 — 프로필 구성이 기본 매핑과 한 벌이어야 매핑이 읽히므로(<c>Mapping.Validate</c>),
    /// 남겨 두면 활성 매핑을 읽는 순간 확장 수집이 통째로 멎는다.</para>
    ///
    /// <para><b>남기는 것.</b> PDF 로 들어온 공고·계약은 공고번호·계약번호라는 진짜 키로 선 자료라
    /// 그대로 둔다 — 링크·메모·덮개도 함께다. <c>erp_source</c>·<c>erp_row</c>·<c>erp_capture</c> 에
    /// 남은 <c>pdf-*</c> 프로필 이름과 <c>pdf:</c> 행 키는 지난 관찰 기록이라 고치지 않는다.</para>
    ///
    /// <para><see cref="V18"/> 은 <c>document</c> 를 딛고 돌므로 그대로 둔다 — 역사 기록이고,
    /// V17 이하의 자료는 V18 을 지나 여기서 걷힌다.</para>
    /// </summary>
    private const string V19 = """
        DROP TABLE IF EXISTS document;

        DELETE FROM app_setting
        WHERE key IN ('source_folder', 'import_keywords', 'overwrite_on_reingest');

        UPDATE erp_mapping SET json = json_set(json, '$.profiles',
            (SELECT json_group_array(json(p.value)) FROM json_each(erp_mapping.json, '$.profiles') p
             WHERE json_extract(p.value, '$.id') NOT LIKE 'pdf-%'))
        WHERE EXISTS (SELECT 1 FROM json_each(erp_mapping.json, '$.profiles') p
                      WHERE json_extract(p.value, '$.id') LIKE 'pdf-%');
        """;

    /// <summary>
    /// <b>접수의 키를 접수번호로</b>(ADR-027).
    ///
    /// <para><b>왜.</b> 접수서 PDF 에는 접수번호가 찍히지 않아 <see cref="V11"/> 은 품목의 최소
    /// 요청번호를 대표로 골라 키를 세웠다. 확장이 화면에서 진짜 접수번호를 읽게 되자 같은 접수가
    /// 두 이름을 갖게 되었고, ERP 수집분은 겹치지 않게 <c>ERP:</c> 를 달고 따로 섰다. 둘을 잇는
    /// <c>erp_request_alias</c> 는 사람이 순서를 지켜야만 동작해, 지키지 않으면 <b>조용히 두 벌이
    /// 섰다</b>. 이제 접수 키는 접수번호·접수차수 그대로이고, 접수서 PDF 는 접수를 세우지 않는다.</para>
    ///
    /// <para><b>옮기는 규칙은 셋이다.</b> <c>ERP:X</c> 는 <c>X</c> 로. 별칭으로 ERP 접수에 이어진
    /// PDF 접수는 그 ERP 접수번호로 — 단 같은 <c>ERP:X</c> 가 따로 서 있으면 그쪽이 이긴다(키가
    /// 둘이 될 수 없다). 그 밖의 것(PDF 에서만 온 접수)은 <b>지운다</b> — 접수번호를 모르니 옮길
    /// 자리가 없다. 지우는 범위는 <see cref="Store.Delete"/> 와 같다: 계열·차수·품목·링크·거부·
    /// 사람 값·덮개·수집 원천, 그리고 그 접수서의 <c>document</c> 행. <c>document</c> 를 남기면
    /// 같은 파일을 다시 넣을 때 무엇이 막혔는지 이력이 흐려진다.</para>
    ///
    /// <para><b>외래키가 꺼진 채 돈다</b>(<c>Database.Migrate</c>). 캐스케이드가 일하지 않으므로
    /// 딸린 표를 손으로 하나씩 걷고 옮긴다. 다 옮긴 뒤 <c>AssertForeignKeysHold</c> 가 확인한다.</para>
    ///
    /// <para><c>erp_capture</c> 는 수집 영수증이라 <c>result_json</c> 을 고치지 않는다 — 같은 수집
    /// ID 로 다시 물으면 그때 돌려준 것을 그대로 돌려주는 것이 영수증의 약속이다. 이력 목록이
    /// 보여 주는 <c>entity_base</c> 만 새 키로 옮긴다.</para>
    /// </summary>
    private const string V18 = """
        -- ── 접수 키가 나타나는 모든 자리를 모은다 ─────────────────────
        CREATE TEMP TABLE v18_base (old_base TEXT PRIMARY KEY);
        INSERT OR IGNORE INTO v18_base SELECT request_base FROM request_series;
        INSERT OR IGNORE INTO v18_base SELECT request_base FROM request;
        INSERT OR IGNORE INTO v18_base SELECT base FROM field_override WHERE entity_type = 'request';
        INSERT OR IGNORE INTO v18_base SELECT entity_base FROM erp_row WHERE entity_type = 'request';
        INSERT OR IGNORE INTO v18_base SELECT entity_base FROM erp_source WHERE entity_type = 'request';

        -- new_base 가 NULL 이면 지운다.
        CREATE TEMP TABLE v18_key (old_base TEXT PRIMARY KEY, new_base TEXT);
        INSERT INTO v18_key (old_base, new_base)
        SELECT b.old_base,
               CASE
                   WHEN substr(b.old_base, 1, 4) = 'ERP:' THEN substr(b.old_base, 5)
                   ELSE (SELECT substr(a.erp_base, 5) FROM erp_request_alias a
                         WHERE a.pdf_base = b.old_base AND substr(a.erp_base, 1, 4) = 'ERP:'
                           AND NOT EXISTS (SELECT 1 FROM v18_base x WHERE x.old_base = a.erp_base))
               END
        FROM v18_base b;

        -- ── PDF 에서만 온 접수를 걷는다. Store.Delete 와 같은 범위다 ─────────
        DELETE FROM document
        WHERE form_type = 'request' AND entity_key_text IS NOT NULL
          AND EXISTS (SELECT 1 FROM v18_key k WHERE k.new_base IS NULL
                      AND substr(document.entity_key_text, 1, length(k.old_base) + 1) = k.old_base || '-');
        DELETE FROM request_item_link      WHERE request_base IN (SELECT old_base FROM v18_key WHERE new_base IS NULL);
        DELETE FROM request_item           WHERE request_base IN (SELECT old_base FROM v18_key WHERE new_base IS NULL);
        DELETE FROM request_link           WHERE request_base IN (SELECT old_base FROM v18_key WHERE new_base IS NULL);
        DELETE FROM request_link_rejection WHERE request_base IN (SELECT old_base FROM v18_key WHERE new_base IS NULL);
        DELETE FROM request_user_field     WHERE request_base IN (SELECT old_base FROM v18_key WHERE new_base IS NULL);
        DELETE FROM field_override WHERE entity_type = 'request'
          AND base IN (SELECT old_base FROM v18_key WHERE new_base IS NULL);
        DELETE FROM erp_row WHERE entity_type = 'request'
          AND entity_base IN (SELECT old_base FROM v18_key WHERE new_base IS NULL);
        DELETE FROM erp_source WHERE entity_type = 'request'
          AND entity_base IN (SELECT old_base FROM v18_key WHERE new_base IS NULL);
        DELETE FROM request        WHERE request_base IN (SELECT old_base FROM v18_key WHERE new_base IS NULL);
        DELETE FROM request_series WHERE request_base IN (SELECT old_base FROM v18_key WHERE new_base IS NULL);
        DELETE FROM v18_key WHERE new_base IS NULL OR new_base = old_base;

        -- ── 남은 것을 접수번호로 옮긴다 ─────────────────────────────
        UPDATE request_series SET request_base = (SELECT new_base FROM v18_key WHERE old_base = request_base)
        WHERE request_base IN (SELECT old_base FROM v18_key);
        UPDATE request SET request_base = (SELECT new_base FROM v18_key WHERE old_base = request_base)
        WHERE request_base IN (SELECT old_base FROM v18_key);
        UPDATE request_item SET request_base = (SELECT new_base FROM v18_key WHERE old_base = request_base)
        WHERE request_base IN (SELECT old_base FROM v18_key);
        UPDATE request_user_field SET request_base = (SELECT new_base FROM v18_key WHERE old_base = request_base)
        WHERE request_base IN (SELECT old_base FROM v18_key);
        UPDATE request_link SET request_base = (SELECT new_base FROM v18_key WHERE old_base = request_base)
        WHERE request_base IN (SELECT old_base FROM v18_key);
        UPDATE request_link_rejection SET request_base = (SELECT new_base FROM v18_key WHERE old_base = request_base)
        WHERE request_base IN (SELECT old_base FROM v18_key);
        UPDATE request_item_link SET request_base = (SELECT new_base FROM v18_key WHERE old_base = request_base)
        WHERE request_base IN (SELECT old_base FROM v18_key);
        UPDATE field_override SET base = (SELECT new_base FROM v18_key WHERE old_base = base)
        WHERE entity_type = 'request' AND base IN (SELECT old_base FROM v18_key);
        UPDATE erp_row SET entity_base = (SELECT new_base FROM v18_key WHERE old_base = entity_base)
        WHERE entity_type = 'request' AND entity_base IN (SELECT old_base FROM v18_key);
        -- 원천 문서에도 키가 한 벌 적혀 있다. 읽는 쪽은 rows 만 보지만 두 벌이 갈리지 않게 함께 고친다.
        UPDATE erp_source SET
            document_json = json_set(document_json, '$.base', (SELECT new_base FROM v18_key WHERE old_base = entity_base)),
            entity_base   = (SELECT new_base FROM v18_key WHERE old_base = entity_base)
        WHERE entity_type = 'request' AND entity_base IN (SELECT old_base FROM v18_key);
        UPDATE erp_capture SET entity_base = (SELECT new_base FROM v18_key WHERE old_base = entity_base)
        WHERE profile LIKE '%-request-%' AND entity_base IN (SELECT old_base FROM v18_key);
        UPDATE document SET entity_key_text =
            (SELECT k.new_base || substr(document.entity_key_text, length(k.old_base) + 1) FROM v18_key k
             WHERE substr(document.entity_key_text, 1, length(k.old_base) + 1) = k.old_base || '-')
        WHERE form_type = 'request' AND entity_key_text IS NOT NULL
          AND EXISTS (SELECT 1 FROM v18_key k
                      WHERE substr(document.entity_key_text, 1, length(k.old_base) + 1) = k.old_base || '-');

        -- ── 별칭은 이제 있을 자리가 없다 ────────────────────────────
        DROP TABLE erp_request_alias;
        DROP TABLE v18_key;
        DROP TABLE v18_base;
        """;

    private const string V17 = """
        ALTER TABLE notice ADD COLUMN warranty_month_part TEXT;
        ALTER TABLE notice ADD COLUMN bid_opens_at TEXT;
        ALTER TABLE notice ADD COLUMN bid_closes_at TEXT;
        ALTER TABLE notice ADD COLUMN opening_at TEXT;
        ALTER TABLE notice ADD COLUMN registration_closes_at TEXT;
        ALTER TABLE notice ADD COLUMN change_reason TEXT;
        CREATE TABLE erp_notice_attachment (notice_base TEXT NOT NULL, seq TEXT NOT NULL, line_no INTEGER NOT NULL,
            document_type TEXT, file_name TEXT, PRIMARY KEY(notice_base,seq,line_no),
            FOREIGN KEY(notice_base,seq) REFERENCES notice(notice_base,seq) ON DELETE CASCADE);
        ALTER TABLE request ADD COLUMN advance_notice_rate TEXT;
        ALTER TABLE request_item ADD COLUMN ref_request_base TEXT;
        ALTER TABLE request_item ADD COLUMN ref_request_seq TEXT;
        ALTER TABLE request_item ADD COLUMN ref_request_item TEXT;
        ALTER TABLE notice ADD COLUMN vat TEXT;
        ALTER TABLE notice ADD COLUMN source_status TEXT;
        ALTER TABLE notice ADD COLUMN delivery_term_text TEXT;
        ALTER TABLE notice ADD COLUMN warranty_years TEXT;
        ALTER TABLE notice_item ADD COLUMN ref_request_base TEXT;
        ALTER TABLE notice_item ADD COLUMN ref_request_seq TEXT;
        ALTER TABLE notice_item ADD COLUMN ref_request_item TEXT;
        ALTER TABLE notice_item ADD COLUMN source_class TEXT;
        ALTER TABLE notice_item ADD COLUMN source_item TEXT;
        ALTER TABLE notice ADD COLUMN warranty_text TEXT;
        ALTER TABLE contract ADD COLUMN first_contracted_on TEXT;
        ALTER TABLE contract ADD COLUMN guarantee_rate TEXT;
        ALTER TABLE contract ADD COLUMN guarantee_amount TEXT;
        ALTER TABLE contract ADD COLUMN guarantee_method TEXT;
        ALTER TABLE contract ADD COLUMN stamp_tax TEXT;
        ALTER TABLE contract ADD COLUMN advance_notice TEXT;
        ALTER TABLE contract ADD COLUMN advance_notice_rate TEXT;
        ALTER TABLE contract ADD COLUMN warranty_years TEXT;
        ALTER TABLE contract ADD COLUMN warranty_months TEXT;
        ALTER TABLE contract ADD COLUMN ref_request_base TEXT;
        ALTER TABLE contract ADD COLUMN ref_notice_base TEXT;
        ALTER TABLE contract ADD COLUMN contract_officer TEXT;
        ALTER TABLE contract ADD COLUMN finance_officer TEXT;
        ALTER TABLE contract ADD COLUMN expenditure_officer TEXT;
        ALTER TABLE contract ADD COLUMN staff TEXT;
        ALTER TABLE contract_item ADD COLUMN ref_request_base TEXT;
        ALTER TABLE contract_item ADD COLUMN ref_request_seq TEXT;
        ALTER TABLE contract_item ADD COLUMN source_item TEXT;
        CREATE TABLE erp_mapping (revision TEXT PRIMARY KEY, json TEXT NOT NULL, active INTEGER NOT NULL DEFAULT 0,
            validated INTEGER NOT NULL DEFAULT 0, samples TEXT NOT NULL DEFAULT '[]', created_at TEXT NOT NULL);
        CREATE UNIQUE INDEX erp_mapping_active ON erp_mapping(active) WHERE active=1;
        CREATE TABLE erp_row (entity_type TEXT NOT NULL, entity_base TEXT NOT NULL, entity_seq TEXT NOT NULL,
            table_name TEXT NOT NULL, source_key TEXT NOT NULL, line_no INTEGER NOT NULL,
            PRIMARY KEY(entity_type,entity_base,entity_seq,table_name,source_key));
        CREATE TABLE erp_source (entity_type TEXT NOT NULL, entity_base TEXT NOT NULL, entity_seq TEXT NOT NULL,
            document_json TEXT NOT NULL, mapping_revision TEXT NOT NULL, updated_at TEXT NOT NULL,
            PRIMARY KEY(entity_type,entity_base,entity_seq));
        CREATE TABLE erp_request_alias (erp_base TEXT PRIMARY KEY, pdf_base TEXT NOT NULL UNIQUE,
            updated_at TEXT NOT NULL, FOREIGN KEY(pdf_base) REFERENCES request_series(request_base) ON DELETE CASCADE);
        CREATE TABLE erp_partner (contract_base TEXT NOT NULL, seq TEXT NOT NULL, line_no INTEGER NOT NULL,
            source_id TEXT, name TEXT, business_number TEXT, representative TEXT, address TEXT, phone TEXT, fax TEXT,
            share_rate TEXT, share_amount TEXT, PRIMARY KEY(contract_base,seq,line_no),
            FOREIGN KEY(contract_base,seq) REFERENCES contract(contract_base,seq) ON DELETE CASCADE);
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
        """;

    private const string V16 = """
        CREATE TABLE erp_dataset (
            singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
            dataset_id TEXT NOT NULL,
            environment TEXT NOT NULL DEFAULT 'unbound'
        );
        INSERT INTO erp_dataset(singleton, dataset_id) VALUES (1, lower(hex(randomblob(16))));
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
        """;

    /// <summary>
    /// <b>공고건(群)을 세우고 링크의 끝점을 그리로 옮긴다.</b>
    ///
    /// <para><b>왜 건이 필요한가.</b> <see cref="V14"/> 가 관련공고를 담되 판정하지 않기로 한
    /// 자리에서 남은 것이 이것이다. 이 저장소가 아는 축은 <b>차수</b> 하나였고, 변경공고는
    /// 그것으로 잡힌다. 그런데 <b>취소 후 재공고는 본번호가 갈린다</b> — 조립대가 그 표본이다:
    /// <c>R26BK09011054</c> 가 취소되고 <c>R26BK09012082</c> 로 다시 나갔는데, 접수도 계약도
    /// 하나다. 차수로는 영영 이어지지 않아 한 조달 건이 <b>두 줄</b>로 서고, 품목과 건명이 같아
    /// 두 링커 모두 후보가 둘이 되어 사람 큐로 밀린다. 계열 위에 건을 하나 더 세워
    /// 접수·계약 링크를 그리로 옮기면, 한 건에 접수 하나라는 약속을 DB 가 계속 지킨다.</para>
    ///
    /// <para><b>왜 이름이 가장 이른 본번호인가.</b> 재공고서는 자기가 대신하는 번호를 적어 두므로
    /// <b>문서가 아직(또는 영영) 없는 조상</b>도 이름이 될 수 있다 — 가스성분분석기가 가리키는
    /// <c>R26BK09013019</c> 는 코퍼스에 없고, 재공고 건에서는 그것이 예외가 아니라 보통이다.
    /// 이름을 최소값으로 두면 재공고를 먼저 넣어도 건의 이름이 원공고로 정해지고, 나중에 당초
    /// 공고가 들어와도 <b>이름이 흔들리지 않는다</b> — 거기 매어 둔 링크가 조용히 옮겨 다니지
    /// 않는다. 나라장터 공고번호는 시간순으로 커지므로 최소값이 곧 원공고다.</para>
    ///
    /// <para><b>차례가 중요하다.</b> <c>notice_group</c> 을 먼저 세워 채우고 →
    /// <c>notice_series</c> 를 갈아 끼우고 → 링크 넷이다. 링크를 먼저 만들면 그
    /// <c>REFERENCES</c> 가 곧 지워질 옛 <c>notice_series</c> 를 겨눈다. 씨앗은 판올림이 스스로
    /// 뿌린다 — <c>AssertForeignKeysHold</c> 는 <c>NoticeGroups.Rebuild</c> 를 기다려 주지 않는다.</para>
    ///
    /// <para><b>남이 외래키로 가리키는 부모 표를 갈아 끼우는 첫 판올림이다.</b> V3·V4·V11·V13 은
    /// 모두 자식이거나 아무도 가리키지 않던 표였다. <c>legacy_alter_table</c> 을 켜지 않았고
    /// 판올림 중에는 <c>foreign_keys = OFF</c> 라(<c>Database.Migrate</c>) <c>RENAME</c> 이 남의
    /// <c>REFERENCES</c> 를 고쳐 주지 않으므로, <b>이름이 같은 새 표가 그 자리를 그대로
    /// 이어받는다</b>. 그것이 실제로 성립하는지는 판올림 뒤 <c>AssertForeignKeysHold</c> 가
    /// 확인한다 — 그 검사가 여기서는 안전장치다.</para>
    ///
    /// <para><b><c>notice_series.group_base</c> 에는 캐스케이드를 걸지 않는다.</b> 걸면
    /// <c>notice_group</c> → <c>notice_series</c> → <c>notice_user_field</c> 로 두 칸 이어져,
    /// 건을 다시 지으며 옛 건 하나를 지우는 것만으로 <b>사람이 적은 검토여부·메모가 지워진다</b>.
    /// 오류 없이 자료를 잃는 종류라 참조만 걸고 지우기는 <c>NoticeGroups.Rebuild</c> 가
    /// 명시 SQL 로 한다.</para>
    ///
    /// <para>옮길 때 값은 그대로 간다 — 아직 건 하나에 계열 하나이므로 건 이름이 곧 본번호다.
    /// 실제로 이어 붙이는 것은 판올림이 아니라 그 뒤에 도는 <c>Rebuild</c> 다.</para>
    /// </summary>
    private const string V15 = """
        -- ── 건을 세우고 씨를 뿌린다 ────────────────────────────────────
        CREATE TABLE notice_group (group_base TEXT PRIMARY KEY);
        INSERT INTO notice_group SELECT notice_base FROM notice_series;

        CREATE TABLE notice_series_new (
            notice_base TEXT PRIMARY KEY,
            -- 캐스케이드를 걸지 않는다. 걸면 notice_user_field 까지 두 칸 이어져 사람 메모가 지워진다.
            group_base  TEXT NOT NULL REFERENCES notice_group(group_base)
        );
        INSERT INTO notice_series_new SELECT notice_base, notice_base FROM notice_series;
        DROP TABLE notice_series;
        ALTER TABLE notice_series_new RENAME TO notice_series;

        -- ── 관련공고를 해소한 값. 채우는 것은 C# 한 군데다(ValueParser.SplitNoticeRef) ──
        -- related 는 적힌 그대로 남는다(V14 의 결정). 해소하지 못하면 NULL 이다.
        ALTER TABLE notice_relation ADD COLUMN related_base TEXT;
        ALTER TABLE notice_relation ADD COLUMN related_seq  TEXT;

        -- ── 링크 넷의 끝점을 계열에서 건으로 ──────────────────────────
        -- notice_group 은 UNIQUE 를 그대로 이어받는다. 접수 1 : 건 1 이다(ADR-021 을 건으로 옮긴다).
        CREATE TABLE request_link_new (
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
        INSERT INTO request_link_new (
            request_base, notice_group, confidence, confirmed_at, decided_by, evidence, rule_version)
        SELECT request_base, notice_base, confidence, confirmed_at, decided_by, evidence, rule_version
        FROM request_link;
        DROP TABLE request_link;
        ALTER TABLE request_link_new RENAME TO request_link;

        CREATE TABLE project_link_new (
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
        INSERT INTO project_link_new (
            contract_base, notice_group, confidence, confirmed_at, decided_by, evidence, rule_version)
        SELECT contract_base, notice_base, confidence, confirmed_at, decided_by, evidence, rule_version
        FROM project_link;
        DROP TABLE project_link;
        ALTER TABLE project_link_new RENAME TO project_link;

        CREATE TABLE link_rejection_new (
            contract_base TEXT NOT NULL,
            notice_group  TEXT NOT NULL,
            rejected_at   TEXT NOT NULL,
            PRIMARY KEY (contract_base, notice_group),
            FOREIGN KEY (contract_base) REFERENCES contract_series(contract_base) ON DELETE CASCADE,
            FOREIGN KEY (notice_group)  REFERENCES notice_group(group_base)       ON DELETE CASCADE
        );
        INSERT INTO link_rejection_new (contract_base, notice_group, rejected_at)
        SELECT contract_base, notice_base, rejected_at FROM link_rejection;
        DROP TABLE link_rejection;
        ALTER TABLE link_rejection_new RENAME TO link_rejection;

        CREATE TABLE request_link_rejection_new (
            request_base TEXT NOT NULL,
            notice_group TEXT NOT NULL,
            rejected_at  TEXT NOT NULL,
            PRIMARY KEY (request_base, notice_group),
            FOREIGN KEY (request_base) REFERENCES request_series(request_base) ON DELETE CASCADE,
            FOREIGN KEY (notice_group) REFERENCES notice_group(group_base)     ON DELETE CASCADE
        );
        INSERT INTO request_link_rejection_new (request_base, notice_group, rejected_at)
        SELECT request_base, notice_base, rejected_at FROM request_link_rejection;
        DROP TABLE request_link_rejection;
        ALTER TABLE request_link_rejection_new RENAME TO request_link_rejection;

        -- 옛 ix_project_link_notice 는 표와 함께 갔다. 건으로 다시 세운다.
        CREATE INDEX ix_project_link_group ON project_link(notice_group);

        -- 뷰의 「현행공고」가 이 열 짝으로 자기를 가리키는 행을 찾는다.
        CREATE INDEX ix_notice_relation_ref ON notice_relation(related_base, related_seq);
        """;

    /// <summary>
    /// <b>관련공고를 담는다 — 판정하지는 않는다</b>(ADR-025).
    ///
    /// <para><b>왜 담나.</b> 지금까지 이 저장소가 아는 축은 <b>차수</b> 하나였다. 변경공고는
    /// 그것으로 잡힌다 — <c>R26BK09017030-000</c> 다음에 <c>-001</c> 이 오고, 뷰가 최신만 낸다.
    /// 그런데 <b>취소 후 재공고는 본번호가 갈린다</b>: <c>R26BK09011054</c> 가 취소되고
    /// <c>R26BK09012082</c> 로 다시 나간다. 차수로는 영영 이어지지 않고, 둘을 잇는 단서는
    /// 공고서의 「관련공고」 칸 하나뿐이다 — 재공고는 거기에 자기가 대신하는 취소공고와 그
    /// 당초를 쉼표로 함께 적는다. 파서는 그 라벨을 사전에 갖고 있으면서 값을 버리고 있었다.</para>
    ///
    /// <para><b>왜 딸린 표인가.</b> 값이 여럿이다(재공고는 둘, 실측에 셋짜리도 있다).
    /// <c>notice</c> 에 열 하나로 눌러 담으면 쉼표로 이어 붙인 문자열이 자연키 옆에 앉아,
    /// 나중에 이것을 타고 계열을 잇는 날 다시 갈라야 한다. <see cref="V9"/> 의
    /// <c>notice_officer_contact</c> 가 같은 까닭으로 세운 표라 그 모양을 그대로 따른다 —
    /// 차수에 매달고, 부모가 지워지면 함께 지워진다.</para>
    ///
    /// <para><b>판정하지 않는다.</b> 이 표를 읽어 "이 계열은 죽었다" 를 정하는 코드는 없다.
    /// 취소공고는 여전히 차수가 높다는 이유로 뷰에서 원공고를 대체하고, 재공고는 여전히
    /// 별개의 조달 건으로 선다. 적힌 사실을 담는 것과 그 사실로 무엇을 판정하는 것은 다른
    /// 일이고, 뒤엣것은 이 표가 쌓인 뒤에야 근거를 갖는다(ADR-016).</para>
    /// </summary>
    private const string V14 = """
        CREATE TABLE notice_relation (
            notice_base TEXT NOT NULL,
            seq         TEXT NOT NULL,
            line_no     INTEGER NOT NULL,  -- 적힌 차례. 원문은 최신에서 과거 순으로 적는다
            related     TEXT NOT NULL,     -- 관련공고 번호. 꼴을 검사하지 않고 적힌 그대로 담는다
            PRIMARY KEY (notice_base, seq, line_no),
            FOREIGN KEY (notice_base, seq) REFERENCES notice(notice_base, seq) ON DELETE CASCADE
        );
        """;

    /// <summary>
    /// <b>계획 서식을 표본에 고정한다</b>(ADR-023 개정).
    ///
    /// <para><b>왜 고정하나.</b> <see cref="V12"/> 는 서식이 해마다·부서마다 달라진다고 보고
    /// 담을 자리를 열한 개로 못 박은 뒤 <b>어느 머리글이 어느 자리로 가는지</b>를
    /// <c>app_setting</c> 의 <c>plan.mapping</c> 에 자료로 두었다. 그런데 실제로 도는 서식은
    /// 하나로 굳었고, 대응표는 값을 하는 대신 <b>갓 깐 사람이 첫 가져오기에서 막히는 자리</b>가
    /// 되었다 — 짚기 전에는 한 줄도 들어가지 않는데, 짚을 것은 표본 그대로였다.</para>
    ///
    /// <para>그래서 열을 <b>표본(「조달계획 (양식 표본).xlsx」)의 머리글 그대로</b> 세운다.
    /// 열 이름이 곧 머리글이라 대응표가 있을 자리가 없다 — 같은 판에서 걷는다. 서식이 정말로
    /// 바뀌는 날에는 그때 판을 올린다. 그날이 오지 않는 데 값을 치르지 않는다.</para>
    ///
    /// <para>옛 행은 <b>뜻이 그대로 겹치는 것만</b> 옮긴다(<c>request_number</c>·
    /// <c>item_name</c>·<c>quantity</c>·<c>unit</c>·<c>officer</c>). 계획년도·요구명·수요기관·
    /// 예산금액·요구일자·비고는 표본에 자리가 없어 버린다 — 계획은 엑셀을 다시 넣으면 그대로
    /// 되살아나는 자료라, 억지로 담을 자리를 만들어 두는 것보다 비우는 편이 정직하다.</para>
    ///
    /// <para>SQLite 는 열을 지우고 차례를 바꾸는 일을 <c>ALTER</c> 로 못 하므로
    /// <see cref="V3"/>·<see cref="V4"/> 가 쓰는 방식대로 <b>표째 다시 세워 옮긴다</b>.</para>
    /// </summary>
    private const string V13 = """
        CREATE TABLE plan_new (
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

        INSERT INTO plan_new (
            request_number, item_name, quantity, unit, officer, source_name, imported_at
        )
        SELECT request_number, item_name, quantity, unit, officer, source_name, imported_at
        FROM plan;

        DROP TABLE plan;
        ALTER TABLE plan_new RENAME TO plan;

        -- 대응표는 이제 있을 자리가 없다. 남겨 두면 다음에 읽는 쪽이 뜻을 짐작하게 된다.
        DELETE FROM app_setting WHERE key = 'plan.mapping';
        """;

    /// <summary>
    /// <b>계획을 네 번째 개체로 세운다.</b>
    ///
    /// <para><b>왜.</b> 여기 쌓이는 셋(접수·공고·계약)은 모두 <b>이미 일어난 일</b>이다.
    /// 그것만으로는 "올해 계획한 것 중 얼마나 왔나" 를 물을 수 없다 — 분자만 있고 분모가
    /// 없다. 그 분모를 쥔 것이 연간 조달계획이고, <c>조달요구번호</c> 로
    /// <c>request_item.request_number</c> 에 그대로 이어진다(<c>ix_request_item_number</c> 가
    /// 이미 서 있다). 다른 셋과 달리 <b>문서가 아니라 엑셀에서 온다</b> — PDF 를 읽는 길과
    /// 나란히 서는 별개의 입구다.</para>
    ///
    /// <para><b>왜 차수가 없는가.</b> 접수·공고·계약은 변경분을 별개의 행으로 쌓는다 —
    /// 그 차수의 문서를 나중에 다시 찍을 일이 있어서다. 계획은 그렇지 않다. 지난 계획을
    /// 문서로 내놓을 일이 없고, 사람이 알고 싶은 것은 <b>지금 계획이 무엇인가</b> 하나뿐이다.
    /// 그래서 계획변경은 차수를 올리는 것이 아니라 <b>같은 조달요구번호를 덮어쓰는 것</b>으로
    /// 본다. 자연키가 <c>request_number</c> 하나라 표에 <c>seq</c> 가 없다.</para>
    ///
    /// <para>금액은 여기서도 TEXT 다 — 머리에 적은 이유 그대로다. 수량만 INTEGER 인 것은
    /// <c>request_item.quantity</c> 와 형을 맞추기 위해서다.</para>
    ///
    /// <para><c>source_name</c>·<c>imported_at</c> 은 <b>어느 엑셀에서 언제 왔는지</b>를 적는다.
    /// 계획 엑셀은 부서마다 따로 돌아 같은 조달요구번호가 다른 파일에서 다시 올 수 있는데,
    /// 덮어쓰기로 보는 이상 마지막에 온 것이 어느 파일이었는지는 남아야 한다.</para>
    /// </summary>
    private const string V12 = """
        CREATE TABLE plan (
            request_number  TEXT PRIMARY KEY,  -- 조달요구번호. 계획의 자연키
            plan_year       TEXT,
            title           TEXT,              -- 요구명
            officer         TEXT,              -- 담당자
            demand_agency   TEXT,
            item_name       TEXT,
            quantity        INTEGER,
            unit            TEXT,
            budget_amount   TEXT,
            requested_on    TEXT,
            remarks         TEXT,
            source_name     TEXT NOT NULL,
            imported_at     TEXT NOT NULL
        );
        """;

    /// <summary>
    /// <b>접수를 세 번째 개체로 세운다</b>(ADR-021).
    ///
    /// <para><b>왜.</b> 조달의 실제 흐름은 공고보다 한 칸 앞에서 시작한다 — 수요기관이
    /// 조달요구번호마다 요청을 내고, 조달청이 그것을 모아 국방계약요청접수서로 접수한 뒤
    /// 하나의 공고를 낸다. 그 앞칸이 비어 있으면 쌓인 자료만으로 <b>"이 계약이 어느
    /// 조달요구에서 나왔는가"</b> 를 답할 수 없다.</para>
    ///
    /// <para><b>키는 접수번호와 접수차수다</b>(<see cref="V18"/> 에서 바뀌었다). 처음에는 접수서에
    /// 접수번호가 찍히지 않아 대표 요청번호(그 문서의 최소값)로 세웠는데, 확장이 화면에서 진짜
    /// 접수번호를 읽게 되어 그리로 옮겼다. 줄 하나가 접수 하나라 통합이 접수1:공고1:계약1 이 된다.</para>
    ///
    /// <para><b><c>notice_base</c> 를 UNIQUE 로 두는 것은 뜻이 있는 선택이다.</b> 접수:공고를
    /// 1:1 로 못 박아, 한 공고에 접수 둘이 붙으면 <c>v_통합_v2</c> 의 줄이 조용히 두 배가 되는
    /// 대신 <b>넣는 자리에서 시끄럽게 실패한다</b>. 공고 하나가 접수서 둘을 묶는 사례가
    /// 나오면 그때 판단한다 — 조용히 틀리는 것보다 막히는 편이 낫다.</para>
    ///
    /// <para><c>request_item_link</c> 는 <b>휴리스틱이 낸 짝</b>을 담는다. 어느 조달요구가 어느
    /// 공고 품목이 되었는가 — 수량·단가 다중집합이 같을 때 지퍼처럼 짝지은 결과다.
    /// 근거가 아니라 <b>결과</b>라 링크를 풀면 함께 걷는다.</para>
    ///
    /// <para><b><c>CHECK</c> 두 개를 넓혀야 한다.</b> SQLite 는 <c>CHECK</c> 를 <c>ALTER</c> 로
    /// 못 고치므로 <c>user_column</c> 과 <c>field_override</c> 를 <b>표째 다시 세워 옮긴다</b>
    /// — <see cref="V3"/>·<see cref="V4"/> 가 쓰는 방식 그대로다. 넓히지 않으면 접수 쪽 사람
    /// 열과 덮개가 <c>CHECK</c> 에 막혀 들어가지 못한다.</para>
    /// </summary>
    private const string V11 = """
        -- ── 접수 ─────────────────────────────────────────────────────
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
            updated_at        TEXT NOT NULL,
            PRIMARY KEY (request_base, seq)
        );

        -- 계열: 차수를 벗은 키. 사람이 붙인 것과 링크가 여기 매달린다(ADR-012).
        CREATE TABLE request_series (request_base TEXT PRIMARY KEY);

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
            expense_request_number TEXT,
            PRIMARY KEY (request_base, seq, line_no),
            FOREIGN KEY (request_base, seq) REFERENCES request(request_base, seq) ON DELETE CASCADE
        );

        CREATE INDEX ix_request_item_number ON request_item(request_number);

        CREATE TABLE request_user_field (
            request_base TEXT NOT NULL,
            field_name   TEXT NOT NULL,
            value        TEXT,
            updated_at   TEXT NOT NULL,
            PRIMARY KEY (request_base, field_name),
            FOREIGN KEY (request_base) REFERENCES request_series(request_base) ON DELETE CASCADE
        );

        -- ── 접수 → 공고 ──────────────────────────────────────────────
        -- project_link 과 같은 꼴에 notice_base 를 UNIQUE 로 조인다. 1:1 을 DB 가 지킨다.
        CREATE TABLE request_link (
            request_base TEXT PRIMARY KEY,
            notice_base  TEXT NOT NULL UNIQUE,
            confidence   REAL,
            confirmed_at TEXT,
            decided_by   TEXT NOT NULL DEFAULT 'human',
            evidence     TEXT,
            rule_version INTEGER,
            FOREIGN KEY (request_base) REFERENCES request_series(request_base) ON DELETE CASCADE,
            FOREIGN KEY (notice_base)  REFERENCES notice_series(notice_base)   ON DELETE CASCADE
        );

        CREATE TABLE request_link_rejection (
            request_base TEXT NOT NULL,
            notice_base  TEXT NOT NULL,
            rejected_at  TEXT NOT NULL,
            PRIMARY KEY (request_base, notice_base),
            FOREIGN KEY (request_base) REFERENCES request_series(request_base) ON DELETE CASCADE,
            FOREIGN KEY (notice_base)  REFERENCES notice_series(notice_base)   ON DELETE CASCADE
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

        CREATE INDEX ix_request_item_link_notice ON request_item_link(notice_base, notice_seq);

        -- ── CHECK 을 넓힌다. ALTER 로는 못 고쳐 표째 갈아 끼운다 ────────
        CREATE TABLE user_column_new (
            entity_type TEXT NOT NULL CHECK (entity_type IN ('contract', 'notice', 'request')),
            field_name  TEXT NOT NULL,
            kind        TEXT NOT NULL DEFAULT 'text' CHECK (kind IN ('text', 'choice', 'date', 'number')),
            options     TEXT,
            sort_order  INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (entity_type, field_name)
        );
        INSERT INTO user_column_new SELECT * FROM user_column;
        DROP TABLE user_column;
        ALTER TABLE user_column_new RENAME TO user_column;

        CREATE TABLE field_override_new (
            entity_type TEXT NOT NULL CHECK (entity_type IN ('notice', 'contract', 'request')),
            base        TEXT NOT NULL,
            seq         TEXT NOT NULL,
            column_name TEXT NOT NULL,
            value       TEXT NOT NULL,
            original    TEXT NOT NULL,
            updated_at  TEXT NOT NULL,
            PRIMARY KEY (entity_type, base, seq, column_name)
        );
        INSERT INTO field_override_new SELECT * FROM field_override;
        DROP TABLE field_override;
        ALTER TABLE field_override_new RENAME TO field_override;
        """;

    /// <summary>
    /// 사람이 <b>파서가 읽은 값을 고칠 자리</b>를 만든다.
    ///
    /// <para><b>왜 덮개인가.</b> 검산을 하지 않기로 한 이상(ADR-016) 틀린 값을 바로잡는 것은
    /// 오직 사람인데, 지금까지 그 사람에게 연필이 없었다. 그렇다고 <c>notice</c>·<c>contract</c>
    /// 를 직접 고치면 <b>기계가 읽은 것과 사람이 붙인 것이 한 칸에 섞인다</b> — 무엇이 문서에
    /// 적힌 값이고 무엇이 사람의 판단인지 그 뒤로 가릴 수 없고, 되돌릴 수도 없다. 사용자 열을
    /// 따로 둔 것과 같은 까닭으로 표를 나눈다(ADR-007·ADR-020).</para>
    ///
    /// <para><b>차수에 매단다.</b> 사람 값(진행상태·메모)은 "이 건" 에 대한 판단이라 계열에
    /// 붙지만(ADR-012), 덮개는 <b>"이 문서의 이 칸을 이렇게 읽어야 한다"</b> 는 정정이다.
    /// 1차에서 고친 금액이 2차 변경계약에 그대로 얹히면 확인한 적 없는 숫자가 확인된 얼굴로
    /// 문서에 찍힌다. 차수가 바뀌면 따라가지 않되, 옛 차수 자리에 그대로 남아 잃지도 않는다.</para>
    ///
    /// <para><c>column_name</c> 은 표의 열이 아니라 <b>뷰의 한글 열 이름</b>이다. 뷰가 표기까지
    /// 마친 값 <b>바깥</b>에 씌우므로 사람이 적은 글자가 변환 없이 그대로 나간다 — 계약면의 값은
    /// 원래 "문서에 그대로 찍힐 문자열" 이라 이것이 맞다.</para>
    ///
    /// <para><c>value</c> 는 <b>빈 문자열도 유효한 값</b>이다. 잘못 읽힌 칸을 비우는 것이 실제
    /// 용례라, 되돌리기는 값을 비우는 것이 아니라 <b>행을 지우는</b> 쪽이 맡는다.</para>
    ///
    /// <para><c>original</c> 은 <b>처음 고칠 때 그 자리에 있던 값</b>이다. 빈 칸을 고쳤으면 빈
    /// 문자열이라 NULL 일 일이 없다 — NOT NULL 로 두면 읽는 쪽이 <c>COALESCE</c> 를 쓰지 않아도
    /// 되고, 그래야 뷰가 아닌 표에서 온 열이라 Dapper 가 타입을 알아본다. 무엇을 무엇으로 고쳤는지
    /// 화면이 보여 주려고 둔다 — 보이지 않으면 고칠 수 있다는 것도 안전하지 않다. 두 번째 편집에서
    /// 다시 적으면 "고치기 전" 이 아니라 "직전에 내가 적은 것" 이 되므로 <b>넣을 때만</b> 적는다.</para>
    ///
    /// <para>공고·계약 둘을 한 표에 담아 외래키를 걸지 못한다 — 지울 때 손으로 함께 지운다
    /// (<see cref="Store.Delete"/>).</para>
    /// </summary>
    private const string V10 = """
        CREATE TABLE field_override (
            entity_type TEXT NOT NULL CHECK (entity_type IN ('notice', 'contract')),
            base        TEXT NOT NULL,
            seq         TEXT NOT NULL,
            column_name TEXT NOT NULL,
            value       TEXT NOT NULL,
            original    TEXT NOT NULL,
            updated_at  TEXT NOT NULL,
            PRIMARY KEY (entity_type, base, seq, column_name)
        );
        """;

    /// <summary>
    /// 공고에서 읽고도 담지 못하던 것을 담는다 — 품목의 <b>납품기한</b>과 <b>수요기관 담당자</b>.
    ///
    /// <para><b>납품기한.</b> 구매대상물품 표의 열로 이미 있었고 파서도 그 칸을 집어내고
    /// 있었는데, 받을 자리가 없어 그 자리에서 버려지고 있었다. 공고 단계에서는 대개
    /// <c>-</c> 라 비지만, 값이 실린 공고가 실재하는 이상 버릴 근거가 없다.
    /// 계약의 <c>contract_item.delivery_due</c> 와 이름·형을 맞춘다.</para>
    ///
    /// <para><b>수요기관 담당자.</b> 별도의 표로 담는 까닭은 <b>수요기관이 여럿</b>일 수 있어서다.
    /// <c>notice</c> 에 열 넷을 더하면 둘째 기관부터 조용히 사라진다. 공고 뷰에는 첫 줄만
    /// 붙이되, 자료 자체는 온전히 남는다.</para>
    ///
    /// <para>공고기관 쪽의 <c>notice_officer</c> 와 다른 사람이다 — 그쪽은 공고를 낸 조달청
    /// 담당자, 이쪽은 물건을 받는 기관의 담당자다. 그래서 합치지 않는다.</para>
    /// </summary>
    private const string V9 = """
        ALTER TABLE notice_item ADD COLUMN delivery_due TEXT;

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
        """;

    /// <summary>
    /// 공고의 검토여부에서도 후보 목록을 뗀다.
    ///
    /// <para><see cref="V7"/> 이 계약의 진행상태에 한 것과 같은 까닭이다 — 미검토·검토중·참여·불참
    /// 넷으로는 실제로 적고 싶은 것이 다 담기지 않는다. 후보가 필요하면 열 고치기에서
    /// <c>choice</c> 로 되돌리고 원하는 낱말을 적으면 된다.</para>
    /// </summary>
    private const string V8 = """
        UPDATE user_column SET kind = 'text', options = NULL
        WHERE entity_type = 'notice' AND field_name = '검토여부';
        """;

    /// <summary>
    /// 기본으로 세우는 열을 <b>둘로 줄인다</b>.
    ///
    /// <para><b>왜.</b> 처음 쓰는 사람에게 열 넷을 미리 세워 주는 것은 친절이 아니다 —
    /// 쓰지 않는 열이 표를 넓히고, 무엇을 채워야 하는지 헷갈리게 한다. 사람마다 필요한 열이
    /// 다르므로 <b>스스로 더하게</b> 하는 것이 맞다(<see cref="Store.AddUserColumn"/>).
    /// 남기는 것은 어디서나 쓰는 둘, 진행상태와 메모다.</para>
    ///
    /// <para><b>진행상태의 후보 목록도 뗀다.</b> 준비·투찰·낙찰… 을 미리 정해 두었는데,
    /// 실제로는 그 밖의 상태가 계속 생긴다. 후보 밖 값을 넣을 때마다 화면이 토를 다는 것보다
    /// 그냥 적게 두는 편이 낫다. 후보가 필요하면 열 고치기에서 <c>choice</c> 로 되돌리면 된다.</para>
    ///
    /// <para><b>적어 둔 값은 지우지 않는다.</b> 열 정의만 없앤다 — <c>contract_user_field</c> 의
    /// 값은 그대로 남아, 같은 이름으로 열을 다시 만들면 되살아난다. 사람이 적은 것을 기계가
    /// 지우지 않는다는 원칙이 여기에도 그대로 걸린다(ADR-007).</para>
    /// </summary>
    private const string V7 = """
        DELETE FROM user_column
        WHERE entity_type = 'contract' AND field_name IN ('내부관리번호', '담당');

        UPDATE user_column SET kind = 'text', options = NULL
        WHERE entity_type = 'contract' AND field_name = '진행상태';
        """;

    /// <summary>
    /// <b>검산을 걷어낸다.</b> 그리고 앱 설정을 담을 자리를 만든다.
    ///
    /// <para><b>왜 걷어내나.</b> 문서에 찍힌 숫자끼리 맞춰 보는 일은 나라장터가 올리기 전에
    /// 이미 끝낸다. 우리가 다시 채점해서 얻는 것은 <b>정상인 문서에 붙는 의심 표시</b>뿐이고,
    /// 그것이 이 저장소의 본분 — <b>읽어서 쌓는 것</b> — 을 흐린다. 값은 문서에 적힌 그대로 담고,
    /// 맞는지 틀리는지는 원본을 보는 사람이 정한다.</para>
    ///
    /// <para>표를 지운다고 값이 사라지지 않는다. <c>*_finding</c> 에 있던 것은 기계의 진술이지
    /// 문서에서 읽은 사실이 아니라, 지워도 잃는 것이 없다. 읽지 못한 문서는 여전히
    /// <c>document.parse_error</c> 에 남는다 — 그쪽은 검산이 아니라 <b>실패 기록</b>이다.</para>
    ///
    /// <para><b>설정을 DB 에 둔다.</b> 별도 파일로 두면 DB 와 설정이 따로 옮겨 다니며 어긋난다.
    /// 자료가 있는 곳에 설정도 있어야 한 벌로 움직인다.</para>
    /// </summary>
    private const string V6 = """
        DROP TABLE IF EXISTS contract_finding;
        DROP TABLE IF EXISTS notice_finding;

        -- 앱 설정. 열을 늘리지 않고 키-값으로 둔다 — 설정 하나 늘 때마다 판올림하지 않게.
        CREATE TABLE app_setting (
            key   TEXT PRIMARY KEY,
            value TEXT
        );
        """;

    /// <summary>
    /// 링크에 <b>누가 이었고 무엇을 근거로 삼았는지</b>를 남기고, <b>아니라는 판정</b>을 담을 자리를 만든다.
    ///
    /// <para><b>무엇이 잘못돼 있었나.</b> 링크는 사람이 누른 것만 있었으므로 결정 주체를 적을
    /// 필요가 없었다. 이제 기계가 건명 전파를 근거로 스스로 잇는다(ADR-015). 그런데 규칙은
    /// 반드시 한 번은 고치게 되고, 그때 <b>자동 링크만 골라 다시 판정하고 사람이 확정한 것은
    /// 건드리지 않아야</b> 한다. 구별할 열이 없으면 둘이 섞여 그 일이 불가능해진다 —
    /// 기계가 읽은 것과 사람이 붙인 것을 갈라 두라는 <see cref="V4"/> 의 교훈이 링크에도 그대로 적용된다.</para>
    ///
    /// <para>또 하나, <b>사람이 "아니다" 라고 판정한 짝을 버리고 있었다</b>. 확정만 기록하니
    /// 물리친 후보가 다음에도 목록 맨 위에 다시 떠서 사람 큐가 줄지 않는다.</para>
    ///
    /// <para><b>어떻게 고치나.</b> <c>project_link</c> 에 결정 주체·근거·규칙판을 더한다.
    /// 표를 갈아 끼우지 않고 열만 붙이면 되는데, 기존 행은 전부 사람이 누른 것이라
    /// <c>'human'</c> 기본값이 사실과 맞는다.</para>
    ///
    /// <para>거부는 <b>따로 표를 세운다</b>. <c>project_link</c> 의 키는 계약 하나라
    /// (계약당 공고는 하나다) 짝마다 생기는 거부를 담을 수 없다. 나누어 두면 덤으로
    /// <c>v_통합_v1</c> 의 조인이 그대로라 계약면 뷰가 흔들리지 않는다.</para>
    /// </summary>
    private const string V5 = """
        ALTER TABLE project_link ADD COLUMN decided_by   TEXT NOT NULL DEFAULT 'human';
        ALTER TABLE project_link ADD COLUMN evidence     TEXT;
        ALTER TABLE project_link ADD COLUMN rule_version INTEGER;

        -- 사람이 물리친 짝. 계약 하나에 여럿 달릴 수 있어 project_link 에 담기지 않는다.
        CREATE TABLE link_rejection (
            contract_base TEXT NOT NULL,
            notice_base   TEXT NOT NULL,
            rejected_at   TEXT NOT NULL,
            PRIMARY KEY (contract_base, notice_base),
            FOREIGN KEY (contract_base) REFERENCES contract_series(contract_base) ON DELETE CASCADE,
            FOREIGN KEY (notice_base)   REFERENCES notice_series(notice_base)     ON DELETE CASCADE
        );
        """;

    /// <summary>
    /// 사람이 붙인 것을 <b>차수에서 떼어낸다</b>.
    ///
    /// <para><b>무엇이 잘못돼 있었나.</b> 손으로 적은 값과 확정한 링크가 <c>(본번호, 차수)</c> 에
    /// 매달려 있었는데 뷰는 <c>MAX(seq)</c> 만 낸다. 그래서 <b>변경계약이 한 건 들어오는 순간</b>
    /// 진행상태·담당·메모가 옛 차수에 남아 뷰에서 빈 문자열이 되고, 링크도 함께 끊겨 공고 열이
    /// 통째로 빈다. 변경공고가 들어오면 링크가 가리키는 차수가 <c>v_공고_v1</c> 에서 밀려나
    /// 같은 일이 세 번째 경로로 벌어진다. 어느 쪽도 오류를 내지 않는다 —
    /// <b>파서에서 그토록 경계한 "조용히 틀림" 이 스키마에 있었다.</b></para>
    ///
    /// <para><b>어떻게 고치나.</b> 차수를 벗은 키(계열)를 표로 세우고, <b>기계가 읽은 사실은
    /// 차수 키에, 사람이 붙인 것은 계열 키에</b> 매단다. 링크는 "이 계약 계열은 저 공고 계열에서
    /// 나왔다" 는 진술이지 차수끼리의 진술이 아니다. 차원 모델링에서 버전 키와 durable key 를
    /// 나누는 것과 같은 처방이다(ADR-012).</para>
    ///
    /// <para>덧붙여 <b>검산 경고를 남긴다</b>. 지금까지 경고는 콘솔에 찍히고 사라져서, 백 건을
    /// 한 번에 넣으면 의심스러운 행을 이후 어떤 조회로도 찾을 수 없었다. 경고는 그 차수를
    /// 읽어낸 결과에 대한 기계의 진술이라 <b>차수 키</b>에 매단다(ADR-013).</para>
    /// </summary>
    private const string V4 = """
        -- ── 계열: 차수를 벗은 키. 사람이 붙인 것이 여기 매달린다 ────────
        -- 파생 표라 파서가 채운다(업서트 때 INSERT OR IGNORE). 본표에서 이쪽으로 외래키를
        -- 걸려면 30열짜리 표 둘을 통째로 갈아 끼워야 해서, 값어치에 비해 비싸 두지 않았다.
        CREATE TABLE contract_series (contract_base TEXT PRIMARY KEY);
        CREATE TABLE notice_series   (notice_base   TEXT PRIMARY KEY);

        INSERT OR IGNORE INTO contract_series SELECT DISTINCT contract_base FROM contract;
        INSERT OR IGNORE INTO notice_series   SELECT DISTINCT notice_base   FROM notice;

        -- ── 손으로 적은 값을 계열에 옮긴다 ──────────────────────────────
        -- 차수마다 따로 적혀 있던 값은 가장 최근 것만 남긴다. 같은 시각이면 높은 차수가 이긴다.
        CREATE TABLE contract_user_field_new (
            contract_base TEXT NOT NULL,
            field_name    TEXT NOT NULL,
            value         TEXT,
            updated_at    TEXT NOT NULL,
            PRIMARY KEY (contract_base, field_name),
            FOREIGN KEY (contract_base) REFERENCES contract_series(contract_base) ON DELETE CASCADE
        );
        INSERT INTO contract_user_field_new (contract_base, field_name, value, updated_at)
        SELECT u.contract_base, u.field_name,
               (SELECT x.value FROM contract_user_field x
                WHERE x.contract_base = u.contract_base AND x.field_name = u.field_name
                ORDER BY x.updated_at DESC, x.seq DESC LIMIT 1),
               MAX(u.updated_at)
        FROM contract_user_field u
        GROUP BY u.contract_base, u.field_name;
        DROP TABLE contract_user_field;
        ALTER TABLE contract_user_field_new RENAME TO contract_user_field;

        CREATE TABLE notice_user_field_new (
            notice_base TEXT NOT NULL,
            field_name  TEXT NOT NULL,
            value       TEXT,
            updated_at  TEXT NOT NULL,
            PRIMARY KEY (notice_base, field_name),
            FOREIGN KEY (notice_base) REFERENCES notice_series(notice_base) ON DELETE CASCADE
        );
        INSERT INTO notice_user_field_new (notice_base, field_name, value, updated_at)
        SELECT u.notice_base, u.field_name,
               (SELECT x.value FROM notice_user_field x
                WHERE x.notice_base = u.notice_base AND x.field_name = u.field_name
                ORDER BY x.updated_at DESC, x.seq DESC LIMIT 1),
               MAX(u.updated_at)
        FROM notice_user_field u
        GROUP BY u.notice_base, u.field_name;
        DROP TABLE notice_user_field;
        ALTER TABLE notice_user_field_new RENAME TO notice_user_field;

        -- ── 링크도 계열끼리 ────────────────────────────────────────────
        -- 계약 계열 하나는 공고 계열 하나만 가리킨다(공고 1 : 계약 N 은 그대로다).
        CREATE TABLE project_link_new (
            contract_base TEXT PRIMARY KEY,
            notice_base   TEXT NOT NULL,
            confidence    REAL,
            confirmed_at  TEXT,
            FOREIGN KEY (contract_base) REFERENCES contract_series(contract_base) ON DELETE CASCADE,
            FOREIGN KEY (notice_base)   REFERENCES notice_series(notice_base)     ON DELETE CASCADE
        );
        INSERT INTO project_link_new (contract_base, notice_base, confidence, confirmed_at)
        SELECT l.contract_base, l.notice_base, l.confidence, l.confirmed_at
        FROM project_link l
        WHERE l.contract_seq = (
            SELECT MAX(x.contract_seq) FROM project_link x WHERE x.contract_base = l.contract_base
        );
        DROP TABLE project_link;
        ALTER TABLE project_link_new RENAME TO project_link;

        CREATE INDEX ix_project_link_notice ON project_link(notice_base);

        -- ── 검산 경고. 값은 저장하되 의심 표시를 데이터와 함께 남긴다 ────
        -- 치명적인 것은 애초에 저장되지 않으므로 여기 오지 않는다 — 그쪽은 document.parse_error 다.
        CREATE TABLE contract_finding (
            contract_base TEXT NOT NULL,
            seq           TEXT NOT NULL,
            line_no       INTEGER NOT NULL,
            field         TEXT NOT NULL,
            message       TEXT NOT NULL,
            found_at      TEXT NOT NULL,
            PRIMARY KEY (contract_base, seq, line_no),
            FOREIGN KEY (contract_base, seq) REFERENCES contract(contract_base, seq) ON DELETE CASCADE
        );

        CREATE TABLE notice_finding (
            notice_base TEXT NOT NULL,
            seq         TEXT NOT NULL,
            line_no     INTEGER NOT NULL,
            field       TEXT NOT NULL,
            message     TEXT NOT NULL,
            found_at    TEXT NOT NULL,
            PRIMARY KEY (notice_base, seq, line_no),
            FOREIGN KEY (notice_base, seq) REFERENCES notice(notice_base, seq) ON DELETE CASCADE
        );
        """;

    /// <summary>
    /// 키 표현을 하나로 모으고 참조를 DB가 강제하게 한다.
    ///
    /// <para><b>무엇이 잘못돼 있었나.</b> 같은 계약을 두 가지로 적고 있었다 — 표에는
    /// 복합키 <c>(contract_base, seq)</c> 로, 링크와 사용자 값에는 이어붙인 문자열로.
    /// 그래서 외래키를 걸 수 없었고, 이어붙이는 규칙(계약은 그냥 잇고 공고는 <c>-</c> 를 끼움)이
    /// <b>코드 열두 군데에 흩어져</b> 있었다. 한 군데만 어긋나도 조용히 안 맞는다.</para>
    ///
    /// <para><b>어떻게 고치나.</b> 참조를 전부 복합키로 통일하고 모든 자식에 외래키를 건다.
    /// 사용자 값은 계약용·공고용으로 나눈다 — 하나의 표에서 둘을 가리키면 외래키를 걸 수 없다.
    /// 이어붙인 문자열은 이제 <b>보여줄 때만</b> 뷰에서 만든다.</para>
    ///
    /// <para><see cref="V1"/> 의 <c>document</c> 는 그대로 둔다. 그것은 감사 기록이라
    /// 대상이 지워져도 "이 파일을 언제 읽었다"가 남아야 한다 — 외래키를 걸지 않는 것이 옳다.</para>
    /// </summary>
    private const string V3 = """
        -- ── 딸린 줄에 외래키를 건다 ──────────────────────────────────
        CREATE TABLE notice_item_new (
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
            delivery_terms     TEXT,
            PRIMARY KEY (notice_base, seq, line_no),
            FOREIGN KEY (notice_base, seq) REFERENCES notice(notice_base, seq) ON DELETE CASCADE
        );
        INSERT INTO notice_item_new SELECT * FROM notice_item;
        DROP TABLE notice_item;
        ALTER TABLE notice_item_new RENAME TO notice_item;

        CREATE TABLE notice_schedule_new (
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
        INSERT INTO notice_schedule_new SELECT * FROM notice_schedule;
        DROP TABLE notice_schedule;
        ALTER TABLE notice_schedule_new RENAME TO notice_schedule;

        CREATE TABLE contract_item_new (
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
            delivery_due          TEXT,
            PRIMARY KEY (contract_base, seq, line_no),
            FOREIGN KEY (contract_base, seq) REFERENCES contract(contract_base, seq) ON DELETE CASCADE
        );
        INSERT INTO contract_item_new SELECT * FROM contract_item;
        DROP TABLE contract_item;
        ALTER TABLE contract_item_new RENAME TO contract_item;

        CREATE TABLE contract_attachment_new (
            contract_base TEXT NOT NULL,
            seq           TEXT NOT NULL,
            line_no       INTEGER NOT NULL,
            document_type TEXT,
            file_name     TEXT,
            PRIMARY KEY (contract_base, seq, line_no),
            FOREIGN KEY (contract_base, seq) REFERENCES contract(contract_base, seq) ON DELETE CASCADE
        );
        INSERT INTO contract_attachment_new SELECT * FROM contract_attachment;
        DROP TABLE contract_attachment;
        ALTER TABLE contract_attachment_new RENAME TO contract_attachment;

        -- ── 링크도 복합키로. 양쪽 모두 외래키를 건다 ──────────────────
        CREATE TABLE project_link_new (
            contract_base TEXT NOT NULL,
            contract_seq  TEXT NOT NULL,
            notice_base   TEXT NOT NULL,
            notice_seq    TEXT NOT NULL,
            confidence    REAL,
            confirmed_at  TEXT,
            PRIMARY KEY (contract_base, contract_seq),
            FOREIGN KEY (contract_base, contract_seq) REFERENCES contract(contract_base, seq) ON DELETE CASCADE,
            FOREIGN KEY (notice_base, notice_seq)     REFERENCES notice(notice_base, seq)     ON DELETE CASCADE
        );
        INSERT INTO project_link_new (contract_base, contract_seq, notice_base, notice_seq, confidence, confirmed_at)
        SELECT substr(contract_key, 1, length(contract_key) - 2),
               substr(contract_key, -2),
               substr(notice_key, 1, instr(notice_key, '-') - 1),
               substr(notice_key, instr(notice_key, '-') + 1),
               confidence, confirmed_at
        FROM project_link;
        DROP TABLE project_link;
        ALTER TABLE project_link_new RENAME TO project_link;

        CREATE INDEX ix_project_link_notice ON project_link(notice_base, notice_seq);

        -- ── 사용자 값을 종류별로 나눈다. 하나로 두면 외래키를 걸 수 없다 ──
        CREATE TABLE contract_user_field (
            contract_base TEXT NOT NULL,
            seq           TEXT NOT NULL,
            field_name    TEXT NOT NULL,
            value         TEXT,
            updated_at    TEXT NOT NULL,
            PRIMARY KEY (contract_base, seq, field_name),
            FOREIGN KEY (contract_base, seq) REFERENCES contract(contract_base, seq) ON DELETE CASCADE
        );
        INSERT INTO contract_user_field
        SELECT substr(entity_key, 1, length(entity_key) - 2), substr(entity_key, -2),
               field_name, value, updated_at
        FROM user_field WHERE entity_type = 'contract';

        CREATE TABLE notice_user_field (
            notice_base TEXT NOT NULL,
            seq         TEXT NOT NULL,
            field_name  TEXT NOT NULL,
            value       TEXT,
            updated_at  TEXT NOT NULL,
            PRIMARY KEY (notice_base, seq, field_name),
            FOREIGN KEY (notice_base, seq) REFERENCES notice(notice_base, seq) ON DELETE CASCADE
        );
        INSERT INTO notice_user_field
        SELECT substr(entity_key, 1, instr(entity_key, '-') - 1),
               substr(entity_key, instr(entity_key, '-') + 1),
               field_name, value, updated_at
        FROM user_field WHERE entity_type = 'notice';

        DROP TABLE user_field;

        -- 열 정의는 참조가 아니라 종류 이름이라 외래키 대신 값을 제한한다.
        CREATE TABLE user_column_new (
            entity_type TEXT NOT NULL CHECK (entity_type IN ('contract', 'notice')),
            field_name  TEXT NOT NULL,
            kind        TEXT NOT NULL DEFAULT 'text' CHECK (kind IN ('text', 'choice', 'date', 'number')),
            options     TEXT,
            sort_order  INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (entity_type, field_name)
        );
        INSERT INTO user_column_new SELECT * FROM user_column;
        DROP TABLE user_column;
        ALTER TABLE user_column_new RENAME TO user_column;

        -- document 는 감사 기록이라 대상이 사라져도 남아야 한다. 표현만 맞춘다.
        ALTER TABLE document RENAME COLUMN entity_key TO entity_key_text;
        """;

    /// <summary>
    /// 사람이 손으로 채우는 열의 정의. 값은 <c>user_field</c> 에 담기고 여기엔 <b>무슨 열이 있는지</b>만 둔다.
    /// 열을 늘려도 스키마를 고칠 필요가 없게 이렇게 나눴다.
    /// </summary>
    private const string V2 = """
        CREATE TABLE IF NOT EXISTS user_column (
            entity_type TEXT NOT NULL,
            field_name  TEXT NOT NULL,
            kind        TEXT NOT NULL DEFAULT 'text',
            options     TEXT,
            sort_order  INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (entity_type, field_name)
        );

        -- 처음 쓸 때 쓸 만한 기본 열. 마음에 안 들면 지우거나 더하면 된다.
        -- choice 후보는 세로줄로 나눈다.
        INSERT OR IGNORE INTO user_column (entity_type, field_name, kind, options, sort_order) VALUES
            ('contract', '내부관리번호', 'text',   NULL, 10),
            ('contract', '진행상태',     'choice', '준비|투찰|낙찰|계약|납품|검수|정산|종료', 20),
            ('contract', '담당',        'text',   NULL, 30),
            ('contract', '메모',        'text',   NULL, 40),
            ('notice',   '검토여부',     'choice', '미검토|검토중|참여|불참', 10),
            ('notice',   '메모',        'text',   NULL, 20);
        """;

    private const string V1 = """
        -- 투입 이력. 원본은 담지 않고 지문과 결과만 남긴다.
        CREATE TABLE IF NOT EXISTS document (
            sha256       TEXT PRIMARY KEY,
            source_name  TEXT NOT NULL,
            source_path  TEXT,
            form_type    TEXT NOT NULL,
            entity_key   TEXT,
            parse_status TEXT NOT NULL,
            parse_error  TEXT,
            ingested_at  TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS notice (
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
            updated_at               TEXT NOT NULL,
            PRIMARY KEY (notice_base, seq)
        );

        CREATE TABLE IF NOT EXISTS notice_item (
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
            delivery_terms     TEXT,
            PRIMARY KEY (notice_base, seq, line_no)
        );

        CREATE TABLE IF NOT EXISTS notice_schedule (
            notice_base TEXT NOT NULL,
            seq         TEXT NOT NULL,
            line_no     INTEGER NOT NULL,
            name        TEXT,
            method      TEXT,
            starts_at   TEXT,
            ends_at     TEXT,
            place       TEXT,
            PRIMARY KEY (notice_base, seq, line_no)
        );

        -- 계약상대자. 사업자등록번호가 자연키다. 주민등록번호는 담지 않는다(ADR-005).
        CREATE TABLE IF NOT EXISTS counterparty (
            business_number TEXT PRIMARY KEY,
            name            TEXT,
            representative  TEXT,
            address         TEXT,
            phone           TEXT,
            fax             TEXT,
            updated_at      TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS contract (
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
            updated_at                 TEXT NOT NULL,
            PRIMARY KEY (contract_base, seq)
        );

        CREATE TABLE IF NOT EXISTS contract_item (
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
            delivery_due          TEXT,
            PRIMARY KEY (contract_base, seq, line_no)
        );

        CREATE TABLE IF NOT EXISTS contract_attachment (
            contract_base TEXT NOT NULL,
            seq           TEXT NOT NULL,
            line_no       INTEGER NOT NULL,
            document_type TEXT,
            file_name     TEXT,
            PRIMARY KEY (contract_base, seq, line_no)
        );

        -- 사람이 손으로 넣는 값. 파서는 이 테이블에 절대 쓰지 않는다(ADR-007).
        -- 그래서 다시 투입해도 사용자가 적은 것이 지워질 수 없다.
        CREATE TABLE IF NOT EXISTS user_field (
            entity_type TEXT NOT NULL,
            entity_key  TEXT NOT NULL,
            field_name  TEXT NOT NULL,
            value       TEXT,
            updated_at  TEXT NOT NULL,
            PRIMARY KEY (entity_type, entity_key, field_name)
        );

        -- 공고와 계약을 잇는 확정 기록. 계약서에 공고번호가 찍히지 않아
        -- 자동으로는 후보만 내고 사람이 확정한다(ADR-004).
        CREATE TABLE IF NOT EXISTS project_link (
            contract_key TEXT PRIMARY KEY,
            notice_key   TEXT NOT NULL,
            confidence   REAL,
            confirmed_at TEXT
        );

        CREATE INDEX IF NOT EXISTS ix_notice_title    ON notice(title);
        CREATE INDEX IF NOT EXISTS ix_contract_title  ON contract(title);
        CREATE INDEX IF NOT EXISTS ix_document_entity ON document(entity_key);
        """;
}

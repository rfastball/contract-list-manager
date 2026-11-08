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
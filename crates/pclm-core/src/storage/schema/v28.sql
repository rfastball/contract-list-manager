ALTER TABLE request ADD COLUMN notice_kind TEXT;
ALTER TABLE request ADD COLUMN estimated_price TEXT;
ALTER TABLE request_item ADD COLUMN classification_number TEXT;
ALTER TABLE request_item ADD COLUMN delivery_unit TEXT;

ALTER TABLE notice ADD COLUMN contract_law TEXT;
ALTER TABLE notice ADD COLUMN project_fee_total TEXT;
ALTER TABLE notice ADD COLUMN bid_status TEXT;
ALTER TABLE notice_item ADD COLUMN demand_agency_code TEXT;
ALTER TABLE notice_item ADD COLUMN classification_number TEXT;
ALTER TABLE notice_item ADD COLUMN item_identification_name TEXT;
ALTER TABLE notice_item ADD COLUMN spec_number TEXT;
ALTER TABLE notice_item ADD COLUMN request_number TEXT;

-- 입찰참가 제한 업종. 여럿일 수 있어 딸린 표다 — 공고 뷰에는 줄 차례대로 이어 붙인다.
CREATE TABLE notice_restricted_industry (
    notice_base TEXT NOT NULL,
    seq         TEXT NOT NULL,
    line_no     INTEGER NOT NULL,
    name        TEXT,
    PRIMARY KEY (notice_base, seq, line_no),
    FOREIGN KEY (notice_base, seq) REFERENCES notice(notice_base, seq) ON DELETE CASCADE
);

ALTER TABLE contract ADD COLUMN agency_officer_id TEXT;
ALTER TABLE contract ADD COLUMN foreign_allowed INTEGER;
ALTER TABLE contract ADD COLUMN contract_law TEXT;
ALTER TABLE contract ADD COLUMN price_adjustment_basis TEXT;
ALTER TABLE contract ADD COLUMN disclosure TEXT;
ALTER TABLE erp_partner ADD COLUMN joint_method TEXT;
ALTER TABLE contract_item ADD COLUMN ref_notice_class TEXT;
ALTER TABLE contract_item ADD COLUMN ref_notice_item TEXT;
ALTER TABLE contract_item ADD COLUMN delivery_place TEXT;
ALTER TABLE contract_item ADD COLUMN delivery_days INTEGER;
ALTER TABLE contract_item ADD COLUMN demand_agency_code TEXT;
ALTER TABLE contract_item ADD COLUMN request_number TEXT;
ALTER TABLE contract_item ADD COLUMN origin_country TEXT;
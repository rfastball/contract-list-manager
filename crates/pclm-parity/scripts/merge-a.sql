-- 취합 대조(merge-*)의 첫 제출본 — 홍길동. seed 의 TEST… 번호에 지어낸 값만 적는다.
INSERT INTO app_setting (key, value) VALUES ('submit.name', '홍길동');
INSERT INTO app_setting (key, value) VALUES ('plan.path', 'C:\남의폴더\조달계획.xlsx');
INSERT INTO counterparty (business_number, name, updated_at) VALUES ('123-45-67890', '지어낸 상사', '2026-09-01T15:30:00.0000000Z');
INSERT INTO plan (request_number, item_name, quantity, source_name, imported_at)
VALUES ('P-1', '지어낸 계획 품목', 3, '지어낸 계획.xlsx', '2026-09-01T09:00:00.0000000Z');
UPDATE request SET title = '홍길동이 고친 접수명', updated_at = '2030-01-01T00:00:00.0000000Z' WHERE request_base = 'TESTR0001';
UPDATE notice SET title = '홍길동이 고친 공고명', updated_at = '2026-09-03T09:00:00.0000000Z' WHERE notice_base = 'TESTN0001';
UPDATE contract SET title = '홍길동이 고친 건명', updated_at = '2026-09-02T15:30:00.0000000Z' WHERE contract_base = 'TESTK0001';
UPDATE contract_item SET item_name = '홍길동이 읽은 품목' WHERE contract_base = 'TESTK0001';

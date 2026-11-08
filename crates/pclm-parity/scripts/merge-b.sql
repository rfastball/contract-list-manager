-- 취합 대조의 둘째 제출본 — 김철수. 같은 키를 다른 값·다른 시각으로 들고 온다.
INSERT INTO app_setting (key, value) VALUES ('submit.name', '김철수');
INSERT INTO counterparty (business_number, name, updated_at) VALUES ('123-45-67890', '지어낸 상사(주)', '2026-09-05T09:00:00.0000000Z');
INSERT INTO plan (request_number, item_name, quantity, source_name, imported_at)
VALUES ('P-1', '지어낸 계획 품목', 4, '지어낸 계획.xlsx', '2026-08-30T09:00:00.0000000Z');
UPDATE request SET title = '김철수가 고친 접수명', updated_at = '2026-09-04T09:00:00.0000000Z' WHERE request_base = 'TESTR0001';
-- 같은 값을 다른 시각에 — 일치라 알리지 않는다.
UPDATE notice SET title = '홍길동이 고친 공고명', updated_at = '2026-09-04T09:00:00.0000000Z' WHERE notice_base = 'TESTN0001';
UPDATE notice SET title = '김철수가 고친 공고명', updated_at = '2026-09-04T09:00:00.0000000Z' WHERE notice_base = 'TESTN0002';
UPDATE contract SET title = '김철수가 고친 건명', updated_at = '2026-09-10T09:00:00.0000000Z' WHERE contract_base = 'TESTK0001';
UPDATE contract_item SET item_name = '김철수가 읽은 품목' WHERE contract_base = 'TESTK0001';
-- 예시 보기의 이름을 바꾼다 — 같은 식별자가 다른 이름으로 두 제출본에 있게 된다.
UPDATE user_view SET name = name || ' (김철수)';

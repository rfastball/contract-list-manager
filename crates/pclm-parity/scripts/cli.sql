-- 명령줄 대조(cli-*)의 작업자료 — seed 의 TEST… 계약 하나에 둘째 차수를 지어 넣는다(status 의 「차수가 쌓인 건」 ·
-- delete 의 차수 하나/전 차수). 값은 지어낸 것이다.
CREATE TEMP TABLE cli_contract AS SELECT * FROM contract WHERE contract_base = 'TESTK0002' AND seq = '00';
UPDATE cli_contract SET seq = '01', title = '시험 의자 계약 (변경)';
INSERT INTO contract SELECT * FROM cli_contract;

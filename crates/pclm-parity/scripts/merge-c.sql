-- 취합 대조의 셋째 — 제출하지 않은 작업자료 그대로(이름이 없어 파일 이름으로 불린다).
UPDATE notice SET title = '이몽룡이 고친 공고명', updated_at = '2026-09-06T09:00:00.0000000Z' WHERE notice_base = 'TESTN0002';
UPDATE user_view SET definition = replace(definition, '"name":"건명"', '"name":"이몽룡 건명"');

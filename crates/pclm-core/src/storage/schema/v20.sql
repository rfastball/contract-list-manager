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
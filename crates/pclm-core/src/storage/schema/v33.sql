DROP TABLE IF EXISTS plan;
DELETE FROM app_setting WHERE key IN ('plan.path','plan.mapping');

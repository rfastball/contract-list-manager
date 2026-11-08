ALTER TABLE notice RENAME COLUMN creditor TO finance_officer;
UPDATE notice SET finance_officer = NULL
WHERE finance_officer IS NOT NULL
  AND NOT EXISTS (
      SELECT 1 FROM erp_source s, json_each(s.document_json, '$.rows') r
      WHERE s.entity_type = 'notice' AND s.entity_base = notice.notice_base AND s.entity_seq = notice.seq
        AND json_extract(r.value, '$.table') = 'notice'
        AND json_extract(r.value, '$.origins.creditor') IN ('g2b-notice-a-v1', 'g2b-notice-a-service-v1')
        AND json_extract(r.value, '$.values.creditor') = notice.finance_officer);
CREATE TABLE user_view_next (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL UNIQUE,
    definition TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0
);
INSERT INTO user_view_next (id, name, definition, sort_order)
SELECT v.id, v.name,
    CASE WHEN json_valid(v.definition) AND json_type(v.definition, '$.columns') = 'array' THEN json_object(
        'version', 2,
        'contracts', CASE WHEN EXISTS (SELECT 1 FROM json_each(v.definition, '$.columns') c WHERE (v.base IN ('notice', 'request')
     AND json_extract(c.value, '$.formula.source') = 'flow'
     AND (SELECT count(*) FROM json_each(c.value, '$.formula.flow.from')) = 2
     AND json_extract(c.value, '$.formula.flow.from.kind') = 'linked'
     AND json_extract(c.value, '$.formula.flow.from.link') = 'contract'
     AND json_array_length(c.value, '$.formula.flow.steps') = 2
     AND (SELECT count(*) FROM json_each(c.value, '$.formula.flow.steps[0]')) = 2
     AND json_extract(c.value, '$.formula.flow.steps[0].op') = 'get'
     AND json_type(c.value, '$.formula.flow.steps[0].field') = 'text'
     AND (SELECT count(*) FROM json_each(c.value, '$.formula.flow.steps[1]')) = 2
     AND json_extract(c.value, '$.formula.flow.steps[1].op') = 'join'
     AND json_extract(c.value, '$.formula.flow.steps[1].text') = ', ')) THEN 'fold' ELSE 'split' END,
        'columns', json(coalesce((SELECT json_group_array(json(
            CASE
                WHEN (v.base IN ('notice', 'request')
     AND json_extract(c.value, '$.formula.source') = 'flow'
     AND (SELECT count(*) FROM json_each(c.value, '$.formula.flow.from')) = 2
     AND json_extract(c.value, '$.formula.flow.from.kind') = 'linked'
     AND json_extract(c.value, '$.formula.flow.from.link') = 'contract'
     AND json_array_length(c.value, '$.formula.flow.steps') = 2
     AND (SELECT count(*) FROM json_each(c.value, '$.formula.flow.steps[0]')) = 2
     AND json_extract(c.value, '$.formula.flow.steps[0].op') = 'get'
     AND json_type(c.value, '$.formula.flow.steps[0].field') = 'text'
     AND (SELECT count(*) FROM json_each(c.value, '$.formula.flow.steps[1]')) = 2
     AND json_extract(c.value, '$.formula.flow.steps[1].op') = 'join'
     AND json_extract(c.value, '$.formula.flow.steps[1].text') = ', ') THEN json_set(c.value, '$.formula.flow', json_object(
                    'from', json_object('kind', 'linked', 'link', 'contract', 'field', json_extract(c.value, '$.formula.flow.steps[0].field')),
                    'steps', json('[]')))
                WHEN json_extract(c.value, '$.formula.source') = 'flow' THEN json_set(c.value,
                    '$.formula.flow.from', json(CASE json_extract(c.value, '$.formula.flow.from.kind')
                        WHEN 'field' THEN json_object('kind', 'linked', 'link', v.base, 'field', json_extract(c.value, '$.formula.flow.from.field'))
                        WHEN 'items' THEN json_object('kind', 'linked', 'link', v.base, 'items', json('true'))
                        ELSE json_extract(c.value, '$.formula.flow.from') END),
                    '$.formula.flow.steps', json(coalesce((SELECT json_group_array(json(
                        CASE WHEN json_extract(s.value, '$.op') IN ('ifEmpty', 'add', 'sub', 'mul', 'div', 'dateDiff')
                                  AND json_type(s.value, '$.field') = 'text'
                             THEN json_set(s.value, '$.field',
                                  CASE v.base WHEN 'notice' THEN '공고' WHEN 'request' THEN '접수' ELSE '계약' END
                                  || '.' || json_extract(s.value, '$.field'))
                             ELSE s.value END) ORDER BY s.key)
                        FROM json_each(c.value, '$.formula.flow.steps') s), '[]')))
                ELSE c.value
            END) ORDER BY c.key)
            FROM json_each(v.definition, '$.columns') c), '[]')))
    ELSE v.definition END,
    v.sort_order
FROM user_view v;
DROP TABLE user_view;
ALTER TABLE user_view_next RENAME TO user_view;
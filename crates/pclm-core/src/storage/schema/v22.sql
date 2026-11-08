-- C# 의 Schema.Steps[3] 과 한 군데 다르다: steps 부질의를 json(…) 으로 감쌌다. SQLite 3.54 는 스칼라 부질의를 지난
-- 값의 JSON 하위형을 떨어뜨려(3.53 은 넘긴다), 감싸지 않으면 steps 가 배열이 아니라 글로 담긴다. 감싸도 3.53 이 내던
-- 글자와 같다 — json_group_array 가 이미 줄인 꼴을 내므로 json() 이 바꿀 것이 없다(계기 migrate-v21 이 본다).
-- 읽을 수 없는 식(손으로 깨뜨린 글)은 건너뛴다 — 그 열은 뒤에 깨진 열로 선다(C# 과 같은 WHERE).
UPDATE user_column
SET options = json_object(
    'version', 2,
    'id', coalesce(json_extract(options, '$.id'), ''),
    'source', 'flow',
    'flow', json_object(
        'from', json_object('kind', 'items'),
        'steps', json((
            SELECT json_group_array(json(
                CASE json_extract(s.value, '$.op')
                    WHEN 'distinct' THEN coalesce(
                        (SELECT CASE WHEN json_array_length(json_extract(p.value, '$.fields')) > 0
                                     THEN json_object('op', 'distinct', 'fields', json(json_extract(p.value, '$.fields'))) END
                         FROM json_each(user_column.options, '$.steps') p
                         WHERE p.key < s.key AND json_extract(p.value, '$.op') = 'pick'
                         ORDER BY p.key DESC LIMIT 1),
                        json_object('op', 'distinct'))
                    WHEN 'at' THEN json_object('op', 'nth', 'n', coalesce(json_extract(s.value, '$.index'), 1))
                    WHEN 'get' THEN json_object('op', 'get', 'field', coalesce(json_extract(s.value, '$.text'), ''))
                    WHEN 'format' THEN json_object('op', 'format', 'text', coalesce(json_extract(s.value, '$.text'), ''))
                    WHEN 'join' THEN json_object('op', 'join', 'text', coalesce(json_extract(s.value, '$.text'), ''))
                    ELSE json_object('op', json_extract(s.value, '$.op'))
                END) ORDER BY s.key)
            FROM json_each(user_column.options, '$.steps') s
            WHERE json_extract(s.value, '$.op') <> 'pick'))))
WHERE kind = 'computed' AND json_extract(CASE WHEN json_valid(options) THEN options END, '$.version') = 1;
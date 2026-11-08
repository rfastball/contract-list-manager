CREATE TABLE user_column_next (
    entity_type TEXT NOT NULL CHECK (entity_type IN ('contract', 'notice', 'request')),
    field_name TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'text' CHECK (kind IN ('text', 'choice', 'date', 'number', 'computed')),
    options TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (entity_type, field_name)
);
INSERT INTO user_column_next SELECT * FROM user_column;
DROP TABLE user_column;
ALTER TABLE user_column_next RENAME TO user_column;
CREATE TABLE user_view (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL UNIQUE,
    base       TEXT NOT NULL CHECK (base IN ('notice', 'contract', 'request')),
    definition TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE tournaments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    slug TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    is_active INTEGER NOT NULL DEFAULT 0,
    start_date TEXT,
    end_date TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

ALTER TABLE problems ADD COLUMN tournament_id INTEGER REFERENCES tournaments(id);

INSERT INTO tournaments (slug, name, description, is_active)
VALUES ('default', 'Default', 'The original problem set.', 1);

UPDATE problems SET tournament_id = (SELECT id FROM tournaments WHERE slug = 'default');

CREATE INDEX idx_problems_tournament ON problems(tournament_id);

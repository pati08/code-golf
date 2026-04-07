CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE,
    email TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    is_admin INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE problems (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    slug TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    difficulty TEXT NOT NULL DEFAULT 'medium',
    is_published INTEGER NOT NULL DEFAULT 0,
    time_limit_ms INTEGER NOT NULL DEFAULT 5000,
    memory_limit_kb INTEGER NOT NULL DEFAULT 65536,
    created_by INTEGER NOT NULL REFERENCES users(id),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE test_cases (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    problem_id INTEGER NOT NULL REFERENCES problems(id) ON DELETE CASCADE,
    input TEXT NOT NULL,
    expected_output TEXT NOT NULL,
    is_sample INTEGER NOT NULL DEFAULT 0,
    ordinal INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE languages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    file_extension TEXT NOT NULL,
    run_command TEXT NOT NULL,
    is_enabled INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE submissions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users(id),
    problem_id INTEGER NOT NULL REFERENCES problems(id),
    language_id INTEGER NOT NULL REFERENCES languages(id),
    code TEXT NOT NULL,
    byte_count INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    error_output TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    judged_at TEXT
);

CREATE TABLE best_submissions (
    user_id INTEGER NOT NULL REFERENCES users(id),
    problem_id INTEGER NOT NULL REFERENCES problems(id),
    language_id INTEGER NOT NULL REFERENCES languages(id),
    submission_id INTEGER NOT NULL REFERENCES submissions(id),
    byte_count INTEGER NOT NULL,
    PRIMARY KEY (user_id, problem_id, language_id)
);

CREATE INDEX idx_submissions_problem ON submissions(problem_id, status, byte_count);
CREATE INDEX idx_submissions_user ON submissions(user_id, created_at DESC);
CREATE INDEX idx_best_problem ON best_submissions(problem_id, byte_count);

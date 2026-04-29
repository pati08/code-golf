-- Migration: Add performance indexes

-- Index on submissions.status for faster filtering by status
CREATE INDEX IF NOT EXISTS idx_submissions_status ON submissions(status);

-- Index on best_submissions.user_id for profile queries
CREATE INDEX IF NOT EXISTS idx_best_submissions_user ON best_submissions(user_id);

-- Index on tournaments.created_at for ordering
CREATE INDEX IF NOT EXISTS idx_tournaments_created ON tournaments(created_at);

-- Index on problems.is_published for published problem filtering
CREATE INDEX IF NOT EXISTS idx_problems_published ON problems(is_published);

-- Index on problems.tournament_id for tournament problem lookups (may already exist from 0005)
CREATE INDEX IF NOT EXISTS idx_problems_tournament ON problems(tournament_id);

-- Index on submissions.user_id (idx_submissions_user already created in 001 with created_at; this is a no-op)
CREATE INDEX IF NOT EXISTS idx_submissions_user_status ON submissions(user_id, status);

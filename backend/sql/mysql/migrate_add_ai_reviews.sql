-- Thought Manifold MySQL migration: add AI review workflow tables
-- Safe to run multiple times.

USE thought_manifold;

ALTER TABLE posts
  ADD COLUMN IF NOT EXISTS is_published BOOLEAN NOT NULL DEFAULT TRUE,
  ADD COLUMN IF NOT EXISTS published_at DATETIME(6) NULL;

CREATE TABLE IF NOT EXISTS ai_review_statuses (
  id TINYINT UNSIGNED PRIMARY KEY,
  code VARCHAR(32) NOT NULL UNIQUE,
  display_name VARCHAR(128) NOT NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS ai_review_triggers (
  id TINYINT UNSIGNED PRIMARY KEY,
  code VARCHAR(32) NOT NULL UNIQUE,
  display_name VARCHAR(128) NOT NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS ai_review_decisions (
  id TINYINT UNSIGNED PRIMARY KEY,
  code VARCHAR(32) NOT NULL UNIQUE,
  display_name VARCHAR(128) NOT NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS post_ai_reviews (
  id BIGINT AUTO_INCREMENT PRIMARY KEY,
  post_id BIGINT NOT NULL,
  status_id TINYINT UNSIGNED NOT NULL,
  trigger_id TINYINT UNSIGNED NOT NULL,
  decision_id TINYINT UNSIGNED NULL,
  model VARCHAR(128) NOT NULL,
  prompt_version VARCHAR(32) NOT NULL,
  language_code VARCHAR(16) NOT NULL DEFAULT 'ko',
  overall_score TINYINT UNSIGNED NULL,
  novelty_score TINYINT UNSIGNED NULL,
  methodology_score TINYINT UNSIGNED NULL,
  clarity_score TINYINT UNSIGNED NULL,
  citation_integrity_score TINYINT UNSIGNED NULL,
  editorial_summary TEXT NULL,
  peer_summary TEXT NULL,
  major_issues_json JSON NULL,
  minor_issues_json JSON NULL,
  required_revisions_json JSON NULL,
  strengths_json JSON NULL,
  input_snapshot_json JSON NULL,
  raw_response_json JSON NULL,
  error_message TEXT NULL,
  created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  completed_at DATETIME(6) NULL,
  INDEX idx_post_ai_reviews_post_created (post_id, created_at),
  INDEX idx_post_ai_reviews_status_created (status_id, created_at),
  CONSTRAINT fk_post_ai_reviews_post_id FOREIGN KEY (post_id) REFERENCES posts(id) ON DELETE CASCADE,
  CONSTRAINT fk_post_ai_reviews_status_id FOREIGN KEY (status_id) REFERENCES ai_review_statuses(id),
  CONSTRAINT fk_post_ai_reviews_trigger_id FOREIGN KEY (trigger_id) REFERENCES ai_review_triggers(id),
  CONSTRAINT fk_post_ai_reviews_decision_id FOREIGN KEY (decision_id) REFERENCES ai_review_decisions(id),
  CONSTRAINT chk_post_ai_reviews_overall_score CHECK (overall_score BETWEEN 1 AND 5 OR overall_score IS NULL),
  CONSTRAINT chk_post_ai_reviews_novelty_score CHECK (novelty_score BETWEEN 1 AND 5 OR novelty_score IS NULL),
  CONSTRAINT chk_post_ai_reviews_methodology_score CHECK (methodology_score BETWEEN 1 AND 5 OR methodology_score IS NULL),
  CONSTRAINT chk_post_ai_reviews_clarity_score CHECK (clarity_score BETWEEN 1 AND 5 OR clarity_score IS NULL),
  CONSTRAINT chk_post_ai_reviews_citation_integrity_score CHECK (citation_integrity_score BETWEEN 1 AND 5 OR citation_integrity_score IS NULL)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

INSERT IGNORE INTO ai_review_statuses (id, code, display_name) VALUES
  (1, 'pending', 'Pending'),
  (2, 'completed', 'Completed'),
  (3, 'failed', 'Failed');

INSERT IGNORE INTO ai_review_triggers (id, code, display_name) VALUES
  (1, 'auto_create', 'Automatic on Create'),
  (2, 'auto_update', 'Automatic on Update'),
  (3, 'manual', 'Manual Rerun');

INSERT IGNORE INTO ai_review_decisions (id, code, display_name) VALUES
  (1, 'accept', 'Accept'),
  (2, 'minor_revision', 'Minor Revision'),
  (3, 'major_revision', 'Major Revision'),
  (4, 'reject', 'Reject');

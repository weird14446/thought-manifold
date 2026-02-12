-- Thought Manifold MySQL migration: add AI review workflow tables
-- Safe to run multiple times.

USE thought_manifold;

ALTER TABLE posts
  ADD COLUMN IF NOT EXISTS is_published BOOLEAN NOT NULL DEFAULT TRUE,
  ADD COLUMN IF NOT EXISTS published_at DATETIME(6) NULL,
  ADD COLUMN IF NOT EXISTS paper_status VARCHAR(32) NOT NULL DEFAULT 'published';

SET @has_paper_status_check := (
  SELECT COUNT(*)
  FROM information_schema.table_constraints
  WHERE table_schema = DATABASE()
    AND table_name = 'posts'
    AND constraint_name = 'chk_posts_paper_status'
);
SET @add_check_sql := IF(
  @has_paper_status_check = 0,
  "ALTER TABLE posts ADD CONSTRAINT chk_posts_paper_status CHECK (paper_status IN ('draft', 'submitted', 'revision', 'accepted', 'published', 'rejected'))",
  "SELECT 1"
);
PREPARE stmt_add_check FROM @add_check_sql;
EXECUTE stmt_add_check;
DEALLOCATE PREPARE stmt_add_check;

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

-- Backfill paper status machine
UPDATE posts p
JOIN post_categories c ON c.id = p.category_id
LEFT JOIN (
  SELECT r.post_id, d.code AS decision
  FROM post_ai_reviews r
  JOIN ai_review_decisions d ON d.id = r.decision_id
  JOIN (
    SELECT post_id, MAX(id) AS max_id
    FROM post_ai_reviews
    WHERE status_id = 2
    GROUP BY post_id
  ) latest ON latest.post_id = r.post_id AND latest.max_id = r.id
  WHERE r.status_id = 2
) latest_review ON latest_review.post_id = p.id
SET p.paper_status = CASE
  WHEN latest_review.decision = 'accept' AND p.is_published = TRUE THEN 'published'
  WHEN latest_review.decision = 'accept' THEN 'accepted'
  WHEN latest_review.decision IN ('minor_revision', 'major_revision') THEN 'revision'
  WHEN latest_review.decision = 'reject' THEN 'rejected'
  ELSE p.paper_status
END
WHERE c.code = 'paper' AND latest_review.decision IS NOT NULL;

UPDATE posts p
JOIN post_categories c ON c.id = p.category_id
LEFT JOIN (
  SELECT r.post_id, s.code AS status_code
  FROM post_ai_reviews r
  JOIN ai_review_statuses s ON s.id = r.status_id
  JOIN (
    SELECT post_id, MAX(id) AS max_id
    FROM post_ai_reviews
    GROUP BY post_id
  ) latest ON latest.post_id = r.post_id AND latest.max_id = r.id
) latest_any ON latest_any.post_id = p.id
LEFT JOIN (
  SELECT post_id, MAX(id) AS latest_completed_id
  FROM post_ai_reviews
  WHERE status_id = 2
  GROUP BY post_id
) latest_completed ON latest_completed.post_id = p.id
SET p.paper_status = 'submitted'
WHERE c.code = 'paper'
  AND latest_completed.latest_completed_id IS NULL
  AND latest_any.status_code IN ('pending', 'failed');

UPDATE posts p
JOIN post_categories c ON c.id = p.category_id
SET p.paper_status = 'draft'
WHERE c.code = 'paper'
  AND (
    p.paper_status NOT IN ('draft', 'submitted', 'revision', 'accepted', 'published', 'rejected')
    OR (p.paper_status = 'published' AND p.is_published = FALSE)
  );

UPDATE posts p
JOIN post_categories c ON c.id = p.category_id
SET
  p.is_published = CASE WHEN p.paper_status = 'published' THEN TRUE ELSE FALSE END,
  p.published_at = CASE
    WHEN p.paper_status = 'published' THEN COALESCE(p.published_at, p.created_at)
    ELSE NULL
  END
WHERE c.code = 'paper';

UPDATE posts p
JOIN post_categories c ON c.id = p.category_id
SET
  p.paper_status = 'published',
  p.is_published = TRUE,
  p.published_at = COALESCE(p.published_at, p.created_at)
WHERE c.code <> 'paper';

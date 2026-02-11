-- Thought Manifold MySQL 3NF migration (legacy schema -> 3NF)
-- Target: MySQL 8.0+
-- Assumption: legacy schema includes posts.category, posts.file_path, posts.file_name, posts.view_count, posts.like_count,
--             post_citations(citing_post_id,cited_post_id,created_at), post_auto_citations(...)

USE thought_manifold;

START TRANSACTION;

CREATE TABLE IF NOT EXISTS post_categories (
  id SMALLINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
  code VARCHAR(64) NOT NULL UNIQUE,
  display_name VARCHAR(128) NOT NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS post_files (
  post_id BIGINT PRIMARY KEY,
  file_path TEXT NOT NULL,
  file_name VARCHAR(255) NOT NULL,
  created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  updated_at DATETIME(6) NULL,
  CONSTRAINT fk_post_files_post_id FOREIGN KEY (post_id) REFERENCES posts(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS post_stats (
  post_id BIGINT PRIMARY KEY,
  view_count BIGINT NOT NULL DEFAULT 0,
  like_count BIGINT NOT NULL DEFAULT 0,
  updated_at DATETIME(6) NULL,
  CONSTRAINT fk_post_stats_post_id FOREIGN KEY (post_id) REFERENCES posts(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS citation_sources (
  id TINYINT UNSIGNED PRIMARY KEY,
  code VARCHAR(32) NOT NULL UNIQUE,
  display_name VARCHAR(128) NOT NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

INSERT IGNORE INTO post_categories (code, display_name) VALUES
  ('paper', 'Paper'),
  ('essay', 'Essay'),
  ('note', 'Note'),
  ('report', 'Report'),
  ('other', 'Other');

INSERT IGNORE INTO citation_sources (id, code, display_name) VALUES
  (1, 'manual', 'Manual citation'),
  (2, 'auto', 'Automatic citation');

-- Keep legacy categories as domain master entries.
INSERT INTO post_categories (code, display_name)
SELECT DISTINCT
  LOWER(TRIM(p.category)) AS code,
  CONCAT(
    UCASE(LEFT(LOWER(TRIM(p.category)), 1)),
    SUBSTRING(LOWER(TRIM(p.category)), 2)
  ) AS display_name
FROM posts p
WHERE p.category IS NOT NULL
  AND TRIM(p.category) <> ''
ON DUPLICATE KEY UPDATE display_name = VALUES(display_name);

ALTER TABLE posts ADD COLUMN IF NOT EXISTS category_id SMALLINT UNSIGNED NULL AFTER summary;

UPDATE posts p
JOIN post_categories c ON c.code = LOWER(TRIM(p.category))
SET p.category_id = c.id
WHERE p.category_id IS NULL;

-- Fallback to "other" for null/empty categories.
UPDATE posts p
JOIN post_categories c ON c.code = 'other'
SET p.category_id = c.id
WHERE p.category_id IS NULL;

INSERT INTO post_files (post_id, file_path, file_name, created_at, updated_at)
SELECT
  p.id,
  p.file_path,
  p.file_name,
  p.created_at,
  p.updated_at
FROM posts p
WHERE p.file_path IS NOT NULL
  AND p.file_name IS NOT NULL
ON DUPLICATE KEY UPDATE
  file_path = VALUES(file_path),
  file_name = VALUES(file_name),
  updated_at = VALUES(updated_at);

INSERT INTO post_stats (post_id, view_count, like_count, updated_at)
SELECT
  p.id,
  COALESCE(p.view_count, 0),
  COALESCE(p.like_count, 0),
  p.updated_at
FROM posts p
ON DUPLICATE KEY UPDATE
  view_count = VALUES(view_count),
  like_count = VALUES(like_count),
  updated_at = VALUES(updated_at);

CREATE TABLE IF NOT EXISTS post_citations_3nf (
  citing_post_id BIGINT NOT NULL,
  cited_post_id BIGINT NOT NULL,
  citation_source_id TINYINT UNSIGNED NOT NULL,
  created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  PRIMARY KEY (citing_post_id, cited_post_id, citation_source_id),
  CONSTRAINT chk_post_citations_3nf_no_self CHECK (citing_post_id <> cited_post_id),
  INDEX idx_post_citations_3nf_citation_source_id (citation_source_id),
  INDEX idx_post_citations_3nf_cited_post_id (cited_post_id),
  CONSTRAINT fk_post_citations_3nf_citing_post_id FOREIGN KEY (citing_post_id) REFERENCES posts(id) ON DELETE CASCADE,
  CONSTRAINT fk_post_citations_3nf_cited_post_id FOREIGN KEY (cited_post_id) REFERENCES posts(id) ON DELETE CASCADE,
  CONSTRAINT fk_post_citations_3nf_source_id FOREIGN KEY (citation_source_id) REFERENCES citation_sources(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

INSERT IGNORE INTO post_citations_3nf (citing_post_id, cited_post_id, citation_source_id, created_at)
SELECT citing_post_id, cited_post_id, 1 AS citation_source_id, created_at
FROM post_citations;

INSERT IGNORE INTO post_citations_3nf (citing_post_id, cited_post_id, citation_source_id, created_at)
SELECT citing_post_id, cited_post_id, 2 AS citation_source_id, created_at
FROM post_auto_citations;

DROP TABLE IF EXISTS post_citations;
DROP TABLE IF EXISTS post_auto_citations;
RENAME TABLE post_citations_3nf TO post_citations;

ALTER TABLE posts DROP INDEX idx_posts_category_created_at;
ALTER TABLE posts
  DROP COLUMN category,
  DROP COLUMN file_path,
  DROP COLUMN file_name,
  DROP COLUMN view_count,
  DROP COLUMN like_count,
  MODIFY COLUMN category_id SMALLINT UNSIGNED NOT NULL,
  ADD INDEX idx_posts_category_created_at (category_id, created_at),
  ADD CONSTRAINT fk_posts_category_id FOREIGN KEY (category_id) REFERENCES post_categories(id);

COMMIT;

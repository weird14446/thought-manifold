USE thought_manifold;

SET @has_posts_current_revision := (
  SELECT COUNT(*)
  FROM information_schema.columns
  WHERE table_schema = DATABASE()
    AND table_name = 'posts'
    AND column_name = 'current_revision'
);
SET @sql_posts_current_revision := IF(
  @has_posts_current_revision = 0,
  "ALTER TABLE posts ADD COLUMN current_revision INT UNSIGNED NOT NULL DEFAULT 0",
  "SELECT 1"
);
PREPARE stmt_posts_current_revision FROM @sql_posts_current_revision;
EXECUTE stmt_posts_current_revision;
DEALLOCATE PREPARE stmt_posts_current_revision;

SET @has_posts_latest_paper_version_id := (
  SELECT COUNT(*)
  FROM information_schema.columns
  WHERE table_schema = DATABASE()
    AND table_name = 'posts'
    AND column_name = 'latest_paper_version_id'
);
SET @sql_posts_latest_paper_version_id := IF(
  @has_posts_latest_paper_version_id = 0,
  "ALTER TABLE posts ADD COLUMN latest_paper_version_id BIGINT NULL",
  "SELECT 1"
);
PREPARE stmt_posts_latest_paper_version_id FROM @sql_posts_latest_paper_version_id;
EXECUTE stmt_posts_latest_paper_version_id;
DEALLOCATE PREPARE stmt_posts_latest_paper_version_id;

SET @has_post_ai_reviews_paper_version_id := (
  SELECT COUNT(*)
  FROM information_schema.columns
  WHERE table_schema = DATABASE()
    AND table_name = 'post_ai_reviews'
    AND column_name = 'paper_version_id'
);
SET @sql_post_ai_reviews_paper_version_id := IF(
  @has_post_ai_reviews_paper_version_id = 0,
  "ALTER TABLE post_ai_reviews ADD COLUMN paper_version_id BIGINT NULL",
  "SELECT 1"
);
PREPARE stmt_post_ai_reviews_paper_version_id FROM @sql_post_ai_reviews_paper_version_id;
EXECUTE stmt_post_ai_reviews_paper_version_id;
DEALLOCATE PREPARE stmt_post_ai_reviews_paper_version_id;

CREATE TABLE IF NOT EXISTS paper_versions (
  id BIGINT AUTO_INCREMENT PRIMARY KEY,
  post_id BIGINT NOT NULL,
  version_number INT UNSIGNED NOT NULL,
  title VARCHAR(255) NOT NULL,
  content LONGTEXT NOT NULL,
  summary TEXT NULL,
  github_url VARCHAR(2048) NULL,
  file_path TEXT NULL,
  file_name VARCHAR(255) NULL,
  tags_json JSON NULL,
  citations_json JSON NULL,
  submitted_by BIGINT NULL,
  submitted_at DATETIME(6) NOT NULL,
  created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  UNIQUE KEY uq_paper_versions_post_version (post_id, version_number),
  INDEX idx_paper_versions_post_version (post_id, version_number),
  INDEX idx_paper_versions_submitted_at (submitted_at),
  CONSTRAINT fk_paper_versions_post_id FOREIGN KEY (post_id) REFERENCES posts(id) ON DELETE CASCADE,
  CONSTRAINT fk_paper_versions_submitted_by FOREIGN KEY (submitted_by) REFERENCES users(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS paper_review_comments (
  id BIGINT AUTO_INCREMENT PRIMARY KEY,
  post_id BIGINT NOT NULL,
  paper_version_id BIGINT NULL,
  author_id BIGINT NOT NULL,
  parent_comment_id BIGINT NULL,
  content TEXT NOT NULL,
  is_deleted BOOLEAN NOT NULL DEFAULT FALSE,
  deleted_at DATETIME(6) NULL,
  created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  updated_at DATETIME(6) NULL,
  INDEX idx_paper_review_comments_post_version_created (post_id, paper_version_id, created_at),
  INDEX idx_paper_review_comments_parent_created (parent_comment_id, created_at),
  INDEX idx_paper_review_comments_author_created (author_id, created_at),
  CONSTRAINT fk_paper_review_comments_post_id FOREIGN KEY (post_id) REFERENCES posts(id) ON DELETE CASCADE,
  CONSTRAINT fk_paper_review_comments_version_id FOREIGN KEY (paper_version_id) REFERENCES paper_versions(id) ON DELETE SET NULL,
  CONSTRAINT fk_paper_review_comments_author_id FOREIGN KEY (author_id) REFERENCES users(id) ON DELETE CASCADE,
  CONSTRAINT fk_paper_review_comments_parent_id FOREIGN KEY (parent_comment_id) REFERENCES paper_review_comments(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

SET @has_idx_posts_latest_paper_version_id := (
  SELECT COUNT(*)
  FROM information_schema.statistics
  WHERE table_schema = DATABASE()
    AND table_name = 'posts'
    AND index_name = 'idx_posts_latest_paper_version_id'
);
SET @sql_idx_posts_latest_paper_version_id := IF(
  @has_idx_posts_latest_paper_version_id = 0,
  "CREATE INDEX idx_posts_latest_paper_version_id ON posts (latest_paper_version_id)",
  "SELECT 1"
);
PREPARE stmt_idx_posts_latest_paper_version_id FROM @sql_idx_posts_latest_paper_version_id;
EXECUTE stmt_idx_posts_latest_paper_version_id;
DEALLOCATE PREPARE stmt_idx_posts_latest_paper_version_id;

SET @has_idx_post_ai_reviews_version_created := (
  SELECT COUNT(*)
  FROM information_schema.statistics
  WHERE table_schema = DATABASE()
    AND table_name = 'post_ai_reviews'
    AND index_name = 'idx_post_ai_reviews_version_created'
);
SET @sql_idx_post_ai_reviews_version_created := IF(
  @has_idx_post_ai_reviews_version_created = 0,
  "CREATE INDEX idx_post_ai_reviews_version_created ON post_ai_reviews (paper_version_id, created_at)",
  "SELECT 1"
);
PREPARE stmt_idx_post_ai_reviews_version_created FROM @sql_idx_post_ai_reviews_version_created;
EXECUTE stmt_idx_post_ai_reviews_version_created;
DEALLOCATE PREPARE stmt_idx_post_ai_reviews_version_created;

SET @has_fk_posts_latest_paper_version_id := (
  SELECT COUNT(*)
  FROM information_schema.table_constraints
  WHERE table_schema = DATABASE()
    AND table_name = 'posts'
    AND constraint_name = 'fk_posts_latest_paper_version_id'
);
SET @sql_fk_posts_latest_paper_version_id := IF(
  @has_fk_posts_latest_paper_version_id = 0,
  "ALTER TABLE posts ADD CONSTRAINT fk_posts_latest_paper_version_id FOREIGN KEY (latest_paper_version_id) REFERENCES paper_versions(id) ON DELETE SET NULL",
  "SELECT 1"
);
PREPARE stmt_fk_posts_latest_paper_version_id FROM @sql_fk_posts_latest_paper_version_id;
EXECUTE stmt_fk_posts_latest_paper_version_id;
DEALLOCATE PREPARE stmt_fk_posts_latest_paper_version_id;

SET @has_fk_post_ai_reviews_paper_version_id := (
  SELECT COUNT(*)
  FROM information_schema.table_constraints
  WHERE table_schema = DATABASE()
    AND table_name = 'post_ai_reviews'
    AND constraint_name = 'fk_post_ai_reviews_paper_version_id'
);
SET @sql_fk_post_ai_reviews_paper_version_id := IF(
  @has_fk_post_ai_reviews_paper_version_id = 0,
  "ALTER TABLE post_ai_reviews ADD CONSTRAINT fk_post_ai_reviews_paper_version_id FOREIGN KEY (paper_version_id) REFERENCES paper_versions(id) ON DELETE SET NULL",
  "SELECT 1"
);
PREPARE stmt_fk_post_ai_reviews_paper_version_id FROM @sql_fk_post_ai_reviews_paper_version_id;
EXECUTE stmt_fk_post_ai_reviews_paper_version_id;
DEALLOCATE PREPARE stmt_fk_post_ai_reviews_paper_version_id;

INSERT INTO paper_versions (
  post_id,
  version_number,
  title,
  content,
  summary,
  github_url,
  file_path,
  file_name,
  tags_json,
  citations_json,
  submitted_by,
  submitted_at,
  created_at
)
SELECT
  p.id,
  1,
  p.title,
  p.content,
  p.summary,
  p.github_url,
  pf.file_path,
  pf.file_name,
  (
    SELECT
      CASE
        WHEN COUNT(*) = 0 THEN NULL
        ELSE JSON_ARRAYAGG(t.name)
      END
    FROM post_tags pt
    JOIN tags t ON t.id = pt.tag_id
    WHERE pt.post_id = p.id
  ),
  (
    SELECT
      CASE
        WHEN COUNT(*) = 0 THEN NULL
        ELSE JSON_ARRAYAGG(pc.cited_post_id)
      END
    FROM post_citations pc
    WHERE pc.citing_post_id = p.id
      AND pc.citation_source_id = 1
  ),
  p.author_id,
  COALESCE(p.updated_at, p.created_at),
  COALESCE(p.updated_at, p.created_at)
FROM posts p
JOIN post_categories c ON c.id = p.category_id
LEFT JOIN post_files pf ON pf.post_id = p.id
LEFT JOIN paper_versions v ON v.post_id = p.id AND v.version_number = 1
WHERE c.code = 'paper'
  AND p.paper_status <> 'draft'
  AND v.id IS NULL;

UPDATE posts p
JOIN post_categories c ON c.id = p.category_id
SET
  p.current_revision = COALESCE(
    (
      SELECT MAX(v.version_number)
      FROM paper_versions v
      WHERE v.post_id = p.id
    ),
    0
  ),
  p.latest_paper_version_id = (
    SELECT v2.id
    FROM paper_versions v2
    WHERE v2.post_id = p.id
    ORDER BY v2.version_number DESC, v2.id DESC
    LIMIT 1
  )
WHERE c.code = 'paper';

UPDATE posts p
JOIN post_categories c ON c.id = p.category_id
SET
  p.current_revision = 0,
  p.latest_paper_version_id = NULL
WHERE c.code = 'paper'
  AND p.paper_status = 'draft';

UPDATE post_ai_reviews r
JOIN posts p ON p.id = r.post_id
SET r.paper_version_id = p.latest_paper_version_id
WHERE r.paper_version_id IS NULL
  AND p.latest_paper_version_id IS NOT NULL;

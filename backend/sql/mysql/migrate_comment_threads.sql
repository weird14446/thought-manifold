-- Thought Manifold MySQL migration: comment thread support
-- Safe to run multiple times.

USE thought_manifold;

ALTER TABLE comments
  ADD COLUMN IF NOT EXISTS parent_comment_id BIGINT NULL AFTER author_id,
  ADD COLUMN IF NOT EXISTS is_deleted BOOLEAN NOT NULL DEFAULT FALSE AFTER content,
  ADD COLUMN IF NOT EXISTS deleted_at DATETIME(6) NULL AFTER is_deleted;

SET @has_comments_parent_index := (
  SELECT COUNT(*)
  FROM information_schema.statistics
  WHERE table_schema = DATABASE()
    AND table_name = 'comments'
    AND index_name = 'idx_comments_post_parent_created'
);
SET @add_comments_parent_index_sql := IF(
  @has_comments_parent_index = 0,
  "CREATE INDEX idx_comments_post_parent_created ON comments (post_id, parent_comment_id, created_at)",
  "SELECT 1"
);
PREPARE stmt_add_comments_parent_index FROM @add_comments_parent_index_sql;
EXECUTE stmt_add_comments_parent_index;
DEALLOCATE PREPARE stmt_add_comments_parent_index;

SET @has_comments_parent_fk := (
  SELECT COUNT(*)
  FROM information_schema.table_constraints
  WHERE table_schema = DATABASE()
    AND table_name = 'comments'
    AND constraint_name = 'fk_comments_parent_comment_id'
);
SET @add_comments_parent_fk_sql := IF(
  @has_comments_parent_fk = 0,
  "ALTER TABLE comments ADD CONSTRAINT fk_comments_parent_comment_id FOREIGN KEY (parent_comment_id) REFERENCES comments(id) ON DELETE SET NULL",
  "SELECT 1"
);
PREPARE stmt_add_comments_parent_fk FROM @add_comments_parent_fk_sql;
EXECUTE stmt_add_comments_parent_fk;
DEALLOCATE PREPARE stmt_add_comments_parent_fk;

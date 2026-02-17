-- Thought Manifold MySQL Schema
-- Recommended MySQL version: 8.0+
-- 3NF notes:
-- 1) post_categories: category domain master (posts.category_id FK)
-- 2) post_files: optional attachment attributes split from posts
-- 3) post_stats: mutable counters split from posts
-- 4) citation_sources + post_citations: citation relation + source type master
-- 5) ai_review_* + post_ai_reviews: AI paper review workflow + history

CREATE DATABASE IF NOT EXISTS thought_manifold
  CHARACTER SET utf8mb4
  COLLATE utf8mb4_unicode_ci;

USE thought_manifold;

CREATE TABLE IF NOT EXISTS users (
  id BIGINT AUTO_INCREMENT PRIMARY KEY,
  username VARCHAR(191) NOT NULL UNIQUE,
  email VARCHAR(191) NOT NULL UNIQUE,
  hashed_password VARCHAR(255) NULL,
  google_id VARCHAR(191) NULL UNIQUE,
  display_name VARCHAR(255) NULL,
  bio TEXT NULL,
  introduction TEXT NULL,
  hobbies TEXT NULL,
  interests TEXT NULL,
  research_areas TEXT NULL,
  avatar_url TEXT NULL,
  is_admin BOOLEAN NOT NULL DEFAULT FALSE,
  created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  updated_at DATETIME(6) NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS post_categories (
  id SMALLINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
  code VARCHAR(64) NOT NULL UNIQUE,
  display_name VARCHAR(128) NOT NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS posts (
  id BIGINT AUTO_INCREMENT PRIMARY KEY,
  title VARCHAR(255) NOT NULL,
  content LONGTEXT NOT NULL,
  summary TEXT NULL,
  github_url VARCHAR(2048) NULL,
  category_id SMALLINT UNSIGNED NOT NULL,
  author_id BIGINT NOT NULL,
  is_published BOOLEAN NOT NULL DEFAULT TRUE,
  published_at DATETIME(6) NULL,
  paper_status VARCHAR(32) NOT NULL DEFAULT 'published',
  current_revision INT UNSIGNED NOT NULL DEFAULT 0,
  latest_paper_version_id BIGINT NULL,
  created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  updated_at DATETIME(6) NULL,
  INDEX idx_posts_author_id (author_id),
  INDEX idx_posts_published_created_at (is_published, created_at),
  INDEX idx_posts_category_created_at (category_id, created_at),
  INDEX idx_posts_paper_status_created_at (paper_status, created_at),
  INDEX idx_posts_latest_paper_version_id (latest_paper_version_id),
  CONSTRAINT chk_posts_paper_status CHECK (paper_status IN ('draft', 'submitted', 'revision', 'accepted', 'published', 'rejected')),
  CONSTRAINT fk_posts_category_id FOREIGN KEY (category_id) REFERENCES post_categories(id),
  CONSTRAINT fk_posts_author_id FOREIGN KEY (author_id) REFERENCES users(id) ON DELETE CASCADE
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

CREATE TABLE IF NOT EXISTS post_likes (
  id BIGINT AUTO_INCREMENT PRIMARY KEY,
  user_id BIGINT NOT NULL,
  post_id BIGINT NOT NULL,
  created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  UNIQUE KEY uq_post_likes_user_post (user_id, post_id),
  INDEX idx_post_likes_post_id (post_id),
  CONSTRAINT fk_post_likes_user_id FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
  CONSTRAINT fk_post_likes_post_id FOREIGN KEY (post_id) REFERENCES posts(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS comments (
  id BIGINT AUTO_INCREMENT PRIMARY KEY,
  post_id BIGINT NOT NULL,
  author_id BIGINT NOT NULL,
  parent_comment_id BIGINT NULL,
  content TEXT NOT NULL,
  is_deleted BOOLEAN NOT NULL DEFAULT FALSE,
  deleted_at DATETIME(6) NULL,
  created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  updated_at DATETIME(6) NULL,
  INDEX idx_comments_post_id_created_at (post_id, created_at),
  INDEX idx_comments_post_parent_created (post_id, parent_comment_id, created_at),
  INDEX idx_comments_author_id (author_id),
  CONSTRAINT fk_comments_post_id FOREIGN KEY (post_id) REFERENCES posts(id) ON DELETE CASCADE,
  CONSTRAINT fk_comments_author_id FOREIGN KEY (author_id) REFERENCES users(id) ON DELETE CASCADE,
  CONSTRAINT fk_comments_parent_comment_id FOREIGN KEY (parent_comment_id) REFERENCES comments(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS tags (
  id BIGINT AUTO_INCREMENT PRIMARY KEY,
  name VARCHAR(191) NOT NULL UNIQUE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS post_tags (
  post_id BIGINT NOT NULL,
  tag_id BIGINT NOT NULL,
  PRIMARY KEY (post_id, tag_id),
  INDEX idx_post_tags_tag_id (tag_id),
  CONSTRAINT fk_post_tags_post_id FOREIGN KEY (post_id) REFERENCES posts(id) ON DELETE CASCADE,
  CONSTRAINT fk_post_tags_tag_id FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS citation_sources (
  id TINYINT UNSIGNED PRIMARY KEY,
  code VARCHAR(32) NOT NULL UNIQUE,
  display_name VARCHAR(128) NOT NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS post_citations (
  citing_post_id BIGINT NOT NULL,
  cited_post_id BIGINT NOT NULL,
  citation_source_id TINYINT UNSIGNED NOT NULL,
  created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  PRIMARY KEY (citing_post_id, cited_post_id, citation_source_id),
  CONSTRAINT chk_post_citations_no_self CHECK (citing_post_id <> cited_post_id),
  INDEX idx_post_citations_citation_source_id (citation_source_id),
  INDEX idx_post_citations_cited_post_id (cited_post_id),
  CONSTRAINT fk_post_citations_citing_post_id FOREIGN KEY (citing_post_id) REFERENCES posts(id) ON DELETE CASCADE,
  CONSTRAINT fk_post_citations_cited_post_id FOREIGN KEY (cited_post_id) REFERENCES posts(id) ON DELETE CASCADE,
  CONSTRAINT fk_post_citations_source_id FOREIGN KEY (citation_source_id) REFERENCES citation_sources(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS post_doi_metadata (
  id BIGINT AUTO_INCREMENT PRIMARY KEY,
  post_id BIGINT NOT NULL,
  doi VARCHAR(255) NOT NULL,
  title TEXT NULL,
  journal VARCHAR(512) NULL,
  publisher VARCHAR(512) NULL,
  published_at VARCHAR(32) NULL,
  source_url VARCHAR(2048) NULL,
  raw_json JSON NULL,
  created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  updated_at DATETIME(6) NULL,
  UNIQUE KEY uq_post_doi_metadata_post_doi (post_id, doi),
  INDEX idx_post_doi_metadata_post_created (post_id, created_at),
  CONSTRAINT fk_post_doi_metadata_post_id FOREIGN KEY (post_id) REFERENCES posts(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

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
  paper_version_id BIGINT NULL,
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
  INDEX idx_post_ai_reviews_version_created (paper_version_id, created_at),
  INDEX idx_post_ai_reviews_post_created (post_id, created_at),
  INDEX idx_post_ai_reviews_status_created (status_id, created_at),
  CONSTRAINT fk_post_ai_reviews_post_id FOREIGN KEY (post_id) REFERENCES posts(id) ON DELETE CASCADE,
  CONSTRAINT fk_post_ai_reviews_paper_version_id FOREIGN KEY (paper_version_id) REFERENCES paper_versions(id) ON DELETE SET NULL,
  CONSTRAINT fk_post_ai_reviews_status_id FOREIGN KEY (status_id) REFERENCES ai_review_statuses(id),
  CONSTRAINT fk_post_ai_reviews_trigger_id FOREIGN KEY (trigger_id) REFERENCES ai_review_triggers(id),
  CONSTRAINT fk_post_ai_reviews_decision_id FOREIGN KEY (decision_id) REFERENCES ai_review_decisions(id),
  CONSTRAINT chk_post_ai_reviews_overall_score CHECK (overall_score BETWEEN 1 AND 5 OR overall_score IS NULL),
  CONSTRAINT chk_post_ai_reviews_novelty_score CHECK (novelty_score BETWEEN 1 AND 5 OR novelty_score IS NULL),
  CONSTRAINT chk_post_ai_reviews_methodology_score CHECK (methodology_score BETWEEN 1 AND 5 OR methodology_score IS NULL),
  CONSTRAINT chk_post_ai_reviews_clarity_score CHECK (clarity_score BETWEEN 1 AND 5 OR clarity_score IS NULL),
  CONSTRAINT chk_post_ai_reviews_citation_integrity_score CHECK (citation_integrity_score BETWEEN 1 AND 5 OR citation_integrity_score IS NULL)
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

INSERT IGNORE INTO post_categories (code, display_name) VALUES
  ('paper', 'Paper'),
  ('essay', 'Essay'),
  ('note', 'Note'),
  ('report', 'Report'),
  ('other', 'Other');

INSERT IGNORE INTO citation_sources (id, code, display_name) VALUES
  (1, 'manual', 'Manual citation'),
  (2, 'auto', 'Automatic citation');

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

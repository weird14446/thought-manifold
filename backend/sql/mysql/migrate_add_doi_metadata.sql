USE thought_manifold;

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

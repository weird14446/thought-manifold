use std::{
    fs::File,
    io::{Cursor, Read},
    path::Path,
    time::Duration,
};

use anyhow::{Context, anyhow};
use chrono::Utc;
use quick_xml::{Reader, events::Event};
use reqwest::StatusCode as HttpStatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{FromRow, MySql, MySqlPool, QueryBuilder};
use tokio::task;
use zip::ZipArchive;

use crate::models::{
    AiReviewDecision, AiReviewEditorial, AiReviewListResponse, AiReviewMetricsSummary,
    AiReviewPeer, AiReviewResponse, AiReviewScores, AiReviewStatus, AiReviewSummary,
    MyPaperReviewItem, MyPaperReviewListResponse, PAPER_STATUS_ACCEPTED, PAPER_STATUS_REJECTED,
    PAPER_STATUS_REVISION,
};

pub const AI_REVIEW_PROMPT_VERSION: &str = "v1";
pub const AI_REVIEW_LANGUAGE: &str = "ko";
pub const DEFAULT_GEMINI_MODEL: &str = "gemini-2.0-flash";
pub const DEFAULT_GEMINI_TIMEOUT_SECS: u64 = 45;
pub const DEFAULT_GEMINI_MAX_RETRIES: u32 = 3;
pub const DEFAULT_GEMINI_RETRY_BASE_MS: u64 = 1500;
pub const DEFAULT_GEMINI_RETRY_MAX_MS: u64 = 12_000;
pub const DEFAULT_MAX_INPUT_CHARS: usize = 24_000;

const AI_REVIEW_STATUS_PENDING_ID: u8 = 1;
const AI_REVIEW_STATUS_COMPLETED_ID: u8 = 2;
const AI_REVIEW_STATUS_FAILED_ID: u8 = 3;

const AI_REVIEW_TRIGGER_AUTO_CREATE_ID: u8 = 1;
const AI_REVIEW_TRIGGER_AUTO_UPDATE_ID: u8 = 2;
const AI_REVIEW_TRIGGER_MANUAL_ID: u8 = 3;

const AI_REVIEW_DECISION_ACCEPT_ID: u8 = 1;
const AI_REVIEW_DECISION_MINOR_REVISION_ID: u8 = 2;
const AI_REVIEW_DECISION_MAJOR_REVISION_ID: u8 = 3;
const AI_REVIEW_DECISION_REJECT_ID: u8 = 4;

const REVIEW_SELECT_COLUMNS: &str = r#"
    SELECT
        r.id,
        r.post_id,
        r.paper_version_id,
        CAST(pv.version_number AS SIGNED) AS version_number,
        s.code AS status,
        t.code AS trigger_code,
        d.code AS decision,
        r.model,
        r.prompt_version,
        r.language_code,
        CAST(r.overall_score AS SIGNED) AS overall_score,
        CAST(r.novelty_score AS SIGNED) AS novelty_score,
        CAST(r.methodology_score AS SIGNED) AS methodology_score,
        CAST(r.clarity_score AS SIGNED) AS clarity_score,
        CAST(r.citation_integrity_score AS SIGNED) AS citation_integrity_score,
        r.editorial_summary,
        r.peer_summary,
        CAST(r.major_issues_json AS CHAR) AS major_issues_json,
        CAST(r.minor_issues_json AS CHAR) AS minor_issues_json,
        CAST(r.required_revisions_json AS CHAR) AS required_revisions_json,
        CAST(r.strengths_json AS CHAR) AS strengths_json,
        CAST(r.input_snapshot_json AS CHAR) AS input_snapshot_json,
        CAST(r.raw_response_json AS CHAR) AS raw_response_json,
        r.error_message,
        r.created_at,
        r.completed_at
"#;

const REVIEW_SELECT_FROM: &str = r#"
    FROM post_ai_reviews r
    JOIN ai_review_statuses s ON s.id = r.status_id
    JOIN ai_review_triggers t ON t.id = r.trigger_id
    LEFT JOIN ai_review_decisions d ON d.id = r.decision_id
    LEFT JOIN paper_versions pv ON pv.id = r.paper_version_id
"#;

#[derive(Debug, Clone, Copy)]
pub enum ReviewTrigger {
    AutoCreate,
    AutoUpdate,
    Manual,
}

impl ReviewTrigger {
    fn id(self) -> u8 {
        match self {
            Self::AutoCreate => AI_REVIEW_TRIGGER_AUTO_CREATE_ID,
            Self::AutoUpdate => AI_REVIEW_TRIGGER_AUTO_UPDATE_ID,
            Self::Manual => AI_REVIEW_TRIGGER_MANUAL_ID,
        }
    }
}

#[derive(Debug, FromRow)]
struct ReviewRow {
    id: i64,
    post_id: i64,
    paper_version_id: Option<i64>,
    version_number: Option<i32>,
    status: String,
    trigger_code: String,
    decision: Option<String>,
    model: Option<String>,
    prompt_version: Option<String>,
    language_code: Option<String>,
    overall_score: Option<i32>,
    novelty_score: Option<i32>,
    methodology_score: Option<i32>,
    clarity_score: Option<i32>,
    citation_integrity_score: Option<i32>,
    editorial_summary: Option<String>,
    peer_summary: Option<String>,
    major_issues_json: Option<String>,
    minor_issues_json: Option<String>,
    required_revisions_json: Option<String>,
    strengths_json: Option<String>,
    input_snapshot_json: Option<String>,
    raw_response_json: Option<String>,
    error_message: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, FromRow)]
struct ReviewPostSource {
    id: i64,
    title: String,
    summary: Option<String>,
    content: String,
    category_code: String,
    file_path: Option<String>,
    file_name: Option<String>,
}

#[derive(Debug, FromRow)]
struct ReviewCenterRow {
    post_id: i64,
    title: String,
    category: String,
    paper_status: String,
    current_revision: i32,
    is_published: bool,
    published_at: Option<chrono::DateTime<chrono::Utc>>,
    review_id: Option<i64>,
    review_paper_version_id: Option<i64>,
    review_version_number: Option<i32>,
    review_status: Option<String>,
    review_decision: Option<String>,
    review_trigger: Option<String>,
    overall_score: Option<i32>,
    error_message: Option<String>,
    review_created_at: Option<chrono::DateTime<chrono::Utc>>,
    review_completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Serialize)]
struct ReviewInputSnapshot {
    post_id: i64,
    title: String,
    summary: Option<String>,
    content_chars: usize,
    truncated: bool,
    max_input_chars: usize,
    attachments: Vec<AttachmentSnapshot>,
}

#[derive(Debug, Serialize)]
struct AttachmentSnapshot {
    file_name: Option<String>,
    file_path: Option<String>,
    extension: Option<String>,
    analyzed: bool,
    extracted_chars: usize,
    skip_reason: Option<String>,
}

#[derive(Debug)]
struct BuiltReviewInput {
    prompt_input: String,
    snapshot: Value,
}

#[derive(Debug, Deserialize)]
struct GeminiReviewOutput {
    decision: String,
    overall_score: i32,
    novelty_score: i32,
    methodology_score: i32,
    clarity_score: i32,
    citation_integrity_score: i32,
    editorial_summary: String,
    peer_summary: String,
    #[serde(default)]
    major_issues: Vec<String>,
    #[serde(default)]
    minor_issues: Vec<String>,
    #[serde(default)]
    required_revisions: Vec<String>,
    #[serde(default)]
    strengths: Vec<String>,
}

pub async fn schedule_review(
    pool: &MySqlPool,
    post_id: i64,
    paper_version_id: Option<i64>,
    trigger: ReviewTrigger,
) -> Result<i64, anyhow::Error> {
    let now = Utc::now();
    let model = std::env::var("GEMINI_MODEL").unwrap_or_else(|_| DEFAULT_GEMINI_MODEL.to_string());

    let result = sqlx::query(
        r#"
        INSERT INTO post_ai_reviews (
            post_id,
            paper_version_id,
            status_id,
            trigger_id,
            model,
            prompt_version,
            language_code,
            created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(post_id)
    .bind(paper_version_id)
    .bind(AI_REVIEW_STATUS_PENDING_ID)
    .bind(trigger.id())
    .bind(model)
    .bind(AI_REVIEW_PROMPT_VERSION)
    .bind(AI_REVIEW_LANGUAGE)
    .bind(now)
    .execute(pool)
    .await?;

    let review_id = result.last_insert_id() as i64;
    let pool_clone = pool.clone();
    tokio::spawn(async move {
        if let Err(error) = run_review(&pool_clone, review_id).await {
            tracing::error!(
                "AI review run failed for review_id={}: {}",
                review_id,
                error
            );
        }
    });

    Ok(review_id)
}

pub async fn run_review(pool: &MySqlPool, review_id: i64) -> Result<(), anyhow::Error> {
    let row: Option<(i64, Option<i64>)> =
        sqlx::query_as("SELECT post_id, paper_version_id FROM post_ai_reviews WHERE id = ?")
        .bind(review_id)
        .fetch_optional(pool)
        .await?;
    let Some((post_id, paper_version_id)) = row else {
        return Err(anyhow!("Review not found: {}", review_id));
    };

    let built_input = match build_review_input(pool, post_id, paper_version_id).await {
        Ok(input) => input,
        Err(error) => {
            mark_failed(pool, review_id, &error.to_string(), None, None).await?;
            return Ok(());
        }
    };

    match invoke_gemini_review(&built_input.prompt_input).await {
        Ok((parsed, raw_response)) => {
            if let Err(error) =
                mark_completed(pool, review_id, parsed, raw_response, built_input.snapshot).await
            {
                mark_failed(pool, review_id, &error.to_string(), None, None).await?;
            }
        }
        Err((error, raw_response)) => {
            mark_failed(
                pool,
                review_id,
                &error.to_string(),
                raw_response,
                Some(built_input.snapshot),
            )
            .await?;
        }
    }

    Ok(())
}

pub async fn fetch_latest_review(
    pool: &MySqlPool,
    post_id: i64,
) -> Result<Option<AiReviewResponse>, sqlx::Error> {
    let sql = format!(
        "{}{} WHERE r.post_id = ? ORDER BY r.created_at DESC LIMIT 1",
        REVIEW_SELECT_COLUMNS, REVIEW_SELECT_FROM
    );
    let row = sqlx::query_as::<_, ReviewRow>(&sql)
        .bind(post_id)
        .fetch_optional(pool)
        .await?;

    Ok(row.map(map_review_row))
}

pub async fn fetch_post_reviews(
    pool: &MySqlPool,
    post_id: i64,
    limit: i32,
    offset: i32,
) -> Result<AiReviewListResponse, sqlx::Error> {
    let per_page = limit.clamp(1, 100);
    let offset = offset.max(0);
    let page = (offset / per_page) + 1;

    let list_sql = format!(
        "{}{} WHERE r.post_id = ? ORDER BY r.created_at DESC LIMIT ? OFFSET ?",
        REVIEW_SELECT_COLUMNS, REVIEW_SELECT_FROM
    );
    let rows = sqlx::query_as::<_, ReviewRow>(&list_sql)
        .bind(post_id)
        .bind(i64::from(per_page))
        .bind(i64::from(offset))
        .fetch_all(pool)
        .await?;

    let (total,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM post_ai_reviews WHERE post_id = ?")
        .bind(post_id)
        .fetch_one(pool)
        .await?;

    Ok(AiReviewListResponse {
        reviews: rows.into_iter().map(map_review_row).collect(),
        total,
        page,
        per_page,
    })
}

pub async fn fetch_admin_reviews(
    pool: &MySqlPool,
    status: Option<&str>,
    page: i32,
    per_page: i32,
) -> Result<AiReviewListResponse, sqlx::Error> {
    let page = page.max(1);
    let per_page = per_page.clamp(1, 100);
    let offset = i64::from(page - 1) * i64::from(per_page);

    let mut list_qb =
        QueryBuilder::<MySql>::new(format!("{}{}", REVIEW_SELECT_COLUMNS, REVIEW_SELECT_FROM));
    let mut has_where = false;

    if let Some(status_code) = status {
        push_condition(&mut list_qb, &mut has_where);
        list_qb.push("s.code = ");
        list_qb.push_bind(status_code);
    }

    list_qb.push(" ORDER BY r.created_at DESC LIMIT ");
    list_qb.push_bind(i64::from(per_page));
    list_qb.push(" OFFSET ");
    list_qb.push_bind(offset);

    let rows = list_qb
        .build_query_as::<ReviewRow>()
        .fetch_all(pool)
        .await?;

    let mut count_qb = QueryBuilder::<MySql>::new(
        "SELECT COUNT(*) FROM post_ai_reviews r JOIN ai_review_statuses s ON s.id = r.status_id",
    );
    let mut count_has_where = false;
    if let Some(status_code) = status {
        push_condition(&mut count_qb, &mut count_has_where);
        count_qb.push("s.code = ");
        count_qb.push_bind(status_code);
    }
    let (total,): (i64,) = count_qb.build_query_as().fetch_one(pool).await?;

    Ok(AiReviewListResponse {
        reviews: rows.into_iter().map(map_review_row).collect(),
        total,
        page,
        per_page,
    })
}

pub async fn fetch_ai_review_metrics(
    pool: &MySqlPool,
) -> Result<AiReviewMetricsSummary, sqlx::Error> {
    let (total_reviews, pending_reviews, completed_reviews, failed_reviews): (i64, i64, i64, i64) =
        sqlx::query_as(
            r#"
            SELECT
                COUNT(*) AS total_reviews,
                CAST(SUM(CASE WHEN s.code = 'pending' THEN 1 ELSE 0 END) AS SIGNED) AS pending_reviews,
                CAST(SUM(CASE WHEN s.code = 'completed' THEN 1 ELSE 0 END) AS SIGNED) AS completed_reviews,
                CAST(SUM(CASE WHEN s.code = 'failed' THEN 1 ELSE 0 END) AS SIGNED) AS failed_reviews
            FROM post_ai_reviews r
            JOIN ai_review_statuses s ON s.id = r.status_id
            "#,
        )
        .fetch_one(pool)
        .await?;

    Ok(AiReviewMetricsSummary {
        total_reviews,
        pending_reviews,
        completed_reviews,
        failed_reviews,
    })
}

pub async fn fetch_user_review_center(
    pool: &MySqlPool,
    user_id: i64,
    page: i32,
    per_page: i32,
) -> Result<MyPaperReviewListResponse, sqlx::Error> {
    let page = page.max(1);
    let per_page = per_page.clamp(1, 100);
    let offset = i64::from(page - 1) * i64::from(per_page);

    let rows = sqlx::query_as::<_, ReviewCenterRow>(
        r#"
        SELECT
            p.id AS post_id,
            p.title AS title,
            c.code AS category,
            p.paper_status AS paper_status,
            CAST(p.current_revision AS SIGNED) AS current_revision,
            p.is_published AS is_published,
            p.published_at AS published_at,
            lr.id AS review_id,
            lr.paper_version_id AS review_paper_version_id,
            CAST(pv.version_number AS SIGNED) AS review_version_number,
            s.code AS review_status,
            d.code AS review_decision,
            t.code AS review_trigger,
            CAST(lr.overall_score AS SIGNED) AS overall_score,
            lr.error_message AS error_message,
            lr.created_at AS review_created_at,
            lr.completed_at AS review_completed_at
        FROM posts p
        JOIN post_categories c ON c.id = p.category_id
        LEFT JOIN post_ai_reviews lr ON lr.id = (
            SELECT r2.id
            FROM post_ai_reviews r2
            WHERE r2.post_id = p.id
            ORDER BY r2.created_at DESC, r2.id DESC
            LIMIT 1
        )
        LEFT JOIN paper_versions pv ON pv.id = lr.paper_version_id
        LEFT JOIN ai_review_statuses s ON s.id = lr.status_id
        LEFT JOIN ai_review_decisions d ON d.id = lr.decision_id
        LEFT JOIN ai_review_triggers t ON t.id = lr.trigger_id
        WHERE p.author_id = ? AND c.code = 'paper'
        ORDER BY p.updated_at DESC, p.created_at DESC
        LIMIT ? OFFSET ?
        "#,
    )
    .bind(user_id)
    .bind(i64::from(per_page))
    .bind(offset)
    .fetch_all(pool)
    .await?;

    let (total,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM posts p JOIN post_categories c ON c.id = p.category_id WHERE p.author_id = ? AND c.code = 'paper'",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    let items = rows
        .into_iter()
        .map(|row| {
            let latest_review = match (row.review_id, row.review_status, row.review_created_at) {
                (Some(review_id), Some(status_code), Some(created_at)) => Some(AiReviewSummary {
                    id: review_id,
                    paper_version_id: row.review_paper_version_id,
                    version_number: row.review_version_number,
                    status: map_status_code(&status_code),
                    decision: row.review_decision.as_deref().and_then(map_decision_code),
                    trigger: row.review_trigger.unwrap_or_else(|| "unknown".to_string()),
                    overall_score: row.overall_score,
                    error_message: row.error_message,
                    created_at,
                    completed_at: row.review_completed_at,
                }),
                _ => None,
            };

            MyPaperReviewItem {
                post_id: row.post_id,
                title: row.title,
                category: row.category,
                paper_status: row.paper_status,
                current_revision: row.current_revision,
                is_published: row.is_published,
                published_at: row.published_at,
                latest_review,
            }
        })
        .collect();

    Ok(MyPaperReviewListResponse {
        items,
        total,
        page,
        per_page,
    })
}

pub fn parse_status_filter(raw: &str) -> Option<&'static str> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "pending" => Some("pending"),
        "completed" => Some("completed"),
        "failed" => Some("failed"),
        _ => None,
    }
}

fn push_condition(query_builder: &mut QueryBuilder<MySql>, has_where: &mut bool) {
    if *has_where {
        query_builder.push(" AND ");
    } else {
        query_builder.push(" WHERE ");
        *has_where = true;
    }
}

fn map_review_row(row: ReviewRow) -> AiReviewResponse {
    AiReviewResponse {
        id: row.id,
        post_id: row.post_id,
        paper_version_id: row.paper_version_id,
        version_number: row.version_number,
        status: map_status_code(&row.status),
        trigger: row.trigger_code,
        decision: row.decision.as_deref().and_then(map_decision_code),
        model: row.model,
        prompt_version: row.prompt_version,
        language_code: row.language_code,
        scores: AiReviewScores {
            overall_score: row.overall_score,
            novelty_score: row.novelty_score,
            methodology_score: row.methodology_score,
            clarity_score: row.clarity_score,
            citation_integrity_score: row.citation_integrity_score,
        },
        editorial: AiReviewEditorial {
            summary: row.editorial_summary,
        },
        peer: AiReviewPeer {
            summary: row.peer_summary,
            major_issues: parse_string_list_json(row.major_issues_json),
            minor_issues: parse_string_list_json(row.minor_issues_json),
            required_revisions: parse_string_list_json(row.required_revisions_json),
            strengths: parse_string_list_json(row.strengths_json),
        },
        input_snapshot: parse_json_value(row.input_snapshot_json),
        raw_response: parse_json_value(row.raw_response_json),
        error_message: row.error_message,
        created_at: row.created_at,
        completed_at: row.completed_at,
    }
}

fn map_status_code(code: &str) -> AiReviewStatus {
    match code {
        "completed" => AiReviewStatus::Completed,
        "failed" => AiReviewStatus::Failed,
        _ => AiReviewStatus::Pending,
    }
}

fn map_decision_code(code: &str) -> Option<AiReviewDecision> {
    match code {
        "accept" => Some(AiReviewDecision::Accept),
        "minor_revision" => Some(AiReviewDecision::MinorRevision),
        "major_revision" => Some(AiReviewDecision::MajorRevision),
        "reject" => Some(AiReviewDecision::Reject),
        _ => None,
    }
}

fn parse_string_list_json(raw: Option<String>) -> Vec<String> {
    raw.and_then(|json_text| serde_json::from_str::<Vec<String>>(&json_text).ok())
        .unwrap_or_default()
}

fn parse_json_value(raw: Option<String>) -> Option<Value> {
    raw.and_then(|json_text| serde_json::from_str::<Value>(&json_text).ok())
}

async fn build_review_input(
    pool: &MySqlPool,
    post_id: i64,
    paper_version_id: Option<i64>,
) -> Result<BuiltReviewInput, anyhow::Error> {
    let source = if let Some(version_id) = paper_version_id {
        sqlx::query_as::<_, ReviewPostSource>(
            r#"
            SELECT
                p.id,
                v.title,
                v.summary,
                v.content,
                c.code AS category_code,
                v.file_path,
                v.file_name
            FROM posts p
            JOIN post_categories c ON c.id = p.category_id
            JOIN paper_versions v ON v.post_id = p.id
            WHERE p.id = ? AND v.id = ?
            "#,
        )
        .bind(post_id)
        .bind(version_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow!("Paper version not found for review: {}", version_id))?
    } else {
        sqlx::query_as::<_, ReviewPostSource>(
            r#"
            SELECT
                p.id,
                p.title,
                p.summary,
                p.content,
                c.code AS category_code,
                pf.file_path,
                pf.file_name
            FROM posts p
            JOIN post_categories c ON c.id = p.category_id
            LEFT JOIN post_files pf ON pf.post_id = p.id
            WHERE p.id = ?
            "#,
        )
        .bind(post_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow!("Post not found for review: {}", post_id))?
    };

    if source.category_code != "paper" {
        return Err(anyhow!(
            "AI review is only available for paper category posts"
        ));
    }

    let mut attachment_snapshots = Vec::new();
    let mut attachment_sections = Vec::new();

    if let Some(path) = source.file_path.as_deref() {
        let file_name = source.file_name.clone();
        let extension = file_name
            .as_deref()
            .and_then(|name| Path::new(name).extension().and_then(|ext| ext.to_str()))
            .map(|ext| ext.to_ascii_lowercase());

        let mut snapshot = AttachmentSnapshot {
            file_name: file_name.clone(),
            file_path: Some(path.to_string()),
            extension: extension.clone(),
            analyzed: false,
            extracted_chars: 0,
            skip_reason: None,
        };

        let extract_result = extract_attachment_text(path, extension.as_deref()).await;
        match extract_result {
            Ok(Some(text)) => {
                snapshot.analyzed = true;
                snapshot.extracted_chars = text.chars().count();
                attachment_sections.push(format!(
                    "[첨부파일: {}]\n{}",
                    file_name.unwrap_or_else(|| "첨부파일".to_string()),
                    text
                ));
            }
            Ok(None) => {
                snapshot.skip_reason = Some("지원하지 않는 첨부 확장자".to_string());
            }
            Err(error) => {
                snapshot.skip_reason = Some(format!("첨부 텍스트 추출 실패: {}", error));
            }
        }

        attachment_snapshots.push(snapshot);
    }

    let mut input_text = format!(
        "제목:\n{}\n\n요약:\n{}\n\n본문:\n{}\n",
        source.title,
        source
            .summary
            .clone()
            .unwrap_or_else(|| "(없음)".to_string()),
        source.content
    );

    if !attachment_sections.is_empty() {
        input_text.push_str("\n첨부 텍스트:\n");
        input_text.push_str(&attachment_sections.join("\n\n"));
        input_text.push('\n');
    }

    let max_chars = max_input_chars();
    let (truncated_input, truncated) = truncate_chars(&input_text, max_chars);

    let snapshot = serde_json::to_value(ReviewInputSnapshot {
        post_id: source.id,
        title: source.title,
        summary: source.summary,
        content_chars: source.content.chars().count(),
        truncated,
        max_input_chars: max_chars,
        attachments: attachment_snapshots,
    })?;

    Ok(BuiltReviewInput {
        prompt_input: build_prompt(&truncated_input),
        snapshot,
    })
}

async fn extract_attachment_text(
    file_path: &str,
    extension: Option<&str>,
) -> Result<Option<String>, anyhow::Error> {
    let Some(ext) = extension else {
        return Ok(None);
    };

    match ext {
        "txt" | "md" => {
            let text = tokio::fs::read_to_string(file_path)
                .await
                .with_context(|| format!("Failed to read text attachment: {}", file_path))?;
            Ok(Some(text))
        }
        "pdf" => {
            let path = file_path.to_string();
            let text = task::spawn_blocking(move || pdf_extract::extract_text(&path))
                .await
                .context("Join error while parsing PDF")?
                .context("Failed to parse PDF")?;
            Ok(Some(text))
        }
        "docx" => {
            let path = file_path.to_string();
            let text = task::spawn_blocking(move || extract_docx_text(&path))
                .await
                .context("Join error while parsing DOCX")??;
            Ok(Some(text))
        }
        _ => Ok(None),
    }
}

fn extract_docx_text(path: &str) -> Result<String, anyhow::Error> {
    let file = File::open(path).with_context(|| format!("Failed to open DOCX: {}", path))?;
    let mut archive = ZipArchive::new(file).context("Invalid DOCX zip structure")?;
    let mut document_xml = String::new();
    archive
        .by_name("word/document.xml")
        .context("Missing word/document.xml in DOCX")?
        .read_to_string(&mut document_xml)
        .context("Failed to read DOCX XML")?;

    let mut reader = Reader::from_reader(Cursor::new(document_xml.into_bytes()));
    reader.config_mut().trim_text(true);

    let mut text = String::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Text(event)) => {
                let decoded = event
                    .unescape()
                    .context("Failed to decode DOCX text node")?;
                let value = decoded.trim();
                if !value.is_empty() {
                    if !text.is_empty() {
                        text.push(' ');
                    }
                    text.push_str(value);
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(anyhow!("Failed to parse DOCX XML: {}", error)),
        }
        buf.clear();
    }

    Ok(text)
}

fn build_prompt(input: &str) -> String {
    format!(
        r#"
너는 학술지 심사 시스템의 AI 심사자다. 반드시 JSON 객체만 출력하고, 마크다운/설명 문장을 추가하지 마라.
응답은 한국어로 작성한다.

필수 JSON 스키마:
{{
  "decision": "accept|minor_revision|major_revision|reject",
  "overall_score": 1~5 정수,
  "novelty_score": 1~5 정수,
  "methodology_score": 1~5 정수,
  "clarity_score": 1~5 정수,
  "citation_integrity_score": 1~5 정수,
  "editorial_summary": "편집자 1차 심사 대체 요약",
  "peer_summary": "동료심사 대체 종합 코멘트",
  "major_issues": ["주요 문제점"],
  "minor_issues": ["경미한 문제점"],
  "required_revisions": ["필수 수정사항"],
  "strengths": ["강점"]
}}

평가 기준:
- novelty_score: 연구의 참신성
- methodology_score: 방법론 타당성/재현 가능성
- clarity_score: 서술 명확성/구성
- citation_integrity_score: 인용 적절성과 출처 정합성

검토 대상 원고:
{}
"#,
        input
    )
}

async fn invoke_gemini_review(
    prompt: &str,
) -> Result<(GeminiReviewOutput, Value), (anyhow::Error, Option<Value>)> {
    let api_key = std::env::var("GEMINI_API_KEY")
        .map_err(|_| (anyhow!("GEMINI_API_KEY is not configured"), None))?;
    let model = std::env::var("GEMINI_MODEL").unwrap_or_else(|_| DEFAULT_GEMINI_MODEL.to_string());
    let timeout_secs = std::env::var("GEMINI_TIMEOUT_SECS")
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
        .unwrap_or(DEFAULT_GEMINI_TIMEOUT_SECS);
    let max_retries = gemini_max_retries();
    let total_attempts = max_retries + 1;
    let retry_base_ms = gemini_retry_base_ms();
    let retry_max_ms = gemini_retry_max_ms().max(retry_base_ms);

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        model, api_key
    );

    let request_body = json!({
        "contents": [
            {
                "role": "user",
                "parts": [{ "text": prompt }]
            }
        ],
        "generationConfig": {
            "temperature": 0.2,
            "responseMimeType": "application/json"
        }
    });

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()
        .map_err(|error| {
            (
                anyhow!("Failed to build Gemini HTTP client: {}", error),
                None,
            )
        })?;

    for attempt in 1..=total_attempts {
        let can_retry = attempt < total_attempts;
        let response = client.post(&url).json(&request_body).send().await;
        let response = match response {
            Ok(resp) => resp,
            Err(error) => {
                if can_retry {
                    let delay = retry_delay_for_attempt(attempt, retry_base_ms, retry_max_ms);
                    tracing::warn!(
                        attempt,
                        total_attempts,
                        delay_ms = delay.as_millis(),
                        "Gemini request failed (network/transport): {}. Retrying...",
                        error
                    );
                    tokio::time::sleep(delay).await;
                    continue;
                }
                return Err((
                    anyhow!(
                        "Failed to call Gemini API after {} attempt(s): {}",
                        total_attempts,
                        error
                    ),
                    None,
                ));
            }
        };

        let status = response.status();
        let body = response.text().await.map_err(|error| {
            (
                anyhow!("Failed to read Gemini response body: {}", error),
                None,
            )
        })?;

        let raw_response: Value =
            serde_json::from_str(&body).unwrap_or_else(|_| json!({ "raw_body": body }));

        if status != HttpStatusCode::OK {
            if can_retry && is_retryable_gemini_status(status) {
                let delay = retry_delay_for_attempt(attempt, retry_base_ms, retry_max_ms);
                tracing::warn!(
                    attempt,
                    total_attempts,
                    status = %status,
                    delay_ms = delay.as_millis(),
                    "Gemini transient API error. Retrying..."
                );
                tokio::time::sleep(delay).await;
                continue;
            }
            return Err((
                anyhow!(
                    "Gemini API error {} after {} attempt(s): {}",
                    status,
                    attempt,
                    body
                ),
                Some(raw_response),
            ));
        }

        let candidate_text = raw_response
            .get("candidates")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("content"))
            .and_then(|content| content.get("parts"))
            .and_then(|parts| parts.as_array())
            .and_then(|parts| parts.first())
            .and_then(|part| part.get("text"))
            .and_then(|text| text.as_str())
            .ok_or_else(|| {
                (
                    anyhow!("Gemini response does not contain candidate text"),
                    Some(raw_response.clone()),
                )
            })?;

        let cleaned = strip_code_fence(candidate_text).trim().to_string();
        let parsed: GeminiReviewOutput = serde_json::from_str(&cleaned).map_err(|error| {
            (
                anyhow!("Failed to parse Gemini structured JSON: {}", error),
                Some(raw_response.clone()),
            )
        })?;

        validate_review_output(&parsed).map_err(|error| (error, Some(raw_response.clone())))?;
        return Ok((parsed, raw_response));
    }

    Err((
        anyhow!(
            "Gemini API request did not succeed after {} attempt(s)",
            total_attempts
        ),
        None,
    ))
}

fn is_retryable_gemini_status(status: HttpStatusCode) -> bool {
    matches!(
        status,
        HttpStatusCode::TOO_MANY_REQUESTS
            | HttpStatusCode::INTERNAL_SERVER_ERROR
            | HttpStatusCode::BAD_GATEWAY
            | HttpStatusCode::SERVICE_UNAVAILABLE
            | HttpStatusCode::GATEWAY_TIMEOUT
    )
}

fn retry_delay_for_attempt(attempt: u32, base_ms: u64, max_ms: u64) -> Duration {
    let exponent = attempt.saturating_sub(1).min(16);
    let multiplier = 1u64 << exponent;
    let delay_ms = base_ms.saturating_mul(multiplier).min(max_ms);
    Duration::from_millis(delay_ms)
}

fn gemini_max_retries() -> u32 {
    std::env::var("GEMINI_MAX_RETRIES")
        .ok()
        .and_then(|raw| raw.parse::<u32>().ok())
        .map(|value| value.min(10))
        .unwrap_or(DEFAULT_GEMINI_MAX_RETRIES)
}

fn gemini_retry_base_ms() -> u64 {
    std::env::var("GEMINI_RETRY_BASE_MS")
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_GEMINI_RETRY_BASE_MS)
}

fn gemini_retry_max_ms() -> u64 {
    std::env::var("GEMINI_RETRY_MAX_MS")
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_GEMINI_RETRY_MAX_MS)
}

fn strip_code_fence(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(stripped) = trimmed
        .strip_prefix("```json")
        .and_then(|s| s.strip_suffix("```"))
    {
        return stripped.trim().to_string();
    }
    if let Some(stripped) = trimmed
        .strip_prefix("```")
        .and_then(|s| s.strip_suffix("```"))
    {
        return stripped.trim().to_string();
    }
    trimmed.to_string()
}

fn validate_review_output(output: &GeminiReviewOutput) -> Result<(), anyhow::Error> {
    let _ = map_decision_to_id(&output.decision)
        .ok_or_else(|| anyhow!("Invalid decision value: {}", output.decision))?;

    for (name, score) in [
        ("overall_score", output.overall_score),
        ("novelty_score", output.novelty_score),
        ("methodology_score", output.methodology_score),
        ("clarity_score", output.clarity_score),
        ("citation_integrity_score", output.citation_integrity_score),
    ] {
        if !(1..=5).contains(&score) {
            return Err(anyhow!("{} must be between 1 and 5", name));
        }
    }

    if output.editorial_summary.trim().is_empty() {
        return Err(anyhow!("editorial_summary must not be empty"));
    }
    if output.peer_summary.trim().is_empty() {
        return Err(anyhow!("peer_summary must not be empty"));
    }

    Ok(())
}

fn map_decision_to_id(code: &str) -> Option<u8> {
    match code.to_ascii_lowercase().as_str() {
        "accept" => Some(AI_REVIEW_DECISION_ACCEPT_ID),
        "minor_revision" => Some(AI_REVIEW_DECISION_MINOR_REVISION_ID),
        "major_revision" => Some(AI_REVIEW_DECISION_MAJOR_REVISION_ID),
        "reject" => Some(AI_REVIEW_DECISION_REJECT_ID),
        _ => None,
    }
}

async fn mark_completed(
    pool: &MySqlPool,
    review_id: i64,
    output: GeminiReviewOutput,
    raw_response: Value,
    input_snapshot: Value,
) -> Result<(), anyhow::Error> {
    let decision_id = map_decision_to_id(&output.decision)
        .ok_or_else(|| anyhow!("Invalid decision during completion: {}", output.decision))?;
    let now = Utc::now();

    sqlx::query(
        r#"
        UPDATE post_ai_reviews
        SET
            status_id = ?,
            decision_id = ?,
            overall_score = ?,
            novelty_score = ?,
            methodology_score = ?,
            clarity_score = ?,
            citation_integrity_score = ?,
            editorial_summary = ?,
            peer_summary = ?,
            major_issues_json = ?,
            minor_issues_json = ?,
            required_revisions_json = ?,
            strengths_json = ?,
            input_snapshot_json = ?,
            raw_response_json = ?,
            error_message = NULL,
            completed_at = ?
        WHERE id = ?
        "#,
    )
    .bind(AI_REVIEW_STATUS_COMPLETED_ID)
    .bind(decision_id)
    .bind(output.overall_score)
    .bind(output.novelty_score)
    .bind(output.methodology_score)
    .bind(output.clarity_score)
    .bind(output.citation_integrity_score)
    .bind(output.editorial_summary.trim())
    .bind(output.peer_summary.trim())
    .bind(serde_json::to_string(&output.major_issues)?)
    .bind(serde_json::to_string(&output.minor_issues)?)
    .bind(serde_json::to_string(&output.required_revisions)?)
    .bind(serde_json::to_string(&output.strengths)?)
    .bind(serde_json::to_string(&input_snapshot)?)
    .bind(serde_json::to_string(&raw_response)?)
    .bind(now)
    .bind(review_id)
    .execute(pool)
    .await?;

    let next_paper_status = match decision_id {
        AI_REVIEW_DECISION_ACCEPT_ID => PAPER_STATUS_ACCEPTED,
        AI_REVIEW_DECISION_MINOR_REVISION_ID | AI_REVIEW_DECISION_MAJOR_REVISION_ID => {
            PAPER_STATUS_REVISION
        }
        AI_REVIEW_DECISION_REJECT_ID => PAPER_STATUS_REJECTED,
        _ => PAPER_STATUS_REVISION,
    };

    sqlx::query(
        r#"
        UPDATE posts
        SET
            paper_status = ?,
            is_published = FALSE,
            published_at = NULL,
            updated_at = ?
        WHERE id = (SELECT post_id FROM post_ai_reviews WHERE id = ?)
          AND (
              (
                  (SELECT paper_version_id FROM post_ai_reviews WHERE id = ?) IS NOT NULL
                  AND (SELECT paper_version_id FROM post_ai_reviews WHERE id = ?) = latest_paper_version_id
              )
              OR (
                  (SELECT paper_version_id FROM post_ai_reviews WHERE id = ?) IS NULL
                  AND latest_paper_version_id IS NULL
              )
          )
        "#,
    )
    .bind(next_paper_status)
    .bind(now)
    .bind(review_id)
    .bind(review_id)
    .bind(review_id)
    .bind(review_id)
    .execute(pool)
    .await?;

    Ok(())
}

async fn mark_failed(
    pool: &MySqlPool,
    review_id: i64,
    error_message: &str,
    raw_response: Option<Value>,
    input_snapshot: Option<Value>,
) -> Result<(), anyhow::Error> {
    let now = Utc::now();
    let raw_json = raw_response
        .as_ref()
        .map(|value| serde_json::to_string(value))
        .transpose()?;
    let input_json = input_snapshot
        .as_ref()
        .map(|value| serde_json::to_string(value))
        .transpose()?;

    sqlx::query(
        r#"
        UPDATE post_ai_reviews
        SET
            status_id = ?,
            error_message = ?,
            raw_response_json = COALESCE(?, raw_response_json),
            input_snapshot_json = COALESCE(?, input_snapshot_json),
            completed_at = ?
        WHERE id = ?
        "#,
    )
    .bind(AI_REVIEW_STATUS_FAILED_ID)
    .bind(error_message)
    .bind(raw_json)
    .bind(input_json)
    .bind(now)
    .bind(review_id)
    .execute(pool)
    .await?;

    Ok(())
}

fn max_input_chars() -> usize {
    std::env::var("AI_REVIEW_MAX_INPUT_CHARS")
        .ok()
        .and_then(|raw| raw.parse::<usize>().ok())
        .filter(|value| *value > 2000)
        .unwrap_or(DEFAULT_MAX_INPUT_CHARS)
}

fn truncate_chars(input: &str, max_chars: usize) -> (String, bool) {
    let char_count = input.chars().count();
    if char_count <= max_chars {
        return (input.to_string(), false);
    }

    let truncated: String = input.chars().take(max_chars).collect();
    (truncated, true)
}

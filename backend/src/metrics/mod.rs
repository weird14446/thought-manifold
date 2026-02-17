use std::collections::HashMap;

use crate::models::{AuthorMetrics, JournalMetrics};
use sqlx::{MySql, MySqlPool, QueryBuilder};

pub const METRIC_VERSION: &str = "v1";
pub const JOURNAL_IMPACT_FORMULA: &str = "jif_2y";
pub const AUTHOR_G_INDEX_FORMULA: &str = "g_index";

pub async fn compute_citation_count(pool: &MySqlPool, post_id: i64) -> Result<i64, sqlx::Error> {
    let (count,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)
        FROM (
            SELECT DISTINCT citing_post_id, cited_post_id
            FROM post_citations
        ) c
        WHERE c.cited_post_id = ?
        "#,
    )
    .bind(post_id)
    .fetch_one(pool)
    .await?;

    Ok(count)
}

pub async fn compute_citation_counts_for_posts(
    pool: &MySqlPool,
    post_ids: &[i64],
) -> Result<HashMap<i64, i64>, sqlx::Error> {
    if post_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let mut query_builder = QueryBuilder::<MySql>::new(
        r#"
        SELECT c.cited_post_id, COUNT(*) as citation_count
        FROM (
            SELECT DISTINCT citing_post_id, cited_post_id
            FROM post_citations
        ) c
        WHERE c.cited_post_id IN (
        "#,
    );
    {
        let mut separated = query_builder.separated(", ");
        for post_id in post_ids {
            separated.push_bind(post_id);
        }
    }
    query_builder.push(") GROUP BY c.cited_post_id");

    let rows: Vec<(i64, i64)> = query_builder.build_query_as().fetch_all(pool).await?;
    Ok(rows.into_iter().collect())
}

#[allow(dead_code)]
pub async fn compute_g_index(pool: &MySqlPool, user_id: i64) -> Result<i64, sqlx::Error> {
    let citation_counts = fetch_author_paper_citation_counts(pool, user_id).await?;
    Ok(calculate_g_index(&citation_counts))
}

pub async fn compute_author_metrics(
    pool: &MySqlPool,
    user_id: i64,
) -> Result<AuthorMetrics, sqlx::Error> {
    let citation_counts = fetch_author_paper_citation_counts(pool, user_id).await?;
    let total_citations = citation_counts.iter().sum::<i64>();
    let g_index = calculate_g_index(&citation_counts);

    Ok(AuthorMetrics {
        user_id,
        g_index,
        total_citations,
        paper_count: citation_counts.len() as i64,
        formula: AUTHOR_G_INDEX_FORMULA.to_string(),
        metric_version: METRIC_VERSION.to_string(),
    })
}

pub async fn compute_impact_factor(
    pool: &MySqlPool,
    year: i32,
) -> Result<JournalMetrics, sqlx::Error> {
    let target_year = year;
    let prev_year = year - 1;
    let prev_prev_year = year - 2;

    let (numerator_citations,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)
        FROM (
            SELECT DISTINCT citing_post_id, cited_post_id
            FROM post_citations
        ) pc
        JOIN posts citing ON citing.id = pc.citing_post_id
        JOIN post_categories citing_category ON citing_category.id = citing.category_id
        JOIN posts cited ON cited.id = pc.cited_post_id
        JOIN post_categories cited_category ON cited_category.id = cited.category_id
        WHERE citing_category.code = 'paper'
          AND cited_category.code = 'paper'
          AND YEAR(citing.created_at) = ?
          AND YEAR(cited.created_at) IN (?, ?)
        "#,
    )
    .bind(target_year)
    .bind(prev_year.clone())
    .bind(prev_prev_year.clone())
    .fetch_one(pool)
    .await?;

    let (denominator_papers,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)
        FROM posts p
        JOIN post_categories c ON c.id = p.category_id
        WHERE c.code = 'paper'
          AND YEAR(p.created_at) IN (?, ?)
        "#,
    )
    .bind(prev_year)
    .bind(prev_prev_year)
    .fetch_one(pool)
    .await?;

    let impact_factor = if denominator_papers > 0 {
        Some(numerator_citations as f64 / denominator_papers as f64)
    } else {
        None
    };

    Ok(JournalMetrics {
        year,
        impact_factor,
        numerator_citations,
        denominator_papers,
        formula: JOURNAL_IMPACT_FORMULA.to_string(),
        metric_version: METRIC_VERSION.to_string(),
    })
}

async fn fetch_author_paper_citation_counts(
    pool: &MySqlPool,
    user_id: i64,
) -> Result<Vec<i64>, sqlx::Error> {
    let rows: Vec<(i64,)> = sqlx::query_as(
        r#"
        SELECT COALESCE(c.citation_count, 0) as citation_count
        FROM posts p
        JOIN post_categories pc ON pc.id = p.category_id
        LEFT JOIN (
            SELECT cited_post_id, COUNT(*) as citation_count
            FROM (
                SELECT DISTINCT citing_post_id, cited_post_id
                FROM post_citations
            ) distinct_citations
            GROUP BY cited_post_id
        ) c ON c.cited_post_id = p.id
        WHERE p.author_id = ? AND pc.code = 'paper'
        ORDER BY citation_count DESC, p.id ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|(count,)| count).collect())
}

fn calculate_g_index(citation_counts: &[i64]) -> i64 {
    let mut running_sum = 0_i64;
    let mut g_index = 0_i64;

    for (idx, count) in citation_counts.iter().enumerate() {
        running_sum += *count;
        let g = (idx as i64) + 1;
        if running_sum >= g * g {
            g_index = g;
        }
    }

    g_index
}

//! Nexus Mods catalog client using GraphQL v2 API

use anyhow::{bail, Context, Result};
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

const GRAPHQL_ENDPOINT: &str = "https://api.nexusmods.com/v2/graphql";
const MAX_RETRIES: u32 = 5;
const BASE_RETRY_DELAY_MS: u64 = 2000;
const MAX_RETRY_DELAY_MS: u64 = 60000;

/// GraphQL client for catalog population
#[derive(Clone)]
pub struct NexusRestClient {
    client: Arc<reqwest::Client>,
}

impl NexusRestClient {
    /// Create a new catalog client
    pub fn new(api_key: &str) -> Result<Self> {
        let api_key = api_key.trim();

        let mut headers = HeaderMap::new();
        headers.insert(
            "apikey",
            HeaderValue::from_str(api_key).context("Invalid API key")?,
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .user_agent("ModSanity/0.1.0")
            .timeout(Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            client: Arc::new(client),
        })
    }

    /// Fetch a page of mods for a game using GraphQL
    pub async fn fetch_mods_page(
        &self,
        game_domain: &str,
        offset: i32,
        count: i32,
    ) -> Result<ModsPageResult> {
        let query = r#"
            query GetMods($gameDomain: String!, $offset: Int!, $count: Int!) {
                mods(
                    filter: {gameDomainName: [{value: $gameDomain}]}
                    offset: $offset
                    count: $count
                ) {
                    nodes {
                        modId
                        name
                        summary
                        description
                        author
                        updatedAt
                    }
                    totalCount
                }
            }
        "#;

        #[derive(Serialize)]
        struct Variables {
            #[serde(rename = "gameDomain")]
            game_domain: String,
            offset: i32,
            count: i32,
        }

        #[derive(Serialize)]
        struct GraphQLRequest {
            query: String,
            variables: Variables,
        }

        let request = GraphQLRequest {
            query: query.to_string(),
            variables: Variables {
                game_domain: game_domain.to_string(),
                offset,
                count,
            },
        };

        let mut attempt = 0;

        loop {
            attempt += 1;

            let response = self
                .client
                .post(GRAPHQL_ENDPOINT)
                .json(&request)
                .send()
                .await
                .context("Failed to send request")?;

            let status = response.status();

            // Handle rate limiting (429)
            if status == 429 {
                if attempt >= MAX_RETRIES {
                    bail!("Rate limited after {} retries", MAX_RETRIES);
                }

                // Check for Retry-After header
                let retry_after = if let Some(retry_header) = response.headers().get("retry-after")
                {
                    retry_header
                        .to_str()
                        .ok()
                        .and_then(|s| s.parse::<u64>().ok())
                        .map(|secs| secs * 1000)
                        .unwrap_or(BASE_RETRY_DELAY_MS)
                } else {
                    // Exponential backoff with jitter
                    let base_delay = BASE_RETRY_DELAY_MS * (1 << (attempt - 1));
                    let jitter = (rand::random::<f64>() * 0.3 + 0.85) as u64; // 85-115% jitter
                    (base_delay * jitter / 100).min(MAX_RETRY_DELAY_MS)
                };

                tracing::warn!(
                    "Rate limited (attempt {}/{}), retrying in {}ms",
                    attempt,
                    MAX_RETRIES,
                    retry_after
                );

                sleep(Duration::from_millis(retry_after)).await;
                continue;
            }

            // Handle server errors (5xx) with retry
            if status.is_server_error() {
                if attempt >= MAX_RETRIES {
                    bail!("Server error after {} retries: {}", MAX_RETRIES, status);
                }

                let delay = BASE_RETRY_DELAY_MS * (1 << (attempt - 1));
                tracing::warn!(
                    "Server error {} (attempt {}/{}), retrying in {}ms",
                    status,
                    attempt,
                    MAX_RETRIES,
                    delay.min(MAX_RETRY_DELAY_MS)
                );

                sleep(Duration::from_millis(delay.min(MAX_RETRY_DELAY_MS))).await;
                continue;
            }

            // Handle client errors (4xx) - don't retry
            if status.is_client_error() {
                let error_text = response.text().await.unwrap_or_default();
                bail!("Client error {}: {}", status, error_text);
            }

            // Success - parse response
            if status.is_success() {
                #[derive(Deserialize)]
                struct GraphQLResponse {
                    data: Option<GraphQLData>,
                    errors: Option<Vec<GraphQLError>>,
                }

                #[derive(Deserialize)]
                struct GraphQLData {
                    mods: ModsPage,
                }

                #[derive(Deserialize)]
                struct GraphQLError {
                    message: String,
                }

                let response_text = response
                    .text()
                    .await
                    .context("Failed to read response body")?;
                let graphql_response: GraphQLResponse = serde_json::from_str(&response_text)
                    .with_context(|| {
                        format!("Failed to parse GraphQL response: {}", response_text)
                    })?;

                if let Some(errors) = graphql_response.errors {
                    let error_messages: Vec<String> =
                        errors.iter().map(|e| e.message.clone()).collect();
                    bail!("GraphQL errors: {}", error_messages.join(", "));
                }

                let data = graphql_response
                    .data
                    .context("GraphQL response contained no data")?;
                let mods_page = data.mods;

                tracing::debug!(
                    "Fetched page offset={} with {} mods out of {} total (attempt {})",
                    offset,
                    mods_page.nodes.len(),
                    mods_page.total_count,
                    attempt
                );

                return Ok(ModsPageResult {
                    mods: mods_page.nodes,
                    total_count: mods_page.total_count,
                });
            }

            // Unexpected status
            bail!("Unexpected response status: {}", status);
        }
    }
}

/// Result from fetching a page of mods
#[derive(Debug, Clone)]
pub struct ModsPageResult {
    pub mods: Vec<ModInfo>,
    pub total_count: i64,
}

/// Mod info from GraphQL API response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModInfo {
    #[serde(rename = "modId")]
    pub mod_id: i64,
    pub name: String,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<String>,
}

#[derive(Deserialize)]
struct ModsPage {
    nodes: Vec<ModInfo>,
    #[serde(rename = "totalCount")]
    total_count: i64,
}

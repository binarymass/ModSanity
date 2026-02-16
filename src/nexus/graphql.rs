//! Nexus Mods GraphQL v2 API client

use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const GRAPHQL_ENDPOINT: &str = "https://api.nexusmods.com/v2/graphql";
const REST_API_BASE: &str = "https://api.nexusmods.com/v1";

/// Nexus Mods GraphQL client
#[derive(Clone)]
pub struct NexusClient {
    client: Arc<reqwest::Client>,
    api_key: String,
}

impl NexusClient {
    /// Create a new Nexus Mods GraphQL client
    pub fn new(api_key: String) -> Result<Self> {
        let api_key = api_key.trim().to_string();

        let mut headers = HeaderMap::new();

        // REST API v1 uses the "apikey" header for authentication
        headers.insert(
            "apikey",
            HeaderValue::from_str(&api_key).context("Invalid API key")?,
        );

        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .user_agent("ModSanity/0.1.0")
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            client: Arc::new(client),
            api_key,
        })
    }

    /// Execute a GraphQL query (with authentication)
    async fn query<V, R>(&self, query: &str, variables: V) -> Result<R>
    where
        V: Serialize,
        R: for<'de> Deserialize<'de>,
    {
        self.query_internal(query, variables, true).await
    }

    /// Execute a GraphQL query without authentication (for public queries)
    async fn query_public<V, R>(&self, query: &str, variables: V) -> Result<R>
    where
        V: Serialize,
        R: for<'de> Deserialize<'de>,
    {
        self.query_internal(query, variables, false).await
    }

    /// Internal query method
    async fn query_internal<V, R>(&self, query: &str, variables: V, use_auth: bool) -> Result<R>
    where
        V: Serialize,
        R: for<'de> Deserialize<'de>,
    {
        #[derive(Serialize)]
        struct GraphQLRequest<V> {
            query: String,
            variables: V,
        }

        #[derive(Deserialize)]
        struct GraphQLResponse<R> {
            data: Option<R>,
            errors: Option<Vec<GraphQLError>>,
        }

        #[derive(Deserialize)]
        struct GraphQLError {
            message: String,
        }

        let request = GraphQLRequest {
            query: query.to_string(),
            variables,
        };

        // Create a client without default headers for public queries
        let client = if use_auth {
            &self.client
        } else {
            // Create a temporary client without auth headers for this request
            &reqwest::Client::builder()
                .user_agent("ModSanity/0.1.0")
                .build()
                .context("Failed to create HTTP client")?
        };

        let response = client
            .post(GRAPHQL_ENDPOINT)
            .header(CONTENT_TYPE, "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send GraphQL request")?;

        let status = response.status();
        let response_text = response
            .text()
            .await
            .context("Failed to read response body")?;

        if !status.is_success() {
            anyhow::bail!(
                "GraphQL request failed with status {}: {}",
                status,
                response_text
            );
        }

        let graphql_response: GraphQLResponse<R> = serde_json::from_str(&response_text)
            .with_context(|| format!("Failed to parse GraphQL response: {}", response_text))?;

        if let Some(errors) = graphql_response.errors {
            let error_messages: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
            anyhow::bail!("GraphQL errors: {}", error_messages.join(", "));
        }

        graphql_response
            .data
            .context("GraphQL response contained no data")
    }

    /// Check for updates to installed mods
    /// Returns a list of (mod_id, current_version, latest_version, has_update)
    pub async fn check_mod_updates(
        &self,
        game_domain: &str,
        mod_ids: &[i64],
    ) -> Result<Vec<ModUpdateInfo>> {
        if mod_ids.is_empty() {
            return Ok(Vec::new());
        }

        // Build UIDs from game domain and mod IDs
        let uids: Vec<String> = mod_ids
            .iter()
            .map(|id| format!("{}:{}", game_domain, id))
            .collect();

        let query = r#"
            query ModsByUid($uids: [ID!]!) {
                modsByUid(uids: $uids) {
                    nodes {
                        modId
                        name
                        version
                        updatedAt
                        viewerDownloaded
                        viewerUpdateAvailable
                    }
                }
            }
        "#;

        #[derive(Serialize)]
        struct Variables {
            uids: Vec<String>,
        }

        #[derive(Deserialize)]
        struct Response {
            #[serde(rename = "modsByUid")]
            mods_by_uid: ModsPage,
        }

        #[derive(Deserialize)]
        struct ModsPage {
            nodes: Vec<ModNode>,
        }

        #[derive(Deserialize)]
        struct ModNode {
            #[serde(rename = "modId")]
            mod_id: i64,
            name: String,
            version: String,
            #[serde(rename = "updatedAt")]
            updated_at: String,
            #[serde(rename = "viewerDownloaded")]
            viewer_downloaded: Option<String>,
            #[serde(rename = "viewerUpdateAvailable")]
            viewer_update_available: Option<bool>,
        }

        let variables = Variables { uids };
        let response: Response = self.query(query, variables).await?;

        let updates = response
            .mods_by_uid
            .nodes
            .into_iter()
            .map(|node| ModUpdateInfo {
                mod_id: node.mod_id,
                name: node.name,
                current_version: node.viewer_downloaded.unwrap_or_default(),
                latest_version: node.version,
                updated_at: node.updated_at,
                has_update: node.viewer_update_available.unwrap_or(false),
            })
            .collect();

        Ok(updates)
    }

    /// Get mod requirements (dependencies)
    pub async fn get_mod_requirements(
        &self,
        game_domain: &str,
        mod_id: i64,
    ) -> Result<Vec<ModRequirement>> {
        // Map game domain to game ID
        let game_id = match game_domain {
            "skyrimspecialedition" => 1704,
            "skyrim" => 110,
            "fallout4" => 1151,
            "fallout3" => 120,
            "falloutnv" => 130,
            "oblivion" => 101,
            "morrowind" => 100,
            _ => anyhow::bail!("Unknown game domain: {}", game_domain),
        };

        // Use legacyMods query with composite ID
        let query = r#"
            query GetModRequirements($ids: [CompositeIdInput!]!) {
                legacyMods(ids: $ids) {
                    nodes {
                        modId
                        name
                        modRequirements {
                            nexusRequirements {
                                nodes {
                                    modId
                                    modName
                                    notes
                                    url
                                    externalRequirement
                                }
                            }
                            dlcRequirements {
                                gameExpansion {
                                    name
                                }
                                notes
                            }
                        }
                    }
                }
            }
        "#;

        #[derive(Serialize)]
        struct CompositeIdInput {
            #[serde(rename = "gameId")]
            game_id: i32,
            #[serde(rename = "modId")]
            mod_id: i64,
        }

        #[derive(Serialize)]
        struct Variables {
            ids: Vec<CompositeIdInput>,
        }

        #[derive(Deserialize)]
        struct Response {
            #[serde(rename = "legacyMods")]
            legacy_mods: ModsPage,
        }

        #[derive(Deserialize)]
        struct ModsPage {
            nodes: Vec<ModNode>,
        }

        #[derive(Deserialize)]
        struct ModNode {
            #[serde(rename = "modRequirements")]
            mod_requirements: ModRequirements,
        }

        #[derive(Deserialize)]
        struct ModRequirements {
            #[serde(rename = "nexusRequirements")]
            nexus_requirements: RequirementsPage,
            #[serde(rename = "dlcRequirements")]
            dlc_requirements: Vec<DlcRequirement>,
        }

        #[derive(Deserialize)]
        struct RequirementsPage {
            nodes: Vec<RequirementNode>,
        }

        #[derive(Deserialize)]
        struct RequirementNode {
            #[serde(rename = "modId")]
            mod_id: String,
            #[serde(rename = "modName")]
            mod_name: String,
            notes: Option<String>,
            #[serde(rename = "url")]
            _url: String,
            #[serde(rename = "externalRequirement")]
            external_requirement: bool,
        }

        #[derive(Deserialize)]
        struct DlcRequirement {
            #[serde(rename = "gameExpansion")]
            game_expansion: GameExpansion,
            notes: Option<String>,
        }

        #[derive(Deserialize)]
        struct GameExpansion {
            name: String,
        }

        let variables = Variables {
            ids: vec![CompositeIdInput {
                game_id: game_id,
                mod_id: mod_id,
            }],
        };

        tracing::debug!(
            "Fetching requirements for mod {} (game: {})",
            mod_id,
            game_domain
        );
        let response: Response = self.query(query, variables).await.map_err(|e| {
            tracing::error!("Failed to fetch requirements for mod {}: {}", mod_id, e);
            e
        })?;

        let mut requirements = Vec::new();

        if let Some(node) = response.legacy_mods.nodes.first() {
            // Add nexus mod requirements
            for req in &node.mod_requirements.nexus_requirements.nodes {
                if !req.external_requirement {
                    // Parse mod_id from string
                    if let Ok(id) = req.mod_id.parse::<i64>() {
                        requirements.push(ModRequirement {
                            mod_id: id,
                            name: req.mod_name.clone(),
                            notes: req.notes.clone(),
                            is_dlc: false,
                        });
                    }
                }
            }

            // Add DLC requirements
            for dlc in &node.mod_requirements.dlc_requirements {
                requirements.push(ModRequirement {
                    mod_id: 0, // DLCs don't have mod IDs
                    name: dlc.game_expansion.name.clone(),
                    notes: dlc.notes.clone(),
                    is_dlc: true,
                });
            }
        }

        Ok(requirements)
    }

    /// Search for mods with filters and sorting
    /// Note: This query doesn't require authentication per GraphQL v2 docs
    pub async fn search_mods(&self, search: ModSearchParams) -> Result<ModSearchPage> {
        let query = r#"
            query SearchMods($filter: ModsFilter, $sort: [ModsSort!], $offset: Int, $count: Int) {
                mods(filter: $filter, sort: $sort, offset: $offset, count: $count) {
                    nodes {
                        modId
                        name
                        summary
                        version
                        author
                        category
                        downloads
                        endorsements
                        pictureUrl
                        thumbnailUrl
                        updatedAt
                        createdAt
                    }
                    totalCount
                }
            }
        "#;

        #[derive(Serialize)]
        struct Variables {
            filter: ModsFilter,
            sort: Vec<ModsSort>,
            offset: i32,
            count: i32,
        }

        #[derive(Serialize)]
        struct ModsFilter {
            #[serde(rename = "gameDomainName")]
            game_domain_name: Option<Vec<FilterValue>>,
            #[serde(rename = "nameStemmed")]
            name_stemmed: Option<Vec<FilterValue>>,
            author: Option<Vec<FilterValue>>,
            #[serde(rename = "categoryName")]
            category_name: Option<Vec<FilterValue>>,
        }

        #[derive(Serialize)]
        struct FilterValue {
            value: String,
            op: String,
        }

        #[derive(Serialize)]
        struct ModsSort {
            #[serde(skip_serializing_if = "Option::is_none")]
            downloads: Option<SortValue>,
            #[serde(skip_serializing_if = "Option::is_none")]
            endorsements: Option<SortValue>,
            #[serde(skip_serializing_if = "Option::is_none")]
            #[serde(rename = "updatedAt")]
            updated_at: Option<SortValue>,
            #[serde(skip_serializing_if = "Option::is_none")]
            relevance: Option<SortValue>,
        }

        #[derive(Serialize)]
        struct SortValue {
            direction: String,
        }

        #[derive(Deserialize)]
        struct Response {
            mods: ModsPage,
        }

        #[derive(Deserialize)]
        struct ModsPage {
            nodes: Vec<ModNode>,
            #[serde(rename = "totalCount")]
            total_count: i64,
        }

        #[derive(Deserialize)]
        struct ModNode {
            #[serde(rename = "modId")]
            mod_id: i64,
            name: String,
            summary: String,
            version: String,
            author: String,
            category: String,
            downloads: i64,
            endorsements: i64,
            #[serde(rename = "pictureUrl")]
            picture_url: Option<String>,
            #[serde(rename = "thumbnailUrl")]
            thumbnail_url: Option<String>,
            #[serde(rename = "updatedAt")]
            updated_at: String,
            #[serde(rename = "createdAt")]
            created_at: String,
        }

        // Build filter
        let mut filter = ModsFilter {
            game_domain_name: None,
            name_stemmed: None,
            author: None,
            category_name: None,
        };

        if let Some(game_domain) = &search.game_domain {
            filter.game_domain_name = Some(vec![FilterValue {
                value: game_domain.clone(),
                op: "EQUALS".to_string(),
            }]);
        }

        if let Some(query_text) = &search.query {
            // Use nameStemmed field which supports wildcards
            filter.name_stemmed = Some(vec![FilterValue {
                value: query_text.clone(),
                op: "WILDCARD".to_string(),
            }]);
        }

        if let Some(author) = &search.author {
            filter.author = Some(vec![FilterValue {
                value: author.clone(),
                op: "EQUALS".to_string(),
            }]);
        }

        if let Some(category) = &search.category {
            filter.category_name = Some(vec![FilterValue {
                value: category.clone(),
                op: "EQUALS".to_string(),
            }]);
        }

        // Build sort
        let mut sort = Vec::new();

        match search.sort_by {
            SortBy::Downloads => sort.push(ModsSort {
                downloads: Some(SortValue {
                    direction: "DESC".to_string(),
                }),
                endorsements: None,
                updated_at: None,
                relevance: None,
            }),
            SortBy::Endorsements => sort.push(ModsSort {
                downloads: None,
                endorsements: Some(SortValue {
                    direction: "DESC".to_string(),
                }),
                updated_at: None,
                relevance: None,
            }),
            SortBy::Updated => sort.push(ModsSort {
                downloads: None,
                endorsements: None,
                updated_at: Some(SortValue {
                    direction: "DESC".to_string(),
                }),
                relevance: None,
            }),
            SortBy::Relevance => sort.push(ModsSort {
                downloads: None,
                endorsements: None,
                updated_at: None,
                relevance: Some(SortValue {
                    direction: "DESC".to_string(),
                }),
            }),
        }

        let variables = Variables {
            filter,
            sort,
            offset: search.offset.unwrap_or(0),
            count: search.limit.unwrap_or(20),
        };

        // Use public query since mods search doesn't require authentication
        let response: Response = self.query_public(query, variables).await?;

        let results = response
            .mods
            .nodes
            .into_iter()
            .map(|node| ModSearchResult {
                mod_id: node.mod_id,
                name: node.name,
                summary: node.summary,
                version: node.version,
                author: node.author,
                category: node.category,
                downloads: node.downloads,
                endorsements: node.endorsements,
                picture_url: node.picture_url,
                thumbnail_url: node.thumbnail_url,
                updated_at: node.updated_at,
                created_at: node.created_at,
            })
            .collect();

        Ok(ModSearchPage {
            results,
            total_count: response.mods.total_count,
        })
    }

    /// Track a mod (add to tracking list)
    pub async fn track_mod(&self, game_domain: &str, mod_id: i64) -> Result<()> {
        let uid = format!("{}:{}", game_domain, mod_id);

        let query = r#"
            mutation TrackMod($modUid: ID!) {
                trackMod(modUid: $modUid) {
                    success
                }
            }
        "#;

        #[derive(Serialize)]
        struct Variables {
            #[serde(rename = "modUid")]
            mod_uid: String,
        }

        #[derive(Deserialize)]
        struct Response {
            #[serde(rename = "trackMod")]
            track_mod: TrackResult,
        }

        #[derive(Deserialize)]
        struct TrackResult {
            success: bool,
        }

        let variables = Variables { mod_uid: uid };
        let response: Response = self.query(query, variables).await?;

        if !response.track_mod.success {
            anyhow::bail!("Failed to track mod");
        }

        Ok(())
    }

    /// Untrack a mod (remove from tracking list)
    pub async fn untrack_mod(&self, game_domain: &str, mod_id: i64) -> Result<()> {
        let uid = format!("{}:{}", game_domain, mod_id);

        let query = r#"
            mutation UntrackMod($modUid: ID!) {
                untrackMod(modUid: $modUid) {
                    success
                }
            }
        "#;

        #[derive(Serialize)]
        struct Variables {
            #[serde(rename = "modUid")]
            mod_uid: String,
        }

        #[derive(Deserialize)]
        struct Response {
            #[serde(rename = "untrackMod")]
            untrack_mod: TrackResult,
        }

        #[derive(Deserialize)]
        struct TrackResult {
            success: bool,
        }

        let variables = Variables { mod_uid: uid };
        let response: Response = self.query(query, variables).await?;

        if !response.untrack_mod.success {
            anyhow::bail!("Failed to untrack mod");
        }

        Ok(())
    }

    /// Endorse a mod
    pub async fn endorse_mod(&self, game_domain: &str, mod_id: i64) -> Result<()> {
        let uid = format!("{}:{}", game_domain, mod_id);

        let query = r#"
            mutation Endorse($modUid: ID!) {
                endorse(modUid: $modUid) {
                    success
                }
            }
        "#;

        #[derive(Serialize)]
        struct Variables {
            #[serde(rename = "modUid")]
            mod_uid: String,
        }

        #[derive(Deserialize)]
        struct Response {
            endorse: EndorseResult,
        }

        #[derive(Deserialize)]
        struct EndorseResult {
            success: bool,
        }

        let variables = Variables { mod_uid: uid };
        let response: Response = self.query(query, variables).await?;

        if !response.endorse.success {
            anyhow::bail!("Failed to endorse mod");
        }

        Ok(())
    }

    /// Get list of files for a mod (GraphQL v2 - no auth required)
    pub async fn get_mod_files(&self, game_id: i64, mod_id: i64) -> Result<Vec<ModFile>> {
        let query = r#"
            query ModFiles($modId: ID!, $gameId: ID!) {
                modFiles(modId: $modId, gameId: $gameId) {
                    fileId
                    name
                    version
                    size
                    sizeInBytes
                    uri
                    category
                    description
                }
            }
        "#;

        #[derive(Serialize)]
        struct Variables {
            #[serde(rename = "modId")]
            mod_id: String,
            #[serde(rename = "gameId")]
            game_id: String,
        }

        #[derive(Deserialize)]
        struct Response {
            #[serde(rename = "modFiles")]
            mod_files: Vec<FileNode>,
        }

        #[derive(Deserialize)]
        struct FileNode {
            #[serde(rename = "fileId")]
            file_id: i64,
            name: String,
            version: String,
            size: i64,
            #[serde(rename = "sizeInBytes")]
            size_in_bytes: Option<String>,
            uri: String,
            category: String,
            description: Option<String>,
        }

        let variables = Variables {
            mod_id: mod_id.to_string(),
            game_id: game_id.to_string(),
        };

        let response: Response = self.query_public(query, variables).await?;

        let files = response
            .mod_files
            .into_iter()
            .map(|f| {
                let size_bytes = f
                    .size_in_bytes
                    .and_then(|s| s.parse::<i64>().ok())
                    .unwrap_or(f.size * 1024);
                ModFile {
                    file_id: f.file_id,
                    name: f.name,
                    version: f.version,
                    category: f.category,
                    size_bytes,
                    file_name: f.uri,
                    description: f.description,
                }
            })
            .collect();

        Ok(files)
    }

    /// Get download link for a mod file (REST API v1 - requires valid API key)
    /// Premium users get direct download links.
    /// Non-premium users will get a 403 and need to use the website.
    pub async fn get_download_link(
        &self,
        game_domain: &str,
        mod_id: i64,
        file_id: i64,
    ) -> Result<Vec<DownloadLink>> {
        let url = format!(
            "{}/games/{}/mods/{}/files/{}/download_link.json",
            REST_API_BASE, game_domain, mod_id, file_id
        );

        #[derive(Deserialize)]
        struct LinkInfo {
            #[serde(rename = "URI")]
            uri: String,
            name: String,
        }

        // Explicitly set apikey header on this request to ensure it's sent
        let response = reqwest::Client::new()
            .get(&url)
            .header("apikey", &self.api_key)
            .header("accept", "application/json")
            .header("user-agent", "ModSanity/0.1.0")
            .send()
            .await
            .context("Failed to get download link")?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            if status.as_u16() == 403 {
                anyhow::bail!("Download requires Nexus Mods Premium membership. Visit the mod page to download manually.");
            }
            anyhow::bail!("Failed to get download link ({}): {}", status, text);
        }

        let links: Vec<LinkInfo> = response
            .json()
            .await
            .context("Failed to parse download link response")?;

        let download_links = links
            .into_iter()
            .map(|l| DownloadLink {
                url: l.uri,
                name: l.name,
            })
            .collect();

        Ok(download_links)
    }

    /// Resolve a Nexus mod display name using REST API.
    pub async fn get_mod_name_by_id(
        &self,
        game_domain: &str,
        mod_id: i64,
    ) -> Result<Option<String>> {
        #[derive(Deserialize)]
        struct ModDetails {
            name: Option<String>,
        }

        let url = format!(
            "{}/games/{}/mods/{}.json",
            REST_API_BASE, game_domain, mod_id
        );
        let response = reqwest::Client::new()
            .get(&url)
            .header("apikey", &self.api_key)
            .header("accept", "application/json")
            .header("user-agent", "ModSanity/0.1.0")
            .send()
            .await
            .with_context(|| {
                format!("Failed to fetch mod details for {}:{}", game_domain, mod_id)
            })?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !response.status().is_success() {
            anyhow::bail!(
                "Failed to fetch mod details (status: {})",
                response.status()
            );
        }

        let details: ModDetails = response
            .json()
            .await
            .context("Failed to parse mod details response")?;

        Ok(details
            .name
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty()))
    }

    /// Download a file from a URL to a local path, reporting progress via callback
    pub async fn download_file(
        url: &str,
        dest: &std::path::Path,
        progress_cb: impl Fn(u64, u64) + Send + 'static,
    ) -> Result<()> {
        let response = reqwest::Client::new()
            .get(url)
            .send()
            .await
            .context("Failed to start download")?;

        if !response.status().is_success() {
            anyhow::bail!("Download failed with status: {}", response.status());
        }

        let total_size = response.content_length().unwrap_or(0);

        let mut file = tokio::fs::File::create(dest)
            .await
            .context("Failed to create download file")?;

        let mut downloaded: u64 = 0;
        let mut stream = response.bytes_stream();

        use futures::StreamExt;
        use tokio::io::AsyncWriteExt;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Error reading download stream")?;
            file.write_all(&chunk)
                .await
                .context("Error writing to file")?;
            downloaded += chunk.len() as u64;
            progress_cb(downloaded, total_size);
        }

        file.flush().await?;
        Ok(())
    }
}

/// Information about a mod update
#[derive(Debug, Clone)]
pub struct ModUpdateInfo {
    pub mod_id: i64,
    pub name: String,
    pub current_version: String,
    pub latest_version: String,
    pub updated_at: String,
    pub has_update: bool,
}

/// Mod requirement/dependency
#[derive(Debug, Clone)]
pub struct ModRequirement {
    pub mod_id: i64,
    pub name: String,
    pub notes: Option<String>,
    pub is_dlc: bool,
}

/// Search parameters for mod search
#[derive(Debug, Clone, Default)]
pub struct ModSearchParams {
    pub game_domain: Option<String>,
    pub query: Option<String>,
    pub author: Option<String>,
    pub category: Option<String>,
    pub sort_by: SortBy,
    pub offset: Option<i32>,
    pub limit: Option<i32>,
}

/// Sort order for search results
#[derive(Debug, Clone, Copy, Default)]
pub enum SortBy {
    #[default]
    Relevance,
    Downloads,
    Endorsements,
    Updated,
}

/// Mod search result
#[derive(Debug, Clone)]
pub struct ModSearchResult {
    pub mod_id: i64,
    pub name: String,
    pub summary: String,
    pub version: String,
    pub author: String,
    pub category: String,
    pub downloads: i64,
    pub endorsements: i64,
    pub picture_url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub updated_at: String,
    pub created_at: String,
}

/// Mod search page results
#[derive(Debug, Clone)]
pub struct ModSearchPage {
    pub results: Vec<ModSearchResult>,
    pub total_count: i64,
}

/// Information about a downloadable mod file
#[derive(Debug, Clone)]
pub struct ModFile {
    pub file_id: i64,
    pub name: String,
    pub version: String,
    pub category: String,
    pub size_bytes: i64,
    pub file_name: String,
    pub description: Option<String>,
}

/// Download link information
#[derive(Debug, Clone)]
pub struct DownloadLink {
    pub url: String,
    pub name: String,
}

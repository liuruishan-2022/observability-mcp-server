use super::SearcherError;
use base64::Engine;
use chrono::{DateTime, FixedOffset, NaiveDate, TimeZone};
use pulldown_cmark::{Options, Parser, html};
use reqwest::Method;
use reqwest::{
    Client, RequestBuilder,
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue, USER_AGENT},
    multipart::{Form, Part},
};
use serde_json::{Map, Value, json};
use std::{
    cmp::Reverse,
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};
use tokio::fs;

pub enum AtlassianAuth {
    Basic { username: String, token: String },
    Bearer { token: String },
}

pub fn normalize_base_url(url: &str) -> String {
    url.trim().trim_end_matches('/').to_string()
}

pub fn is_cloud_url(url: &str) -> bool {
    let normalized = normalize_base_url(url).to_ascii_lowercase();
    normalized.contains(".atlassian.net") || normalized.contains("api.atlassian.com")
}

pub fn build_http_client(auth: AtlassianAuth, ssl_verify: bool) -> Result<Client, SearcherError> {
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static("observability-mcp-server/0.5.0"),
    );

    let auth_value = match auth {
        AtlassianAuth::Basic { username, token } => {
            let encoded =
                base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", username, token));
            format!("Basic {}", encoded)
        }
        AtlassianAuth::Bearer { token } => format!("Bearer {}", token),
    };

    let header_value = HeaderValue::from_str(&auth_value)
        .map_err(|e| SearcherError::Other(format!("invalid auth header value: {}", e)))?;
    headers.insert(AUTHORIZATION, header_value);

    Client::builder()
        .danger_accept_invalid_certs(!ssl_verify)
        .default_headers(headers)
        .build()
        .map_err(SearcherError::RequestError)
}

pub async fn send_json(request: RequestBuilder) -> Result<Value, SearcherError> {
    let response = request.send().await?;
    handle_json_response(response).await
}

pub async fn send_empty(request: RequestBuilder) -> Result<(), SearcherError> {
    let response = request.send().await?;
    let status = response.status();

    if status.is_success() {
        return Ok(());
    }

    let error_text = response
        .text()
        .await
        .unwrap_or_else(|_| "Unknown error".to_string());
    Err(SearcherError::ApiError(format!(
        "Atlassian API returned {}: {}",
        status, error_text
    )))
}

pub async fn send_bytes(request: RequestBuilder) -> Result<Vec<u8>, SearcherError> {
    let response = request.send().await?;
    let status = response.status();

    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(SearcherError::ApiError(format!(
            "Atlassian API returned {}: {}",
            status, error_text
        )));
    }

    response
        .bytes()
        .await
        .map(|bytes| bytes.to_vec())
        .map_err(SearcherError::RequestError)
}

async fn handle_json_response(response: reqwest::Response) -> Result<Value, SearcherError> {
    let status = response.status();

    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(SearcherError::ApiError(format!(
            "Atlassian API returned {}: {}",
            status, error_text
        )));
    }

    if status == reqwest::StatusCode::NO_CONTENT {
        return Ok(Value::Null);
    }

    response.json().await.map_err(SearcherError::RequestError)
}

pub fn ensure_object(
    value: Option<Value>,
    field_name: &str,
) -> Result<Map<String, Value>, SearcherError> {
    match value {
        Some(Value::Object(map)) => Ok(map),
        Some(Value::Null) | None => Ok(Map::new()),
        Some(_) => Err(SearcherError::ApiError(format!(
            "{} must be a JSON object",
            field_name
        ))),
    }
}

pub fn merge_objects(target: &mut Map<String, Value>, extra: Map<String, Value>) {
    for (key, value) in extra {
        target.insert(key, value);
    }
}

pub fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub fn markdown_to_html(markdown: &str) -> String {
    if markdown.trim().is_empty() {
        return "<p></p>".to_string();
    }

    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_HEADING_ATTRIBUTES);

    let parser = Parser::new_ext(markdown, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

pub fn html_to_text_lossy(html: &str) -> String {
    let mut output = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut tag_buf = String::new();
    let mut last_was_space = false;
    let mut chars = html.chars().peekable();

    while let Some(ch) = chars.next() {
        if in_tag {
            if ch == '>' {
                in_tag = false;
                let tag = tag_buf.trim().to_ascii_lowercase();
                if matches!(
                    tag.as_str(),
                    "br" | "br/"
                        | "/p"
                        | "/div"
                        | "/li"
                        | "/tr"
                        | "/h1"
                        | "/h2"
                        | "/h3"
                        | "/h4"
                        | "/h5"
                        | "/h6"
                ) {
                    if !output.ends_with('\n') {
                        output.push('\n');
                    }
                } else if matches!(tag.as_str(), "li") {
                    if !output.ends_with('\n') {
                        output.push('\n');
                    }
                    output.push_str("- ");
                }
                tag_buf.clear();
            } else {
                tag_buf.push(ch);
            }
            continue;
        }

        if ch == '<' {
            in_tag = true;
            continue;
        }

        if ch == '&' {
            let mut entity = String::new();
            while let Some(next) = chars.next() {
                entity.push(next);
                if next == ';' || entity.len() > 10 {
                    break;
                }
            }
            let decoded = decode_html_entity(&entity);
            if decoded == " " {
                if !last_was_space {
                    output.push(' ');
                    last_was_space = true;
                }
            } else {
                output.push_str(decoded);
                last_was_space = false;
            }
            continue;
        }

        if ch.is_whitespace() {
            if ch == '\n' {
                if !output.ends_with('\n') {
                    output.push('\n');
                }
                last_was_space = false;
            } else if !last_was_space {
                output.push(' ');
                last_was_space = true;
            }
            continue;
        }

        output.push(ch);
        last_was_space = false;
    }

    let lines = output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    lines.join("\n")
}

fn decode_html_entity(entity: &str) -> &'static str {
    match entity {
        "amp;" => "&",
        "lt;" => "<",
        "gt;" => ">",
        "quot;" => "\"",
        "#39;" | "apos;" => "'",
        "nbsp;" => " ",
        _ => "",
    }
}

const DEFAULT_READ_FIELDS: &[&str] = &[
    "summary",
    "status",
    "assignee",
    "reporter",
    "priority",
    "issuetype",
    "description",
    "created",
    "updated",
    "comment",
];

pub struct JiraClient {
    client: reqwest::Client,
    base_url: String,
    api_version: &'static str,
    is_cloud: bool,
    projects_filter: Option<String>,
}

impl JiraClient {
    pub fn new(
        base_url: String,
        username: Option<String>,
        api_token: Option<String>,
        personal_token: Option<String>,
        ssl_verify: bool,
        projects_filter: Option<String>,
    ) -> Result<Self, SearcherError> {
        let base_url = normalize_base_url(&base_url);
        let is_cloud = is_cloud_url(&base_url);
        let auth = match personal_token {
            Some(token) if !token.trim().is_empty() => AtlassianAuth::Bearer { token },
            _ => {
                let username = username.ok_or_else(|| {
                    SearcherError::ApiError(
                        "Jira requires either JIRA_PERSONAL_TOKEN or JIRA_USERNAME/JIRA_API_TOKEN"
                            .to_string(),
                    )
                })?;
                let token = api_token.ok_or_else(|| {
                    SearcherError::ApiError(
                        "Jira requires either JIRA_PERSONAL_TOKEN or JIRA_USERNAME/JIRA_API_TOKEN"
                            .to_string(),
                    )
                })?;
                AtlassianAuth::Basic { username, token }
            }
        };

        Ok(Self {
            client: build_http_client(auth, ssl_verify)?,
            base_url,
            api_version: if is_cloud { "3" } else { "2" },
            is_cloud,
            projects_filter,
        })
    }

    pub async fn get_issue(
        &self,
        issue_key: &str,
        fields: Option<&[String]>,
        expand: Option<&str>,
        comment_limit: Option<usize>,
        properties: Option<&[String]>,
        update_history: Option<bool>,
    ) -> Result<Value, SearcherError> {
        self.ensure_issue_project_allowed(issue_key)?;

        let mut query = vec![(
            "fields".to_string(),
            self.fields_csv(fields, DEFAULT_READ_FIELDS),
        )];
        if let Some(expand) = expand.filter(|value| !value.trim().is_empty()) {
            query.push(("expand".to_string(), expand.to_string()));
        }
        if let Some(properties) = properties.filter(|value| !value.is_empty()) {
            query.push(("properties".to_string(), properties.join(",")));
        }
        if let Some(update_history) = update_history {
            query.push(("updateHistory".to_string(), update_history.to_string()));
        }

        let mut issue = self
            .request_json(
                Method::GET,
                &format!("issue/{}", issue_key),
                Some(query),
                None,
            )
            .await?;

        if let Some(limit) = comment_limit {
            if let Some(comments) = issue
                .get_mut("fields")
                .and_then(Value::as_object_mut)
                .and_then(|fields| fields.get_mut("comment"))
                .and_then(Value::as_object_mut)
                .and_then(|comment| comment.get_mut("comments"))
                .and_then(Value::as_array_mut)
            {
                comments.truncate(limit);
            }
        }

        Ok(issue)
    }

    pub async fn search_issues(
        &self,
        jql: &str,
        fields: Option<&[String]>,
        start_at: Option<usize>,
        limit: Option<usize>,
        projects_filter: Option<&str>,
        expand: Option<&str>,
        page_token: Option<&str>,
    ) -> Result<Value, SearcherError> {
        let jql = self.apply_projects_filter(jql, projects_filter);
        let limit = limit.unwrap_or(20).clamp(1, 100);

        let mut body = json!({
            "jql": jql,
            "fields": self.fields_vec(fields, DEFAULT_READ_FIELDS),
            "maxResults": limit,
            "startAt": start_at.unwrap_or(0),
        });

        if let Some(expand) = expand.filter(|value| !value.trim().is_empty()) {
            body["expand"] = Value::String(expand.to_string());
        }
        if let Some(page_token) = page_token.filter(|value| !value.trim().is_empty()) {
            body["nextPageToken"] = Value::String(page_token.to_string());
        }

        let path = if self.is_cloud { "search" } else { "search" };
        self.request_json(Method::POST, path, None, Some(body))
            .await
    }

    pub async fn search_fields(
        &self,
        keyword: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Value, SearcherError> {
        let fields = self.request_json(Method::GET, "field", None, None).await?;
        let mut fields = fields.as_array().cloned().unwrap_or_default();
        let keyword = keyword.unwrap_or("").trim().to_ascii_lowercase();
        let limit = limit.unwrap_or(10).clamp(1, 100);

        if !keyword.is_empty() {
            fields.sort_by_key(|field| Reverse(field_match_score(field, &keyword)));
            fields.retain(|field| field_match_score(field, &keyword) > 0);
        }

        fields.truncate(limit);
        Ok(Value::Array(fields))
    }

    pub async fn get_field_options(
        &self,
        field_id: &str,
        context_id: Option<&str>,
        project_key: Option<&str>,
        issue_type: Option<&str>,
        contains: Option<&str>,
        return_limit: Option<usize>,
        values_only: Option<bool>,
    ) -> Result<Value, SearcherError> {
        let contains = contains.map(|value| value.to_ascii_lowercase());
        let return_limit = return_limit.unwrap_or(50).clamp(1, 500);
        let values_only = values_only.unwrap_or(false);

        let options = if self.is_cloud {
            let context_id = match context_id {
                Some(id) => id.to_string(),
                None => self.resolve_field_context_id(field_id).await?,
            };
            let response = self
                .request_json(
                    Method::GET,
                    &format!("field/{}/context/{}/option", field_id, context_id),
                    None,
                    None,
                )
                .await?;
            response
                .get("values")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        } else {
            let project_key = project_key.ok_or_else(|| {
                SearcherError::ApiError(
                    "project_key is required for Jira Server/DC field option lookup".to_string(),
                )
            })?;
            let issue_type = issue_type.ok_or_else(|| {
                SearcherError::ApiError(
                    "issue_type is required for Jira Server/DC field option lookup".to_string(),
                )
            })?;
            self.resolve_server_field_options(field_id, project_key, issue_type)
                .await?
        };

        let mut filtered = options
            .into_iter()
            .filter(|option| match &contains {
                Some(keyword) => option_matches_keyword(option, keyword),
                None => true,
            })
            .collect::<Vec<_>>();
        filtered.truncate(return_limit);

        if values_only {
            Ok(Value::Array(
                filtered
                    .into_iter()
                    .filter_map(|option| extract_option_value(&option).map(Value::String))
                    .collect(),
            ))
        } else {
            Ok(Value::Array(filtered))
        }
    }

    pub async fn get_project_issues(
        &self,
        project_key: &str,
        fields: Option<&[String]>,
        start_at: Option<usize>,
        limit: Option<usize>,
        expand: Option<&str>,
    ) -> Result<Value, SearcherError> {
        let jql = format!(
            "project = {} ORDER BY updated DESC",
            quote_jql_literal(project_key)
        );
        self.search_issues(&jql, fields, start_at, limit, None, expand, None)
            .await
    }

    pub async fn get_transitions(&self, issue_key: &str) -> Result<Value, SearcherError> {
        self.ensure_issue_project_allowed(issue_key)?;
        let response = self
            .request_json(
                Method::GET,
                &format!("issue/{}/transitions", issue_key),
                None,
                None,
            )
            .await?;
        Ok(response.get("transitions").cloned().unwrap_or(response))
    }

    pub async fn get_worklog(&self, issue_key: &str) -> Result<Value, SearcherError> {
        self.ensure_issue_project_allowed(issue_key)?;
        self.request_json(
            Method::GET,
            &format!("issue/{}/worklog", issue_key),
            None,
            None,
        )
        .await
    }

    pub async fn get_project_versions(&self, project_key: &str) -> Result<Value, SearcherError> {
        self.request_json(
            Method::GET,
            &format!("project/{}/versions", project_key),
            None,
            None,
        )
        .await
    }

    pub async fn get_project_components(&self, project_key: &str) -> Result<Value, SearcherError> {
        self.request_json(
            Method::GET,
            &format!("project/{}/components", project_key),
            None,
            None,
        )
        .await
    }

    pub async fn get_all_projects(&self, include_archived: bool) -> Result<Value, SearcherError> {
        let query = vec![
            ("maxResults".to_string(), "1000".to_string()),
            ("includeArchived".to_string(), include_archived.to_string()),
        ];

        match self
            .request_json(Method::GET, "project/search", Some(query), None)
            .await
        {
            Ok(value) => Ok(value.get("values").cloned().unwrap_or(value)),
            Err(_) => self.request_json(Method::GET, "project", None, None).await,
        }
    }

    pub async fn get_user_profile(&self, user_identifier: &str) -> Result<Value, SearcherError> {
        if self.is_cloud {
            let users = self.search_users(user_identifier).await?;
            let user = pick_best_user_match(&users, user_identifier).ok_or_else(|| {
                SearcherError::ApiError(format!("Jira user not found: {}", user_identifier))
            })?;
            Ok(user)
        } else {
            let query = vec![("username".to_string(), user_identifier.to_string())];
            self.request_json(Method::GET, "user", Some(query), None)
                .await
        }
    }

    pub async fn get_issue_watchers(&self, issue_key: &str) -> Result<Value, SearcherError> {
        self.ensure_issue_project_allowed(issue_key)?;
        self.request_json(
            Method::GET,
            &format!("issue/{}/watchers", issue_key),
            None,
            None,
        )
        .await
    }

    pub async fn add_watcher(
        &self,
        issue_key: &str,
        user_identifier: &str,
    ) -> Result<Value, SearcherError> {
        self.ensure_issue_project_allowed(issue_key)?;
        let user_reference = if self.is_cloud {
            self.resolve_cloud_account_id(user_identifier).await?
        } else {
            user_identifier.to_string()
        };

        self.request_json(
            Method::POST,
            &format!("issue/{}/watchers", issue_key),
            None,
            Some(Value::String(user_reference.clone())),
        )
        .await?;

        Ok(json!({
            "success": true,
            "issue_key": issue_key,
            "user": user_reference,
        }))
    }

    pub async fn remove_watcher(
        &self,
        issue_key: &str,
        username: Option<&str>,
        account_id: Option<&str>,
    ) -> Result<Value, SearcherError> {
        self.ensure_issue_project_allowed(issue_key)?;
        let mut query = Vec::new();
        if self.is_cloud {
            let account_id = match account_id {
                Some(value) => value.to_string(),
                None => {
                    let username = username.ok_or_else(|| {
                        SearcherError::ApiError(
                            "account_id or username is required to remove a watcher".to_string(),
                        )
                    })?;
                    self.resolve_cloud_account_id(username).await?
                }
            };
            query.push(("accountId".to_string(), account_id.clone()));
        } else {
            let username = username.ok_or_else(|| {
                SearcherError::ApiError(
                    "username is required to remove a watcher on Jira Server/DC".to_string(),
                )
            })?;
            query.push(("username".to_string(), username.to_string()));
        }

        self.request_empty(
            Method::DELETE,
            &format!("issue/{}/watchers", issue_key),
            Some(query.clone()),
            None,
        )
        .await?;

        Ok(json!({
            "success": true,
            "issue_key": issue_key,
            "query": query,
        }))
    }

    pub async fn create_issue(
        &self,
        project_key: &str,
        summary: &str,
        issue_type: &str,
        assignee: Option<&str>,
        description: Option<&str>,
        components: Option<&[String]>,
        additional_fields: Option<Value>,
    ) -> Result<Value, SearcherError> {
        let mut fields = Map::new();
        fields.insert("project".to_string(), json!({ "key": project_key }));
        fields.insert("summary".to_string(), Value::String(summary.to_string()));
        fields.insert("issuetype".to_string(), json!({ "name": issue_type }));

        if let Some(description) = description.filter(|value| !value.trim().is_empty()) {
            fields.insert(
                "description".to_string(),
                self.jira_content_value(description),
            );
        }
        if let Some(components) = components.filter(|value| !value.is_empty()) {
            fields.insert(
                "components".to_string(),
                Value::Array(
                    components
                        .iter()
                        .map(|component| json!({ "name": component }))
                        .collect(),
                ),
            );
        }
        if let Some(assignee) = assignee.filter(|value| !value.trim().is_empty()) {
            fields.insert(
                "assignee".to_string(),
                self.resolve_assignee(assignee).await?,
            );
        }

        let extra = ensure_object(additional_fields, "additional_fields")?;
        merge_objects(&mut fields, extra);
        self.normalize_issue_fields(&mut fields).await?;

        let response = self
            .request_json(
                Method::POST,
                "issue",
                None,
                Some(json!({ "fields": fields })),
            )
            .await?;

        let issue_key = response
            .get("key")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if issue_key.is_empty() {
            return Ok(response);
        }
        self.get_issue(&issue_key, None, None, Some(10), None, Some(false))
            .await
    }

    pub async fn update_issue(
        &self,
        issue_key: &str,
        fields: Option<Value>,
        additional_fields: Option<Value>,
        components: Option<&[String]>,
    ) -> Result<Value, SearcherError> {
        self.ensure_issue_project_allowed(issue_key)?;

        let mut merged = ensure_object(fields, "fields")?;
        let extra = ensure_object(additional_fields, "additional_fields")?;
        merge_objects(&mut merged, extra);

        if let Some(components) = components.filter(|value| !value.is_empty()) {
            merged.insert(
                "components".to_string(),
                Value::Array(
                    components
                        .iter()
                        .map(|component| json!({ "name": component }))
                        .collect(),
                ),
            );
        }

        self.normalize_issue_fields(&mut merged).await?;

        self.request_empty(
            Method::PUT,
            &format!("issue/{}", issue_key),
            None,
            Some(json!({ "fields": merged })),
        )
        .await?;

        self.get_issue(issue_key, None, None, Some(10), None, Some(false))
            .await
    }

    pub async fn delete_issue(&self, issue_key: &str) -> Result<Value, SearcherError> {
        self.ensure_issue_project_allowed(issue_key)?;
        self.request_empty(Method::DELETE, &format!("issue/{}", issue_key), None, None)
            .await?;
        Ok(json!({ "success": true, "issue_key": issue_key }))
    }

    pub async fn add_comment(
        &self,
        issue_key: &str,
        comment: &str,
        visibility: Option<Value>,
    ) -> Result<Value, SearcherError> {
        self.ensure_issue_project_allowed(issue_key)?;
        let mut body = Map::new();
        body.insert("body".to_string(), self.jira_content_value(comment));

        let visibility = ensure_object(visibility, "visibility")?;
        if !visibility.is_empty() {
            body.insert("visibility".to_string(), Value::Object(visibility));
        }

        self.request_json(
            Method::POST,
            &format!("issue/{}/comment", issue_key),
            None,
            Some(Value::Object(body)),
        )
        .await
    }

    pub async fn add_worklog(
        &self,
        issue_key: &str,
        time_spent: &str,
        comment: Option<&str>,
        started: Option<&str>,
        original_estimate: Option<&str>,
        remaining_estimate: Option<&str>,
    ) -> Result<Value, SearcherError> {
        self.ensure_issue_project_allowed(issue_key)?;

        if let Some(original_estimate) = original_estimate {
            self.request_empty(
                Method::PUT,
                &format!("issue/{}", issue_key),
                None,
                Some(json!({
                    "fields": {
                        "timetracking": {
                            "originalEstimate": original_estimate,
                        }
                    }
                })),
            )
            .await?;
        }

        let mut query = Vec::new();
        if let Some(remaining_estimate) = remaining_estimate {
            query.push(("adjustEstimate".to_string(), "new".to_string()));
            query.push(("newEstimate".to_string(), remaining_estimate.to_string()));
        }

        let mut body = Map::new();
        body.insert(
            "timeSpentSeconds".to_string(),
            Value::Number(parse_time_spent(time_spent).into()),
        );
        if let Some(comment) = comment.filter(|value| !value.trim().is_empty()) {
            body.insert("comment".to_string(), self.jira_content_value(comment));
        }
        if let Some(started) = started.filter(|value| !value.trim().is_empty()) {
            body.insert("started".to_string(), Value::String(started.to_string()));
        }

        self.request_json(
            Method::POST,
            &format!("issue/{}/worklog", issue_key),
            (!query.is_empty()).then_some(query),
            Some(Value::Object(body)),
        )
        .await
    }

    pub async fn transition_issue(
        &self,
        issue_key: &str,
        transition_id: &str,
        fields: Option<Value>,
        comment: Option<&str>,
    ) -> Result<Value, SearcherError> {
        self.ensure_issue_project_allowed(issue_key)?;

        let mut body = Map::new();
        body.insert("transition".to_string(), json!({ "id": transition_id }));

        let mut fields = ensure_object(fields, "fields")?;
        self.normalize_issue_fields(&mut fields).await?;
        if !fields.is_empty() {
            body.insert("fields".to_string(), Value::Object(fields));
        }

        if let Some(comment) = comment.filter(|value| !value.trim().is_empty()) {
            body.insert(
                "update".to_string(),
                json!({
                    "comment": [
                        {
                            "add": {
                                "body": self.jira_content_value(comment)
                            }
                        }
                    ]
                }),
            );
        }

        self.request_empty(
            Method::POST,
            &format!("issue/{}/transitions", issue_key),
            None,
            Some(Value::Object(body)),
        )
        .await?;

        self.get_issue(
            issue_key,
            None,
            Some("transitions"),
            Some(10),
            None,
            Some(false),
        )
        .await
    }

    pub async fn get_link_types(&self) -> Result<Value, SearcherError> {
        let response = self
            .request_json(Method::GET, "issueLinkType", None, None)
            .await?;
        Ok(response.get("issueLinkTypes").cloned().unwrap_or(response))
    }

    async fn normalize_issue_fields(
        &self,
        fields: &mut Map<String, Value>,
    ) -> Result<(), SearcherError> {
        let keys = fields.keys().cloned().collect::<Vec<_>>();
        for key in keys {
            if let Some(value) = fields.get(&key).cloned() {
                match key.as_str() {
                    "description" => {
                        if let Some(text) = value.as_str() {
                            fields.insert(key, self.jira_content_value(text));
                        }
                    }
                    "assignee" => {
                        if let Some(identifier) = value.as_str() {
                            fields.insert(key, self.resolve_assignee(identifier).await?);
                        }
                    }
                    "parent" => {
                        if let Some(parent_key) = value.as_str() {
                            fields.insert(key, json!({ "key": parent_key }));
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    async fn resolve_assignee(&self, identifier: &str) -> Result<Value, SearcherError> {
        if self.is_cloud {
            let account_id = self.resolve_cloud_account_id(identifier).await?;
            Ok(json!({ "accountId": account_id }))
        } else {
            Ok(json!({ "name": identifier }))
        }
    }

    async fn resolve_cloud_account_id(&self, identifier: &str) -> Result<String, SearcherError> {
        if let Some(stripped) = identifier.strip_prefix("accountid:") {
            return Ok(stripped.to_string());
        }

        let users = self.search_users(identifier).await?;
        let user = pick_best_user_match(&users, identifier).ok_or_else(|| {
            SearcherError::ApiError(format!("Jira user not found: {}", identifier))
        })?;
        user.get("accountId")
            .and_then(Value::as_str)
            .map(|value| value.to_string())
            .ok_or_else(|| SearcherError::ApiError("Unable to resolve Jira accountId".to_string()))
    }

    async fn search_users(&self, identifier: &str) -> Result<Vec<Value>, SearcherError> {
        let query = if self.is_cloud {
            vec![
                ("query".to_string(), identifier.to_string()),
                ("maxResults".to_string(), "20".to_string()),
            ]
        } else {
            vec![("username".to_string(), identifier.to_string())]
        };

        let response = self
            .request_json(Method::GET, "user/search", Some(query), None)
            .await?;
        Ok(response.as_array().cloned().unwrap_or_default())
    }

    async fn resolve_field_context_id(&self, field_id: &str) -> Result<String, SearcherError> {
        let response = self
            .request_json(
                Method::GET,
                &format!("field/{}/context", field_id),
                None,
                None,
            )
            .await?;
        response
            .get("values")
            .and_then(Value::as_array)
            .and_then(|values| values.first())
            .and_then(|value| value.get("id"))
            .and_then(Value::as_str)
            .map(|value| value.to_string())
            .ok_or_else(|| {
                SearcherError::ApiError(format!("No Jira field context found for {}", field_id))
            })
    }

    async fn resolve_server_field_options(
        &self,
        field_id: &str,
        project_key: &str,
        issue_type: &str,
    ) -> Result<Vec<Value>, SearcherError> {
        let query = vec![
            ("projectKeys".to_string(), project_key.to_string()),
            ("issuetypeNames".to_string(), issue_type.to_string()),
            (
                "expand".to_string(),
                "projects.issuetypes.fields".to_string(),
            ),
        ];
        let response = self
            .request_json(Method::GET, "issue/createmeta", Some(query), None)
            .await?;

        let field = response
            .get("projects")
            .and_then(Value::as_array)
            .and_then(|projects| projects.first())
            .and_then(|project| project.get("issuetypes"))
            .and_then(Value::as_array)
            .and_then(|issuetypes| {
                issuetypes.iter().find(|candidate| {
                    candidate
                        .get("name")
                        .and_then(Value::as_str)
                        .map(|name| name.eq_ignore_ascii_case(issue_type))
                        .unwrap_or(false)
                })
            })
            .and_then(|issuetype| issuetype.get("fields"))
            .and_then(Value::as_object)
            .and_then(|fields| fields.get(field_id))
            .ok_or_else(|| {
                SearcherError::ApiError(format!(
                    "Field {} not found in createmeta for project {} / issue type {}",
                    field_id, project_key, issue_type
                ))
            })?;

        Ok(field
            .get("allowedValues")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default())
    }

    pub async fn download_attachments(&self, issue_key: &str) -> Result<Value, SearcherError> {
        self.ensure_issue_project_allowed(issue_key)?;
        let attachments = self.issue_attachments(issue_key).await?;
        let mut results = Vec::new();
        for attachment in attachments {
            let mut item = attachment.clone();
            if let Some(url) = attachment.get("content").and_then(Value::as_str) {
                let bytes = self.request_absolute_bytes(Method::GET, url).await?;
                item["data_base64"] =
                    Value::String(base64::engine::general_purpose::STANDARD.encode(bytes));
            }
            results.push(item);
        }
        Ok(Value::Array(results))
    }

    pub async fn get_issue_images(&self, issue_key: &str) -> Result<Value, SearcherError> {
        let attachments = self.issue_attachments(issue_key).await?;
        let mut results = Vec::new();
        for attachment in attachments {
            let media_type = attachment
                .get("mimeType")
                .or_else(|| attachment.get("contentType"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            let filename = attachment
                .get("filename")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_ascii_lowercase();
            let is_image = media_type.starts_with("image/")
                || [".png", ".jpg", ".jpeg", ".gif", ".bmp", ".webp", ".svg"]
                    .iter()
                    .any(|ext| filename.ends_with(ext));
            if !is_image {
                continue;
            }

            let mut item = attachment.clone();
            if let Some(url) = attachment.get("content").and_then(Value::as_str) {
                let bytes = self.request_absolute_bytes(Method::GET, url).await?;
                item["data_base64"] =
                    Value::String(base64::engine::general_purpose::STANDARD.encode(bytes));
            }
            results.push(item);
        }
        Ok(Value::Array(results))
    }

    pub async fn get_agile_boards(
        &self,
        board_name: Option<&str>,
        project_key: Option<&str>,
        board_type: Option<&str>,
        start_at: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Value, SearcherError> {
        let mut query = vec![
            ("startAt".to_string(), start_at.unwrap_or(0).to_string()),
            (
                "maxResults".to_string(),
                limit.unwrap_or(10).clamp(1, 100).to_string(),
            ),
        ];
        if let Some(board_name) = board_name.filter(|value| !value.trim().is_empty()) {
            query.push(("name".to_string(), board_name.to_string()));
        }
        if let Some(project_key) = project_key.filter(|value| !value.trim().is_empty()) {
            query.push(("projectKeyOrId".to_string(), project_key.to_string()));
        }
        if let Some(board_type) = board_type.filter(|value| !value.trim().is_empty()) {
            query.push(("type".to_string(), board_type.to_string()));
        }

        let response = self
            .request_agile_json(Method::GET, "board", Some(query), None)
            .await?;
        Ok(response.get("values").cloned().unwrap_or(response))
    }

    pub async fn get_board_issues(
        &self,
        board_id: &str,
        jql: &str,
        fields: Option<&[String]>,
        start_at: Option<usize>,
        limit: Option<usize>,
        expand: Option<&str>,
    ) -> Result<Value, SearcherError> {
        let mut query = vec![
            ("jql".to_string(), jql.to_string()),
            (
                "fields".to_string(),
                self.fields_csv(fields, DEFAULT_READ_FIELDS),
            ),
            ("startAt".to_string(), start_at.unwrap_or(0).to_string()),
            (
                "maxResults".to_string(),
                limit.unwrap_or(10).clamp(1, 100).to_string(),
            ),
        ];
        if let Some(expand) = expand.filter(|value| !value.trim().is_empty()) {
            query.push(("expand".to_string(), expand.to_string()));
        }
        self.request_agile_json(
            Method::GET,
            &format!("board/{}/issue", board_id),
            Some(query),
            None,
        )
        .await
    }

    pub async fn get_sprints_from_board(
        &self,
        board_id: &str,
        state: Option<&str>,
        start_at: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Value, SearcherError> {
        let mut query = vec![
            ("startAt".to_string(), start_at.unwrap_or(0).to_string()),
            (
                "maxResults".to_string(),
                limit.unwrap_or(10).clamp(1, 100).to_string(),
            ),
        ];
        if let Some(state) = state.filter(|value| !value.trim().is_empty()) {
            query.push(("state".to_string(), state.to_string()));
        }
        let response = self
            .request_agile_json(
                Method::GET,
                &format!("board/{}/sprint", board_id),
                Some(query),
                None,
            )
            .await?;
        Ok(response.get("values").cloned().unwrap_or(response))
    }

    pub async fn get_sprint_issues(
        &self,
        sprint_id: &str,
        fields: Option<&[String]>,
        start_at: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Value, SearcherError> {
        let query = vec![
            (
                "fields".to_string(),
                self.fields_csv(fields, DEFAULT_READ_FIELDS),
            ),
            ("startAt".to_string(), start_at.unwrap_or(0).to_string()),
            (
                "maxResults".to_string(),
                limit.unwrap_or(10).clamp(1, 100).to_string(),
            ),
        ];
        self.request_agile_json(
            Method::GET,
            &format!("sprint/{}/issue", sprint_id),
            Some(query),
            None,
        )
        .await
    }

    pub async fn batch_create_issues(
        &self,
        issues: &[Value],
        validate_only: bool,
    ) -> Result<Value, SearcherError> {
        let mut issue_updates = Vec::new();
        for issue in issues {
            let issue = issue.as_object().ok_or_else(|| {
                SearcherError::ApiError("Each item in issues must be a JSON object".to_string())
            })?;
            let project_key = issue
                .get("project_key")
                .and_then(Value::as_str)
                .ok_or_else(|| SearcherError::ApiError("project_key is required".to_string()))?;
            let summary = issue
                .get("summary")
                .and_then(Value::as_str)
                .ok_or_else(|| SearcherError::ApiError("summary is required".to_string()))?;
            let issue_type = issue
                .get("issue_type")
                .and_then(Value::as_str)
                .ok_or_else(|| SearcherError::ApiError("issue_type is required".to_string()))?;

            let mut fields = Map::new();
            fields.insert("project".to_string(), json!({ "key": project_key }));
            fields.insert("summary".to_string(), Value::String(summary.to_string()));
            fields.insert("issuetype".to_string(), json!({ "name": issue_type }));
            if let Some(description) = issue.get("description").and_then(Value::as_str) {
                fields.insert(
                    "description".to_string(),
                    self.jira_content_value(description),
                );
            }
            if let Some(assignee) = issue.get("assignee").and_then(Value::as_str) {
                fields.insert(
                    "assignee".to_string(),
                    self.resolve_assignee(assignee).await?,
                );
            }
            if let Some(components) = issue.get("components").and_then(Value::as_array) {
                fields.insert(
                    "components".to_string(),
                    Value::Array(
                        components
                            .iter()
                            .filter_map(Value::as_str)
                            .map(|component| json!({ "name": component }))
                            .collect(),
                    ),
                );
            }
            let mut fields = fields;
            for (key, value) in issue {
                if matches!(
                    key.as_str(),
                    "project_key"
                        | "summary"
                        | "issue_type"
                        | "description"
                        | "assignee"
                        | "components"
                ) {
                    continue;
                }
                fields.insert(key.clone(), value.clone());
            }
            self.normalize_issue_fields(&mut fields).await?;
            issue_updates.push(Value::Object({
                let mut root = Map::new();
                root.insert("fields".to_string(), Value::Object(fields));
                root
            }));
        }

        let mut query = Vec::new();
        if validate_only {
            query.push(("validateOnly".to_string(), "true".to_string()));
        }
        self.request_json(
            Method::POST,
            "issue/bulk",
            (!query.is_empty()).then_some(query),
            Some(json!({ "issueUpdates": issue_updates })),
        )
        .await
    }

    pub async fn batch_get_changelogs(
        &self,
        issue_ids_or_keys: &[String],
        fields: Option<&[String]>,
        limit: Option<usize>,
    ) -> Result<Value, SearcherError> {
        if !self.is_cloud {
            return Err(SearcherError::ApiError(
                "Batch get issue changelogs is only available on Jira Cloud".to_string(),
            ));
        }

        let mut body = json!({
            "issueIdsOrKeys": issue_ids_or_keys,
        });
        if let Some(fields) = fields.filter(|items| !items.is_empty()) {
            body["fieldIds"] = Value::Array(
                fields
                    .iter()
                    .map(|field| Value::String(field.clone()))
                    .collect(),
            );
        }

        let mut all_changes = BTreeMap::<String, Vec<Value>>::new();
        let mut next_page_token: Option<String> = None;
        loop {
            let mut request_body = body.clone();
            if let Some(token) = next_page_token.as_deref() {
                request_body["nextPageToken"] = Value::String(token.to_string());
            }
            let page = self
                .request_json(
                    Method::POST,
                    "changelog/bulkfetch",
                    None,
                    Some(request_body),
                )
                .await?;
            if let Some(entries) = page.get("issueChangeLogs").and_then(Value::as_array) {
                for entry in entries {
                    let issue_id = entry
                        .get("issueId")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let target = all_changes.entry(issue_id).or_default();
                    if let Some(changes) = entry.get("changeHistories").and_then(Value::as_array) {
                        target.extend(changes.clone());
                    }
                }
            }
            next_page_token = page
                .get("nextPageToken")
                .and_then(Value::as_str)
                .map(|value| value.to_string());
            if next_page_token.is_none() {
                break;
            }
        }

        let per_issue_limit = limit.unwrap_or(usize::MAX);
        let results = all_changes
            .into_iter()
            .map(|(issue_id, mut changes)| {
                if per_issue_limit != usize::MAX {
                    changes.truncate(per_issue_limit);
                }
                json!({ "issue_id": issue_id, "changelogs": changes })
            })
            .collect::<Vec<_>>();
        Ok(Value::Array(results))
    }

    pub async fn edit_comment(
        &self,
        issue_key: &str,
        comment_id: &str,
        body: &str,
        visibility: Option<Value>,
    ) -> Result<Value, SearcherError> {
        self.ensure_issue_project_allowed(issue_key)?;
        let mut payload = Map::new();
        payload.insert("body".to_string(), self.jira_content_value(body));
        let visibility = ensure_object(visibility, "visibility")?;
        if !visibility.is_empty() {
            payload.insert("visibility".to_string(), Value::Object(visibility));
        }
        self.request_json(
            Method::PUT,
            &format!("issue/{}/comment/{}", issue_key, comment_id),
            None,
            Some(Value::Object(payload)),
        )
        .await
    }

    pub async fn link_to_epic(
        &self,
        issue_key: &str,
        epic_key: &str,
    ) -> Result<Value, SearcherError> {
        self.ensure_issue_project_allowed(issue_key)?;
        let parent_attempt = self
            .request_empty(
                Method::PUT,
                &format!("issue/{}", issue_key),
                None,
                Some(json!({ "fields": { "parent": { "key": epic_key } } })),
            )
            .await;
        if parent_attempt.is_ok() {
            return self
                .get_issue(issue_key, None, None, Some(10), None, Some(false))
                .await;
        }

        if let Some(field_id) = self.detect_epic_link_field().await? {
            self.request_empty(
                Method::PUT,
                &format!("issue/{}", issue_key),
                None,
                Some(json!({ "fields": { field_id: epic_key } })),
            )
            .await?;
            return self
                .get_issue(issue_key, None, None, Some(10), None, Some(false))
                .await;
        }

        Err(SearcherError::ApiError(format!(
            "Could not determine epic link field to link {} -> {}",
            issue_key, epic_key
        )))
    }

    pub async fn create_issue_link(
        &self,
        link_type: &str,
        inward_issue_key: &str,
        outward_issue_key: &str,
        comment: Option<&str>,
        comment_visibility: Option<Value>,
    ) -> Result<Value, SearcherError> {
        let mut payload = Map::new();
        payload.insert("type".to_string(), json!({ "name": link_type }));
        payload.insert(
            "inwardIssue".to_string(),
            json!({ "key": inward_issue_key }),
        );
        payload.insert(
            "outwardIssue".to_string(),
            json!({ "key": outward_issue_key }),
        );
        if let Some(comment) = comment.filter(|value| !value.trim().is_empty()) {
            let mut comment_obj = Map::new();
            comment_obj.insert("body".to_string(), self.jira_content_value(comment));
            let visibility = ensure_object(comment_visibility, "comment_visibility")?;
            if !visibility.is_empty() {
                comment_obj.insert("visibility".to_string(), Value::Object(visibility));
            }
            payload.insert("comment".to_string(), Value::Object(comment_obj));
        }
        self.request_json(
            Method::POST,
            "issueLink",
            None,
            Some(Value::Object(payload)),
        )
        .await
    }

    pub async fn create_remote_issue_link(
        &self,
        issue_key: &str,
        url: &str,
        title: &str,
        summary: Option<&str>,
        relationship: Option<&str>,
        icon_url: Option<&str>,
    ) -> Result<Value, SearcherError> {
        let mut object = Map::new();
        object.insert("url".to_string(), Value::String(url.to_string()));
        object.insert("title".to_string(), Value::String(title.to_string()));
        if let Some(summary) = summary.filter(|value| !value.trim().is_empty()) {
            object.insert("summary".to_string(), Value::String(summary.to_string()));
        }
        if let Some(icon_url) = icon_url.filter(|value| !value.trim().is_empty()) {
            object.insert(
                "icon".to_string(),
                json!({
                    "url16x16": icon_url,
                    "title": title,
                }),
            );
        }

        let mut payload = Map::new();
        payload.insert("object".to_string(), Value::Object(object));
        if let Some(relationship) = relationship.filter(|value| !value.trim().is_empty()) {
            payload.insert(
                "relationship".to_string(),
                Value::String(relationship.to_string()),
            );
        }

        self.request_json(
            Method::POST,
            &format!("issue/{}/remotelink", issue_key),
            None,
            Some(Value::Object(payload)),
        )
        .await
    }

    pub async fn remove_issue_link(&self, link_id: &str) -> Result<Value, SearcherError> {
        self.request_empty(
            Method::DELETE,
            &format!("issueLink/{}", link_id),
            None,
            None,
        )
        .await?;
        Ok(json!({ "success": true, "link_id": link_id }))
    }

    pub async fn create_sprint(
        &self,
        board_id: &str,
        name: &str,
        start_date: &str,
        end_date: &str,
        goal: Option<&str>,
    ) -> Result<Value, SearcherError> {
        let mut payload = Map::new();
        payload.insert("name".to_string(), Value::String(name.to_string()));
        payload.insert(
            "originBoardId".to_string(),
            json!(board_id.parse::<u64>().unwrap_or(0)),
        );
        payload.insert(
            "startDate".to_string(),
            Value::String(start_date.to_string()),
        );
        payload.insert("endDate".to_string(), Value::String(end_date.to_string()));
        if let Some(goal) = goal.filter(|value| !value.trim().is_empty()) {
            payload.insert("goal".to_string(), Value::String(goal.to_string()));
        }
        self.request_agile_json(Method::POST, "sprint", None, Some(Value::Object(payload)))
            .await
    }

    pub async fn update_sprint(
        &self,
        sprint_id: &str,
        name: Option<&str>,
        state: Option<&str>,
        start_date: Option<&str>,
        end_date: Option<&str>,
        goal: Option<&str>,
    ) -> Result<Value, SearcherError> {
        let mut payload = Map::new();
        if let Some(name) = name.filter(|value| !value.trim().is_empty()) {
            payload.insert("name".to_string(), Value::String(name.to_string()));
        }
        if let Some(state) = state.filter(|value| !value.trim().is_empty()) {
            payload.insert("state".to_string(), Value::String(state.to_string()));
        }
        if let Some(start_date) = start_date.filter(|value| !value.trim().is_empty()) {
            payload.insert(
                "startDate".to_string(),
                Value::String(start_date.to_string()),
            );
        }
        if let Some(end_date) = end_date.filter(|value| !value.trim().is_empty()) {
            payload.insert("endDate".to_string(), Value::String(end_date.to_string()));
        }
        if let Some(goal) = goal.filter(|value| !value.trim().is_empty()) {
            payload.insert("goal".to_string(), Value::String(goal.to_string()));
        }
        self.request_agile_json(
            Method::PUT,
            &format!("sprint/{}", sprint_id),
            None,
            Some(Value::Object(payload)),
        )
        .await
    }

    pub async fn add_issues_to_sprint(
        &self,
        sprint_id: &str,
        issue_keys: &[String],
    ) -> Result<Value, SearcherError> {
        self.request_agile_json(
            Method::POST,
            &format!("sprint/{}/issue", sprint_id),
            None,
            Some(json!({ "issues": issue_keys })),
        )
        .await
    }

    pub async fn get_service_desk_for_project(
        &self,
        project_key: &str,
    ) -> Result<Value, SearcherError> {
        let mut start = 0;
        loop {
            let response = self
                .request_servicedesk_json(
                    Method::GET,
                    "servicedesk",
                    Some(vec![
                        ("start".to_string(), start.to_string()),
                        ("limit".to_string(), "50".to_string()),
                    ]),
                    None,
                )
                .await?;
            let values = response
                .get("values")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            if let Some(service_desk) = values.iter().find(|desk| {
                desk.get("projectKey")
                    .and_then(Value::as_str)
                    .map(|value| value.eq_ignore_ascii_case(project_key))
                    .unwrap_or(false)
            }) {
                return Ok(service_desk.clone());
            }
            let is_last = response
                .get("isLastPage")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            if is_last || values.is_empty() {
                break;
            }
            start += values.len();
        }
        Ok(Value::Null)
    }

    pub async fn get_service_desk_queues(
        &self,
        service_desk_id: &str,
        start_at: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Value, SearcherError> {
        self.request_servicedesk_json(
            Method::GET,
            &format!("servicedesk/{}/queue", service_desk_id),
            Some(vec![
                ("start".to_string(), start_at.unwrap_or(0).to_string()),
                (
                    "limit".to_string(),
                    limit.unwrap_or(50).clamp(1, 100).to_string(),
                ),
                ("includeCount".to_string(), "true".to_string()),
            ]),
            None,
        )
        .await
    }

    pub async fn get_queue_issues(
        &self,
        service_desk_id: &str,
        queue_id: &str,
        start_at: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Value, SearcherError> {
        self.request_servicedesk_json(
            Method::GET,
            &format!("servicedesk/{}/queue/{}/issue", service_desk_id, queue_id),
            Some(vec![
                ("start".to_string(), start_at.unwrap_or(0).to_string()),
                (
                    "limit".to_string(),
                    limit.unwrap_or(50).clamp(1, 100).to_string(),
                ),
            ]),
            None,
        )
        .await
    }

    pub async fn create_version(
        &self,
        project_key: &str,
        name: &str,
        start_date: Option<&str>,
        release_date: Option<&str>,
        description: Option<&str>,
    ) -> Result<Value, SearcherError> {
        let mut payload = Map::new();
        payload.insert(
            "project".to_string(),
            Value::String(project_key.to_string()),
        );
        payload.insert("name".to_string(), Value::String(name.to_string()));
        if let Some(start_date) = start_date.filter(|value| !value.trim().is_empty()) {
            payload.insert(
                "startDate".to_string(),
                Value::String(start_date.to_string()),
            );
        }
        if let Some(release_date) = release_date.filter(|value| !value.trim().is_empty()) {
            payload.insert(
                "releaseDate".to_string(),
                Value::String(release_date.to_string()),
            );
        }
        if let Some(description) = description.filter(|value| !value.trim().is_empty()) {
            payload.insert(
                "description".to_string(),
                Value::String(description.to_string()),
            );
        }
        self.request_json(Method::POST, "version", None, Some(Value::Object(payload)))
            .await
    }

    pub async fn batch_create_versions(
        &self,
        project_key: &str,
        versions: &[Value],
    ) -> Result<Value, SearcherError> {
        let mut results = Vec::new();
        for version in versions {
            let version = version.as_object().ok_or_else(|| {
                SearcherError::ApiError("Each version must be a JSON object".to_string())
            })?;
            let name = version
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| SearcherError::ApiError("Version name is required".to_string()))?;
            let created = self
                .create_version(
                    project_key,
                    name,
                    version.get("start_date").and_then(Value::as_str),
                    version.get("release_date").and_then(Value::as_str),
                    version.get("description").and_then(Value::as_str),
                )
                .await?;
            results.push(created);
        }
        Ok(Value::Array(results))
    }

    pub async fn get_issue_proforma_forms(&self, issue_key: &str) -> Result<Value, SearcherError> {
        self.request_forms_json(Method::GET, &format!("/issue/{}/form", issue_key), None)
            .await
    }

    pub async fn get_proforma_form_details(
        &self,
        issue_key: &str,
        form_id: &str,
    ) -> Result<Value, SearcherError> {
        self.request_forms_json(
            Method::GET,
            &format!("/issue/{}/form/{}", issue_key, form_id),
            None,
        )
        .await
    }

    pub async fn update_proforma_form_answers(
        &self,
        issue_key: &str,
        form_id: &str,
        answers: &[Value],
    ) -> Result<Value, SearcherError> {
        self.request_forms_json(
            Method::PUT,
            &format!("/issue/{}/form/{}", issue_key, form_id),
            Some(json!({ "answers": answers })),
        )
        .await
    }

    pub async fn get_issue_dates(
        &self,
        issue_key: &str,
        include_status_changes: bool,
        include_status_summary: bool,
    ) -> Result<Value, SearcherError> {
        let issue = self
            .get_issue(
                issue_key,
                Some(&vec![
                    "created".to_string(),
                    "updated".to_string(),
                    "duedate".to_string(),
                    "resolutiondate".to_string(),
                    "status".to_string(),
                ]),
                Some("changelog"),
                Some(0),
                None,
                Some(false),
            )
            .await?;
        let fields = issue
            .get("fields")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let created = fields.get("created").and_then(Value::as_str);
        let updated = fields.get("updated").and_then(Value::as_str);
        let due_date = fields.get("duedate").and_then(Value::as_str);
        let resolution_date = fields.get("resolutiondate").and_then(Value::as_str);
        let current_status = fields
            .get("status")
            .and_then(|status| status.get("name"))
            .and_then(Value::as_str)
            .unwrap_or_default();

        let status_changes = if include_status_changes || include_status_summary {
            self.extract_status_changes(&issue)?
        } else {
            Vec::new()
        };
        let status_summary = if include_status_summary {
            summarize_status_changes(&status_changes)
        } else {
            Vec::new()
        };

        Ok(json!({
            "issue_key": issue_key,
            "created": created,
            "updated": updated,
            "due_date": due_date,
            "resolution_date": resolution_date,
            "current_status": current_status,
            "status_changes": if include_status_changes { status_changes } else { Vec::new() },
            "status_summary": status_summary,
        }))
    }

    pub async fn get_issue_sla(
        &self,
        issue_key: &str,
        metrics: Option<&[String]>,
        working_hours_only: Option<bool>,
        include_raw_dates: bool,
    ) -> Result<Value, SearcherError> {
        let raw = self.get_issue_dates(issue_key, true, true).await?;
        let selected = metrics.map(|items| items.to_vec()).unwrap_or_else(|| {
            vec![
                "cycle_time".to_string(),
                "lead_time".to_string(),
                "time_in_status".to_string(),
                "due_date_compliance".to_string(),
                "resolution_time".to_string(),
                "first_response_time".to_string(),
            ]
        });
        let created = raw
            .get("created")
            .and_then(Value::as_str)
            .and_then(parse_jira_datetime);
        let resolution = raw
            .get("resolution_date")
            .and_then(Value::as_str)
            .and_then(parse_jira_datetime);
        let due_date = raw
            .get("due_date")
            .and_then(Value::as_str)
            .and_then(parse_jira_datetime);
        let status_summary = raw
            .get("status_summary")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let first_response = raw
            .get("status_changes")
            .and_then(Value::as_array)
            .and_then(|changes| changes.first())
            .and_then(|change| change.get("entered_at"))
            .and_then(Value::as_str)
            .and_then(parse_jira_datetime);
        let mut metrics_map = Map::new();
        for metric in selected {
            match metric.as_str() {
                "cycle_time" | "lead_time" | "resolution_time" => {
                    if let (Some(created), Some(resolution)) = (created, resolution) {
                        metrics_map.insert(
                            metric,
                            json!({
                                "minutes": (resolution - created).num_minutes(),
                                "working_hours_only": working_hours_only.unwrap_or(false),
                            }),
                        );
                    }
                }
                "due_date_compliance" => {
                    if let (Some(due_date), Some(resolution)) = (due_date, resolution) {
                        metrics_map.insert(
                            metric,
                            json!({
                                "met": resolution <= due_date,
                                "due_date": due_date.to_rfc3339(),
                                "resolution_date": resolution.to_rfc3339(),
                            }),
                        );
                    }
                }
                "first_response_time" => {
                    if let (Some(created), Some(first_response)) = (created, first_response) {
                        metrics_map.insert(
                            metric,
                            json!({
                                "minutes": (first_response - created).num_minutes(),
                            }),
                        );
                    }
                }
                "time_in_status" => {
                    metrics_map.insert(metric, Value::Array(status_summary.clone()));
                }
                _ => {}
            }
        }
        Ok(json!({
            "issue_key": issue_key,
            "metrics": metrics_map,
            "raw_dates": include_raw_dates.then_some(raw).unwrap_or(Value::Null),
        }))
    }

    pub async fn get_issue_development_info(
        &self,
        issue_key: &str,
        application_type: Option<&str>,
        data_type: Option<&str>,
    ) -> Result<Value, SearcherError> {
        let issue = self
            .request_json(
                Method::GET,
                &format!("issue/{}", issue_key),
                Some(vec![("fields".to_string(), "id".to_string())]),
                None,
            )
            .await?;
        let issue_id = issue.get("id").and_then(Value::as_str).ok_or_else(|| {
            SearcherError::ApiError(format!("Could not get issue id for {}", issue_key))
        })?;

        let app_types = application_type
            .map(|value| vec![value.to_string()])
            .unwrap_or_else(|| {
                vec![
                    "stash".to_string(),
                    "bitbucket".to_string(),
                    "github".to_string(),
                    "gitlab".to_string(),
                ]
            });
        let data_types = data_type
            .map(|value| vec![value.to_string()])
            .unwrap_or_else(|| {
                vec![
                    "pullrequest".to_string(),
                    "branch".to_string(),
                    "repository".to_string(),
                ]
            });

        let mut merged = json!({
            "issue_key": issue_key,
            "detail": [],
            "pullRequests": [],
            "branches": [],
            "commits": [],
            "repositories": [],
        });
        for app in &app_types {
            for dt in &data_types {
                let response = self
                    .client
                    .request(
                        Method::GET,
                        format!("{}/rest/dev-status/1.0/issue/detail", self.base_url),
                    )
                    .query(&[
                        ("issueId", issue_id),
                        ("applicationType", app.as_str()),
                        ("dataType", dt.as_str()),
                    ])
                    .send()
                    .await?;
                if !response.status().is_success() {
                    continue;
                }
                let value: Value = response.json().await?;
                merge_development_info(&mut merged, &value);
            }
        }
        Ok(merged)
    }

    pub async fn get_issues_development_info(
        &self,
        issue_keys: &[String],
        application_type: Option<&str>,
        data_type: Option<&str>,
    ) -> Result<Value, SearcherError> {
        let mut results = Vec::new();
        for issue_key in issue_keys {
            results.push(
                self.get_issue_development_info(issue_key, application_type, data_type)
                    .await?,
            );
        }
        Ok(Value::Array(results))
    }

    async fn detect_epic_link_field(&self) -> Result<Option<String>, SearcherError> {
        let fields = self.request_json(Method::GET, "field", None, None).await?;
        Ok(fields.as_array().and_then(|fields| {
            fields.iter().find_map(|field| {
                let id = field.get("id").and_then(Value::as_str)?;
                let name = field
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let is_match = name.eq_ignore_ascii_case("Epic Link")
                    || field
                        .get("clauseNames")
                        .and_then(Value::as_array)
                        .map(|clauses| {
                            clauses.iter().filter_map(Value::as_str).any(|clause| {
                                clause.eq_ignore_ascii_case("epic link")
                                    || clause.eq_ignore_ascii_case("epicLink")
                            })
                        })
                        .unwrap_or(false);
                is_match.then(|| id.to_string())
            })
        }))
    }

    async fn issue_attachments(&self, issue_key: &str) -> Result<Vec<Value>, SearcherError> {
        let issue = self
            .request_json(
                Method::GET,
                &format!("issue/{}", issue_key),
                Some(vec![("fields".to_string(), "attachment".to_string())]),
                None,
            )
            .await?;
        Ok(issue
            .get("fields")
            .and_then(|fields| fields.get("attachment"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default())
    }

    fn extract_status_changes(&self, issue: &Value) -> Result<Vec<Value>, SearcherError> {
        let histories = issue
            .get("changelog")
            .and_then(|changelog| changelog.get("histories"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let created = issue
            .get("fields")
            .and_then(|fields| fields.get("created"))
            .and_then(Value::as_str)
            .and_then(parse_jira_datetime);
        let mut transitions = Vec::<(
            Option<String>,
            Option<String>,
            DateTime<FixedOffset>,
            String,
        )>::new();
        for history in histories {
            let author = history
                .get("author")
                .and_then(|author| author.get("displayName"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let created = history
                .get("created")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    SearcherError::ApiError("Missing changelog created timestamp".to_string())
                })?;
            let created_dt = parse_jira_datetime(created).ok_or_else(|| {
                SearcherError::ApiError(format!("Invalid Jira changelog timestamp: {}", created))
            })?;
            if let Some(items) = history.get("items").and_then(Value::as_array) {
                for item in items {
                    let field = item
                        .get("field")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    if field != "status" {
                        continue;
                    }
                    transitions.push((
                        item.get("fromString")
                            .and_then(Value::as_str)
                            .map(|value| value.to_string()),
                        item.get("toString")
                            .and_then(Value::as_str)
                            .map(|value| value.to_string()),
                        created_dt,
                        author.clone(),
                    ));
                }
            }
        }
        transitions.sort_by_key(|(_, _, entered_at, _)| *entered_at);

        let mut changes = Vec::new();
        if let (Some(created), Some((Some(initial_status), _, first_entered_at, _))) =
            (created, transitions.first())
        {
            let duration_minutes = (*first_entered_at - created).num_minutes();
            changes.push(json!({
                "status": initial_status,
                "from_status": Value::Null,
                "entered_at": created.to_rfc3339(),
                "exited_at": first_entered_at.to_rfc3339(),
                "duration_minutes": duration_minutes,
                "duration_formatted": format_duration_minutes(duration_minutes),
                "transitioned_by": Value::Null,
            }));
        }

        for (index, (from_status, to_status, entered_at, transitioned_by)) in
            transitions.iter().enumerate()
        {
            let next_entered_at = transitions
                .get(index + 1)
                .map(|(_, _, next_entered_at, _)| next_entered_at);
            let duration_minutes = next_entered_at.map(|next| (*next - *entered_at).num_minutes());
            changes.push(json!({
                "status": to_status.clone(),
                "from_status": from_status.clone(),
                "entered_at": entered_at.to_rfc3339(),
                "exited_at": next_entered_at.map(|value| value.to_rfc3339()),
                "duration_minutes": duration_minutes,
                "duration_formatted": duration_minutes.map(format_duration_minutes),
                "transitioned_by": if transitioned_by.is_empty() { Value::Null } else { Value::String(transitioned_by.clone()) },
            }));
        }
        Ok(changes)
    }

    fn ensure_issue_project_allowed(&self, issue_key: &str) -> Result<(), SearcherError> {
        let Some(filter) = self.projects_filter.as_deref() else {
            return Ok(());
        };
        let issue_project = issue_key.split('-').next().unwrap_or_default();
        let allowed = filter
            .split(',')
            .map(|item| item.trim())
            .filter(|item| !item.is_empty())
            .any(|item| item.eq_ignore_ascii_case(issue_project));

        if allowed {
            Ok(())
        } else {
            Err(SearcherError::ApiError(format!(
                "Issue project '{}' is restricted by JIRA_PROJECTS_FILTER",
                issue_project
            )))
        }
    }

    fn apply_projects_filter(&self, jql: &str, override_filter: Option<&str>) -> String {
        let filter = override_filter
            .and_then(|value| (!value.trim().is_empty()).then_some(value.trim()))
            .or(self.projects_filter.as_deref());

        let Some(filter) = filter else {
            return jql.to_string();
        };

        let projects = filter
            .split(',')
            .map(|item| item.trim())
            .filter(|item| !item.is_empty())
            .map(quote_jql_literal)
            .collect::<Vec<_>>();
        if projects.is_empty() {
            return jql.to_string();
        }

        let project_clause = if projects.len() == 1 {
            format!("project = {}", projects[0])
        } else {
            format!("project IN ({})", projects.join(", "))
        };

        if jql.trim().is_empty() {
            project_clause
        } else if jql.to_ascii_lowercase().contains("project =")
            || jql.to_ascii_lowercase().contains("project in")
        {
            jql.to_string()
        } else {
            format!("({}) AND {}", jql, project_clause)
        }
    }

    fn fields_csv(&self, fields: Option<&[String]>, defaults: &[&str]) -> String {
        self.fields_vec(fields, defaults).join(",")
    }

    fn fields_vec(&self, fields: Option<&[String]>, defaults: &[&str]) -> Vec<String> {
        match fields {
            Some(values) if !values.is_empty() => values.to_vec(),
            _ => defaults.iter().map(|value| value.to_string()).collect(),
        }
    }

    fn jira_content_value(&self, content: &str) -> Value {
        if !self.is_cloud {
            return Value::String(content.to_string());
        }

        let paragraphs = content
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                json!({
                    "type": "paragraph",
                    "content": [
                        {
                            "type": "text",
                            "text": line,
                        }
                    ]
                })
            })
            .collect::<Vec<_>>();

        json!({
            "type": "doc",
            "version": 1,
            "content": if paragraphs.is_empty() {
                vec![json!({ "type": "paragraph", "content": [] })]
            } else {
                paragraphs
            }
        })
    }

    fn api_url(&self, path: &str) -> String {
        format!(
            "{}/rest/api/{}/{}",
            self.base_url,
            self.api_version,
            path.trim_start_matches('/'),
        )
    }

    async fn request_json(
        &self,
        method: Method,
        path: &str,
        query: Option<Vec<(String, String)>>,
        body: Option<Value>,
    ) -> Result<Value, SearcherError> {
        let mut request = self.client.request(method, self.api_url(path));
        if let Some(query) = query.filter(|items| !items.is_empty()) {
            request = request.query(&query);
        }
        if let Some(body) = body {
            request = request.json(&body);
        }
        send_json(request).await
    }

    async fn request_empty(
        &self,
        method: Method,
        path: &str,
        query: Option<Vec<(String, String)>>,
        body: Option<Value>,
    ) -> Result<(), SearcherError> {
        let mut request = self.client.request(method, self.api_url(path));
        if let Some(query) = query.filter(|items| !items.is_empty()) {
            request = request.query(&query);
        }
        if let Some(body) = body {
            request = request.json(&body);
        }
        send_empty(request).await
    }

    async fn request_absolute_bytes(
        &self,
        method: Method,
        url: &str,
    ) -> Result<Vec<u8>, SearcherError> {
        send_bytes(
            self.client
                .request(method, url)
                .header(ACCEPT, HeaderValue::from_static("*/*")),
        )
        .await
    }

    async fn request_agile_json(
        &self,
        method: Method,
        path: &str,
        query: Option<Vec<(String, String)>>,
        body: Option<Value>,
    ) -> Result<Value, SearcherError> {
        let mut request = self.client.request(
            method,
            format!(
                "{}/rest/agile/1.0/{}",
                self.base_url,
                path.trim_start_matches('/')
            ),
        );
        if let Some(query) = query.filter(|items| !items.is_empty()) {
            request = request.query(&query);
        }
        if let Some(body) = body {
            request = request.json(&body);
        }
        send_json(request).await
    }

    async fn request_servicedesk_json(
        &self,
        method: Method,
        path: &str,
        query: Option<Vec<(String, String)>>,
        body: Option<Value>,
    ) -> Result<Value, SearcherError> {
        let mut request = self.client.request(
            method,
            format!(
                "{}/rest/servicedeskapi/{}",
                self.base_url,
                path.trim_start_matches('/')
            ),
        );
        if let Some(query) = query.filter(|items| !items.is_empty()) {
            request = request.query(&query);
        }
        if let Some(body) = body {
            request = request.json(&body);
        }
        send_json(request).await
    }

    async fn request_forms_json(
        &self,
        method: Method,
        endpoint: &str,
        body: Option<Value>,
    ) -> Result<Value, SearcherError> {
        let cloud_id = std::env::var("ATLASSIAN_OAUTH_CLOUD_ID")
            .map_err(|_| SearcherError::EnvVarNotSet("ATLASSIAN_OAUTH_CLOUD_ID".to_string()))?;
        let mut request = self.client.request(
            method,
            format!(
                "https://api.atlassian.com/jira/forms/cloud/{}{}",
                cloud_id,
                if endpoint.starts_with('/') {
                    endpoint.to_string()
                } else {
                    format!("/{}", endpoint)
                }
            ),
        );
        if let Some(body) = body {
            request = request.json(&body);
        }
        send_json(request).await
    }
}

fn field_match_score(field: &Value, keyword: &str) -> usize {
    let mut candidates = Vec::new();
    if let Some(id) = field.get("id").and_then(Value::as_str) {
        candidates.push(id.to_ascii_lowercase());
    }
    if let Some(key) = field.get("key").and_then(Value::as_str) {
        candidates.push(key.to_ascii_lowercase());
    }
    if let Some(name) = field.get("name").and_then(Value::as_str) {
        candidates.push(name.to_ascii_lowercase());
    }
    if let Some(clause_names) = field.get("clauseNames").and_then(Value::as_array) {
        for clause_name in clause_names {
            if let Some(clause_name) = clause_name.as_str() {
                candidates.push(clause_name.to_ascii_lowercase());
            }
        }
    }

    candidates
        .into_iter()
        .map(|candidate| {
            if candidate == keyword {
                300
            } else if candidate.contains(keyword) {
                200 + keyword.len()
            } else {
                shared_char_score(&candidate, keyword)
            }
        })
        .max()
        .unwrap_or(0)
}

fn shared_char_score(candidate: &str, keyword: &str) -> usize {
    keyword.chars().filter(|ch| candidate.contains(*ch)).count()
}

fn option_matches_keyword(option: &Value, keyword: &str) -> bool {
    extract_option_value(option)
        .map(|value| value.to_ascii_lowercase().contains(keyword))
        .unwrap_or(false)
        || option
            .get("children")
            .and_then(Value::as_array)
            .map(|children| {
                children
                    .iter()
                    .any(|child| option_matches_keyword(child, keyword))
            })
            .unwrap_or(false)
}

fn extract_option_value(option: &Value) -> Option<String> {
    option
        .get("value")
        .and_then(Value::as_str)
        .map(|value| value.to_string())
        .or_else(|| {
            option
                .get("name")
                .and_then(Value::as_str)
                .map(|value| value.to_string())
        })
}

fn quote_jql_literal(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn pick_best_user_match(users: &[Value], identifier: &str) -> Option<Value> {
    let identifier = identifier.trim();
    let normalized = identifier.to_ascii_lowercase();

    users
        .iter()
        .find_map(|user| {
            let account_id_matches = user
                .get("accountId")
                .and_then(Value::as_str)
                .map(|value| value.eq_ignore_ascii_case(identifier))
                .unwrap_or(false);
            let display_name_matches = user
                .get("displayName")
                .and_then(Value::as_str)
                .map(|value| value.eq_ignore_ascii_case(identifier))
                .unwrap_or(false);
            let email_matches = user
                .get("emailAddress")
                .and_then(Value::as_str)
                .map(|value| value.eq_ignore_ascii_case(identifier))
                .unwrap_or(false);

            (account_id_matches || display_name_matches || email_matches).then(|| user.clone())
        })
        .or_else(|| {
            users.iter().find_map(|user| {
                let haystack = ["accountId", "displayName", "emailAddress", "name"]
                    .into_iter()
                    .filter_map(|key| user.get(key).and_then(Value::as_str))
                    .map(|value| value.to_ascii_lowercase())
                    .collect::<Vec<_>>()
                    .join(" ");
                haystack.contains(&normalized).then(|| user.clone())
            })
        })
}

fn parse_time_spent(time_spent: &str) -> u64 {
    let mut total_seconds = 0_u64;
    let mut current_number = String::new();

    for ch in time_spent.chars() {
        if ch.is_ascii_digit() {
            current_number.push(ch);
            continue;
        }

        let Ok(number) = current_number.parse::<u64>() else {
            continue;
        };
        let unit_seconds = match ch {
            'w' | 'W' => 7 * 24 * 60 * 60,
            'd' | 'D' => 24 * 60 * 60,
            'h' | 'H' => 60 * 60,
            'm' | 'M' => 60,
            's' | 'S' => 1,
            _ => {
                current_number.clear();
                continue;
            }
        };
        total_seconds += number * unit_seconds;
        current_number.clear();
    }

    if total_seconds == 0 {
        time_spent.trim().parse::<u64>().unwrap_or(60)
    } else {
        total_seconds
    }
}

pub struct ConfluenceClient {
    client: reqwest::Client,
    base_url: String,
    is_cloud: bool,
    spaces_filter: Option<String>,
}

impl ConfluenceClient {
    pub fn new(
        base_url: String,
        username: Option<String>,
        api_token: Option<String>,
        personal_token: Option<String>,
        ssl_verify: bool,
        spaces_filter: Option<String>,
    ) -> Result<Self, SearcherError> {
        let base_url = normalize_base_url(&base_url);
        let is_cloud = is_cloud_url(&base_url);
        let auth = match personal_token {
            Some(token) if !token.trim().is_empty() => AtlassianAuth::Bearer { token },
            _ => {
                let username = username.ok_or_else(|| {
                    SearcherError::ApiError(
                        "Confluence requires either CONFLUENCE_PERSONAL_TOKEN or CONFLUENCE_USERNAME/CONFLUENCE_API_TOKEN".to_string(),
                    )
                })?;
                let token = api_token.ok_or_else(|| {
                    SearcherError::ApiError(
                        "Confluence requires either CONFLUENCE_PERSONAL_TOKEN or CONFLUENCE_USERNAME/CONFLUENCE_API_TOKEN".to_string(),
                    )
                })?;
                AtlassianAuth::Basic { username, token }
            }
        };

        Ok(Self {
            client: build_http_client(auth, ssl_verify)?,
            base_url,
            is_cloud,
            spaces_filter,
        })
    }

    pub async fn search(
        &self,
        query: &str,
        limit: Option<usize>,
        spaces_filter: Option<&str>,
    ) -> Result<Value, SearcherError> {
        let limit = limit.unwrap_or(10).clamp(1, 50);
        let simple_query = !is_likely_cql(query);
        let primary_cql = self.build_cql(query, spaces_filter, true);
        let response = match self
            .request_json(
                Method::GET,
                "content/search",
                Some(vec![
                    ("cql".to_string(), primary_cql),
                    ("limit".to_string(), limit.to_string()),
                    (
                        "expand".to_string(),
                        "space,version,history.lastUpdated".to_string(),
                    ),
                ]),
                None,
            )
            .await
        {
            Ok(value) => value,
            Err(error) if simple_query => {
                let fallback_cql = self.build_cql(query, spaces_filter, false);
                self.request_json(
                    Method::GET,
                    "content/search",
                    Some(vec![
                        ("cql".to_string(), fallback_cql),
                        ("limit".to_string(), limit.to_string()),
                        (
                            "expand".to_string(),
                            "space,version,history.lastUpdated".to_string(),
                        ),
                    ]),
                    None,
                )
                .await
                .map_err(|_| error)?
            }
            Err(error) => return Err(error),
        };

        let results = response
            .get("results")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|item| simplify_search_result(&self.base_url, item))
            .collect();
        Ok(Value::Array(results))
    }

    pub async fn get_page(
        &self,
        page_id: Option<&str>,
        title: Option<&str>,
        space_key: Option<&str>,
        include_metadata: bool,
        convert_to_markdown: bool,
    ) -> Result<Value, SearcherError> {
        let page = match page_id.filter(|value| !value.trim().is_empty()) {
            Some(page_id) => self.fetch_page_by_id(page_id, None).await?,
            None => {
                let title = title.ok_or_else(|| {
                    SearcherError::ApiError(
                        "Either page_id or title + space_key must be provided".to_string(),
                    )
                })?;
                let space_key = space_key.ok_or_else(|| {
                    SearcherError::ApiError(
                        "Either page_id or title + space_key must be provided".to_string(),
                    )
                })?;
                self.fetch_page_by_title(space_key, title).await?
            }
        };

        Ok(simplify_page(page, include_metadata, convert_to_markdown))
    }

    pub async fn get_page_children(
        &self,
        parent_id: &str,
        expand: Option<&str>,
        limit: Option<usize>,
        include_content: Option<bool>,
        convert_to_markdown: Option<bool>,
        start: Option<usize>,
        _include_folders: Option<bool>,
    ) -> Result<Value, SearcherError> {
        let expand = expand.unwrap_or("version,space");
        let query = vec![
            ("expand".to_string(), expand.to_string()),
            (
                "limit".to_string(),
                limit.unwrap_or(25).clamp(1, 100).to_string(),
            ),
            ("start".to_string(), start.unwrap_or(0).to_string()),
        ];
        let response = self
            .request_json(
                Method::GET,
                &format!("content/{}/child/page", parent_id),
                Some(query),
                None,
            )
            .await?;

        let include_content = include_content.unwrap_or(false);
        let convert_to_markdown = convert_to_markdown.unwrap_or(true);
        let results = response
            .get("results")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|page| simplify_page(page, include_content, convert_to_markdown))
            .collect();
        Ok(Value::Array(results))
    }

    pub async fn get_comments(&self, page_id: &str) -> Result<Value, SearcherError> {
        let response = self
            .request_json(
                Method::GET,
                &format!("content/{}/child/comment", page_id),
                Some(vec![(
                    "expand".to_string(),
                    "body.view,version,container,history".to_string(),
                )]),
                None,
            )
            .await?;

        let comments = response
            .get("results")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(simplify_comment)
            .collect();
        Ok(Value::Array(comments))
    }

    pub async fn get_labels(&self, page_id: &str) -> Result<Value, SearcherError> {
        let response = self
            .request_json(
                Method::GET,
                &format!("content/{}/label", page_id),
                Some(vec![("limit".to_string(), "200".to_string())]),
                None,
            )
            .await?;
        Ok(response.get("results").cloned().unwrap_or(response))
    }

    pub async fn add_label(&self, page_id: &str, name: &str) -> Result<Value, SearcherError> {
        self.request_json(
            Method::POST,
            &format!("content/{}/label", page_id),
            None,
            Some(json!([{ "prefix": "global", "name": name }])),
        )
        .await
    }

    pub async fn create_page(
        &self,
        space_key: &str,
        title: &str,
        content: &str,
        parent_id: Option<&str>,
        content_format: Option<&str>,
        _enable_heading_anchors: Option<bool>,
        include_content: Option<bool>,
        _emoji: Option<&str>,
    ) -> Result<Value, SearcherError> {
        let mut body = Map::new();
        body.insert("type".to_string(), Value::String("page".to_string()));
        body.insert("title".to_string(), Value::String(title.to_string()));
        body.insert("space".to_string(), json!({ "key": space_key }));
        body.insert(
            "body".to_string(),
            json!({
                "storage": {
                    "value": self.to_storage_content(content, content_format),
                    "representation": "storage",
                }
            }),
        );
        if let Some(parent_id) = parent_id.filter(|value| !value.trim().is_empty()) {
            body.insert("ancestors".to_string(), json!([{ "id": parent_id }]));
        }

        let response = self
            .request_json(Method::POST, "content", None, Some(Value::Object(body)))
            .await?;
        let page_id = response
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if page_id.is_empty() {
            return Ok(response);
        }
        self.get_page(
            Some(&page_id),
            None,
            None,
            include_content.unwrap_or(false),
            true,
        )
        .await
    }

    pub async fn update_page(
        &self,
        page_id: &str,
        title: &str,
        content: &str,
        is_minor_edit: Option<bool>,
        version_comment: Option<&str>,
        parent_id: Option<&str>,
        content_format: Option<&str>,
        _enable_heading_anchors: Option<bool>,
        include_content: Option<bool>,
        _emoji: Option<&str>,
    ) -> Result<Value, SearcherError> {
        let current = self
            .fetch_page_by_id(page_id, Some("version,space"))
            .await?;
        let current_version = current
            .get("version")
            .and_then(|version| version.get("number"))
            .and_then(Value::as_u64)
            .unwrap_or(1);
        let space_key = current
            .get("space")
            .and_then(|space| space.get("key"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();

        let mut body = Map::new();
        body.insert("id".to_string(), Value::String(page_id.to_string()));
        body.insert("type".to_string(), Value::String("page".to_string()));
        body.insert("title".to_string(), Value::String(title.to_string()));
        if !space_key.is_empty() {
            body.insert("space".to_string(), json!({ "key": space_key }));
        }
        body.insert(
            "version".to_string(),
            json!({
                "number": current_version + 1,
                "minorEdit": is_minor_edit.unwrap_or(false),
                "message": version_comment.unwrap_or(""),
            }),
        );
        body.insert(
            "body".to_string(),
            json!({
                "storage": {
                    "value": self.to_storage_content(content, content_format),
                    "representation": "storage",
                }
            }),
        );
        if let Some(parent_id) = parent_id.filter(|value| !value.trim().is_empty()) {
            body.insert("ancestors".to_string(), json!([{ "id": parent_id }]));
        }

        self.request_json(
            Method::PUT,
            &format!("content/{}", page_id),
            None,
            Some(Value::Object(body)),
        )
        .await?;

        self.get_page(
            Some(page_id),
            None,
            None,
            include_content.unwrap_or(false),
            true,
        )
        .await
    }

    pub async fn delete_page(&self, page_id: &str) -> Result<Value, SearcherError> {
        self.request_empty(Method::DELETE, &format!("content/{}", page_id), None, None)
            .await?;
        Ok(json!({ "success": true, "page_id": page_id }))
    }

    pub async fn add_comment(&self, page_id: &str, body: &str) -> Result<Value, SearcherError> {
        self.request_json(
            Method::POST,
            "content",
            None,
            Some(json!({
                "type": "comment",
                "container": {
                    "id": page_id,
                    "type": "page",
                },
                "body": {
                    "storage": {
                        "value": self.to_storage_content(body, Some("markdown")),
                        "representation": "storage",
                    }
                }
            })),
        )
        .await
    }

    pub async fn reply_to_comment(
        &self,
        comment_id: &str,
        body: &str,
    ) -> Result<Value, SearcherError> {
        self.request_json(
            Method::POST,
            "content",
            None,
            Some(json!({
                "type": "comment",
                "container": {
                    "id": comment_id,
                    "type": "comment",
                },
                "body": {
                    "storage": {
                        "value": self.to_storage_content(body, Some("markdown")),
                        "representation": "storage",
                    }
                }
            })),
        )
        .await
    }

    pub async fn search_user(
        &self,
        query: &str,
        limit: Option<usize>,
        group_name: Option<&str>,
    ) -> Result<Value, SearcherError> {
        let limit = limit.unwrap_or(10).clamp(1, 50);
        if self.is_cloud {
            let response = self
                .request_json(
                    Method::GET,
                    "search/user",
                    Some(vec![
                        (
                            "cql".to_string(),
                            format!("user.fullname ~ \"{}\"", escape_cql_value(query)),
                        ),
                        ("limit".to_string(), limit.to_string()),
                    ]),
                    None,
                )
                .await?;
            return Ok(response.get("results").cloned().unwrap_or(response));
        }

        let group_name = group_name.unwrap_or("confluence-users");
        let response = self
            .request_json(
                Method::GET,
                &format!("group/{}/member", group_name),
                Some(vec![
                    ("start".to_string(), "0".to_string()),
                    ("limit".to_string(), "200".to_string()),
                ]),
                None,
            )
            .await?;
        let normalized = query.to_ascii_lowercase();
        let mut users = response
            .get("results")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|user| {
                ["displayName", "username", "email"]
                    .into_iter()
                    .filter_map(|key| user.get(key).and_then(Value::as_str))
                    .any(|value| value.to_ascii_lowercase().contains(&normalized))
            })
            .collect::<Vec<_>>();
        users.truncate(limit);
        Ok(Value::Array(users))
    }

    pub async fn get_page_history(
        &self,
        page_id: &str,
        version: u32,
        convert_to_markdown: bool,
    ) -> Result<Value, SearcherError> {
        let page = self
            .request_json(
                Method::GET,
                &format!("content/{}", page_id),
                Some(vec![
                    ("status".to_string(), "historical".to_string()),
                    ("version".to_string(), version.to_string()),
                    (
                        "expand".to_string(),
                        "body.storage,body.view,version,space,history".to_string(),
                    ),
                ]),
                None,
            )
            .await?;
        Ok(simplify_page(page, true, convert_to_markdown))
    }

    pub async fn get_page_views(
        &self,
        page_id: &str,
        include_title: bool,
    ) -> Result<Value, SearcherError> {
        if !self.is_cloud {
            return Err(SearcherError::ApiError(
                "Confluence page views are only available on Confluence Cloud".to_string(),
            ));
        }

        let views = send_json(self.client.request(
            Method::GET,
            format!(
                "{}/rest/api/analytics/content/{}/views",
                self.base_url, page_id
            ),
        ))
        .await?;

        if include_title {
            let title = self
                .fetch_page_by_id(page_id, Some("title"))
                .await?
                .get("title")
                .cloned()
                .unwrap_or(Value::Null);
            Ok(json!({
                "page_id": page_id,
                "page_title": title,
                "views": views,
            }))
        } else {
            Ok(json!({
                "page_id": page_id,
                "views": views,
            }))
        }
    }

    pub async fn get_space_page_tree(
        &self,
        space_key: &str,
        limit: Option<usize>,
    ) -> Result<Value, SearcherError> {
        let limit = limit.unwrap_or(100).clamp(1, 1000);
        let page_size = 200usize;
        let mut start = 0usize;
        let mut all_pages = Vec::new();
        let mut has_more = false;

        while all_pages.len() < limit {
            let fetch_limit = page_size.min(limit - all_pages.len());
            let response = self
                .request_json(
                    Method::GET,
                    "content",
                    Some(vec![
                        ("spaceKey".to_string(), space_key.to_string()),
                        ("type".to_string(), "page".to_string()),
                        ("expand".to_string(), "ancestors".to_string()),
                        ("start".to_string(), start.to_string()),
                        ("limit".to_string(), fetch_limit.to_string()),
                    ]),
                    None,
                )
                .await?;

            let batch = response
                .get("results")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let next_link = response
                .get("_links")
                .and_then(|value| value.get("next"))
                .and_then(Value::as_str);

            if batch.is_empty() {
                has_more = false;
                break;
            }

            start += batch.len();
            all_pages.extend(batch);
            has_more = all_pages.len() >= limit && next_link.is_some();

            if next_link.is_none() || all_pages.len() >= limit {
                break;
            }
        }

        let mut pages = all_pages
            .into_iter()
            .map(|page| {
                let ancestors = page
                    .get("ancestors")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let parent_id = ancestors
                    .last()
                    .and_then(|value| value.get("id"))
                    .cloned()
                    .unwrap_or(Value::Null);
                let position = page
                    .get("extensions")
                    .and_then(|value| value.get("position"))
                    .cloned()
                    .unwrap_or(Value::Null);

                json!({
                    "id": page.get("id").cloned().unwrap_or(Value::Null),
                    "title": page.get("title").cloned().unwrap_or(Value::String("Untitled".to_string())),
                    "parent_id": parent_id,
                    "position": position,
                    "depth": ancestors.len(),
                })
            })
            .collect::<Vec<_>>();

        pages.sort_by(|left, right| {
            let left_depth = left.get("depth").and_then(Value::as_u64).unwrap_or(0);
            let right_depth = right.get("depth").and_then(Value::as_u64).unwrap_or(0);
            let left_position = left
                .get("position")
                .and_then(Value::as_i64)
                .unwrap_or(i64::MAX);
            let right_position = right
                .get("position")
                .and_then(Value::as_i64)
                .unwrap_or(i64::MAX);
            let left_title = left
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let right_title = right
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or_default();
            (left_depth, left_position, left_title).cmp(&(right_depth, right_position, right_title))
        });

        let mut result = json!({
            "space_key": space_key,
            "total_pages": pages.len(),
            "has_more": has_more,
            "pages": pages,
        });
        if has_more {
            result["next_start"] = json!(start);
        }
        Ok(result)
    }

    pub async fn move_page(
        &self,
        page_id: &str,
        target_parent_id: Option<&str>,
        target_space_key: Option<&str>,
        position: Option<&str>,
    ) -> Result<Value, SearcherError> {
        let position = position.unwrap_or("append");
        if target_parent_id.is_none() && target_space_key.is_none() {
            return Err(SearcherError::ApiError(
                "At least one of target_parent_id or target_space_key must be provided".to_string(),
            ));
        }

        if let Some(target_space_key) = target_space_key.filter(|value| !value.trim().is_empty()) {
            let current = self
                .fetch_page_by_id(page_id, Some("body.storage,version,title"))
                .await?;
            let current_version = current
                .get("version")
                .and_then(|version| version.get("number"))
                .and_then(Value::as_u64)
                .unwrap_or(1);
            let title = current
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let content = current
                .get("body")
                .and_then(|body| body.get("storage"))
                .and_then(|storage| storage.get("value"))
                .and_then(Value::as_str)
                .unwrap_or_default();

            let mut body = Map::new();
            body.insert("id".to_string(), Value::String(page_id.to_string()));
            body.insert("type".to_string(), Value::String("page".to_string()));
            body.insert("title".to_string(), Value::String(title.to_string()));
            body.insert("space".to_string(), json!({ "key": target_space_key }));
            body.insert(
                "version".to_string(),
                json!({
                    "number": current_version + 1,
                }),
            );
            body.insert(
                "body".to_string(),
                json!({
                    "storage": {
                        "value": content,
                        "representation": "storage",
                    }
                }),
            );
            if let Some(target_parent_id) =
                target_parent_id.filter(|value| !value.trim().is_empty())
            {
                body.insert("ancestors".to_string(), json!([{ "id": target_parent_id }]));
            }

            self.request_json(
                Method::PUT,
                &format!("content/{}", page_id),
                None,
                Some(Value::Object(body)),
            )
            .await?;
        } else {
            let api_position = match position {
                "append" => "append",
                "above" => "before",
                "below" => "after",
                other => {
                    return Err(SearcherError::ApiError(format!(
                        "Unsupported move position '{}'; expected append, above, or below",
                        other
                    )));
                }
            };
            let path = match target_parent_id.filter(|value| !value.trim().is_empty()) {
                Some(target_id) => {
                    format!("content/{}/move/{}/{}", page_id, api_position, target_id)
                }
                None => format!("content/{}/move/{}", page_id, api_position),
            };
            self.request_empty(Method::PUT, &path, None, None).await?;
        }

        self.get_page(Some(page_id), None, None, true, true).await
    }

    pub async fn get_page_diff(
        &self,
        page_id: &str,
        from_version: u32,
        to_version: u32,
    ) -> Result<Value, SearcherError> {
        let from_page = self.get_page_history(page_id, from_version, true).await?;
        let to_page = self.get_page_history(page_id, to_version, true).await?;
        let from_content = from_page
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let to_content = to_page
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let title = to_page
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or_default();

        Ok(json!({
            "page_id": page_id,
            "title": title,
            "from_version": from_version,
            "to_version": to_version,
            "diff": build_unified_diff(
                &format!("v{}", from_version),
                &format!("v{}", to_version),
                from_content,
                to_content,
            ),
        }))
    }

    pub async fn upload_attachment(
        &self,
        content_id: &str,
        file_path: &str,
        comment: Option<&str>,
        minor_edit: Option<bool>,
    ) -> Result<Value, SearcherError> {
        let path = resolve_local_path(file_path)?;
        let filename = path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| SearcherError::ApiError(format!("Invalid file path: {}", file_path)))?
            .to_string();
        let bytes = fs::read(&path).await?;
        let size = bytes.len() as u64;
        let part = Part::bytes(bytes).file_name(filename.clone());
        let mut form = Form::new().part("file", part);
        if let Some(comment) = comment.filter(|value| !value.trim().is_empty()) {
            form = form.text("comment", comment.to_string());
        }
        form = form.text(
            "minorEdit",
            minor_edit.unwrap_or(false).to_string().to_ascii_lowercase(),
        );

        let response = send_json(
            self.client
                .request(
                    Method::PUT,
                    format!(
                        "{}/rest/api/content/{}/child/attachment",
                        self.base_url, content_id
                    ),
                )
                .header("X-Atlassian-Token", "nocheck")
                .multipart(form),
        )
        .await?;

        let attachment = response
            .get("results")
            .and_then(Value::as_array)
            .and_then(|results| results.first())
            .cloned()
            .unwrap_or(response);

        Ok(json!({
            "success": true,
            "content_id": content_id,
            "filename": filename,
            "size": size,
            "id": attachment.get("id").cloned().unwrap_or(Value::Null),
            "attachment": attachment,
        }))
    }

    pub async fn upload_attachments(
        &self,
        content_id: &str,
        file_paths: &[String],
        comment: Option<&str>,
        minor_edit: Option<bool>,
    ) -> Result<Value, SearcherError> {
        let mut uploaded = Vec::new();
        let mut failed = Vec::new();

        for file_path in file_paths {
            match self
                .upload_attachment(content_id, file_path, comment, minor_edit)
                .await
            {
                Ok(value) => uploaded.push(value),
                Err(error) => failed.push(json!({
                    "filename": Path::new(file_path)
                        .file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or(file_path),
                    "error": error.to_string(),
                })),
            }
        }

        Ok(json!({
            "success": true,
            "content_id": content_id,
            "total": file_paths.len(),
            "uploaded": uploaded,
            "failed": failed,
        }))
    }

    pub async fn get_attachments(
        &self,
        content_id: &str,
        start: Option<usize>,
        limit: Option<usize>,
        filename: Option<&str>,
        media_type: Option<&str>,
    ) -> Result<Value, SearcherError> {
        let start = start.unwrap_or(0);
        let limit = limit.unwrap_or(50).clamp(1, 100);
        let response = self
            .request_json(
                Method::GET,
                &format!("content/{}/child/attachment", content_id),
                Some(vec![
                    ("start".to_string(), start.to_string()),
                    ("limit".to_string(), limit.to_string()),
                ]),
                None,
            )
            .await?;

        let mut attachments = response
            .get("results")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        if filename.is_some() || media_type.is_some() {
            attachments.retain(|attachment| {
                let title_matches = filename.map(|expected| {
                    attachment
                        .get("title")
                        .and_then(Value::as_str)
                        .map(|actual| actual == expected)
                        .unwrap_or(false)
                });
                let media_matches = media_type.map(|expected| {
                    attachment_media_type(attachment)
                        .map(|actual| actual == expected)
                        .unwrap_or(false)
                });
                title_matches.unwrap_or(true) && media_matches.unwrap_or(true)
            });
        }

        Ok(json!({
            "success": true,
            "content_id": content_id,
            "attachments": attachments,
            "total": attachments.len(),
            "start": start,
            "limit": limit,
        }))
    }

    pub async fn download_attachment(&self, attachment_id: &str) -> Result<Value, SearcherError> {
        let attachment = self
            .request_json(
                Method::GET,
                &format!("content/{}", attachment_id),
                None,
                None,
            )
            .await?;
        let download_url = attachment
            .get("_links")
            .and_then(|links| links.get("download"))
            .and_then(Value::as_str)
            .map(|url| resolve_relative_url(&self.base_url, url))
            .ok_or_else(|| {
                SearcherError::ApiError(format!(
                    "Could not find download URL for attachment {}",
                    attachment_id
                ))
            })?;
        let bytes = self
            .request_absolute_bytes(Method::GET, &download_url)
            .await?;
        let filename = attachment
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or(attachment_id);
        let mut result = attachment.clone();
        result["data_base64"] =
            Value::String(base64::engine::general_purpose::STANDARD.encode(bytes));
        result["download_url"] = Value::String(download_url);
        result["filename"] = Value::String(filename.to_string());
        Ok(result)
    }

    pub async fn download_content_attachments(
        &self,
        content_id: &str,
    ) -> Result<Value, SearcherError> {
        let response = self
            .get_attachments(content_id, Some(0), Some(200), None, None)
            .await?;
        let attachments = response
            .get("attachments")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let mut downloaded = Vec::new();
        let mut failed = Vec::new();
        for attachment in attachments {
            let attachment_id = attachment
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let filename = attachment
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or(&attachment_id)
                .to_string();

            match self.download_attachment(&attachment_id).await {
                Ok(value) => downloaded.push(value),
                Err(error) => failed.push(json!({
                    "filename": filename,
                    "error": error.to_string(),
                })),
            }
        }

        Ok(json!({
            "success": true,
            "content_id": content_id,
            "total": response.get("total").cloned().unwrap_or(json!(0)),
            "downloaded": downloaded,
            "failed": failed,
        }))
    }

    pub async fn delete_attachment(&self, attachment_id: &str) -> Result<Value, SearcherError> {
        self.request_empty(
            Method::DELETE,
            &format!("content/{}", attachment_id),
            None,
            None,
        )
        .await?;
        Ok(json!({
            "success": true,
            "attachment_id": attachment_id,
            "message": "Attachment deleted successfully",
        }))
    }

    pub async fn get_page_images(&self, content_id: &str) -> Result<Value, SearcherError> {
        let attachments = self
            .get_attachments(content_id, Some(0), Some(200), None, None)
            .await?
            .get("attachments")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let mut images = Vec::new();
        let mut failed = Vec::new();
        for attachment in attachments {
            let media_type = attachment_media_type(&attachment);
            let filename = attachment
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let Some(image_mime) = detect_image_mime(media_type, Some(filename)) else {
                continue;
            };

            let attachment_id = attachment
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            match self.download_attachment(&attachment_id).await {
                Ok(mut value) => {
                    value["mimeType"] = Value::String(image_mime);
                    images.push(value);
                }
                Err(error) => failed.push(json!({
                    "filename": filename,
                    "error": error.to_string(),
                })),
            }
        }

        Ok(json!({
            "success": true,
            "content_id": content_id,
            "total_images": images.len() + failed.len(),
            "images": images,
            "failed": failed,
        }))
    }

    fn build_cql(
        &self,
        query: &str,
        spaces_filter: Option<&str>,
        prefer_site_search: bool,
    ) -> String {
        let mut cql = if is_likely_cql(query) {
            query.to_string()
        } else {
            let search_field = if prefer_site_search {
                "siteSearch"
            } else {
                "text"
            };
            format!("{} ~ \"{}\"", search_field, escape_cql_value(query))
        };

        let filter = spaces_filter
            .and_then(|value| {
                let trimmed = value.trim();
                (!trimmed.is_empty()).then_some(trimmed)
            })
            .or(self.spaces_filter.as_deref());

        if let Some(filter) = filter {
            let clauses = filter
                .split(',')
                .map(|item| item.trim())
                .filter(|item| !item.is_empty())
                .map(|space| format!("space = \"{}\"", escape_cql_value(space)))
                .collect::<Vec<_>>();
            if !clauses.is_empty() && !cql.to_ascii_lowercase().contains("space =") {
                cql = format!("({}) AND ({})", cql, clauses.join(" OR "));
            }
        }

        cql
    }

    fn to_storage_content(&self, content: &str, content_format: Option<&str>) -> String {
        match content_format
            .unwrap_or("markdown")
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "storage" | "html" => content.to_string(),
            "wiki" => format!("<pre>{}</pre>", escape_html(content)),
            _ => markdown_to_html(content),
        }
    }

    async fn fetch_page_by_id(
        &self,
        page_id: &str,
        expand: Option<&str>,
    ) -> Result<Value, SearcherError> {
        let expand = expand
            .unwrap_or("body.storage,body.view,version,space,history,ancestors,metadata.labels");
        self.request_json(
            Method::GET,
            &format!("content/{}", page_id),
            Some(vec![("expand".to_string(), expand.to_string())]),
            None,
        )
        .await
    }

    async fn fetch_page_by_title(
        &self,
        space_key: &str,
        title: &str,
    ) -> Result<Value, SearcherError> {
        let response = self
            .request_json(
                Method::GET,
                "content",
                Some(vec![
                    ("title".to_string(), title.to_string()),
                    ("spaceKey".to_string(), space_key.to_string()),
                    (
                        "expand".to_string(),
                        "body.storage,body.view,version,space,history,ancestors,metadata.labels"
                            .to_string(),
                    ),
                ]),
                None,
            )
            .await?;

        response
            .get("results")
            .and_then(Value::as_array)
            .and_then(|results| results.first())
            .cloned()
            .ok_or_else(|| {
                SearcherError::ApiError(format!(
                    "Confluence page '{}' not found in space '{}'",
                    title, space_key
                ))
            })
    }

    fn api_url(&self, path: &str) -> String {
        format!(
            "{}/rest/api/{}",
            self.base_url,
            path.trim_start_matches('/')
        )
    }

    async fn request_json(
        &self,
        method: Method,
        path: &str,
        query: Option<Vec<(String, String)>>,
        body: Option<Value>,
    ) -> Result<Value, SearcherError> {
        let mut request = self.client.request(method, self.api_url(path));
        if let Some(query) = query.filter(|items| !items.is_empty()) {
            request = request.query(&query);
        }
        if let Some(body) = body {
            request = request.json(&body);
        }
        send_json(request).await
    }

    async fn request_empty(
        &self,
        method: Method,
        path: &str,
        query: Option<Vec<(String, String)>>,
        body: Option<Value>,
    ) -> Result<(), SearcherError> {
        let mut request = self.client.request(method, self.api_url(path));
        if let Some(query) = query.filter(|items| !items.is_empty()) {
            request = request.query(&query);
        }
        if let Some(body) = body {
            request = request.json(&body);
        }
        send_empty(request).await
    }

    async fn request_absolute_bytes(
        &self,
        method: Method,
        url: &str,
    ) -> Result<Vec<u8>, SearcherError> {
        send_bytes(
            self.client
                .request(method, url)
                .header(ACCEPT, HeaderValue::from_static("*/*")),
        )
        .await
    }
}

fn is_likely_cql(query: &str) -> bool {
    [" = ", " ~ ", " > ", " < ", " AND ", " OR ", "currentUser()"]
        .into_iter()
        .any(|needle| query.contains(needle))
}

fn escape_cql_value(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn simplify_search_result(base_url: &str, item: Value) -> Value {
    let content = item.get("content").cloned().unwrap_or_else(|| item.clone());
    let page = simplify_page(content, true, true);
    json!({
        "page": page,
        "excerpt": item.get("excerpt").map(|value| {
            value.as_str()
                .map(html_to_text_lossy)
                .map(Value::String)
                .unwrap_or_else(|| value.clone())
        }).unwrap_or(Value::Null),
        "url": item
            .get("url")
            .and_then(Value::as_str)
            .map(|url| format!("{}{}", base_url, url))
            .unwrap_or_default(),
    })
}

fn simplify_page(page: Value, include_metadata: bool, convert_to_markdown: bool) -> Value {
    let content = extract_page_content(&page, convert_to_markdown);
    let mut result = Map::new();
    result.insert(
        "id".to_string(),
        page.get("id").cloned().unwrap_or(Value::Null),
    );
    result.insert(
        "title".to_string(),
        page.get("title").cloned().unwrap_or(Value::Null),
    );
    result.insert(
        "type".to_string(),
        page.get("type")
            .cloned()
            .unwrap_or(Value::String("page".to_string())),
    );
    result.insert("content".to_string(), content);

    if include_metadata {
        result.insert(
            "space".to_string(),
            page.get("space").cloned().unwrap_or(Value::Null),
        );
        result.insert(
            "version".to_string(),
            page.get("version").cloned().unwrap_or(Value::Null),
        );
        result.insert(
            "history".to_string(),
            page.get("history").cloned().unwrap_or(Value::Null),
        );
        result.insert(
            "ancestors".to_string(),
            page.get("ancestors")
                .cloned()
                .unwrap_or(Value::Array(vec![])),
        );
        let labels = page
            .get("metadata")
            .and_then(|metadata| metadata.get("labels"))
            .and_then(|labels| labels.get("results"))
            .cloned()
            .unwrap_or(Value::Array(vec![]));
        result.insert("labels".to_string(), labels);
    }

    Value::Object(result)
}

fn extract_page_content(page: &Value, convert_to_markdown: bool) -> Value {
    let raw = if convert_to_markdown {
        page.get("body")
            .and_then(|body| body.get("view"))
            .and_then(|view| view.get("value"))
            .and_then(Value::as_str)
            .or_else(|| {
                page.get("body")
                    .and_then(|body| body.get("storage"))
                    .and_then(|storage| storage.get("value"))
                    .and_then(Value::as_str)
            })
            .map(html_to_text_lossy)
            .unwrap_or_default()
    } else {
        page.get("body")
            .and_then(|body| body.get("storage"))
            .and_then(|storage| storage.get("value"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };

    Value::String(raw)
}

fn simplify_comment(comment: Value) -> Value {
    json!({
        "id": comment.get("id").cloned().unwrap_or(Value::Null),
        "title": comment.get("title").cloned().unwrap_or(Value::Null),
        "version": comment.get("version").cloned().unwrap_or(Value::Null),
        "history": comment.get("history").cloned().unwrap_or(Value::Null),
        "body": comment
            .get("body")
            .and_then(|body| body.get("view"))
            .and_then(|view| view.get("value"))
            .and_then(Value::as_str)
            .map(html_to_text_lossy)
            .unwrap_or_default(),
        "container": comment.get("container").cloned().unwrap_or(Value::Null),
    })
}

fn parse_jira_datetime(value: &str) -> Option<DateTime<FixedOffset>> {
    DateTime::parse_from_rfc3339(value).ok().or_else(|| {
        NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .ok()
            .and_then(|date| {
                FixedOffset::east_opt(0)?
                    .from_local_datetime(&date.and_hms_opt(0, 0, 0)?)
                    .single()
            })
    })
}

fn summarize_status_changes(status_changes: &[Value]) -> Vec<Value> {
    let mut aggregates = BTreeMap::<String, (i64, u64)>::new();
    for change in status_changes {
        let status = change
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if status.is_empty() {
            continue;
        }
        let entry = aggregates.entry(status).or_insert((0, 0));
        entry.1 += 1;
        if let Some(duration_minutes) = change.get("duration_minutes").and_then(Value::as_i64) {
            entry.0 += duration_minutes;
        }
    }

    let mut summaries = aggregates
        .into_iter()
        .map(|(status, (total_minutes, visit_count))| {
            json!({
                "status": status,
                "total_duration_minutes": total_minutes,
                "total_duration_formatted": format_duration_minutes(total_minutes),
                "visit_count": visit_count,
            })
        })
        .collect::<Vec<_>>();
    summaries.sort_by_key(|item| {
        Reverse(
            item.get("total_duration_minutes")
                .and_then(Value::as_i64)
                .unwrap_or(0),
        )
    });
    summaries
}

fn format_duration_minutes(minutes: i64) -> String {
    let total_minutes = minutes.max(0);
    let days = total_minutes / (24 * 60);
    let hours = (total_minutes % (24 * 60)) / 60;
    let mins = total_minutes % 60;
    let mut parts = Vec::new();
    if days > 0 {
        parts.push(format!("{}d", days));
    }
    if hours > 0 {
        parts.push(format!("{}h", hours));
    }
    if mins > 0 || parts.is_empty() {
        parts.push(format!("{}m", mins));
    }
    parts.join(" ")
}

fn merge_development_info(target: &mut Value, incoming: &Value) {
    if target.get("error").is_none() {
        if let Some(error) = incoming.get("error").cloned() {
            target["error"] = error;
        }
    }

    for key in [
        "detail",
        "pullRequests",
        "branches",
        "commits",
        "repositories",
    ] {
        let target_array = target
            .get_mut(key)
            .and_then(Value::as_array_mut)
            .expect("development info target array must exist");
        let mut seen = target_array
            .iter()
            .map(value_fingerprint)
            .collect::<BTreeSet<_>>();
        if let Some(incoming_array) = incoming.get(key).and_then(Value::as_array) {
            for item in incoming_array {
                let fingerprint = value_fingerprint(item);
                if seen.insert(fingerprint) {
                    target_array.push(item.clone());
                }
            }
        }
    }
}

fn value_fingerprint(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "<unserializable>".to_string())
}

fn build_unified_diff(from_label: &str, to_label: &str, from: &str, to: &str) -> String {
    let from_lines = from.lines().map(str::to_string).collect::<Vec<_>>();
    let to_lines = to.lines().map(str::to_string).collect::<Vec<_>>();
    let diff_lines = line_diff(&from_lines, &to_lines);
    let mut output = vec![
        format!("--- {}", from_label),
        format!("+++ {}", to_label),
        format!(
            "@@ -1,{} +1,{} @@",
            from_lines.len().max(1),
            to_lines.len().max(1)
        ),
    ];
    output.extend(
        diff_lines
            .into_iter()
            .map(|(prefix, line)| format!("{}{}", prefix, line)),
    );
    output.join("\n")
}

fn line_diff(from: &[String], to: &[String]) -> Vec<(char, String)> {
    let m = from.len();
    let n = to.len();
    let mut dp = vec![vec![0usize; n + 1]; m + 1];

    for i in (0..m).rev() {
        for j in (0..n).rev() {
            dp[i][j] = if from[i] == to[j] {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }

    let mut i = 0usize;
    let mut j = 0usize;
    let mut diff = Vec::new();
    while i < m && j < n {
        if from[i] == to[j] {
            diff.push((' ', from[i].clone()));
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            diff.push(('-', from[i].clone()));
            i += 1;
        } else {
            diff.push(('+', to[j].clone()));
            j += 1;
        }
    }
    while i < m {
        diff.push(('-', from[i].clone()));
        i += 1;
    }
    while j < n {
        diff.push(('+', to[j].clone()));
        j += 1;
    }
    diff
}

fn resolve_relative_url(base_url: &str, url: &str) -> String {
    if url.starts_with("http://") || url.starts_with("https://") {
        url.to_string()
    } else {
        format!(
            "{}/{}",
            base_url.trim_end_matches('/'),
            url.trim_start_matches('/')
        )
    }
}

fn resolve_local_path(file_path: &str) -> Result<PathBuf, SearcherError> {
    let path = Path::new(file_path);
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };

    if absolute.exists() {
        Ok(absolute)
    } else {
        Err(SearcherError::ApiError(format!(
            "File not found: {}",
            absolute.display()
        )))
    }
}

fn attachment_media_type(attachment: &Value) -> Option<&str> {
    attachment
        .get("extensions")
        .and_then(|value| value.get("mediaType"))
        .and_then(Value::as_str)
        .or_else(|| {
            attachment
                .get("metadata")
                .and_then(|value| value.get("mediaType"))
                .and_then(Value::as_str)
        })
        .or_else(|| attachment.get("mediaType").and_then(Value::as_str))
}

fn detect_image_mime(media_type: Option<&str>, filename: Option<&str>) -> Option<String> {
    if let Some(media_type) = media_type {
        if media_type.starts_with("image/") {
            return Some(media_type.to_string());
        }
    }

    let filename = filename?.to_ascii_lowercase();
    let ext = filename.rsplit('.').next()?;
    let mime = match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => return None,
    };
    Some(mime.to_string())
}

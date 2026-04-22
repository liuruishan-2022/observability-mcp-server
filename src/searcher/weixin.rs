use crate::searcher::SearcherError;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// 企业微信机器人客户端
pub struct WeixinClient {
    webhook_url: String,
    client: Client,
}

#[derive(Debug, Serialize, Deserialize)]
struct TextMessage {
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    mentioned_list: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mentioned_mobile_list: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct WebhookRequest {
    msgtype: String,
    text: TextMessage,
}

#[derive(Debug, Serialize, Deserialize)]
struct WebhookResponse {
    errcode: i32,
    errmsg: String,
}

impl WeixinClient {
    /// 创建新的企业微信机器人客户端
    pub fn new(webhook_url: String) -> Self {
        Self {
            webhook_url,
            client: Client::new(),
        }
    }

    /// 发送文本消息
    pub async fn send_text(
        &self,
        content: &str,
        mentioned_list: Option<Vec<String>>,
        mentioned_mobile_list: Option<Vec<String>>,
    ) -> Result<String, SearcherError> {
        let payload = WebhookRequest {
            msgtype: "text".to_string(),
            text: TextMessage {
                content: content.to_string(),
                mentioned_list,
                mentioned_mobile_list,
            },
        };

        let response = self
            .client
            .post(&self.webhook_url)
            .json(&payload)
            .send()
            .await?;

        if response.status().is_success() {
            let result: WebhookResponse = response.json().await?;
            if result.errcode == 0 {
                Ok(json!({
                    "success": true,
                    "message": "Message sent successfully",
                    "data": result
                })
                .to_string())
            } else {
                Err(SearcherError::ApiError(format!(
                    "WeChat API error: {} (errcode: {})",
                    result.errmsg, result.errcode
                )))
            }
        } else {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read response body".to_string());
            Err(SearcherError::ApiError(format!(
                "HTTP error: {} - {}",
                status.as_u16(),
                body
            )))
        }
    }

    /// 发送 Markdown 消息
    pub async fn send_markdown(&self, content: &str) -> Result<String, SearcherError> {
        let payload = json!({
            "msgtype": "markdown",
            "markdown": {
                "content": content
            }
        });

        let response = self
            .client
            .post(&self.webhook_url)
            .json(&payload)
            .send()
            .await?;

        if response.status().is_success() {
            let result: WebhookResponse = response.json().await?;
            if result.errcode == 0 {
                Ok(json!({
                    "success": true,
                    "message": "Markdown message sent successfully",
                    "data": result
                })
                .to_string())
            } else {
                Err(SearcherError::ApiError(format!(
                    "WeChat API error: {} (errcode: {})",
                    result.errmsg, result.errcode
                )))
            }
        } else {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read response body".to_string());
            Err(SearcherError::ApiError(format!(
                "HTTP error: {} - {}",
                status.as_u16(),
                body
            )))
        }
    }

    /// 发送图片消息（需要先上传图片获取 media_id）
    pub async fn send_image(&self, media_id: &str) -> Result<String, SearcherError> {
        let payload = json!({
            "msgtype": "image",
            "image": {
                "media_id": media_id
            }
        });

        let response = self
            .client
            .post(&self.webhook_url)
            .json(&payload)
            .send()
            .await?;

        if response.status().is_success() {
            let result: WebhookResponse = response.json().await?;
            if result.errcode == 0 {
                Ok(json!({
                    "success": true,
                    "message": "Image message sent successfully",
                    "data": result
                })
                .to_string())
            } else {
                Err(SearcherError::ApiError(format!(
                    "WeChat API error: {} (errcode: {})",
                    result.errmsg, result.errcode
                )))
            }
        } else {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read response body".to_string());
            Err(SearcherError::ApiError(format!(
                "HTTP error: {} - {}",
                status.as_u16(),
                body
            )))
        }
    }

    /// 发送图文消息
    pub async fn send_news(&self, articles: Vec<Article>) -> Result<String, SearcherError> {
        let payload = json!({
            "msgtype": "news",
            "news": {
                "articles": articles
            }
        });

        let response = self
            .client
            .post(&self.webhook_url)
            .json(&payload)
            .send()
            .await?;

        if response.status().is_success() {
            let result: WebhookResponse = response.json().await?;
            if result.errcode == 0 {
                Ok(json!({
                    "success": true,
                    "message": "News message sent successfully",
                    "data": result
                })
                .to_string())
            } else {
                Err(SearcherError::ApiError(format!(
                    "WeChat API error: {} (errcode: {})",
                    result.errmsg, result.errcode
                )))
            }
        } else {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read response body".to_string());
            Err(SearcherError::ApiError(format!(
                "HTTP error: {} - {}",
                status.as_u16(),
                body
            )))
        }
    }

    /// 发送文件消息
    pub async fn send_file(&self, media_id: &str) -> Result<String, SearcherError> {
        let payload = json!({
            "msgtype": "file",
            "file": {
                "media_id": media_id
            }
        });

        let response = self
            .client
            .post(&self.webhook_url)
            .json(&payload)
            .send()
            .await?;

        if response.status().is_success() {
            let result: WebhookResponse = response.json().await?;
            if result.errcode == 0 {
                Ok(json!({
                    "success": true,
                    "message": "File message sent successfully",
                    "data": result
                })
                .to_string())
            } else {
                Err(SearcherError::ApiError(format!(
                    "WeChat API error: {} (errcode: {})",
                    result.errmsg, result.errcode
                )))
            }
        } else {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read response body".to_string());
            Err(SearcherError::ApiError(format!(
                "HTTP error: {} - {}",
                status.as_u16(),
                body
            )))
        }
    }
}

/// 图文消息文章
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Article {
    pub title: String,
    pub description: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub picurl: Option<String>,
}

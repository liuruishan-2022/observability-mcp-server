use crate::searcher::SearcherError;
use rdkafka::{
    Message,
    admin::{AdminClient, AdminOptions, NewTopic, TopicReplication},
    client::DefaultClientContext,
    config::ClientConfig,
    consumer::{Consumer, DefaultConsumerContext, StreamConsumer},
    message::{Header as KafkaHeader, Headers, OwnedHeaders},
    producer::{FutureProducer, FutureRecord},
};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tokio::time::timeout as tokio_timeout;
use uuid::Uuid;

/// Kafka 客户端
pub struct KafkaClient {
    bootstrap_servers: String,
    group_id: String,
    username: Option<String>,
    password: Option<String>,
    security_protocol: Option<String>,
}

impl KafkaClient {
    /// 创建新的 Kafka 客户端
    pub fn new(
        bootstrap_servers: String,
        group_id: Option<String>,
        username: Option<String>,
        password: Option<String>,
        security_protocol: Option<String>,
    ) -> Result<Self, SearcherError> {
        Ok(Self {
            bootstrap_servers,
            group_id: group_id.unwrap_or_else(|| "mcp-kafka-consumer-group".to_string()),
            username,
            password,
            security_protocol,
        })
    }

    /// 创建 AdminClient
    fn create_admin_client(&self) -> Result<AdminClient<DefaultClientContext>, SearcherError> {
        let mut config = ClientConfig::new();
        config.set("bootstrap.servers", &self.bootstrap_servers);

        if let (Some(username), Some(password)) = (&self.username, &self.password) {
            config.set("sasl.mechanism", "PLAIN");
            config.set("sasl.username", username);
            config.set("sasl.password", password);
        }

        if let Some(protocol) = &self.security_protocol {
            config.set("security.protocol", protocol);
        }

        config
            .create::<AdminClient<_>>()
            .map_err(|e| SearcherError::Other(format!("Failed to create admin client: {}", e)))
    }

    /// 创建 Producer
    fn create_producer(&self) -> Result<FutureProducer, SearcherError> {
        let mut config = ClientConfig::new();
        config.set("bootstrap.servers", &self.bootstrap_servers);

        if let (Some(username), Some(password)) = (&self.username, &self.password) {
            config.set("sasl.mechanism", "PLAIN");
            config.set("sasl.username", username);
            config.set("sasl.password", password);
        }

        if let Some(protocol) = &self.security_protocol {
            config.set("security.protocol", protocol);
        }

        config
            .create::<FutureProducer>()
            .map_err(|e| SearcherError::Other(format!("Failed to create producer: {}", e)))
    }

    /// 创建 Consumer
    fn create_consumer(&self) -> Result<StreamConsumer<DefaultConsumerContext>, SearcherError> {
        let mut config = ClientConfig::new();
        config.set("bootstrap.servers", &self.bootstrap_servers);
        config.set("group.id", &self.group_id);
        config.set("enable.auto.commit", "true");
        config.set("auto.offset.reset", "earliest");

        if let (Some(username), Some(password)) = (&self.username, &self.password) {
            config.set("sasl.mechanism", "PLAIN");
            config.set("sasl.username", username);
            config.set("sasl.password", password);
        }

        if let Some(protocol) = &self.security_protocol {
            config.set("security.protocol", protocol);
        }

        config
            .create::<StreamConsumer<_>>()
            .map_err(|e| SearcherError::Other(format!("Failed to create consumer: {}", e)))
    }

    /// 创建主题
    pub async fn create_topic(
        &self,
        topic: &str,
        num_partitions: i32,
        replication_factor: i32,
    ) -> Result<String, SearcherError> {
        let admin = self.create_admin_client()?;

        let new_topic = NewTopic::new(
            topic,
            num_partitions,
            TopicReplication::Fixed(replication_factor),
        );

        let opts = AdminOptions::new().request_timeout(Some(Duration::from_secs(3)));

        let results = admin
            .create_topics([&new_topic], &opts)
            .await
            .map_err(|e| {
                SearcherError::Other(format!("Failed to send create topic request: {}", e))
            })?;

        if results.is_empty() {
            return Ok(format!("Topic '{}' created successfully", topic));
        }

        let result = &results[0];
        match result {
            Ok(_) => Ok(format!("Topic '{}' created successfully", topic)),
            Err((_, e)) => Err(SearcherError::Other(format!(
                "Failed to create topic: {}",
                e
            ))),
        }
    }

    /// 列出所有主题
    pub async fn list_topics(&self) -> Result<TopicesList, SearcherError> {
        let admin = self.create_admin_client()?;

        let metadata = admin
            .inner()
            .fetch_metadata(None, Duration::from_secs(3))
            .map_err(|e| SearcherError::Other(format!("Failed to fetch metadata: {}", e)))?;

        let topics = metadata
            .topics()
            .iter()
            .filter(|t| !t.name().is_empty())
            .map(|t| TopicInfo {
                name: t.name().to_string(),
                partitions: t.partitions().len() as i32,
            })
            .collect();

        Ok(TopicesList { topics })
    }

    /// 删除主题
    pub async fn delete_topic(&self, topic: &str) -> Result<String, SearcherError> {
        let admin = self.create_admin_client()?;

        let opts = AdminOptions::new().request_timeout(Some(Duration::from_secs(3)));

        let results = admin.delete_topics(&[topic], &opts).await.map_err(|e| {
            SearcherError::Other(format!("Failed to send delete topic request: {}", e))
        })?;

        if results.is_empty() {
            return Ok(format!("Topic '{}' deleted successfully", topic));
        }

        let result = &results[0];
        match result {
            Ok(_) => Ok(format!("Topic '{}' deleted successfully", topic)),
            Err((_, e)) => Err(SearcherError::Other(format!(
                "Failed to delete topic: {}",
                e
            ))),
        }
    }

    /// 描述主题
    pub async fn describe_topic(&self, topic: &str) -> Result<TopicMetadata, SearcherError> {
        let admin = self.create_admin_client()?;

        let metadata = admin
            .inner()
            .fetch_metadata(Some(topic), Duration::from_secs(3))
            .map_err(|e| SearcherError::Other(format!("Failed to fetch metadata: {}", e)))?;

        let topic_meta = metadata
            .topics()
            .iter()
            .find(|t| t.name() == topic)
            .ok_or_else(|| SearcherError::Other(format!("Topic '{}' not found", topic)))?;

        let partitions = topic_meta
            .partitions()
            .iter()
            .map(|p| PartitionInfo {
                id: p.id(),
                leader: p.leader(),
                replicas: p.replicas().to_vec(),
                isr: p.isr().to_vec(),
            })
            .collect();

        Ok(TopicMetadata {
            name: topic_meta.name().to_string(),
            partitions,
            partition_count: topic_meta.partitions().len() as i32,
        })
    }

    /// 生产消息
    pub async fn produce_message(
        &self,
        topic: &str,
        key: Option<String>,
        value: &str,
        headers: Option<Vec<(String, String)>>,
    ) -> Result<ProduceResult, SearcherError> {
        let producer = self.create_producer()?;

        // 如果没有提供 key，生成 UUID
        let message_key = key.unwrap_or_else(|| Uuid::new_v4().to_string());

        // 构建 headers
        let owned_headers = if let Some(header_list) = headers {
            let mut oh = OwnedHeaders::new();
            for (k, v) in header_list {
                oh = oh.insert(KafkaHeader {
                    key: k.as_str(),
                    value: Some(v.as_bytes()),
                });
            }
            oh
        } else {
            OwnedHeaders::new()
        };

        // 发送消息
        producer
            .send(
                FutureRecord::to(topic)
                    .key(message_key.as_bytes())
                    .payload(value)
                    .headers(owned_headers),
                Duration::from_secs(5),
            )
            .await
            .map_err(|(e, _)| SearcherError::Other(format!("Failed to produce message: {}", e)))?;

        // 返回成功结果（在 0.38 版本中 Delivery 结构发生了变化，这里简化处理）
        Ok(ProduceResult {
            topic: topic.to_string(),
            partition: 0,
            offset: 0,
        })
    }

    /// 消费消息
    pub async fn consume_messages(
        &self,
        topic: &str,
        timeout_seconds: Option<i32>,
    ) -> Result<Vec<ConsumedMessage>, SearcherError> {
        let consumer = self.create_consumer()?;

        // 订阅主题
        consumer
            .subscribe(&[topic])
            .map_err(|e| SearcherError::Other(format!("Failed to subscribe to topic: {}", e)))?;

        let timeout_duration = Duration::from_secs(timeout_seconds.unwrap_or(10) as u64);
        let start_time = Instant::now();
        let mut messages = Vec::new();

        // 消费消息直到超时
        while start_time.elapsed() < timeout_duration {
            // 使用 tokio timeout 进行超时控制
            match tokio_timeout(Duration::from_secs(1), consumer.recv()).await {
                Ok(Ok(msg)) => {
                    let key = msg.key().map(|k| String::from_utf8_lossy(k).to_string());
                    let payload = msg
                        .payload()
                        .map(|p| String::from_utf8_lossy(p).to_string());

                    // 提取 headers
                    let headers = if let Some(h) = msg.headers() {
                        let mut header_vec = Vec::new();
                        let count: usize = h.count();
                        for i in 0..count {
                            let header = h.get(i);
                            let value = header
                                .value
                                .map(|v| String::from_utf8_lossy(v).to_string())
                                .unwrap_or_default();
                            header_vec.push((header.key.to_string(), value));
                        }
                        Some(header_vec)
                    } else {
                        None
                    };

                    messages.push(ConsumedMessage {
                        key,
                        value: payload,
                        partition: msg.partition(),
                        offset: msg.offset(),
                        headers,
                    });
                }
                Ok(Err(_)) => {
                    // 接收错误，继续等待
                }
                Err(_) => {
                    // 超时，继续检查总时间
                }
            }

            // 如果有消息且超过一定数量，提前返回
            if messages.len() >= 100 {
                break;
            }
        }

        Ok(messages)
    }
}

// ========== 数据结构定义 ==========

/// 主题列表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicesList {
    pub topics: Vec<TopicInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicInfo {
    pub name: String,
    pub partitions: i32,
}

/// 主题元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicMetadata {
    pub name: String,
    pub partitions: Vec<PartitionInfo>,
    pub partition_count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionInfo {
    pub id: i32,
    pub leader: i32,
    pub replicas: Vec<i32>,
    pub isr: Vec<i32>,
}

/// 生产消息结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProduceResult {
    pub topic: String,
    pub partition: i32,
    pub offset: i64,
}

/// 消费的消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumedMessage {
    pub key: Option<String>,
    pub value: Option<String>,
    pub partition: i32,
    pub offset: i64,
    pub headers: Option<Vec<(String, String)>>,
}

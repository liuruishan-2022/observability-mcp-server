use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

use crate::searcher::SearcherError;

/// 文档块
#[derive(Debug, Clone)]
pub struct DocChunk {
    pub id: String,
    pub file_name: String,
    pub chunk_index: usize,
    pub content: String,
}

impl DocChunk {
    pub fn new(file_name: String, chunk_index: usize, content: String) -> Self {
        let id = format!("{}#{}", file_name, chunk_index);
        DocChunk {
            id,
            file_name,
            chunk_index,
            content,
        }
    }
}

/// 文档加载器 - 负责加载、分块和搜索文档
pub struct DocsLoader {
    chunks: Vec<DocChunk>,
    file_map: HashMap<String, Vec<usize>>, // 文件名 -> chunk 索引列表
}

impl DocsLoader {
    /// 从文档目录创建新的文档加载器
    pub fn from_dir<P: AsRef<Path>>(docs_dir: P) -> Result<Self, SearcherError> {
        let docs_dir = docs_dir.as_ref();
        info!("Loading documentation from: {}", docs_dir.display());

        let mut chunks = Vec::new();
        let mut file_map = HashMap::new();

        // 遍历文档目录，查找所有 .md 文件
        let entries = fs::read_dir(docs_dir).map_err(|e| SearcherError::IoError(e))?;

        for entry in entries {
            let entry = entry.map_err(|e| SearcherError::IoError(e))?;
            let path = entry.path();

            if path.is_dir() {
                continue;
            }

            // 只处理 .md 文件
            if path.extension().and_then(|s| s.to_str()) != Some("md") {
                continue;
            }

            let file_name = path
                .file_name()
                .and_then(|s| s.to_str())
                .ok_or_else(|| SearcherError::Other("Invalid file name".to_string()))?
                .to_string();

            debug!("Processing documentation file: {}", file_name);

            // 读取文件内容
            let content = fs::read_to_string(&path).map_err(|e| SearcherError::IoError(e))?;

            // 移除 frontmatter（如果存在）
            let content = Self::strip_frontmatter(&content);

            // 将内容分块（每块约 4KB，重叠 512 字节）
            let file_chunks = Self::chunk_content(&content, 4096, 512);
            let mut chunk_indices = Vec::new();

            for (idx, chunk_content) in file_chunks.iter().enumerate() {
                let chunk = DocChunk::new(file_name.clone(), idx + 1, chunk_content.clone());
                chunk_indices.push(chunks.len());
                chunks.push(chunk);
            }

            file_map.insert(file_name, chunk_indices);
        }

        info!(
            "Loaded {} documentation chunks from {} files",
            chunks.len(),
            file_map.len()
        );

        Ok(DocsLoader { chunks, file_map })
    }

    /// 移除 markdown 文件的 frontmatter
    fn strip_frontmatter(content: &str) -> String {
        // 检查是否以 --- 开头
        if !content.starts_with("---") {
            return content.to_string();
        }

        // 查找第二个 ---
        let content_without_first = &content[3..];
        if let Some(end_pos) = content_without_first.find("\n---") {
            let remaining = &content_without_first[end_pos + 5..];
            return remaining.trim_start().to_string();
        }

        content.to_string()
    }

    /// 将内容分成重叠的块
    fn chunk_content(content: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
        let chars: Vec<char> = content.chars().collect();
        let mut chunks = Vec::new();
        let mut pos = 0;

        while pos < chars.len() {
            let end = (pos + chunk_size).min(chars.len());
            let chunk: String = chars[pos..end].iter().collect();
            chunks.push(chunk.trim().to_string());

            // 移动到下一个块的起始位置（带重叠）
            if end >= chars.len() {
                break;
            }
            pos += chunk_size - overlap;
        }

        chunks
    }

    /// 列出所有文档文件
    pub fn list_files(&self) -> Vec<String> {
        let mut files: Vec<String> = self.file_map.keys().cloned().collect();
        files.sort();
        files
    }

    /// 读取指定文件的内容
    pub fn read_file(&self, file_name: &str) -> Result<String, SearcherError> {
        let indices = self
            .file_map
            .get(file_name)
            .ok_or_else(|| SearcherError::Other(format!("File not found: {}", file_name)))?;

        let mut full_content = String::new();
        for idx in indices {
            if let Some(chunk) = self.chunks.get(*idx) {
                full_content.push_str(&chunk.content);
                full_content.push_str("\n\n");
            }
        }

        Ok(full_content)
    }

    /// 搜索文档内容（简单的关键词匹配）
    pub fn search(&self, query: &str, limit: usize) -> Vec<String> {
        let query_lower = query.to_lowercase();
        let mut matches: Vec<(String, usize)> = Vec::new();

        for chunk in &self.chunks {
            let content_lower = chunk.content.to_lowercase();

            // 统计匹配次数
            let count = content_lower.matches(&query_lower).count();

            if count > 0 {
                matches.push((chunk.file_name.clone(), count));
            }
        }

        // 按匹配次数排序
        matches.sort_by(|a, b| b.1.cmp(&a.1));

        // 去重并限制结果数量
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();

        for (file_name, _) in matches {
            if seen.insert(file_name.clone()) {
                result.push(file_name);
                if result.len() >= limit {
                    break;
                }
            }
        }

        result
    }
}

/// 共享的文档加载器
pub type SharedDocsLoader = Arc<RwLock<Option<DocsLoader>>>;

/// 创建共享的文档加载器
pub fn create_shared_docs_loader() -> SharedDocsLoader {
    Arc::new(RwLock::new(None))
}

/// 初始化文档加载器
pub async fn init_docs_loader(
    shared: &SharedDocsLoader,
    docs_dir: &str,
) -> Result<(), SearcherError> {
    let loader = DocsLoader::from_dir(docs_dir)?;
    let mut guard = shared.write().await;
    *guard = Some(loader);
    Ok(())
}

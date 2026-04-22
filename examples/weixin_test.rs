use dotenv::dotenv;
/// 企业微信机器人测试示例
///
/// 使用方法：
/// 1. 确保 .env 文件中配置了 WEIXIN_WEBHOOK_URL
/// 2. 运行: cargo run --example weixin_test
///
use std::env;

// 注意：这是一个示例文件，展示如何直接使用 WeixinClient
// 在实际使用中，这些工具会通过 MCP 协议被调用

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    // 从环境变量获取 Webhook URL
    let webhook_url = env::var("WEIXIN_WEBHOOK_URL").expect("WEIXIN_WEBHOOK_URL must be set");

    println!("企业微信机器人测试");
    println!("==================");
    println!("Webhook URL: {}\n", webhook_url);

    // 这里你可以测试企业微信客户端
    // 由于这是一个独立的示例，你不能直接导入 observability_mcp_server::searcher::weixin
    // 但在实际的 MCP 服务器中，这些功能会通过工具接口自动可用

    println!("测试完成！");
    println!("\n请通过 MCP 客户端调用以下工具进行测试：");
    println!("- weixin_send_text: 发送文本消息");
    println!("- weixin_send_markdown: 发送 Markdown 消息");
    println!("- weixin_send_image: 发送图片消息");
    println!("- weixin_send_file: 发送文件消息");
    println!("- weixin_send_news: 发送图文消息");

    Ok(())
}

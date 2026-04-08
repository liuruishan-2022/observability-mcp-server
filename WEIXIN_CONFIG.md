# 企业微信机器人 MCP 接口配置指南

## 功能说明

本 MCP 服务器提供了企业微信机器人的消息推送功能，支持以下消息类型：

- **文本消息** (`weixin_send_text`): 发送文本消息���支持 `@` 提醒特定用户
- **Markdown 消息** (`weixin_send_markdown`): 发送格式化的 Markdown 消息
- **图片消息** (`weixin_send_image`): 发送图片（需要先上传获取 media_id）
- **文件消息** (`weixin_send_file`): 发送文件（需要先上传获取 media_id）
- **图文消息** (`weixin_send_news`): 发送图文链接

## 配置步骤

### 1. 获取企业微信机器人 Webhook URL

1. 在企业微信群聊中，点击右上角 `...` 菜单
2. 选择 `群机器人` → `添加机器人`
3. 选择 `新建机器人`
4. 设置机器人名称和头像
5. 创建完成后，复制 Webhook URL，格式类似：
   ```
   https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=693axxx6-7aoc-4bc4-97a0-0ec2sifa5aaa
   ```

### 2. 配置环境变量

在项目根目录创建或编辑 `.env` 文件，添加以下配置：

```env
# 企业微信机器人 Webhook URL (必需)
WEIXIN_WEBHOOK_URL=https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=YOUR_KEY_HERE

# 其他必需的环境变量
PROMETHEUS_ROOT=http://localhost:9090
LOKI_ROOT=http://localhost:3100
```

### 3. 重启服务

```bash
# 重新编译并运行
cargo run
```

## 使用示例

### 发送文本消息

```json
{
  "content": "广州今日天气：29度，大部分多云，降雨概率：60%",
  "mentioned_list": ["wangqing", "@all"],
  "mentioned_mobile_list": ["13800001111", "@all"]
}
```

### 发送 Markdown 消息

```json
{
  "content": "实时新增用户反馈<font color=\"warning\">132例</font>，请相关同事注意。\n>类型:<font color=\"info\">用户反馈</font>\n>普通用户反馈:<font color=\"comment\">117例</font>\n>VIP用户反馈:<font color=\"comment\">15例</font>"
}
```

### 发送图文消息

```json
{
  "articles": [
    {
      "title": "中秋节礼品领取",
      "description": "今年中秋节公司给广大职工发了丰富的礼品...",
      "url": "URL",
      "picurl": "http://res.mail.qq.com/node/ww/wwopenmng/images/independent/doc/test_pic_msg1.png"
    }
  ]
}
```

## MCP 工具列表

| 工具名称 | 描述 | 参数 |
|---------|------|------|
| `weixin_send_text` | 发送文本消息 | content, mentioned_list?, mentioned_mobile_list? |
| `weixin_send_markdown` | 发送 Markdown 消息 | content |
| `weixin_send_image` | 发送图片消息 | media_id |
| `weixin_send_file` | 发送文件消息 | media_id |
| `weixin_send_news` | 发送图文消息 | articles (数组) |

## 错误处理

如果配置不正确，调用工具时会返回以下错误：

```
Error: WeChat client not configured. Please set WEIXIN_WEBHOOK_URL environment variable.
```

请确保 `.env` 文件中正确配置了 `WEIXIN_WEBHOOK_URL`。

## 注意事项

1. 企业微信机器人每个群最多支持 10 个机器人
2. 文本消息支持 `@all` 提醒所有人，或 `@userid` 提醒特定成员
3. Markdown 消息只支持部分 Markdown 标记，详见 [企业微信官方文档](https://developer.work.weixin.qq.com/document/path/91770)
4. 图片和文件消息需要先通过素材接口上传获取 media_id
5. 每个机器人每分钟最多发送 20 条消息

## 参考文档

- [企业微信机器人 API 文档](https://developer.work.weixin.qq.com/document/path/91770)
- [群机器人配置说明](https://developer.work.weixin.qq.com/document/path/91770)

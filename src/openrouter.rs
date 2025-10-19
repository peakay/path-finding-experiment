use async_stream::stream;
use futures::Stream;
use futures::stream::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::json;
use std::pin::Pin;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ToolCall {
    pub id: String,
    pub r#type: String,
    pub function: ToolCallFunction,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Message {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Tool {
    #[serde(rename = "type")]
    pub type_: String,
    pub function: Function,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Function {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug)]
pub enum OpenRouterEvent {
    Content(String),
    ToolCallDelta {
        name: Option<String>,
        arguments_delta: Option<String>,
    },
}

pub fn open_router_event_stream(
    api_key: String,
    model: String,
    messages: Vec<Message>,
    system_prompt: Option<String>,
    tools: Option<Vec<Tool>>,
) -> Pin<Box<dyn Stream<Item = Result<OpenRouterEvent, Box<dyn std::error::Error + Send + Sync>>>>>
{
    let full_messages = if let Some(system) = system_prompt {
        let mut msgs = vec![Message {
            role: "system".to_string(),
            content: Some(system),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }];
        msgs.extend(messages);
        msgs
    } else {
        messages
    };

    let json_body = json!({
        "model": model,
        "messages": full_messages,
        "stream": true,
        "tools": tools
    });

    Box::pin(stream! {
        let client = Client::new();
        let response = client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .header("X-Title", "pk-chat-agent")
            .json(&json_body)
            .send()
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
            let chunk_str = String::from_utf8_lossy(&bytes);
            for line in chunk_str.lines() {
                if !line.starts_with("data: ") { continue; }
                let data = &line[6..];
                if data == "[DONE]" { break; }
                if let Ok(json) = serde_json::from_str::<Value>(data) {
                    if let Some(choice) = json["choices"].as_array().and_then(|c| c.first()) {
                        if let Some(delta) = choice["delta"].as_object() {
                            if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                                yield Ok(OpenRouterEvent::Content(content.to_string()));
                            }
                            if let Some(tool_calls) = delta.get("tool_calls").and_then(|tc| tc.as_array()) {
                                for tc in tool_calls {
                                    if let Some(function) = tc.get("function").and_then(|f| f.as_object()) {
                                        let name = function.get("name").and_then(|n| n.as_str()).map(|s| s.to_string());
                                        let args = function.get("arguments").and_then(|a| a.as_str()).map(|s| s.to_string());
                                        if name.is_some() || args.is_some() {
                                            yield Ok(OpenRouterEvent::ToolCallDelta { name, arguments_delta: args });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    })
}

//! Provider adapters: one internal message model, one wire format per provider.
//!
//! Messages are stored as `StoredMessage` (parts + tools). These functions turn
//! them into the payload each API expects, so the agent loop stays generic.

#![allow(dead_code)]

use crate::sessions::StoredMessage;
use serde_json::{json, Value};

fn text_of(message: &StoredMessage) -> String {
    if !message.parts.is_empty() {
        let mut out = String::new();
        for part in &message.parts {
            if let crate::sessions::MessagePart::Text { text } = part {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(text);
            }
        }
        if !out.is_empty() {
            return out;
        }
    }
    message.content.clone()
}

fn tool_calls_of(message: &StoredMessage) -> Vec<Value> {
    message
        .tools
        .iter()
        .map(|tool| {
            json!({
                "id": tool.id,
                "type": "function",
                "function": { "name": tool.name, "arguments": tool.arguments }
            })
        })
        .collect()
}

/// OpenAI-compatible payload: system, assistant with `tool_calls`, `tool` results.
pub fn to_openai(messages: &[StoredMessage]) -> Vec<Value> {
    let mut out = Vec::new();

    for message in messages {
        match message.role.as_str() {
            "tool" => {
                for tool in &message.tools {
                    out.push(json!({
                        "role": "tool",
                        "tool_call_id": tool.id,
                        "content": tool.output.clone().unwrap_or_default(),
                    }));
                }
            }
            "assistant" => {
                let calls = tool_calls_of(message);
                let text = text_of(message);
                if calls.is_empty() {
                    out.push(json!({ "role": "assistant", "content": text }));
                } else {
                    out.push(json!({
                        "role": "assistant",
                        "content": if text.is_empty() { Value::Null } else { Value::String(text) },
                        "tool_calls": calls,
                    }));
                }
            }
            role => {
                let text = text_of(message);
                if role == "user" && !message.images.is_empty() {
                    let mut blocks: Vec<Value> = Vec::new();
                    if !text.is_empty() {
                        blocks.push(json!({ "type": "text", "text": text }));
                    }
                    for image in &message.images {
                        blocks.push(json!({
                            "type": "image_url",
                            "image_url": { "url": data_url(image) },
                        }));
                    }
                    out.push(json!({ "role": role, "content": blocks }));
                } else {
                    out.push(json!({ "role": role, "content": text }));
                }
            }
        }
    }

    out
}

fn data_url(image: &crate::sessions::StoredImage) -> String {
    format!("data:{};base64,{}", image.mime, image.data)
}

/// Anthropic payload: system is separate, tool results are user content blocks.
pub fn to_anthropic(messages: &[StoredMessage]) -> (Option<String>, Vec<Value>) {
    let mut system = String::new();
    let mut out = Vec::new();

    for message in messages {
        match message.role.as_str() {
            "system" => {
                if !system.is_empty() {
                    system.push_str("\n\n");
                }
                system.push_str(&text_of(message));
            }
            "tool" => {
                let blocks: Vec<Value> = message
                    .tools
                    .iter()
                    .map(|tool| {
                        json!({
                            "type": "tool_result",
                            "tool_use_id": tool.id,
                            "content": tool.output.clone().unwrap_or_default(),
                            "is_error": tool.is_error,
                        })
                    })
                    .collect();
                out.push(json!({ "role": "user", "content": blocks }));
            }
            "assistant" => {
                let mut blocks: Vec<Value> = Vec::new();
                let text = text_of(message);
                if !text.is_empty() {
                    blocks.push(json!({ "type": "text", "text": text }));
                }
                for tool in &message.tools {
                    let input: Value =
                        serde_json::from_str(&tool.arguments).unwrap_or_else(|_| json!({}));
                    blocks.push(json!({
                        "type": "tool_use",
                        "id": tool.id,
                        "name": tool.name,
                        "input": input,
                    }));
                }
                if blocks.is_empty() {
                    blocks.push(json!({ "type": "text", "text": "" }));
                }
                out.push(json!({ "role": "assistant", "content": blocks }));
            }
            role => {
                if role == "user" && !message.images.is_empty() {
                    let mut blocks: Vec<Value> = Vec::new();
                    let text = text_of(message);
                    if !text.is_empty() {
                        blocks.push(json!({ "type": "text", "text": text }));
                    }
                    blocks.extend(message.images.iter().map(|image| {
                        json!({
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": image.mime,
                                "data": image.data,
                            },
                        })
                    }));
                    out.push(json!({ "role": role, "content": blocks }));
                } else {
                    out.push(json!({ "role": role, "content": text_of(message) }));
                }
            }
        }
    }

    (if system.is_empty() { None } else { Some(system) }, out)
}

/// Tool definitions in Anthropic's shape, from the OpenAI-style ones we keep.
pub fn tools_to_anthropic(tools: &[Value]) -> Vec<Value> {
    tools
        .iter()
        .filter_map(|tool| {
            let function = tool.get("function")?;
            Some(json!({
                "name": function.get("name")?.clone(),
                "description": function.get("description").cloned().unwrap_or(Value::Null),
                "input_schema": function.get("parameters").cloned().unwrap_or_else(|| json!({"type": "object"})),
            }))
        })
        .collect()
}

/// `data:image/png;base64,AAAA…` → an Anthropic image block.
fn anthropic_image(data_url: &str) -> Option<Value> {
    let rest = data_url.strip_prefix("data:")?;
    let (meta, data) = rest.split_once(',')?;
    if !meta.contains("base64") || data.is_empty() {
        return None;
    }
    let media_type = meta
        .split(';')
        .next()
        .filter(|mime| mime.starts_with("image/"))
        .unwrap_or("image/png");
    Some(json!({
        "type": "image",
        "source": { "type": "base64", "media_type": media_type, "data": data },
    }))
}

/// Splits an OpenAI `content` value (a string or a list of blocks) into its text
/// and its images, already in Anthropic's shape.
fn split_content(content: Option<&Value>) -> (String, Vec<Value>) {
    let Some(value) = content else {
        return (String::new(), Vec::new());
    };
    if let Some(text) = value.as_str() {
        return (text.to_string(), Vec::new());
    }
    let mut text = String::new();
    let mut images = Vec::new();
    for part in value.as_array().into_iter().flatten() {
        match part.get("type").and_then(Value::as_str) {
            Some("text") => {
                if let Some(chunk) = part.get("text").and_then(Value::as_str) {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(chunk);
                }
            }
            Some("image_url") => {
                if let Some(url) = part.pointer("/image_url/url").and_then(Value::as_str) {
                    if let Some(block) = anthropic_image(url) {
                        images.push(block);
                    }
                }
            }
            _ => {}
        }
    }
    (text, images)
}

/// Converts OpenAI-style messages (as the agent loop builds them) to Anthropic's
/// format: system apart, `tool_calls` -> `tool_use` blocks, `tool` -> `tool_result`
/// blocks grouped in one user message, `image_url` -> `image`.
pub fn openai_to_anthropic(messages: &[Value]) -> (Option<String>, Vec<Value>) {
    let mut system: Vec<String> = Vec::new();
    let mut out: Vec<Value> = Vec::new();

    for message in messages {
        let role = message.get("role").and_then(Value::as_str).unwrap_or("");
        let (text, images) = split_content(message.get("content"));
        match role {
            "system" => system.push(text),
            "tool" => {
                let block = json!({
                    "type": "tool_result",
                    "tool_use_id": message.get("tool_call_id").cloned().unwrap_or(Value::Null),
                    "content": text,
                });
                // Consecutive results share one user message.
                if let Some(last) = out.last_mut() {
                    let is_results = last["role"] == "user"
                        && last["content"]
                            .as_array()
                            .and_then(|blocks| blocks.first())
                            .map_or(false, |b| b["type"] == "tool_result");
                    if is_results {
                        last["content"].as_array_mut().unwrap().push(block);
                        continue;
                    }
                }
                out.push(json!({ "role": "user", "content": [block] }));
            }
            "assistant" => {
                let mut blocks: Vec<Value> = Vec::new();
                if !text.trim().is_empty() {
                    blocks.push(json!({ "type": "text", "text": text }));
                }
                blocks.extend(images);
                if let Some(calls) = message.get("tool_calls").and_then(Value::as_array) {
                    for call in calls {
                        let args = call
                            .pointer("/function/arguments")
                            .and_then(Value::as_str)
                            .unwrap_or("{}");
                        let input = serde_json::from_str::<Value>(args)
                            .ok()
                            .filter(Value::is_object)
                            .unwrap_or_else(|| json!({}));
                        blocks.push(json!({
                            "type": "tool_use",
                            "id": call.get("id").cloned().unwrap_or(Value::Null),
                            "name": call.pointer("/function/name").cloned().unwrap_or(Value::Null),
                            "input": input,
                        }));
                    }
                }
                if !blocks.is_empty() {
                    out.push(json!({ "role": "assistant", "content": blocks }));
                }
            }
            _ => {
                if text.trim().is_empty() && images.is_empty() {
                    continue;
                }
                if images.is_empty() {
                    // Anthropic wants roles to alternate: join a plain message to a plain one before it
                    // (this happens right after a compaction, which sends a summary and then the user).
                    if let Some(last) = out.last_mut() {
                        if last["role"] == role && last["content"].is_string() {
                            let joined = format!("{}\n\n{}", last["content"].as_str().unwrap_or(""), text);
                            last["content"] = Value::String(joined);
                            continue;
                        }
                    }
                    out.push(json!({ "role": role, "content": text }));
                } else {
                    // A message that carries images cannot be joined, so it goes as its own blocks.
                    let mut blocks: Vec<Value> = Vec::new();
                    if !text.is_empty() {
                        blocks.push(json!({ "type": "text", "text": text }));
                    }
                    blocks.extend(images);
                    out.push(json!({ "role": role, "content": blocks }));
                }
            }
        }
    }

    let system = system.join("\n\n");
    ((!system.is_empty()).then_some(system), out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sessions::StoredTool;

    fn message(role: &str, content: &str) -> StoredMessage {
        StoredMessage {
            role: role.into(),
            content: content.into(),
            reasoning: None,
            agent: None,
            model: None,
            parts: Vec::new(),
            tools: Vec::new(),
            images: Vec::new(),
            compaction: false,
        }
    }

    #[test]
    fn openai_turns_tools_into_tool_calls_and_results() {
        let mut assistant = message("assistant", "thinking out loud");
        assistant.tools = vec![StoredTool {
            id: "call_1".into(),
            name: "read".into(),
            arguments: "{\"path\":\"a.txt\"}".into(),
            output: Some("body".into()),
            is_error: false,
            duration_ms: Some(10),
        }];
        let messages = vec![message("user", "hi"), assistant];

        let payload = to_openai(&messages);
        assert_eq!(payload[0]["role"], "user");
        assert_eq!(payload[1]["tool_calls"][0]["function"]["name"], "read");
        assert_eq!(payload[1]["content"], "thinking out loud");
    }

    #[test]
    fn anthropic_puts_system_apart_and_tools_as_blocks() {
        let mut assistant = message("assistant", "hi");
        assistant.tools = vec![StoredTool {
            id: "t1".into(),
            name: "glob".into(),
            arguments: "{\"pattern\":\"*.md\"}".into(),
            output: None,
            is_error: false,
            duration_ms: None,
        }];
        let messages = vec![message("system", "be nice"), message("user", "go"), assistant];

        let (system, payload) = to_anthropic(&messages);
        assert_eq!(system.as_deref(), Some("be nice"));
        assert_eq!(payload[1]["content"][1]["type"], "tool_use");
        assert_eq!(payload[1]["content"][1]["input"]["pattern"], "*.md");
    }

    #[test]
    fn anthropic_tool_definitions_use_input_schema() {
        let tools = crate::tools::definitions("build");
        let converted = tools_to_anthropic(&tools);
        assert_eq!(converted[0]["name"], "read");
        assert_eq!(converted[0]["input_schema"]["type"], "object");
    }

    #[test]
    fn converts_openai_loop_messages_to_anthropic() {
        let messages = vec![
            json!({"role":"system","content":"sys"}),
            json!({"role":"user","content":"hi"}),
            json!({"role":"assistant","content":"looking","tool_calls":[
                {"id":"a","type":"function","function":{"name":"read","arguments":"{\"path\":\"x\"}"}},
                {"id":"b","type":"function","function":{"name":"grep","arguments":"{}"}}]}),
            json!({"role":"tool","tool_call_id":"a","content":"one"}),
            json!({"role":"tool","tool_call_id":"b","content":"two"}),
        ];
        let (system, out) = openai_to_anthropic(&messages);
        assert_eq!(system.as_deref(), Some("sys"));
        assert_eq!(out.len(), 3);
        assert_eq!(out[1]["content"][0]["type"], "text");
        assert_eq!(out[1]["content"][1]["type"], "tool_use");
        assert_eq!(out[1]["content"][1]["input"]["path"], "x");
        assert_eq!(out[2]["role"], "user");
        assert_eq!(out[2]["content"].as_array().unwrap().len(), 2);
        assert_eq!(out[2]["content"][1]["tool_use_id"], "b");
    }

    #[test]
    fn openai_user_images_become_anthropic_image_blocks() {
        let messages = vec![json!({
            "role": "user",
            "content": [
                { "type": "text", "text": "what is this?" },
                { "type": "image_url", "image_url": { "url": "data:image/png;base64,AAAA" } }
            ]
        })];
        let (system, out) = openai_to_anthropic(&messages);
        assert!(system.is_none());
        assert_eq!(out.len(), 1);
        let blocks = out[0]["content"].as_array().unwrap();
        assert_eq!(blocks[0]["type"], "text");
        assert_eq!(blocks[1]["type"], "image");
        assert_eq!(blocks[1]["source"]["media_type"], "image/png");
        assert_eq!(blocks[1]["source"]["data"], "AAAA");
    }

    #[test]
    fn stored_user_images_reach_both_wire_formats() {
        let mut message = message("user", "look");
        message.images.push(crate::sessions::StoredImage {
            name: "shot.png".into(),
            mime: "image/png".into(),
            data: "AAAA".into(),
        });

        let openai = to_openai(std::slice::from_ref(&message));
        assert_eq!(openai[0]["content"][1]["type"], "image_url");
        assert_eq!(
            openai[0]["content"][1]["image_url"]["url"],
            "data:image/png;base64,AAAA"
        );

        let (_, anthropic) = to_anthropic(&[message]);
        assert_eq!(anthropic[0]["content"][1]["type"], "image");
    }

    #[test]
    fn consecutive_plain_messages_of_one_role_are_joined_for_anthropic() {
        let messages = vec![
            json!({"role":"user","content":"summary of before"}),
            json!({"role":"user","content":"new question"}),
            json!({"role":"assistant","content":"answer"}),
        ];
        let (_, out) = openai_to_anthropic(&messages);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0]["content"], "summary of before\n\nnew question");
    }
}

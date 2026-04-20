/// Fetch a web page and return title + content snippet.
use reqwest;

#[derive(Clone)]
pub struct WebFetchTool;

impl WebFetchTool {
    pub fn new() -> Self {
        Self
    }

    pub async fn fetch(&self, args: &str) -> String {
        let url = args.trim();
        if url.is_empty() {
            return "URLを入力してください: `!skill web_fetch https://example.com`".to_string();
        }

        let client = reqwest::Client::new();
        let response = match client.get(url).send().await {
            Ok(resp) => resp,
            Err(e) => return format!("リクエストに失敗しました: `{}`\nエラー: {}", url, e),
        };

        let status = response.status();
        if !status.is_success() {
            return format!("HTTP {}: {}", status, url);
        }

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .map(|v| v.to_str().unwrap_or("").to_string())
            .unwrap_or_default();

        if !content_type.contains("text/html") && !content_type.contains("text/plain") {
            return format!(
                "**{}**\nコンテンツタイプ: {}\n\n(バイナリまたは非テキストコンテンツ)",
                url, content_type
            );
        }

        let body = match response.text().await {
            Ok(t) => t,
            Err(e) => return format!("ページの取得に失敗しました: `{}`\nエラー: {}", url, e),
        };

        // Simple HTML text extraction
        let text = extract_text(&body);

        // Try to find title
        let title = extract_title(&body);

        let truncated = truncate(&text, 3000);
        format!(
            "**{}**{}\n\n```\n{}\n```",
            title.unwrap_or_else(|| url.to_string()),
            if content_type.contains("text/html") {
                format!("\n(HTML, {}文字)", text.len())
            } else {
                format!("\n({}文字)", text.len())
            },
            truncated
        )
    }
}

fn extract_title(html: &str) -> Option<String> {
    let start = html.find("<title")?;
    let end = html[start..].find('>')?;
    let title_start = start + end + 1;
    let title_end = html[title_start..].find("</title>")?;
    Some(html[title_start..title_start + title_end].trim().to_string())
}

fn extract_text(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    let skip_tags = ["script", "style", "noscript", "meta", "link", "head"];

    for ch in html.chars() {
        if ch == '<' {
            in_tag = true;
            // Check if this is a skip tag
            let remaining = &html[html.find('<').unwrap_or(0)..];
            for tag in &skip_tags {
                if remaining.starts_with(&format!("<{}", tag))
                    || remaining.starts_with(&format!("</{}", tag))
                {
                    // Skip until closing tag
                    if let Some(close) = remaining.find(&format!("</{}>", tag)) {
                        // Skip past closing tag
                        let _skip_len = close + tag.len() + 3;
                        // We'll handle this by just not adding chars
                        continue;
                    }
                }
            }
            continue;
        }
        if ch == '>' {
            in_tag = false;
            continue;
        }
        if !in_tag {
            if ch == '\n' || ch == '\r' {
                result.push(' ');
            } else {
                result.push(ch);
            }
        }
    }

    // Collapse whitespace
    result.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate(text: &str, max: usize) -> String {
    if text.len() <= max {
        text.to_string()
    } else {
        format!("{}...", text.chars().take(max).collect::<String>())
    }
}

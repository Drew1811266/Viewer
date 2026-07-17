use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, html};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalUrl(String);

impl ExternalUrl {
    pub fn parse(value: &str) -> Result<Self, ExternalUrlError> {
        if value.len() > 8_192 {
            return Err(ExternalUrlError);
        }
        let parsed = tauri::Url::parse(value).map_err(|_| ExternalUrlError)?;
        if !matches!(parsed.scheme(), "http" | "https")
            || parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
        {
            return Err(ExternalUrlError);
        }
        Ok(Self(parsed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExternalUrlError;

pub fn image_destinations(markdown: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    Parser::new_ext(markdown, markdown_options())
        .filter_map(|event| match event {
            Event::Start(Tag::Image { dest_url, .. }) => {
                let destination = dest_url.into_string();
                seen.insert(destination.clone()).then_some(destination)
            }
            _ => None,
        })
        .collect()
}

pub fn render_safe_markdown(markdown: &str, images: &HashMap<String, String>) -> String {
    let mut image_stack = Vec::new();
    let filtered = Parser::new_ext(markdown, markdown_options()).filter_map(|event| match event {
        Event::Html(_) | Event::InlineHtml(_) => None,
        Event::Start(Tag::HtmlBlock) | Event::End(TagEnd::HtmlBlock) => None,
        Event::Start(Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        }) => {
            let Some(resolved) = images.get(dest_url.as_ref()) else {
                image_stack.push(false);
                return None;
            };
            image_stack.push(true);
            Some(Event::Start(Tag::Image {
                link_type,
                dest_url: resolved.clone().into(),
                title,
                id,
            }))
        }
        Event::End(TagEnd::Image) => image_stack
            .pop()
            .unwrap_or(false)
            .then_some(Event::End(TagEnd::Image)),
        event => Some(event),
    });
    let mut rendered = String::new();
    html::push_html(&mut rendered, filtered);
    crate::sanitize_markdown_html(&rendered)
}

fn markdown_options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH
}

#[cfg(test)]
mod tests {
    use super::{ExternalUrl, image_destinations, render_safe_markdown};
    use std::collections::HashMap;

    #[test]
    fn renderer_drops_raw_html_and_unresolved_images() {
        let source = "<script>bad()</script>\n\n![ok](local.png) ![bad](https://x/a.png)";
        let destinations = image_destinations(source);
        assert_eq!(destinations, ["local.png", "https://x/a.png"]);
        let images = HashMap::from([(
            "local.png".to_owned(),
            "viewer-image://localhost/session/token".to_owned(),
        )]);

        let html = render_safe_markdown(source, &images);

        assert!(!html.contains("script"));
        assert!(!html.contains("https://x"));
        assert!(html.contains("viewer-image://localhost/session/token"));
    }

    #[test]
    fn external_url_is_normalized_and_rejects_credentials_and_non_http_schemes() {
        assert_eq!(
            ExternalUrl::parse("https://EXAMPLE.com/a")
                .unwrap()
                .as_str(),
            "https://example.com/a"
        );
        for unsafe_url in [
            "file:///tmp/a",
            "javascript:alert(1)",
            "https://u:p@x.test/",
        ] {
            assert!(ExternalUrl::parse(unsafe_url).is_err());
        }
    }
}

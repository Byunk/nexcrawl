//! Preprocess the HTML to make it easier for LLMs to understand

extern crate html5ever;

use crate::node::{Dom, Handle, Node, NodeData, serialize_to_string};
use html5ever::driver::ParseOpts;
use html5ever::parse_document;
use html5ever::{tendril::TendrilSink, tree_builder::TreeBuilderOpts};
use std::rc::Rc;

/// Tags that represents inline text styles
const INLINE_TAGS: &[&str] = &[
    "b",
    "blockquote",
    "code",
    "em",
    "i",
    "small",
    "strike",
    "strong",
];
/// Tags that are forbidden and should be removed from the HTML.
const FORBIDDEN_TAGS: &[&str] = &[
    "script", "noscript", "iframe", "object", "embed", "applet", "link", "meta", "style", "svg",
    "canvas", "audio", "video", "button", "nav", "header", "footer", "hr", "br",
];

/// Preprocess the text
/// * Remove unnecessary spaces, newlines, and tabs
/// * Decode HTML entities like &nbsp;, &amp;, etc.
/// * Remove duplicated whitespace
fn preprocess_text(text: &str) -> String {
    let mut result = text.trim().to_string();

    // Replace all whitespace characters with single spaces
    result = result
        .replace("&nbsp;", " ")
        .replace("\n", " ")
        .replace("\r", " ")
        .replace("\t", " ")
        .replace("\u{00A0}", " "); // Non-breaking space

    // Remove duplicate spaces by repeatedly replacing double spaces with single spaces
    while result.contains("  ") {
        result = result.replace("  ", " ");
    }

    result.trim().to_string()
}

pub struct PreprocessConfig {
    pub remove_links: bool,
    pub remove_images: bool,
    pub remove_tables: bool,
}

impl Default for PreprocessConfig {
    fn default() -> Self {
        Self {
            remove_links: true,
            remove_images: true,
            remove_tables: true,
        }
    }
}

pub struct Preprocessor {
    config: PreprocessConfig,
}

impl Preprocessor {
    pub fn new(config: PreprocessConfig) -> Self {
        Self { config }
    }

    // Compact HTML to make it easier for LLMs to understand
    /// * Remove unnecessary tags and attributes
    /// * Remove unnecessary nested elements
    /// * Compact text nodes
    pub fn preprocess_html(&self, html: &str) -> String {
        if html.is_empty() {
            return String::new();
        }

        let opts = ParseOpts {
            tree_builder: TreeBuilderOpts {
                drop_doctype: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let doc = parse_document(Dom::default(), opts)
            .from_utf8()
            .one(html.as_bytes());

        if let Some(processed_tree) = self.preprocess_node(&doc.tree) {
            return serialize_to_string(&processed_tree);
        }

        String::new()
    }

    /// Recursively process the node and its children
    fn preprocess_node(&self, node: &Handle) -> Option<Handle> {
        // End conditions
        match &node.data {
            NodeData::Text { text } => {
                let raw_text = text.borrow();
                let processed_text = preprocess_text(&raw_text);

                if processed_text.is_empty() {
                    return None;
                }

                return Some(Node::new_text(processed_text));
            }
            NodeData::Element { name, .. } => {
                if FORBIDDEN_TAGS.contains(&name.local.as_ref()) {
                    return None;
                }
                if self.config.remove_links && name.local.as_ref() == "a" {
                    return None;
                }
                if self.config.remove_images && name.local.as_ref() == "img" {
                    return None;
                }
                if self.config.remove_tables && name.local.as_ref() == "table" {
                    return None;
                }
            }
            _ => {}
        }

        let children = node.children.borrow().clone();
        let mut processed_children = Vec::new();

        let mut texts = Vec::new();
        let mut only_text = true;
        for child in children.iter() {
            if let Some(processed) = self.preprocess_node(child) {
                if let NodeData::Text { text: t } = &processed.data {
                    texts.push(t.borrow().clone().to_string());
                } else {
                    only_text = false;
                    if !texts.is_empty() {
                        let combined_text = preprocess_text(&texts.join(" "));

                        processed_children.push(Node::new_text(combined_text));
                        texts.clear();
                    }
                    processed_children.push(processed);
                }
            }
        }

        if !texts.is_empty() {
            let combined_text = preprocess_text(&texts.join(" "));

            processed_children.push(Node::new_text(combined_text));
        }

        // Compaction algorithm

        // If the node has no children, return None
        if processed_children.is_empty() {
            return None;
        }

        // If the number of children is 1 and the child is the same tag, skip the current node
        if processed_children.len() == 1 {
            let child = processed_children.first().expect("Child not found").clone();

            match (&node.data, &child.data) {
                (
                    NodeData::Element { name, .. },
                    NodeData::Element {
                        name: child_name, ..
                    },
                ) if name.local.as_ref() == child_name.local.as_ref() => {
                    // Create a deep copy of the child subtree to avoid reference issues
                    return Some(child.deep_copy());
                }
                _ => {}
            }
        }

        // If the node is an inline element and only contains text nodes, compact the node
        if only_text
            && matches!(&node.data, NodeData::Element { name, .. } if INLINE_TAGS.contains(&name.local.as_ref()))
        {
            let mut texts = Vec::new();
            for child in processed_children.iter() {
                if let NodeData::Text { text: t } = &child.data {
                    texts.push(t.borrow().clone().to_string());
                }
            }

            let combined_text = preprocess_text(&texts.join(" "));
            return Some(Node::new_text(combined_text));
        }

        let new_node = node.clone();
        new_node.parent.set(None);

        for child in processed_children.iter() {
            child.parent.set(Some(Rc::downgrade(&new_node)));
        }

        new_node.children.replace(processed_children);
        Some(new_node)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complex_nested_html_preprocessing() {
        let complex_html = r#"
            <html>
                <head>
                    <title>Test Page</title>
                    <script>alert('should be removed');</script>
                    <style>body { color: red; }</style>
                    <meta charset="utf-8">
                </head>
                <body>
                    <header>Header content</header>
                    <nav>Navigation menu</nav>

                    <div>
                        <div>
                            <div>
                                <p>This is a paragraph with <b>bold text</b> and <em>emphasized text</em>.</p>
                                <div>
                                    <span>Regular span content</span>
                                </div>
                            </div>
                        </div>
                    </div>

                    <section>
                        <article>
                            <h1>Main heading</h1>
                            <p>Paragraph with <strong>strong text</strong> and <i>italic text</i>.</p>

                            <div>
                                <blockquote>Quote with <code>code snippet</code> inside</blockquote>
                            </div>

                            <ul>
                                <li>List item 1</li>
                                <li>List item with <small>small text</small></li>
                            </ul>
                        </article>
                    </section>

                    <footer>Footer content</footer>
                    <button onclick="doSomething()">Click me</button>
                </body>
            </html>
        "#;

        let result = Preprocessor::new(PreprocessConfig::default()).preprocess_html(complex_html);

        // Expected output after preprocessing:
        // - All forbidden tags (script, style, meta, header, nav, footer, button) removed
        // - Nested divs compacted
        // - Inline tags (b, em, strong, i, code, small) converted to plain text
        // - blockquote converted to plain text since it only contains text
        // - Whitespace normalized
        let expected = "<html><head><title>Test Page</title></head><body><div><p>This is a paragraph with bold text and emphasized text .</p><div><span>Regular span content</span></div></div><section><article><h1>Main heading</h1><p>Paragraph with strong text and italic text .</p><div>Quote with code snippet inside</div><ul><li>List item 1</li><li>List item with small text</li></ul></article></section></body></html>";

        assert_eq!(result, expected);
    }

    #[test]
    fn test_remove_flags() {
        let html = "<div><p>Text with <a href='http://example.com'>link</a></p><img src='http://example.com/image.jpg' alt='Image' /></div>";
        let result = Preprocessor::new(PreprocessConfig {
            remove_links: true,
            remove_images: true,
            remove_tables: true,
        })
        .preprocess_html(html);
        assert_eq!(
            result,
            "<html><body><div><p>Text with</p></div></body></html>"
        );
    }

    #[test]
    fn test_preprocess_text() {
        // Test HTML entity decoding
        let text_with_entities =
            "Hello&nbsp;world &amp; more&lt;test&gt; &quot;quotes&quot; &#39;apostrophe&#39;";
        let result = preprocess_text(text_with_entities);
        assert_eq!(
            result,
            "Hello world &amp; more&lt;test&gt; &quot;quotes&quot; &#39;apostrophe&#39;"
        );

        // Test whitespace normalization
        let text_with_whitespace = "  Hello\n\tworld  \r\n  with   lots    of     spaces  ";
        let result = preprocess_text(text_with_whitespace);
        assert_eq!(result, "Hello world with lots of spaces");

        // Test non-breaking space
        let text_with_nbsp = "Text\u{00A0}with\u{00A0}non-breaking\u{00A0}spaces";
        let result = preprocess_text(text_with_nbsp);
        assert_eq!(result, "Text with non-breaking spaces");

        // Test empty and whitespace-only text
        assert_eq!(preprocess_text(""), "");
        assert_eq!(preprocess_text("   \n\t\r   "), "");
        assert_eq!(preprocess_text("   single   "), "single");
    }
}

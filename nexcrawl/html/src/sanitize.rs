use std::collections::HashSet;

use ammonia::Builder;

/// All HTML tags
const HTML_TAGS: &[&str] = &[
    // --- Document Structure and Metadata ---
    "!DOCTYPE",
    "html",
    "head",
    "title",
    "body",
    "meta",
    "link",
    "style",
    "script",
    "noscript",
    "base",
    // --- Text Content and Formatting ---
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "p",
    "br",
    "hr",
    "wbr",
    "strong",
    "em",
    "b",
    "i",
    "u",
    "s",
    "strike",
    "del",
    "ins",
    "mark",
    "small",
    "sub",
    "sup",
    "code",
    "pre",
    "kbd",
    "samp",
    "var",
    "abbr",
    "cite",
    "q",
    "blockquote",
    "bdi",
    "bdo",
    "span",
    // --- Lists ---
    "ul",
    "ol",
    "li",
    "dl",
    "dt",
    "dd",
    "menu",
    // --- Links and Navigation ---
    "a",
    "nav",
    "menuitem",
    // --- Media and Embedded Content ---
    "img",
    "picture",
    "source",
    "audio",
    "video",
    "track",
    "embed",
    "object",
    "param",
    "iframe",
    "canvas",
    "svg",
    "math",
    "map",
    "area",
    "figure",
    "figcaption",
    // --- Tables ---
    "table",
    "caption",
    "thead",
    "tbody",
    "tfoot",
    "tr",
    "th",
    "td",
    "col",
    "colgroup",
    // --- Forms and Input ---
    "form",
    "input",
    "textarea",
    "button",
    "select",
    "option",
    "optgroup",
    "label",
    "fieldset",
    "legend",
    "datalist",
    "output",
    "progress",
    "meter",
    "keygen",
    // --- Semantic and Structural Elements ---
    "header",
    "footer",
    "main",
    "section",
    "article",
    "aside",
    "details",
    "summary",
    "dialog",
    "data",
    "time",
    "address",
    "div",
    // --- Ruby Annotations ---
    "ruby",
    "rt",
    "rp",
    "rtc",
    // --- Web Components ---
    "template",
    "slot",
    // --- Deprecated but Still Encountered ---
    "acronym",
    "applet",
    "basefont",
    "big",
    "center",
    "dir",
    "font",
    "frame",
    "frameset",
    "noframes",
    "tt",
    "xmp",
    "plaintext",
    "listing",
    "blink",
    "marquee",
];

/// Default allowed attributes
const ALLOWED_ATTRS: &[&str] = &["class", "id", "title", "lang"];

/// Default tags that are removed
const BLACKLISTED_TAGS: &[&str] = &["script", "style"];

/// Sanitize options
#[derive(Default)]
pub struct SanitizeOptions {
    pub allowed_attributes: HashSet<String>,
    pub blacklisted_tags: HashSet<String>,

    // Useful boolean flags
    pub remove_links: bool,
    pub remove_images: bool,
    pub remove_tables: bool,
}


/// Sanitize the HTML
///
/// Here is the default behavior:
/// * Remain only official HTML tags
/// * Remain only allowed attributes (class, id, title, lang)
/// * Remove contents for blacklisted tags (script, style)
pub fn sanitize_html(html: &str, options: &SanitizeOptions) -> String {
    if html.is_empty() {
        return String::new();
    }

    let mut builder = Builder::empty();

    // Configure blacklisted tags
    let mut blacklisted_tags: HashSet<&str> = HashSet::from_iter(BLACKLISTED_TAGS.iter().copied());

    // Add custom blacklisted tags
    blacklisted_tags.extend(options.blacklisted_tags.iter().map(|tag| tag.as_str()));

    // Add blacklisted from flags
    if options.remove_links {
        blacklisted_tags.insert("a");
    }
    if options.remove_images {
        blacklisted_tags.insert("img");
    }
    if options.remove_tables {
        let table_tags = [
            "table", "th", "tr", "td", "caption", "colgroup", "col", "thead", "tbody", "tfoot",
        ];
        blacklisted_tags.extend(table_tags.iter().copied());
    }

    // Configure allowed tags
    let mut allowed_tags: HashSet<&str> = HashSet::from_iter(HTML_TAGS.iter().copied());
    allowed_tags = allowed_tags
        .difference(&blacklisted_tags)
        .copied()
        .collect();

    // Configure allowed attributes
    let mut allowed_attributes: HashSet<&str> = HashSet::from_iter(ALLOWED_ATTRS.iter().copied());
    allowed_attributes.extend(options.allowed_attributes.iter().map(|attr| attr.as_str()));

    // Configure tag specific attributes
    let mut tag_specific_attributes = builder.clone_tag_attributes();
    for tag in blacklisted_tags.iter() {
        tag_specific_attributes.remove(tag);
    }

    builder
        .add_tags(allowed_tags)
        .add_generic_attributes(allowed_attributes)
        .tag_attributes(tag_specific_attributes)
        .clean_content_tags(blacklisted_tags)
        .link_rel(None)
        .clean(html)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_html() {
        let html = "<div>Keep</div><custom>Remove</custom><remove>Also remove</remove>";
        let sanitized = sanitize_html(html, &SanitizeOptions::default());
        assert_eq!(sanitized, "<div>Keep</div>RemoveAlso remove");
    }

    #[test]
    fn test_sanitize_malformed_html() {
        let html = "<div>Content<p>Paragraph";
        let sanitized = sanitize_html(html, &SanitizeOptions::default());
        assert_eq!(sanitized, "<div>Content<p>Paragraph</p></div>");
    }

    #[test]
    fn test_sanitize_html_remove_links() {
        let html = "<p>Text with <a href='http://example.com'>link</a></p>";
        let sanitized = sanitize_html(
            html,
            &SanitizeOptions {
                remove_links: true,
                ..Default::default()
            },
        );
        assert_eq!(sanitized, "<p>Text with </p>");
    }

    #[test]
    fn test_sanitize_html_remove_images() {
        let html = "<p>Text with <img src='http://example.com/image.jpg' alt='Image' /></p>";
        let sanitized = sanitize_html(
            html,
            &SanitizeOptions {
                remove_images: true,
                ..Default::default()
            },
        );
        assert_eq!(sanitized, "<p>Text with </p>");
    }

    #[test]
    fn test_sanitize_html_remove_tables() {
        let html = "<p>Text with </p><table><tr><td>Table</td></tr></table>";
        let sanitized = sanitize_html(
            html,
            &SanitizeOptions {
                remove_tables: true,
                ..Default::default()
            },
        );
        assert_eq!(sanitized, "<p>Text with </p>");
    }
}

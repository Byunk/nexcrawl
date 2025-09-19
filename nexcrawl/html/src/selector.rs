use crate::node::{Handle, NodeData};

/// Represents a single segment of a selector (e.g., "div.class#id")
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(test, derive())]
pub(crate) struct SelectorSegment {
    element: Option<String>,
    classes: Vec<String>,
    id: Option<String>,
}

/// Select all matching nodes in the tree
///
/// CSS-like selector utility for querying DOM nodes.
///
/// Supports:
/// - Element selectors: "div", "span", "p"
/// - Class selectors: ".className"
/// - ID selectors: "#idName"
/// - Combined selectors: "div.className#id"
/// - Descendant selectors: "div span.active"
///
/// Returns a vector of all matching nodes, or an empty vector if no matches are found.
///
/// # Examples
///
/// ```
/// use nexcrawl_html::select;
/// use nexcrawl_html::node::{Node, NodeData};
///
/// let root = Node::new(NodeData::Document);
/// let results = select(&root, "div.item");
/// ```
pub fn select(tree: &Handle, selector: &str) -> Vec<Handle> {
    if selector.trim().is_empty() {
        return Vec::new();
    }

    let segments = parse_selector_private(selector);

    if segments.is_empty() {
        return Vec::new();
    }

    let mut results = Vec::new();
    select_all_recursive(tree, &segments, 0, &mut results);
    results
}

/// Parse a selector string into structured components (private)
fn parse_selector_private(selector: &str) -> Vec<SelectorSegment> {
    parse_selector_impl(selector)
}

/// Parse a selector string into structured components (implementation)
fn parse_selector_impl(selector: &str) -> Vec<SelectorSegment> {
    let segments: Vec<&str> = selector.trim().split_whitespace().collect();

    segments.into_iter().map(|segment| {
        let mut element: Option<String> = None;
        let mut classes: Vec<String> = Vec::new();
        let mut id: Option<String> = None;

        let mut current_token = String::new();
        let mut current_type = 'e'; // 'e' for element, 'c' for class, 'i' for id

        for ch in segment.chars() {
            match ch {
                '.' => {
                    if current_type == 'e' && !current_token.is_empty() {
                        element = Some(current_token.clone());
                    } else if current_type == 'c' && !current_token.is_empty() {
                        classes.push(current_token.clone());
                    } else if current_type == 'i' && !current_token.is_empty() {
                        id = Some(current_token.clone());
                    }
                    current_token.clear();
                    current_type = 'c';
                }
                '#' => {
                    if current_type == 'e' && !current_token.is_empty() {
                        element = Some(current_token.clone());
                    } else if current_type == 'c' && !current_token.is_empty() {
                        classes.push(current_token.clone());
                    }
                    current_token.clear();
                    current_type = 'i';
                }
                _ => {
                    current_token.push(ch);
                }
            }
        }

        // Handle the last token
        if !current_token.is_empty() {
            match current_type {
                'e' => element = Some(current_token),
                'c' => classes.push(current_token),
                'i' => id = Some(current_token),
                _ => {}
            }
        }

        SelectorSegment { element, classes, id }
    }).collect()
}

/// Unified recursive function for collecting all matches (both simple and descendant selectors)
fn select_all_recursive(node: &Handle, segments: &[SelectorSegment], segment_index: usize, results: &mut Vec<Handle>) {
    if segment_index >= segments.len() {
        return;
    }

    let current_segment = &segments[segment_index];

    // Check if current node matches the current segment
    if matches_segment(node, current_segment) {
        // If this is the last segment, we found a match
        if segment_index == segments.len() - 1 {
            results.push(node.clone());
        } else {
            // Otherwise, search descendants for the next segment
            for child in node.children.borrow().iter() {
                select_all_recursive(child, segments, segment_index + 1, results);
            }
        }
    }

    // Continue searching in children for current segment
    for child in node.children.borrow().iter() {
        select_all_recursive(child, segments, segment_index, results);
    }
}

/// Check if a node matches a selector segment
fn matches_segment(node: &Handle, segment: &SelectorSegment) -> bool {
    match &node.data {
        NodeData::Element { name, attrs, .. } => {
            // Check element name match
            if let Some(ref element_name) = segment.element {
                if name.local.as_ref() != element_name {
                    return false;
                }
            }

            let borrowed_attrs = attrs.borrow();

            // Check ID match
            if let Some(ref required_id) = segment.id {
                let has_matching_id = borrowed_attrs.iter().any(|attr| {
                    attr.name.local.as_ref() == "id" && attr.value.as_ref() == required_id
                });
                if !has_matching_id {
                    return false;
                }
            }

            // Check class matches
            if !segment.classes.is_empty() {
                let class_attr = borrowed_attrs.iter().find(|attr| {
                    attr.name.local.as_ref() == "class"
                });

                if let Some(class_attr) = class_attr {
                    let node_classes: Vec<&str> = class_attr.value.split_whitespace().collect();

                    // All required classes must be present
                    for required_class in &segment.classes {
                        if !node_classes.contains(&required_class.as_str()) {
                            return false;
                        }
                    }
                } else {
                    // Node has no classes but selector requires classes
                    return false;
                }
            }

            true
        }
        _ => false, // Only elements can match selectors
    }
}

/// Get the selector string for a node
///
/// # Example
///
/// Input: <div class="test">Hello</div>
/// Output: div.test
pub fn get_selector(node: &Handle) -> Option<String> {
    match &node.data {
        NodeData::Element { name, attrs, .. } => {
            let mut selector = name.local.to_string();
            for attr in attrs.borrow().iter() {
                match attr.name.local.as_ref() {
                    "class" => {
                        let classes = attr.value.split_whitespace().collect::<Vec<&str>>();
                        for class in classes {
                            selector.push_str(&format!(".{}", class));
                        }
                    }
                    "id" => {
                        selector.push_str(&format!("#{}", attr.value));
                    }
                    _ => {}
                }
            }

            // Get the parent selector
            if let Some(weak) = node.parent.take() && let Some(parent) = weak.upgrade() {
                let parent_selector = get_selector(&parent);
                if let Some(parent_selector) = parent_selector {
                    selector = format!("{} {}", parent_selector, selector);
                }
            }

            Some(selector)
        }
        _ => None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::{Node, NodeData};
    use html5ever::{QualName, Attribute, LocalName, Namespace};
    use std::cell::RefCell;

    // Test helper function to expose parse_selector functionality
    fn parse_selector(selector: &str) -> Vec<SelectorSegment> {
        parse_selector_impl(selector)
    }

    #[test]
    fn test_get_selector_div_with_class_and_id() {
        let name = QualName::new(None, Namespace::from(""), LocalName::from("div"));
        let attrs = vec![
            Attribute {
                name: QualName::new(None, Namespace::from(""), LocalName::from("class")),
                value: "test".into(),
            },
            Attribute {
                name: QualName::new(None, Namespace::from(""), LocalName::from("id")),
                value: "myid".into(),
            },
        ];

        let node = Node::new(NodeData::Element {
            name,
            attrs: RefCell::new(attrs),
            template_contents: RefCell::new(None),
            mathml_annotation_xml_integration_point: false,
        });

        let selector = get_selector(&node);
        assert_eq!(selector, Some("div.test#myid".to_string()));
    }

    #[test]
    fn test_get_selector_with_multiple_classes() {
        let name = QualName::new(None, Namespace::from(""), LocalName::from("div"));
        let attrs = vec![
            Attribute {
                name: QualName::new(None, Namespace::from(""), LocalName::from("class")),
                value: "test1 test2".into(),
            },
        ];

        let node = Node::new(NodeData::Element {
            name,
            attrs: RefCell::new(attrs),
            template_contents: RefCell::new(None),
            mathml_annotation_xml_integration_point: false,
        });

        let selector = get_selector(&node);
        assert_eq!(selector, Some("div.test1.test2".to_string()));
    }

    #[test]
    fn test_selector_simple_element() {
        let segments = parse_selector("div");
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].element, Some("div".to_string()));
        assert!(segments[0].classes.is_empty());
        assert_eq!(segments[0].id, None);
    }

    #[test]
    fn test_selector_class_only() {
        let segments = parse_selector(".test");
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].element, None);
        assert_eq!(segments[0].classes, vec!["test"]);
        assert_eq!(segments[0].id, None);
    }

    #[test]
    fn test_selector_id_only() {
        let segments = parse_selector("#myid");
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].element, None);
        assert!(segments[0].classes.is_empty());
        assert_eq!(segments[0].id, Some("myid".to_string()));
    }

    #[test]
    fn test_selector_combined() {
        let segments = parse_selector("div.test1.test2#myid");
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].element, Some("div".to_string()));
        assert_eq!(segments[0].classes, vec!["test1", "test2"]);
        assert_eq!(segments[0].id, Some("myid".to_string()));
    }

    #[test]
    fn test_selector_descendant() {
        let segments = parse_selector("div span.active");
        assert_eq!(segments.len(), 2);

        assert_eq!(segments[0].element, Some("div".to_string()));
        assert!(segments[0].classes.is_empty());
        assert_eq!(segments[0].id, None);

        assert_eq!(segments[1].element, Some("span".to_string()));
        assert_eq!(segments[1].classes, vec!["active"]);
        assert_eq!(segments[1].id, None);
    }

    fn create_test_node(tag: &str, classes: &[&str], id: Option<&str>) -> Handle {
        let name = QualName::new(None, Namespace::from(""), LocalName::from(tag));
        let mut attrs = Vec::new();

        if !classes.is_empty() {
            attrs.push(Attribute {
                name: QualName::new(None, Namespace::from(""), LocalName::from("class")),
                value: classes.join(" ").into(),
            });
        }

        if let Some(id_val) = id {
            attrs.push(Attribute {
                name: QualName::new(None, Namespace::from(""), LocalName::from("id")),
                value: id_val.into(),
            });
        }

        Node::new(NodeData::Element {
            name,
            attrs: RefCell::new(attrs),
            template_contents: RefCell::new(None),
            mathml_annotation_xml_integration_point: false,
        })
    }

    #[test]
    fn test_matches_segment_element() {
        let node = create_test_node("div", &[], None);

        let results = select(&node, "div");
        assert!(!results.is_empty());

        let results_wrong = select(&node, "span");
        assert!(results_wrong.is_empty());
    }

    #[test]
    fn test_matches_segment_class() {
        let node = create_test_node("div", &["test", "active"], None);

        let results = select(&node, ".test");
        assert!(!results.is_empty());

        let results_multiple = select(&node, ".test.active");
        assert!(!results_multiple.is_empty());

        let results_missing = select(&node, ".missing");
        assert!(results_missing.is_empty());
    }

    #[test]
    fn test_matches_segment_id() {
        let node = create_test_node("div", &[], Some("myid"));

        let results = select(&node, "#myid");
        assert!(!results.is_empty());

        let results_wrong = select(&node, "#wrongid");
        assert!(results_wrong.is_empty());
    }

    #[test]
    fn test_matches_segment_combined() {
        let node = create_test_node("div", &["test"], Some("myid"));

        let results = select(&node, "div.test#myid");
        assert!(!results.is_empty());

        let results_wrong = select(&node, "span.test#myid");
        assert!(results_wrong.is_empty());
    }

    fn create_tree() -> Handle {
        // Create a simple tree:
        // <div id="root" class="container">
        //   <span class="item">Item 1</span>
        //   <div class="item active">
        //     <p>Paragraph</p>
        //   </div>
        //   <span class="item">Item 2</span>
        // </div>

        let root = create_test_node("div", &["container"], Some("root"));
        let span1 = create_test_node("span", &["item"], None);
        let div1 = create_test_node("div", &["item", "active"], None);
        let p = create_test_node("p", &[], None);
        let span2 = create_test_node("span", &["item"], None);

        // Build the tree structure
        div1.children.borrow_mut().push(p.clone());
        p.parent.set(Some(std::rc::Rc::downgrade(&div1)));

        root.children.borrow_mut().push(span1.clone());
        root.children.borrow_mut().push(div1.clone());
        root.children.borrow_mut().push(span2.clone());

        span1.parent.set(Some(std::rc::Rc::downgrade(&root)));
        div1.parent.set(Some(std::rc::Rc::downgrade(&root)));
        span2.parent.set(Some(std::rc::Rc::downgrade(&root)));

        root
    }

    #[test]
    fn test_select_simple_element() {
        let tree = create_tree();

        let results = select(&tree, "div");
        let result = results.first();
        assert!(result.is_some());
        let node = result.unwrap();
        if let NodeData::Element { name, attrs, .. } = &node.data {
            assert_eq!(name.local.as_ref(), "div");
            let borrowed_attrs = attrs.borrow();
            let id = borrowed_attrs.iter().find(|attr| attr.name.local.as_ref() == "id");
            assert!(id.is_some());
            assert_eq!(id.unwrap().value.as_ref(), "root");
        }
    }

    #[test]
    fn test_select_class() {
        let tree = create_tree();

        let results = select(&tree, ".item");
        let result = results.first();
        assert!(result.is_some());
        let node = result.unwrap();
        if let NodeData::Element { name, .. } = &node.data {
            assert_eq!(name.local.as_ref(), "span"); // First item should be span
        }
    }

    #[test]
    fn test_select_id() {
        let tree = create_tree();

        let results = select(&tree, "#root");
        let result = results.first();
        assert!(result.is_some());
        let node = result.unwrap();
        if let NodeData::Element { name, .. } = &node.data {
            assert_eq!(name.local.as_ref(), "div");
        }
    }

    #[test]
    fn test_select_combined() {
        let tree = create_tree();

        let results = select(&tree, "div.active");
        let result = results.first();
        assert!(result.is_some());
        let node = result.unwrap();
        if let NodeData::Element { name, attrs, .. } = &node.data {
            assert_eq!(name.local.as_ref(), "div");
            let borrowed_attrs = attrs.borrow();
            let class_attr = borrowed_attrs.iter()
                .find(|attr| attr.name.local.as_ref() == "class")
                .unwrap();
            let classes: Vec<&str> = class_attr.value.split_whitespace().collect();
            assert!(classes.contains(&"active"));
            assert!(classes.contains(&"item"));
        }
    }

    #[test]
    fn test_select_descendant() {
        let tree = create_tree();

        let results = select(&tree, "div p");
        let result = results.first();
        assert!(result.is_some());
        let node = result.unwrap();
        if let NodeData::Element { name, .. } = &node.data {
            assert_eq!(name.local.as_ref(), "p");
        }
    }

    #[test]
    fn test_select_not_found() {
        let tree = create_tree();

        let results = select(&tree, "table");
        let result = results.first();
        assert!(result.is_none());

        let results = select(&tree, ".nonexistent");
        let result = results.first();
        assert!(result.is_none());

        let results = select(&tree, "#nonexistent");
        let result = results.first();
        assert!(result.is_none());
    }

    #[test]
    fn test_select_all_class() {
        let tree = create_tree();

        let results = select(&tree, ".item");
        assert_eq!(results.len(), 3); // 2 spans + 1 div with class "item"
    }

    #[test]
    fn test_select_all_element() {
        let tree = create_tree();

        let results = select(&tree, "span");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_select_all_empty() {
        let tree = create_tree();

        let results = select(&tree, "table");
        assert!(results.is_empty());
    }

    #[test]
    fn test_select_empty_selector() {
        let tree = create_tree();

        let results = select(&tree, "");
        let result = results.first();
        assert!(result.is_none());

        let results = select(&tree, "   ");
        let result = results.first();
        assert!(result.is_none());

        let results = select(&tree, "");
        assert!(results.is_empty());
    }
}

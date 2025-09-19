//! Extract the minimum DOM tree from the HTML

use std::cell::RefCell;
use std::collections::HashMap;

use crate::node::{Handle, NodeData};

pub struct MinimumDomTree {
    cache: RefCell<HashMap<Handle, String>>,
}

impl Default for MinimumDomTree {
    fn default() -> Self {
        Self::new()
    }
}

impl MinimumDomTree {
    pub fn new() -> Self {
        Self {
            cache: RefCell::new(HashMap::new()),
        }
    }

    pub fn build(&self, tree: &Handle, text: &str) -> Option<Vec<Handle>> {
        // If the text cannot be extracted from the tree, return empty
        if text.is_empty() || !self.is_subset(text, self.get_text(tree).as_str()) {
            return None;
        }

        let mdt= self.minimum_dom_tree(tree, text);
        // Clear the cache
        self.cache.borrow_mut().clear();
        mdt
    }

    fn minimum_dom_tree(&self, node: &Handle, text: &str) -> Option<Vec<Handle>> {
        if text.is_empty() {
            return None;
        }

        let node_text = self.get_text(node);

        // If the text is subset of the node, continue traversal
        // If the node is subset of the text, it should be included in the minimum DOM tree
        // Else, return None
        let text_subset_of_node = self.is_subset(text, &node_text);
        let node_subset_of_text = self.is_subset(&node_text, text);

        if !text_subset_of_node && !node_subset_of_text {
            return None;
        }

        let mut nodes = Vec::new();
        for child in node.children.borrow().iter() {
            if let Some(mdt) = self.minimum_dom_tree(child, text) {
                nodes.extend(mdt);
            }
        }

        if nodes.is_empty() {
            // If no children returned nodes, but this node is a subset of the target text,
            // then this node should be included in the minimum DOM tree
            if node_subset_of_text {
                return Some(vec![node.clone()]);
            } else {
                return None;
            }
        }

        // If all children are included in the nodes, they are selected with parent node instead of themselves
        let mut can_merge = true;
        for child in node.children.borrow().iter() {
            if !nodes.contains(child) {
                can_merge = false;
                break;
            }
        }

        if can_merge {
            // Remove all children from the nodes
            let mut new_nodes = Vec::new();
            for elem in nodes.iter() {
                if !node.children.borrow().contains(elem) {
                    new_nodes.push(elem.clone());
                }
            }
            new_nodes.push(node.clone());
            nodes = new_nodes;
        }

        Some(nodes)
    }

    fn get_text(&self, node: &Handle) -> String {
        if let Some(text) = self.cache.borrow().get(node) {
            return text.clone();
        }

        match &node.data {
            NodeData::Text { text } => text.borrow().clone().to_string(),
            _ => {
                let mut texts = Vec::new();
                for child in node.children.borrow().iter() {
                    let text = self.get_text(child);
                    texts.push(text);
                }
                let joined_text = texts.join(" ");
                self.cache
                    .borrow_mut()
                    .insert(node.clone(), joined_text.clone());
                joined_text
            }
        }
    }

    /// Check if the text t1 is a subset of t2
    fn is_subset(&self, t1: &str, t2: &str) -> bool {
        let tokens1: Vec<&str> = t1.split_whitespace().collect();
        let tokens2: Vec<&str> = t2.split_whitespace().collect();

        let mut i = 0;
        let mut j = 0;

        while i < tokens1.len() && j < tokens2.len() {
            if tokens1[i] == tokens2[j] {
                i += 1;
            }
            j += 1;
        }

        i == tokens1.len()
    }
}

#[cfg(test)]
mod tests {
    use crate::node::Node;

    use super::*;

    #[test]
    fn test_minimum_dom_tree() {
        use html5ever::QualName;
        use html5ever::tendril::StrTendril;
        use std::str::FromStr;

        // Create a DOM tree with height = 4
        // Structure:
        // div (root)
        //   ├── p
        //   │   ├── span "Hello"
        //   │   │   └── b "world" (height 3)
        //   │   └── "from" (height 2)
        //   └── div
        //       ├── "the" (height 2)
        //       └── em "test"
        //           └── "tree" (height 3)

        // Create root div element
        let root = Node::new(NodeData::Element {
            name: QualName::new(None, html5ever::ns!(html), html5ever::local_name!("div")),
            attrs: std::cell::RefCell::new(Vec::new()),
            template_contents: std::cell::RefCell::new(None),
            mathml_annotation_xml_integration_point: false,
        });

        // Create first p element
        let p_elem = Node::new(NodeData::Element {
            name: QualName::new(None, html5ever::ns!(html), html5ever::local_name!("p")),
            attrs: std::cell::RefCell::new(Vec::new()),
            template_contents: std::cell::RefCell::new(None),
            mathml_annotation_xml_integration_point: false,
        });

        // Create span element with "Hello" text
        let span_elem = Node::new(NodeData::Element {
            name: QualName::new(None, html5ever::ns!(html), html5ever::local_name!("span")),
            attrs: std::cell::RefCell::new(Vec::new()),
            template_contents: std::cell::RefCell::new(None),
            mathml_annotation_xml_integration_point: false,
        });

        let hello_text = Node::new(NodeData::Text {
            text: std::cell::RefCell::new(StrTendril::from_str("Hello").unwrap()),
        });

        // Create b element with "world" text (height 3)
        let b_elem = Node::new(NodeData::Element {
            name: QualName::new(None, html5ever::ns!(html), html5ever::local_name!("b")),
            attrs: std::cell::RefCell::new(Vec::new()),
            template_contents: std::cell::RefCell::new(None),
            mathml_annotation_xml_integration_point: false,
        });

        let world_text = Node::new(NodeData::Text {
            text: std::cell::RefCell::new(StrTendril::from_str("world").unwrap()),
        });

        // Create "from" text node
        let from_text = Node::new(NodeData::Text {
            text: std::cell::RefCell::new(StrTendril::from_str("from").unwrap()),
        });

        // Create second div element
        let div2_elem = Node::new(NodeData::Element {
            name: QualName::new(None, html5ever::ns!(html), html5ever::local_name!("div")),
            attrs: std::cell::RefCell::new(Vec::new()),
            template_contents: std::cell::RefCell::new(None),
            mathml_annotation_xml_integration_point: false,
        });

        // Create "the" text node
        let the_text = Node::new(NodeData::Text {
            text: std::cell::RefCell::new(StrTendril::from_str("the").unwrap()),
        });

        // Create em element with "test" text
        let em_elem = Node::new(NodeData::Element {
            name: QualName::new(None, html5ever::ns!(html), html5ever::local_name!("em")),
            attrs: std::cell::RefCell::new(Vec::new()),
            template_contents: std::cell::RefCell::new(None),
            mathml_annotation_xml_integration_point: false,
        });

        let test_text = Node::new(NodeData::Text {
            text: std::cell::RefCell::new(StrTendril::from_str("test").unwrap()),
        });

        // Create "tree" text node (height 3)
        let tree_text = Node::new(NodeData::Text {
            text: std::cell::RefCell::new(StrTendril::from_str("tree").unwrap()),
        });

        // Build the tree structure
        // Set parent relationships and add children

        // b element contains "world" text
        b_elem.children.borrow_mut().push(world_text.clone());
        world_text.parent.set(Some(std::rc::Rc::downgrade(&b_elem)));

        // span element contains "Hello" text and b element
        span_elem.children.borrow_mut().push(hello_text.clone());
        span_elem.children.borrow_mut().push(b_elem.clone());
        hello_text
            .parent
            .set(Some(std::rc::Rc::downgrade(&span_elem)));
        b_elem.parent.set(Some(std::rc::Rc::downgrade(&span_elem)));

        // p element contains span and "from" text
        p_elem.children.borrow_mut().push(span_elem.clone());
        p_elem.children.borrow_mut().push(from_text.clone());
        span_elem.parent.set(Some(std::rc::Rc::downgrade(&p_elem)));
        from_text.parent.set(Some(std::rc::Rc::downgrade(&p_elem)));

        // em element contains "test" text and "tree" text
        em_elem.children.borrow_mut().push(test_text.clone());
        em_elem.children.borrow_mut().push(tree_text.clone());
        test_text.parent.set(Some(std::rc::Rc::downgrade(&em_elem)));
        tree_text.parent.set(Some(std::rc::Rc::downgrade(&em_elem)));

        // div2 element contains "the" text and em element
        div2_elem.children.borrow_mut().push(the_text.clone());
        div2_elem.children.borrow_mut().push(em_elem.clone());
        the_text
            .parent
            .set(Some(std::rc::Rc::downgrade(&div2_elem)));
        em_elem.parent.set(Some(std::rc::Rc::downgrade(&div2_elem)));

        // root div contains p and div2
        root.children.borrow_mut().push(p_elem.clone());
        root.children.borrow_mut().push(div2_elem.clone());
        p_elem.parent.set(Some(std::rc::Rc::downgrade(&root)));
        div2_elem.parent.set(Some(std::rc::Rc::downgrade(&root)));

        // Test the minimum DOM tree extraction
        let min_dom_tree = MinimumDomTree::new();
        let target_text = "Hello world from test tree"; // "the" is not included

        let result = min_dom_tree.build(&root, target_text);

        // The result should not be None and should contain the root node
        // since it contains all the text
        assert!(result.is_some());
        let nodes = result.unwrap();
        assert!(!nodes.is_empty());

        // Verify that all text can be extracted from the returned nodes
        let mut extracted_texts = Vec::new();
        for node in &nodes {
            let text = min_dom_tree.get_text(node);
            extracted_texts.push(text);
        }
        let extracted_text = extracted_texts.join(" ");

        // Check that the extracted text is the same as the target text
        assert_eq!(extracted_text, target_text);
    }

    #[test]
    fn test_is_subset() {
        let min_dom_tree = MinimumDomTree::new();
        assert!(min_dom_tree.is_subset("Hello world", "Hello world"));
        assert!(min_dom_tree.is_subset("Hello world", "Hello world from"));
        assert!(min_dom_tree.is_subset("Hello world from", "Hello world from"));
        assert!(min_dom_tree.is_subset("Hello world from", "Hello world from test"));
        assert!(min_dom_tree.is_subset("Hello world from test", "Hello world from test tree"));
        assert!(min_dom_tree.is_subset("Hello test tree", "Hello world from test tree"));
    }
}

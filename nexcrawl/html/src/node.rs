//! A simple reference-counted DOM.
//!
//! This is sufficient as a static parse tree, but don't build a
//! web browser using it. :)
//!
//! A DOM is a [tree structure] with ordered children that can be represented in an XML-like
//! format. For example, the following graph
//!
//! ```text
//! div
//!  +- "text node"
//!  +- span
//! ```
//! in HTML would be serialized as
//!
//! ```html
//! <div>text node<span></span></div>
//! ```
//!
//! See the [document object model article on wikipedia][dom wiki] for more information.
//!
//! This implementation stores the information associated with each node once, and then hands out
//! refs to children. The nodes themselves are reference-counted to avoid copying - you can create
//! a new ref and then a node will outlive the document. Nodes own their children, but only have
//! weak references to their parents.
//!
//! [tree structure]: https://en.wikipedia.org/wiki/Tree_(data_structure)
//! [dom wiki]: https://en.wikipedia.org/wiki/Document_Object_Model

use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::collections::{HashSet, VecDeque};
use std::default::Default;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::io;
use std::mem;
use std::rc::{Rc, Weak};
use std::str::FromStr;

use html5ever::interface::{ElementFlags, NodeOrText, QuirksMode, TreeSink};
use html5ever::serialize::{Serialize, SerializeOpts, Serializer, TraversalScope, serialize};
use html5ever::tendril::StrTendril;
use html5ever::{Attribute, ExpandedName, QualName};

/// Reference to a DOM node.
pub type Handle = Rc<Node>;

/// Weak reference to a DOM node, used for parent pointers.
pub type WeakHandle = Weak<Node>;

/// The different kinds of nodes in the DOM.
#[derive(Debug, PartialEq, Eq)]
pub enum NodeData {
    /// The `Document` itself - the root node of a HTML document.
    Document,

    /// A `DOCTYPE` with name, public id, and system id. See
    /// [document type declaration on wikipedia][dtd wiki].
    ///
    /// [dtd wiki]: https://en.wikipedia.org/wiki/Document_type_declaration
    Doctype {
        name: StrTendril,
        public_id: StrTendril,
        system_id: StrTendril,
    },

    /// A text node.
    Text { text: RefCell<StrTendril> },

    /// A comment.
    Comment { comment: StrTendril },

    /// An element with attributes.
    Element {
        name: QualName,
        attrs: RefCell<Vec<Attribute>>,

        /// For HTML \<template\> elements, the [template contents].
        ///
        /// [template contents]: https://html.spec.whatwg.org/multipage/#template-contents
        template_contents: RefCell<Option<Handle>>,

        /// Whether the node is a [HTML integration point].
        ///
        /// [HTML integration point]: https://html.spec.whatwg.org/multipage/#html-integration-point
        mathml_annotation_xml_integration_point: bool,
    },

    /// A Processing instruction.
    ProcessingInstruction {
        target: StrTendril,
        data: StrTendril,
    },
}

impl Hash for NodeData {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // First hash the discriminant (which variant it is)
        std::mem::discriminant(self).hash(state);

        match self {
            NodeData::Document => {}
            NodeData::Doctype {
                name,
                public_id,
                system_id,
            } => {
                name.hash(state);
                public_id.hash(state);
                system_id.hash(state);
            }
            NodeData::Text { text } => {
                text.borrow().hash(state); // Borrow and hash the inner value
            }
            NodeData::Comment { comment } => {
                comment.hash(state);
            }
            NodeData::Element {
                name,
                attrs,
                template_contents,
                mathml_annotation_xml_integration_point,
            } => {
                name.hash(state);
                attrs.borrow().iter().for_each(|attr| {
                    attr.name.hash(state);
                    attr.value.hash(state);
                });
                template_contents
                    .borrow()
                    .as_ref()
                    .map(Rc::as_ptr)
                    .hash(state);
                mathml_annotation_xml_integration_point.hash(state);
            }
            NodeData::ProcessingInstruction { target, data } => {
                target.hash(state);
                data.hash(state);
            }
        }
    }
}

/// A DOM node.
pub struct Node {
    /// Parent node.
    pub parent: Cell<Option<WeakHandle>>,
    /// Child nodes of this node.
    pub children: RefCell<Vec<Handle>>,
    /// Represents this node's data.
    pub data: NodeData,
}

impl Node {
    /// Create a new node from its contents
    pub fn new(data: NodeData) -> Rc<Self> {
        Rc::new(Node {
            data,
            parent: Cell::new(None),
            children: RefCell::new(Vec::new()),
        })
    }

    pub fn new_text(text: String) -> Rc<Self> {
        Node::new(NodeData::Text {
            text: RefCell::new(StrTendril::from_str(&text).unwrap_or_default()),
        })
    }

    /// Deep copy this node and all its descendants with fresh references
    pub fn deep_copy(self: &Rc<Self>) -> Rc<Self> {
        // Create a new node with the same data
        let new_node = match &self.data {
            NodeData::Text { text } => Node::new(NodeData::Text {
                text: RefCell::new(text.borrow().clone()),
            }),
            NodeData::Element {
                name,
                attrs,
                template_contents,
                mathml_annotation_xml_integration_point,
            } => Node::new(NodeData::Element {
                name: name.clone(),
                attrs: RefCell::new(attrs.borrow().clone()),
                template_contents: RefCell::new(template_contents.borrow().clone()),
                mathml_annotation_xml_integration_point: *mathml_annotation_xml_integration_point,
            }),
            NodeData::Document => Node::new(NodeData::Document),
            NodeData::Comment { comment } => Node::new(NodeData::Comment {
                comment: comment.clone(),
            }),
            NodeData::Doctype {
                name,
                public_id,
                system_id,
            } => Node::new(NodeData::Doctype {
                name: name.clone(),
                public_id: public_id.clone(),
                system_id: system_id.clone(),
            }),
            NodeData::ProcessingInstruction { target, data } => {
                Node::new(NodeData::ProcessingInstruction {
                    target: target.clone(),
                    data: data.clone(),
                })
            }
        };

        // Clear parent reference (it will be set by the caller)
        new_node.parent.set(None);

        // Recursively copy all children
        let children = self.children.borrow();
        let mut new_children = Vec::new();
        for child in children.iter() {
            let new_child = child.deep_copy();
            new_child.parent.set(Some(Rc::downgrade(&new_node)));
            new_children.push(new_child);
        }
        new_node.children.replace(new_children);

        new_node
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        let mut nodes = mem::take(&mut *self.children.borrow_mut());
        while let Some(node) = nodes.pop() {
            let children = mem::take(&mut *node.children.borrow_mut());
            nodes.extend(children.into_iter());
        }
    }
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

impl Eq for Node {}

impl Hash for Node {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.data.hash(state);
    }
}

impl fmt::Debug for Node {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Node")
            .field("data", &self.data)
            .field("children", &self.children)
            .finish()
    }
}

/// Append a parentless node to another nodes' children
fn append(new_parent: &Handle, child: Handle) {
    let previous_parent = child.parent.replace(Some(Rc::downgrade(new_parent)));
    // Invariant: child cannot have existing parent
    assert!(previous_parent.is_none());
    new_parent.children.borrow_mut().push(child);
}

/// If the node has a parent, get it and this node's position in its children
fn get_parent_and_index(target: &Handle) -> Option<(Handle, usize)> {
    if let Some(weak) = target.parent.take() {
        let parent = weak.upgrade().expect("dangling weak pointer");
        target.parent.set(Some(weak));
        let i = match parent
            .children
            .borrow()
            .iter()
            .enumerate()
            .find(|&(_, child)| Rc::ptr_eq(child, target))
        {
            Some((i, _)) => i,
            None => panic!("have parent but couldn't find in parent's children!"),
        };
        Some((parent, i))
    } else {
        None
    }
}

fn append_to_existing_text(prev: &Handle, text: &str) -> bool {
    match prev.data {
        NodeData::Text { text: ref t } => {
            t.borrow_mut().push_slice(text);
            true
        }
        _ => false,
    }
}

fn remove_from_parent(target: &Handle) {
    if let Some((parent, i)) = get_parent_and_index(target) {
        parent.children.borrow_mut().remove(i);
        target.parent.set(None);
    }
}

/// The DOM itself; the result of parsing.
pub struct Dom {
    /// The `Document` itself.
    pub tree: Handle,

    /// Errors that occurred during parsing.
    pub errors: RefCell<Vec<Cow<'static, str>>>,

    /// The document's quirks mode.
    pub quirks_mode: Cell<QuirksMode>,
}

impl TreeSink for Dom {
    type Handle = Handle;
    type Output = Self;
    type ElemName<'a>
        = ExpandedName<'a>
    where
        Self: 'a;

    fn finish(self) -> Self {
        self
    }

    fn parse_error(&self, msg: Cow<'static, str>) {
        self.errors.borrow_mut().push(msg);
    }

    fn get_document(&self) -> Handle {
        self.tree.clone()
    }

    fn get_template_contents(&self, target: &Handle) -> Handle {
        if let NodeData::Element {
            ref template_contents,
            ..
        } = target.data
        {
            template_contents
                .borrow()
                .as_ref()
                .expect("not a template element!")
                .clone()
        } else {
            panic!("not a template element!")
        }
    }

    fn set_quirks_mode(&self, mode: QuirksMode) {
        self.quirks_mode.set(mode);
    }

    fn same_node(&self, x: &Handle, y: &Handle) -> bool {
        Rc::ptr_eq(x, y)
    }

    fn elem_name<'a>(&self, target: &'a Handle) -> ExpandedName<'a> {
        match target.data {
            NodeData::Element { ref name, .. } => name.expanded(),
            _ => panic!("not an element!"),
        }
    }

    fn create_element(&self, name: QualName, attrs: Vec<Attribute>, flags: ElementFlags) -> Handle {
        Node::new(NodeData::Element {
            name,
            attrs: RefCell::new(attrs),
            template_contents: RefCell::new(if flags.template {
                Some(Node::new(NodeData::Document))
            } else {
                None
            }),
            mathml_annotation_xml_integration_point: flags.mathml_annotation_xml_integration_point,
        })
    }

    fn create_comment(&self, text: StrTendril) -> Handle {
        Node::new(NodeData::Comment { comment: text })
    }

    fn create_pi(&self, target: StrTendril, data: StrTendril) -> Handle {
        Node::new(NodeData::ProcessingInstruction { target, data })
    }

    fn append(&self, parent: &Handle, child: NodeOrText<Handle>) {
        // Append to an existing Text node if we have one.
        if let NodeOrText::AppendText(text) = &child
            && let Some(h) = parent.children.borrow().last()
            && append_to_existing_text(h, text)
        {
            return;
        }

        append(
            parent,
            match child {
                NodeOrText::AppendText(text) => Node::new(NodeData::Text {
                    text: RefCell::new(text),
                }),
                NodeOrText::AppendNode(node) => node,
            },
        );
    }

    fn append_before_sibling(&self, sibling: &Handle, child: NodeOrText<Handle>) {
        let (parent, i) = get_parent_and_index(sibling)
            .expect("append_before_sibling called on node without parent");

        let child = match (child, i) {
            // No previous node.
            (NodeOrText::AppendText(text), 0) => Node::new(NodeData::Text {
                text: RefCell::new(text),
            }),

            // Look for a text node before the insertion point.
            (NodeOrText::AppendText(text), i) => {
                let children = parent.children.borrow();
                let prev = &children[i - 1];
                if append_to_existing_text(prev, &text) {
                    return;
                }
                Node::new(NodeData::Text {
                    text: RefCell::new(text),
                })
            }

            // The tree builder promises we won't have a text node after
            // the insertion point.

            // Any other kind of node.
            (NodeOrText::AppendNode(node), _) => node,
        };

        remove_from_parent(&child);

        child.parent.set(Some(Rc::downgrade(&parent)));
        parent.children.borrow_mut().insert(i, child);
    }

    fn append_based_on_parent_node(
        &self,
        element: &Self::Handle,
        prev_element: &Self::Handle,
        child: NodeOrText<Self::Handle>,
    ) {
        let parent = element.parent.take();
        let has_parent = parent.is_some();
        element.parent.set(parent);

        if has_parent {
            self.append_before_sibling(element, child);
        } else {
            self.append(prev_element, child);
        }
    }

    fn append_doctype_to_document(
        &self,
        name: StrTendril,
        public_id: StrTendril,
        system_id: StrTendril,
    ) {
        append(
            &self.tree,
            Node::new(NodeData::Doctype {
                name,
                public_id,
                system_id,
            }),
        );
    }

    fn add_attrs_if_missing(&self, target: &Handle, attrs: Vec<Attribute>) {
        let mut existing = if let NodeData::Element { ref attrs, .. } = target.data {
            attrs.borrow_mut()
        } else {
            panic!("not an element")
        };

        let existing_names = existing
            .iter()
            .map(|e| e.name.clone())
            .collect::<HashSet<_>>();
        existing.extend(
            attrs
                .into_iter()
                .filter(|attr| !existing_names.contains(&attr.name)),
        );
    }

    fn remove_from_parent(&self, target: &Handle) {
        remove_from_parent(target);
    }

    fn reparent_children(&self, node: &Handle, new_parent: &Handle) {
        let mut children = node.children.borrow_mut();
        let mut new_children = new_parent.children.borrow_mut();
        for child in children.iter() {
            let previous_parent = child.parent.replace(Some(Rc::downgrade(new_parent)));
            assert!(Rc::ptr_eq(
                node,
                &previous_parent.unwrap().upgrade().expect("dangling weak")
            ))
        }
        new_children.extend(mem::take(&mut *children));
    }
}

impl Default for Dom {
    fn default() -> Dom {
        Dom {
            tree: Node::new(NodeData::Document),
            errors: Default::default(),
            quirks_mode: Cell::new(QuirksMode::NoQuirks),
        }
    }
}

enum SerializeOp {
    Open(Handle),
    Close(QualName),
}

pub struct SerializableHandle(Handle);

impl From<Handle> for SerializableHandle {
    fn from(h: Handle) -> SerializableHandle {
        SerializableHandle(h)
    }
}

impl Serialize for SerializableHandle {
    fn serialize<S>(&self, serializer: &mut S, traversal_scope: TraversalScope) -> io::Result<()>
    where
        S: Serializer,
    {
        let mut ops = VecDeque::new();
        match traversal_scope {
            TraversalScope::IncludeNode => ops.push_back(SerializeOp::Open(self.0.clone())),
            TraversalScope::ChildrenOnly(_) => ops.extend(
                self.0
                    .children
                    .borrow()
                    .iter()
                    .map(|h| SerializeOp::Open(h.clone())),
            ),
        }

        while let Some(op) = ops.pop_front() {
            match op {
                SerializeOp::Open(handle) => match handle.data {
                    NodeData::Element {
                        ref name,
                        ref attrs,
                        ..
                    } => {
                        serializer.start_elem(
                            name.clone(),
                            attrs.borrow().iter().map(|at| (&at.name, &at.value[..])),
                        )?;

                        ops.reserve(1 + handle.children.borrow().len());
                        ops.push_front(SerializeOp::Close(name.clone()));

                        for child in handle.children.borrow().iter().rev() {
                            ops.push_front(SerializeOp::Open(child.clone()));
                        }
                    }

                    NodeData::Doctype { ref name, .. } => serializer.write_doctype(name)?,

                    NodeData::Text { ref text } => serializer.write_text(&text.borrow())?,

                    NodeData::Comment { ref comment } => serializer.write_comment(comment)?,

                    NodeData::ProcessingInstruction {
                        ref target,
                        ref data,
                    } => serializer.write_processing_instruction(target, data)?,

                    NodeData::Document => panic!("Can't serialize Document node itself"),
                },

                SerializeOp::Close(name) => {
                    serializer.end_elem(name)?;
                }
            }
        }

        Ok(())
    }
}

pub fn serialize_to_string(node: &Handle) -> String {
    let mut output = Vec::new();
    let serialize_opts = SerializeOpts::default();
    let serializable = SerializableHandle::from(node.clone());
    serialize(&mut output, &serializable, serialize_opts).unwrap();
    String::from_utf8(output).unwrap()
}

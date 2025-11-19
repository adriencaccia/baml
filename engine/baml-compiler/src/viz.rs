use baml_viz_events::RuntimeNodeType;
use baml_vm::VizNodeMeta;

/// Representation of a viz node during compilation.
#[derive(Clone, Debug)]
pub struct VizNode {
    pub node_id: u32,
    pub id: String,
    pub parent: Option<String>,
    pub node_type: RuntimeNodeType,
    pub label: String,
    pub header_level: Option<u8>,
}

/// Accumulator for viz nodes within a single function.
#[derive(Clone, Debug, Default)]
pub struct VizNodes {
    nodes: Vec<VizNode>,
}

impl VizNodes {
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn push(&mut self, node: VizNode) -> usize {
        let idx = self.nodes.len();
        self.nodes.push(node);
        idx
    }

    pub fn into_vec(self) -> Vec<VizNode> {
        self.nodes
    }

    pub fn into_vm_meta(self) -> Vec<VizNodeMeta> {
        self.nodes
            .into_iter()
            .map(|node| VizNodeMeta {
                node_id: node.node_id,
                id: node.id,
                parent: node.parent,
                node_type: node.node_type,
                label: node.label,
                header_level: node.header_level,
            })
            .collect()
    }
}

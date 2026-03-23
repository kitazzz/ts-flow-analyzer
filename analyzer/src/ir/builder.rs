use super::graph::*;
use std::collections::BTreeMap;

pub struct GraphBuilder {
    next_id: u32,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

impl GraphBuilder {
    pub fn new() -> Self {
        Self {
            next_id: 0,
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    pub fn add_node(
        &mut self,
        kind: NodeKind,
        label: String,
        symbol_name: Option<String>,
        loc: SourceLoc,
    ) -> NodeId {
        let id = NodeId(self.next_id);
        self.next_id += 1;
        self.nodes.push(GraphNode {
            id,
            kind,
            label,
            symbol_name,
            loc,
            properties: BTreeMap::new(),
        });
        id
    }

    pub fn add_edge(
        &mut self,
        source: NodeId,
        target: NodeId,
        kind: EdgeKind,
        label: Option<String>,
    ) {
        self.edges.push(GraphEdge {
            source,
            target,
            kind,
            label,
            properties: BTreeMap::new(),
        });
    }

    pub fn build(self, file_path: String) -> GraphIR {
        GraphIR {
            file_path,
            nodes: self.nodes,
            edges: self.edges,
        }
    }
}

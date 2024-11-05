use fixedbitset::FixedBitSet;
use ironplc_dsl::common::Type;
use petgraph::{
    stable_graph::{NodeIndex, StableDiGraph},
    visit::Dfs,
};
use std::collections::HashMap;

pub type SymbolNode = NodeIndex;

pub struct SymbolGraph<N> {
    graph: StableDiGraph<(), (), u32>,
    nodes: HashMap<Type, (SymbolNode, N)>,
}

impl<N> SymbolGraph<N> {
    pub fn new() -> Self {
        Self {
            graph: StableDiGraph::new(),
            nodes: HashMap::new(),
        }
    }

    pub fn add_node(&mut self, id: &Type, data: N) -> SymbolNode {
        let nodes = &self.nodes;
        match nodes.get(id) {
            Some(node_and_data) => node_and_data.0,
            None => {
                let node = self.graph.add_node(());
                self.nodes.insert(id.clone(), (node, data));
                node
            }
        }
    }

    /// Returns a reference to the key and value corresponding to the key.
    ///
    /// Returning the key matters because objects such as Id's can be equal
    /// even if not identical.
    pub fn get_node(&self, id: &Type) -> Option<(&Type, &SymbolNode)> {
        self.nodes.get_key_value(id).map(|kv| (kv.0, &kv.1 .0))
    }

    pub fn data(&self, id: &Type) -> Option<&N> {
        match self.nodes.get(id) {
            Some(node_and_data) => Some(&node_and_data.1),
            None => None,
        }
    }

    pub fn add_edge(&mut self, from: SymbolNode, to: SymbolNode) {
        self.graph.add_edge(from, to, ());
    }

    pub fn dfs(&self, start: SymbolNode) -> SymbolDfs {
        SymbolDfs::new(&self.graph, start)
    }
}

pub struct SymbolDfs {
    dfs: Dfs<SymbolNode, FixedBitSet>,
}

impl SymbolDfs {
    fn new(graph: &StableDiGraph<(), (), u32>, start: SymbolNode) -> Self {
        Self {
            dfs: Dfs::new(graph, start),
        }
    }
    pub fn next<N>(&mut self, graph: &SymbolGraph<N>) -> Option<SymbolNode> {
        self.dfs.next(&graph.graph)
    }
}

//! Dependency graph for strategy execution ordering

use crate::Result;
use std::collections::{HashMap, HashSet};

/// Dependency graph for resolving execution order
#[derive(Debug)]
pub struct DependencyGraph {
    /// Nodes in the graph (strategy ID -> priority)
    nodes: HashMap<String, usize>,

    /// Edges: dependency -> dependents
    /// If A depends on B, then edges[B] contains A
    edges: HashMap<String, Vec<String>>,

    /// Reverse edges for easy lookup
    /// If A depends on B, then reverse_edges[A] contains B
    reverse_edges: HashMap<String, Vec<String>>,
}

impl DependencyGraph {
    /// Create a new empty dependency graph
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            reverse_edges: HashMap::new(),
        }
    }

    /// Add a node to the graph
    pub fn add_node(&mut self, id: String, priority: usize) {
        self.nodes.insert(id.clone(), priority);
        self.edges.entry(id.clone()).or_default();
        self.reverse_edges.entry(id).or_default();
    }

    /// Add a dependency edge
    /// `from` depends on `to` (i.e., `to` must execute before `from`)
    pub fn add_edge(&mut self, from: String, to: String) {
        // Ensure both nodes exist
        if !self.nodes.contains_key(&from) {
            self.add_node(from.clone(), 999);
        }
        if !self.nodes.contains_key(&to) {
            self.add_node(to.clone(), 999);
        }

        // Add edge: to -> from (to must execute before from)
        self.edges.entry(to.clone()).or_default().push(from.clone());

        // Add reverse edge for lookup
        self.reverse_edges.entry(from).or_default().push(to);
    }

    /// Check for cycles in the dependency graph
    pub fn has_cycle(&self) -> bool {
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();

        for node in self.nodes.keys() {
            if self.has_cycle_util(node, &mut visited, &mut rec_stack) {
                return true;
            }
        }

        false
    }

    fn has_cycle_util(
        &self,
        node: &str,
        visited: &mut HashSet<String>,
        rec_stack: &mut HashSet<String>,
    ) -> bool {
        if rec_stack.contains(node) {
            return true;
        }

        if visited.contains(node) {
            return false;
        }

        visited.insert(node.to_string());
        rec_stack.insert(node.to_string());

        if let Some(deps) = self.reverse_edges.get(node) {
            for dep in deps {
                if self.has_cycle_util(dep, visited, rec_stack) {
                    return true;
                }
            }
        }

        rec_stack.remove(node);
        false
    }

    /// Perform topological sort with priority ordering
    /// Returns nodes in execution order (dependencies first)
    pub fn topological_sort(&self) -> Result<Vec<String>> {
        // Check for cycles first
        if self.has_cycle() {
            anyhow::bail!("Circular dependency detected in strategy graph");
        }

        // Calculate in-degrees (how many dependencies each node has)
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        for node in self.nodes.keys() {
            in_degree.insert(node.clone(), 0);
        }

        // Count incoming edges for each node
        for (node, deps) in &self.reverse_edges {
            for _dep in deps {
                *in_degree.entry(node.clone()).or_insert(0) += 1;
            }
        }

        // Priority queue: nodes with 0 in-degree, sorted by priority
        let mut queue: Vec<(usize, String)> = in_degree
            .iter()
            .filter(|(_, &degree)| degree == 0)
            .map(|(node, _)| {
                let priority = self.nodes.get(node).copied().unwrap_or(999);
                (priority, node.clone())
            })
            .collect();

        // Sort by priority (lower number = higher priority = earlier execution)
        queue.sort_by_key(|(priority, _)| *priority);

        let mut result = Vec::new();

        while !queue.is_empty() {
            // Pop the highest priority node
            let (_, node) = queue.remove(0);
            result.push(node.clone());

            // Reduce in-degree of dependent nodes
            if let Some(dependents) = self.edges.get(&node) {
                for dependent in dependents {
                    let degree = in_degree.get_mut(dependent).unwrap();
                    *degree -= 1;

                    if *degree == 0 {
                        let priority = self.nodes.get(dependent).copied().unwrap_or(999);
                        queue.push((priority, dependent.clone()));
                        // Re-sort to maintain priority order
                        queue.sort_by_key(|(p, _)| *p);
                    }
                }
            }
        }

        // Check if all nodes were processed
        if result.len() != self.nodes.len() {
            anyhow::bail!("Graph contains unreachable nodes");
        }

        Ok(result)
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_graph() {
        let mut graph = DependencyGraph::new();

        // A depends on B
        graph.add_node("A".to_string(), 200);
        graph.add_node("B".to_string(), 100);
        graph.add_edge("A".to_string(), "B".to_string());

        let order = graph.topological_sort().unwrap();

        // B should come before A
        let b_pos = order.iter().position(|x| x == "B").unwrap();
        let a_pos = order.iter().position(|x| x == "A").unwrap();
        assert!(b_pos < a_pos);
    }

    #[test]
    fn test_priority_ordering() {
        let mut graph = DependencyGraph::new();

        // Three independent nodes with different priorities
        graph.add_node("high".to_string(), 100);
        graph.add_node("medium".to_string(), 200);
        graph.add_node("low".to_string(), 300);

        let order = graph.topological_sort().unwrap();

        // Should be ordered by priority
        assert_eq!(order[0], "high");
        assert_eq!(order[1], "medium");
        assert_eq!(order[2], "low");
    }

    #[test]
    fn test_dependency_chain() {
        let mut graph = DependencyGraph::new();

        // C depends on B depends on A
        graph.add_node("A".to_string(), 300);
        graph.add_node("B".to_string(), 200);
        graph.add_node("C".to_string(), 100);

        graph.add_edge("B".to_string(), "A".to_string());
        graph.add_edge("C".to_string(), "B".to_string());

        let order = graph.topological_sort().unwrap();

        // Should be A, B, C (despite C having highest priority)
        assert_eq!(order, vec!["A", "B", "C"]);
    }

    #[test]
    fn test_cycle_detection() {
        let mut graph = DependencyGraph::new();

        // Create a cycle: A -> B -> C -> A
        graph.add_node("A".to_string(), 100);
        graph.add_node("B".to_string(), 200);
        graph.add_node("C".to_string(), 300);

        graph.add_edge("A".to_string(), "B".to_string());
        graph.add_edge("B".to_string(), "C".to_string());
        graph.add_edge("C".to_string(), "A".to_string());

        assert!(graph.has_cycle());

        let result = graph.topological_sort();
        assert!(result.is_err());
    }

    #[test]
    fn test_complex_dependencies() {
        let mut graph = DependencyGraph::new();

        // React depends on NodeJS
        // Vite depends on NodeJS
        // NextJS depends on React and NodeJS

        graph.add_node("nodejs".to_string(), 100);
        graph.add_node("react".to_string(), 200);
        graph.add_node("vite".to_string(), 300);
        graph.add_node("nextjs".to_string(), 250);

        graph.add_edge("react".to_string(), "nodejs".to_string());
        graph.add_edge("vite".to_string(), "nodejs".to_string());
        graph.add_edge("nextjs".to_string(), "react".to_string());
        graph.add_edge("nextjs".to_string(), "nodejs".to_string());

        let order = graph.topological_sort().unwrap();

        // nodejs must come first
        assert_eq!(order[0], "nodejs");

        // react must come before nextjs
        let react_pos = order.iter().position(|x| x == "react").unwrap();
        let nextjs_pos = order.iter().position(|x| x == "nextjs").unwrap();
        assert!(react_pos < nextjs_pos);
    }

    #[test]
    fn test_diamond_dependency() {
        let mut graph = DependencyGraph::new();

        //     A
        //    / \
        //   B   C
        //    \ /
        //     D

        graph.add_node("A".to_string(), 100);
        graph.add_node("B".to_string(), 200);
        graph.add_node("C".to_string(), 200);
        graph.add_node("D".to_string(), 300);

        graph.add_edge("B".to_string(), "A".to_string());
        graph.add_edge("C".to_string(), "A".to_string());
        graph.add_edge("D".to_string(), "B".to_string());
        graph.add_edge("D".to_string(), "C".to_string());

        let order = graph.topological_sort().unwrap();

        // A must be first
        assert_eq!(order[0], "A");

        // D must be last
        assert_eq!(order[3], "D");

        // B and C can be in any order (both have same priority)
        assert!(order[1] == "B" || order[1] == "C");
        assert!(order[2] == "B" || order[2] == "C");
    }
}

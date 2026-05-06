use std::collections::BTreeMap;

use crate::redis::client::RedisType;

#[derive(Debug, Clone, PartialEq)]
pub struct KeyEntry {
    pub full_name: String,
    pub redis_type: Option<RedisType>,
    pub ttl: Option<crate::redis::client::Ttl>,
}

#[derive(Debug, Clone)]
pub struct TreeNode {
    pub segment: String,
    pub children: BTreeMap<String, TreeNode>,
    pub keys: Vec<KeyEntry>,
    pub expanded: bool,
}

impl TreeNode {
    pub fn new(segment: String) -> Self {
        Self {
            segment,
            children: BTreeMap::new(),
            keys: Vec::new(),
            expanded: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NamespaceTree {
    separator: char,
    pub root: TreeNode,
    max_keys: usize,
    current_keys: usize,
}

#[derive(Debug, Clone)]
pub struct TreeRow {
    pub depth: usize,
    pub label: String,
    pub is_namespace: bool,
    pub key: Option<KeyEntry>,
    pub path: Vec<String>,
}

impl NamespaceTree {
    pub fn new(separator: char, max_keys: usize) -> Self {
        Self {
            separator,
            root: TreeNode::new(String::new()),
            max_keys,
            current_keys: 0,
        }
    }

    pub fn insert(&mut self, key: KeyEntry) -> bool {
        if self.current_keys >= self.max_keys {
            return false;
        }
        let parts: Vec<&str> = key.full_name.split(self.separator).collect();
        if parts.is_empty() {
            self.root.keys.push(key);
            self.current_keys += 1;
            return true;
        }

        let mut node = &mut self.root;
        for (i, part) in parts.iter().enumerate() {
            if i == parts.len() - 1 {
                node.keys.push(key.clone());
                self.current_keys += 1;
                return true;
            }
            let segment = part.to_string();
            if !node.children.contains_key(&segment) {
                node.children.insert(segment.clone(), TreeNode::new(segment.clone()));
            }
            node = node.children.get_mut(&segment).unwrap();
        }
        false
    }

    pub fn at_capacity(&self) -> bool {
        self.current_keys >= self.max_keys
    }

    pub fn visible_rows(&self) -> Vec<TreeRow> {
        let mut rows = Vec::new();
        self.collect_rows(&self.root, 0, &[], &mut rows);
        rows
    }

    fn collect_rows(&self, node: &TreeNode, depth: usize, path: &[String], rows: &mut Vec<TreeRow>) {
        for (name, child) in &node.children {
            let prefix = if child.expanded { "▼ " } else { "▶ " };
            let mut row_path = path.to_vec();
            row_path.push(name.clone());
            rows.push(TreeRow {
                depth,
                label: format!("{}{}", prefix, name),
                is_namespace: true,
                key: None,
                path: row_path.clone(),
            });
            if child.expanded {
                self.collect_rows(child, depth + 1, &row_path, rows);
            }
        }
        for key in &node.keys {
            rows.push(TreeRow {
                depth,
                label: key.full_name.rsplit_once(self.separator).map(|(_, s)| s.to_string()).unwrap_or_else(|| key.full_name.clone()),
                is_namespace: false,
                key: Some(key.clone()),
                path: path.to_vec(),
            });
        }
    }

    pub fn toggle(&mut self, path: &[&str]) {
        if let Some(node) = self.find_node_mut(path) {
            node.expanded = !node.expanded;
        }
    }

    pub fn expand(&mut self, path: &[&str]) {
        if let Some(node) = self.find_node_mut(path) {
            node.expanded = true;
        }
    }

    pub fn collapse(&mut self, path: &[&str]) {
        if let Some(node) = self.find_node_mut(path) {
            node.expanded = false;
        }
    }

    pub fn remove(&mut self, full_name: &str) -> bool {
        let parts: Vec<&str> = full_name.split(self.separator).collect();
        if let Some(node) = self.find_parent_node_mut(&parts) {
            let before = node.keys.len();
            node.keys.retain(|k| k.full_name != full_name);
            let removed = node.keys.len() < before;
            if removed {
                self.current_keys = self.current_keys.saturating_sub(1);
            }
            return removed;
        }
        false
    }

    pub fn total_keys(&self) -> usize {
        self.count_keys(&self.root)
    }

    pub fn all_keys(&self) -> Vec<KeyEntry> {
        let mut out = Vec::new();
        self.collect_all_keys(&self.root, &mut out);
        out
    }

    pub fn expand_all(&mut self) {
        Self::expand_all_nodes(&mut self.root);
    }

    fn count_keys(&self, node: &TreeNode) -> usize {
        let mut count = node.keys.len();
        for child in node.children.values() {
            count += self.count_keys(child);
        }
        count
    }

    fn collect_all_keys(&self, node: &TreeNode, out: &mut Vec<KeyEntry>) {
        for key in &node.keys {
            out.push(key.clone());
        }
        for child in node.children.values() {
            self.collect_all_keys(child, out);
        }
    }

    fn expand_all_nodes(node: &mut TreeNode) {
        node.expanded = true;
        for child in node.children.values_mut() {
            Self::expand_all_nodes(child);
        }
    }

    fn find_node_mut(&mut self, path: &[&str]) -> Option<&mut TreeNode> {
        let mut node = &mut self.root;
        for segment in path {
            node = node.children.get_mut(*segment)?;
        }
        Some(node)
    }

    fn find_parent_node_mut(&mut self, parts: &[&str]) -> Option<&mut TreeNode> {
        let mut node = &mut self.root;
        for part in parts.iter().take(parts.len().saturating_sub(1)) {
            let segment = part.to_string();
            node = node.children.get_mut(&segment)?;
        }
        Some(node)
    }
}

use std::collections::HashMap;

use crate::dom::NodeId;

#[derive(Debug, Clone, Copy, Default)]
pub struct LayoutRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug)]
pub struct LayoutCache {
    dirty: bool,
    pub(crate) viewport: (f32, f32),
    pub(crate) results: HashMap<NodeId, LayoutRect>,
}

impl Default for LayoutCache {
    fn default() -> Self {
        Self {
            dirty: true,
            viewport: (1280.0, 800.0),
            results: HashMap::new(),
        }
    }
}

impl LayoutCache {
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn set_clean(&mut self) {
        self.dirty = false;
    }

    pub fn get_rect(&self, node_id: NodeId) -> Option<&LayoutRect> {
        self.results.get(&node_id)
    }
}

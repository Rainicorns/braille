use crate::dom::node::{NodeData, NodeId};

use super::DomTree;

impl DomTree {
    // -----------------------------------------------------------------------
    // CharacterData interface methods
    // All offsets and counts are in UTF-16 code units.
    // -----------------------------------------------------------------------

    /// Returns a reference to the character data content string for Text, CDATASection,
    /// Comment, or ProcessingInstruction nodes. Returns None for other node types.
    fn char_data_content(&self, id: NodeId) -> Option<&String> {
        match &self.nodes[id].data {
            NodeData::Text { content } | NodeData::CDATASection { content } => Some(content),
            NodeData::Comment { content } => Some(content),
            NodeData::ProcessingInstruction { data, .. } => Some(data),
            _ => None,
        }
    }

    /// Returns a mutable reference to the character data content string for Text,
    /// CDATASection, Comment, or ProcessingInstruction nodes. Returns None for other types.
    fn char_data_content_mut(&mut self, id: NodeId) -> Option<&mut String> {
        match &mut self.nodes[id].data {
            NodeData::Text { content } | NodeData::CDATASection { content } => Some(content),
            NodeData::Comment { content } => Some(content),
            NodeData::ProcessingInstruction { data, .. } => Some(data),
            _ => None,
        }
    }

    /// Returns the text content of a Text or Comment node, or None for other node types.
    pub fn character_data_get(&self, id: NodeId) -> Option<String> {
        self.char_data_content(id).cloned()
    }

    /// Sets the text content of a Text or Comment node.
    pub fn character_data_set(&mut self, id: NodeId, new_data: &str) {
        if let Some(content) = self.char_data_content_mut(id) {
            *content = new_data.to_string();
        }
    }

    /// Returns the length of the CharacterData in UTF-16 code units.
    pub fn character_data_length(&self, id: NodeId) -> usize {
        self.char_data_content(id)
            .map(|c| c.encode_utf16().count())
            .unwrap_or(0)
    }

    /// Appends data to a Text or Comment node.
    pub fn character_data_append(&mut self, id: NodeId, append_data: &str) {
        if let Some(content) = self.char_data_content_mut(id) {
            content.push_str(append_data);
        }
    }

    /// Deletes count UTF-16 code units starting at offset.
    /// Returns Err if offset > length.
    pub fn character_data_delete(&mut self, id: NodeId, offset: usize, count: usize) -> Result<(), &'static str> {
        let content = match self.char_data_content(id) {
            Some(c) => c.clone(),
            None => return Ok(()),
        };
        let utf16: Vec<u16> = content.encode_utf16().collect();
        if offset > utf16.len() {
            return Err("IndexSizeError");
        }
        let end = std::cmp::min(offset + count, utf16.len());
        let new_content = Self::utf16_splice_units(&utf16, offset, end, "");
        self.character_data_set(id, &new_content);
        Ok(())
    }

    /// Inserts data at offset (in UTF-16 code units).
    /// Returns Err if offset > length.
    pub fn character_data_insert(&mut self, id: NodeId, offset: usize, data: &str) -> Result<(), &'static str> {
        let content = match self.char_data_content(id) {
            Some(c) => c.clone(),
            None => return Ok(()),
        };
        let utf16: Vec<u16> = content.encode_utf16().collect();
        if offset > utf16.len() {
            return Err("IndexSizeError");
        }
        let new_content = Self::utf16_splice_units(&utf16, offset, offset, data);
        self.character_data_set(id, &new_content);
        Ok(())
    }

    /// Replaces count UTF-16 code units starting at offset with data.
    /// Returns Err if offset > length.
    pub fn character_data_replace(
        &mut self,
        id: NodeId,
        offset: usize,
        count: usize,
        data: &str,
    ) -> Result<(), &'static str> {
        let content = match self.char_data_content(id) {
            Some(c) => c.clone(),
            None => return Ok(()),
        };
        let utf16: Vec<u16> = content.encode_utf16().collect();
        if offset > utf16.len() {
            return Err("IndexSizeError");
        }
        let end = std::cmp::min(offset + count, utf16.len());
        let new_content = Self::utf16_splice_units(&utf16, offset, end, data);
        self.character_data_set(id, &new_content);
        Ok(())
    }

    /// Returns a substring of count UTF-16 code units starting at offset.
    /// Returns Err if offset > length.
    pub fn character_data_substring(&self, id: NodeId, offset: usize, count: usize) -> Result<String, &'static str> {
        let content = match self.char_data_content(id) {
            Some(c) => c,
            None => return Ok(String::new()),
        };
        let utf16_units: Vec<u16> = content.encode_utf16().collect();
        if offset > utf16_units.len() {
            return Err("IndexSizeError");
        }
        let end = std::cmp::min(offset + count, utf16_units.len());
        let slice = &utf16_units[offset..end];
        Ok(String::from_utf16_lossy(slice))
    }

    /// Splices pre-encoded UTF-16 units at code unit boundaries.
    /// Replaces units in range [start..end) with `replacement` (encoded from UTF-8).
    fn utf16_splice_units(utf16_units: &[u16], start: usize, end: usize, replacement: &str) -> String {
        let mut result_utf16: Vec<u16> = Vec::with_capacity(utf16_units.len() + replacement.encode_utf16().count());
        result_utf16.extend_from_slice(&utf16_units[..start]);
        result_utf16.extend(replacement.encode_utf16());
        result_utf16.extend_from_slice(&utf16_units[end..]);
        String::from_utf16_lossy(&result_utf16)
    }

    /// Converts a UTF-16 code unit offset to a byte offset in a UTF-8 string.
    /// Returns None if the offset is out of range.
    pub(crate) fn utf16_offset_to_byte_offset(s: &str, utf16_offset: usize) -> Option<usize> {
        let mut utf16_pos = 0;
        for (byte_pos, ch) in s.char_indices() {
            if utf16_pos == utf16_offset {
                return Some(byte_pos);
            }
            utf16_pos += ch.len_utf16();
        }
        if utf16_pos == utf16_offset {
            return Some(s.len());
        }
        None // offset out of range
    }

    /// Splits a Text node at the given offset (in UTF-16 code units).
    ///
    /// - Keeps data[..offset] in the original node
    /// - Creates a new Text node with data[offset..]
    /// - If the original node has a parent, inserts the new node as next sibling
    /// - Returns Ok(new_node_id) or Err("IndexSizeError") if offset > length
    pub fn split_text(&mut self, node_id: NodeId, utf16_offset: usize) -> Result<NodeId, &'static str> {
        // Get the text content
        let content = match &self.nodes[node_id].data {
            NodeData::Text { content } => content.clone(),
            _ => return Err("InvalidNodeTypeError"),
        };

        let utf16_len = content.encode_utf16().count();
        if utf16_offset > utf16_len {
            return Err("IndexSizeError");
        }

        let byte_offset = Self::utf16_offset_to_byte_offset(&content, utf16_offset).ok_or("IndexSizeError")?;

        // Split the data
        let kept = &content[..byte_offset];
        let split_off = &content[byte_offset..];

        // Create new Text node with the split-off portion
        let new_id = self.create_text(split_off);

        // Update the original node's data to keep only the first part
        if let NodeData::Text { ref mut content } = self.nodes[node_id].data {
            *content = kept.to_string();
        }

        // If original node has a parent, insert the new node after it
        let parent = self.nodes[node_id].parent;
        if parent.is_some() {
            self.insert_after(node_id, new_id);
        }

        Ok(new_id)
    }
}

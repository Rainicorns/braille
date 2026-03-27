mod errors;
mod adoption;
mod validation;
mod operations;
mod registration;
mod tests;

// Re-export the public API so callers outside this module are unaffected.
pub(crate) use adoption::{
    adopt_node, adopt_node_with_mapping, update_node_cache_after_adoption,
    update_node_cache_for_adoption_mapping,
};
pub(crate) use operations::{
    capture_insert_state, do_insert, fire_insert_records, fire_range_removal_for_move, RemovalInfo,
};
pub(crate) use registration::{
    document_append, document_normalize, document_prepend, document_replace_children,
    register_mutation,
};
pub(crate) use validation::validate_pre_insert;

/// Recursively clone a node from one DomTree into another.
pub(crate) fn clone_node_cross_tree(
    src: &crate::dom::DomTree,
    src_id: crate::dom::NodeId,
    dst: &mut crate::dom::DomTree,
) -> crate::dom::NodeId {
    use crate::dom::NodeData;

    let src_node = src.get_node(src_id);
    let new_id = match &src_node.data {
        NodeData::Element {
            tag_name,
            attributes,
            namespace,
        } => {
            let id = dst.create_element(tag_name);
            if let NodeData::Element {
                attributes: ref mut dst_attrs,
                namespace: ref mut dst_ns,
                ..
            } = dst.get_node_mut(id).data
            {
                *dst_attrs = attributes.clone();
                *dst_ns = namespace.clone();
            }
            id
        }
        NodeData::Text { content } => dst.create_text(content),
        NodeData::Comment { content } => dst.create_comment(content),
        NodeData::CDATASection { content } => dst.create_cdata_section(content),
        NodeData::Doctype {
            name,
            public_id,
            system_id,
        } => dst.create_doctype(name, public_id, system_id),
        NodeData::ProcessingInstruction { target, data } => dst.create_processing_instruction(target, data),
        NodeData::DocumentFragment | NodeData::ShadowRoot { .. } => dst.create_document_fragment(),
        NodeData::Document => unreachable!("nested Document nodes not supported"),
        NodeData::Attr { .. } => unreachable!("Attr nodes should not be children"),
    };

    // Recursively clone children
    let child_ids: Vec<crate::dom::NodeId> = src.get_node(src_id).children.clone();
    for child_id in child_ids {
        let cloned_child = clone_node_cross_tree(src, child_id, dst);
        dst.append_child(new_id, cloned_child);
    }

    new_id
}

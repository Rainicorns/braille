use rquickjs::{Ctx, Function};

use crate::dom::node::{DomString, NodeData};
use crate::dom::NodeId;

use super::{with_tree, with_tree_mut};

pub(super) fn register_native_attributes(ctx: &Ctx<'_>) {
    let g = ctx.globals();

    // getAttribute(nodeId, name) -> string | null (empty string = null)
    g.set("__n_getAttribute", Function::new(ctx.clone(), |node_id: u32, name: DomString| -> String {
        with_tree(|tree| {
            tree.get_attribute(node_id as NodeId, &name).map(|v| v.to_string()).unwrap_or_default()
        })
    }).unwrap()).unwrap();

    // hasAttribute(nodeId, name) -> bool
    g.set("__n_hasAttribute", Function::new(ctx.clone(), |node_id: u32, name: DomString| -> bool {
        with_tree(|tree| tree.has_attribute(node_id as NodeId, &name))
    }).unwrap()).unwrap();

    // hasAttributes(nodeId) -> bool (any attributes at all)
    g.set("__n_hasAttributes", Function::new(ctx.clone(), |node_id: u32| -> bool {
        with_tree(|tree| tree.has_attributes(node_id as NodeId))
    }).unwrap()).unwrap();

    // setAttribute(nodeId, name, value)
    g.set("__n_setAttribute", Function::new(ctx.clone(), |node_id: u32, name: DomString, value: DomString| {
        with_tree_mut(|tree| tree.set_attribute(node_id as NodeId, &name, &value));
    }).unwrap()).unwrap();

    // removeAttribute(nodeId, name)
    g.set("__n_removeAttribute", Function::new(ctx.clone(), |node_id: u32, name: DomString| {
        with_tree_mut(|tree| { tree.remove_attribute(node_id as NodeId, &name); });
    }).unwrap()).unwrap();

    // setAttributeNS(nodeId, namespace, qualifiedName, value)
    g.set("__n_setAttributeNS", Function::new(ctx.clone(), |node_id: u32, namespace: DomString, qualified_name: DomString, value: DomString| {
        with_tree_mut(|tree| tree.set_attribute_ns(node_id as NodeId, &namespace, &qualified_name, &value));
    }).unwrap()).unwrap();

    // getAttributeNS(nodeId, namespace, localName) -> string (empty = not found)
    g.set("__n_getAttributeNS", Function::new(ctx.clone(), |node_id: u32, namespace: DomString, local_name: DomString| -> String {
        with_tree(|tree| tree.get_attribute_ns(node_id as NodeId, &namespace, &local_name).map(|v| v.to_string()).unwrap_or_default())
    }).unwrap()).unwrap();

    // getAttributeNodeNS(nodeId, namespace, localName) -> JSON string with full attr info, or empty
    g.set("__n_getAttributeNodeNS", Function::new(ctx.clone(), |node_id: u32, namespace: DomString, local_name: DomString| -> String {
        with_tree(|tree| {
            let node = tree.get_node(node_id as NodeId);
            if let NodeData::Element { ref attributes, .. } = node.data {
                if let Some(attr) = attributes.iter().find(|a| a.matches_ns(&namespace, &local_name)) {
                    return format!(
                        "{{\"localName\":\"{}\",\"prefix\":\"{}\",\"namespace\":\"{}\",\"value\":\"{}\"}}",
                        attr.local_name,
                        attr.prefix,
                        attr.namespace,
                        attr.value.replace('\\', "\\\\").replace('"', "\\\"")
                    );
                }
            }
            String::new()
        })
    }).unwrap()).unwrap();

    // hasAttributeNS(nodeId, namespace, localName) -> bool
    g.set("__n_hasAttributeNS", Function::new(ctx.clone(), |node_id: u32, namespace: DomString, local_name: DomString| -> bool {
        with_tree(|tree| tree.has_attribute_ns(node_id as NodeId, &namespace, &local_name))
    }).unwrap()).unwrap();

    // removeAttributeNS(nodeId, namespace, localName)
    g.set("__n_removeAttributeNS", Function::new(ctx.clone(), |node_id: u32, namespace: DomString, local_name: DomString| {
        with_tree_mut(|tree| { tree.remove_attribute_ns(node_id as NodeId, &namespace, &local_name); });
    }).unwrap()).unwrap();

    // __n_getAttributeNames(nodeId) -> JSON array of attribute names
    g.set("__n_getAttributeNames", Function::new(ctx.clone(), |node_id: u32| -> String {
        with_tree(|tree| {
            let names = tree.attribute_names(node_id as NodeId);
            serde_json::to_string(&names).unwrap_or_else(|_| "[]".to_string())
        })
    }).unwrap()).unwrap();

    // __n_getAttributesFull(nodeId) -> JSON array of {name, value, ns, prefix} objects
    g.set("__n_getAttributesFull", Function::new(ctx.clone(), |node_id: u32| -> String {
        with_tree(|tree| {
            let node = tree.get_node(node_id as NodeId);
            if let NodeData::Element { ref attributes, .. } = node.data {
                let entries: Vec<_> = attributes.iter().map(|a| {
                    serde_json::json!({
                        "name": a.qualified_name(),
                        "localName": a.local_name,
                        "value": a.value,
                        "ns": if a.namespace.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(a.namespace.clone()) },
                        "prefix": if a.prefix.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(a.prefix.clone()) }
                    })
                }).collect();
                serde_json::to_string(&entries).unwrap_or_else(|_| "[]".to_string())
            } else {
                "[]".to_string()
            }
        })
    }).unwrap()).unwrap();
}

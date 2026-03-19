use boa_engine::property::PropertyDescriptor;
use boa_engine::JsValue;

/// Configurable, non-enumerable getter with set:undefined.
/// Used for readonly IDL attributes (e.g., node.nodeType, event.type).
pub(crate) fn readonly_accessor(getter: impl Into<JsValue>) -> PropertyDescriptor {
    PropertyDescriptor::builder()
        .get(getter)
        .set(JsValue::undefined())
        .configurable(true)
        .enumerable(false)
        .build()
}

/// Writable, configurable, non-enumerable data property.
/// Used for methods and mutable IDL attributes.
pub(crate) fn data_prop(value: impl Into<JsValue>) -> PropertyDescriptor {
    PropertyDescriptor::builder()
        .value(value)
        .writable(true)
        .configurable(true)
        .enumerable(false)
        .build()
}

/// Non-writable, non-configurable, non-enumerable data property.
/// Used for Constructor.prototype links.
pub(crate) fn prototype_on_ctor(proto: impl Into<JsValue>) -> PropertyDescriptor {
    PropertyDescriptor::builder()
        .value(proto)
        .writable(false)
        .configurable(false)
        .enumerable(false)
        .build()
}

/// Writable, configurable, non-enumerable data property.
/// Used for proto.constructor back-links.
pub(crate) fn constructor_on_proto(ctor: impl Into<JsValue>) -> PropertyDescriptor {
    // Same shape as data_prop, but named for clarity at call sites.
    PropertyDescriptor::builder()
        .value(ctor)
        .writable(true)
        .configurable(true)
        .enumerable(false)
        .build()
}

/// Non-writable, non-configurable, enumerable data property.
/// Used for interface constants (e.g., Node.ELEMENT_NODE = 1).
pub(crate) fn readonly_constant(value: impl Into<JsValue>) -> PropertyDescriptor {
    PropertyDescriptor::builder()
        .value(value)
        .writable(false)
        .configurable(false)
        .enumerable(false)
        .build()
}

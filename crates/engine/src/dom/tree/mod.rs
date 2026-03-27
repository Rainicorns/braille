mod core;
mod mutation;
mod query;
mod serialize;
mod character_data;
mod comparison;
mod namespace;
mod validation;
mod tests;

pub use self::core::DomTree;
pub use validation::{
    is_valid_attribute_name, is_valid_doctype_name, is_valid_dom_name, is_valid_element_name,
    is_valid_xml_name,
};

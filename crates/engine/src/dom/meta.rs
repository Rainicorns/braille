/// A cached `<meta>` tag entry, extracted at parse time.
#[derive(Debug, Clone)]
pub struct MetaEntry {
    pub name: String,
    pub content: String,
    pub http_equiv: Option<String>,
}

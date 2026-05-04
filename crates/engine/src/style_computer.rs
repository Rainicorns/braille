use crate::dom::tree::DomTree;

/// Trait abstracting CSS style computation.
///
/// The default implementation uses Braille's built-in CSS cascade
/// (`crate::css::style_tree::compute_all_styles`). Alternative backends
/// (e.g. Servo's Stylo) can implement this trait to provide their own
/// style resolution.
pub trait StyleComputer {
    /// Compute styles for all elements in the tree.
    /// After this returns, every Element node should have `computed_style` populated.
    fn compute_all_styles(&self, tree: &mut DomTree);
}

/// Braille's built-in CSS style computer.
pub struct BrailleStyleComputer;

impl StyleComputer for BrailleStyleComputer {
    fn compute_all_styles(&self, tree: &mut DomTree) {
        crate::css::style_tree::compute_all_styles(tree);
    }
}

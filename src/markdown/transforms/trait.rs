use pulldown_cmark::Event;

use crate::markdown::TransformContext;

/// Trait for AST transformations
#[allow(unused)]
pub trait AstTransform: Send + Sync {
    /// Human-readable name for debugging/logging
    fn name(&self) -> &'static str;

    /// Priority for ordering (lower runs first)
    fn priority(&self) -> i32 {
        100
    }

    /// Transform the events, returning modified events
    fn transform<'a>(&self, events: Vec<Event<'a>>, ctx: &TransformContext<'_>) -> Vec<Event<'a>>;
}

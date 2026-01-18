//! Asset processing (CSS bundling, image optimization)

mod css;
pub mod images;

pub use css::{build_css, minify_css};
pub use images::{ImageConfig, optimize_images, optimize_single_image};

//! Embedded static assets from the original Structurizr viewer.

use rust_embed::Embed;

#[derive(Embed)]
#[folder = "assets/"]
pub struct Assets;

/// The pre-built mdBook documentation site (`site/` at the repo root, built
/// with `mdbook build site`). `structurizr-web` only consumes this output —
/// it never invokes mdBook itself; `build.rs` just ensures the directory
/// exists so this embed doesn't fail to compile before the book has been
/// built for the first time.
///
/// `demo/` holds the exported interactive viewer that `site/build.sh` adds for
/// the published site; it carries its own copy of every viewer asset, so it
/// is left out here rather than doubling the binary's embedded assets.
#[derive(Embed)]
#[folder = "../../site/book/"]
#[exclude = "demo/*"]
pub struct DocsAssets;

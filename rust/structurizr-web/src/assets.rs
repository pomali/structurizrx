//! Embedded static assets from the original Structurizr viewer.

use rust_embed::Embed;

#[derive(Embed)]
#[folder = "assets/"]
pub struct Assets;

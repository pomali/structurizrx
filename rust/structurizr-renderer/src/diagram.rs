/// Output format of a rendered diagram.
#[derive(Debug, Clone, PartialEq)]
pub enum DiagramFormat {
    PlantUml,
    Mermaid,
    Dot,
}

impl std::fmt::Display for DiagramFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiagramFormat::PlantUml => write!(f, "plantuml"),
            DiagramFormat::Mermaid => write!(f, "mermaid"),
            DiagramFormat::Dot => write!(f, "dot"),
        }
    }
}

/// A rendered diagram with key, content and format.
#[derive(Debug, Clone)]
pub struct Diagram {
    pub key: String,
    pub content: String,
    pub format: DiagramFormat,
}

impl Diagram {
    pub fn new(key: impl Into<String>, content: impl Into<String>, format: DiagramFormat) -> Self {
        Self {
            key: key.into(),
            content: content.into(),
            format,
        }
    }

    /// File extension for this format.
    pub fn extension(&self) -> &str {
        match self.format {
            DiagramFormat::PlantUml => "puml",
            DiagramFormat::Mermaid => "md",
            DiagramFormat::Dot => "dot",
        }
    }
}

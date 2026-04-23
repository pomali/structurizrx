/// Helper for producing indented text output.
pub struct IndentingWriter {
    buf: String,
    indent: usize,
    indent_str: String,
}

impl IndentingWriter {
    pub fn new() -> Self {
        Self {
            buf: String::new(),
            indent: 0,
            indent_str: "    ".to_string(),
        }
    }

    pub fn indent(&mut self) {
        self.indent += 1;
    }

    pub fn outdent(&mut self) {
        if self.indent > 0 {
            self.indent -= 1;
        }
    }

    pub fn line(&mut self, s: &str) {
        for _ in 0..self.indent {
            self.buf.push_str(&self.indent_str);
        }
        self.buf.push_str(s);
        self.buf.push('\n');
    }

    pub fn blank(&mut self) {
        self.buf.push('\n');
    }

    pub fn into_string(self) -> String {
        self.buf
    }
}

impl Default for IndentingWriter {
    fn default() -> Self {
        Self::new()
    }
}

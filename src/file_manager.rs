use std::collections::HashMap;

use tower_lsp::lsp_types::{TextDocumentContentChangeEvent, Url};

#[derive(Debug)]
struct TextDoc {
    text: String,
    ver: i32,
}

#[derive(Debug)]
pub struct FileManager {
    curr_files: HashMap<Url, TextDoc>,
}

impl FileManager {
    pub fn new() -> Self {
        Self {
            curr_files: HashMap::new(),
        }
    }

    pub fn on_opened_file(&mut self, uri: Url, text: String, ver: i32) {
        self.curr_files.insert(
            uri,
            TextDoc {
                text: text,
                ver: ver,
            },
        );
    }

    pub fn on_changed_file(
        &mut self,
        uri: &Url,
        changed: &[TextDocumentContentChangeEvent],
        ver: i32,
    ) {
        if let Some(doc) = self.curr_files.get_mut(uri) {
            for change in changed {
                doc.text = change.text.clone();
            }
            doc.ver = ver;
        }
    }

    pub fn on_closed_file(&mut self, uri: &Url) {
        self.curr_files.remove(uri);
    }

    pub fn get_text(&self, uri: &Url) -> Option<&str> {
        self.curr_files.get(uri).map(|doc| doc.text.as_str())
    }
}

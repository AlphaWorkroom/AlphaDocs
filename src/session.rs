//! Open document sessions retain independent content, selection, undo and save state.
use crate::{
    engine::Editor,
    model::{Document, Position},
    persistence,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub struct DocumentSession {
    pub editor: Editor,
    pub file: Option<PathBuf>,
    pub source: Option<PathBuf>,
    pub saved_library_revision: u64,
    pub viewing: bool,
    pub collaboration: Option<crate::collaboration::live::LiveSession>,
}
impl DocumentSession {
    pub fn new(doc: Document, file: Option<PathBuf>) -> Self {
        Self {
            editor: Editor::new(doc),
            source: file.clone(),
            file,
            saved_library_revision: u64::MAX,
            viewing: false,
            collaboration: None,
        }
    }
    pub fn dirty(&self) -> bool {
        self.saved_library_revision != self.editor.revision
    }
    pub fn save_local(&mut self) -> Result<(), String> {
        self.editor.refresh_collaboration(true);
        if self.dirty() {
            persistence::autosave(&self.editor.doc)?;
            self.saved_library_revision = self.editor.revision;
        }
        Ok(())
    }
}
pub struct DocumentTab {
    pub id: String,
    pub title: String,
    pub session: Option<DocumentSession>,
}
#[derive(Default)]
pub struct TabStore {
    pub tabs: Vec<DocumentTab>,
    pub active: Option<usize>,
}
impl TabStore {
    pub fn refresh(&mut self, current: &DocumentSession) {
        if let Some(i) = self.active
            && let Some(tab) = self.tabs.get_mut(i)
        {
            tab.id = current.editor.doc.id.clone();
            tab.title = current.editor.doc.title.clone();
        }
    }
    pub fn open(&mut self, current: &mut DocumentSession, next: DocumentSession) {
        if let Some(index) = self
            .tabs
            .iter()
            .position(|tab| tab.id == next.editor.doc.id)
        {
            self.switch(current, index);
            return;
        }
        let tab = DocumentTab {
            id: next.editor.doc.id.clone(),
            title: next.editor.doc.title.clone(),
            session: None,
        };
        let previous = std::mem::replace(current, next);
        if let Some(i) = self.active {
            self.tabs[i].session = Some(previous);
        }
        self.tabs.push(tab);
        self.active = Some(self.tabs.len() - 1);
    }
    pub fn switch(&mut self, current: &mut DocumentSession, index: usize) -> bool {
        if self.active == Some(index) {
            return true;
        }
        let Some(tab) = self.tabs.get_mut(index) else {
            return false;
        };
        let Some(next) = tab.session.take() else {
            return false;
        };
        let previous = std::mem::replace(current, next);
        if let Some(i) = self.active {
            self.tabs[i].title = previous.editor.doc.title.clone();
            self.tabs[i].session = Some(previous);
        }
        self.active = Some(index);
        true
    }
    pub fn save_all(&mut self, current: &mut DocumentSession) -> Result<(), String> {
        let mut failures = Vec::new();
        if self.active.is_some()
            && let Err(e) = current.save_local()
        {
            failures.push(format!("{}: {e}", current.editor.doc.title));
        }
        for tab in &mut self.tabs {
            if let Some(session) = &mut tab.session
                && let Err(e) = session.save_local()
            {
                failures.push(format!("{}: {e}", session.editor.doc.title));
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(failures.join("\n"))
        }
    }
    /// A failed write leaves the tab and all its undo state in place.
    pub fn close(&mut self, current: &mut DocumentSession, index: usize) -> Result<(), String> {
        if index >= self.tabs.len() {
            return Ok(());
        }
        if self.active == Some(index) {
            current.save_local()?;
            if current.collaboration.is_some() {
                current.editor.disconnect_collaboration();
                current.collaboration = None;
            }
        } else if let Some(s) = &mut self.tabs[index].session {
            s.save_local()?;
        }
        self.tabs.remove(index);
        match self.active {
            Some(active) if active == index => {
                self.active = None;
                if !self.tabs.is_empty() {
                    self.switch(current, index.min(self.tabs.len() - 1));
                }
            }
            Some(active) if active > index => self.active = Some(active - 1),
            _ => {}
        }
        Ok(())
    }
    pub fn find_source(&self, current: &DocumentSession, path: &std::path::Path) -> Option<usize> {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        self.tabs.iter().enumerate().find_map(|(i, tab)| {
            let session = if self.active == Some(i) {
                Some(current)
            } else {
                tab.session.as_ref()
            }?;
            [&session.source, &session.file]
                .into_iter()
                .flatten()
                .any(|p| p.canonicalize().unwrap_or_else(|_| p.clone()) == canonical)
                .then_some(i)
        })
    }
    pub fn manifest(&self, current: &DocumentSession, home: bool) -> SessionManifest {
        SessionManifest {
            documents: self
                .tabs
                .iter()
                .enumerate()
                .filter_map(|(i, tab)| {
                    let session = if self.active == Some(i) {
                        Some(current)
                    } else {
                        tab.session.as_ref()
                    }?;
                    Some(SessionEntry {
                        id: tab.id.clone(),
                        file: session.file.clone(),
                        source: session.source.clone(),
                        caret: session.editor.caret,
                        anchor: session.editor.anchor,
                        viewing: session.viewing,
                        tab_selections: session.editor.tab_selections(),
                    })
                })
                .collect(),
            active: self.active,
            home,
        }
    }
}
#[derive(Default, Serialize, Deserialize)]
pub struct SessionManifest {
    pub documents: Vec<SessionEntry>,
    pub active: Option<usize>,
    pub home: bool,
}
#[derive(Serialize, Deserialize)]
pub struct SessionEntry {
    pub id: String,
    pub file: Option<PathBuf>,
    pub source: Option<PathBuf>,
    pub caret: Position,
    pub anchor: Position,
    pub viewing: bool,
    #[serde(default)]
    pub tab_selections: std::collections::HashMap<String, (Position, Position)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manifest_retains_selections_in_each_content_tab() {
        let mut store = TabStore::default();
        let mut current = DocumentSession::new(Document::new("Placeholder"), None);
        store.open(
            &mut current,
            DocumentSession::new(Document::new("Tab selections"), None),
        );
        current.editor.insert("First");
        current.editor.select_all();
        let first = (current.editor.caret, current.editor.anchor);
        current.editor.add_tab();
        current.editor.insert("Second");
        let manifest = store.manifest(&current, false);
        let loaded: SessionManifest =
            serde_json::from_slice(&serde_json::to_vec(&manifest).unwrap()).unwrap();
        let mut reopened = Editor::new(current.editor.doc.clone());
        reopened.restore_tab_selections(loaded.documents[0].tab_selections.clone());
        reopened.switch_tab(0);
        assert_eq!((reopened.caret, reopened.anchor), first);
    }
    #[test]
    fn switching_preserves_selection_and_independent_undo() {
        let mut store = TabStore::default();
        let mut current = DocumentSession::new(Document::new("Placeholder"), None);
        store.open(
            &mut current,
            DocumentSession::new(Document::new("One"), None),
        );
        current.editor.insert("first");
        current.editor.select_all();
        let selection = current.editor.range();
        store.open(
            &mut current,
            DocumentSession::new(Document::new("Two"), None),
        );
        current.editor.insert("second");
        assert!(store.switch(&mut current, 0));
        assert_eq!(current.editor.doc.text(), "first");
        assert_eq!(current.editor.range(), selection);
        current.editor.undo();
        assert_eq!(current.editor.doc.text(), "");
        assert!(store.switch(&mut current, 1));
        assert_eq!(current.editor.doc.text(), "second");
        current.editor.undo();
        assert_eq!(current.editor.doc.text(), "");
    }
    #[test]
    fn reopening_same_document_does_not_duplicate_tab() {
        let doc = Document::new("One");
        let mut store = TabStore::default();
        let mut current = DocumentSession::new(doc.clone(), None);
        store.open(&mut current, DocumentSession::new(doc.clone(), None));
        current.editor.insert("unsaved");
        store.open(&mut current, DocumentSession::new(doc, None));
        assert_eq!(store.tabs.len(), 1);
        assert_eq!(current.editor.doc.text(), "unsaved");
    }
}

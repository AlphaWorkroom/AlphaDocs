//! All collaboration projections enter the editing engine here, never via UI text mutation.
use super::*;
use crate::collaboration::{
    live::{Presence, Room},
    replica::Replica,
};
use std::sync::{Arc, Mutex};
pub(super) struct SharedEditor {
    pub replica: Replica,
    room: Arc<Mutex<Room>>,
    last_poll: Instant,
}
impl SharedEditor {
    pub fn commit(&mut self, before: &Document, document: &Document) -> Result<(), String> {
        document.validate()?;
        self.replica.commit_delta(before, document)?;
        if let Err(error) = self.publish()
            && let Ok(mut room) = self.room.lock()
        {
            room.status = format!("Synchronization paused: {error} · local draft retained");
            room.writable = false;
        }
        Ok(())
    }
    fn publish(&self) -> Result<(), String> {
        let mut room = self.room.lock().map_err(|_| "Session unavailable")?;
        if !room.writable {
            return Err("This shared document is read-only".into());
        }
        let update = self.replica.diff(&room.replica.vector())?;
        room.replica.accept(&update)
    }
    pub fn report(&self, error: String) {
        if let Ok(mut room) = self.room.lock() {
            room.status = error;
        }
    }
}
impl Editor {
    pub fn collaboration_name(&self) -> String {
        self.collaboration
            .as_ref()
            .and_then(|s| s.room.lock().ok().map(|r| r.local.name.clone()))
            .unwrap_or_else(|| "You".into())
    }
    pub fn attach_collaboration(&mut self, room: Arc<Mutex<Room>>) -> Result<(), String> {
        let mut replica = Replica::load(
            &room
                .lock()
                .map_err(|_| "Session unavailable")?
                .replica
                .save(),
        )?;
        let document = replica.project(&self.doc)?;
        self.doc = document;
        self.collaboration = Some(SharedEditor {
            replica,
            room,
            last_poll: Instant::now() - Duration::from_secs(1),
        });
        self.undo.clear();
        self.redo.clear();
        self.last_insert = None;
        self.revision += 1;
        self.clamp();
        self.sync_style();
        Ok(())
    }
    pub fn disconnect_collaboration(&mut self) {
        self.refresh_collaboration(true);
        self.collaboration = None;
        self.undo.clear();
        self.redo.clear();
        self.last_insert = None;
    }
    pub fn collaboration_writable(&self) -> bool {
        self.collaboration
            .as_ref()
            .is_none_or(|s| s.room.lock().is_ok_and(|r| r.writable))
    }
    pub fn refresh_collaboration(&mut self, force: bool) {
        let Some(mut shared) = self.collaboration.take() else {
            return;
        };
        if !force && shared.last_poll.elapsed() < Duration::from_millis(100) {
            self.collaboration = Some(shared);
            return;
        }
        shared.last_poll = Instant::now();
        let caret = shared.replica.anchor(&self.doc, self.caret);
        let anchor = shared.replica.anchor(&self.doc, self.anchor);
        let result = (|| {
            let update = shared
                .room
                .lock()
                .map_err(|_| "Session unavailable")?
                .replica
                .diff(&shared.replica.vector())?;
            if update == [0, 0] {
                return Ok(self.doc.clone());
            }
            shared.replica.apply(&update)?;
            shared.replica.project(&self.doc)
        })();
        match result {
            Ok(document) if document != self.doc => {
                self.doc = document;
                if let Some(p) = caret.as_ref().and_then(|a| shared.replica.resolve(a)) {
                    self.caret = p;
                }
                if let Some(p) = anchor.as_ref().and_then(|a| shared.replica.resolve(a)) {
                    self.anchor = p;
                }
                self.revision += 1;
                self.last_insert = None;
                self.clamp();
                self.sync_style();
            }
            Err(error) => shared.report(error),
            _ => {}
        }
        if let Ok(mut room) = shared.room.lock() {
            room.local.caret = shared.replica.anchor(&self.doc, self.caret);
            room.local.anchor = shared.replica.anchor(&self.doc, self.anchor);
        }
        self.collaboration = Some(shared);
    }
    pub(super) fn shared_undo(&mut self, redo: bool) -> bool {
        if self.collaboration.is_none() {
            return false;
        }
        if !self.collaboration_writable() {
            return true;
        }
        self.refresh_collaboration(true);
        let Some(mut shared) = self.collaboration.take() else {
            return true;
        };
        let caret = shared.replica.anchor(&self.doc, self.caret);
        let anchor = shared.replica.anchor(&self.doc, self.anchor);
        shared.replica.undo(redo);
        if let Err(error) = shared.publish()
            && let Ok(mut room) = shared.room.lock()
        {
            room.status = format!("Synchronization paused: {error} · local draft retained");
            room.writable = false;
        }
        match shared.replica.project(&self.doc) {
            Ok(document) => {
                self.doc = document;
                if let Some(p) = caret.as_ref().and_then(|a| shared.replica.resolve(a)) {
                    self.caret = p;
                }
                if let Some(p) = anchor.as_ref().and_then(|a| shared.replica.resolve(a)) {
                    self.anchor = p;
                }
                self.revision += 1;
                self.last_insert = None;
                self.clamp();
                self.sync_style();
            }
            Err(error) => shared.report(error),
        }
        self.collaboration = Some(shared);
        true
    }
    pub fn remote_selections(&self) -> Vec<(String, String, Position, Position)> {
        let Some(shared) = &self.collaboration else {
            return vec![];
        };
        let Ok(room) = shared.room.lock() else {
            return vec![];
        };
        let tab = &self.doc.tabs[self.doc.active_tab].id;
        room.peers
            .values()
            .filter(|p| p.seen.elapsed() < Duration::from_secs(10))
            .filter_map(|peer| {
                let Presence {
                    name,
                    caret,
                    anchor,
                } = &peer.presence;
                let caret = caret.as_ref()?;
                let anchor = anchor.as_ref()?;
                if &caret.tab != tab || &anchor.tab != tab {
                    return None;
                }
                Some((
                    peer.id.clone(),
                    name.clone(),
                    shared.replica.resolve(caret)?,
                    shared.replica.resolve(anchor)?,
                ))
            })
            .collect()
    }
}

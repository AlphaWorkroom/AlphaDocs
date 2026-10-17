//! Native structured document projection onto Yrs shared maps/arrays/rich text.
//! Navigation and original image payloads are kept out of text transactions.
use crate::model::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use yrs::{
    Any, Array, ArrayPrelim, Assoc, Doc, GetString, In, IndexedSequence, Map, MapPrelim, MapRef,
    Out, ReadTxn, StateVector, StickyIndex, Text, TextPrelim, TextRef, Transact, TransactionMut,
    UndoManager, Update,
    types::Attrs,
    updates::{decoder::Decode, encoder::Encode},
};

const MAX_STATE: usize = 32 * 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Anchor {
    pub tab: String,
    pub cursor: Vec<u8>,
}
pub struct Replica {
    doc: Doc,
    root: MapRef,
    assets: MapRef,
    undo: UndoManager,
    media: HashMap<String, SharedVec<u8>>,
    derived_media: HashMap<String, (std::sync::Arc<str>, SharedVec<u8>)>,
    projection: HashMap<(String, usize), (Value, Block)>,
}
impl Replica {
    fn empty() -> Self {
        let doc = Doc::with_options(yrs::Options {
            offset_kind: yrs::OffsetKind::Utf16,
            ..Default::default()
        });
        let root = doc.get_or_insert_map("document");
        let assets = doc.get_or_insert_map("assets");
        let mut undo = UndoManager::new();
        undo.expand_scope(&doc, &root);
        undo.include_origin("local");
        Self {
            doc,
            root,
            assets,
            undo,
            media: HashMap::new(),
            derived_media: HashMap::new(),
            projection: HashMap::new(),
        }
    }
    pub fn new(document: &Document) -> Result<Self, String> {
        document.validate()?;
        let mut replica = Self::empty();
        replica.commit(document)?;
        if replica.save().len() > MAX_STATE {
            return Err("Shared document exceeds the 32 MiB session limit".into());
        }
        replica.undo.clear_all();
        Ok(replica)
    }
    pub fn load(bytes: &[u8]) -> Result<Self, String> {
        let mut replica = Self::empty();
        replica.apply(bytes)?;
        Ok(replica)
    }
    pub fn save(&self) -> Vec<u8> {
        self.doc
            .transact()
            .encode_state_as_update_v1(&StateVector::default())
    }
    pub fn vector(&self) -> Vec<u8> {
        self.doc.transact().state_vector().encode_v1()
    }
    pub fn diff(&self, vector: &[u8]) -> Result<Vec<u8>, String> {
        let sv = StateVector::decode_v1(vector).map_err(|_| "Invalid synchronization clock")?;
        Ok(self.doc.transact().encode_state_as_update_v1(&sv))
    }
    pub fn apply(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() > MAX_STATE {
            return Err("Shared document exceeds 32 MiB".into());
        }
        let update = Update::decode_v1(bytes).map_err(|_| "Invalid collaboration update")?;
        self.doc
            .transact_mut_with("remote")
            .apply_update(update)
            .map_err(|_| "Could not merge collaboration update".to_string())
    }
    /// Validate a speculative replica before changing the accepted network state.
    pub fn accept(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes == [0, 0] {
            return Ok(());
        }
        let mut candidate = Self::load(&self.save())?;
        candidate.apply(bytes)?;
        candidate.project(&Document::new(""))?.validate()?;
        if candidate.save().len() > MAX_STATE {
            return Err("Shared session is full; keep a local copy".into());
        }
        self.apply(bytes)
    }
    pub fn can_undo(&self) -> bool {
        self.undo.can_undo()
    }
    pub fn can_redo(&self) -> bool {
        self.undo.can_redo()
    }
    pub fn undo(&mut self, redo: bool) {
        if redo {
            self.undo.redo_blocking();
        } else {
            self.undo.undo_blocking();
        }
    }
    pub fn commit(&mut self, document: &Document) -> Result<(), String> {
        let mut clean = document.clone();
        for i in 0..document.tabs.len() {
            let blocks: &mut [Block] = if i == clean.active_tab {
                &mut clean.blocks
            } else {
                &mut clean.tabs[i].blocks
            };
            for block in blocks.iter_mut() {
                if let Block::Image(image) = block {
                    let key = image.id.clone();
                    self.media.insert(key.clone(), image.data.clone());
                    // Binary immutable assets are inserted once, not serialized as per-byte JSON.
                    if self.assets.get(&self.doc.transact(), &key).is_none() {
                        self.assets.insert(
                            &mut self.doc.transact_mut_with("asset"),
                            key,
                            &image.data[..],
                        );
                    }
                    image.data = SharedVec::default();
                }
            }
        }
        let tabs: Vec<_> = (0..clean.tabs.len()).map(|i| json!({
            "id": clean.tabs[i].id, "title": clean.tabs[i].title, "blocks": clean.tab_blocks(i)
        })).collect();
        let value = normalize(
            json!({"title":clean.title,"page":clean.page,"citations":clean.citations,"tabs":tabs}),
        );
        self.undo.reset();
        let mut txn = self.doc.transact_mut_with("local");
        reconcile_map(
            &self.root,
            &mut txn,
            value.as_object().ok_or("Invalid document projection")?,
            0,
        )?;
        Ok(())
    }
    /// Ordinary edits serialize only changed blocks, keeping untouched rich text/media shared.
    pub fn commit_delta(&mut self, before: &Document, after: &Document) -> Result<(), String> {
        if before.citations != after.citations {
            return self.commit(after);
        }
        if before.tabs.len() != after.tabs.len()
            || before
                .tabs
                .iter()
                .zip(&after.tabs)
                .any(|(a, b)| a.id != b.id)
            || (0..after.tabs.len())
                .any(|i| before.tab_blocks(i).len() != after.tab_blocks(i).len())
        {
            return self.commit(after);
        }
        let mut changes = vec![];
        for i in 0..after.tabs.len() {
            for (index, (old, new)) in before
                .tab_blocks(i)
                .iter()
                .zip(after.tab_blocks(i))
                .enumerate()
            {
                if old == new {
                    continue;
                }
                let mut clean = new.clone();
                if let Block::Image(image) = &mut clean {
                    self.media.insert(image.id.clone(), image.data.clone());
                    if self.assets.get(&self.doc.transact(), &image.id).is_none() {
                        self.assets.insert(
                            &mut self.doc.transact_mut_with("asset"),
                            image.id.clone(),
                            &image.data[..],
                        );
                    }
                    image.data = SharedVec::default();
                }
                let value = normalize(serde_json::to_value(clean).map_err(|e| e.to_string())?);
                self.projection.insert(
                    (after.tabs[i].id.clone(), index),
                    (value.clone(), new.clone()),
                );
                changes.push((i, index, value));
            }
        }
        self.undo.reset();
        let mut txn = self.doc.transact_mut_with("local");
        if before.title != after.title {
            self.root.insert(&mut txn, "title", after.title.as_str());
        }
        if before.page != after.page {
            let value = normalize(serde_json::to_value(&after.page).map_err(|e| e.to_string())?);
            let out = self
                .root
                .get(&txn, "page")
                .ok_or("Shared page is missing")?;
            reconcile_value(&out, &mut txn, &value, 0)?;
        }
        let Some(Out::YArray(tabs)) = self.root.get(&txn, "tabs") else {
            return Err("Shared tabs are missing".into());
        };
        for (i, (old, new)) in before.tabs.iter().zip(&after.tabs).enumerate() {
            if old.title != new.title
                && let Some(Out::YMap(tab)) = tabs.get(&txn, i as u32)
            {
                tab.insert(&mut txn, "title", new.title.as_str());
            }
        }
        for (i, index, value) in changes {
            let Some(Out::YMap(tab)) = tabs.get(&txn, i as u32) else {
                return Err("Shared tab is missing".into());
            };
            let Some(Out::YArray(blocks)) = tab.get(&txn, "blocks") else {
                return Err("Shared blocks are missing".into());
            };
            let out = blocks
                .get(&txn, index as u32)
                .ok_or("Shared block is missing")?;
            if !reconcile_value(&out, &mut txn, &value, 0)? {
                blocks.remove(&mut txn, index as u32);
                let out = blocks.insert(&mut txn, index as u32, prelim(&value, 0)?);
                initialize(&out, &mut txn, &value, 0)?;
            }
        }
        Ok(())
    }
    pub fn project(&mut self, local: &Document) -> Result<Document, String> {
        let txn = self.doc.transact();
        let mut budget = 200_000;
        let value = read_value(&Out::YMap(self.root.clone()), &txn, 0, &mut budget)?;
        let mut doc = local.clone();
        doc.title =
            serde_json::from_value(value["title"].clone()).map_err(|_| "Missing shared title")?;
        doc.page = serde_json::from_value(value["page"].clone())
            .map_err(|_| "Invalid shared page setup")?;
        doc.citations = if value["citations"].is_null() {
            Default::default()
        } else {
            serde_json::from_value(value["citations"].clone())
                .map_err(|_| "Invalid shared citations")?
        };
        let mut projection = HashMap::new();
        doc.tabs = vec![];
        for tab in value["tabs"]
            .as_array()
            .ok_or("Invalid shared content tabs")?
        {
            let id = tab["id"].as_str().ok_or("Invalid tab identity")?.to_owned();
            let title = tab["title"].as_str().ok_or("Invalid tab title")?.to_owned();
            let mut blocks = vec![];
            for (index, value) in tab["blocks"]
                .as_array()
                .ok_or("Invalid tab blocks")?
                .iter()
                .enumerate()
            {
                let key = (id.clone(), index);
                let block = if let Some((old, block)) = self.projection.get(&key)
                    && old == value
                {
                    block.clone()
                } else {
                    serde_json::from_value(value.clone()).map_err(|_| "Invalid shared block")?
                };
                projection.insert(key, (value.clone(), block.clone()));
                blocks.push(block);
            }
            doc.tabs.push(ContentTab {
                id,
                title,
                blocks: blocks.into(),
            });
        }
        self.projection = projection;
        if doc.tabs.is_empty() {
            return Err("A shared document needs a content tab".into());
        }
        let mut ids = std::collections::HashSet::new();
        // Concurrent copies/moves can create identical tab IDs; keep one projection.
        doc.tabs.retain(|t| ids.insert(t.id.clone()));
        let tab_id = &local.tabs[local.active_tab].id;
        doc.active_tab = doc.tabs.iter().position(|t| &t.id == tab_id).unwrap_or(0);
        doc.blocks = std::mem::take(&mut doc.tabs[doc.active_tab].blocks).to_vec();
        let mut total = 0;
        for i in 0..doc.tabs.len() {
            let blocks: &mut [Block] = if i == doc.active_tab {
                &mut doc.blocks
            } else {
                &mut doc.tabs[i].blocks
            };
            for block in blocks.iter_mut() {
                if let Block::Image(image) = block {
                    if let Some(object) = &image.editable {
                        if let Some((svg, data)) = self.derived_media.get(&image.id)
                            && svg == &object.svg
                        {
                            image.data = data.clone();
                        } else {
                            let (data, _, _) = crate::editable_objects::raster_preview(object)?;
                            self.derived_media
                                .insert(image.id.clone(), (object.svg.clone(), data.clone()));
                            image.data = data;
                        }
                        total += object.svg.len();
                    } else if let Some(data) = self.media.get(&image.id) {
                        image.data = data.clone();
                    } else if let Some(Out::Any(Any::Buffer(bytes))) =
                        self.assets.get(&txn, &image.id)
                    {
                        image.data = bytes.as_ref().to_vec().into();
                        self.media.insert(image.id.clone(), image.data.clone());
                    } else {
                        return Err("Shared image source is missing".into());
                    }
                    total += image.data.len();
                    if total > MAX_STATE {
                        return Err("Shared media exceeds 32 MiB".into());
                    }
                }
            }
        }
        doc.version = DOCUMENT_VERSION;
        doc.validate()?;
        Ok(doc)
    }
    fn texts(&self, txn: &impl ReadTxn, tab_id: &str) -> Vec<(Address, TextRef)> {
        let mut result = vec![];
        if let Some(Out::YMap(page)) = self.root.get(txn, "page") {
            for (row, name) in ["header", "footer"].iter().enumerate() {
                if let Some(Out::YMap(p)) = page.get(txn, name)
                    && let Some(Out::YText(text)) = p.get(txn, "chars")
                {
                    result.push((
                        Address {
                            block: usize::MAX,
                            row,
                            cell: 0,
                        },
                        text,
                    ));
                }
            }
        }
        let Some(Out::YArray(tabs)) = self.root.get(txn, "tabs") else {
            return result;
        };
        for tab in tabs.iter(txn) {
            let Out::YMap(tab) = tab else {
                continue;
            };
            if tab.get(txn, "id") != Some(Out::Any(tab_id.into())) {
                continue;
            }
            let Some(Out::YArray(blocks)) = tab.get(txn, "blocks") else {
                continue;
            };
            for (block, object) in blocks.iter(txn).enumerate() {
                let Out::YMap(object) = object else {
                    continue;
                };
                if let Some(Out::YMap(p)) = object.get(txn, "Paragraph") {
                    if let Some(Out::YText(text)) = p.get(txn, "chars") {
                        result.push((
                            Address {
                                block,
                                row: 0,
                                cell: 0,
                            },
                            text,
                        ));
                    }
                } else if let Some(Out::YArray(rows)) = object.get(txn, "Table") {
                    for (row, cells) in rows.iter(txn).enumerate() {
                        let Out::YArray(cells) = cells else {
                            continue;
                        };
                        for (cell, p) in cells.iter(txn).enumerate() {
                            if let Out::YMap(p) = p
                                && let Some(Out::YText(text)) = p.get(txn, "chars")
                            {
                                result.push((Address { block, row, cell }, text));
                            }
                        }
                    }
                }
            }
        }
        result
    }
    pub fn anchor(&self, document: &Document, pos: Position) -> Option<Anchor> {
        let tab = document.tabs[document.active_tab].id.clone();
        let txn = self.doc.transact();
        let (_, text) = self
            .texts(&txn, &tab)
            .into_iter()
            .find(|(a, _)| *a == pos.address)?;
        let byte = text
            .get_string(&txn)
            .chars()
            .take(pos.offset)
            .map(char::len_utf16)
            .sum::<usize>();
        let cursor = cursor_at(&text, &txn, byte, Assoc::After)?.encode_v1();
        Some(Anchor { tab, cursor })
    }
    pub fn resolve(&self, anchor: &Anchor) -> Option<Position> {
        if anchor.cursor.len() > 512 {
            return None;
        }
        let cursor = StickyIndex::decode_v1(&anchor.cursor).ok()?;
        let txn = self.doc.transact();
        let offset = cursor.get_offset(&txn)?;
        for (address, text) in self.texts(&txn, &anchor.tab) {
            if std::ptr::eq(text.as_ref(), offset.branch.as_ref()) {
                let s = text.get_string(&txn);
                let offset = scalar_at(&s, offset.index);
                return Some(Position { address, offset });
            }
        }
        None
    }
}

fn any(value: &Value) -> Result<Any, String> {
    Any::from_json(&value.to_string()).map_err(|_| "Invalid shared value".into())
}
fn json_any(value: &Any) -> Result<Value, String> {
    let mut s = String::new();
    value.to_json(&mut s);
    serde_json::from_str(&s)
        .map(normalize)
        .map_err(|_| "Invalid shared JSON".into())
}
fn normalize(value: Value) -> Value {
    match value {
        Value::Number(n) => {
            if let Some(v) = n.as_f64()
                && v.fract() == 0.0
                && v.abs() < 9_007_199_254_740_992.0
            {
                Value::from(v as i64)
            } else {
                Value::Number(n)
            }
        }
        Value::Array(a) => Value::Array(a.into_iter().map(normalize).collect()),
        Value::Object(m) => Value::Object(m.into_iter().map(|(k, v)| (k, normalize(v))).collect()),
        v => v,
    }
}
fn prelim(value: &Value, depth: usize) -> Result<In, String> {
    if depth > 64 {
        return Err("Shared content is too deeply nested".into());
    }
    Ok(match value {
        Value::Object(map) if map.len() == 1 && map.contains_key("runs") => {
            TextPrelim::new("").into()
        }
        Value::Object(map) => In::Map(MapPrelim::from_iter(
            map.iter()
                .map(|(k, v)| Ok((k.clone(), prelim(v, depth + 1)?)))
                .collect::<Result<Vec<_>, String>>()?,
        )),
        Value::Array(array) if !array.is_empty() && array.iter().all(Value::is_number) => {
            In::Any(any(value)?)
        }
        Value::Array(array) => In::Array(ArrayPrelim::from_iter(
            array
                .iter()
                .map(|v| prelim(v, depth + 1))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        _ => In::Any(any(value)?),
    })
}
fn read_value(
    out: &Out,
    txn: &impl ReadTxn,
    depth: usize,
    budget: &mut usize,
) -> Result<Value, String> {
    if depth > 64 || *budget == 0 {
        return Err("Shared content exceeds structural limits".into());
    }
    *budget -= 1;
    match out {
        Out::Any(a) => json_any(a),
        Out::YMap(m) => {
            let mut value: serde_json::Map<String, Value> = m
                .iter(txn)
                .filter(|(k, _)| *k != "_anchors")
                .map(|(k, v)| Ok((k.into(), read_value(&v, txn, depth + 1, budget)?)))
                .collect::<Result<_, String>>()?;
            if let Some(Out::YText(text)) = m.get(txn, "chars")
                && let Some(Out::YMap(anchors)) = m.get(txn, "_anchors")
                && let Some(Value::Array(notes)) = value.get_mut("annotations")
            {
                for note in notes {
                    if let Some(id) = note["id"].as_str()
                        && let Some(Out::Any(Any::Array(pair))) = anchors.get(txn, id)
                        && pair.len() == 2
                    {
                        let start = annotation_offset(&pair[0], &text, txn).unwrap_or(0);
                        let end = annotation_offset(&pair[1], &text, txn).unwrap_or(start);
                        note["start"] = start.min(end).into();
                        note["end"] = end.max(start).into();
                    }
                }
            }
            Ok(Value::Object(value))
        }
        Out::YArray(a) => Ok(Value::Array(
            a.iter(txn)
                .map(|v| read_value(&v, txn, depth + 1, budget))
                .collect::<Result<_, _>>()?,
        )),
        Out::YText(t) => {
            let mut runs = vec![];
            for diff in t.diff(txn, yrs::types::text::YChange::identity) {
                let Out::Any(Any::String(text)) = diff.insert else {
                    return Err("Unsupported embedded text object".into());
                };
                if text.len() > 2_000_000 {
                    return Err("Shared text is too large".into());
                }
                let mut style =
                    serde_json::to_value(TextStyle::default()).map_err(|e| e.to_string())?;
                if let Some(attrs) = diff.attributes {
                    for (k, v) in attrs.iter() {
                        style[k.as_ref()] = json_any(v)?;
                    }
                }
                runs.push(json!({"text":text.as_ref(),"style":style}));
            }
            Ok(normalize(json!({"runs":runs})))
        }
        _ => Err("Unsupported shared object type".into()),
    }
}
fn reconcile_map(
    map: &MapRef,
    txn: &mut TransactionMut,
    target: &serde_json::Map<String, Value>,
    depth: usize,
) -> Result<(), String> {
    let keys: Vec<_> = map.keys(txn).map(str::to_owned).collect();
    for key in keys {
        if key != "_anchors" && !target.contains_key(&key) {
            map.remove(txn, &key);
        }
    }
    for (key, value) in target {
        if let Some(out) = map.get(txn, key)
            && reconcile_value(&out, txn, value, depth + 1)?
        {
            continue;
        }
        let out = map.insert(txn, key.as_str(), prelim(value, depth + 1)?);
        initialize(&out, txn, value, depth + 1)?;
    }
    annotation_anchors(map, txn, target)?;
    Ok(())
}
fn annotation_offset(value: &Any, text: &TextRef, txn: &impl ReadTxn) -> Option<usize> {
    let Any::Buffer(bytes) = value else {
        return None;
    };
    let pos = StickyIndex::decode_v1(bytes).ok()?.get_offset(txn)?;
    if !std::ptr::eq(text.as_ref(), pos.branch.as_ref()) {
        return None;
    }
    Some(scalar_at(&text.get_string(txn), pos.index))
}
fn scalar_at(text: &str, index: u32) -> usize {
    text.chars()
        .scan(0u32, |offset, ch| {
            let start = *offset;
            *offset += ch.len_utf16() as u32;
            Some(start)
        })
        .take_while(|start| *start < index)
        .count()
}
fn cursor_at(text: &TextRef, txn: &impl ReadTxn, byte: usize, assoc: Assoc) -> Option<StickyIndex> {
    if byte == text.len(txn) as usize && assoc == Assoc::After {
        Some(StickyIndex::from_type(txn, text, Assoc::After))
    } else {
        text.sticky_index(txn, byte as u32, assoc)
    }
}
fn annotation_anchors(
    map: &MapRef,
    txn: &mut TransactionMut,
    target: &serde_json::Map<String, Value>,
) -> Result<(), String> {
    let Some(Out::YText(text)) = map.get(txn, "chars") else {
        return Ok(());
    };
    let Some(Value::Array(notes)) = target.get("annotations") else {
        return Ok(());
    };
    let anchors = match map.get(txn, "_anchors") {
        Some(Out::YMap(a)) => a,
        _ => map.insert(txn, "_anchors", MapPrelim::default()),
    };
    let keys: Vec<_> = anchors.keys(txn).map(str::to_owned).collect();
    for key in keys {
        if !notes.iter().any(|n| n["id"].as_str() == Some(&key)) {
            anchors.remove(txn, &key);
        }
    }
    for note in notes {
        let id = note["id"].as_str().ok_or("Invalid annotation identity")?;
        let start = note["start"].as_u64().ok_or("Invalid annotation range")? as usize;
        let end = note["end"].as_u64().ok_or("Invalid annotation range")? as usize;
        let existing = anchors.get(txn, id);
        if let Some(Out::Any(Any::Array(pair))) = existing
            && pair.len() == 2
            && annotation_offset(&pair[0], &text, txn) == Some(start)
            && annotation_offset(&pair[1], &text, txn) == Some(end)
        {
            continue;
        }
        let s = text.get_string(txn);
        let mut pair = vec![];
        for (endpoint, index) in [start, end].into_iter().enumerate() {
            let byte = s.chars().take(index).map(char::len_utf16).sum::<usize>();
            let cursor = cursor_at(
                &text,
                txn,
                byte,
                if endpoint == 0 {
                    Assoc::After
                } else {
                    Assoc::Before
                },
            )
            .ok_or("Invalid annotation cursor")?;
            pair.push(Any::Buffer(cursor.encode_v1().into()));
        }
        anchors.insert(txn, id, Any::Array(pair.into()));
    }
    Ok(())
}
fn initialize(
    out: &Out,
    txn: &mut TransactionMut,
    value: &Value,
    depth: usize,
) -> Result<(), String> {
    if depth > 64 {
        return Err("Shared content is too deeply nested".into());
    }
    // Newly integrated prelims need their rich-text payloads populated recursively.
    match (out, value) {
        (Out::YText(t), _) => reconcile_text(t, txn, value),
        (Out::YMap(m), Value::Object(values)) => {
            for (k, v) in values {
                if let Some(out) = m.get(txn, k) {
                    initialize(&out, txn, v, depth + 1)?;
                }
            }
            annotation_anchors(m, txn, values)
        }
        (Out::YArray(a), Value::Array(values)) => {
            for (i, v) in values.iter().enumerate() {
                if let Some(out) = a.get(txn, i as u32) {
                    initialize(&out, txn, v, depth + 1)?;
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
fn reconcile_value(
    out: &Out,
    txn: &mut TransactionMut,
    value: &Value,
    depth: usize,
) -> Result<bool, String> {
    if depth > 64 {
        return Err("Shared content is too deeply nested".into());
    }
    match (out, value) {
        (Out::YText(t), Value::Object(m)) if m.contains_key("runs") => {
            reconcile_text(t, txn, value)?
        }
        (Out::YMap(m), Value::Object(v)) => reconcile_map(m, txn, v, depth)?,
        (Out::YArray(a), Value::Array(v)) => {
            let old: Vec<_> = a
                .iter(txn)
                .map(|v| read_value(&v, txn, 0, &mut 200_000))
                .collect::<Result<_, _>>()?;
            let (mut i, mut j, mut index) = (0, 0, 0);
            while j < v.len() {
                if i < old.len() && old[i] == v[j] {
                    i += 1;
                    j += 1;
                    index += 1;
                    continue;
                }
                if i < old.len()
                    && let Some(k) = old[i + 1..].iter().position(|o| o == &v[j])
                {
                    let n = k + 1;
                    a.remove_range(txn, index, n as u32);
                    i += n;
                    continue;
                }
                let insert = i >= old.len() || v[j + 1..].contains(&old[i]);
                if !insert
                    && let Some(current) = a.get(txn, index)
                    && reconcile_value(&current, txn, &v[j], depth + 1)?
                {
                    i += 1;
                    j += 1;
                    index += 1;
                    continue;
                }
                if !insert {
                    a.remove(txn, index);
                    i += 1;
                }
                let out = a.insert(txn, index, prelim(&v[j], depth + 1)?);
                initialize(&out, txn, &v[j], depth + 1)?;
                j += 1;
                index += 1;
            }
            if i < old.len() {
                a.remove_range(txn, index, (old.len() - i) as u32);
            }
        }
        (Out::Any(a), v) if json_any(a)? == *v => {}
        _ => return Ok(false),
    }
    Ok(true)
}
fn styled(value: &Value) -> Result<Vec<StyledChar>, String> {
    let mut chars = vec![];
    for run in value["runs"].as_array().ok_or("Invalid text runs")? {
        let style: TextStyle =
            serde_json::from_value(run["style"].clone()).map_err(|_| "Invalid shared style")?;
        let text = run["text"].as_str().ok_or("Invalid shared text")?;
        if chars.len() + text.chars().count() > 2_000_000 {
            return Err("Shared paragraph is too large".into());
        }
        chars.extend(text.chars().map(|ch| StyledChar {
            ch,
            style: style.clone(),
        }));
    }
    Ok(chars)
}
fn reconcile_text(text: &TextRef, txn: &mut TransactionMut, value: &Value) -> Result<(), String> {
    let old = styled(&read_value(
        &Out::YText(text.clone()),
        txn,
        0,
        &mut 200_000,
    )?)?;
    let new = styled(value)?;
    let prefix = old
        .iter()
        .zip(&new)
        .take_while(|(a, b)| a.ch == b.ch)
        .count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(a, b)| a.ch == b.ch)
        .count();
    let start = old[..prefix]
        .iter()
        .map(|c| c.ch.len_utf16())
        .sum::<usize>() as u32;
    let remove = old[prefix..old.len() - suffix]
        .iter()
        .map(|c| c.ch.len_utf16())
        .sum::<usize>() as u32;
    if remove > 0 {
        text.remove_range(txn, start, remove);
    }
    let inserted: String = new[prefix..new.len() - suffix]
        .iter()
        .map(|c| c.ch)
        .collect();
    if !inserted.is_empty() {
        text.insert(txn, start, &inserted);
    }
    // Attributes are independent properties, so concurrent bold/italic can merge.
    let current = styled(&read_value(
        &Out::YText(text.clone()),
        txn,
        0,
        &mut 200_000,
    )?)?;
    let mut index = 0;
    for (a, b) in current.iter().zip(&new) {
        let len = b.ch.len_utf16() as u32;
        if a.style != b.style {
            let before = serde_json::to_value(&a.style).map_err(|e| e.to_string())?;
            let after = serde_json::to_value(&b.style).map_err(|e| e.to_string())?;
            let attrs: Attrs = after
                .as_object()
                .ok_or("Invalid style")?
                .iter()
                .filter(|(k, v)| before[*k] != **v)
                .map(|(k, v)| Ok((k.as_str().into(), any(v)?)))
                .collect::<Result<_, String>>()?;
            text.format(txn, index, len, attrs);
        }
        index += len;
    }
    Ok(())
}

//! Local, file-delivered signing packets bound to an immutable document snapshot.
//!
//! Signer names and dates are self-declared/local values. This module does not
//! provide identity verification, certificates, trusted timestamps, or hosted
//! delivery.

use crate::{
    layout::Layout,
    model::{Block, DOCUMENT_VERSION, Document, EmbeddedImage, Paragraph},
    persistence,
};
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage, imageops};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashSet},
    fs::File,
    io::{Cursor, Read},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use time::{OffsetDateTime, UtcOffset, macros::format_description};

pub const SIGNING_FORMAT_VERSION: u32 = 1;
pub const MAX_SIGNERS: usize = 10;
pub const MAX_FIELDS: usize = 200;
pub const MAX_PACKET_BYTES: u64 = 128 * 1024 * 1024;
const MAX_NAME_BYTES: usize = 256;
const MAX_TEXT_BYTES: usize = 4096;
const MAX_STROKES: usize = 64;
const MAX_POINTS_PER_STROKE: usize = 512;
const MAX_TOTAL_POINTS: usize = 5000;
const SIGNATURE_IMAGE_WIDTH: u32 = 720;
const SIGNATURE_IMAGE_HEIGHT: u32 = 200;
const SIGNATURE_RASTER_SCALE: u32 = 4;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SigningStatus {
    Pending,
    Completed,
    Declined,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SignerStatus {
    Completed,
    Declined,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    Signature,
    Initials,
    Name,
    Date,
    Text,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Signer {
    pub id: String,
    pub name: String,
}

impl Signer {
    pub fn new(name: impl Into<String>) -> Result<Self, String> {
        let name = clean_bounded(name.into(), MAX_NAME_BYTES, "Signer name")?;
        let id = random_id("signer");
        Ok(Self { id, name })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SigningField {
    pub id: String,
    pub signer_id: String,
    pub kind: FieldKind,
    pub required: bool,
    pub label: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Point {
    /// Normalized horizontal coordinate, inclusive range 0–1.
    pub x: f32,
    /// Normalized vertical coordinate, inclusive range 0–1.
    pub y: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Stroke {
    pub points: Vec<Point>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case", tag = "format", content = "value")]
pub enum FieldValue {
    Text(String),
    Date(String),
    TypedSignature(String),
    DrawnSignature(Vec<Stroke>),
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuditAction {
    RequestCreated,
    Signed,
    Declined,
    ResponseMerged,
    RequestCancelled,
    Completed,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditEvent {
    /// Local wall-clock timestamp recorded by this device; not independently trusted.
    pub local_time: String,
    pub actor: String,
    pub action: AuditAction,
    pub signer_id: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SigningRequest {
    pub format_version: u32,
    pub request_id: String,
    pub document_sha256: String,
    /// Hash of the immutable request assignment manifest as well as the document.
    pub manifest_sha256: String,
    /// Frozen schema-current document, including every content tab.
    pub document: Document,
    pub requester: String,
    pub signers: Vec<Signer>,
    pub fields: Vec<SigningField>,
    pub responses: Vec<SignerResponse>,
    pub status: SigningStatus,
    pub created_at: String,
    pub audit: Vec<AuditEvent>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SignerResponse {
    pub format_version: u32,
    pub request_id: String,
    pub document_sha256: String,
    pub manifest_sha256: String,
    pub signer_id: String,
    pub signer_name: String,
    pub status: SignerStatus,
    /// Generated locally when the signer submits. Date fields use this value.
    pub submitted_at: String,
    pub values: BTreeMap<String, FieldValue>,
    pub audit: Vec<AuditEvent>,
}

impl SigningRequest {
    pub fn new(
        document: &Document,
        requester: impl Into<String>,
        signers: Vec<Signer>,
        fields: Vec<SigningField>,
    ) -> Result<Self, String> {
        let mut frozen = document.clone();
        frozen.version = DOCUMENT_VERSION;
        frozen.validate()?;
        let requester = clean_bounded(requester.into(), MAX_NAME_BYTES, "Requester name")?;
        let created_at = local_timestamp();
        let mut request = Self {
            format_version: SIGNING_FORMAT_VERSION,
            request_id: random_id("request"),
            document_sha256: document_hash(&frozen)?,
            manifest_sha256: String::new(),
            document: frozen,
            requester: requester.clone(),
            signers,
            fields,
            responses: Vec::new(),
            status: SigningStatus::Pending,
            created_at: created_at.clone(),
            audit: vec![AuditEvent {
                local_time: created_at,
                actor: requester,
                action: AuditAction::RequestCreated,
                signer_id: None,
                detail: "Signing request created from a frozen document snapshot".into(),
            }],
        };
        request.manifest_sha256 = request.calculate_manifest_hash()?;
        request.validate()?;
        Ok(request)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.format_version != SIGNING_FORMAT_VERSION {
            return Err("Unsupported signing packet version".into());
        }
        if !valid_id(&self.request_id) {
            return Err("Invalid signing request identifier".into());
        }
        if self.document.version != DOCUMENT_VERSION {
            return Err("Signing snapshot must use the current document schema".into());
        }
        self.document.validate()?;
        if document_hash(&self.document)? != self.document_sha256 {
            return Err("Frozen document snapshot hash does not match the request".into());
        }
        validate_name(&self.requester, "Requester name")?;
        validate_timestamp(&self.created_at)?;
        if self.signers.is_empty() || self.signers.len() > MAX_SIGNERS {
            return Err(format!(
                "A signing request must have 1 to {MAX_SIGNERS} signers"
            ));
        }
        if self.fields.is_empty() || self.fields.len() > MAX_FIELDS {
            return Err(format!(
                "A signing request must have 1 to {MAX_FIELDS} fields"
            ));
        }
        let mut signer_ids = HashSet::new();
        for signer in &self.signers {
            if !valid_id(&signer.id) || !signer_ids.insert(&signer.id) {
                return Err("Invalid or duplicate signer identifier".into());
            }
            validate_name(&signer.name, "Signer name")?;
        }
        let mut field_ids = HashSet::new();
        for field in &self.fields {
            if !valid_id(&field.id) || !field_ids.insert(&field.id) {
                return Err("Invalid or duplicate signing field identifier".into());
            }
            if !signer_ids.contains(&field.signer_id) {
                return Err("Signing field is assigned to an unknown signer".into());
            }
            validate_optional_label(&field.label)?;
        }
        if self.calculate_manifest_hash()? != self.manifest_sha256 {
            return Err("Signing request manifest hash does not match its assignments".into());
        }
        if self
            .signers
            .iter()
            .any(|signer| !self.fields.iter().any(|field| field.signer_id == signer.id))
        {
            return Err("Every named signer must have at least one assigned field".into());
        }
        let mut responders = HashSet::new();
        for response in &self.responses {
            response.validate_for(self)?;
            if !responders.insert(&response.signer_id) {
                return Err("Duplicate response for a signer".into());
            }
        }
        let derived_status = status_for(&self.signers, &self.responses);
        let cancelled_pending = self.status == SigningStatus::Cancelled
            && derived_status == SigningStatus::Pending
            && self
                .audit
                .iter()
                .any(|event| event.action == AuditAction::RequestCancelled);
        if derived_status != self.status && !cancelled_pending {
            return Err("Signing request status does not match its responses".into());
        }
        validate_audit(&self.audit)?;
        Ok(())
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_PACKET_BYTES {
            return Err("Signing packet exceeds the 128 MB file limit".into());
        }
        persistence::atomic_write(path, &bytes)
    }

    fn calculate_manifest_hash(&self) -> Result<String, String> {
        #[derive(Serialize)]
        struct Manifest<'a> {
            format_version: u32,
            request_id: &'a str,
            document_sha256: &'a str,
            requester: &'a str,
            signers: &'a [Signer],
            fields: &'a [SigningField],
            created_at: &'a str,
        }
        let manifest = Manifest {
            format_version: self.format_version,
            request_id: &self.request_id,
            document_sha256: &self.document_sha256,
            requester: &self.requester,
            signers: &self.signers,
            fields: &self.fields,
            created_at: &self.created_at,
        };
        let bytes = serde_json::to_vec(&manifest).map_err(|error| error.to_string())?;
        Ok(Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect())
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = read_bounded(path)?;
        let request: Self = serde_json::from_slice(&bytes)
            .map_err(|e| format!("Cannot read AlphaDocs signing request: {e}"))?;
        request.validate()?;
        Ok(request)
    }

    pub fn cancel(&mut self, actor: impl Into<String>) -> Result<(), String> {
        self.validate()?;
        if self.status != SigningStatus::Pending {
            return Err("Only a pending signing request can be cancelled".into());
        }
        let actor = clean_bounded(actor.into(), MAX_NAME_BYTES, "Actor name")?;
        let at = local_timestamp();
        let mut updated = self.clone();
        updated.audit.push(AuditEvent {
            local_time: at,
            actor,
            action: AuditAction::RequestCancelled,
            signer_id: None,
            detail: "Requester cancelled the signing request".into(),
        });
        updated.status = SigningStatus::Cancelled;
        updated.validate()?;
        *self = updated;
        Ok(())
    }

    /// Merge a signer response against the original request and frozen snapshot.
    /// Identical repeated imports are harmless; conflicting or stale responses fail.
    pub fn merge_response(&mut self, response: SignerResponse) -> Result<(), String> {
        self.validate()?;
        response.validate_for(self)?;
        if let Some(existing) = self
            .responses
            .iter()
            .find(|existing| existing.signer_id == response.signer_id)
        {
            return if existing == &response {
                Ok(())
            } else {
                Err("A different response for this signer was already merged".into())
            };
        }
        if self.status != SigningStatus::Pending {
            return Err("This signing request is already closed".into());
        }
        let signer = self
            .signers
            .iter()
            .find(|signer| signer.id == response.signer_id)
            .ok_or_else(|| "Response belongs to an unknown signer".to_string())?;
        let at = local_timestamp();
        let mut updated = self.clone();
        updated.audit.extend(response.audit.iter().cloned());
        updated.audit.push(AuditEvent {
            local_time: at.clone(),
            actor: signer.name.clone(),
            action: AuditAction::ResponseMerged,
            signer_id: Some(signer.id.clone()),
            detail: "Signer response imported by requester".into(),
        });
        updated.responses.push(response);
        updated.status = status_for(&updated.signers, &updated.responses);
        if updated.status == SigningStatus::Completed {
            updated.audit.push(AuditEvent {
                local_time: at,
                actor: updated.requester.clone(),
                action: AuditAction::Completed,
                signer_id: None,
                detail: "All assigned signers completed the request".into(),
            });
        }
        updated.validate()?;
        *self = updated;
        Ok(())
    }

    pub fn is_completed(&self) -> bool {
        self.status == SigningStatus::Completed && self.validate().is_ok()
    }

    /// Export all frozen content tabs followed by a distinct signature/audit appendix.
    pub fn completed_pdf(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        if self.status != SigningStatus::Completed {
            return Err("Only a completed signing request can be exported".into());
        }
        let mut document = self.document.flattened();
        document.version = DOCUMENT_VERSION;
        let missing_fonts = missing_document_fonts(&document);
        document.blocks.push(Block::PageBreak);
        push_heading(&mut document.blocks, "Signature and audit appendix");
        push_paragraph(
            &mut document.blocks,
            &format!("Request ID: {}", self.request_id),
        );
        push_paragraph(
            &mut document.blocks,
            &format!("Frozen document SHA-256: {}", self.document_sha256),
        );
        push_paragraph(
            &mut document.blocks,
            &format!("Signing request manifest SHA-256: {}", self.manifest_sha256),
        );
        push_paragraph(
            &mut document.blocks,
            "Names and dates are self-declared and locally recorded; this appendix does not certify identity or trusted time.",
        );
        if !missing_fonts.is_empty() {
            push_paragraph(
                &mut document.blocks,
                &format!(
                    "Font warning: unavailable families [{}] were rendered with the bundled fallback.",
                    missing_fonts.join(", ")
                ),
            );
        }
        for signer in &self.signers {
            push_heading(&mut document.blocks, &signer.name);
            let response = self
                .responses
                .iter()
                .find(|response| response.signer_id == signer.id)
                .ok_or_else(|| "Completed request is missing a signer response".to_string())?;
            for field in self
                .fields
                .iter()
                .filter(|field| field.signer_id == signer.id)
            {
                let value = response
                    .values
                    .get(&field.id)
                    .ok_or_else(|| "Completed request is missing a required field".to_string())?;
                let label = if field.label.trim().is_empty() {
                    field_kind_label(field.kind)
                } else {
                    field.label.as_str()
                };
                match value {
                    FieldValue::DrawnSignature(strokes) => {
                        push_paragraph(&mut document.blocks, &format!("{label}:"));
                        document
                            .blocks
                            .push(Block::Image(signature_image(strokes)?));
                    }
                    FieldValue::Text(text)
                    | FieldValue::Date(text)
                    | FieldValue::TypedSignature(text) => {
                        push_paragraph(&mut document.blocks, &format!("{label}: {text}"));
                    }
                }
            }
            push_paragraph(
                &mut document.blocks,
                &format!("Submitted locally: {}", response.submitted_at),
            );
        }
        push_heading(&mut document.blocks, "Audit events");
        for event in &self.audit {
            let signer = event
                .signer_id
                .as_ref()
                .map(|id| format!(" ({id})"))
                .unwrap_or_default();
            push_paragraph(
                &mut document.blocks,
                &format!(
                    "{} — {} — {}{} — {}",
                    event.local_time,
                    audit_action_label(event.action),
                    event.actor,
                    signer,
                    event.detail
                ),
            );
        }
        document.validate()?;
        let context = eframe::egui::Context::default();
        crate::fonts::install(&context);
        let mut result = None;
        for _ in 0..2 {
            let mut output = context.run_ui(Default::default(), |_| {
                if !crate::fonts::prepare(&context, &document, &crate::model::TextStyle::default())
                {
                    let layout = Layout::measure(&context, &document);
                    result = Some(crate::compat::pdf::encode(&document, &layout));
                }
            });
            output.textures_delta.clear();
            if result.is_some() {
                break;
            }
        }
        result.expect("one egui frame measured the signing export")
    }
}

fn missing_document_fonts(document: &Document) -> Vec<String> {
    let mut names = std::collections::BTreeSet::new();
    let mut collect = |style: &crate::model::TextStyle| {
        if !crate::fonts::available(&style.font) {
            names.insert(style.font.clone());
        }
    };
    collect(&document.page.header.style);
    collect(&document.page.footer.style);
    for character in document
        .page
        .header
        .chars
        .iter()
        .chain(&document.page.footer.chars)
    {
        collect(&character.style);
    }
    for block in &document.blocks {
        let paragraphs: Vec<_> = match block {
            Block::Paragraph(paragraph) => vec![paragraph],
            Block::Table(rows) => rows.iter().flatten().collect(),
            _ => Vec::new(),
        };
        for paragraph in paragraphs {
            collect(&paragraph.style);
            for character in &paragraph.chars {
                collect(&character.style);
            }
        }
    }
    names.into_iter().collect()
}

impl SignerResponse {
    /// Complete the assigned fields. Date values are inserted from the local
    /// submission timestamp and cannot be supplied as arbitrary text.
    pub fn complete(
        request: &SigningRequest,
        signer_id: &str,
        mut values: BTreeMap<String, FieldValue>,
    ) -> Result<Self, String> {
        request.validate()?;
        if request.status != SigningStatus::Pending {
            return Err("This signing request is closed".into());
        }
        let signer = request
            .signers
            .iter()
            .find(|signer| signer.id == signer_id)
            .ok_or_else(|| "Unknown signer".to_string())?;
        if request
            .responses
            .iter()
            .any(|response| response.signer_id == signer_id)
        {
            return Err("This signer has already responded".into());
        }
        let submitted_at = local_timestamp();
        for field in request
            .fields
            .iter()
            .filter(|field| field.signer_id == signer_id)
        {
            if field.kind == FieldKind::Date {
                if values.contains_key(&field.id) {
                    return Err("Date fields are generated at submission".into());
                }
                values.insert(field.id.clone(), FieldValue::Date(submitted_at.clone()));
            }
        }
        let response = Self {
            format_version: SIGNING_FORMAT_VERSION,
            request_id: request.request_id.clone(),
            document_sha256: request.document_sha256.clone(),
            manifest_sha256: request.manifest_sha256.clone(),
            signer_id: signer.id.clone(),
            signer_name: signer.name.clone(),
            status: SignerStatus::Completed,
            submitted_at: submitted_at.clone(),
            values,
            audit: vec![AuditEvent {
                local_time: submitted_at,
                actor: signer.name.clone(),
                action: AuditAction::Signed,
                signer_id: Some(signer.id.clone()),
                detail: "Signer completed the assigned fields".into(),
            }],
        };
        response.validate_for(request)?;
        Ok(response)
    }

    pub fn decline(request: &SigningRequest, signer_id: &str) -> Result<Self, String> {
        request.validate()?;
        if request.status != SigningStatus::Pending {
            return Err("This signing request is closed".into());
        }
        let signer = request
            .signers
            .iter()
            .find(|signer| signer.id == signer_id)
            .ok_or_else(|| "Unknown signer".to_string())?;
        if request
            .responses
            .iter()
            .any(|response| response.signer_id == signer_id)
        {
            return Err("This signer has already responded".into());
        }
        let submitted_at = local_timestamp();
        let response = Self {
            format_version: SIGNING_FORMAT_VERSION,
            request_id: request.request_id.clone(),
            document_sha256: request.document_sha256.clone(),
            manifest_sha256: request.manifest_sha256.clone(),
            signer_id: signer.id.clone(),
            signer_name: signer.name.clone(),
            status: SignerStatus::Declined,
            submitted_at: submitted_at.clone(),
            values: BTreeMap::new(),
            audit: vec![AuditEvent {
                local_time: submitted_at,
                actor: signer.name.clone(),
                action: AuditAction::Declined,
                signer_id: Some(signer.id.clone()),
                detail: "Signer declined the request".into(),
            }],
        };
        response.validate_for(request)?;
        Ok(response)
    }

    pub fn validate_for(&self, request: &SigningRequest) -> Result<(), String> {
        if self.format_version != SIGNING_FORMAT_VERSION
            || self.request_id != request.request_id
            || self.document_sha256 != request.document_sha256
            || document_hash(&request.document)? != request.document_sha256
            || request.calculate_manifest_hash()? != request.manifest_sha256
            || self.manifest_sha256 != request.manifest_sha256
        {
            return Err(
                "Signer response belongs to a different request or document snapshot".into(),
            );
        }
        validate_timestamp(&self.submitted_at)?;
        validate_name(&self.signer_name, "Signer name")?;
        let signer = request
            .signers
            .iter()
            .find(|signer| signer.id == self.signer_id)
            .ok_or_else(|| "Response belongs to an unknown signer".to_string())?;
        if signer.name != self.signer_name {
            return Err("Response signer name does not match the named assignment".into());
        }
        validate_audit(&self.audit)?;
        let expected_action = match self.status {
            SignerStatus::Completed => AuditAction::Signed,
            SignerStatus::Declined => AuditAction::Declined,
        };
        if !self.audit.iter().any(|event| {
            event.action == expected_action
                && event.local_time == self.submitted_at
                && event.actor == self.signer_name
                && event.signer_id.as_deref() == Some(&self.signer_id)
        }) {
            return Err("Signer response audit does not match its submitted status".into());
        }
        match self.status {
            SignerStatus::Declined => {
                if !self.values.is_empty() {
                    return Err("Declined response cannot contain filled fields".into());
                }
            }
            SignerStatus::Completed => {
                let assigned: Vec<_> = request
                    .fields
                    .iter()
                    .filter(|field| field.signer_id == self.signer_id)
                    .collect();
                if assigned.is_empty() {
                    return Err("Signer has no assigned fields".into());
                }
                let allowed: HashSet<_> = assigned.iter().map(|field| field.id.as_str()).collect();
                if self.values.keys().any(|id| !allowed.contains(id.as_str())) {
                    return Err("Response contains a field not assigned to this signer".into());
                }
                for field in assigned {
                    match self.values.get(&field.id) {
                        Some(value) => validate_field_value(field, value, &self.submitted_at)?,
                        None if field.required => {
                            return Err(format!("Required field '{}' is incomplete", field.id));
                        }
                        None => {}
                    }
                }
            }
        }
        Ok(())
    }

    pub fn save(&self, request: &SigningRequest, path: &Path) -> Result<(), String> {
        self.validate_for(request)?;
        let bytes = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_PACKET_BYTES {
            return Err("Signer response exceeds the 128 MB file limit".into());
        }
        persistence::atomic_write(path, &bytes)
    }

    pub fn load(path: &Path, request: &SigningRequest) -> Result<Self, String> {
        request.validate()?;
        let bytes = read_bounded(path)?;
        let response: Self = serde_json::from_slice(&bytes)
            .map_err(|e| format!("Cannot read AlphaDocs signer response: {e}"))?;
        response.validate_for(request)?;
        Ok(response)
    }
}

fn validate_field_value(
    field: &SigningField,
    value: &FieldValue,
    submitted_at: &str,
) -> Result<(), String> {
    let valid = match (field.kind, value) {
        (FieldKind::Signature, FieldValue::TypedSignature(text)) => {
            validate_text(text, "Typed signature").is_ok()
        }
        (FieldKind::Signature, FieldValue::DrawnSignature(strokes)) => {
            validate_strokes(strokes).is_ok()
        }
        (FieldKind::Initials | FieldKind::Name | FieldKind::Text, FieldValue::Text(text)) => {
            validate_text(text, "Field value").is_ok()
        }
        (FieldKind::Date, FieldValue::Date(date)) => date == submitted_at,
        _ => false,
    };
    if !valid {
        return Err(format!("Invalid value for signing field '{}'", field.id));
    }
    if let FieldValue::Text(text) | FieldValue::TypedSignature(text) = value {
        validate_text(text, "Field value")?;
    }
    if let FieldValue::DrawnSignature(strokes) = value {
        validate_strokes(strokes)?;
    }
    Ok(())
}

fn validate_strokes(strokes: &[Stroke]) -> Result<(), String> {
    if strokes.is_empty() || strokes.len() > MAX_STROKES {
        return Err("Drawn signature has an invalid number of strokes".into());
    }
    let mut total = 0usize;
    for stroke in strokes {
        if stroke.points.is_empty() || stroke.points.len() > MAX_POINTS_PER_STROKE {
            return Err("Drawn signature stroke has an invalid point count".into());
        }
        total += stroke.points.len();
        if total > MAX_TOTAL_POINTS
            || stroke.points.iter().any(|point| {
                !point.x.is_finite()
                    || !point.y.is_finite()
                    || !(0.0..=1.0).contains(&point.x)
                    || !(0.0..=1.0).contains(&point.y)
            })
        {
            return Err("Drawn signature coordinates exceed their bounds".into());
        }
    }
    Ok(())
}

fn signature_image(strokes: &[Stroke]) -> Result<EmbeddedImage, String> {
    validate_strokes(strokes)?;
    let raster_width = SIGNATURE_IMAGE_WIDTH * SIGNATURE_RASTER_SCALE;
    let raster_height = SIGNATURE_IMAGE_HEIGHT * SIGNATURE_RASTER_SCALE;
    let mut canvas = RgbaImage::from_pixel(raster_width, raster_height, Rgba([255, 255, 255, 0]));
    let ink = Rgba([25, 43, 70, 255]);
    for stroke in strokes {
        let points: Vec<_> = stroke
            .points
            .iter()
            .map(|point| {
                (
                    (point.x * (raster_width - 1) as f32).round() as i32,
                    (point.y * (raster_height - 1) as f32).round() as i32,
                )
            })
            .collect();
        if points.len() == 1 {
            paint_dot(
                &mut canvas,
                points[0].0,
                points[0].1,
                ink,
                2 * SIGNATURE_RASTER_SCALE as i32,
            );
        }
        for pair in points.windows(2) {
            paint_line(
                &mut canvas,
                pair[0],
                pair[1],
                ink,
                2 * SIGNATURE_RASTER_SCALE as i32,
            );
        }
    }
    let canvas = imageops::resize(
        &canvas,
        SIGNATURE_IMAGE_WIDTH,
        SIGNATURE_IMAGE_HEIGHT,
        imageops::FilterType::Lanczos3,
    );
    let mut bytes = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(canvas)
        .write_to(&mut bytes, ImageFormat::Png)
        .map_err(|e| format!("Cannot render signature strokes: {e}"))?;
    let image = EmbeddedImage {
        name: "Drawn signature".into(),
        alt_text: "Signer-drawn signature".into(),
        data: bytes.into_inner().into(),
        width: 216.0,
        height: 60.0,
        ..Default::default()
    };
    Ok(image)
}

fn paint_line(
    canvas: &mut RgbaImage,
    start: (i32, i32),
    end: (i32, i32),
    color: Rgba<u8>,
    radius: i32,
) {
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    let steps = dx.abs().max(dy.abs()).max(1);
    for step in 0..=steps {
        let t = step as f32 / steps as f32;
        let x = (start.0 as f32 + dx as f32 * t).round() as i32;
        let y = (start.1 as f32 + dy as f32 * t).round() as i32;
        paint_dot(canvas, x, y, color, radius);
    }
}

fn paint_dot(canvas: &mut RgbaImage, x: i32, y: i32, color: Rgba<u8>, radius: i32) {
    for offset_y in -radius..=radius {
        for offset_x in -radius..=radius {
            if offset_x * offset_x + offset_y * offset_y <= radius * radius {
                let px = x + offset_x;
                let py = y + offset_y;
                if px >= 0 && py >= 0 && px < canvas.width() as i32 && py < canvas.height() as i32 {
                    canvas.put_pixel(px as u32, py as u32, color);
                }
            }
        }
    }
}

fn status_for(signers: &[Signer], responses: &[SignerResponse]) -> SigningStatus {
    if responses
        .iter()
        .any(|response| response.status == SignerStatus::Declined)
    {
        SigningStatus::Declined
    } else if signers.iter().all(|signer| {
        responses.iter().any(|response| {
            response.signer_id == signer.id && response.status == SignerStatus::Completed
        })
    }) {
        SigningStatus::Completed
    } else {
        SigningStatus::Pending
    }
}

fn document_hash(document: &Document) -> Result<String, String> {
    let mut current = document.clone();
    current.version = DOCUMENT_VERSION;
    current.validate()?;
    let bytes = serde_json::to_vec(&current).map_err(|e| e.to_string())?;
    let digest = Sha256::digest(bytes);
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() > MAX_PACKET_BYTES {
        return Err("Signing packet exceeds the 128 MB file limit".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_PACKET_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_PACKET_BYTES {
        return Err("Signing packet exceeds the 128 MB file limit".into());
    }
    Ok(bytes)
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn random_id(prefix: &str) -> String {
    let mut bytes = [0u8; 16];
    rand::rng().fill_bytes(&mut bytes);
    format!(
        "{prefix}-{}",
        bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

fn clean_bounded(value: String, max_bytes: usize, label: &str) -> Result<String, String> {
    let value = value.trim().to_owned();
    if value.is_empty() || value.len() > max_bytes || value.chars().any(char::is_control) {
        return Err(format!(
            "{label} is empty or exceeds its allowed text bounds"
        ));
    }
    Ok(value)
}

fn validate_name(value: &str, label: &str) -> Result<(), String> {
    if value.trim().is_empty()
        || value.len() > MAX_NAME_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(format!("Invalid {label}"));
    }
    Ok(())
}

fn validate_text(value: &str, label: &str) -> Result<(), String> {
    if value.trim().is_empty()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(format!(
            "{label} is empty or exceeds its allowed text bounds"
        ));
    }
    Ok(())
}

fn validate_optional_label(value: &str) -> Result<(), String> {
    if value.len() > MAX_NAME_BYTES || value.chars().any(char::is_control) {
        return Err("Signing field label exceeds its allowed text bounds".into());
    }
    Ok(())
}

fn validate_timestamp(value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 64 || value.chars().any(char::is_control) {
        return Err("Invalid local signing timestamp".into());
    }
    Ok(())
}

fn validate_audit(events: &[AuditEvent]) -> Result<(), String> {
    if events.is_empty() || events.len() > 1024 {
        return Err("Signing audit trail has an invalid event count".into());
    }
    for event in events {
        validate_timestamp(&event.local_time)?;
        validate_name(&event.actor, "audit actor")?;
        if event.detail.len() > MAX_TEXT_BYTES || event.detail.chars().any(char::is_control) {
            return Err("Signing audit event exceeds its text bounds".into());
        }
        if event.signer_id.as_deref().is_some_and(|id| !valid_id(id)) {
            return Err("Invalid audit signer identifier".into());
        }
    }
    Ok(())
}

fn local_timestamp() -> String {
    OffsetDateTime::now_local()
        .unwrap_or_else(|_| OffsetDateTime::now_utc().to_offset(UtcOffset::UTC))
        .format(format_description!("[year]-[month]-[day]T[hour]:[minute]:[second][offset_hour sign:mandatory]:[offset_minute]"))
        .unwrap_or_else(|_| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                .to_string()
        })
}

fn field_kind_label(kind: FieldKind) -> &'static str {
    match kind {
        FieldKind::Signature => "Signature",
        FieldKind::Initials => "Initials",
        FieldKind::Name => "Name",
        FieldKind::Date => "Date signed",
        FieldKind::Text => "Text",
    }
}

fn audit_action_label(action: AuditAction) -> &'static str {
    match action {
        AuditAction::RequestCreated => "Request created",
        AuditAction::Signed => "Signed",
        AuditAction::Declined => "Declined",
        AuditAction::ResponseMerged => "Response merged",
        AuditAction::RequestCancelled => "Request cancelled",
        AuditAction::Completed => "Completed",
    }
}

fn push_heading(blocks: &mut Vec<Block>, text: &str) {
    let mut paragraph = Paragraph::from_text(text);
    paragraph.set_heading(2);
    blocks.push(Block::Paragraph(paragraph));
}

fn push_paragraph(blocks: &mut Vec<Block>, text: &str) {
    let paragraph = Paragraph::from_text(text);
    blocks.push(Block::Paragraph(paragraph));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (SigningRequest, Signer) {
        let mut document = Document::new("Frozen source");
        document.blocks = vec![Block::Paragraph(Paragraph::from_text("Source body"))];
        let mut second = Document::new("temporary");
        second.blocks = vec![Block::Paragraph(Paragraph::from_text("Second tab text"))];
        let tab = crate::model::ContentTab {
            id: format!("tab-{}", second.id),
            title: "Second tab".into(),
            blocks: second.blocks.into(),
        };
        document.tabs.push(tab);
        let signer = Signer::new("Alex Example").unwrap();
        let fields = vec![
            SigningField {
                id: "signature-1".into(),
                signer_id: signer.id.clone(),
                kind: FieldKind::Signature,
                required: true,
                label: "Signature".into(),
            },
            SigningField {
                id: "name-1".into(),
                signer_id: signer.id.clone(),
                kind: FieldKind::Name,
                required: true,
                label: "Printed name".into(),
            },
            SigningField {
                id: "date-1".into(),
                signer_id: signer.id.clone(),
                kind: FieldKind::Date,
                required: true,
                label: "Date signed".into(),
            },
        ];
        let request =
            SigningRequest::new(&document, "Requester", vec![signer.clone()], fields).unwrap();
        (request, signer)
    }

    fn response(request: &SigningRequest, signer: &Signer) -> SignerResponse {
        let mut values = BTreeMap::new();
        values.insert(
            "signature-1".into(),
            FieldValue::DrawnSignature(vec![Stroke {
                points: vec![
                    Point { x: 0.05, y: 0.7 },
                    Point { x: 0.45, y: 0.2 },
                    Point { x: 0.9, y: 0.65 },
                ],
            }]),
        );
        values.insert("name-1".into(), FieldValue::Text("Alex Example".into()));
        SignerResponse::complete(request, &signer.id, values).unwrap()
    }

    #[test]
    fn request_and_response_roundtrip_bind_all_tabs_and_export_appendix_without_mutation() {
        let (mut request, signer) = fixture();
        let Block::Paragraph(source_paragraph) = &mut request.document.blocks[0] else {
            panic!("fixture source content is a paragraph")
        };
        source_paragraph.style.font = "Unavailable Signing Test Face".into();
        request.document_sha256 = document_hash(&request.document).unwrap();
        request.manifest_sha256 = request.calculate_manifest_hash().unwrap();
        request.validate().unwrap();
        let original = request.document.clone();
        let response = response(&request, &signer);
        let date = response.values.get("date-1").unwrap();
        assert_eq!(date, &FieldValue::Date(response.submitted_at.clone()));

        let base = std::env::temp_dir().join(format!("signing-{}", request.request_id));
        std::fs::create_dir_all(&base).unwrap();
        let request_path = base.join("request.asign");
        let response_path = base.join("response.asign");
        request.save(&request_path).unwrap();
        response.save(&request, &response_path).unwrap();
        request = SigningRequest::load(&request_path).unwrap();
        let response = SignerResponse::load(&response_path, &request).unwrap();

        request.merge_response(response.clone()).unwrap();
        assert_eq!(request.status, SigningStatus::Completed);
        assert!(request.is_completed());
        request.merge_response(response).unwrap();
        let pdf = request.completed_pdf().unwrap();
        assert!(pdf.starts_with(b"%PDF"));
        let parsed = printpdf::PdfDocument::parse(
            &pdf,
            &printpdf::PdfParseOptions::default(),
            &mut Vec::new(),
        )
        .unwrap();
        let extracted = parsed
            .pages
            .iter()
            .flat_map(|page| page.extract_text(&parsed.resources))
            .collect::<Vec<_>>()
            .join("");
        assert!(extracted.contains("Sourcebody"), "{extracted}");
        assert!(extracted.contains("Secondtabtext"), "{extracted}");
        assert!(
            extracted.contains("Signatureandauditappendix"),
            "{extracted}"
        );
        assert!(extracted.contains("AlexExample"), "{extracted}");
        assert!(extracted.contains("Completed"), "{extracted}");
        assert!(
            extracted.contains("SigningrequestmanifestSHA-256"),
            "{extracted}"
        );
        assert!(
            extracted.contains("Fontwarning:unavailablefamilies[UnavailableSigningTestFace]"),
            "{extracted}"
        );
        assert_eq!(request.document, original);
        assert_eq!(request.document_sha256, document_hash(&original).unwrap());
        assert!(
            request
                .audit
                .iter()
                .any(|event| event.action == AuditAction::Completed)
        );

        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn responses_reject_wrong_hash_wrong_signer_missing_fields_and_conflicts() {
        let (mut request, signer) = fixture();
        let valid = response(&request, &signer);
        let mut wrong_hash = valid.clone();
        wrong_hash.document_sha256 = "0".repeat(64);
        assert!(wrong_hash.validate_for(&request).is_err());

        let mut wrong_signer = valid.clone();
        wrong_signer.signer_name = "Someone else".into();
        assert!(wrong_signer.validate_for(&request).is_err());

        let mut reassigned_request = request.clone();
        reassigned_request.fields[0].signer_id = format!("{}-changed", signer.id);
        assert!(reassigned_request.validate().is_err());
        assert!(valid.validate_for(&reassigned_request).is_err());

        let mut relabeled_request = request.clone();
        relabeled_request.fields[0].required = false;
        assert!(relabeled_request.validate().is_err());

        let mut values = BTreeMap::new();
        values.insert(
            "signature-1".into(),
            FieldValue::TypedSignature("Alex".into()),
        );
        assert!(SignerResponse::complete(&request, &signer.id, values).is_err());

        request.merge_response(valid.clone()).unwrap();
        let mut conflict = valid;
        conflict
            .values
            .insert("name-1".into(), FieldValue::Text("Changed".into()));
        assert!(request.merge_response(conflict).is_err());
    }

    #[test]
    fn drawn_signatures_are_bounded_and_decline_and_cancel_are_terminal() {
        let (mut request, signer) = fixture();
        let mut invalid = BTreeMap::new();
        invalid.insert(
            "signature-1".into(),
            FieldValue::DrawnSignature(vec![Stroke {
                points: vec![Point {
                    x: f32::NAN,
                    y: 0.4,
                }],
            }]),
        );
        invalid.insert("name-1".into(), FieldValue::Text("Alex".into()));
        assert!(SignerResponse::complete(&request, &signer.id, invalid).is_err());

        let declined = SignerResponse::decline(&request, &signer.id).unwrap();
        request.merge_response(declined).unwrap();
        assert_eq!(request.status, SigningStatus::Declined);
        assert!(request.cancel("Requester").is_err());
        assert!(request.completed_pdf().is_err());

        let (mut cancelled, _) = fixture();
        let signer_id = cancelled.signers[0].id.clone();
        cancelled.cancel("Requester").unwrap();
        assert_eq!(cancelled.status, SigningStatus::Cancelled);
        assert!(SignerResponse::decline(&cancelled, &signer_id).is_err());
    }
}

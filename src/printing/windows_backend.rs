//! Safe platform wrappers contain the Windows FFI; AlphaDocs does not use unsafe code.
use super::*;
use winprint::{
    printer::{FilePrinter, PrinterDevice, WinPdfPrinter},
    ticket::{
        Copies, PrintCapabilities, PrintTicket, PrintTicketBuilder,
        document::{
            NS_PSF, NS_PSK, PrintTicketDocument, WithProperties, WithScoredProperties,
            reader::ParsableXmlDocument,
        },
    },
};

fn device(name: &str) -> Result<PrinterDevice, String> {
    PrinterDevice::all()
        .map_err(|e| format!("Could not list printers: {e}"))?
        .into_iter()
        .find(|p| p.name() == name)
        .ok_or_else(|| {
            "The selected printer is no longer available. Refresh the printer list.".into()
        })
}
fn com() -> Result<winsafe::guard::CoUninitializeGuard, String> {
    winsafe::CoInitializeEx(winsafe::co::COINIT::MULTITHREADED)
        .map_err(|e| format!("Could not initialize printing: {e}"))
}
pub fn discover() -> Result<Vec<Printer>, String> {
    let default = winsafe::GetDefaultPrinter().ok();
    let mut printers: Vec<_> = PrinterDevice::all()
        .map_err(|e| format!("Could not list printers: {e}"))?
        .into_iter()
        .map(|p| Printer {
            is_default: default.as_deref() == Some(p.name()),
            name: p.name().into(),
        })
        .collect();
    printers.sort_by(|a, b| b.is_default.cmp(&a.is_default).then(a.name.cmp(&b.name)));
    Ok(printers)
}
fn description(option: &winprint::ticket::document::PrintFeatureOption) -> String {
    option
        .get_property("DisplayName", Some(NS_PSK))
        .and_then(|v| v.value.as_ref())
        .and_then(|v| v.string())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            option
                .name
                .as_ref()
                .map_or("Driver default".into(), |n| n.local_name.clone())
        })
}
fn option_monochrome(option: &winprint::ticket::document::PrintFeatureOption) -> Option<bool> {
    // Class drivers may advertise AutoSelect on monochrome hardware. Use the physical
    // device depth; DriverBitsPerPixel is often 24 even when the device is mono.
    let device_bits = option
        .get_scored_property("DeviceBitsPerPixel", Some(NS_PSK))
        .and_then(|p| p.value.as_ref())
        .and_then(|v| v.integer());
    if device_bits == Some(1) {
        return Some(true);
    }
    option
        .name
        .as_ref()
        .and_then(|n| match n.local_name.as_str() {
            "Grayscale" | "Monochrome" => Some(true),
            "Color" => Some(false),
            _ => None,
        })
}
pub fn capabilities(printer: &str) -> Result<Capabilities, String> {
    let _com = com()?;
    let xml = PrintCapabilities::fetch_xml(&device(printer)?)
        .map_err(|e| format!("The printer driver could not report its settings: {e:?}"))?;
    let xml = filter_device_constraints(&xml)?;
    let caps = PrintCapabilities {
        document: winprint::ticket::document::PrintCapabilitiesDocument::parse_from_bytes(xml)
            .map_err(|e| format!("Could not read printer capabilities: {e:?}"))?,
    };
    let media = caps
        .page_media_sizes()
        .filter_map(|m| {
            use winprint::ticket::FeatureOptionPack;
            let size = m.size();
            let points = [
                size.width_in_micron() as f32 * 72.0 / 25400.0,
                size.height_in_micron() as f32 * 72.0 / 25400.0,
            ];
            if !points.iter().all(|v| v.is_finite() && *v > 0.0) {
                return None;
            }
            let label = m
                .display_name()
                .map(str::to_owned)
                .unwrap_or_else(|| description(m.option()));
            Some(Media {
                option: DriverOption {
                    label,
                    ticket: PrintTicket::from(m).into_xml(),
                    landscape: None,
                    monochrome: false,
                },
                size: points,
            })
        })
        .collect();
    let mut features = Vec::new();
    // Finishing is shown only if the driver actually publishes it. Keep its private option XML.
    for (names, label) in [
        (&["PageOrientation"][..], "Orientation"),
        (&["PageOutputColor"][..], "Colour"),
        (
            &["JobDuplexAllDocumentsContiguously", "DocumentDuplex"][..],
            "Two-sided",
        ),
        (
            &["JobInputBin", "DocumentInputBin", "PageInputBin"][..],
            "Paper tray",
        ),
        (&["JobStapleAllDocuments", "DocumentStaple"][..], "Stapling"),
        (
            &["DocumentCollate", "JobCollateAllDocuments"][..],
            "Collation",
        ),
        (&["PageResolution"][..], "Quality"),
    ] {
        if let Some(feature) = caps.document.features.iter().find(|f| {
            names.contains(&f.name.local_name.as_str()) && f.name.namespace_ref() == Some(NS_PSK)
        }) {
            let options = feature
                .options
                .iter()
                .filter(|o| {
                    label != "Orientation"
                        || o.name.as_ref().is_some_and(|n| {
                            matches!(n.local_name.as_str(), "Portrait" | "Landscape")
                        })
                })
                .map(|o| {
                    let mut selected = feature.clone();
                    selected.options = vec![o.clone()];
                    // Capability-only SelectionType/Constrained properties are not ticket settings.
                    selected.properties.clear();
                    selected.options[0].properties.clear();
                    selected.features.clear();
                    let ticket: PrintTicket = PrintTicketDocument {
                        properties: vec![],
                        parameter_inits: caps
                            .default_parameters_for(&o.parameters_dependent())
                            .collect(),
                        features: vec![selected],
                    }
                    .into();
                    DriverOption {
                        label: description(o),
                        ticket: ticket.into_xml(),
                        landscape: if label == "Orientation" {
                            o.name.as_ref().map(|n| n.local_name == "Landscape")
                        } else {
                            None
                        },
                        monochrome: label == "Colour" && option_monochrome(o) == Some(true),
                    }
                })
                .collect::<Vec<_>>();
            if !options.is_empty()
                && (options.len() > 1 || matches!(label, "Orientation" | "Colour"))
            {
                features.push(Feature {
                    label: label.into(),
                    options,
                });
            }
        }
    }
    let max_copies = caps
        .document
        .parameter_defs
        .iter()
        .find(|p| {
            p.name.local_name == "JobCopiesAllDocuments" && p.name.namespace_ref() == Some(NS_PSK)
        })
        .and_then(|p| p.get_property("MaxValue", Some(NS_PSF)))
        .and_then(|p| p.value.as_ref())
        .and_then(|v| v.integer())
        .and_then(|v| u16::try_from(v).ok());
    Ok(Capabilities {
        media,
        features,
        max_copies,
    })
}
/// The wrapper DOM drops Option's `constrained` attribute. Remove unavailable
/// hardware/administrative options before parsing, while retaining ticket-dependent
/// choices: complete-ticket validation handles their combinations.
fn filter_device_constraints(xml: &[u8]) -> Result<Vec<u8>, String> {
    use quick_xml::{
        NsReader, Writer,
        events::Event,
        name::{QName, ResolveResult},
    };
    let mut reader = NsReader::from_reader(xml);
    let mut writer = Writer::new(Vec::new());
    let mut skip = 0usize;
    loop {
        let event = reader
            .read_event()
            .map_err(|e| format!("Invalid printer capabilities XML: {e}"))?;
        match &event {
            Event::Start(start) | Event::Empty(start) => {
                let is_start = matches!(event, Event::Start(_));
                if skip > 0 {
                    if is_start {
                        skip += 1;
                    }
                    continue;
                }
                let option = reader.resolver().resolve_element(start.name());
                let unavailable = matches!(&option.0,ResolveResult::Bound(n) if n.as_ref()==NS_PSF.as_bytes())
                    && option.1.as_ref() == b"Option"
                    && start
                        .attributes()
                        .flatten()
                        .find(|a| a.key.as_ref() == b"constrained")
                        .is_some_and(|a| {
                            let (ns, value) = reader.resolver().resolve_element(QName(&a.value));
                            matches!(&ns,ResolveResult::Bound(n) if n.as_ref()==NS_PSK.as_bytes())
                                && matches!(value.as_ref(), b"DeviceSettings" | b"AdminSettings")
                        });
                if unavailable {
                    if is_start {
                        skip = 1;
                    }
                    continue;
                }
            }
            Event::End(_) if skip > 0 => {
                skip -= 1;
                continue;
            }
            Event::Eof => break,
            _ if skip > 0 => continue,
            _ => {}
        }
        writer.write_event(event).map_err(|e| e.to_string())?;
    }
    Ok(writer.into_inner())
}
pub fn validate(printer: &str, settings: &Settings) -> Result<Validated, String> {
    let _com = com()?;
    let device = device(printer)?;
    let mut builder = PrintTicketBuilder::new(&device)
        .map_err(|e| format!("Could not open driver settings: {e:?}"))?;
    // Merge a single complete delta; per-field merging can silently undo an earlier choice.
    let mut requested = PrintTicketDocument {
        properties: vec![],
        parameter_inits: vec![],
        features: vec![],
    };
    for xml in &settings.tickets {
        let ticket = PrintTicketDocument::parse_from_bytes(xml)
            .map_err(|e| format!("Invalid driver option: {e:?}"))?;
        requested.features.extend(ticket.features);
        for parameter in ticket.parameter_inits {
            if !requested
                .parameter_inits
                .iter()
                .any(|p| p.name == parameter.name)
            {
                requested.parameter_inits.push(parameter);
            }
        }
    }
    let copies: PrintTicket = Copies(settings.copies).into();
    let copies = PrintTicketDocument::parse_from_bytes(copies.get_xml())
        .map_err(|e| format!("Invalid copies setting: {e:?}"))?;
    requested.parameter_inits.extend(copies.parameter_inits);
    let delta: PrintTicket = requested.clone().into();
    builder
        .merge(delta)
        .map_err(|e| format!("These settings are incompatible with the printer: {e:?}"))?;
    let ticket = builder
        .build()
        .map_err(|e| format!("Could not validate printer settings: {e:?}"))?;
    let relevant = relevant_ticket(ticket.get_xml(), &requested)?;
    let accepted = PrintTicketDocument::parse_from_bytes(&relevant)
        .map_err(|e| format!("Could not read validated settings: {e:?}"))?;
    for feature in &requested.features {
        let found = accepted.features.iter().find(|f| {
            f.name.local_name == feature.name.local_name
                && f.name.namespace == feature.name.namespace
        });
        let requested_name = feature.options.first().and_then(|o| o.name.as_ref());
        let accepted_name = found
            .and_then(|f| f.options.first())
            .and_then(|o| o.name.as_ref());
        if requested_name.is_some_and(|name| {
            !accepted_name.is_some_and(|accepted| {
                name.local_name == accepted.local_name && name.namespace == accepted.namespace
            })
        }) {
            return Err(format!(
                "The printer changed {} because the combination is unsupported. Choose a different setting.",
                feature.name.local_name
            ));
        }
        if let Some(option) = feature.options.first().filter(|o| o.name.is_none()) {
            let accepted = found
                .and_then(|f| f.options.first())
                .ok_or("Printer omitted a requested quality setting")?;
            for property in &option.scored_properties {
                if !accepted.scored_properties.iter().any(|p| {
                    p.name == property.name
                        && p.value == property.value
                        && p.parameter_ref == property.parameter_ref
                }) {
                    return Err(format!(
                        "The printer changed {}. Choose a different quality setting.",
                        feature.name.local_name
                    ));
                }
            }
        }
    }
    for parameter in &requested.parameter_inits {
        if !accepted
            .parameter_inits
            .iter()
            .any(|p| p.name == parameter.name && p.value == parameter.value)
        {
            return Err(format!(
                "The driver could not accept {}. Choose a different setting.",
                parameter.name.local_name
            ));
        }
    }
    let xml = PrintCapabilities::fetch_xml_for_ticket(&device, Some(&ticket))
        .map_err(|e| format!("Could not query printable margins: {e:?}"))?;
    let caps = winprint::ticket::document::PrintCapabilitiesDocument::parse_from_bytes(&xml)
        .map_err(|e| format!("Could not read printable margins: {e:?}"))?;
    let area = caps
        .get_property("PageImageableSize", Some(NS_PSK))
        .and_then(|s| s.get_property("ImageableArea", Some(NS_PSK)))
        .and_then(|a| {
            let get = |name| {
                a.get_property(name, Some(NS_PSK))?
                    .value
                    .as_ref()?
                    .integer()
                    .map(|v| v as f32 * 72.0 / 25400.0)
            };
            Some(Area {
                origin: [get("OriginWidth")?, get("OriginHeight")?],
                size: [get("ExtentWidth")?, get("ExtentHeight")?],
            })
        })
        .filter(|a| a.valid_for(settings.output_size()));
    let ticket_colour = accepted
        .features
        .iter()
        .find(|f| f.name.local_name == "PageOutputColor" && f.name.namespace_ref() == Some(NS_PSK))
        .and_then(|f| f.options.first())
        .and_then(option_monochrome);
    let caps = PrintCapabilities { document: caps };
    let only_monochrome = caps.page_output_colors().all(|o| {
        use winprint::ticket::FeatureOptionPack;
        option_monochrome(o.option()) == Some(true)
    }) && caps.page_output_colors().next().is_some();
    Ok(Validated {
        monochrome: ticket_colour
            .or(if only_monochrome { Some(true) } else { None })
            .or(effective_monochrome(&ticket.to_dev_mode(&device).map_err(
                |e| format!("Could not read the driver's effective colour mode: {e:?}"),
            )?)),
        ticket: ticket.into_xml(),
        area,
    })
}
fn effective_monochrome(bytes: &[u8]) -> Option<bool> {
    use windows::Win32::Graphics::Gdi::{DEVMODEW, DM_COLOR, DMCOLOR_COLOR, DMCOLOR_MONOCHROME};
    let size = u16::from_le_bytes(
        bytes
            .get(
                std::mem::offset_of!(DEVMODEW, dmSize)..std::mem::offset_of!(DEVMODEW, dmSize) + 2,
            )?
            .try_into()
            .ok()?,
    ) as usize;
    let offset = std::mem::offset_of!(DEVMODEW, dmColor);
    if size < offset + 2 || bytes.len() < size {
        return None;
    }
    let fields_offset = std::mem::offset_of!(DEVMODEW, dmFields);
    let fields = u32::from_le_bytes(
        bytes
            .get(fields_offset..fields_offset + 4)?
            .try_into()
            .ok()?,
    );
    if fields & DM_COLOR.0 == 0 {
        return None;
    }
    match i16::from_le_bytes(bytes.get(offset..offset + 2)?.try_into().ok()?) {
        v if v == DMCOLOR_MONOCHROME.0 => Some(true),
        v if v == DMCOLOR_COLOR.0 => Some(false),
        _ => None,
    }
}
/// Inspect only the requested fields. Drivers may add empty private parameters
/// which the upstream DOM parser rejects; retain the original full ticket for submission.
fn relevant_ticket(xml: &[u8], requested: &PrintTicketDocument) -> Result<Vec<u8>, String> {
    use quick_xml::{
        NsReader, Writer,
        events::Event,
        name::{QName, ResolveResult},
    };
    let mut reader = NsReader::from_reader(xml);
    let mut writer = Writer::new(Vec::new());
    let mut depth = 0usize;
    let mut keep = true;
    loop {
        let event = reader
            .read_event()
            .map_err(|e| format!("Invalid validated ticket XML: {e}"))?;
        match &event {
            Event::Start(start) | Event::Empty(start) => {
                if depth == 1 {
                    keep = false;
                    let name = start
                        .attributes()
                        .flatten()
                        .find(|a| a.key.as_ref() == b"name")
                        .map(|a| a.value.into_owned());
                    if let Some(name) = name {
                        let (ns, local) = reader.resolver().resolve_element(QName(&name));
                        let colour = local.as_ref() == b"PageOutputColor"
                            && matches!(&ns,ResolveResult::Bound(n) if n.as_ref()==NS_PSK.as_bytes());
                        let wanted = requested
                            .features
                            .iter()
                            .map(|f| &f.name)
                            .chain(requested.parameter_inits.iter().map(|p| &p.name));
                        keep = colour
                            || wanted.into_iter().any(|n| {
                                n.local_name.as_bytes() == local.as_ref()
                                    && match &ns {
                                        ResolveResult::Bound(ns) => n
                                            .namespace
                                            .as_deref()
                                            .is_some_and(|v| v.as_bytes() == ns.as_ref()),
                                        ResolveResult::Unbound => n.namespace.is_none(),
                                        _ => false,
                                    }
                            });
                    }
                }
                if keep || depth == 0 {
                    writer
                        .write_event(event.clone())
                        .map_err(|e| e.to_string())?;
                }
                if matches!(event, Event::Start(_)) {
                    depth += 1;
                }
            }
            Event::End(_) => {
                depth = depth.saturating_sub(1);
                if keep || depth == 0 {
                    writer
                        .write_event(event.clone())
                        .map_err(|e| e.to_string())?;
                }
                if depth == 1 {
                    keep = true;
                }
            }
            Event::Eof => break,
            _ => {
                if keep {
                    writer.write_event(event).map_err(|e| e.to_string())?;
                }
            }
        }
    }
    Ok(writer.into_inner())
}
pub fn submit(printer: &str, path: &std::path::Path, ticket: Vec<u8>) -> Result<(), String> {
    WinPdfPrinter::new(device(printer)?)
        .print(path, PrintTicket::from_xml(ticket))
        .map_err(|e| format!("The printer did not accept the job: {e:?}"))
}
pub fn render_preview(bytes: &[u8], page: usize) -> Result<Vec<u8>, String> {
    let _com = com()?;
    render(bytes, page).map_err(|e| format!("Could not render print preview: {e}"))
}
fn render(bytes: &[u8], page: usize) -> windows::core::Result<Vec<u8>> {
    use windows::{
        Data::Pdf::{PdfDocument, PdfPageRenderOptions},
        Storage::Streams::{DataReader, DataWriter, InMemoryRandomAccessStream},
    };
    let input = InMemoryRandomAccessStream::new()?;
    let writer = DataWriter::CreateDataWriter(&input)?;
    writer.WriteBytes(bytes)?;
    writer.StoreAsync()?.join()?;
    writer.FlushAsync()?.join()?;
    input.Seek(0)?;
    let pdf = PdfDocument::LoadFromStreamAsync(&input)?.join()?;
    let page = pdf.GetPage(page as u32)?;
    let output = InMemoryRandomAccessStream::new()?;
    let options = PdfPageRenderOptions::new()?;
    let size = page.Size()?;
    options.SetDestinationWidth(1200)?;
    options.SetDestinationHeight((1200.0 * size.Height / size.Width).round() as u32)?;
    options.SetIsIgnoringHighContrast(true)?;
    page.RenderWithOptionsToStreamAsync(&output, &options)?
        .join()?;
    let reader = DataReader::CreateDataReader(&output.GetInputStreamAt(0)?)?;
    let size = output.Size()? as u32;
    reader.LoadAsync(size)?.join()?;
    let mut png = vec![0; size as usize];
    reader.ReadBytes(&mut png)?;
    Ok(png)
}

#[cfg(test)]
mod tests {
    use super::*;
    use winprint::ticket::document::{
        OwnedName, PrintFeature, PrintFeatureOption, PropertyValue, ScoredProperty,
    };
    fn option(name: &str, bits: Option<i32>) -> PrintFeatureOption {
        PrintFeatureOption {
            name: Some(OwnedName::qualified(name, NS_PSK, Some("psk"))),
            properties: vec![],
            scored_properties: bits
                .into_iter()
                .map(|bits| ScoredProperty {
                    name: Some(OwnedName::qualified(
                        "DeviceBitsPerPixel",
                        NS_PSK,
                        Some("psk"),
                    )),
                    value: Some(PropertyValue::Integer(bits)),
                    parameter_ref: None,
                    properties: vec![],
                    scored_properties: vec![],
                })
                .collect(),
        }
    }
    #[test]
    fn automatic_class_driver_colour_uses_device_depth() {
        assert_eq!(
            option_monochrome(&option("AutoSelect", Some(1))),
            Some(true)
        );
        assert_eq!(option_monochrome(&option("AutoSelect", Some(24))), None);
        assert_eq!(
            option_monochrome(&option("Monochrome", Some(24))),
            Some(true)
        );
        assert_eq!(option_monochrome(&option("Grayscale", None)), Some(true));
        assert_eq!(option_monochrome(&option("Color", Some(24))), Some(false));
    }
    #[test]
    fn hardware_disabled_finishing_is_filtered_without_dropping_ticket_dependent_choices() {
        let xml = format!(
            r#"<f:PrintCapabilities xmlns:f="{NS_PSF}" xmlns:k="{NS_PSK}" version="1"><f:Feature name="k:JobStapleAllDocuments"><f:Option name="k:None" constrained="k:None"/><f:Option name="k:StapleTopLeft" constrained="k:DeviceSettings"><f:Property name="k:DisplayName"/></f:Option><f:Option name="k:StapleTopRight" constrained="k:AdminSettings"/><f:Option name="k:StapleBottomLeft" constrained="k:PrintTicketSettings"/></f:Feature></f:PrintCapabilities>"#
        );
        let filtered = filter_device_constraints(xml.as_bytes()).unwrap();
        let caps =
            winprint::ticket::document::PrintCapabilitiesDocument::parse_from_bytes(filtered)
                .unwrap();
        let names: Vec<_> = caps.features[0]
            .options
            .iter()
            .map(|o| o.name.as_ref().unwrap().local_name.as_str())
            .collect();
        assert_eq!(names, ["None", "StapleBottomLeft"]);
    }
    #[test]
    fn validated_private_empty_parameters_do_not_discard_colour_defaults() {
        let xml = format!(
            r#"<f:PrintTicket xmlns:f="{NS_PSF}" xmlns:k="{NS_PSK}" xmlns:p="urn:driver" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema" version="1"><f:ParameterInit name="p:Empty"><f:Value xsi:type="xsd:string"/></f:ParameterInit><f:Feature name="k:PageOrientation"><f:Option name="k:Portrait"/></f:Feature><f:Feature name="k:PageOutputColor"><f:Option name="k:Monochrome"/></f:Feature></f:PrintTicket>"#
        );
        let requested = PrintTicketDocument {
            properties: vec![],
            parameter_inits: vec![],
            features: vec![PrintFeature {
                name: OwnedName::qualified("PageOrientation", NS_PSK, Some("psk")),
                properties: vec![],
                options: vec![option("Portrait", None)],
                features: vec![],
            }],
        };
        let filtered = relevant_ticket(xml.as_bytes(), &requested).unwrap();
        let accepted = PrintTicketDocument::parse_from_bytes(filtered).unwrap();
        assert_eq!(accepted.features.len(), 2);
        assert!(accepted.parameter_inits.is_empty());
        assert_eq!(
            option_monochrome(&accepted.features[1].options[0]),
            Some(true)
        );
        assert!(
            xml.contains("p:Empty"),
            "Original driver ticket remains untouched"
        );
    }
    #[test]
    fn devmode_colour_reader_checks_size_and_presence_flags() {
        use windows::Win32::Graphics::Gdi::{DEVMODEW, DM_COLOR};
        assert_eq!(effective_monochrome(&[]), None);
        let mut bytes = vec![0; std::mem::size_of::<DEVMODEW>()];
        let size = std::mem::offset_of!(DEVMODEW, dmSize);
        let len = bytes.len() as u16;
        bytes[size..size + 2].copy_from_slice(&len.to_le_bytes());
        let color = std::mem::offset_of!(DEVMODEW, dmColor);
        bytes[color..color + 2].copy_from_slice(&1i16.to_le_bytes());
        assert_eq!(effective_monochrome(&bytes), None);
        let flags = std::mem::offset_of!(DEVMODEW, dmFields);
        bytes[flags..flags + 4].copy_from_slice(&DM_COLOR.0.to_le_bytes());
        assert_eq!(effective_monochrome(&bytes), Some(true));
        bytes[color..color + 2].copy_from_slice(&2i16.to_le_bytes());
        assert_eq!(effective_monochrome(&bytes), Some(false));
        bytes.truncate(80);
        assert_eq!(effective_monochrome(&bytes), None);
    }
}

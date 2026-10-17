use super::*;

#[cfg(test)]
#[derive(Clone)]
pub(super) struct HeaderBounds {
    pub row: Rect,
    pub brand: Rect,
    pub search: Rect,
    pub actions: Vec<Rect>,
}

fn header(app: &mut AlphaDocs, ui: &mut Ui) {
    let p = app.palette;
    let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 44.0), Sense::hover());
    let brand_rect = Rect::from_min_size(row.min, vec2(176.0, row.height()));
    let mut brand_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(brand_rect)
            .layout(UiLayout::left_to_right(Align::Center)),
    );
    let (r, _) = brand_ui.allocate_exact_size(vec2(28.0, 36.0), Sense::hover());
    icons::brand(&mut brand_ui, r, p.accent);
    brand_ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.label(RichText::new("AlphaDocs").size(23.0));
        ui.label(RichText::new("by Alpha Workroom").size(11.0).color(p.muted));
    });
    // Allocate the actual actions first. Search gets only the space left over,
    // including its frame padding, instead of extending underneath the buttons.
    let mut actions_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(Rect::from_min_max(
                pos2(brand_rect.right() + 16.0, row.top()),
                row.max,
            ))
            .layout(UiLayout::right_to_left(Align::Center)),
    );
    let appearance = actions_ui
        .menu_button("Appearance", |ui| app.appearance(ui))
        .response;
    let about = actions_ui.button("About");
    if about.clicked() {
        app.queue(Action::About);
    }
    let join = actions_ui.button("Join session");
    if join.clicked() {
        app.queue(Action::Share);
    }
    let open = tonal(&mut actions_ui, "Open…", p, true);
    if open.clicked() {
        app.queue(Action::Open);
    }
    let available = Rect::from_min_max(
        pos2(brand_rect.right() + 16.0, row.top()),
        pos2(open.rect.left() - 16.0, row.bottom()),
    );
    let width = available.width().clamp(0.0, 720.0);
    let search_rect = Rect::from_center_size(available.center(), vec2(width, row.height()));
    let mut search_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(search_rect)
            .layout(UiLayout::left_to_right(Align::Center)),
    );
    search_ui.set_clip_rect(ui.clip_rect().intersect(search_rect));
    let search = Frame::new()
        .fill(p.toolbar)
        .corner_radius(24)
        .inner_margin(Margin::symmetric(12, 5))
        .show(&mut search_ui, |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
                icons::draw(ui, rect, Icon::Search, p.muted);
                ui.add(
                    egui::TextEdit::singleline(&mut app.search)
                        .hint_text("Search your documents")
                        .desired_width((width - 50.0).max(0.0))
                        .frame(Frame::NONE),
                );
            });
        });
    #[cfg(test)]
    ui.ctx().data_mut(|d| {
        d.insert_temp(
            Id::new("homepage_header_bounds"),
            HeaderBounds {
                row,
                brand: brand_rect,
                search: search.response.rect,
                actions: vec![open.rect, join.rect, about.rect, appearance.rect],
            },
        )
    });
    #[cfg(not(test))]
    let _ = (search, appearance);
}

pub(super) fn show(app: &mut AlphaDocs, ui: &mut Ui) {
    let p = app.palette;
    egui::Panel::top("home_header")
        .frame(
            Frame::new()
                .fill(p.chrome)
                .inner_margin(Margin::symmetric(24, 10)),
        )
        .show(ui, |ui| {
            header(app, ui);
        });
    egui::CentralPanel::default()
        .frame(Frame::new().fill(p.chrome))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let width = ui.available_width().min(1128.0);
                Frame::new()
                    .fill(p.toolbar)
                    .inner_margin(Margin::same(24))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width() - 48.0);
                        centered(ui, width, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Start a new document").size(16.0));
                                ui.with_layout(UiLayout::right_to_left(Align::Center), |ui| {
                                    ui.label(RichText::new("Templates").size(13.0).color(p.muted));
                                });
                            });
                            ui.add_space(14.0);
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing.x = 18.0;
                                for (kind, title, subtitle) in [
                                    (0, "Blank document", ""),
                                    (1, "Resume", "Serif"),
                                    (2, "Letter", "Simple"),
                                    (3, "Project proposal", "Blueprint"),
                                    (4, "Report", "Modern"),
                                    (5, "Meeting notes", "Essential"),
                                    (6, "Essay", "Classic"),
                                ] {
                                    ui.vertical(|ui| {
                                        let (r, response) = ui.allocate_exact_size(
                                            vec2(146.0, 185.0),
                                            Sense::click(),
                                        );
                                        paper(ui, r, response.hovered(), p);
                                        if kind == 0 {
                                            let o = r.center();
                                            ui.painter().line_segment(
                                                [o - vec2(22.0, 0.0), o + vec2(22.0, 0.0)],
                                                Stroke::new(5.0, p.accent),
                                            );
                                            ui.painter().line_segment(
                                                [o - vec2(0.0, 22.0), o + vec2(0.0, 22.0)],
                                                Stroke::new(5.0, p.accent),
                                            );
                                        } else {
                                            preview(ui, &template(kind), r, p);
                                        }
                                        response.widget_info(|| {
                                            egui::WidgetInfo::labeled(
                                                egui::WidgetType::Button,
                                                true,
                                                format!("Create {title}"),
                                            )
                                        });
                                        if response.clicked() {
                                            app.flush();
                                            app.load(template(kind), None);
                                            app.flush();
                                        }
                                        ui.label(title);
                                        ui.label(RichText::new(subtitle).size(12.0).color(p.muted));
                                    });
                                }
                            });
                        });
                    });
                ui.add_space(22.0);
                centered(ui, width, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Recent documents").size(16.0));
                        ui.with_layout(UiLayout::right_to_left(Align::Center), |ui| {
                            if icons::button(
                                ui,
                                Icon::Sort,
                                "Sort documents by title",
                                app.sort_title,
                                true,
                            )
                            .clicked()
                            {
                                app.sort_title = !app.sort_title;
                            }
                            if icons::button(
                                ui,
                                Icon::Star,
                                "Show starred documents",
                                app.starred_only,
                                true,
                            )
                            .clicked()
                            {
                                app.starred_only = !app.starred_only;
                            }
                            if icons::button(ui, Icon::Folder, "Open a document", false, true)
                                .clicked()
                            {
                                app.queue(Action::Open);
                            }
                        });
                    });
                    ui.add_space(16.0);
                    if app.recents.is_empty() {
                        ui.add_space(28.0);
                        ui.label(RichText::new("Your next idea starts here").size(22.0));
                        ui.label(
                            RichText::new(
                                "Create a document above, or open an AlphaDocs, Word or text file.",
                            )
                            .color(p.muted),
                        );
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new(
                                if app.prefs.autosave {
                                    "Documents autosave on this computer. Your work stays local."
                                } else {
                                    "Autosave is off. Use Save to keep your changes. Your work stays local."
                                },
                            )
                            .size(12.0)
                            .color(p.muted),
                        );
                    }
                    let mut indices: Vec<_> = (0..app.recents.len())
                        .filter(|i| {
                            let d = &app.recents[*i].doc;
                            (!app.starred_only || d.starred)
                                && d.title.to_lowercase().contains(&app.search.to_lowercase())
                        })
                        .collect();
                    if app.sort_title {
                        indices.sort_by_key(|i| app.recents[*i].doc.title.to_lowercase());
                    }
                    let (mut open, mut duplicate, mut trash, mut location) =
                        (None, None, None, None);
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(20.0, 20.0);
                        for i in indices {
                            let recent = &app.recents[i];
                            let doc = &recent.doc;
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 0.0;
                                let (_, r) = ui.allocate_space(vec2(210.0, 265.0));
                                let response = ui.interact(
                                    r,
                                    Id::new(("recent_document", &doc.id)),
                                    Sense::click(),
                                );
                                paper(ui, r, response.hovered(), p);
                                preview(ui, doc, r, p);
                                response.widget_info(|| {
                                    egui::WidgetInfo::labeled(
                                        egui::WidgetType::Button,
                                        true,
                                        format!("Open {}", doc.title),
                                    )
                                });
                                if response.clicked() {
                                    open = Some(i);
                                }
                                response.context_menu(|ui| {
                                    document_menu(
                                        ui,
                                        i,
                                        &recent.path,
                                        &mut open,
                                        &mut duplicate,
                                        &mut trash,
                                        &mut location,
                                    );
                                });
                                response.on_hover_text(format!(
                                    "{}\nModified {}",
                                    doc.title,
                                    persistence::readable_time(recent.modified)
                                ));
                                // Keep the footer attached to the preview as one document card.
                                ui.spacing_mut().item_spacing.y = 0.0;
                                let footer = Frame::new()
                                    .stroke(Stroke::new(0.7, p.border))
                                    .inner_margin(Margin::same(12))
                                    .show(ui, |ui| {
                                        ui.set_width(184.0);
                                        ui.label(
                                            RichText::new(truncate(&doc.title, 25)).size(14.0),
                                        );
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new(format!("{} words", doc.counts().0))
                                                    .size(12.0)
                                                    .color(p.muted),
                                            );
                                            ui.with_layout(
                                                UiLayout::right_to_left(Align::Center),
                                                |ui| {
                                                    let more = icons::button_with_id(
                                                        ui,
                                                        Id::new((
                                                            "recent_document_options",
                                                            &doc.id,
                                                        )),
                                                        Icon::More,
                                                        "Document options",
                                                        false,
                                                        true,
                                                    );
                                                    egui::Popup::menu(&more).show(|ui| {
                                                        document_menu(
                                                            ui,
                                                            i,
                                                            &recent.path,
                                                            &mut open,
                                                            &mut duplicate,
                                                            &mut trash,
                                                            &mut location,
                                                        );
                                                    });
                                                },
                                            );
                                        });
                                    });
                                footer.response.context_menu(|ui| {
                                    document_menu(
                                        ui,
                                        i,
                                        &recent.path,
                                        &mut open,
                                        &mut duplicate,
                                        &mut trash,
                                        &mut location,
                                    );
                                });
                            });
                        }
                    });
                    if let Some(i) = open {
                        app.open_path(app.recents[i].path.clone());
                    }
                    if let Some(i) = duplicate {
                        let mut d = app.recents[i].doc.clone();
                        d.id = Document::new("").id;
                        d.title = format!("Copy of {}", d.title);
                        app.load(d, None);
                        app.flush();
                    }
                    if let Some(i) = location
                        && let Some(parent) = app.recents[i].path.parent()
                        && let Err(error) = open::that(parent)
                    {
                        app.status = format!("Could not open file location: {error}");
                    }
                    if let Some(i) = trash {
                        app.delete_id = Some(app.recents[i].doc.id.clone());
                        app.dialog = Some(Dialog::Delete);
                    }
                });
            });
        });
}
fn document_menu(
    ui: &mut Ui,
    index: usize,
    path: &std::path::Path,
    open: &mut Option<usize>,
    duplicate: &mut Option<usize>,
    trash: &mut Option<usize>,
    location: &mut Option<usize>,
) {
    ui.set_min_width(200.0);
    if menu_width(ui, "Open", "", true, 200.0) {
        *open = Some(index);
    }
    if menu_width(ui, "Make a copy", "", true, 200.0) {
        *duplicate = Some(index);
    }
    submenu_width(ui, "File location", Icon::Folder, 200.0, |ui| {
        ui.set_max_width(360.0);
        ui.with_layout(UiLayout::top_down(Align::Min), |ui| {
            ui.add(egui::Label::new(RichText::new(path.display().to_string()).monospace()).wrap());
        });
        ui.separator();
        if menu(
            ui,
            "Open containing folder",
            "",
            path.parent().is_some_and(|p| p.is_dir()),
        ) {
            *location = Some(index);
        }
        if menu(ui, "Copy file path", "", true) {
            ui.ctx().copy_text(path.display().to_string());
        }
    });
    ui.separator();
    if menu_width(ui, "Move to trash…", "", true, 200.0) {
        *trash = Some(index);
    }
}
fn centered(ui: &mut Ui, width: f32, add: impl FnOnce(&mut Ui)) {
    let margin = ((ui.available_width() - width) * 0.5).max(0.0);
    ui.horizontal(|ui| {
        ui.add_space(margin);
        ui.vertical(|ui| {
            ui.set_max_width(width);
            ui.set_min_width(width);
            add(ui);
        });
    });
}
fn paper(ui: &Ui, rect: Rect, hover: bool, p: Palette) {
    ui.painter().rect_filled(rect, 3.0, p.paper);
    ui.painter().rect_stroke(
        rect,
        3.0,
        Stroke::new(
            if hover { 1.5 } else { 0.7 },
            if hover { p.accent } else { p.border },
        ),
        egui::StrokeKind::Inside,
    );
}
fn preview(ui: &Ui, doc: &Document, rect: Rect, p: Palette) {
    if fonts::prepare(ui.ctx(), doc, &TextStyle::default()) {
        ui.ctx().request_repaint();
        return;
    }
    let mut preview = doc.clone();
    preview.blocks.truncate(24);
    let measured = Layout::measure(ui.ctx(), &preview);
    let scale = rect.width() / measured.size.x;
    let painter = ui.painter().with_clip_rect(rect.shrink(3.0));
    let decorate = |bounds: Rect, style: BoxStyle| {
        let bounds = Rect::from_min_size(
            rect.min + bounds.min.to_vec2() * scale,
            bounds.size() * scale,
        );
        if let Some(fill) = style.fill {
            painter.rect_filled(bounds, 0.0, layout::color(fill));
        }
        for (a, b, border) in layout::box_edges(bounds, style) {
            if border.width > 0.0 {
                painter.line_segment(
                    [a, b],
                    Stroke::new(
                        (border.width * scale).max(0.3),
                        layout::color(border.colour),
                    ),
                );
            }
        }
    };
    for (page, bounds, style) in layout::paragraph_boxes(&measured, doc) {
        if page == 0 {
            decorate(bounds, style);
        }
    }
    for front in [false, true] {
        for object in &measured.objects {
            match object {
                Object::Image {
                    block,
                    page: 0,
                    rect: r,
                    ..
                } => {
                    let Block::Image(image) = &doc.blocks[*block] else {
                        continue;
                    };
                    if (image.placement.flow == ImageFlow::Front) != front {
                        continue;
                    }
                    let id = Id::new(("thumbnail_image", &doc.id, &image.id));
                    let mut cached = ui.ctx().data_mut(|d| {
                        d.get_temp::<(SharedVec<u8>, ImageEffects, egui::TextureHandle)>(id)
                    });
                    if cached.as_ref().is_none_or(|(data, effects, _)| {
                        *data != image.data || *effects != image.effects
                    }) && let Ok(decoded) = image::load_from_memory(&image.data)
                    {
                        let mut small = decoded.thumbnail(210, 265).to_rgba8();
                        crate::image_edit::apply_effects(&mut small, image.effects);
                        let pixels = egui::ColorImage::from_rgba_unmultiplied(
                            [small.width() as usize, small.height() as usize],
                            small.as_raw(),
                        );
                        let handle = ui.ctx().load_texture(
                            format!("thumbnail_{}_{}", doc.id, image.id),
                            pixels,
                            egui::TextureOptions::LINEAR,
                        );
                        let value = (image.data.clone(), image.effects, handle);
                        ui.ctx().data_mut(|d| d.insert_temp(id, value.clone()));
                        cached = Some(value);
                    }
                    if let Some((_, _, texture)) = cached {
                        let [left, top, right, bottom] = image.visible_crop();
                        let [inset_x, inset_y] = image.content_inset_fraction();
                        painter.image(
                            texture.id(),
                            Rect::from_min_size(
                                rect.min + r.min.to_vec2() * scale,
                                r.size() * scale,
                            )
                            .shrink2(vec2(
                                r.width() * scale * inset_x,
                                r.height() * scale * inset_y,
                            )),
                            Rect::from_min_max(pos2(left, top), pos2(right, bottom)),
                            Color32::WHITE,
                        );
                    }
                }
                Object::Border {
                    page: 0,
                    rect: r,
                    address,
                } if !front => {
                    decorate(
                        *r,
                        doc.paragraph(*address)
                            .and_then(|p| p.box_style)
                            .unwrap_or_default(),
                    );
                }
                _ => {}
            }
        }
        if !front {
            for line in measured
                .lines
                .iter()
                .chain(&measured.running)
                .filter(|l| l.page == 0)
            {
                let pos = rect.min + line.rect.min.to_vec2() * scale;
                let display = layout::display_line(
                    ui.ctx(),
                    doc,
                    line,
                    scale,
                    [p.paper.r(), p.paper.g(), p.paper.b()],
                );
                painter.galley(pos, display, p.text);
            }
        }
    }
}

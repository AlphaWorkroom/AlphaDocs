use super::*;
impl AlphaDocs {
    pub(super) fn help_contents(&mut self, ui: &mut Ui) -> bool {
        let p = self.palette;
        ui.label(RichText::new("AlphaDocs help").size(24.0));
        ui.add_space(12.0);
        if ui
            .add(
                egui::Button::new(super::super::platform::shortcut_label(
                    ui.ctx(),
                    "Search features  ·  Alt+/",
                ))
                .corner_radius(18)
                .min_size(vec2(220.0, 36.0)),
            )
            .clicked()
        {
            self.open_feature_search();
        }
        ui.add_space(8.0);
        if ui
            .button("Send feedback or report a bug (opens browser)")
            .clicked()
        {
            ui.ctx().open_url(egui::OpenUrl::new_tab(FEEDBACK_FORM));
        }
        ui.label(
            RichText::new(
                "Opens the feedback form in your browser. No document is sent automatically.",
            )
            .size(12.0)
            .color(p.muted),
        );
        ui.add_space(12.0);
        egui::ScrollArea::vertical().id_salt("alphadocs-help-topics-v2").max_height((ui.ctx().content_rect().height()-380.0).clamp(180.0,520.0)).show(ui,|ui| {
            keyboard_help(ui);
            for (title,body) in [
                ("Modes and compact menus", "The pencil/eye selector at the right of either toolbar switches Editing and Viewing. Viewing allows reading, selection, copying and printing while document changes are disabled. Suggesting is visibly unavailable until tracked changes are implemented. The adjacent chevron or Ctrl+Shift+F hides/shows the document title and menu header. The toolbar, ruler, document tabs and native window controls stay available. The compact preference is remembered; the chevron always restores the menus."),
                ("Writing and formatting", "Type Unicode text directly on the page. Enter starts a paragraph; Shift+Enter inserts a soft line break. Select text to change its font, point size, colour, highlight, bold, italic or underline. Format contains headings, alignment, line/paragraph spacing, indentation and lists. Drag the ruler markers to change margins/indentation. Page setup offers 11 paper presets and custom dimensions, orientation, page colour and margins, plus columns and running text in Advanced. Pages/Pageless changes the canvas; exports and printing retain the stored physical paper settings. Set as default applies geometry to new documents, without copying header/footer text."),
                ("Writing tools and online services", "Tools has manual citations and basic bibliography text, independent text-comparison reports, measured body line numbers and an offline personal dictionary. Review suggested edits, verified signature requests and externally linked objects are not implemented. Dictionary Search sends only the entered word to Free Dictionary API. Translation stays hidden until enabled in Preferences; clicking Translate sends all document text to MyMemory, subject to its free 5,000-character daily quota. It creates a new copy; inline formatting, links and text annotations reset in that copy. Preferences → Substitutions controls optional token replacements. Voice typing explains your platform's dictation controls; AlphaDocs does not capture microphone audio."),
                ("Files, autosave and history", "The Documents homepage has right-click and More menus for opening, copying, viewing/copying the native file location, opening its folder and moving documents to trash. Long open-file titles pan across the tab while active or hovered; the close button stays fixed. Open-file tabs retain their own undo history and selection. Save keeps a local library copy and your selected file. Preferences → General has a clear Autosave on/off card. Automatic saving waits until you have stopped editing for 30 seconds; every edit restarts the wait. Turn it off there to use manual Save; explicit Save, closing and switching documents still preserve work locally. Version history stores snapshots when autosave is enabled. Restore a snapshot from File → Version history. Import/export compatibility warnings are shown explicitly."),
                ("Document tabs and outline", "Sidebar tabs are separate content within one file. The plus adds a tab. Double-click a tab name or use its More menu to rename; that menu also duplicates or deletes tabs. Headings appear beneath the active tab. Switching tabs retains content and selection. All export formats include every document tab in order."),
                ("Tables", "Insert → Table opens the grid picker. Click an empty cell to type; Tab and Shift+Tab move between cells, and Tab in the final cell adds a row. Right-click a cell for row/column operations. Use the table move handle to drag its block through the document. Undo restores the whole change. Merged and nested tables are not supported."),
                ("Images, transparency and placement", "Insert an image, select it, then open Image options in the right sidebar. Drag its body to move it; edges and corners resize with directional cursors. Break text can drop between measured lines inside a paragraph, splitting the paragraph at that text boundary in one undoable move; Wrap, Behind and In front allow floating offsets. Wrap margins are clearances from text. Transparency can trim empty edges without changing original bytes, and add reversible transparent padding. Adjustments include opacity, brightness, contrast and grayscale. Double-click adjustment rails to reset. Freeform, text-line and proximity-line snapping, alignment guides and optional magnetism are saved per image. Genuine inline image atoms and rotation remain unavailable."),
                ("Comments and emoji reactions", "Select text or an image, then use Insert → Comment or Ctrl+Alt+M. Ctrl+Enter posts the draft. Comments remain attached when content is edited or moved. Click a card to reply, resolve or open its More menu. Insert → Emoji has Pinned, Recent and All groups; right-click to pin. Annotation metadata survives native/DOCX reopening, but is not standard Word comments. Shared comments retain the participant's chosen display name. Reaction toggles synchronize; per-person reaction counts are not implemented."),
                ("Nearby device invitations", "On both computers, open Share, enter a display name and choose Make this device visible. On the host, start sharing, choose Can edit or Can view, then click Invite beside the other device. The receiver chooses Join or Decline. Compare the short device IDs; names are not verified identities. Discovery shares your chosen name, device type and temporary ID on the LAN. Invitations are encrypted. Stop nearby visibility when finished. Some Wi-Fi/firewall/VPN setups block discovery; Copy invitation still works."),
                ("Who is working where?", "The Working together strip shows each connected person and their current content tab. Each person has a colour matching their caret and selected text. Caret labels say Caret or Selection so you can tell what the marker means. Short IDs distinguish people using the same name. These markers show position and selection, not a claim that someone is typing right now."),
                ("Live collaboration", "Share or File → Share or join starts an encrypted direct LAN/VPN session. Choose your display name and local interface address, then copy an edit or view invitation and send it privately. Guests paste it into Join a session; the homepage also has Join session. No account or hosted AlphaDocs service is needed. Invitations expire after an hour; closing the host document or End sharing stops access. Up to eight guests can join. Text/formatting, table cells, tabs, images and comments synchronize; remote cursors and selections appear on the current tab. Navigation stays local. Undo affects your own session edits. Connection failures retry while retaining local edits. Native saves keep the resulting document, but sessions and session undo history do not resume after app restart. Starting or leaving a session starts a new undo history. Concurrent structural changes can require manual review; rejected updates retain the local draft. Google sign-in, verified email identities, relay connectivity and per-person reaction counts are not available. Direct encrypted sharing has not had an independent security audit."),
                ("Find, links and navigation", "Ctrl+F or Ctrl+H opens Find and replace for the current content tab. Match case controls sensitivity; Previous/Next select measured matches. Replace and Replace all are undoable. Ctrl+K opens the hyperlink dialog. Shift+arrows extend selection, Ctrl+arrows move by words, Home/End move to line bounds and Ctrl+Home/End to document bounds. Use the title-bar feature search if you cannot find a menu command; Enter opens it and the destination pulses until your next click."),
                ("Spelling, grammar and rich paste", "Automatic offline Australian English checks underline possible spelling errors in red and supported grammar issues in blue after a short typing pause. Right-click the marked word for corrections, Ignore all for this document/session, Add to personal dictionary, or the full checking card. Corrections are undoable. The A/check button beside Print or Ctrl+Alt+X opens that card. The floating card highlights the current issue. Choose a suggestion, then Accept; Ignore skips that occurrence. Its arrows navigate issues and its More menu contains Add to dictionary. The document stays usable while checking. Corrections are undoable. Grammar checks cover repeated words, a/an, common subject–verb agreement and the pronoun I; they do not analyse every grammar construction. Ctrl+V accepts native structured copies, HTML text/tables/images and bitmap-only copies. Ctrl+Shift+V pastes plain text. SVG/drawings are retained as images where a renderable representation exists, not editable shapes. Merged/nested tables and inaccessible images show notices. Public HTTPS image URLs may be fetched without account cookies; document text is never uploaded."),
                ("Exports and printing", "Export and File → Download offer a complete native AlphaDocs (.adocs) copy, Word (DOCX), PDF, OpenDocument (ODT), plain text (TXT), rich text (RTF), zipped offline HTML, EPUB and Markdown. Files are written locally without a conversion service. TXT omits formatting and images. Markdown pictures use a companion folder that must travel with the file. HTML/EPUB reflow in the reader; floating pictures use reading order in these new rich formats. Local annotations require the native file. Open currently imports native, DOCX and TXT; these additional writers do not add importers. PDF uses the editor's measured page layout; interface themes never change printed document colours. Original image media is preserved in native/DOCX while PDF flattens view edits. DOCX supports paragraphs, runs, lists, links, tables, pictures and page setup; complex Word page objects and tracked changes remain unsupported. Print on Windows lists installed printers and driver-supported options. Output paper, page ranges and scale are separate from document page setup; double-click the scale slider to reset to 100%. More settings holds supported trays, duplex, finishing and quality. Preview uses the prepared PDF and detects grayscale devices/defaults. Print sends a native job; Save as PDF writes the scaled pages. macOS/Linux native print backends remain outstanding. Manufacturer colour/dither profiles may differ from the neutral grayscale preview."),
                ("Appearance and accessibility", "Preferences follows the operating-system theme or remembers Light/Dark. Layout and commands are the same in both. Outline, ruler, live word count and autosave can be switched independently. Image alt text and titles are retained. Common reaction chips use coloured artwork; document/picker emoji use Noto Emoji monochrome glyphs."),
            ] {
                egui::CollapsingHeader::new(title).default_open(false).show_unindented(ui,|ui|{let body = if ui.ctx().os() == egui::os::OperatingSystem::Mac && title == "Find, links and navigation" {
                    "Cmd+F or Cmd+Shift+H opens Find and replace. Cmd+K opens links. Option+Left/Right moves by word; Cmd+Left/Right moves to the start/end of the measured line; Cmd+Up/Down moves to document bounds. Add Shift to select. Control-click or a two-finger click opens a context menu. Feature search opens commands directly."
                } else { body }; ui.label(super::super::platform::shortcut_label(ui.ctx(), body));ui.add_space(8.0);});
            }

        });
        ui.add_space(16.0);
        primary(ui, "Done", p, true).clicked()
    }
}

pub(super) const FEEDBACK_FORM: &str = "https://docs.google.com/forms/d/e/1FAIpQLSczvu4GiVbR1PXFWfW_arILYPqBVQKb5kq7uisTbHpCYZ60KQ/viewform?usp=publish-editor";

fn keyboard_help(ui: &mut Ui) {
    egui::CollapsingHeader::new("Keyboard shortcuts").id_salt("task-keyboard-help-v2").default_open(true).show_unindented(ui, |ui| {
        ui.label("Click inside the document first. Hold the modifier keys, press the last key once, then release. On Mac, Cmd means the Command key; Option means the Alt/Option key.");
        for (group, rows) in [
            ("Documents", vec![
                ("New document", "Ctrl+N", "Start a blank document in another file tab."),
                ("Open a file", "Ctrl+O", "Choose an existing document on this computer."),
                ("Save", "Ctrl+S", "Save the current document to its file."),
                ("Save a new copy", "Ctrl+Shift+S", "Choose a different name or location."),
                ("Print", "Ctrl+P", "Open print preview and output settings."),
                ("Close file tab", "Ctrl+W", "Close this document tab while keeping your work locally."),
            ]),
            ("Write and edit", vec![
                ("Undo", "Ctrl+Z", "Reverse your last edit. Press again to go back another step."),
                ("Redo", "Ctrl+Shift+Z", "Restore the edit you just undid."),
                ("Select everything", "Ctrl+A", "Select the current document content tab."),
                ("Copy", "Ctrl+C", "Copy selected text, images or table content."),
                ("Cut", "Ctrl+X", "Copy the selection and remove it from this document."),
                ("Paste", "Ctrl+V", "Insert copied content with its supported formatting."),
                ("Paste plain text", "Ctrl+Shift+V", "Insert just the words, without copied formatting or images."),
                ("Bold", "Ctrl+B", "Make selected text bold, or toggle bold for the next words you type."),
                ("Italic", "Ctrl+I", "Toggle italic for the selection or new typing."),
                ("Underline", "Ctrl+U", "Toggle underline for the selection or new typing."),
                ("Find and replace", "Ctrl+H", "Search this content tab and optionally replace matches."),
                ("Insert a link", "Ctrl+K", "Attach a web address to the selected text."),
                ("Word count", "Ctrl+Shift+C", "Show the current content tab's word and character counts."),
                ("Dictionary", "Ctrl+Shift+Y", "Open a word lookup. Search explicitly sends that word to the dictionary service."),
                ("Check spelling", "Ctrl+Alt+X", "Review each supported spelling or grammar suggestion."),
            ]),
            ("Paragraphs, tables and comments", vec![
                ("New paragraph", "Enter", "Start a new paragraph with its own spacing and formatting."),
                ("New line", "Shift+Enter", "Move down one line within the same paragraph."),
                ("New page", "Ctrl+Enter", "Continue on the next page."),
                ("Next table cell", "Tab", "Move to the next cell. In the last cell, add a new row."),
                ("Previous table cell", "Shift+Tab", "Move back one cell without leaving the table."),
                ("Add a comment", "Ctrl+Alt+M", "Select text or an image first, then open a comment draft."),
                ("Post a comment", "Ctrl+Enter", "While typing a comment, post that draft to its thread."),
                ("Show or hide menus", "Ctrl+Shift+F", "Gain more writing space. The same shortcut restores the menus."),
                ("Search commands", "Alt+/", "Find an app action by its name."),
            ]),
        ] {
            ui.add_space(12.0);
            ui.label(RichText::new(group).strong().size(16.0));
            for (action, shortcut, meaning) in rows {
                Frame::new().inner_margin(Margin::symmetric(8, 7)).show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(action).strong());
                        ui.label(RichText::new(super::super::platform::shortcut_label(ui.ctx(), shortcut)).monospace().color(ui.visuals().hyperlink_color));
                    });
                    ui.label(meaning);
                });
            }
        }
        ui.add_space(12.0);
        ui.label(RichText::new("Move and select text").strong().size(16.0));
        let mac = ui.ctx().os() == egui::os::OperatingSystem::Mac;
        for (keys, meaning) in if mac {
            [("Option+← / →", "Move one word at a time."), ("Cmd+← / →", "Move to the start or end of the visible line."), ("Cmd+↑ / ↓", "Move to the start or end of this content tab."), ("Add Shift", "Select the text you move across instead of just moving the caret."), ("Control-click / two-finger click", "Open the menu for the word, image, table cell or document card you click.")]
        } else {
            [("Ctrl+← / →", "Move one word at a time."), ("Home / End", "Move to the start or end of the visible line."), ("Ctrl+Home / End", "Move to the start or end of this content tab."), ("Add Shift", "Select the text you move across instead of just moving the caret."), ("Right-click", "Open the menu for the word, image, table cell or document card you click.")]
        } { ui.label(RichText::new(keys).strong()); ui.label(meaning); ui.add_space(6.0); }
    });
}

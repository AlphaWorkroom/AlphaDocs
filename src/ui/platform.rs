//! Platform conventions for application chrome and native pointer input.
use eframe::egui::os::OperatingSystem;
use eframe::egui::{Context, Event, PointerButton, RawInput};
use std::borrow::Cow;

pub fn shortcut_label<'a>(ctx: &Context, text: &'a str) -> Cow<'a, str> {
    if ctx.os() != OperatingSystem::Mac {
        return Cow::Borrowed(text);
    }
    Cow::Owned(
        text.replace("Ctrl+H", "Ctrl+Shift+H")
            .replace("Ctrl+Y", "Ctrl+Shift+Z")
            .replace("F11", "Control+Cmd+F")
            .replace("Ctrl+", "Cmd+")
            .replace("Alt+", "Option+"),
    )
}

/// Convert the whole Control-click gesture, including a release after Control
/// is released. Do this before egui hit testing so every native context menu
/// targets the clicked item and no primary selection/drag starts underneath it.
#[derive(Default)]
pub struct ContextPointer {
    control_primary: bool,
}
impl ContextPointer {
    pub fn translate(&mut self, os: OperatingSystem, input: &mut RawInput) {
        if os != OperatingSystem::Mac {
            return;
        }
        for event in &mut input.events {
            match event {
                Event::PointerButton {
                    button,
                    pressed,
                    modifiers,
                    ..
                } if *button == PointerButton::Primary => {
                    if *pressed {
                        self.control_primary = modifiers.ctrl && !modifiers.mac_cmd;
                    }
                    if self.control_primary {
                        *button = PointerButton::Secondary;
                    }
                    if !*pressed {
                        self.control_primary = false;
                    }
                }
                Event::WindowFocused(false) => self.control_primary = false,
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui;
    #[test]
    fn control_click_release_stays_secondary_after_modifier_release() {
        let mut state = ContextPointer::default();
        for (pressed, modifiers) in [
            (true, egui::Modifiers::CTRL),
            (false, egui::Modifiers::NONE),
        ] {
            let mut input = RawInput {
                events: vec![Event::PointerButton {
                    pos: egui::pos2(15.0, 25.0),
                    button: PointerButton::Primary,
                    pressed,
                    modifiers,
                }],
                ..Default::default()
            };
            state.translate(OperatingSystem::Mac, &mut input);
            assert!(matches!(
                input.events[0],
                Event::PointerButton {
                    button: PointerButton::Secondary,
                    ..
                }
            ));
        }
        assert!(!state.control_primary);
    }
    #[test]
    fn platform_shortcut_labels_preserve_windows_and_use_mac_conventions() {
        let ctx = Context::default();
        ctx.set_os(OperatingSystem::Windows);
        assert_eq!(shortcut_label(&ctx, "Redo (Ctrl+Y)"), "Redo (Ctrl+Y)");
        ctx.set_os(OperatingSystem::Mac);
        assert_eq!(shortcut_label(&ctx, "Redo (Ctrl+Y)"), "Redo (Cmd+Shift+Z)");
        assert_eq!(shortcut_label(&ctx, "Ctrl+Alt+X"), "Cmd+Option+X");
        assert_eq!(shortcut_label(&ctx, "Ctrl+H"), "Cmd+Shift+H");
    }
}

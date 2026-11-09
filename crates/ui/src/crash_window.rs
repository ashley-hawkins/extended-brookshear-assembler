//! The window shown after a native panic: the crash details with copy / report / close buttons.

pub const NEW_ISSUE_URL: &str =
    "https://github.com/ashley-hawkins/extended-brookshear-assembler/issues/new";

/// Percent-encodes `text` for a URL query value, escaping exactly what JavaScript's
/// `encodeURIComponent` escapes (everything except letters, digits and `-_.!~*'()`).
fn push_encoded(out: &mut String, text: &str) {
    use std::fmt::Write as _;
    for &byte in text.as_bytes() {
        match byte {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')' => out.push(byte as char),
            _ => write!(out, "%{byte:02X}").unwrap(),
        }
    }
}

/// A prefilled "new issue" link carrying the crash details in a code fence.
/// Keep the title, fence and length limit in sync with the report link in index.html.
pub fn report_url(details: &str) -> String {
    // Truncated to stay under GitHub's URL length limit; the full text is on the clipboard button.
    let details: String = details.chars().take(3500).collect();
    let mut url = format!("{NEW_ISSUE_URL}?title=Crash%20report&body=");
    push_encoded(&mut url, &format!("```\n{details}\n```"));
    url
}

struct CrashWindow(String);

impl eframe::App for CrashWindow {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.heading("The application has crashed");
        ui.label("An unexpected error occurred. Please report this issue on GitHub and include the details below.");
        ui.horizontal(|ui| {
            if ui.button("Copy details").clicked() {
                ui.ctx().copy_text(self.0.clone());
            }
            if ui.button("Report on GitHub").clicked() {
                ui.ctx()
                    .open_url(egui::OpenUrl::new_tab(report_url(&self.0)));
            }
            if ui.button("Close").clicked() {
                ui.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
        ui.separator();
        // Without auto_shrink(false) the area shrinks to fit its content instead of filling the window.
        egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
            ui.add(egui::Label::new(egui::RichText::new(&self.0).monospace()).selectable(true));
        });
    }
}

/// Runs the crash window as a standalone eframe app. Used from a fresh process after a panic.
pub fn run(details: String) -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([800.0, 500.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Brookshear Machine crashed",
        options,
        Box::new(|cc| {
            // egui is built without default fonts, so without this no text renders.
            brookshear_ui::install_fonts(&cc.egui_ctx);
            Ok(Box::new(CrashWindow(details)))
        }),
    )
}

#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

#[cfg(not(target_arch = "wasm32"))]
mod crash_window;

// When compiling natively:
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    if let Ok(details) = std::env::var(CRASH_ENV) {
        return crash_window::run(details);
    }

    let panic_info = install_panic_hook();
    match std::panic::catch_unwind(run_native) {
        Ok(result) => result,
        Err(_) => {
            report_panic(&panic_info);
            std::process::exit(101);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
type PanicInfoSlot = std::sync::Arc<std::sync::Mutex<Option<String>>>;

/// Records the panic message, location and backtrace so it can be shown once unwinding reaches `main`.
#[cfg(not(target_arch = "wasm32"))]
fn install_panic_hook() -> PanicInfoSlot {
    let slot: PanicInfoSlot = Default::default();
    let hook_slot = slot.clone();
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_hook(info);
        let backtrace = std::backtrace::Backtrace::force_capture();
        let thread = std::thread::current();
        let text = format!(
            "{info}\nthread: {}\nversion: {}\nbuild: {}{}\n\nbacktrace:\n{backtrace:#}",
            thread.name().unwrap_or("<unnamed>"),
            env!("CARGO_PKG_VERSION"),
            env!("GIT_HASH"),
            if env!("GIT_DIRTY") == "true" {
                " (dirty)"
            } else {
                ""
            },
        );
        if let Ok(mut guard) = hook_slot.lock() {
            guard.get_or_insert(text);
        }
    }));
    slot
}

#[cfg(not(target_arch = "wasm32"))]
const CRASH_ENV: &str = "BROOKSHEAR_CRASH_REPORT";

/// Shows the crash window in a fresh process: the event loop of this one just unwound,
/// and winit does not reliably allow creating a second one.
#[cfg(not(target_arch = "wasm32"))]
fn report_panic(slot: &PanicInfoSlot) {
    let mut details = slot
        .lock()
        .ok()
        .and_then(|mut g| g.take())
        .unwrap_or_else(|| "no panic details were captured".to_owned());
    eprintln!(
        "\nThe Brookshear Machine UI crashed. Please report this at {} and include the details above.",
        crash_window::NEW_ISSUE_URL
    );
    // Environment strings have a size limit; keep well below it.
    details.truncate(details.floor_char_boundary(60_000));
    let spawned = std::env::current_exe()
        .and_then(|exe| {
            std::process::Command::new(exe)
                .env(CRASH_ENV, &details)
                .spawn()
        })
        .and_then(|mut child| child.wait());
    if spawned.is_err() {
        // Last resort; needs zenity on Linux and can fail silently.
        let _ = rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Error)
            .set_title("Brookshear Machine crashed")
            .set_description(format!(
                "The application must close. Please report this at:\n{}",
                crash_window::NEW_ISSUE_URL
            ))
            .set_buttons(rfd::MessageButtons::Ok)
            .show();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn run_native() -> eframe::Result {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_min_inner_size([1000.0, 700.0])
            .with_icon(
                // NOTE: Adding an icon is optional
                eframe::icon_data::from_png_bytes(
                    &include_bytes!("../assets/favicon-512x512.png")[..],
                )
                .expect("Failed to load icon"),
            ),
        ..Default::default()
    };
    eframe::run_native(
        "Brookshear Machine",
        native_options,
        Box::new(|cc| Ok(Box::new(brookshear_ui::App::new(cc)))),
    )
}

// When compiling to web using trunk:
#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    // Redirect `log` message to `console.log` and friends:
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    // Stash the panic message where the JS crash overlay in index.html can read it.
    // Installed before eframe so eframe's own hook chains to it.
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Some(window) = web_sys::window() {
            let _ = js_sys::Reflect::set(
                &window,
                &"__panic_message".into(),
                &format!(
                    "{info}\nversion: {}\nbuild: {}{}",
                    env!("CARGO_PKG_VERSION"),
                    env!("GIT_HASH"),
                    if env!("GIT_DIRTY") == "true" {
                        " (dirty)"
                    } else {
                        ""
                    },
                )
                .into(),
            );
        }
        previous_hook(info);
    }));

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("No window")
            .document()
            .expect("No document");

        let canvas = document
            .get_element_by_id("the_canvas_id")
            .expect("Failed to find the_canvas_id")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("the_canvas_id was not a HtmlCanvasElement");

        let start_result = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(brookshear_ui::App::new(cc)))),
            )
            .await;

        // Remove the loading text and spinner:
        if let Some(loading_text) = document.get_element_by_id("loading_text") {
            match start_result {
                Ok(_) => {
                    loading_text.remove();
                }
                Err(e) => {
                    loading_text.set_inner_html(
                        "<p> The app has crashed. See the developer console for details. </p>",
                    );
                    panic!("Failed to start eframe: {e:?}");
                }
            }
        }
    });
}

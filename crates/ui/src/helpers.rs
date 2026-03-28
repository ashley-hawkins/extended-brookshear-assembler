#[cfg(not(target_arch = "wasm32"))]
pub fn open_file() -> futures::channel::oneshot::Receiver<Vec<u8>> {
    let (sender, receiver) = futures::channel::oneshot::channel();
    let path = rfd::FileDialog::new().pick_file();

    if let Some(path) = path {
        sender
            .send(std::fs::read(path).expect("Failed to read file"))
            .expect("Failed to send path through channel");
    } else {
        drop(sender);
    }
    receiver
}

#[cfg(target_arch = "wasm32")]
pub fn open_file() -> futures::channel::oneshot::Receiver<Vec<u8>> {
    let (sender, receiver) = futures::channel::oneshot::channel();
    wasm_bindgen_futures::spawn_local(async move {
        let file = rfd::AsyncFileDialog::new().pick_file().await;
        if let Some(file) = file {
            let data = file.read().await;
            sender
                .send(data)
                .expect("Failed to send file data through channel");
        } else {
            drop(sender);
        }
    });
    receiver
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_file(data: Vec<u8>, name_hint: &str) {
    let path = rfd::FileDialog::new().set_file_name(name_hint).save_file();
    if let Some(path) = path {
        std::fs::write(path, data).expect("Failed to write file");
    }
}

#[cfg(target_arch = "wasm32")]
pub fn save_file(data: Vec<u8>, name_hint: &str) {
    use js_sys::wasm_bindgen::JsCast;
    let blob = web_sys::Blob::new_with_u8_array_sequence(&js_sys::Array::of1(
        &js_sys::Uint8Array::from(&data[..]),
    ))
    .expect("Failed to create blob");
    let url =
        web_sys::Url::create_object_url_with_blob(&blob).expect("Failed to create object URL");
    let a = web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .create_element("a")
        .unwrap()
        .dyn_into::<web_sys::HtmlAnchorElement>()
        .unwrap();
    a.set_attribute("href", &url).unwrap();
    a.set_attribute("download", name_hint).unwrap();
    a.click();
}

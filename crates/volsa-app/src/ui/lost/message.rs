//! Words in a message box of the system.

/// Show `text` in a message box of the system, on a thread of its own: the box waits for the
/// person to close it, and nothing else may wait with it.
pub fn in_a_window(text: &str) {
    let text = text.to_owned();
    let started = std::thread::Builder::new()
        .name("volsa graphics notice".to_owned())
        .spawn(move || {
            rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Error)
                .set_title("Volsa")
                .set_description(text)
                .set_buttons(rfd::MessageButtons::Ok)
                .show();
        });
    if started.is_err() {
        tracing::warn!("the message box for the graphics card could not be started");
    }
}

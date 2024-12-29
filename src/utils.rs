use gtk::glib;

pub fn send<T: 'static>(sender: &async_channel::Sender<T>, message: T) {
    let fut = glib::clone!(
        #[strong]
        sender,
        async move {
            if let Err(err) = sender.send(message).await {
                error!(
                    "Failed to send \"{}\" action due to {err}",
                    stringify!(message),
                );
            }
        }
    );
    glib::spawn_future_local(fut);
}

pub fn format_duration(d: u64) -> String {
    let dt = glib::DateTime::from_unix_local(d.try_into().unwrap_or_default()).unwrap();
    dt.format("%M:%S").unwrap_or_default().to_string()
}

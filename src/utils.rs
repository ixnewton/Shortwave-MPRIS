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

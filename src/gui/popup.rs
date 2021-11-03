use gtk::{
    prelude::{DialogExt, GtkWindowExt, WidgetExt},
    ButtonsType, MessageDialog, MessageType,
};
use log::debug;

pub fn blocking_prompt<S0, S1>(level: MessageType, text_1: S0, text_2: S1)
where
    S0: AsRef<str>,
    S1: AsRef<str>,
{
    debug!("Showing popup; type: {}, title: {}", level, text_1.as_ref());
    let dialog = MessageDialog::builder()
        .buttons(ButtonsType::Ok)
        .deletable(true)
        .message_type(level)
        .secondary_text(text_2.as_ref())
        .secondary_use_markup(true)
        .text(text_1.as_ref())
        .title("shadowsocks-gtk-rs")
        .build();
    dialog.connect_response(|dialog, _| {
        dialog.emit_close();
    }); // handle close
    dialog.show_all(); // render
    dialog.present(); // bring to foreground
}

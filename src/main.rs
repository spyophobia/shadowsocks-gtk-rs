mod tray;
fn main() {
    gtk::init().unwrap();

    let _tray_item = tray::start();

    gtk::main();
}

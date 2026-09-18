fn main() {
    let ctx = egui::Context::default();
    egui_extras::install_image_loaders(&ctx);
    let uri = "file:///Users/yocan/.well/generated/image_0881c0360372.png";
    let bytes_poll = ctx.try_load_bytes(uri);
    println!("bytes_poll: {:?}", bytes_poll);
}

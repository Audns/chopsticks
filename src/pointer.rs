use wayland_client::Connection;
use wayland_client::protocol::wl_pointer;
use wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_v1;

pub const BTN_LEFT: u32 = 0x110;

pub fn emit_click(
    virtual_pointer: &zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1,
    conn: &Connection,
    x: u32,
    y: u32,
    window_width: u32,
    window_height: u32,
) {
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u32;

    println!("Emitting click at ({x}, {y}) [window: {window_width}x{window_height}]");

    virtual_pointer.motion_absolute(time, x, y, window_width, window_height);
    virtual_pointer.frame();
    conn.flush().expect("Failed to flush motion");

    std::thread::sleep(std::time::Duration::from_millis(50));

    virtual_pointer.button(time, BTN_LEFT, wl_pointer::ButtonState::Pressed);
    virtual_pointer.frame();
    conn.flush().expect("Failed to flush press");

    std::thread::sleep(std::time::Duration::from_millis(50));

    virtual_pointer.button(time, BTN_LEFT, wl_pointer::ButtonState::Released);
    virtual_pointer.frame();
    conn.flush().expect("Failed to flush release");

    std::thread::sleep(std::time::Duration::from_millis(100));
    println!("Click complete");
}

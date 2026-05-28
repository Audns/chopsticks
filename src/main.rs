use std::os::unix::io::AsFd;
use wayland_client::{
    Connection, Dispatch, QueueHandle,
    protocol::{wl_buffer, wl_compositor, wl_keyboard, wl_output, wl_registry, wl_seat, wl_shm, wl_shm_pool, wl_surface},
};
use wayland_protocols_wlr::{
    layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1},
    virtual_pointer::v1::client::{zwlr_virtual_pointer_manager_v1, zwlr_virtual_pointer_v1},
};
use calloop::EventLoop;
use calloop_wayland_source::WaylandSource;

mod config;
mod input;
mod pointer;
mod render;

use config::{ConfigFile, load_config};
use input::{InputState, keycode_to_char, precision_key_to_subcell, compute_precision_coordinate, compute_cell_bounds, ESCAPE_KEYCODE};
use pointer::emit_click;
use render::{PixelBuffer, render_frame};

struct AppState {
    running: bool,
    compositor: Option<wl_compositor::WlCompositor>,
    shm: Option<wl_shm::WlShm>,
    layer_shell: Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    virtual_pointer_manager: Option<zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1>,
    virtual_pointer: Option<zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1>,
    seat: Option<wl_seat::WlSeat>,
    output: Option<wl_output::WlOutput>,
    surface: Option<wl_surface::WlSurface>,
    layer_surface: Option<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    buffer: Option<wl_buffer::WlBuffer>,
    shm_pool: Option<wl_shm_pool::WlShmPool>,
    configured: bool,
    width: u32,
    height: u32,
    window_width: u32,
    window_height: u32,
    pixel_buffer: Option<PixelBuffer>,
    input_state: InputState,
    pending_click: Option<(u32, u32)>,
    waiting_key_release: Option<u32>,
    config: ConfigFile,
}

impl Dispatch<wl_registry::WlRegistry, ()> for AppState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global { name, interface, version } = event {
            match &interface[..] {
                "wl_compositor" => {
                    state.compositor = Some(registry.bind(name, version, qh, ()));
                }
                "wl_shm" => {
                    state.shm = Some(registry.bind(name, version, qh, ()));
                }
                "zwlr_layer_shell_v1" => {
                    state.layer_shell = Some(registry.bind(name, version, qh, ()));
                }
                "wl_seat" => {
                    state.seat = Some(registry.bind(name, version, qh, ()));
                }
                "zwlr_virtual_pointer_manager_v1" => {
                    state.virtual_pointer_manager = Some(registry.bind(name, version, qh, ()));
                }
                "wl_output"
                    if state.output.is_none() => {
                        state.output = Some(registry.bind(name, version, qh, ()));
                    }
                _ => {}
            }
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for AppState {
    fn event(
        state: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities { capabilities } = event
            && let Ok(caps) = capabilities.into_result()
                && caps.contains(wl_seat::Capability::Keyboard) && state.keyboard.is_none() {
                    state.keyboard = Some(seat.get_keyboard(qh, ()));
                }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for AppState {
    fn event(
        state: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let wl_keyboard::Event::Key { key, state: key_state, .. } = event {
            // Handle key release first if we're waiting for one
            if let Ok(wl_keyboard::KeyState::Released) = key_state.into_result() {
                if state.waiting_key_release == Some(key) {
                    println!("Key released, executing click...");
                    state.waiting_key_release = None;
                    // Key released, now safe to proceed with click
                }
                return;
            }

            if let Ok(wl_keyboard::KeyState::Pressed) = key_state.into_result() {
                if key == ESCAPE_KEYCODE {
                    println!("Escape pressed, exiting...");
                    state.running = false;
                    return;
                }

                if let Some(ch) = keycode_to_char(key) {
                    match state.input_state {
                        InputState::Idle => {
                            if ch == ' ' {
                                return;
                            }
                            println!("First key: {}", ch);
                            state.input_state = InputState::WaitingSecond { first: ch };
                            
                            if let Some(ref mut pb) = state.pixel_buffer {
                                let row = (ch as u32) - ('a' as u32);
                                render_frame(pb, state.width, state.height, Some(row), None, &state.config);
                                println!("Re-rendered with row {} highlighted", row);
                            }
                            if let Some(ref surface) = state.surface {
                                if let Some(ref buffer) = state.buffer {
                                    surface.attach(Some(buffer), 0, 0);
                                }
                                surface.damage(0, 0, state.width as i32, state.height as i32);
                                surface.commit();
                                conn.flush().expect("Failed to flush re-render");
                            }
                        }
                        InputState::WaitingSecond { first } => {
                            if ch == ' ' {
                                return;
                            }
                            println!("Second key: {}", ch);
                            let row = (first as u32) - ('a' as u32);
                            let col = (ch as u32) - ('a' as u32);
                            state.input_state = InputState::WaitingThird { first, second: ch };
                            
                            if let Some(ref mut pb) = state.pixel_buffer {
                                render_frame(pb, state.width, state.height, Some(row), Some((row, col)), &state.config);
                                println!("Re-rendered with precision grid for cell ({}, {})", first, ch);
                            }
                            if let Some(ref surface) = state.surface {
                                if let Some(ref buffer) = state.buffer {
                                    surface.attach(Some(buffer), 0, 0);
                                }
                                surface.damage(0, 0, state.width as i32, state.height as i32);
                                surface.commit();
                                conn.flush().expect("Failed to flush re-render");
                            }
                        }
                        InputState::WaitingThird { first, second } => {
                            let col = (second as u32) - ('a' as u32);
                            let row = (first as u32) - ('a' as u32);
                            let (x1, y1, x2, y2) = compute_cell_bounds(col, row, state.width, state.height);
                            
                            if ch == ' ' {
                                let x = x1 + (x2 - x1) / 2;
                                let y = y1 + (y2 - y1) / 2;
                                println!("Space pressed: clicking center of cell ({}, {})", first, second);
                                println!("Target coordinate: ({}, {})", x, y);
                                state.input_state = InputState::Done;
                                state.waiting_key_release = Some(key);
                                state.pending_click = Some((x, y));
                            } else if let Some((sub_col, sub_row)) = precision_key_to_subcell(ch) {
                                let (x, y) = compute_precision_coordinate(
                                    first, second, sub_col, sub_row, state.width, state.height
                                );
                                println!("Precision key: {} -> sub-cell ({}, {})", ch, sub_col, sub_row);
                                println!("Target coordinate: ({}, {})", x, y);
                                state.input_state = InputState::Done;
                                state.pending_click = Some((x, y));
                            } else {
                                println!("Invalid precision key: {}. Use y,u,i,o,h,j,k,l or space", ch);
                            }
                        }
                        InputState::Done => {
                            state.input_state = InputState::Idle;
                        }
                    }
                } else {
                    println!("Invalid key, resetting...");
                    state.input_state = InputState::Idle;
                }
            }
        }
    }
}

impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, ()> for AppState {
    fn event(
        state: &mut Self,
        layer_surface: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_layer_surface_v1::Event::Configure { serial, width, height } => {
                println!("Configured: {}x{}", width, height);
                state.width = if width == 0 { 2560 } else { width };
                state.height = if height == 0 { 1440 } else { height };
                state.configured = true;
                layer_surface.ack_configure(serial);
            }
            zwlr_layer_surface_v1::Event::Closed => {
                println!("Layer surface closed");
                state.running = false;
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_surface::WlSurface, ()> for AppState {
    fn event(_: &mut Self, _: &wl_surface::WlSurface, _: wl_surface::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<wl_compositor::WlCompositor, ()> for AppState {
    fn event(_: &mut Self, _: &wl_compositor::WlCompositor, _: wl_compositor::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<wl_shm::WlShm, ()> for AppState {
    fn event(_: &mut Self, _: &wl_shm::WlShm, _: wl_shm::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<wl_shm_pool::WlShmPool, ()> for AppState {
    fn event(_: &mut Self, _: &wl_shm_pool::WlShmPool, _: wl_shm_pool::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<wl_buffer::WlBuffer, ()> for AppState {
    fn event(_: &mut Self, _: &wl_buffer::WlBuffer, _: wl_buffer::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<zwlr_layer_shell_v1::ZwlrLayerShellV1, ()> for AppState {
    fn event(_: &mut Self, _: &zwlr_layer_shell_v1::ZwlrLayerShellV1, _: zwlr_layer_shell_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<wl_output::WlOutput, ()> for AppState {
    fn event(
        state: &mut Self,
        _: &wl_output::WlOutput,
        event: wl_output::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_output::Event::Geometry { x, y, physical_width, physical_height, .. } = event {
            println!("Output geometry: {}x{}+{}+{} ({}x{} mm)", 
                physical_width, physical_height, x, y, physical_width, physical_height);
        }
        if let wl_output::Event::Mode { flags, width, height, refresh } = event
            && let Ok(f) = flags.into_result()
                && f.contains(wl_output::Mode::Current) {
                    println!("Output mode: {}x{} @ {}Hz", width, height, refresh);
                    if state.window_width == 0 {
                        state.window_width = width as u32;
                    }
                    if state.window_height == 0 {
                        state.window_height = height as u32;
                    }
                }
    }
}

impl Dispatch<zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1, ()> for AppState {
    fn event(_: &mut Self, _: &zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1, _: zwlr_virtual_pointer_manager_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1, ()> for AppState {
    fn event(_: &mut Self, _: &zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1, _: zwlr_virtual_pointer_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

fn create_wayland_buffer(
    shm: &wl_shm::WlShm,
    pixel_buffer: &PixelBuffer,
    file: &std::fs::File,
    qh: &QueueHandle<AppState>,
) -> (wl_buffer::WlBuffer, wl_shm_pool::WlShmPool) {
    let size = pixel_buffer.data.len() as i32;
    let pool = shm.create_pool(file.as_fd(), size, qh, ());
    let buffer = pool.create_buffer(
        0,
        pixel_buffer.width as i32,
        pixel_buffer.height as i32,
        pixel_buffer.stride as i32,
        wl_shm::Format::Argb8888,
        qh,
        (),
    );
    (buffer, pool)
}

fn main() {
    let lock_file_path = std::path::PathBuf::from("/tmp/chopsticks.lock");
    let lock_file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&lock_file_path)
        .expect("Failed to create lock file");
    
    if lock_file.try_lock().is_err() {
        eprintln!("Another instance of chopsticks is already running.");
        std::process::exit(1);
    }
    
    println!("Starting chopsticks...");
    
    let config = load_config();
    println!("Config: {:?}", config);

    let out_w = config.window_width.unwrap_or(0);
    let out_h = config.window_height.unwrap_or(0);

    let conn = Connection::connect_to_env().expect("Failed to connect to Wayland display");
    let mut event_queue = conn.new_event_queue();
    let qh = event_queue.handle();

    let mut state = AppState {
        running: true,
        compositor: None,
        shm: None,
        layer_shell: None,
        virtual_pointer_manager: None,
        virtual_pointer: None,
        seat: None,
        output: None,
        surface: None,
        layer_surface: None,
        keyboard: None,
        buffer: None,
        shm_pool: None,
        configured: false,
        width: 0,
        height: 0,
        window_width: out_w,
        window_height: out_h,
        pixel_buffer: None,
        input_state: InputState::Idle,
        pending_click: None,
        waiting_key_release: None,
        config,
    };

    let display = conn.display();
    display.get_registry(&qh, ());
    event_queue.roundtrip(&mut state).expect("Roundtrip failed");

    println!("Globals bound");

    if let Some(ref vpm) = state.virtual_pointer_manager
        && let Some(ref seat) = state.seat {
            let vp = if let Some(ref output) = state.output {
                vpm.create_virtual_pointer_with_output(Some(seat), Some(output), &qh, ())
            } else {
                vpm.create_virtual_pointer(Some(seat), &qh, ())
            };
            state.virtual_pointer = Some(vp);
            println!("Virtual pointer created");
        }

    event_queue.roundtrip(&mut state).expect("Roundtrip after creating virtual pointer failed");

    let compositor = state.compositor.as_ref().expect("No compositor available");
    let surface = compositor.create_surface(&qh, ());
    state.surface = Some(surface);

    let layer_shell = state.layer_shell.as_ref().expect("No layer shell available");
    let layer_surface = layer_shell.get_layer_surface(
        state.surface.as_ref().unwrap(),
        None,
        zwlr_layer_shell_v1::Layer::Overlay,
        "chopsticks".to_string(),
        &qh,
        (),
    );
    layer_surface.set_size(0, 0);
    layer_surface.set_anchor(
        zwlr_layer_surface_v1::Anchor::Top
            | zwlr_layer_surface_v1::Anchor::Bottom
            | zwlr_layer_surface_v1::Anchor::Left
            | zwlr_layer_surface_v1::Anchor::Right,
    );
    layer_surface.set_exclusive_zone(-1);
    layer_surface.set_keyboard_interactivity(
        zwlr_layer_surface_v1::KeyboardInteractivity::Exclusive,
    );
    state.layer_surface = Some(layer_surface);

    state.surface.as_ref().unwrap().commit();
    println!("Surface committed, waiting for configure...");

    while !state.configured {
        event_queue.blocking_dispatch(&mut state).expect("Dispatch failed");
    }

    if let Some(w) = state.config.logical_width() {
        state.width = w;
    }
    if let Some(h) = state.config.logical_height() {
        state.height = h;
    }

    let (mut pixel_buffer, file) = PixelBuffer::new(state.width, state.height);
    render_frame(&mut pixel_buffer, state.width, state.height, None, None, &state.config);
    state.pixel_buffer = Some(pixel_buffer);

    let shm = state.shm.as_ref().expect("No shm available");
    let pixel_buffer = state.pixel_buffer.as_ref().unwrap();
    let (buffer, pool) = create_wayland_buffer(shm, pixel_buffer, &file, &qh);
    state.buffer = Some(buffer);
    state.shm_pool = Some(pool);

    let surface = state.surface.as_ref().unwrap();
    surface.attach(Some(state.buffer.as_ref().unwrap()), 0, 0);
    surface.damage(0, 0, state.width as i32, state.height as i32);
    surface.commit();
    println!("Buffer attached and committed");

    conn.flush().expect("Failed to flush connection");
    
    let mut event_loop = EventLoop::try_new().expect("Failed to create event loop");
    let wayland_source = WaylandSource::new(conn.clone(), event_queue);
    wayland_source.insert(event_loop.handle()).expect("Failed to insert wayland source");

    println!("Event loop running. Press Escape to exit.");

    while state.running {
        event_loop.dispatch(None, &mut state).expect("Dispatch failed");
        
        // Wait for key release before processing click (prevents key leakage to other apps)
        if state.waiting_key_release.is_some() {
            continue;
        }
        
        if let Some((x, y)) = state.pending_click.take() {
            if let Some(ref layer_surface) = state.layer_surface {
                layer_surface.destroy();
                conn.flush().expect("Failed to flush destroy");
                std::thread::sleep(std::time::Duration::from_millis(50));
                println!("Layer surface destroyed");
            }
            
            if let Some(ref vp) = state.virtual_pointer {
                let out_w = if state.window_width > 0 { state.window_width } else { state.width };
                let out_h = if state.window_height > 0 { state.window_height } else { state.height };
                let scale = state.config.scale_ratio;
                let sx = (x as f64 * scale) as u32;
                let sy = (y as f64 * scale) as u32;
                emit_click(
                    vp, &conn, sx, sy, out_w, out_h
                );
                
                for _ in 0..5 {
                    event_loop.dispatch(Some(std::time::Duration::from_millis(10)), &mut state).ok();
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            } else {
                println!("Warning: No virtual pointer available");
            }
            state.running = false;
        }
    }

    println!("Exiting cleanly");
}

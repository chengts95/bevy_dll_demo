use bevy_app::{App, AppExit, Update};
use bevy_ecs::prelude::*;
use shared_api::{GameLogicSet, RenderSet, MainThreadMarker, Transform, Visual};

pub struct WindowResource {
    pub window: minifb::Window,
}

pub struct CanvasResource {
    pub pixmap: tiny_skia::Pixmap,
}

#[no_mangle]
pub unsafe extern "C" fn setup_mod(app_ptr: *mut std::ffi::c_void) {
    let app = &mut *(app_ptr as *mut App);

    app.insert_non_send(MainThreadMarker);
    
    let mut window = minifb::Window::new(
        "Bevy ECS Canvas Demo (Strict Schedule)",
        800,
        600,
        minifb::WindowOptions::default(),
    ).unwrap();
    window.limit_update_rate(Some(std::time::Duration::from_micros(16600)));

    app.insert_non_send(WindowResource { window });
    app.insert_non_send(CanvasResource {
        pixmap: tiny_skia::Pixmap::new(800, 600).unwrap(),
    });
    app.init_resource::<shared_api::InputState>();

    // Strictly follow the schedule division: Logic first, Render last.
    app.configure_sets(
        Update,
        (
            GameLogicSet::Update,
            GameLogicSet::PostUpdate,
            RenderSet::Clear,
            RenderSet::DrawOpaque,
            RenderSet::DrawUI,
        ).chain()
    );

    app.add_systems(Update, clear_canvas_system.in_set(RenderSet::Clear));
    app.add_systems(Update, draw_opaque_system.in_set(RenderSet::DrawOpaque));
    app.add_systems(Update, present_window_system.in_set(RenderSet::DrawUI));

    // Runner 被降级为最纯粹的空壳，绝不包含任何业务和渲染代码！
    app.set_runner(|mut app| {
        loop {
            app.update();
            
            // 检查窗口是否关闭
            let window_res = app.world().non_send::<WindowResource>();
            if !window_res.window.is_open() || window_res.window.is_key_down(minifb::Key::Escape) {
                break;
            }
        }
        AppExit::Success
    });
}

fn clear_canvas_system(mut canvas: NonSendMut<CanvasResource>) {
    canvas.pixmap.fill(tiny_skia::Color::from_rgba8(40, 45, 50, 255));
}

fn draw_opaque_system(
    mut canvas: NonSendMut<CanvasResource>,
    query: Query<(&Transform, &Visual)>,
) {
    for (transform, visual) in query.iter() {
        let mut paint = tiny_skia::Paint::default();
        paint.set_color_rgba8(
            (visual.color[0] * 255.0) as u8,
            (visual.color[1] * 255.0) as u8,
            (visual.color[2] * 255.0) as u8,
            (visual.color[3] * 255.0) as u8,
        );

        let ts = tiny_skia::Transform::from_translate(transform.position[0], transform.position[1])
            .pre_rotate(transform.rotation.to_degrees());

        let rect = tiny_skia::Rect::from_xywh(
            -transform.size[0] / 2.0,
            -transform.size[1] / 2.0,
            transform.size[0],
            transform.size[1]
        ).unwrap();

        canvas.pixmap.fill_rect(rect, &paint, ts, None);
    }
}

fn present_window_system(mut window_res: NonSendMut<WindowResource>, canvas: NonSend<CanvasResource>, mut input: ResMut<shared_api::InputState>) {
    let window = &mut window_res.window;
    
    input.left_down = window.is_key_down(minifb::Key::Left);
    input.right_down = window.is_key_down(minifb::Key::Right);
    let keys = window.get_keys_pressed(minifb::KeyRepeat::No);
    input.space_pressed = keys.contains(&minifb::Key::Space);

    let buffer: Vec<u32> = canvas.pixmap.pixels().iter().map(|p| {
        let r = p.red() as u32;
        let g = p.green() as u32;
        let b = p.blue() as u32;
        (r << 16) | (g << 8) | b
    }).collect();

    window.update_with_buffer(&buffer, 800, 600).unwrap();
}

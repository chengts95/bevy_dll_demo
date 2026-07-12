use std::sync::Arc;

use bevy_app::{App, AppExit, Update};
use bevy_ecs::prelude::*;
use shared_api::{GameLogicSet, InputState, MainThreadMarker, RenderSet, Transform, Visual};
use vello::kurbo::{Affine, Rect};
use vello::peniko::{Color, Fill};
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu::{self, CurrentSurfaceTexture};
use vello::{AaConfig, RenderParams, Renderer, RendererOptions, Scene};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowAttributes, WindowId};

const INITIAL_WIDTH: u32 = 800;
const INITIAL_HEIGHT: u32 = 600;

struct WindowState {
    window: Arc<Window>,
    left_down: bool,
    right_down: bool,
    space_pressed: bool,
}

struct VelloState {
    context: RenderContext,
    surface: RenderSurface<'static>,
    renderer: Renderer,
    scene: Scene,
    valid_surface: bool,
}

struct VelloApplication {
    app: Option<App>,
}

impl VelloApplication {
    fn app_mut(&mut self) -> &mut App {
        self.app.as_mut().expect("Vello application is unavailable")
    }
}

impl ApplicationHandler for VelloApplication {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self
            .app_mut()
            .world()
            .get_non_send::<WindowState>()
            .is_some()
        {
            return;
        }

        let attributes = WindowAttributes::default()
            .with_title("Bevy DLL Demo - Vello Renderer")
            .with_inner_size(winit::dpi::LogicalSize::new(INITIAL_WIDTH, INITIAL_HEIGHT));
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .expect("failed to create Vello window"),
        );
        let size = window.inner_size();
        let mut context = RenderContext::new();
        let surface = pollster::block_on(context.create_surface(
            window.clone(),
            size.width.max(1),
            size.height.max(1),
            wgpu::PresentMode::AutoVsync,
        ))
        .expect("failed to create Vello render surface");
        let renderer = Renderer::new(
            &context.devices[surface.dev_id].device,
            RendererOptions {
                antialiasing_support: [AaConfig::Area].into_iter().collect(),
                ..Default::default()
            },
        )
        .expect("failed to create Vello renderer");

        let app = self.app_mut();
        app.insert_non_send(WindowState {
            window,
            left_down: false,
            right_down: false,
            space_pressed: false,
        });
        app.insert_non_send(VelloState {
            context,
            surface,
            renderer,
            scene: Scene::new(),
            valid_surface: size.width > 0 && size.height > 0,
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let app = self.app_mut();
        let Some(window) = app.world().get_non_send::<WindowState>() else {
            return;
        };
        if window.window.id() != window_id {
            return;
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. } => {
                let pressed = event.state == ElementState::Pressed;
                match event.logical_key.as_ref() {
                    Key::Named(NamedKey::ArrowLeft) => {
                        app.world_mut().non_send_mut::<WindowState>().left_down = pressed;
                    }
                    Key::Named(NamedKey::ArrowRight) => {
                        app.world_mut().non_send_mut::<WindowState>().right_down = pressed;
                    }
                    Key::Named(NamedKey::Space) if pressed && !event.repeat => {
                        app.world_mut().non_send_mut::<WindowState>().space_pressed = true;
                    }
                    Key::Named(NamedKey::Escape) if pressed => event_loop.exit(),
                    _ => {}
                }
            }
            WindowEvent::Resized(size) => {
                let mut state = app.world_mut().non_send_mut::<VelloState>();
                if size.width == 0 || size.height == 0 {
                    state.valid_surface = false;
                } else {
                    let VelloState {
                        context, surface, ..
                    } = &mut *state;
                    context.resize_surface(surface, size.width, size.height);
                    state.valid_surface = true;
                }
            }
            WindowEvent::RedrawRequested => app.update(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = self.app_mut().world().get_non_send::<WindowState>() {
            window.window.request_redraw();
        }
    }
}

#[no_mangle]
/// Installs the Vello renderer into the host-owned Bevy application.
///
/// # Safety
///
/// `app_ptr` must be a valid, uniquely borrowed pointer to a live [`App`]
/// created with the same shared Bevy ABI as this dynamic library.
pub unsafe extern "C" fn setup_mod(app_ptr: *mut std::ffi::c_void) {
    let app = &mut *(app_ptr as *mut App);
    app.insert_non_send(MainThreadMarker);
    app.init_resource::<InputState>();
    app.configure_sets(
        Update,
        (
            GameLogicSet::Update,
            GameLogicSet::PostUpdate,
            RenderSet::Clear,
            RenderSet::DrawOpaque,
            RenderSet::DrawUI,
        )
            .chain(),
    );
    app.add_systems(Update, clear_scene_system.in_set(RenderSet::Clear));
    app.add_systems(Update, draw_opaque_system.in_set(RenderSet::DrawOpaque));
    app.add_systems(Update, present_window_system.in_set(RenderSet::DrawUI));
    app.set_runner(|app| {
        let event_loop = EventLoop::new().expect("failed to create winit event loop");
        let mut application = VelloApplication { app: Some(app) };
        event_loop
            .run_app(&mut application)
            .expect("Vello event loop failed");
        drop(application.app.take());
        AppExit::Success
    });
}

fn clear_scene_system(mut state: NonSendMut<VelloState>) {
    state.scene.reset();
}

fn draw_opaque_system(mut state: NonSendMut<VelloState>, query: Query<(&Transform, &Visual)>) {
    for (transform, visual) in &query {
        let half_width = f64::from(transform.size[0]) / 2.0;
        let half_height = f64::from(transform.size[1]) / 2.0;
        let rect = Rect::new(-half_width, -half_height, half_width, half_height);
        let affine = Affine::translate((
            f64::from(transform.position[0]),
            f64::from(transform.position[1]),
        )) * Affine::rotate(f64::from(transform.rotation));
        let color = Color::from_rgba8(
            color_channel(visual.color[0]),
            color_channel(visual.color[1]),
            color_channel(visual.color[2]),
            color_channel(visual.color[3]),
        );
        state.scene.fill(Fill::NonZero, affine, color, None, &rect);
    }
}

fn present_window_system(
    mut state: NonSendMut<VelloState>,
    mut window: NonSendMut<WindowState>,
    mut input: ResMut<InputState>,
) {
    input.left_down = window.left_down;
    input.right_down = window.right_down;
    input.space_pressed = window.space_pressed;
    window.space_pressed = false;

    if !state.valid_surface {
        return;
    }

    let VelloState {
        context,
        surface,
        renderer,
        scene,
        ..
    } = &mut *state;
    let device_handle = &context.devices[surface.dev_id];
    renderer
        .render_to_texture(
            &device_handle.device,
            &device_handle.queue,
            scene,
            &surface.target_view,
            &RenderParams {
                base_color: Color::from_rgb8(40, 45, 50),
                width: surface.config.width,
                height: surface.config.height,
                antialiasing_method: AaConfig::Area,
            },
        )
        .expect("Vello rendering failed");

    let surface_texture = match surface.surface.get_current_texture() {
        CurrentSurfaceTexture::Success(texture) => texture,
        CurrentSurfaceTexture::Outdated | CurrentSurfaceTexture::Suboptimal(_) => {
            context.configure_surface(surface);
            window.window.request_redraw();
            return;
        }
        CurrentSurfaceTexture::Occluded | CurrentSurfaceTexture::Timeout => return,
        CurrentSurfaceTexture::Lost => panic!("Vello surface was lost"),
        CurrentSurfaceTexture::Validation => panic!("Vello surface validation failed"),
    };
    let mut encoder =
        device_handle
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Vello surface blit"),
            });
    surface.blitter.copy(
        &device_handle.device,
        &mut encoder,
        &surface.target_view,
        &surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default()),
    );
    device_handle.queue.submit([encoder.finish()]);
    surface_texture.present();
}

fn color_channel(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::color_channel;

    #[test]
    fn color_conversion_clamps_and_rounds() {
        assert_eq!(color_channel(-1.0), 0);
        assert_eq!(color_channel(0.5), 128);
        assert_eq!(color_channel(1.0), 255);
        assert_eq!(color_channel(2.0), 255);
    }
}

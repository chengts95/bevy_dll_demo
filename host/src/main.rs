use bevy_app::{App, Startup};
use bevy_ecs::prelude::*;
use libloading::Library;
use shared_api::{Spin, Transform, Visual};

struct RunnerContext {
    app: App,
    _libs: Vec<Library>,
}

fn spawn_blocks(mut commands: Commands) {
    commands.spawn((
        Transform {
            position: [400.0, 300.0, 0.0],
            size: [100.0, 100.0, 0.0],
            rotation: 0.0,
        },
        Visual {
            color: [0.8, 0.2, 0.3, 1.0],
        },
        Spin { speed: 0.05 },
    ));
    commands.spawn((
        Transform {
            position: [200.0, 150.0, 0.0],
            size: [60.0, 60.0, 0.0],
            rotation: 0.0,
        },
        Visual {
            color: [0.2, 0.8, 0.3, 1.0],
        },
        Spin { speed: -0.02 },
    ));
}

fn main() {
    let mut ctx = RunnerContext {
        app: App::new(),
        _libs: Vec::new(),
    };

    ctx.app.add_systems(Startup, spawn_blocks);

    let target_dir = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };

    let dll_paths = vec![
        format!("/tmp/bevy_macroquad_target/{}/libmod_canvas.so", target_dir),
        format!("/tmp/bevy_macroquad_target/{}/libmod_block.so", target_dir),
    ];

    for path in dll_paths {
        unsafe {
            if let Ok(lib) = Library::new(&path) {
                if let Ok(setup_mod) =
                    lib.get::<unsafe extern "C" fn(*mut std::ffi::c_void)>(b"setup_mod")
                {
                    println!("Loaded mod from {}", path);
                    setup_mod(&mut ctx.app as *mut _ as *mut std::ffi::c_void);
                } else {
                    eprintln!("Failed to find setup_mod in {}", path);
                }
                ctx._libs.push(lib);
            } else {
                eprintln!("Failed to load {}", path);
            }
        }
    }

    println!("Running app...");
    ctx.app.run();
}

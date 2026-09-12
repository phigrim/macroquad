//! Run with `cargo run --release --no-default-features --features wgpu --example wgpu_smoke`.
use macroquad::prelude::*;
fn conf() -> Conf {
    Conf {
        window_title: "macroquad wgpu smoke".into(),
        sample_count: 4,
        platform: miniquad::conf::Platform {
            prefer_gfx_api: miniquad::conf::GfxApi::Wgpu,
            ..Default::default()
        },
        ..Default::default()
    }
}
#[macroquad::main(conf)]
async fn main() {
    let target = render_target(64, 64);
    for frame in 0..12 {
        set_camera(&Camera2D {
            render_target: Some(target.clone()),
            ..Camera2D::from_display_rect(Rect::new(0., 0., 64., 64.))
        });
        clear_background(Color::new(1., 0., 0., 1.));
        draw_rectangle(16., 16., 32., 32., Color::new(0., 1., 0., 1.));
        set_default_camera();
        clear_background(BLACK);
        draw_texture_ex(
            &target.texture,
            20.,
            20.,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(128., 128.)),
                ..Default::default()
            },
        );
        draw_text("OpenGL + Metal + wgpu", 20., 180., 28., WHITE);
        macroquad::ui::root_ui().label(None, "WGSL UI smoke test");
        if frame == 3 {
            request_new_screen_size(640., 480.);
        }
        next_frame().await;
    }
    let image = target.texture.get_texture_data();
    assert!(image.bytes.chunks_exact(4).any(|p| p == [255, 0, 0, 255]));
    assert!(image.bytes.chunks_exact(4).any(|p| p == [0, 255, 0, 255]));
    println!("wgpu window, text, UI, resize and offscreen readback passed");
}

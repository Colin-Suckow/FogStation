use std::{borrow::Cow, rc::Rc, time::{SystemTime, UNIX_EPOCH}};
use glium::{Texture2d, backend::Facade, texture::{ClientFormat, RawImage2d}, uniforms::{MagnifySamplerFilter, MinifySamplerFilter, SamplerBehavior}};
use imgui::*;
use imgui_glium_renderer::Texture;
use crate::{EmuState, support};


pub(crate) fn run_gui(mut state: EmuState) {
    let system = support::init("VaporStation");
    let mut start = SystemTime::now();
    let mut frame_time = 0;

    system.main_loop(move |_, ui, gl_ctx, textures| {
        start = SystemTime::now();
        if !state.halted {
            state.emu.run_frame();
        }
        frame_time = SystemTime::now()
            .duration_since(start)
            .expect("Error getting frame duration")
            .as_millis();

        Window::new(im_str!("Registers"))
            .size([300.0, 600.0], Condition::FirstUseEver)
            .build(ui, || {
                ui.text(format!("PC: {:#X}", &state.emu.r3000.pc));
                for (i, v) in state.emu.r3000.gen_registers.iter().enumerate() {
                    ui.text(format!("R{}: {:#X}", i, v));
                }
            });
        Window::new(im_str!("VRAM"))
            .content_size([1024.0, 512.0])
            .build(ui, || {
                let texture = create_texture_from_buffer(gl_ctx, state.emu.get_vram(), 1024, 512);
                let id = TextureId::new(0); //This is an awful hack that needs to be replaced
                textures.replace(id, texture);
                Image::new(id, [1024.0, 512.0]).build(ui);
            });

        Window::new(im_str!("Viewport"))
            .content_size([800.0, 600.0])
            .build(ui, || {
                let res = state.emu.display_resolution();
                let texture = create_texture_from_buffer(gl_ctx, state.emu.get_vram(), res.width as usize, res.height as usize);
                let id = TextureId::new(1); //This is an awful hack that needs to be replaced
                textures.replace(id, texture);
                Image::new(id, [800.0, 600.0]).build(ui);
            });

        Window::new(im_str!("Emulator Controls"))
            .content_size([250.0, 100.0])
            .build(ui, || {
                if ui.button(im_str!("Reset"), [80.0, 20.0]) {
                    state.emu.reset();
                }

                if ui.button(
                    if state.halted {
                        im_str!("Resume")
                    } else {
                        im_str!("Halt")
                    },
                    [80.0, 20.0],
                ) {
                    state.halted = !state.halted;
                }
                if !state.halted {
                    ui.text(format!("{:.1} FPS", (1000.0 / frame_time as f64)));
                } else {
                    ui.text("Halted");
                    if ui.button(im_str!("Step Instruction"), [120.0, 20.0]) {
                        state.emu.step_cycle();
                    }
                }

                if ui.button(
                    if state.logging {
                        im_str!("Stop Logging")
                    } else {
                        im_str!("Start Logging")
                    },
                    [120.0, 20.0],
                ) {
                    state.logging = !state.logging;
                    state.emu.r3000.log = state.logging;
                }

                match state.emu.loaded_disc() {
                    Some(disc) => ui.text(format!("Drive loaded: {}", disc.title())),
                    None => ui.text("No disc in drive"),
                };
            });
    });
}

/// Creates OpenGL texture from 16 bit, psx format, framebuffer
fn create_texture_from_buffer<F>(
    gl_ctx: &F,
    data: &Vec<u16>,
    width: usize,
    height: usize,
) -> Texture
where
    F: Facade,
{
    let mut gl_raw_data: Vec<u8> = Vec::new();
    for index in 0..(width * height) {
        gl_raw_data.extend_from_slice(&ps_pixel_to_gl(
            &data[((index / width) * 1024) + index % width],
        ));
    }

    let image = RawImage2d {
        data: Cow::from(gl_raw_data),
        width: width as u32,
        height: height as u32,
        format: ClientFormat::U8U8U8,
    };
    let gl_texture = Texture2d::new(gl_ctx, image).unwrap();
    Texture {
        texture: Rc::new(gl_texture),
        sampler: SamplerBehavior {
            magnify_filter: MagnifySamplerFilter::Linear,
            minify_filter: MinifySamplerFilter::Linear,
            ..Default::default()
        },
    }
}

///Converts 16 bit psx pixel format to u8u8u8
fn ps_pixel_to_gl(pixel_data: &u16) -> [u8; 3] {
    [
        ((pixel_data & 0x1F) * 8) as u8,
        (((pixel_data >> 5) & 0x1F) * 8) as u8,
        (((pixel_data >> 10) & 0x1F) * 8) as u8,
    ]
}
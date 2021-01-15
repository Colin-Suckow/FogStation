use imgui_glium_renderer::Texture;
use psx_emu::PSXEmu;
use std::{fs, rc::Rc};

use imgui::*;


use std::time::{Duration, Instant};

mod support;

use glium::{
    backend::Facade,
    texture::{ClientFormat, RawImage2d, Texture2d},
    uniforms::{MagnifySamplerFilter, MinifySamplerFilter, SamplerBehavior},
};

use std::borrow::Cow;

fn main() {
    let bios_data = match fs::read("SCPH1001.BIN") {
        Ok(data) => data,
        _ => {
            println!("Unable to read bios file. Make sure there is a file named SCPH1001.BIN in the same directory.");
            return;
        }
    };

    let mut emu = PSXEmu::new(bios_data);
    emu.reset();

    // loop {
    //     emu.step_instruction();
    // }

    // let start = Instant::now();
    // for _ in 0..100000 {
    //     emu.step_instruction();
    // }
    // let end_time = start.elapsed();
    // let elapsed_seconds = end_time.as_micros() as f64 * 0.000001;
    // println!("Frequency {:.2}mhz", (100000.0 / elapsed_seconds) / 1000000.0);

    let system = support::init("psx-emu");

    system.main_loop(move |_, ui, gl_ctx, textures| {
        emu.run_frame();
        Window::new(im_str!("Registers"))
            .size([300.0, 600.0], Condition::FirstUseEver)
            .build(ui, || {
                ui.text(format!("PC: {:#X}", &emu.r3000.pc));
                for (i, v) in emu.r3000.gen_registers.iter().enumerate() {
                    ui.text(format!("R{}: {:#X}", i, v));
                }
            });
        Window::new(im_str!("VRAM"))
            .content_size([1024.0, 512.0])
            .build(ui, || {
                let texture = create_texture_from_buffer(gl_ctx, emu.get_vram(), 1024, 512);
                let id = TextureId::new(0); //This is an awful hack that needs to be replaced
                textures.replace(id, texture);
                Image::new(id, [1024.0, 512.0]).build(ui);
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
    for p in data {
        gl_raw_data.append(&mut ps_pixel_to_gl(p));
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
fn ps_pixel_to_gl(pixel_data: &u16) -> Vec<u8> {
    vec![
        ((pixel_data & 0x1F) * 8) as u8,
        (((pixel_data >> 5) & 0x1F) * 8) as u8,
        (((pixel_data >> 10) & 0x1F) * 8) as u8,
    ]
}

#[cfg(test)]
mod pixel_tests {
    use super::*;
    #[test]
    fn test_ps_pixel_to_gl() {
        assert_eq!(ps_pixel_to_gl(&0xFFFF), vec![0xFF, 0xFF, 0xFF]);
    }
}

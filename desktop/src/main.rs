use imgui_glium_renderer::Texture;
use psx_emu::PSXEmu;

use byteorder::{ByteOrder, LittleEndian};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fs, rc::Rc};

use imgui::*;
use std::env;

mod support;

use glium::{
    backend::Facade,
    texture::{ClientFormat, RawImage2d, Texture2d},
    uniforms::{MagnifySamplerFilter, MinifySamplerFilter, SamplerBehavior},
};

use std::borrow::Cow;

fn main() {
    let args: Vec<String> = env::args().collect();
    let bios_data = match fs::read("SCPH1001.BIN") {
        //SCPH1001.BIN openbios.bin
        Ok(data) => data,
        _ => {
            println!("Unable to read bios file. Make sure there is a file named SCPH1001.BIN in the same directory.");
            return;
        }
    };

    let mut emu = PSXEmu::new(bios_data);
    let mut halted = false;
    emu.reset();

    if args.len() == 2 {
        let exe = fs::read(&args[1]).unwrap();
        let exe_data = exe[0x800..].to_vec();
        let destination = LittleEndian::read_u32(&exe[0x18..0x1C]);
        let entrypoint = LittleEndian::read_u32(&exe[0x10..0x14]);
        let init_sp = LittleEndian::read_u32(&exe[0x30..0x34]);
        println!(
            "Destination is {:#X}\nEntrypoint is {:#X}\nSP is {:#X}",
            destination, entrypoint, init_sp
        );
        emu.load_executable(destination, entrypoint, init_sp, &exe_data);
    }

    let system = support::init("VaporStation");
    let mut start = SystemTime::now();
    let mut frame_time = 0;

    system.main_loop(move |_, ui, gl_ctx, textures| {
        start = SystemTime::now();
        if !halted {
            emu.run_frame();
        }
        frame_time = SystemTime::now()
            .duration_since(start)
            .expect("Error getting frame duration")
            .as_millis();

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

        Window::new(im_str!("Viewport"))
            .content_size([800.0, 600.0])
            .build(ui, || {
                let texture = create_texture_from_buffer(gl_ctx, emu.get_vram(), 640, 480);
                let id = TextureId::new(1); //This is an awful hack that needs to be replaced
                textures.replace(id, texture);
                Image::new(id, [800.0, 600.0]).build(ui);
            });

        Window::new(im_str!("Emulator Controls"))
            .content_size([150.0, 100.0])
            .build(ui, || {
                if ui.button(im_str!("Reset"), [80.0, 20.0]) {
                    emu.reset();
                }

                if ui.button(
                    if halted {
                        im_str!("Resume")
                    } else {
                        im_str!("Halt")
                    },
                    [80.0, 20.0],
                ) {
                    halted = !halted;
                }
                if !halted {
                    ui.text(format!("{:.1} FPS", (1000.0 / frame_time as f64)));
                } else {
                    ui.text("Halted");
                }
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

#[cfg(test)]
mod pixel_tests {
    use super::*;
    //#[test]
    // fn test_ps_pixel_to_gl() {
    //     assert_eq!(ps_pixel_to_gl(&0xFFFF), vec![0xFF, 0xFF, 0xFF]);
    // }
}

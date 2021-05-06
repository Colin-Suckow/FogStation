use imgui_glium_renderer::Texture;
use psx_emu::PSXEmu;
use disc::*;
use getopts::Options;


use byteorder::{ByteOrder, LittleEndian};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fs, rc::Rc};

use imgui::*;
use std::env;

mod support;
mod disc;

use glium::{
    backend::Facade,
    texture::{ClientFormat, RawImage2d, Texture2d},
    uniforms::{MagnifySamplerFilter, MinifySamplerFilter, SamplerBehavior},
};

use std::borrow::Cow;
use std::path::{Path, PathBuf};



fn main() {
    let mut bios_path = "SCPH1001.BIN".to_string();
    let mut cue_path: Option<&str> = None;
    let mut exe_path: Option<&str> = None;
    let mut logging = false;
    let mut headless = false;

    let args: Vec<String> = env::args().collect();
    
    let mut opts = Options::new();
    opts.optopt("b", "bios", "BIOS file path", "FILE");
    opts.optopt("c", "cue", "CUE file path", "FILE");
    opts.optopt("e", "exe", "EXE file path", "FILE");
    opts.optflag("l", "log", "Enable logging");
    opts.optflag("h", "headless", "Run without GUI");

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => { m }
        Err(f) => { panic!(f.to_string()) }
    };

    if let Some(new_path) = matches.opt_str("b") {
        println!("Using alternate bios file: {}", new_path);
        bios_path = new_path;
    } else {
        println!("Using defualt bios file: {}", bios_path);
    }

    let bios_data = match fs::read(&bios_path) {
        //SCPH1001.BIN openbios.bin
        Ok(data) => data,
        _ => {
            println!("Unable to read bios file!");
            return;
        }
    };

    let mut emu = PSXEmu::new(bios_data);
    let mut halted = false;
    emu.reset();

    if matches.opt_present("l") {
        logging = true;
        emu.r3000.log = true;
    }

    if matches.opt_present("h") {
        headless = true;
    }

    if let Some(disc_path) = matches.opt_str("c") {
        println!("Loading CUE: {}", disc_path);
        let disc = load_disc_from_cuesheet(Path::new(&disc_path).to_path_buf());
        emu.load_disc(disc);
    }
    

    if let Some(exe_path) = matches.opt_str("e") {
        println!("Loading executable: {}", exe_path);
        let exe = fs::read(exe_path).unwrap();
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

    
    if !headless {

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
                .content_size([250.0, 100.0])
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
                        if ui.button(im_str!("Step Instruction"), [120.0, 20.0]) {
                            emu.step_instruction();
                        }
                    }
    
                    if ui.button(
                        if logging {
                            im_str!("Stop Logging")
                        } else {
                            im_str!("Start Logging")
                        },
                        [120.0, 20.0],
                    ) {
                        logging = !logging;
                        emu.r3000.log = logging;
                    }
    
                    match emu.loaded_disc() {
                        Some(disc) => ui.text(format!("Drive loaded: {}", disc.title())),
                        None => ui.text("No disc in drive")
                    };
    
                });
        });
    } else {
        //Run headless loop
        loop {
            emu.step_instruction();
        }
    }
    
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

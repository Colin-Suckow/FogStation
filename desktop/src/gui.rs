use std::{borrow::Cow, rc::Rc};
use glium::{Texture2d, backend::Facade, texture::{ClientFormat, RawImage2d}, uniforms::{MagnifySamplerFilter, MinifySamplerFilter, SamplerBehavior}};
use imgui::*;
use imgui_glium_renderer::Texture;
use winit::event::VirtualKeyCode;
use crate::{ClientMessage, EmuMessage, ClientState, support};
use psx_emu::controller::{ButtonState, ControllerType};
use psx_emu::gpu::Resolution;

pub(crate) fn run_gui(mut state: ClientState) {
    let system = support::init("VaporStation");
    let mut latest_frame: Vec<u16> = vec![0; 524_288];
    let mut latest_resolution = Resolution {
        width: 640,
        height: 480,
    };
    let mut times = AverageList::new();

    let mut awaiting_gdb = false;
    let mut latest_pc: u32 = 0;

    system.main_loop(move |_, ui, gl_ctx, textures| {
        state.comm.tx.send(EmuMessage::UpdateControllers(get_button_state(ui))).unwrap();

        loop {
            match state.comm.rx.try_recv() {
                Ok(msg) => {
                    match msg {
                        ClientMessage::FrameReady(frame, frame_time) => {
                            latest_frame = frame;
                            times.push(frame_time as usize);
                            state.comm.tx.send(EmuMessage::StartFrame).unwrap();
                        },
                        ClientMessage::ResolutionChanged(res) => latest_resolution = res,
                        ClientMessage::AwaitingGDBClient => {
                            awaiting_gdb = true;
                            state.halted = true;
                        },
                        ClientMessage::GDBClientConnected => {
                            awaiting_gdb = false;
                            state.halted = false;
                        },
                        ClientMessage::LatestPC(pc) => {
                            latest_pc = pc;
                        }
                    }
                },
                Err(e) => {
                    match e {
                        std::sync::mpsc::TryRecvError::Empty => break, // No messages left, break out of the loop
                        std::sync::mpsc::TryRecvError::Disconnected => panic!("Emu thread died!"),
                    }
                },
            }
        }

        // Window::new(im_str!("Registers"))
        //     .size([300.0, 600.0], Condition::FirstUseEver)
        //     .build(ui, || {
        //         ui.text(format!("PC: {:#X}", &state.emu.r3000.pc));
        //         for (i, v) in state.emu.r3000.gen_registers.iter().enumerate() {
        //             ui.text(format!("R{}: {:#X}", i, v));
        //         }
        //     });
        Window::new(im_str!("VRAM"))
            .content_size([1024.0, 512.0])
            .build(ui, || {
                let texture = create_texture_from_buffer(gl_ctx, &latest_frame, 1024, 512);
                let id = TextureId::new(0); //This is an awful hack that needs to be replaced
                textures.replace(id, texture);
                Image::new(id, [1024.0, 512.0]).build(ui);
            });

        Window::new(im_str!("Viewport"))
            .content_size([800.0, 600.0])
            .build(ui, || {
                let texture = create_texture_from_buffer(gl_ctx, &latest_frame, latest_resolution.width as usize, latest_resolution.height as usize);
                let id = TextureId::new(1); //This is an awful hack that needs to be replaced
                textures.replace(id, texture);
                Image::new(id, [800.0, 600.0]).build(ui);
            });

        Window::new(im_str!("Emulator Controls"))
            .content_size([250.0, 100.0])
            .build(ui, || {
                if ui.button(im_str!("Reset"), [80.0, 20.0]) {
                    state.comm.tx.send(EmuMessage::Reset).unwrap();
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
                    if state.halted {
                        state.comm.tx.send(EmuMessage::Halt).unwrap();
                    } else {
                        state.comm.tx.send(EmuMessage::Continue).unwrap();
                    }
                }
                if !state.halted {
                    ui.text(format!("{:.1} FPS", (1000.0 / times.average())));
                } else {
                    ui.text("Halted");
                    ui.text(format!("PC: {:#X}", latest_pc));
                    if ui.button(im_str!("Step Instruction"), [120.0, 20.0]) {
                        state.comm.tx.send(EmuMessage::StepCPU).unwrap();
                    }
                }

                if awaiting_gdb {
                    ui.text(im_str!("Awaiting connection from GDB client!"));
                }

                // if ui.button(
                //     if state.logging {
                //         im_str!("Stop Logging")
                //     } else {
                //         im_str!("Start Logging")
                //     },
                //     [120.0, 20.0],
                // ) {
                //     state.logging = !state.logging;
                //     state.emu.r3000.log = state.logging;
                // }

                // match state.emu.loaded_disc() {
                //     Some(disc) => ui.text(format!("Drive loaded: {}", disc.title())),
                //     None => ui.text("No disc in drive"),
                // };
            });
    });
}

fn get_button_state(ui: &mut Ui) -> ButtonState {
    ButtonState {
        controller_type: ControllerType::DigitalPad,
        button_x: is_key_down(ui, VirtualKeyCode::K),
        button_square: is_key_down(ui, VirtualKeyCode::J),
        button_triangle: is_key_down(ui, VirtualKeyCode::I),
        button_circle: is_key_down(ui, VirtualKeyCode::L),
        button_up: is_key_down(ui, VirtualKeyCode::W),
        button_down: is_key_down(ui, VirtualKeyCode::S),
        button_left: is_key_down(ui, VirtualKeyCode::A),
        button_right: is_key_down(ui, VirtualKeyCode::D),
        button_l1: false,
        button_l2: false,
        button_l3: false,
        button_r1: false,
        button_r2: false,
        button_r3: false,
        button_select: false,
        button_start: is_key_down(ui, VirtualKeyCode::Apostrophe),
        
    }
}

fn is_key_down(ui: &mut Ui, keycode: VirtualKeyCode) -> bool {
    ui.io().keys_down[keycode as usize]
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

struct AverageList {
    values: [usize; 32]
}

impl AverageList {
    fn new() -> Self {
        Self {
            values: [0; 32]
        }
    }

    fn push(&mut self, val: usize) {
        self.values.rotate_right(1);
        self.values[0] = val;
    }

    fn average(&self) -> f64 {
        let mut sum = 0;
        for val in &self.values {
            sum += val;
        }

        sum as f64 / 32.0

    }
}
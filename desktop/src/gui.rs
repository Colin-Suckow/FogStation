use eframe::{egui::{self, Direction, TextureId, pos2}, epi};
use psx_emu::gpu::Resolution;

use crate::{ClientMessage, ClientState, EmuMessage};

// use std::{borrow::Cow, rc::Rc};
// use glium::{Texture2d, backend::Facade, texture::{ClientFormat, RawImage2d}, uniforms::{MagnifySamplerFilter, MinifySamplerFilter, SamplerBehavior}};
// use winit::event::VirtualKeyCode;
// use crate::{ClientMessage, EmuMessage, ClientState};
// use psx_emu::controller::{ButtonState, ControllerType};
// use psx_emu::gpu::Resolution;

const VRAM_WIDTH: usize = 1024;
const VRAM_HEIGHT: usize = 512;

pub(crate) fn run_gui(state: ClientState) {
    let app = VaporstationApp::new(state);
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(Box::new(app), native_options);
}

struct VaporstationApp {
    emu_handle: ClientState,
    times: AverageList,
    latest_resolution: Resolution,
    awaiting_gdb: bool,
    latest_pc: u32,
    vram_texture: Option<TextureId>,
    show_vram_window: bool,
}

impl VaporstationApp {
    fn new(state: ClientState) -> Self {
        let default_resolution = Resolution {
            width: 640,
            height: 480,
        };

        Self {
            emu_handle: state,
            times: AverageList::new(),
            latest_resolution: default_resolution,
            awaiting_gdb: false,
            latest_pc: 0,
            vram_texture: None,
            show_vram_window: false,
        }
    }
}

impl epi::App for VaporstationApp {

    fn setup(&mut self, _ctx: &egui::CtxRef, frame: &mut epi::Frame<'_>, _storage: Option<&dyn epi::Storage>) {
        self.emu_handle.comm.tx.send(EmuMessage::RequestDrawCallback(frame.repaint_signal())).unwrap();
    }


    fn update(&mut self, ctx: &eframe::egui::CtxRef, frame: &mut epi::Frame<'_>) {
        
        // Process emu messages until empty
        loop {
            match self.emu_handle.comm.rx.try_recv() {
                Ok(msg) => match msg {
                    ClientMessage::FrameReady(vram_frame, frame_time) => {
                        self.vram_texture = Some(create_texture_from_buffer(frame, &vram_frame, VRAM_WIDTH, VRAM_HEIGHT));
                        self.times.push(frame_time as usize);
                    }
                    ClientMessage::ResolutionChanged(res) => self.latest_resolution = res,
                    ClientMessage::AwaitingGDBClient => {
                        self.awaiting_gdb = true;
                        self.emu_handle.halted = true;
                    }
                    ClientMessage::GDBClientConnected => {
                        self.awaiting_gdb = false;
                        self.emu_handle.halted = false;
                    }
                    ClientMessage::LatestPC(pc) => {
                        self.latest_pc = pc;
                    }
                },
                Err(e) => {
                    match e {
                        std::sync::mpsc::TryRecvError::Empty => break, // No messages left, break out of the loop
                        std::sync::mpsc::TryRecvError::Disconnected => panic!("Emu thread died!"),
                    }
                }
            }
        }

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:
            egui::menu::bar(ui, |ui| {
                egui::menu::menu(ui, "File", |ui| {
                    if ui.button("Quit").clicked() {
                        frame.quit();
                    }
                });
                egui::menu::menu(ui, "Debug", |ui| {
                    ui.checkbox(&mut self.show_vram_window, "VRAM Viewer");
                });
                ui.label(format!("{:.2} fps", self.times.average()));
            });
        });

        if self.show_vram_window {
            egui::Window::new("VRAM Viewer").show(ctx, |ui| {
                if let Some(vram) = self.vram_texture {
                    ui.image(vram, [VRAM_WIDTH as f32, VRAM_HEIGHT as f32]);
                }
            });
        }
        

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(vram) = self.vram_texture {
                ui.with_layout(egui::Layout::centered_and_justified(Direction::TopDown), |ui| {
                    let width = self.latest_resolution.width as usize;
                    let height = self.latest_resolution.height as usize;
                    let ratio = width as f32 / height as f32;
                    let scaled_height = ui.max_rect().height();
                    let scaled_width = scaled_height * ratio;
                    let viewport_rect = egui::Rect::from_min_max(pos2(0.0,0.0), pos2((width - 1) as f32 / VRAM_WIDTH as f32, (height - 1) as f32 / VRAM_HEIGHT as f32));
                    let image = egui::Image::new(vram, [scaled_width, scaled_height]).uv(viewport_rect);
                    ui.add(image);
                });
                
            }
            
        });
    }

    fn name(&self) -> &str {
        "Vaporstation"
    }
}

// fn create_display(event_loop: &glutin::event_loop::EventLoop<()>) -> glium::Display {
//     let window_builder = glutin::window::WindowBuilder::new()
//         .with_resizable(true)
//         .with_inner_size(glutin::dpi::LogicalSize {
//             width: 800.0,
//             height: 600.0,
//         })
//         .with_title("egui_glium example");

//     let context_builder = glutin::ContextBuilder::new()
//         .with_depth_buffer(0)
//         .with_srgb(true)
//         .with_stencil_buffer(0)
//         .with_vsync(true);

//     glium::Display::new(window_builder, context_builder, event_loop).unwrap()
// }

// pub(crate) fn run_gui(mut state: ClientState) {
//     let event_loop = glutin::event_loop::EventLoop::with_user_event();
//     let display = create_display(&event_loop);
//     let mut egui_glium = egui_glium::EguiGlium::new(&display);

//     let mut latest_frame: Vec<u16> = vec![0; 524_288];
//     let mut latest_resolution = Resolution {
//         width: 640,
//         height: 480,
//     };
//     let mut times = AverageList::new();

//     let mut awaiting_gdb = false;
//     let mut latest_pc: u32 = 0;

//     event_loop.run(move |event, _, control_flow| {
//         state.comm.tx.send(EmuMessage::UpdateControllers(get_button_state(ui))).unwrap();

//         loop {
//             match state.comm.rx.try_recv() {
//                 Ok(msg) => {
//                     match msg {
//                         ClientMessage::FrameReady(frame, frame_time) => {
//                             latest_frame = frame;
//                             times.push(frame_time as usize);
//                             state.comm.tx.send(EmuMessage::StartFrame).unwrap();
//                         },
//                         ClientMessage::ResolutionChanged(res) => latest_resolution = res,
//                         ClientMessage::AwaitingGDBClient => {
//                             awaiting_gdb = true;
//                             state.halted = true;
//                         },
//                         ClientMessage::GDBClientConnected => {
//                             awaiting_gdb = false;
//                             state.halted = false;
//                         },
//                         ClientMessage::LatestPC(pc) => {
//                             latest_pc = pc;
//                         }
//                     }
//                 },
//                 Err(e) => {
//                     match e {
//                         std::sync::mpsc::TryRecvError::Empty => break, // No messages left, break out of the loop
//                         std::sync::mpsc::TryRecvError::Disconnected => panic!("Emu thread died!"),
//                     }
//                 },
//             }
//         }

//         // Window::new(im_str!("Registers"))
//         //     .size([300.0, 600.0], Condition::FirstUseEver)
//         //     .build(ui, || {
//         //         ui.text(format!("PC: {:#X}", &state.emu.r3000.pc));
//         //         for (i, v) in state.emu.r3000.gen_registers.iter().enumerate() {
//         //             ui.text(format!("R{}: {:#X}", i, v));
//         //         }
//         //     });
//         Window::new(im_str!("VRAM"))
//             .content_size([1024.0, 512.0])
//             .build(ui, || {
//                 let texture = create_texture_from_buffer(gl_ctx, &latest_frame, 1024, 512);
//                 let id = TextureId::new(0); //This is an awful hack that needs to be replaced
//                 textures.replace(id, texture);
//                 Image::new(id, [1024.0, 512.0]).build(ui);
//             });

//         Window::new(im_str!("Viewport"))
//             .content_size([800.0, 600.0])
//             .build(ui, || {
//                 let texture = create_texture_from_buffer(gl_ctx, &latest_frame, latest_resolution.width as usize, latest_resolution.height as usize);
//                 let id = TextureId::new(1); //This is an awful hack that needs to be replaced
//                 textures.replace(id, texture);
//                 Image::new(id, [800.0, 600.0]).build(ui);
//             });

//         Window::new(im_str!("Emulator Controls"))
//             .content_size([250.0, 100.0])
//             .build(ui, || {
//                 if ui.button(im_str!("Reset"), [80.0, 20.0]) {
//                     state.comm.tx.send(EmuMessage::Reset).unwrap();
//                 }

//                 if ui.button(
//                     if state.halted {
//                         im_str!("Resume")
//                     } else {
//                         im_str!("Halt")
//                     },
//                     [80.0, 20.0],
//                 ) {
//                     state.halted = !state.halted;
//                     if state.halted {
//                         state.comm.tx.send(EmuMessage::Halt).unwrap();
//                     } else {
//                         state.comm.tx.send(EmuMessage::Continue).unwrap();
//                     }
//                 }
//                 if !state.halted {
//                     ui.text(format!("{:.1} FPS", (1000.0 / times.average())));
//                 } else {
//                     ui.text("Halted");
//                     ui.text(format!("PC: {:#X}", latest_pc));
//                     if ui.button(im_str!("Step Instruction"), [120.0, 20.0]) {
//                         state.comm.tx.send(EmuMessage::StepCPU).unwrap();
//                     }
//                 }

//                 if awaiting_gdb {
//                     ui.text(im_str!("Awaiting connection from GDB client!"));
//                 }

//                 // if ui.button(
//                 //     if state.logging {
//                 //         im_str!("Stop Logging")
//                 //     } else {
//                 //         im_str!("Start Logging")
//                 //     },
//                 //     [120.0, 20.0],
//                 // ) {
//                 //     state.logging = !state.logging;
//                 //     state.emu.r3000.log = state.logging;
//                 // }

//                 // match state.emu.loaded_disc() {
//                 //     Some(disc) => ui.text(format!("Drive loaded: {}", disc.title())),
//                 //     None => ui.text("No disc in drive"),
//                 // };
//             });
//     });
// }

// fn get_button_state(ui: &mut Ui) -> ButtonState {
//     ButtonState {
//         controller_type: ControllerType::DigitalPad,
//         button_x: is_key_down(ui, VirtualKeyCode::K),
//         button_square: is_key_down(ui, VirtualKeyCode::J),
//         button_triangle: is_key_down(ui, VirtualKeyCode::I),
//         button_circle: is_key_down(ui, VirtualKeyCode::L),
//         button_up: is_key_down(ui, VirtualKeyCode::W),
//         button_down: is_key_down(ui, VirtualKeyCode::S),
//         button_left: is_key_down(ui, VirtualKeyCode::A),
//         button_right: is_key_down(ui, VirtualKeyCode::D),
//         button_l1: false,
//         button_l2: false,
//         button_l3: false,
//         button_r1: false,
//         button_r2: false,
//         button_r3: false,
//         button_select: false,
//         button_start: is_key_down(ui, VirtualKeyCode::Apostrophe),

//     }
// }

// fn is_key_down(ui: &mut Ui, keycode: VirtualKeyCode) -> bool {
//     ui.io().keys_down[keycode as usize]
// }

/// Creates eframe texture from 16 bit, psx format, framebuffer
fn create_texture_from_buffer(
    frame: &mut epi::Frame<'_>,
    data: &Vec<u16>,
    width: usize,
    height: usize,
) -> TextureId {
    let pixel_data = data
        .iter()
        .map(|p| {
            let colors = ps_pixel_to_gl(p);
            egui::Color32::from_rgba_unmultiplied(colors[0], colors[1], colors[2], 255)
        })
        .collect::<Vec<_>>();

    frame
        .tex_allocator()
        .alloc_srgba_premultiplied((width, height), &pixel_data)
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
    values: [usize; 32],
}

impl AverageList {
    fn new() -> Self {
        Self { values: [0; 32] }
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

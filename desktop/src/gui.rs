use std::ops::Add;

use byteorder::LittleEndian;
use eframe::{
    egui::{self, pos2, Direction, Key, Layout, TextureId, Color32, Rect, Pos2},
    epi,
};
use psx_emu::{
    controller::{ButtonState, ControllerType},
    gpu::{DrawCall, Resolution, Transparency},
};

use crate::{ClientMessage, ClientState, EmuMessage};

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
    display_texture: Option<TextureId>,
    show_vram_window: bool,
    gdb_connected: bool,
    display_origin: (usize, usize),
    latest_gpu_log: Vec<DrawCall>,
    show_gpu_call_window: bool,
    highlighted_gpu_calls: Vec<usize>,
    last_frame_data: Vec<Color32>,
    memory_logging: bool,
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
            gdb_connected: false,
            display_origin: (0, 0),
            latest_gpu_log: vec![],
            show_gpu_call_window: false,
            highlighted_gpu_calls: vec![],
            last_frame_data: vec!(),
            memory_logging: false,
            display_texture: None,
        }
    }

    fn set_halt(&mut self, should_halt: bool) {
        self.emu_handle.halted = should_halt;
        if self.emu_handle.halted {
            self.emu_handle.comm.tx.send(EmuMessage::Halt).unwrap();
        } else {
            self.emu_handle.comm.tx.send(EmuMessage::Continue).unwrap();
        }
    }

    fn halted(&self) -> bool {
        self.emu_handle.halted
    }
}

impl epi::App for VaporstationApp {
    fn setup(
        &mut self,
        _ctx: &egui::CtxRef,
        frame: &mut epi::Frame<'_>,
        _storage: Option<&dyn epi::Storage>,
    ) {
        self.emu_handle
            .comm
            .tx
            .send(EmuMessage::RequestDrawCallback(frame.repaint_signal()))
            .unwrap();
    }

    fn update(&mut self, ctx: &eframe::egui::CtxRef, frame: &mut epi::Frame<'_>) {
        self.emu_handle
            .comm
            .tx
            .send(EmuMessage::UpdateControllers(get_button_state(ctx.input())))
            .unwrap();
        // Process emu messages until empty
        loop {
            match self.emu_handle.comm.rx.try_recv() {
                Ok(msg) => match msg {
                    ClientMessage::FrameReady(vram_frame, frame_time, is_full_color) => {
                        // Free the old texture if it exists
                        if let Some(vram_texture) = self.vram_texture {
                            frame.tex_allocator().free(vram_texture);
                        }

                        if let Some(display_texture) = self.display_texture {
                            frame.tex_allocator().free(display_texture);
                        }


                        let pixel_data = transform_psx16_to_32(&vram_frame, 0, 0, VRAM_WIDTH as u32, VRAM_HEIGHT as u32);
                        
                        self.vram_texture = Some(frame
                            .tex_allocator()
                            .alloc_srgba_premultiplied((VRAM_WIDTH, VRAM_HEIGHT), &pixel_data));

                            
                            
                        let display_data = if is_full_color {
                            transform_psx24_to_32(&vram_frame, self.display_origin.0 as u32, self.display_origin.1 as u32, self.latest_resolution.width, self.latest_resolution.height)
                        } else {
                            transform_psx16_to_32(&vram_frame, self.display_origin.0 as u32, self.display_origin.1 as u32, self.latest_resolution.width, self.latest_resolution.height)
                        };
                            
                        self.display_texture = Some(frame
                            .tex_allocator()
                            .alloc_srgba_premultiplied((self.latest_resolution.width as usize, self.latest_resolution.height as usize), &display_data));

                        self.last_frame_data = pixel_data;
                        self.times.push(frame_time as usize);
                    }
                    ClientMessage::ResolutionChanged(res) => self.latest_resolution = res,
                    ClientMessage::AwaitingGDBClient => {
                        self.awaiting_gdb = true;
                        self.emu_handle.halted = true;
                    }
                    ClientMessage::GDBClientConnected => {
                        self.awaiting_gdb = false;
                        self.gdb_connected = true;
                    }
                    ClientMessage::LatestPC(pc) => {
                        self.latest_pc = pc;
                    }
                    ClientMessage::Halted => self.emu_handle.halted = true,
                    ClientMessage::Continuing => self.emu_handle.halted = false,
                    ClientMessage::DisplayOriginChanged(new_origin) => {
                        self.display_origin = new_origin
                    }
                    ClientMessage::LatestGPULog(call_log) => {
                        self.latest_gpu_log = call_log;
                        self.highlighted_gpu_calls.clear();
                        println!("Calls in log: {}", self.latest_gpu_log.len());
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
            egui::menu::bar(ui, |ui| {
                egui::menu::menu(ui, "File", |ui| {
                    if ui.button("Quit").clicked() {
                        frame.quit();
                    }
                });
                egui::menu::menu(ui, "Control", |ui| {
                    let halt_button_text = if self.halted() { "Resume" } else { "Halt" };
                    if ui.button(halt_button_text).clicked() {
                        self.set_halt(!self.halted());
                    };

                    if ui
                        .checkbox(&mut self.emu_handle.frame_limited, "Frame Limiter")
                        .clicked()
                    {
                        self.emu_handle
                            .comm
                            .tx
                            .send(EmuMessage::SetFrameLimiter(self.emu_handle.frame_limited))
                            .unwrap();
                    };
                });
                egui::menu::menu(ui, "Debug", |ui| {
                    ui.checkbox(&mut self.show_vram_window, "VRAM Viewer");
                    ui.checkbox(&mut self.show_gpu_call_window, "GPU Call Debugger");
                    if ui.checkbox(&mut self.memory_logging, "Memory Logging").clicked() {
                        self.emu_handle
                            .comm
                            .tx
                            .send(EmuMessage::SetMemLogging(self.memory_logging))
                            .unwrap();
                    };
                });

                ui.with_layout(Layout::right_to_left(), |ui| {
                    if self.halted() {
                        ui.label(format!("HALTED at {:#X}", self.latest_pc));
                    } else {
                        ui.label(format!("{:.2} fps", 1000.0 / self.times.average()));
                    }

                    if self.awaiting_gdb {
                        ui.label("Awaiting GDB connection...");
                    }

                    if self.gdb_connected {
                        ui.label("GDB Connected");
                    }
                });
            });
        });

        if self.show_vram_window {
            egui::Window::new("VRAM Viewer").show(ctx, |ui| {
                if let Some(vram) = self.vram_texture {
                    ui.image(vram, [VRAM_WIDTH as f32, VRAM_HEIGHT as f32]);
                }
            });
        }

        if self.show_gpu_call_window {
            egui::Window::new("GPU Call Debugger").show(ctx, |ui| {
                if self.halted() {
                    if self.latest_gpu_log.len() == 0 {
                        ui.label("No GPU calls were made during this frame :(");
                    } else {
                        // Grid header
                        egui::Grid::new("draw_element_grid_header")
                            .striped(true)
                            .show(ui, |ui| {
                                ui.label("Number");
                                ui.label("Type");
                                ui.label("Shading");
                                ui.label("Surface");
                                ui.label("Transparency");
                                ui.label("CLUT Depth");
                                ui.label("Highlighted?");
                                ui.end_row();
                            });

                        // Grid contents
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            egui::Grid::new("draw_element_grid").show(ui, |ui| {
                                for (i, command) in self.latest_gpu_log.iter().enumerate() {
                                    if command.call_dropped {
                                        ui.label(format!("{}x", i));
                                    } else {
                                        ui.label(format!("{} ", i));
                                    }

                                    ui.label(command.operation.to_string());
                                    if let Some(shading) = command.shading {
                                        ui.label(shading.to_string());
                                    } else {
                                        ui.label("N/A");
                                    }

                                    if let Some(surface) = command.surface {
                                        ui.label(surface.to_string());
                                    } else {
                                        ui.label("N/A");
                                    }

                                    if let Some(transparency) = command.transparency {
                                        ui.label(transparency.to_string());
                                    } else {
                                        ui.label("N/A");
                                    }

                                    ui.label(command.clut_size.to_string());
                                  
                                    let mut should_be_highlighted =
                                        self.highlighted_gpu_calls.contains(&i);
                                    ui.checkbox(&mut should_be_highlighted, "");

                                    let mut should_update_highlights = false;

                                    if should_be_highlighted
                                        && !self.highlighted_gpu_calls.contains(&i)
                                    {
                                        self.highlighted_gpu_calls.push(i);
                                        should_update_highlights = true;
                                    } else if !should_be_highlighted
                                        && self.highlighted_gpu_calls.contains(&i)
                                    {
                                        let index = self
                                            .highlighted_gpu_calls
                                            .iter()
                                            .position(|x| *x == i)
                                            .unwrap();
                                        self.highlighted_gpu_calls.remove(index);
                                        should_update_highlights = true;
                                    }

                                    // Push a newly highlighted frame to the screen
                                    if should_update_highlights {
                                        let mut new_frame = self.last_frame_data.clone();

                                        apply_highlights(&self, &mut new_frame);


                                         // Free the old texture if it exists
                                        if let Some(vram_texture) = self.vram_texture {
                                            frame.tex_allocator().free(vram_texture);
                                        }

                                        self.vram_texture = Some(frame
                                            .tex_allocator()
                                            .alloc_srgba_premultiplied((VRAM_WIDTH, VRAM_HEIGHT), &new_frame));
                
                                    }

                                    ui.end_row();
                                }
                            });
                        });
                    }
                } else {
                    ui.label("Must be halted to use gpu call debugger");
                }
            });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(display_texture) = self.display_texture {
                ui.with_layout(
                    egui::Layout::centered_and_justified(Direction::TopDown),
                    |ui| {
                        
                        let pane_size = ui.max_rect();
                        let (scaled_height, scaled_width) =
                            if pane_size.width() > pane_size.height() * 1.3333 {
                                (pane_size.height(), pane_size.height() * 1.3333)
                            } else {
                                (pane_size.width() * 0.75, pane_size.width())
                            };
                            
            
                        let image = egui::Image::new(display_texture, [scaled_width, scaled_height]).uv(Rect { min: Pos2::new(0.00625, 0.00833), max: Pos2::new(1.0 - 0.00625, 1.0 - 0.00833)});
                        ui.add(image);
                    },
                );
            }
        });
    }

    fn name(&self) -> &str {
        "Vaporstation"
    }
}


fn get_button_state(input_state: &egui::InputState) -> ButtonState {
    ButtonState {
        controller_type: ControllerType::DigitalPad,
        button_x: input_state.key_down(Key::K),
        button_square: input_state.key_down(Key::J),
        button_triangle: input_state.key_down(Key::I),
        button_circle: input_state.key_down(Key::L),
        button_up: input_state.key_down(Key::W),
        button_down: input_state.key_down(Key::S),
        button_left: input_state.key_down(Key::A),
        button_right: input_state.key_down(Key::D),
        button_l1: input_state.key_down(Key::E),
        button_l2: input_state.key_down(Key::Q),
        button_l3: false,
        button_r1: input_state.key_down(Key::U),
        button_r2: input_state.key_down(Key::P),
        button_r3: false,
        button_select: false,
        button_start: input_state.key_down(Key::Enter),
    }
}


fn transform_psx16_to_32(psx_data: &Vec<u16>, origin_x: u32, origin_y: u32, width: u32, height: u32) -> Vec<Color32> {
    psx_data.iter()
        .enumerate()
        .filter(|(i, v)| {
            (i % VRAM_WIDTH) >= origin_x as usize &&  (i / VRAM_WIDTH) >= origin_y as usize && (i % VRAM_WIDTH) < (origin_x + width) as usize && (i / VRAM_WIDTH) < (origin_y + height) as usize
        })
        .map(|(i, p)| {
            let colors = ps_pixel_to_gl(p);
            egui::Color32::from_rgba_unmultiplied(colors[0], colors[1], colors[2], 255)
        })
        .collect::<Vec<_>>()
}

fn transform_psx24_to_32(psx_data: &Vec<u16>, origin_x: u32, origin_y: u32, width: u32, height: u32) -> Vec<Color32> {
    psx_data.iter()
        .fold(vec!(), |mut vec, val| {
            vec.extend(val.to_le_bytes());
            vec
        })
        .iter()
        .enumerate()
        .filter(|(i, v)| {
            (i % (VRAM_WIDTH * 2)) >= (origin_x * 3) as usize && ((i) / (VRAM_WIDTH * 2)) >= origin_y as usize && (i % (VRAM_WIDTH * 2)) < ((origin_x + width) * 3) as usize && ((i) / (VRAM_WIDTH * 2)) < (origin_y + height) as usize
        })
        .map(|(i, v)| {*v})
        .collect::<Vec<u8>>()
        .chunks_exact(3).map(|colors| {
            egui::Color32::from_rgba_unmultiplied(colors[0], colors[1], colors[2], 255)
        }).collect()
}

fn apply_highlights(app: &VaporstationApp, pixel_data: &mut Vec<Color32>) {
    for call_index in &app.highlighted_gpu_calls {
        let call = &app.latest_gpu_log[*call_index];

        if let Some(points) = &call.points {
            let min_x = points.iter().min_by_key(|v| v.x).unwrap().x;
            let min_y = points.iter().min_by_key(|v| v.y).unwrap().y;
            
            let max_x = points.iter().max_by_key(|v| v.x).unwrap().x;
            let max_y = points.iter().max_by_key(|v| v.y).unwrap().y;

            let tex_base_x = (call.tex_base_x * 64) as i16;
            let tex_base_y = (call.tex_base_y * 256) as i16;

            let tex_min_x = points.iter().min_by_key(|v| v.tex_x).unwrap().tex_x;
            let tex_min_y = points.iter().min_by_key(|v| v.tex_y).unwrap().tex_y;

            let clut_div = match call.clut_size {
                psx_emu::gpu::TextureColorMode::FourBit => 4,
                psx_emu::gpu::TextureColorMode::EightBit => 2,
                psx_emu::gpu::TextureColorMode::FifteenBit => 1,
            };

            // Do some wacky division stuff so the adjust the highlight size for clut
            let tex_max_x = ((points.iter().max_by_key(|v| v.tex_x).unwrap().tex_x - tex_min_x) / clut_div) + tex_min_x;
            let tex_max_y = points.iter().max_by_key(|v| v.tex_y).unwrap().tex_y;

            println!("Highlighting ({}, {}) -> ({}, {})", min_x, min_y, max_x, max_y);
            println!("Tex coords ({}, {}) -> ({}, {})", tex_min_x, tex_min_y, tex_max_x, tex_max_y);
            println!("base x {} base y {}", tex_base_x, tex_base_y);

            for y in min_y..max_y {
                for x in min_x..max_x {
                    let addr = (y as i32) * 1024 + x as i32;
                    let current_pixel = pixel_data[addr as usize];
                    let highlight_color = Color32::from_rgba_unmultiplied(155, 0, 0, 155);

                    pixel_data[addr as usize] = Color32::from_rgba_unmultiplied((current_pixel.r() + highlight_color.r()), (current_pixel.g() + highlight_color.g()), (current_pixel.b() + highlight_color.b()), 255);
                }
            }

            for y in tex_min_y..tex_max_y {
                for x in tex_min_x..tex_max_x {
                    let addr = ((y + tex_base_y) as i32) * 1024 + (x + tex_base_x) as i32;
                    let current_pixel = pixel_data[addr as usize];
                    let highlight_color = Color32::from_rgba_unmultiplied(0, 155, 0, 155);

                    pixel_data[addr as usize] = Color32::from_rgba_unmultiplied((current_pixel.r() + highlight_color.r()), (current_pixel.g() + highlight_color.g()), (current_pixel.b() + highlight_color.b()), 255);
                }
            }
        }
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

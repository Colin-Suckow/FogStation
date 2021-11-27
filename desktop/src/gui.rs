use eframe::{egui::{self, Direction, Key, Layout, TextureId, pos2}, epi};
use psx_emu::{controller::{ButtonState, ControllerType}, gpu::Resolution};

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
    show_vram_window: bool,
    gdb_connected: bool,
    display_origin: (usize, usize),
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
            display_origin: (0,0),
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

    fn setup(&mut self, _ctx: &egui::CtxRef, frame: &mut epi::Frame<'_>, _storage: Option<&dyn epi::Storage>) {
        self.emu_handle.comm.tx.send(EmuMessage::RequestDrawCallback(frame.repaint_signal())).unwrap();
    }


    fn update(&mut self, ctx: &eframe::egui::CtxRef, frame: &mut epi::Frame<'_>) {
        self.emu_handle.comm.tx.send(EmuMessage::UpdateControllers(get_button_state(ctx.input()))).unwrap();
        // Process emu messages until empty
        loop {
            match self.emu_handle.comm.rx.try_recv() {
                Ok(msg) => match msg {
                    ClientMessage::FrameReady(vram_frame, frame_time) => {
                        // Free the old texture if it exists
                        if let Some(vram_texture) = self.vram_texture {
                            frame.tex_allocator().free(vram_texture);
                        }
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
                        self.gdb_connected = true;
                    }
                    ClientMessage::LatestPC(pc) => {
                        self.latest_pc = pc;
                    }
                    ClientMessage::Halted => self.emu_handle.halted = true,
                    ClientMessage::Continuing => self.emu_handle.halted = false,
                    ClientMessage::DisplayOriginChanged(new_origin) => self.display_origin = new_origin,
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
                    let halt_button_text = if self.halted() {"Resume"} else {"Halt"};
                    if ui.button(halt_button_text).clicked() {
                        self.set_halt(!self.halted());
                    };

                    if ui.checkbox(&mut self.emu_handle.frame_limited, "Frame Limiter").clicked() {
                        self.emu_handle.comm.tx.send(EmuMessage::SetFrameLimiter(self.emu_handle.frame_limited)).unwrap();
                    };
                });
                egui::menu::menu(ui, "Debug", |ui| {
                    ui.checkbox(&mut self.show_vram_window, "VRAM Viewer");
                });

                ui.with_layout(Layout::right_to_left(), |ui| {
                    if self.emu_handle.halted {
                        ui.label("HALTED");
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
        

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(vram) = self.vram_texture {
                ui.with_layout(egui::Layout::centered_and_justified(Direction::TopDown), |ui| {
                    let width = self.latest_resolution.width as usize;
                    let height = self.latest_resolution.height as usize;
                    let pane_size = ui.max_rect();
                    let (scaled_height, scaled_width) = if pane_size.width() > pane_size.height() * 1.3333 {
                        (pane_size.height(), pane_size.height() * 1.3333)
                    } else {
                        (pane_size.width() * 0.75, pane_size.width())
                    };
                    let origin_x = self.display_origin.0;
                    let origin_y = self.display_origin.1;
                    let viewport_rect = egui::Rect::from_min_max(pos2(origin_x as f32 / VRAM_WIDTH as f32, origin_y as f32 / VRAM_HEIGHT as f32), pos2((origin_x  + width - 1) as f32 / VRAM_WIDTH as f32,(origin_y + height - 1) as f32 / VRAM_HEIGHT as f32));
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

fn get_button_state(input_state: &egui::InputState) -> ButtonState {
    ButtonState {
        controller_type: ControllerType::DigitalPad,
        button_x: input_state.key_down(Key::K),
        button_square: input_state.key_down( Key::J),
        button_triangle: input_state.key_down( Key::I),
        button_circle: input_state.key_down( Key::L),
        button_up: input_state.key_down( Key::W),
        button_down: input_state.key_down( Key::S),
        button_left: input_state.key_down( Key::A),
        button_right: input_state.key_down( Key::D),
        button_l1: false,
        button_l2: false,
        button_l3: false,
        button_r1: false,
        button_r2: false,
        button_r3: false,
        button_select: false,
        button_start: input_state.key_down( Key::Enter),

    }
}


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

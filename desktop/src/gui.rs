use std::sync::{Arc, Mutex};

use eframe::{
    egui::{self, Color32, Direction, Key, Layout, Pos2, Rect, TextureId},
    epaint::TextureHandle,
    glow::{self, HasContext, NativeTexture}, egui_glow,
};
use gilrs::{Button, GamepadId, Gilrs};
use psx_emu::{
    controller::{ButtonState, ControllerType},
    gpu::{DrawCall, Resolution},
};

use crate::{ClientMessage, ClientState, EmuMessage};

const VRAM_WIDTH: usize = 1024;
const VRAM_HEIGHT: usize = 512;

pub(crate) fn run_gui(state: ClientState) {
    let native_options = eframe::NativeOptions {
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };

    eframe::run_native(
        "Vaporstation",
        native_options,
        Box::new(|cc| Box::new(VaporstationApp::new(state, cc))),
    );
}

struct VaporstationApp {
    emu_handle: ClientState,
    times: AverageList,
    latest_resolution: Resolution,
    awaiting_gdb: bool,
    latest_pc: u32,
    irq_mask: u32,
    vram_texture: Option<TextureHandle>,
    display_texture: Option<TextureHandle>,
    show_vram_window: bool,
    gdb_connected: bool,
    display_origin: (usize, usize),
    latest_gpu_log: Vec<DrawCall>,
    show_gpu_call_window: bool,
    highlighted_gpu_calls: Vec<usize>,
    last_frame_data: Vec<u8>,
    memory_logging: bool,
    gilrs_instance: Gilrs,
    active_controller_id: Option<GamepadId>,
    show_gamepad_window: bool,
    has_initialized: bool,
    disp_shader_manager: Arc<Mutex<DisplayShaderManager>>,
    last_display_data: Vec<u8>
    //shader_layer: ShaderLayer,
}

impl VaporstationApp {
    fn new(state: ClientState, cc: &eframe::CreationContext<'_>) -> Self {
        let default_resolution = Resolution {
            width: 640,
            height: 480,
        };

        let gl = cc
            .gl
            .as_ref()
            .expect("You need to run eframe with the glow backend");

        Self {
            emu_handle: state,
            times: AverageList::new(),
            latest_resolution: default_resolution,
            awaiting_gdb: false,
            latest_pc: 0,
            irq_mask: 0,
            vram_texture: None,
            show_vram_window: false,
            gdb_connected: false,
            display_origin: (0, 0),
            latest_gpu_log: vec![],
            show_gpu_call_window: false,
            highlighted_gpu_calls: vec![],
            last_frame_data: vec![],
            memory_logging: false,
            display_texture: None,
            gilrs_instance: Gilrs::new().unwrap(),
            active_controller_id: None,
            show_gamepad_window: false,
            has_initialized: false,
            disp_shader_manager: Arc::new(Mutex::new(DisplayShaderManager::new(gl))),
            last_display_data: Vec::new(),
            //shader_layer: ShaderLayer::new(cc.gl.as_ref().unwrap().clone()),
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

    fn get_button_state(&self, input_state: &egui::InputState) -> ButtonState {
        if let Some(gamepad_id) = self.active_controller_id {
            let gamepad = self.gilrs_instance.gamepad(gamepad_id);
            ButtonState {
                controller_type: ControllerType::DigitalPad,
                button_x: gamepad.is_pressed(Button::South),
                button_square: gamepad.is_pressed(Button::West),
                button_triangle: gamepad.is_pressed(Button::North),
                button_circle: gamepad.is_pressed(Button::East),
                button_up: gamepad.is_pressed(Button::DPadUp),
                button_down: gamepad.is_pressed(Button::DPadDown),
                button_left: gamepad.is_pressed(Button::DPadLeft),
                button_right: gamepad.is_pressed(Button::DPadRight),
                button_l1: gamepad.is_pressed(Button::LeftTrigger),
                button_l2: gamepad.is_pressed(Button::LeftTrigger2),
                button_l3: false,
                button_r1: gamepad.is_pressed(Button::RightTrigger),
                button_r2: gamepad.is_pressed(Button::RightTrigger2),
                button_r3: false,
                button_select: gamepad.is_pressed(Button::Select),
                button_start: gamepad.is_pressed(Button::Start),
            }
        } else {
            get_button_state_from_keyboard(input_state)
        }
    }

    fn custom_painting(&mut self, ui: &mut egui::Ui, frame_data: Vec<u8>, frame_width: f32, frame_height: f32, psx_disp_width: i32, psx_disp_height: i32) {
        let (rect, response) =
            ui.allocate_exact_size(egui::Vec2::new(frame_width as f32, frame_height as f32), egui::Sense::drag());

        //self.angle += response.drag_delta().x * 0.01;

        // Clone locals so we can move them into the paint callback:
        //let angle = self.angle;
        let rotating_triangle = self.disp_shader_manager.clone();

        let callback = egui::PaintCallback {
            rect,
            callback: std::sync::Arc::new(egui_glow::CallbackFn::new(move |_info, painter| {
                rotating_triangle.lock().unwrap().paint(painter.gl(), &frame_data, psx_disp_width, psx_disp_height, frame_width as i32, frame_height as i32);
            })),
        };
        ui.painter().add(callback);
    }

}

impl eframe::App for VaporstationApp {
    fn update(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {

        if !self.has_initialized {
            self.emu_handle
                .comm
                .tx
                .send(EmuMessage::RecieveGuiContext(ctx.clone()))
                .unwrap();

            // let shader_code = "#version 330 core
            // out vec4 FragColor;
            
            // void main()
            // {
            //     FragColor = vec4(1.0f, 0.5f, 0.2f, 1.0f);
            // } ".into();

            //self.shader_layer.create_new_shader(shader_code);
            
            self.has_initialized = true;
        }

        // TODO: Fix this. Runs the envent loop enough to grab most of the controller updates
        for _ in 0..16 {
            self.gilrs_instance.next_event();
        }
        let psx_button_state = self.get_button_state(&ctx.input());
        self.emu_handle
            .comm
            .tx
            .send(EmuMessage::UpdateControllers(psx_button_state))
            .unwrap();
        // Process emu messages until empty
        loop {
            match self.emu_handle.comm.rx.try_recv() {
                Ok(msg) => match msg {
                    ClientMessage::FrameReady(vram_frame, frame_time, is_full_color) => {
                        let pixel_data = transform_psx16_to_32(
                            &vram_frame,
                            0,
                            0,
                            VRAM_WIDTH as u32,
                            VRAM_HEIGHT as u32,
                        );

                        self.vram_texture = Some(ctx.load_texture(
                            "VRAM",
                            egui::ColorImage::from_rgba_unmultiplied(
                                [VRAM_WIDTH, VRAM_HEIGHT],
                                &pixel_data,
                            ),
                            egui::TextureOptions::LINEAR,
                        ));

                        let display_data = if is_full_color {
                            transform_psx24_to_32(
                                &vram_frame,
                                self.display_origin.0 as u32,
                                self.display_origin.1 as u32,
                                self.latest_resolution.width,
                                self.latest_resolution.height,
                            )
                        } else {
                            transform_psx16_to_32(
                                &vram_frame,
                                self.display_origin.0 as u32,
                                self.display_origin.1 as u32,
                                self.latest_resolution.width,
                                self.latest_resolution.height,
                            )
                        };

                        self.display_texture = Some(ctx.load_texture(
                            "FRAME",
                            egui::ColorImage::from_rgba_unmultiplied(
                                [
                                    self.latest_resolution.width as usize,
                                    self.latest_resolution.height as usize,
                                ],
                                &display_data,
                            ),
                            egui::TextureOptions::LINEAR,
                        ));

                        self.last_frame_data = pixel_data;
                        self.last_display_data = display_data;
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
                    ClientMessage::LatestIrqMask(irq_mask) => {
                        self.irq_mask = irq_mask;
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
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        frame.quit();
                    }
                });

                ui.menu_button("Settings", |ui| {
                    ui.checkbox(&mut self.show_gamepad_window, "Controller");
                });
                ui.menu_button("Control", |ui| {
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
                ui.menu_button("Debug", |ui| {
                    ui.checkbox(&mut self.show_vram_window, "VRAM Viewer");
                    ui.checkbox(&mut self.show_gpu_call_window, "GPU Call Debugger");
                    if ui
                        .checkbox(&mut self.memory_logging, "Memory Logging")
                        .clicked()
                    {
                        self.emu_handle
                            .comm
                            .tx
                            .send(EmuMessage::SetMemLogging(self.memory_logging))
                            .unwrap();
                    };
                });

                ui.with_layout(Layout::right_to_left(eframe::emath::Align::Center), |ui| {
                    if self.halted() {
                        ui.label(format!("HALTED at {:#X}", self.latest_pc));
                        ui.label(format!("IRQ mask: {:#X}", self.irq_mask));
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
                if let Some(vram) = &self.vram_texture {
                    ui.image(vram.id(), [VRAM_WIDTH as f32, VRAM_HEIGHT as f32]);
                }
            });
        }

        if self.show_gamepad_window {
            egui::Window::new("Settings | Controller").show(ctx, |ui| {
                let current_id = self.active_controller_id;
                let current_gamepad = if let Some(id) = current_id {
                    Some(self.gilrs_instance.gamepad(id))
                } else {
                    None
                };
                egui::ComboBox::from_label("Input Source")
                    .selected_text(format!(
                        "{}",
                        match &current_gamepad {
                            Some(gamepad) => gamepad.name(),
                            _ => "Keyboard",
                        }
                    ))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.active_controller_id, None, "Keyboard");
                        for (id, gamepad) in self.gilrs_instance.gamepads() {
                            let connected_string = if gamepad.is_connected() {
                                ""
                            } else {
                                " DISCONNECTED"
                            };
                            ui.selectable_value(
                                &mut self.active_controller_id,
                                Some(id),
                                format!("{}{}", gamepad.name(), connected_string),
                            );
                        }
                    });
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

                                        self.vram_texture = Some(ctx.load_texture(
                                            "VRAM",
                                            egui::ColorImage::from_rgba_unmultiplied(
                                                [VRAM_WIDTH, VRAM_HEIGHT],
                                                &new_frame,
                                            ),
                                            egui::TextureOptions::LINEAR,
                                        ));
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
            if let Some(_) = &self.display_texture {
                let frame_data_copy = self.last_display_data.clone();
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
                        
                        egui::Frame::canvas(ui.style()).show(ui, |ui| {
                            self.custom_painting(ui, frame_data_copy, scaled_width, scaled_height, self.latest_resolution.width as i32, self.latest_resolution.height as i32);
                        });
                    },
                );
            }
        });
    }
}

fn get_button_state_from_keyboard(input_state: &egui::InputState) -> ButtonState {
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
        button_select: input_state.key_down(Key::Backspace),
        button_start: input_state.key_down(Key::Enter),
    }
}

fn transform_psx16_to_32(
    psx_data: &Vec<u16>,
    origin_x: u32,
    origin_y: u32,
    width: u32,
    height: u32,
) -> Vec<u8> {
    psx_data
        .iter()
        .enumerate()
        .filter(|(i, _v)| {
            (i % VRAM_WIDTH) >= origin_x as usize
                && (i / VRAM_WIDTH) >= origin_y as usize
                && (i % VRAM_WIDTH) < (origin_x + width) as usize
                && (i / VRAM_WIDTH) < (origin_y + height) as usize
        })
        .map(|(_i, p)| ps_pixel_to_gl(p))
        .flatten()
        .collect::<Vec<u8>>()
}

fn transform_psx24_to_32(
    psx_data: &Vec<u16>,
    origin_x: u32,
    origin_y: u32,
    width: u32,
    height: u32,
) -> Vec<u8> {
    psx_data
        .iter()
        .fold(vec![], |mut vec, val| {
            vec.extend(val.to_le_bytes());
            vec
        })
        .iter()
        .enumerate()
        .filter(|(i, _v)| {
            (i % (VRAM_WIDTH * 2)) >= (origin_x * 2) as usize
                && ((i) / (VRAM_WIDTH * 2)) >= origin_y as usize
                && (i % (VRAM_WIDTH * 2)) < ((origin_x * 2) + (width * 3)) as usize
                && ((i) / (VRAM_WIDTH * 2)) < (origin_y + height) as usize
        })
        .map(|(_i, v)| *v)
        .collect::<Vec<u8>>()
}

fn apply_highlights(app: &VaporstationApp, pixel_data: &mut Vec<u8>) {
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
            let tex_max_x = ((points.iter().max_by_key(|v| v.tex_x).unwrap().tex_x - tex_min_x)
                / clut_div)
                + tex_min_x;
            let tex_max_y = points.iter().max_by_key(|v| v.tex_y).unwrap().tex_y;

            println!(
                "Highlighting ({}, {}) -> ({}, {})",
                min_x, min_y, max_x, max_y
            );
            println!(
                "Tex coords ({}, {}) -> ({}, {})",
                tex_min_x, tex_min_y, tex_max_x, tex_max_y
            );
            println!("base x {} base y {}", tex_base_x, tex_base_y);

            for y in min_y..max_y {
                for x in min_x..max_x {
                    let addr = ((y as i32) * 1024 + x as i32) * 3;
                    let current_pixel = pixel_data[addr as usize];
                    let highlight_color = Color32::from_rgba_unmultiplied(155, 0, 0, 155);

                    pixel_data[addr as usize] += highlight_color.r();
                    pixel_data[(addr + 1) as usize] += highlight_color.g();
                    pixel_data[(addr + 2) as usize] += highlight_color.b();
                }
            }

            for y in tex_min_y..tex_max_y {
                for x in tex_min_x..tex_max_x {
                    let addr = (((y + tex_base_y) as i32) * 1024 + (x + tex_base_x) as i32) * 3;
                    let current_pixel = pixel_data[addr as usize];
                    let highlight_color = Color32::from_rgba_unmultiplied(0, 155, 0, 155);

                    pixel_data[addr as usize] += highlight_color.r();
                    pixel_data[(addr + 1) as usize] += highlight_color.g();
                    pixel_data[(addr + 2) as usize] += highlight_color.b();
                }
            }
        }
    }
    
}

///Converts 16 bit psx pixel format to u8u8u8u8
fn ps_pixel_to_gl(pixel_data: &u16) -> [u8; 4] {
    [
        ((pixel_data & 0x1F) * 8) as u8,
        (((pixel_data >> 5) & 0x1F) * 8) as u8,
        (((pixel_data >> 10) & 0x1F) * 8) as u8,
        255
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



const DEFAULT_FRAGMENT_SHADER: &str = r#"
#version 330

#if __VERSION__ >= 130
#define COMPAT_VARYING in
#define COMPAT_TEXTURE texture
out vec4 FragColor;
#else
#define COMPAT_VARYING varying
#define FragColor gl_FragColor
#define COMPAT_TEXTURE texture2D
#endif

#ifdef GL_ES
#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif
#define COMPAT_PRECISION mediump
#else
#define COMPAT_PRECISION
#endif

uniform COMPAT_PRECISION int FrameDirection;
uniform COMPAT_PRECISION int FrameCount;
uniform COMPAT_PRECISION vec2 OutputSize;
uniform COMPAT_PRECISION vec2 TextureSize;
uniform COMPAT_PRECISION vec2 InputSize;
uniform sampler2D Texture;
COMPAT_VARYING vec4 TEX0;

// compatibility #defines
#define Source Texture
#define vTexCoord TEX0.xy

#define SourceSize vec4(TextureSize, 1.0 / TextureSize) //either TextureSize or InputSize
#define OutSize vec4(OutputSize, 1.0 / OutputSize)

#ifdef PARAMETER_UNIFORM
uniform COMPAT_PRECISION float SCANLINE_BASE_BRIGHTNESS;
uniform COMPAT_PRECISION float SCANLINE_SINE_COMP_A;
uniform COMPAT_PRECISION float SCANLINE_SINE_COMP_B;
uniform COMPAT_PRECISION float warpX;
uniform COMPAT_PRECISION float warpY;
uniform COMPAT_PRECISION float corner_round;
uniform COMPAT_PRECISION float cgwg;
uniform COMPAT_PRECISION float crt_gamma;
uniform COMPAT_PRECISION float monitor_gamma;
uniform COMPAT_PRECISION float boost;
#else
#define SCANLINE_BASE_BRIGHTNESS 0.95
#define SCANLINE_SINE_COMP_A 0.0
#define SCANLINE_SINE_COMP_B 0.40
#define warpX 0.031
#define warpY 0.041
#define corner_round 0.030
#define cgwg 0.4
#define crt_gamma 2.2
#define monitor_gamma 2.4
#define boost 0.00
#endif

vec4 scanline(vec2 coord, vec4 frame)
{

	vec2 omega = vec2(3.1415 * OutputSize.x, 2.0 * 3.1415 * TextureSize.y);
	vec2 sine_comp = vec2(SCANLINE_SINE_COMP_A, SCANLINE_SINE_COMP_B);
	vec3 res = frame.xyz;
	
	vec3 scanline = res * (SCANLINE_BASE_BRIGHTNESS + dot(sine_comp * sin(coord * omega), vec2(1.0, 1.0)));

	return vec4(scanline.x, scanline.y, scanline.z, 1.0);


}

#ifdef CURVATURE
// Distortion of scanlines, and end of screen alpha.
vec2 Warp(vec2 pos)
{
    pos  = pos*2.0-1.0;    
    pos *= vec2(1.0 + (pos.y*pos.y)*warpX, 1.0 + (pos.x*pos.x)*warpY);
    
    return pos*0.5 + 0.5;
}

float corner(vec2 coord)
{
                coord *= TextureSize / InputSize;
                coord = (coord - vec2(0.5)) * 1.0 + vec2(0.5);
                coord = min(coord, vec2(1.0)-coord) * vec2(1.0, InputSize.y/InputSize.x);
                vec2 cdist = vec2(corner_round);
                coord = (cdist - min(coord,cdist));
                float dist = sqrt(dot(coord,coord));
                return clamp((cdist.x-dist)*300.0,0.0, 1.0);
}  
#endif

// mask calculation
	// cgwg mask.
	vec4 Mask(vec2 pos)
	{
	  vec3 mask = vec3(1.0);
	{
      float mf = floor(mod(pos.x,2.0));
      float mc = 1.0 - cgwg;	
      if (mf == 0.0) { mask.g = mc; }
      else { mask.r = mc; mask.b = mc; };
   }  
		return vec4(mask, 1.0);
	}


void main()
{
#ifdef CURVATURE
	vec2 pos = Warp(TEX0.xy*(TextureSize.xy/InputSize.xy))*(InputSize.xy/TextureSize.xy);
#else
	vec2 pos = TEX0.xy;
#endif

//borrowed from CRT-Pi
		vec2 OGL2Pos = pos * TextureSize;
		vec2 pC4 = floor(OGL2Pos) + 0.5;
		vec2 coord = pC4 / TextureSize;
		vec2 deltas = OGL2Pos - pC4;
		vec2 signs = sign(deltas);
		deltas.x *= 2.0;
		deltas = deltas * deltas;
		deltas.y = deltas.y * deltas.y;
		deltas.x *= 0.5;
		deltas.y *= 8.0;
		deltas /= TextureSize;
		deltas *= signs;
		vec2 tc = coord + deltas;


// mask effects look bad unless applied in linear gamma space
	vec4 in_gamma = vec4(crt_gamma, crt_gamma, crt_gamma, 1.0);
	vec4 out_gamma = vec4(1.0 / monitor_gamma, 1.0 / monitor_gamma, 1.0 / monitor_gamma, 1.0);
	
	vec4 res = COMPAT_TEXTURE(Texture, tc);
	
	res=pow(res,in_gamma);

	// apply the mask; looks bad with vert scanlines so make them mutually exclusive
	res *= Mask(gl_FragCoord.xy * 1.0001);


#if defined CURVATURE && defined GL_ES
    // hacky clamp fix for GLES
    vec2 bordertest = (tc);
    if ( bordertest.x > 0.0001 && bordertest.x < 0.9999 && bordertest.y > 0.0001 && bordertest.y < 0.9999)
        res = res;
    else
        res = vec4(0.,0.,0.,0.);
#endif

    // re-apply the gamma curve for the mask path
    vec4 color = pow(scanline(pos, res), out_gamma);
    color+=boost*color;
    FragColor = color;

} 
"#;

const DEFAULT_VERTEX_SHADER: &str = r#"
#version 330

const vec3 verts[3] = vec3[3](
    vec3(-1.0, -1.0, 0.0),
    vec3(3.0, -1.0, 0.0),
    vec3(-1.0, 3.0, 0.0)
);

out vec2 TexCoord;

void main()
{
    gl_Position = vec4(verts[gl_VertexID], 1.0);
    TexCoord = vec2((0.5 - 0.00833) * gl_Position.x + 0.5, (0.5 - 0.00625) * -gl_Position.y + 0.5);
}
"#;




struct DisplayShaderManager {
    program: glow::Program,
    vertex_array: glow::VertexArray,
    frame_count: usize,
}

impl DisplayShaderManager {
    fn new(gl: &glow::Context) -> Self {
        use glow::HasContext as _;

        unsafe {
            let program = gl.create_program().expect("Cannot create program");

            let (vertex_shader_source, fragment_shader_source) = (
                DEFAULT_VERTEX_SHADER, DEFAULT_FRAGMENT_SHADER
            );

            let shader_sources = [
                (glow::VERTEX_SHADER, vertex_shader_source),
                (glow::FRAGMENT_SHADER, fragment_shader_source),
            ];

            let shaders: Vec<_> = shader_sources
                .iter()
                .map(|(shader_type, shader_source)| {
                    let shader = gl
                        .create_shader(*shader_type)
                        .expect("Cannot create shader");
                    gl.shader_source(shader, &shader_source);
                    gl.compile_shader(shader);
                    assert!(
                        gl.get_shader_compile_status(shader),
                        "Failed to compile {shader_type}: {}",
                        gl.get_shader_info_log(shader)
                    );
                    gl.attach_shader(program, shader);
                    shader
                })
                .collect();

            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                panic!("{}", gl.get_program_info_log(program));
            }

            for shader in shaders {
                gl.detach_shader(program, shader);
                gl.delete_shader(shader);
            }

            let vertex_array = gl
                .create_vertex_array()
                .expect("Cannot create vertex array");

            Self {
                program,
                vertex_array,
                frame_count: 0,
            }
        }
    }

    fn destroy(&self, gl: &glow::Context) {
        use glow::HasContext as _;
        unsafe {
            gl.delete_program(self.program);
            gl.delete_vertex_array(self.vertex_array);
        }
    }

    //uniform COMPAT_PRECISION int FrameDirection;
    //uniform COMPAT_PRECISION int FrameCount;
    //uniform COMPAT_PRECISION vec2 OutputSize;
    //uniform COMPAT_PRECISION vec2 TextureSize;
    //uniform COMPAT_PRECISION vec2 InputSize;

    fn paint(&mut self, gl: &glow::Context, image_data: &[u8], psx_frame_width: i32, psx_frame_height: i32, ui_width: i32, ui_height: i32) {
        use glow::HasContext as _;
        self.frame_count += 1;
        unsafe {
            gl.use_program(Some(self.program));

            // Display texture
            let disp_tex = gl.create_texture().unwrap();
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(disp_tex));
            gl.tex_image_2d(glow::TEXTURE_2D, 0.into(), glow::RGBA as i32, psx_frame_width, psx_frame_height, 0, glow::RGBA, glow::UNSIGNED_BYTE, Some(image_data));
            
            gl.uniform_1_i32(gl.get_uniform_location(self.program, "FrameDirection").as_ref(), 1);
            gl.uniform_1_i32(gl.get_uniform_location(self.program, "FrameCount").as_ref(), self.frame_count as i32);
            gl.uniform_2_f32(gl.get_uniform_location(self.program, "OutputSize").as_ref(), ui_width as f32, ui_height as f32);
            gl.uniform_2_f32(gl.get_uniform_location(self.program, "TextureSize").as_ref(), ui_width as f32, ui_height as f32);
            gl.uniform_2_f32(gl.get_uniform_location(self.program, "InputSize").as_ref(), ui_width as f32, ui_height as f32);
            
            
            gl.generate_mipmap(glow::TEXTURE_2D);
            gl.bind_vertex_array(Some(self.vertex_array));
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
            gl.delete_texture(disp_tex);
        }
    }
}
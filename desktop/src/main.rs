use byteorder::{ByteOrder, LittleEndian};
use disc::*;
use eframe::epi::RepaintSignal;
use gdbstub::{DisconnectReason, GdbStub, GdbStubError};
use getopts::Matches;
use getopts::Options;
use psx_emu::controller::ButtonState;
use psx_emu::gpu::DrawCall;
use psx_emu::gpu::Resolution;
use psx_emu::PSXEmu;
use psx_emu::toggle_memory_logging;
use std::env;
use std::fs;
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::channel;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::thread;
use std::thread::JoinHandle;
use std::time::SystemTime;
use simple_logger::SimpleLogger;

mod disc;
mod gdb;
mod gui;

const DEFAULT_GDB_PORT: u16 = 4444;
const DEFAULT_BIOS_PATH: &str = "SCPH1001.BIN";
const START_HALTED: bool = false;
const START_FRAME_LIMITED: bool = true;

#[allow(dead_code)]
struct ClientState {
    comm: ClientComms,
    emu_thread: JoinHandle<()>,
    halted: bool,
    frame_limited: bool,
}

struct EmuState {
    emu: PSXEmu,
    comm: EmuComms,
    halted: bool,
    current_resolution: Resolution,
    debugging: bool,
    last_frame_time: SystemTime,
    waiting_for_client: bool,
    redraw_signal: Option<Arc<dyn RepaintSignal>>,
    frame_limited: bool,
    current_origin: (usize, usize),
    latest_draw_log: Vec<DrawCall>
}

fn main() {
    let mut headless = false;
    let args: Vec<String> = env::args().collect();

    let mut opts = Options::new();
    opts.optopt("b", "bios", "BIOS file path", "FILE");
    opts.optopt("c", "cue", "CUE file path", "FILE");
    opts.optopt("e", "exe", "EXE file path", "FILE");

    opts.optflag("l", "log", "Enable logging");
    opts.optflag("h", "headless", "Run without GUI");
    opts.optflag("g", "gdb", "Start GDB server on port 4444");

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => {
            panic!("{}", f.to_string())
        }
    };

    let (emu_sender, client_receiver) = channel();
    let (client_sender, emu_receiver) = channel();

    let emu_comm = EmuComms {
        rx: emu_receiver,
        tx: emu_sender,
    };

    let client_comm = ClientComms {
        rx: client_receiver,
        tx: client_sender,
    };

    let emu_thread = start_emu_thread(matches, emu_comm);

    let state = ClientState {
        emu_thread,
        comm: client_comm,
        halted: START_HALTED,
        frame_limited: START_FRAME_LIMITED,
    };

    
    if !headless {
        gui::run_gui(state);
    } else {
        run_headless(state);
    }
    
}

fn run_headless(state: ClientState) {
    state.comm.tx.send(EmuMessage::Continue).unwrap();
    loop {
        match state.comm.rx.try_recv() {
            _ => ()
        };
    }
}

fn wait_for_gdb_connection(port: u16) -> std::io::Result<TcpStream> {
    let sockaddr = format!("localhost:{}", port);
    eprintln!("Waiting for a GDB connection on {:?}...", sockaddr);
    let sock = TcpListener::bind(sockaddr)?;
    let (stream, addr) = sock.accept()?;

    // Blocks until a GDB client connects via TCP.
    // i.e: Running `target remote localhost:<port>` from the GDB prompt.

    eprintln!("Debugger connected from {}", addr);
    Ok(stream)
}

fn create_emu(matches: Matches, emu_comm: EmuComms) -> EmuState {
    let mut headless = false;
    let bios_path = if let Some(new_path) = matches.opt_str("b") {
        println!("Using alternate bios file: {}", new_path);
        new_path
    } else {
        println!("Using defualt bios file: {}", DEFAULT_BIOS_PATH);
        DEFAULT_BIOS_PATH.to_string()
    };

    let bios_data = match fs::read(&bios_path) {
        Ok(data) => data,
        _ => {
            panic!("Unable to read bios file!");
        }
    };

    let mut emu = PSXEmu::new(bios_data);
    emu.reset();

    if matches.opt_present("l") {
        SimpleLogger::new().init().unwrap();
    }

    if matches.opt_present("h") {
        headless = true;
    }

   

    //Loads entire disc into memory (Don't worry about it)
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

    EmuState {
        emu: emu,
        comm: emu_comm,
        halted: START_HALTED,
        current_resolution: Resolution {
            width: 640,
            height: 480,
        },
        debugging: matches.opt_present("g"),
        last_frame_time: SystemTime::now(),
        waiting_for_client: false,
        redraw_signal: None,
        frame_limited: START_FRAME_LIMITED,
        current_origin: (0, 0),
        latest_draw_log: vec!(),
    }
}

#[allow(dead_code)]
enum EmuMessage {
    Halt,
    Continue,
    AddBreakpoint(u32),
    RemoveBreakpoint(u32),
    Kill,
    StepCPU,
    UpdateControllers(ButtonState),
    Reset,
    StartFrame,
    RequestDrawCallback(Arc<dyn RepaintSignal>),
    SetFrameLimiter(bool),
    ClearGpuLog,
    SetMemLogging(bool),
}

enum ClientMessage {
    FrameReady(Vec<u16>, u128, bool),
    ResolutionChanged(Resolution),
    AwaitingGDBClient,
    GDBClientConnected,
    LatestPC(u32),
    Halted,
    Continuing,
    DisplayOriginChanged((usize, usize)),
    LatestGPULog(Vec<DrawCall>),
}

struct EmuComms {
    rx: Receiver<EmuMessage>,
    tx: Sender<ClientMessage>,
}

struct ClientComms {
    rx: Receiver<ClientMessage>,
    tx: Sender<EmuMessage>,
}

fn start_emu_thread(
    matches: Matches,
    emu_comm: EmuComms
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut state = create_emu(matches, emu_comm);
        let mut debugger = if state.debugging {
            state.comm.tx.send(ClientMessage::AwaitingGDBClient).unwrap();
            let gdb_conn = wait_for_gdb_connection(DEFAULT_GDB_PORT).unwrap();
            state.comm.tx.send(ClientMessage::GDBClientConnected).unwrap();
            Some(GdbStub::<EmuState, TcpStream>::new(gdb_conn))
        } else {
            None
        };

        if let Some(dbg) = &mut debugger {
            match dbg.run(&mut state) {
                Ok(disconnect_reason) => match disconnect_reason {
                    DisconnectReason::Disconnect => println!("Client disconnected!"),
                    DisconnectReason::TargetHalted => println!("Target halted!"),
                    DisconnectReason::Kill => println!("GDB client sent a kill command!"),
                },
                Err(GdbStubError::TargetError(e)) => {
                    println!("Target raised a fatal error: {:?}", e);
                }
                Err(e) => println!("Something else happened {}", e.to_string()),
            }
        } else {
            loop {
                if let Err(e) = emu_loop_step(&mut state) {
                    println!("ERROR | EmuThread: Encountered error: {:?}, exiting...", e);
                    break;
                }
            }
        }
    })
}

#[derive(Debug)]
enum EmuThreadError {
    ClientDied,
    Killed,
}

fn emu_loop_step(state: &mut EmuState) -> Result<(), EmuThreadError> {
    // Handle incoming messages
    if let Ok(msg) = state.comm.rx.try_recv() {
        match msg {
            EmuMessage::Halt => {
                state.halted = true;
                state.comm.tx.send(ClientMessage::LatestPC(state.emu.pc())).unwrap();
                state.comm.tx.send(ClientMessage::LatestGPULog(state.latest_draw_log.clone())).unwrap();
            },
            EmuMessage::Continue => {
                state.halted = false;
                state.emu.clear_halt();
            }
            EmuMessage::AddBreakpoint(addr) => state.emu.add_sw_breakpoint(addr),
            EmuMessage::RemoveBreakpoint(addr) => state.emu.remove_sw_breakpoint(addr),
            EmuMessage::Kill => return Err(EmuThreadError::Killed),
            EmuMessage::StepCPU => state.emu.run_cpu_cycle(), // Warning! Doing this too many times will desync the gpu
            EmuMessage::UpdateControllers(button_state) => {
                state.emu.update_controller_state(button_state)
            }
            EmuMessage::Reset => state.emu.reset(),
            EmuMessage::StartFrame => state.waiting_for_client = false,
            EmuMessage::RequestDrawCallback(signal) => state.redraw_signal = Some(signal),
            EmuMessage::SetFrameLimiter(val) => state.frame_limited = val,
            EmuMessage::ClearGpuLog => state.emu.clear_gpu_call_log(),
            EmuMessage::SetMemLogging(enabled) => toggle_memory_logging(enabled),
        }
    }

    if state.emu.halt_requested() {
        state.halted = true;
    }

    if !state.halted && !state.waiting_for_client {
        state.emu.step_cycle();

        if state.emu.frame_ready() {
            //Check for any viewport resolution changes
            if state.emu.display_resolution() != state.current_resolution {
                state.current_resolution = state.emu.display_resolution();
                state.comm.tx.send(ClientMessage::ResolutionChanged(
                    state.current_resolution.clone(),
                )).unwrap();
            };

            if state.emu.display_origin() != state.current_origin {
                state.current_origin = state.emu.display_origin();
                state.comm.tx.send(ClientMessage::DisplayOriginChanged(state.current_origin)).unwrap();
            }

            //Calculate frame time delta
            let mut frame_time = SystemTime::now()
                .duration_since(state.last_frame_time)
                .expect("Error getting frame duration")
                .as_millis();
    
            let frame = state.emu.get_vram().clone();
            let depth_full = state.emu.is_full_color_depth();
            // Wait for frame limiter time to pass
            while state.frame_limited && frame_time < 17 {
                frame_time = SystemTime::now()
                .duration_since(state.last_frame_time)
                .expect("Error getting frame duration")
                .as_millis();
            }
    
            // Send the new frame over to the gui thread
            if let Err(_) = state
                .comm
                .tx
                .send(ClientMessage::FrameReady(frame, frame_time, depth_full))
            {
                //The other side hung up, so lets end the emu thread
                return Err(EmuThreadError::ClientDied);
            };
            // Request redraw
            if let Some(redraw_signal) = &state.redraw_signal {
                redraw_signal.request_repaint();
            }

            state.latest_draw_log = state.emu.take_gpu_call_log();

            //state.waiting_for_client = true; // Wait until next frame is ready
            state.last_frame_time = SystemTime::now();
        };
    } else {
        //thread::sleep(Duration::from_millis(1));


    }

   
    Ok(())
}

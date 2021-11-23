use byteorder::{ByteOrder, LittleEndian};
use disc::*;
use gdbstub::{DisconnectReason, GdbStub, GdbStubError};
use getopts::Options;
use psx_emu::controller::ButtonState;
use psx_emu::gpu::Resolution;
use psx_emu::PSXEmu;
use std::env;
use std::fs;
use std::net::{TcpListener, TcpStream};
use std::path::Path;
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
mod support;

const DEFAULT_GDB_PORT: u16 = 4444;
const DEFAULT_BIOS_PATH: &str = "SCPH1001.BIN";
const START_HALTED: bool = false;

#[allow(dead_code)]
struct ClientState {
    comm: ClientComms,
    emu_thread: JoinHandle<()>,
    halted: bool,
}

struct EmuState {
    emu: PSXEmu,
    comm: EmuComms,
    halted: bool,
    current_resolution: Resolution,
    debugging: bool,
    last_frame_time: SystemTime,
    waiting_for_client: bool,
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
            println!("Unable to read bios file!");
            return;
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

    let emu_state = EmuState {
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
    };

    let emu_thread = start_emu_thread(emu_state);

    let state = ClientState {
        emu_thread,
        comm: client_comm,
        halted: START_HALTED,
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
            Ok(ClientMessage::FrameReady(_, _)) => {state.comm.tx.send(EmuMessage::StartFrame).unwrap();}, // Drop the frame, but tell the emu to keep going
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
}

enum ClientMessage {
    FrameReady(Vec<u16>, u128),
    ResolutionChanged(Resolution),
    AwaitingGDBClient,
    GDBClientConnected,
    LatestPC(u32),
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
    mut state: EmuState,
) -> JoinHandle<()> {
    thread::spawn(move || {

        let mut debugger = if state.debugging {
            state.comm.tx.send(ClientMessage::AwaitingGDBClient).unwrap();
            let gdb_conn = wait_for_gdb_connection(DEFAULT_GDB_PORT).unwrap();
            state.comm.tx.send(ClientMessage::GDBClientConnected).unwrap();
            state.halted = false;
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
                state.comm.tx.send(ClientMessage::LatestPC(state.emu.pc()));
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

            //Calculate frame time delta
            let frame_time = SystemTime::now()
                .duration_since(state.last_frame_time)
                .expect("Error getting frame duration")
                .as_millis();
    
            let frame = state.emu.get_vram().clone();
    
            // Send the new frame over to the gui thread
            if let Err(_) = state
                .comm
                .tx
                .send(ClientMessage::FrameReady(frame, frame_time))
            {
                //The other side hung up, so lets end the emu thread
                return Err(EmuThreadError::ClientDied);
            };
            state.waiting_for_client = true; // Wait until next frame is ready
            state.last_frame_time = SystemTime::now();
        };
    } else {
        //thread::sleep(Duration::from_millis(1));
    }

   
    Ok(())
}

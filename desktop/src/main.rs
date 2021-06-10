use disc::*;
use gdbstub::{DisconnectReason, GdbStub, GdbStubError};
use getopts::Options;
use psx_emu::PSXEmu;
use psx_emu::gpu::Resolution;
use byteorder::{ByteOrder, LittleEndian};
use psx_emu::controller::ButtonState;
use std::fs;
use std::env;
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::Mutex;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::sync::mpsc::channel;
use std::thread;
use std::thread::JoinHandle;

mod disc;
mod support;
mod gdb;
mod gui;




const DEFAULT_GDB_PORT: u16 = 4444;
const DEFAULT_BIOS_PATH: &str = "SCPH1001.BIN";

struct EmuState {
    comm: ClientComms,
    emu_thread: JoinHandle<()>,
    halted: bool,
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
            panic!(f.to_string())
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
        //SCPH1001.BIN openbios.bin
        Ok(data) => data,
        _ => {
            println!("Unable to read bios file!");
            return;
        }
    };

    let mut emu = PSXEmu::new(bios_data);
    emu.reset();

    // if matches.opt_present("l") {
    //     state.logging = true;
    //     state.emu.r3000.log = true;
    // }

    if matches.opt_present("h") {
        headless = true;
    }



    // let mut debugger = if matches.opt_present("g") {
    //     let gdb_conn = wait_for_gdb_connection(DEFAULT_GDB_PORT).unwrap();
    //     Some(GdbStub::<EmuState, TcpStream>::new(gdb_conn))
    // } else {
    //     None
    // };

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
        emu
            .load_executable(destination, entrypoint, init_sp, &exe_data);
    }

    let (emu_sender, client_receiver) = channel();
    let (client_sender, emu_receiver) = channel();
    let latest_frame = Mutex::new(Vec::<u16>::new());

    let emu_comm = EmuComms {
        rx: emu_receiver,
        tx: emu_sender,
    };

    let client_comm = ClientComms {
        rx: client_receiver,
        tx: client_sender,
    };

    let emu_thread = start_emu_thread(emu, emu_comm);

    let mut state = EmuState {
        emu_thread,
        comm: client_comm,
        halted: false,
    };

    // if let Some(dbg) = &mut debugger {
    //     match dbg.run(&mut state) {
    //         Ok(disconnect_reason) => match disconnect_reason {
    //             DisconnectReason::Disconnect => {
    //                 state.comm.tx.send(EmuMessage::Kill);
    //             },
    //             DisconnectReason::TargetHalted => println!("Target halted!"),
    //             DisconnectReason::Kill => println!("GDB client sent a kill command!"),
    //         },
    //         Err(GdbStubError::TargetError(e)) => {
    //             println!("Target raised a fatal error: {:?}", e);
    //         },
    //         Err(e) => println!("Something else happened {}", e.to_string())
    //     }
    // } else {
    {
        if !headless {
            gui::run_gui(state);
        } else {
            run_headless(state);
        }
    }
}

fn run_headless(mut state: EmuState) {
    state.comm.tx.send(EmuMessage::Continue);
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

enum EmuMessage {
    Halt,
    Continue,
    AddBreakpoint(u32),
    RemoveBreakpoint(u32),
    Kill,
    StepCPU,
    UpdateControllers(ButtonState),
    Reset,
}

enum ClientMessage {
    FrameReady(Vec<u16>),
    ResolutionChanged(Resolution),
}

struct EmuComms {
    rx: Receiver<EmuMessage>,
    tx: Sender<ClientMessage>,}

struct ClientComms {
    rx: Receiver<ClientMessage>,
    tx: Sender<EmuMessage>,
}

fn start_emu_thread(mut emu: PSXEmu, comms: EmuComms) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut halted = false;
        let mut current_resolution = Resolution {width: 640, height: 480};

        loop {
            // Handle incoming messages
            if let Ok(msg) = comms.rx.try_recv() {
                match msg {
                    EmuMessage::Halt => halted = true,
                    EmuMessage::Continue => {
                        halted = false;
                        emu.clear_halt();
                    },
                    EmuMessage::AddBreakpoint(addr) => emu.add_sw_breakpoint(addr),
                    EmuMessage::RemoveBreakpoint(addr) => emu.remove_sw_breakpoint(addr),
                    EmuMessage::Kill => break,
                    EmuMessage::StepCPU => emu.run_cpu_cycle(), // Warning! Doing this too many times will desync the gpu
                    EmuMessage::UpdateControllers(state) => emu.update_controller_state(state),
                    EmuMessage::Reset => emu.reset(),
                }
            }

            if emu.halt_requested() {
                halted = true;
            }

            if !halted {
                emu.step_cycle();
            }

            if emu.frame_ready() {

                //Check for any viewport resolution changes
                if emu.display_resolution() != current_resolution {
                    current_resolution = emu.display_resolution();
                    comms.tx.send(ClientMessage::ResolutionChanged(current_resolution.clone()));
                }

                // Send the new frame over to the gui thread
                if let Err(_) = comms.tx.send(ClientMessage::FrameReady(emu.get_vram().clone())) {
                    //The other side hung up, so lets end the emu thread
                    break;
                }
            }

        }
    })
}

#[cfg(test)]
mod pixel_tests {
    use super::*;
    //#[test]
    // fn test_ps_pixel_to_gl() {
    //     assert_eq!(ps_pixel_to_gl(&0xFFFF), vec![0xFF, 0xFF, 0xFF]);
    // }
}

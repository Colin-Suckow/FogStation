use std::fs;
use psx_emu::PSXEmu;
fn main() { 
    let bios_data = match fs::read("SCPH1001.BIN") {
        Ok(data) => data,
        _ => {
            println!("Unable to read bios file. Make sure there is a file named SCPH1001.BIN in the same directory.");
            return;
        }
    };

    let mut emu = PSXEmu::new(bios_data);
    emu.reset();

    loop {
        emu.step_instruction();
    }
}

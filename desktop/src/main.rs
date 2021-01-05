use psx_emu::PSXEmu;
use std::fs;

use imgui::*;

mod support;

fn main() {
    let bios_data = match fs::read("MemoryTransfer16BPP.bin") {
        Ok(data) => data,
        _ => {
            println!("Unable to read bios file. Make sure there is a file named SCPH1001.BIN in the same directory.");
            return;
        }
    };

    let mut emu = PSXEmu::new(bios_data);
    emu.reset();

    // let system = support::init("psx-emu");
    // system.main_loop(move |_, ui| {
    //     Window::new(im_str!("Registers"))
    //         .size([300.0, 110.0], Condition::FirstUseEver)
    //         .build(ui, || {
    //             ui.text(format!("PC: {:#X}", &emu.r3000.pc));
    //             ui.text(im_str!("こんにちは世界！"));
    //             ui.text(im_str!("This...is...imgui-rs!"));
    //             ui.separator();
    //             let mouse_pos = ui.io().mouse_pos;
    //             ui.text(format!(
    //                 "Mouse Position: ({:.1},{:.1})",
    //                 mouse_pos[0], mouse_pos[1]
    //             ));
    //         });
    // });

    loop {
        emu.step_instruction();
    }
}

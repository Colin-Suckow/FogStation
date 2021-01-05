use psx_emu::PSXEmu;
use std::fs;

use imgui::*;

use std::time::{Duration, Instant};

mod support;

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

    // loop {
    //     emu.step_instruction();
    // }

    let start = Instant::now();
    for _ in 0..100000 {
        emu.step_instruction();
    }
    let end_time = start.elapsed();
    let elapsed_seconds = end_time.as_micros() as f64 * 0.000001;
    println!("Frequency {:.2}mhz", (100000.0 / elapsed_seconds) / 1000000.0);

    // let system = support::init("psx-emu");
    // system.main_loop(move |_, ui| {
    //     //for _ in 0..564480 {
    //         emu.step_instruction();
    //     //}
    //     Window::new(im_str!("Registers"))
    //         .size([300.0, 110.0], Condition::FirstUseEver)
    //         .build(ui, || {
    //             ui.text(format!("PC: {:#X}", emu.r3000.pc.clone()));
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
}

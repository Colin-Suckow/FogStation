mod bios;
mod bus;
mod cpu;

use std::rc::Rc;

use bios::Bios;
use bus::MainBus;
use cpu::R3000;

pub struct PSXEmu {
    main_bus: Rc<MainBus>,
    r3000: R3000,
}

impl PSXEmu {
    pub fn new(bios: Vec<u8>) -> PSXEmu {
        let bios = Bios::new(bios);
        let bus = Rc::new(MainBus::new(bios));
        let r3000 = R3000::new(bus.clone());
        PSXEmu {
            main_bus: bus,
            r3000: r3000,
        }
    }
    
    pub fn reset(&mut self) {
        self.r3000.reset();
    }
}

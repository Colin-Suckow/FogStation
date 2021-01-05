pub struct Gpu {
    vram: Vec<u8>,
    status_reg: u32,
    pixel_count: u32,
    enabled: bool,
}

impl Gpu {
    pub fn new() -> Gpu {
        Gpu {
            vram: vec![0; 1_048_576],
            status_reg: 0,
            pixel_count: 0,
            enabled: false,
        }
    }

    pub fn read_status_register(&self) -> u32 {
        println!("Status read!");
        self.status_reg
    }

    pub fn send_gp0_command(&mut self, command: u32) {
        match command.gp0_header() {
            _ => ()//panic!("unknown gp0 command {:#X}!", command.gp0_header())
        }
    }

    pub fn send_gp1_command(&mut self, command: u32) {
        match command.command() {
            0x0 => {
                //Reset GPU
                self.enabled = false;
                self.status_reg = 0;
                self.pixel_count = 0;
                self.vram = vec![0; 1_000_000];
            }

            0x6 => {
                //Horizontal Display Range
            }
            _ => ()//println!("Unknown gp1 command {:#X} parameter {}!", command.command(), command.parameter())
        }
    }

    pub fn step_cycle(&mut self) {
        if self.enabled {
            self.pixel_count += 1;
        }
    }
}

//Helper trait + impl
trait Command {
    fn gp0_header(&self) -> u8;
    fn command(&self) -> u8;
    fn parameter(&self) -> u32;
}

impl Command for u32 {
    fn gp0_header(&self) -> u8 {
        ((self.clone() >> 28) & 0x7) as u8
    }

    fn command(&self) -> u8 {
        ((self.clone() >> 23) & 0xFF) as u8
    }

    fn parameter(&self) -> u32 {
        (self.clone() & 0x7FFFFF)
    }
}
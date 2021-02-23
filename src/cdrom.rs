use std::collections::VecDeque;
pub struct CDDrive {
    parameter_queue: VecDeque<u8>,
    data_queue: VecDeque<u8>,
    response_queue: VecDeque<u8>,

    status_index: u8,
}

impl CDDrive {
    pub fn new() -> Self {
        Self {
            parameter_queue: VecDeque::new(),
            data_queue: VecDeque::new(),
            response_queue: VecDeque::new(),
            status_index: 0,
        }
    }

    pub fn write_byte(&mut self, addr: u32, val: u8) {
        match addr {
            0x1F801800 => self.status_index = val & 0x3, //Status
            0x1F801803 => {
                match self.status_index {
                    0 => panic!("CD: 0x1F801803 write byte unknown index 0"),
                    1 => self.write_interrupt_flag_register(val),
                    2 => panic!("CD: 0x1F801803 write byte unknown index 2"),
                    3 => panic!("CD: Cannot read Interrupt Flag Register in write command"),
                    _ => unreachable!()
                }
            }
            _ => panic!("CD: Tried to write unknown byte. Address: {:#X} Value: {:#X} Index: {}", addr, val, self.status_index)
        }
    }

    pub fn read_byte(&self, addr: u32) -> u8 {
        match addr {
            0x1F801800 => self.get_status_register(),
            _ => panic!("CD: Tried to read unknown byte. Address: {:#X} Index: {}", addr, self.status_index),
        }
    }

    fn get_status_register(&self) -> u8 {
        let mut status: u8 = 0;
        //0-1 index
        status |= self.status_index;
        //3 prmempt
        status |= (self.parameter_queue.is_empty() as u8) << 3;
        //4 prmrdy
        status |= (!(self.parameter_queue.len() >= 16) as u8) << 4;
        //5 RSLRRDY
        status |= (!self.response_queue.is_empty() as u8) << 5;
        //6 DRQSTS
        status |= (!self.data_queue.is_empty() as u8) << 6;
        // 7 BUSYSTS
        //TODO when I find out what it means to be busy

        status
    }

    fn write_interrupt_flag_register(&mut self, val: u8) {
        todo!();
    }

    fn read_interrupt_flag_register(&mut self) -> u8 {
        todo!();
    }
}
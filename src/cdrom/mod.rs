use std::collections::VecDeque;
use bit_field::BitField;
use commands::*;

mod commands;

pub(super) enum IntCause {
    INT1,
    INT2,
    INT3,
    INT4,
    INT5,
    INT6,
    INT7,
    INT8,
    INT10h,
}

pub(super) struct PendingResponse {
    cause: IntCause,
    response: Vec<u8>,
    execution_cycles: u64,
}

pub struct CDDrive {
    cycle_counter: u64,
    pending_responses: VecDeque<PendingResponse>,

    parameter_queue: VecDeque<u8>,
    data_queue: VecDeque<u8>,
    response_queue: VecDeque<u8>,

    status_index: u8,

    reg_interrupt_flag: u8,
    reg_interrupt_enable: u8,

    //Probably useless registers
    reg_sound_map_data_out: u8,
}

impl CDDrive {
    pub fn new() -> Self {
        Self {
            cycle_counter: 0,
            pending_responses: VecDeque::new(),
            parameter_queue: VecDeque::new(),
            data_queue: VecDeque::new(),
            response_queue: VecDeque::new(),
            status_index: 0,

            reg_interrupt_flag: 0,
            reg_interrupt_enable: 0,
            
            
            //Probably useless registers
            reg_sound_map_data_out: 0,
        }
    }

    pub fn step_cycle(&mut self) {
        if self.cycle_counter > 0 {
            self.cycle_counter -= 1;
            return;
        } else {
            //TODO: Fire IRQ or something
        }
    }

    pub fn write_byte(&mut self, addr: u32, val: u8) {
        match addr {
            0x1F801800 => self.status_index = val & 0x3, //Status
            0x1F801801 => {
                match self.status_index {
                    0 => self.execute_command(val),
                    1 => self.reg_sound_map_data_out = val,
                    2 => panic!("CD: 0x1F801801 write byte unknown index 2"),
                    3 => panic!("CD: 0x1F801801 write byte unknown index 3"),
                    _ => unreachable!()
                }
            }
            0x1F801802 => {
                match self.status_index {
                    0 => self.push_parameter(val),
                    1 => self.write_interrupt_enable_register(val),
                    2 => panic!("CD: 0x1F801802 write byte unknown index 2"),
                    3 => panic!("CD: 0x1F801802 write byte unknown index 3"),
                    _ => unreachable!()
                }
            }
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

    fn execute_command(&mut self, command: u8) {
        let parameters: Vec<&u8> = self.parameter_queue.iter().collect();
        self.pending_responses.push_front(match command {
            0x19 => {
                //sub_function commands
                match parameters[0] {
                    0x20 => commands::get_bios_date(),
                    _ => panic!("CD: Unknown sub_function command {:#X}", parameters[0])
                }
            }
            _ => panic!("CD: Unknown command {:#X}!", command)
        });
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

    fn push_parameter(&mut self, val: u8) {
        self.parameter_queue.push_front(val);
    }

    fn write_interrupt_flag_register(&mut self, val: u8) {
        self.reg_interrupt_flag &= !val;
        self.response_queue = VecDeque::new(); //Reset queue
        if self.reg_interrupt_flag.get_bit(6) {
            self.parameter_queue = VecDeque::new();
        }
    }

    fn write_interrupt_enable_register(&mut self, val: u8) {
        self.reg_interrupt_enable = val;
    }

    fn read_interrupt_flag_register(&mut self) -> u8 {
        todo!();
    }
}
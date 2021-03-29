use std::collections::VecDeque;
use bit_field::BitField;
use commands::*;

use crate::cpu::{InterruptSource, R3000};

mod commands;

#[derive(Debug, PartialEq, Copy, Clone)]
pub(super) enum DriveState {
    Play,
    Seek,
    Read,
    Idle,
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub(super) enum MotorState {
    Off,
    SpinUp,
    On,
}

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

impl IntCause {
    fn bitflag(&self) -> u8 {
        match self {
            IntCause::INT1 => 1,
            IntCause::INT2 => 2,
            IntCause::INT3 => 3,
            IntCause::INT4 => 4,
            IntCause::INT5 => 5,
            IntCause::INT6 => 6,
            IntCause::INT7 => 7,
            IntCause::INT8 => 8,
            IntCause::INT10h => 0x10,
        }
    }
}

pub(super) struct PendingResponse {
    cause: IntCause,
    response: Vec<u8>,
    execution_cycles: u32,
    extra_response: Option<Box<PendingResponse>>
}

pub struct CDDrive {
    cycle_counter: u32,
    pending_responses: VecDeque<PendingResponse>,

    drive_state: DriveState,
    motor_state: MotorState,
    disk_inserted: bool,

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

            drive_state: DriveState::Idle,
            motor_state: MotorState::On,
            disk_inserted: true,

            reg_interrupt_flag: 0,
            reg_interrupt_enable: 0,
            
            
            //Probably useless registers
            reg_sound_map_data_out: 0,
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

    pub fn read_byte(&mut self, addr: u32) -> u8 {
        match addr {
            0x1F801800 => self.get_status_register(),
            0x1F801801 => {
                match self.status_index {
                    0 => panic!("CD: 0x1F801801 read byte unknown index 0"),
                    1 => self.pop_response(),
                    2 => panic!("CD: 0x1F801803 read byte unknown index 2"),
                    3 => panic!("CD: 0x1F801801 read byte unknown index 3"),
                    _ => unreachable!()
                }
            }
            0x1F801803 => {
                match self.status_index {
                    0 => panic!("CD: 0x1F801803 read byte unknown index 0"),
                    1 => self.reg_interrupt_flag,
                    2 => panic!("CD: 0x1F801803 read byte unknown index 2"),
                    3 => self.reg_interrupt_flag, //Register mirror
                    _ => unreachable!()
                }
            }
            _ => panic!("CD: Tried to read unknown byte. Address: {:#X} Index: {}", addr, self.status_index),
        }
    }

    fn execute_command(&mut self, command: u8) {
        //Execute
        {
            let parameters: Vec<&u8> = self.parameter_queue.iter().collect();
            let response = match command {
                0x1 => get_stat(self),
                0xA => init(self),
                0x1A => get_id(self),
                0x19 => {
                    //sub_function commands
                    match parameters[0] {
                        0x20 => commands::get_bios_date(),
                        _ => panic!("CD: Unknown sub_function command {:#X}", parameters[0])
                    }
                }
                _ => panic!("CD: Unknown command {:#X}!", command)
            };
            self.pending_responses.push_back(response);
        }

        //If this is the first response added to the queue, update the cycle counter
        if self.pending_responses.len() == 1 {
            self.cycle_counter = self.pending_responses[0].execution_cycles;
        }

        //Clear out old parameters
        self.parameter_queue.clear();
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

    fn get_stat(&self) -> u8 {
        let mut status: u8 = 0;
        status |= match self.drive_state {
            DriveState::Play => 0x80,
            DriveState::Seek => 0x40,
            DriveState::Read => 0x20,
            DriveState::Idle => 0,
        };

        if self.motor_state == MotorState::On {
            status |= 0x2;
        };

        status
    }

    fn push_parameter(&mut self, val: u8) {
        self.parameter_queue.push_back(val);
    }

    fn pop_response(&mut self) -> u8 {
        match self.response_queue.pop_front() {
            Some(val) => val,
            None => {
                println!("CD: Tried to read response from empty response queue! Returning 0...");
                0
            }
        }
    }

    fn write_interrupt_flag_register(&mut self, val: u8) {
        self.reg_interrupt_flag &= !val;
        self.response_queue = VecDeque::new(); //Reset queue
        if self.reg_interrupt_flag.get_bit(6) {
            self.parameter_queue = VecDeque::new();
        }
    }

    fn write_interrupt_enable_register(&mut self, val: u8) {
        self.reg_interrupt_enable = val & 0x1f;
    }
}

pub fn step_cycle(cpu: &mut R3000) {
    if cpu.main_bus.cd_drive.cycle_counter > 0 {
        cpu.main_bus.cd_drive.cycle_counter -= 1;
        return;
    } else {
        if cpu.main_bus.cd_drive.pending_responses.len() == 0 {
            //We could respond, but theres no pending responses
            return;
        }

        let response = cpu.main_bus.cd_drive.pending_responses.pop_front().expect("CD: Unable to pop pending response!");
        cpu.main_bus.cd_drive.response_queue = VecDeque::with_capacity(response.response.len()); //Clear queue
        cpu.main_bus.cd_drive.response_queue.extend(response.response.iter());
        cpu.main_bus.cd_drive.reg_interrupt_flag = response.cause.bitflag();

        //Check if interrupt enabled. If so, fire interrupt
        if cpu.main_bus.cd_drive.reg_interrupt_enable & response.cause.bitflag() == response.cause.bitflag() {
            cpu.fire_external_interrupt(InterruptSource::CDROM);
        }

        //If the response has an extra response, push that to the front of the line
        if let Some(ext_response) = response.extra_response {
            cpu.main_bus.cd_drive.pending_responses.push_front(*ext_response);
        }

        //Set cycle counter for next pending response
        //This seems inaccurate. It doesn't start counting the delay till
        //it is at the front of the queue. Realistically, the delay should
        //start counting from the moment the command is fired.
        if cpu.main_bus.cd_drive.pending_responses.len() > 0 {
            cpu.main_bus.cd_drive.cycle_counter = cpu.main_bus.cd_drive.pending_responses[0].execution_cycles;
        }
    }
}
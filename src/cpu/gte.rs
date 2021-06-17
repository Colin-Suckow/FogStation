use bit_field::BitField;
use fixed::types::{I16F16, I20F12, I28F4, I4F12, I8F24, I8F8};
use log::error;

pub(super) struct GTE {
    // Control Registers
    ZSF3: i16,
    ZSF4: i16,
    H: u16,
    DQA: i16,
    DQB: i32,
    OFX: i32,
    OFY: i32,
    RBK: i32,
    BBK: i32,
    GBK: i32,
    RFC: i32,
    GFC: i32,
    BFC: i32,
    LR1: i16,
    LR2: i16,
    LR3: i16,
    LG1: i16,
    LG2: i16,
    LG3: i16,
    LB1: i16,
    LB2: i16,
    LB3: i16,
    L11: i16,
    L12: i16,
    L13: i16,
    L21: i16,
    L22: i16,
    L23: i16,
    L31: i16,
    L32: i16,
    L33: i16,
    RT11: i16,
    RT12: i16,
    RT13: i16,
    RT21: i16,
    RT22: i16,
    RT23: i16,
    RT31: i16,
    RT32: i16,
    RT33: i16,
    TRX: i32,
    TRY: i32,
    TRZ: i32,
    FLAG: u32,

    // Data registers
    VX0: i16,
    VY0: i16,
    VZ0: i16,
    VX1: i16,
    VY1: i16,
    VZ1: i16,
    VX2: i16,
    VY2: i16,
    VZ2: i16,
    IR1: i16,
    IR2: i16,
    IR3: i16,
    MAC0: i32,
    MAC1: i32,
    MAC2: i32,
    MAC3: i32,
    SZ0: u16,
    SZ1: u16,
    SZ2: u16,
    SZ3: u16,
}

// Interface
impl GTE {
    pub(super) fn new() -> Self {
        Self {
            // Control Registers
            ZSF3: 0,
            ZSF4: 0,
            H: 0,
            DQA: 0,
            DQB: 0,
            OFX: 0,
            OFY: 0,
            RBK: 0,
            BBK: 0,
            GBK: 0,
            RFC: 0,
            GFC: 0,
            BFC: 0,
            LR1: 0,
            LR2: 0,
            LR3: 0,
            LG1: 0,
            LG2: 0,
            LG3: 0,
            LB1: 0,
            LB2: 0,
            LB3: 0,
            L11: 0,
            L12: 0,
            L13: 0,
            L21: 0,
            L22: 0,
            L23: 0,
            L31: 0,
            L32: 0,
            L33: 0,
            RT11: 0,
            RT12: 0,
            RT13: 0,
            RT21: 0,
            RT22: 0,
            RT23: 0,
            RT31: 0,
            RT32: 0,
            RT33: 0,
            TRX: 0,
            TRY: 0,
            TRZ: 0,
            FLAG: 0,

            // Data Registers
            VX0: 0,
            VY0: 0,
            VZ0: 0,
            VX1: 0,
            VY1: 0,
            VZ1: 0,
            VX2: 0,
            VY2: 0,
            VZ2: 0,
            IR1: 0,
            IR2: 0,
            IR3: 0,
            MAC0: 0,
            MAC1: 0,
            MAC2: 0,
            MAC3: 0,
            SZ0: 0,
            SZ1: 0,
            SZ2: 0,
            SZ3: 0,
        }
    }

    pub(super) fn set_control_register(&mut self, reg: usize, val: u32) {
        match reg {
            0 => {
                self.RT11 = (val & 0xFFFF) as i16;
                self.RT12 = ((val >> 16) & 0xFFFF) as i16;
            },
            1 => {
                self.RT13 = (val & 0xFFFF) as i16;
                self.RT21 = ((val >> 16) & 0xFFFF) as i16;
            },
            2 => {
                self.RT22 = (val & 0xFFFF) as i16;
                self.RT23 = ((val >> 16) & 0xFFFF) as i16;
            },
            3 => {
                self.RT31 = (val & 0xFFFF) as i16;
                self.RT32 = ((val >> 16) & 0xFFFF) as i16;
            },
            4 => {self.RT33 = val as i16},
            5 => {self.TRX = val as i32},
            6 => {self.TRY = val as i32},
            7 => {self.TRZ = val as i32},
            8 => {
                self.L11 = (val & 0xFFFF) as i16;
                self.L12 = ((val >> 16) & 0xFFFF) as i16;
            },
            9 => {
                self.L13 = (val & 0xFFFF) as i16;
                self.L21 = ((val >> 16) & 0xFFFF) as i16;
            },
            10 => {
                self.L22 = (val & 0xFFFF) as i16;
                self.L23 = ((val >> 16) & 0xFFFF) as i16;
            },
            11 => {
                self.L31 = (val & 0xFFFF) as i16;
                self.L32 = ((val >> 16) & 0xFFFF) as i16;
            },
            12 => {self.L33 = val as i16},
            13 => {self.RBK = val as i32},
            14 => {self.GBK = val as i32},
            15 => {self.BBK = val as i32},
            16 => {
                self.LR1 = (val & 0xFFFF) as i16;
                self.LR2 = ((val >> 16) & 0xFFFF) as i16;
            },
            17 => {
                self.LR3 = (val & 0xFFFF) as i16;
                self.LG1 = ((val >> 16) & 0xFFFF) as i16;
            },
            18 => {
                self.LG2 = (val & 0xFFFF) as i16;
                self.LG3 = ((val >> 16) & 0xFFFF) as i16;
            },
            19 => {
                self.LB1 = (val & 0xFFFF) as i16;
                self.LB2 = ((val >> 16) & 0xFFFF) as i16;
            },
            20 => {self.LB3 = val as i16},
            21 => {self.RFC = val as i32},
            22 => {self.GFC = val as i32},
            23 => {self.BFC = val as i32},
            24 => {self.OFX = val as i32},
            25 => {self.OFY = val as i32},
            26 => {self.H = val as u16},
            27 => {self.DQA = val as i16},
            28 => {self.DQB = val as i32},
            29 => {self.ZSF3 = val as i16},
            30 => {self.ZSF4 = val as i16},
            _ => panic!("Tried to write unknown GTE control register {} ({} RAW)", ctrl_reg_name[reg], reg)
        }
    }

    pub(super) fn set_data_register(&mut self, reg: usize, val: u32) {
        match reg {
            0 => {
                self.VX0 = (val & 0xFFFF) as i16;
                self.VY0 = ((val >> 16) & 0xFFFF) as i16;
            },
            1 => {self.VZ0 = val as i16},
            2 => {
                self.VX1 = (val & 0xFFFF) as i16;
                self.VY1 = ((val >> 16) & 0xFFFF) as i16;
            },
            3 => {self.VZ1 = val as i16},
            4 => {
                self.VX2 = (val & 0xFFFF) as i16;
                self.VY2 = ((val >> 16) & 0xFFFF) as i16;
            },
            5 => {self.VZ2 = val as i16},
            9 => {self.IR1 = val as i16},
            10 => {self.IR2 = val as i16},
            11 => {self.IR3 = val as i16},
            _ => panic!("Tried to write unknown GTE data register {} ({} RAW)", data_reg_name[reg], reg)
        }
    }

    pub(super) fn execute_command(&mut self, command: u32) {
        self.FLAG = 0; // Reset calculation error flags
        match command & 0x3F {
            //0x30 => self.rtpt(command),
            _ => panic!("Unknown GTE command {:#X}!", command & 0x3F)
        };
    }
}

// Register functions
impl GTE {
   fn push_sz(&mut self, val: u16) {
       self.SZ0 = self.SZ1;
       self.SZ1 = self.SZ2;
       self.SZ2 = self.SZ3;
       self.SZ3 = val;
   }
}

// Internal GTE commands
impl GTE {
    fn rtpt(&mut self, command: u32) {
        self.MAC1 = (self.TRX * 0x1000 + (self.RT11*self.VX0 + self.RT12*self.VY0 + self.RT13*self.VZ0) as i32) >> ((command.get_bit(19) as usize) * 12);
        self.MAC2 = (self.TRY * 0x1000 + (self.RT21*self.VX0 + self.RT22*self.VY0 + self.RT23*self.VZ0) as i32) >> ((command.get_bit(19) as usize) * 12);
        self.MAC3 = (self.TRZ * 0x1000 + (self.RT31*self.VX0 + self.RT32*self.VY0 + self.RT33*self.VZ0) as i32) >> ((command.get_bit(19) as usize) * 12);
        self.IR1 = self.MAC1 as i16;
        self.IR2 = self.MAC2 as i16;
        self.IR3 = self.MAC3 as i16;
        self.push_sz((self.MAC3 >> ((!command.get_bit(19) as usize) * 12)) as u16);

        let mut div_val = (((self.H as u32*0x20000/self.SZ3 as u32)+1)/2);
        if div_val > 0x1FFFF {
            div_val = 0x1FFFF;
            self.FLAG.set_bit(17, true);
        }
    }
}


const data_reg_name: [&str; 32] = [
    "vxy0", "vz0",  "vxy1", "vz1",  "vxy2", "vz2",  "rgb",  "otz",   // 00
    "ir0",  "ir1",  "ir2",  "ir3",  "sxy0", "sxy1", "sxy2", "sxyp",  // 08
    "sz0",  "sz1",  "sz2",  "sz3",  "rgb0", "rgb1", "rgb2", "res1",  // 10
    "mac0", "mac1", "mac2", "mac3", "irgb", "orgb", "lzcs", "lzcr",  // 18
];

const ctrl_reg_name: [&str; 32] = [
    "r11r12", "r13r21", "r22r23", "r31r32", "r33", "trx",  "try",  "trz",   // 00
    "l11l12", "l13l21", "l22l23", "l31l32", "l33", "rbk",  "gbk",  "bbk",   // 08
    "lr1lr2", "lr3lg1", "lg2lg3", "lb1lb2", "lb3", "rfc",  "gfc",  "bfc",   // 10
    "ofx",    "ofy",    "h",      "dqa",    "dqb", "zsf3", "zsf4", "flag",  // 18
];
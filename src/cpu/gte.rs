use fixed::types::{I16F16, I20F12, I28F4, I4F12, I8F24, I8F8};

pub(super) struct GTE {
    // Control Registers
    ZSF3: i16,
    ZSF4: i16,
    H: u16,
    DQA: I8F8,
    DQB: I8F24,
    OFX: I16F16,
    OFY: I16F16,
    RBK: I20F12,
    BBK: I20F12,
    GBK: I20F12,
    RFC: I28F4,
    GFC: I28F4,
    BFC: I28F4,
    LR1: I4F12,
    LR2: I4F12,
    LR3: I4F12,
    LG1: I4F12,
    LG2: I4F12,
    LG3: I4F12,
    LB1: I4F12,
    LB2: I4F12,
    LB3: I4F12,
    L11: I4F12,
    L12: I4F12,
    L13: I4F12,
    L21: I4F12,
    L22: I4F12,
    L23: I4F12,
    L31: I4F12,
    L32: I4F12,
    L33: I4F12,
    RT11: I4F12,
    RT12: I4F12,
    RT13: I4F12,
    RT21: I4F12,
    RT22: I4F12,
    RT23: I4F12,
    RT31: I4F12,
    RT32: I4F12,
    RT33: I4F12,
    TRX: i32,
    TRY: i32,
    TRZ: i32,

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
}

// Interface
impl GTE {
    pub(super) fn new() -> Self {
        Self {
            // Control Registers
            ZSF3: 0,
            ZSF4: 0,
            H: 0,
            DQA: I8F8::from_num(0),
            DQB: I8F24::from_num(0),
            OFX: I16F16::from_num(0),
            OFY: I16F16::from_num(0),
            RBK: I20F12::from_num(0),
            BBK: I20F12::from_num(0),
            GBK: I20F12::from_num(0),
            RFC: I28F4::from_num(0),
            GFC: I28F4::from_num(0),
            BFC: I28F4::from_num(0),
            LR1: I4F12::from_num(0),
            LR2: I4F12::from_num(0),
            LR3: I4F12::from_num(0),
            LG1: I4F12::from_num(0),
            LG2: I4F12::from_num(0),
            LG3: I4F12::from_num(0),
            LB1: I4F12::from_num(0),
            LB2: I4F12::from_num(0),
            LB3: I4F12::from_num(0),
            L11: I4F12::from_num(0),
            L12: I4F12::from_num(0),
            L13: I4F12::from_num(0),
            L21: I4F12::from_num(0),
            L22: I4F12::from_num(0),
            L23: I4F12::from_num(0),
            L31: I4F12::from_num(0),
            L32: I4F12::from_num(0),
            L33: I4F12::from_num(0),
            RT11: I4F12::from_num(0),
            RT12: I4F12::from_num(0),
            RT13: I4F12::from_num(0),
            RT21: I4F12::from_num(0),
            RT22: I4F12::from_num(0),
            RT23: I4F12::from_num(0),
            RT31: I4F12::from_num(0),
            RT32: I4F12::from_num(0),
            RT33: I4F12::from_num(0),
            TRX: 0,
            TRY: 0,
            TRZ: 0,

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

        }
    }

    pub(super) fn set_control_register(&mut self, reg: usize, val: u32) {
        match reg {
            0 => {
                self.RT11 = I4F12::from_bits((val & 0xFFFF) as i16);
                self.RT12 = I4F12::from_bits(((val >> 16) & 0xFFFF) as i16);
            },
            1 => {
                self.RT13 = I4F12::from_bits((val & 0xFFFF) as i16);
                self.RT21 = I4F12::from_bits(((val >> 16) & 0xFFFF) as i16);
            },
            2 => {
                self.RT22 = I4F12::from_bits((val & 0xFFFF) as i16);
                self.RT23 = I4F12::from_bits(((val >> 16) & 0xFFFF) as i16);
            },
            3 => {
                self.RT31 = I4F12::from_bits((val & 0xFFFF) as i16);
                self.RT32 = I4F12::from_bits(((val >> 16) & 0xFFFF) as i16);
            },
            4 => {self.RT33 = I4F12::from_bits(val as i16)},
            5 => {self.TRX = val as i32},
            6 => {self.TRY = val as i32},
            7 => {self.TRZ = val as i32},
            8 => {
                self.L11 = I4F12::from_bits((val & 0xFFFF) as i16);
                self.L12 = I4F12::from_bits(((val >> 16) & 0xFFFF) as i16);
            },
            9 => {
                self.L13 = I4F12::from_bits((val & 0xFFFF) as i16);
                self.L21 = I4F12::from_bits(((val >> 16) & 0xFFFF) as i16);
            },
            10 => {
                self.L22 = I4F12::from_bits((val & 0xFFFF) as i16);
                self.L23 = I4F12::from_bits(((val >> 16) & 0xFFFF) as i16);
            },
            11 => {
                self.L31 = I4F12::from_bits((val & 0xFFFF) as i16);
                self.L32 = I4F12::from_bits(((val >> 16) & 0xFFFF) as i16);
            },
            12 => {self.L33 = I4F12::from_bits(val as i16)},
            13 => {self.RBK = I20F12::from_bits(val as i32)},
            14 => {self.GBK = I20F12::from_bits(val as i32)},
            15 => {self.BBK = I20F12::from_bits(val as i32)},
            16 => {
                self.LR1 = I4F12::from_bits((val & 0xFFFF) as i16);
                self.LR2 = I4F12::from_bits(((val >> 16) & 0xFFFF) as i16);
            },
            17 => {
                self.LR3 = I4F12::from_bits((val & 0xFFFF) as i16);
                self.LG1 = I4F12::from_bits(((val >> 16) & 0xFFFF) as i16);
            },
            18 => {
                self.LG2 = I4F12::from_bits((val & 0xFFFF) as i16);
                self.LG3 = I4F12::from_bits(((val >> 16) & 0xFFFF) as i16);
            },
            19 => {
                self.LB1 = I4F12::from_bits((val & 0xFFFF) as i16);
                self.LB2 = I4F12::from_bits(((val >> 16) & 0xFFFF) as i16);
            },
            20 => {self.LB3 = I4F12::from_bits(val as i16)},
            21 => {self.RFC = I28F4::from_bits(val as i32)},
            22 => {self.GFC = I28F4::from_bits(val as i32)},
            23 => {self.BFC = I28F4::from_bits(val as i32)},
            24 => {self.OFX = I16F16::from_bits(val as i32)},
            25 => {self.OFY = I16F16::from_bits(val as i32)},
            26 => {self.H = val as u16},
            27 => {self.DQA = I8F8::from_bits(val as i16)},
            28 => {self.DQB = I8F24::from_bits(val as i32)},
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
        match command & 0x3F {
            0x30 => self.rtpt(),
            _ => panic!("Unknown GTE command {:#X}!", command & 0x3F)
        };
    }
}

// Register functions
impl GTE {
   
}

// Internal GTE commands
impl GTE {
    fn rtpt(&mut self) {
        self.MAC1 = (I4F12::from_num(self.TRX * 0x1000) + self.RT11*self.VX0 + self.RT12*self.VY0 + self.RT13*self.VZ0).to_num();
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
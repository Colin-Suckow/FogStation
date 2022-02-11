use std::os::raw;

use bit_field::BitField;

use super::MdecCommand;

const END_CODE: u16 = 0xFE00;

#[derive(Clone, Copy, Debug)]
pub(crate) enum ColorDepth {
    B4 = 0,
    B8 = 1,
    B24 = 2,
    B15 = 3,
}
#[derive(Clone, Copy)]
pub(crate) struct DecodeMacroblockCommand {
    depth: ColorDepth,
    signed: bool,
    set_b15: bool,
    size: usize,
}

#[derive(Debug)]
enum MacroblockDecodeState {
    Cr,
    Cb,
    Y1,
    Y2,
    Y3,
    Y4
}

impl MacroblockDecodeState {
    fn next(self) -> Self {
        match self {
            Self::Cr => Self::Cb,
            Self::Cb => Self::Y1,
            Self::Y1 => Self::Y2,
            Self::Y2 => Self::Y3,
            Self::Y3 => Self::Y4,
            Self::Y4 => panic!("MDEC: No valid state transition from block Y4"),
        }
    }
}

struct Macroblock {
    cr_block: Vec<u16>,
    cb_block: Vec<u16>,
    y1_block: Vec<u16>,
    y2_block: Vec<u16>,
    y3_block: Vec<u16>,
    y4_block: Vec<u16>,
}

impl Macroblock {
    fn new() -> Self {
        Self {
            cr_block: vec!(),
            cb_block: vec!(),
            y1_block: vec!(),
            y2_block: vec!(),
            y3_block: vec!(),
            y4_block: vec!(),
        }
    }

    fn push_block_value(&mut self, block: &MacroblockDecodeState, value: u16) {
        match block {
            MacroblockDecodeState::Cr => &mut self.cr_block,
            MacroblockDecodeState::Cb => &mut self.cb_block,
            MacroblockDecodeState::Y1 => &mut self.y1_block,
            MacroblockDecodeState::Y2 => &mut self.y2_block,
            MacroblockDecodeState::Y3 => &mut self.y3_block,
            MacroblockDecodeState::Y4 => &mut self.y4_block,
            _ => panic!("MDEC: Header is not a block that can be pushed to")
        }.push(value);
    }

    fn block_data(&self, block: &MacroblockDecodeState) -> &Vec<u16> {
        match block {
            MacroblockDecodeState::Cr => &self.cr_block,
            MacroblockDecodeState::Cb => &self.cb_block,
            MacroblockDecodeState::Y1 => &self.y1_block,
            MacroblockDecodeState::Y2 => &self.y2_block,
            MacroblockDecodeState::Y3 => &self.y3_block,
            MacroblockDecodeState::Y4 => &self.y4_block,
            _ => panic!("MDEC: Not a block"),
        }
    }
}

impl DecodeMacroblockCommand {
    pub(crate) fn new(command_word: u32) -> Self {
        if command_word >> 29 != 1 {
            panic!(
                "Not a decode_macroblock command! Command number = {}",
                command_word >> 29
            );
        }

        let depth = match (command_word >> 27) & 3 {
            0 => ColorDepth::B4,
            1 => ColorDepth::B8,
            2 => ColorDepth::B24,
            3 => ColorDepth::B15,
            _ => unreachable!(),
        };

        let signed = command_word.get_bit(26);
        let set_b15 = command_word.get_bit(25);
        let size = (command_word & 0xFFFF) as usize;
        println!("Color depth {}", (command_word >> 27) & 3);
        println!("MACROBLOCK size: {}", size);

        Self {
            depth,
            signed,
            set_b15,
            size
        }
    }
}

impl MdecCommand for DecodeMacroblockCommand {
    fn parameter_words(&self) -> usize {
        self.size
    }



    fn execute(&self, ctx: &mut super::MDEC) {
        let mut parameters = vec!();
        for w in &ctx.parameter_buffer {
            parameters.push((w & 0xFFFF) as u16);
            parameters.push((w >> 16) as u16);
        }

        // for p in &mut parameters {
        //     let lower = *p & 0xFF;
        //     *p = *p >> 8;
        //     *p |= lower << 8;
        // }

        let mut input_state = MacroblockDecodeState::Cr;
        let mut current_macroblock = Macroblock::new();
        let mut rlc_count: usize = 0;

        for parameter in parameters {
            //println!("state {:?} rlc_count {} parameter {:#X}", input_state, rlc_count, parameter);
            match input_state {
                MacroblockDecodeState::Cr => {
                    if parameter != END_CODE && rlc_count < 63 {
                        if !current_macroblock.block_data(&input_state).is_empty() {
                            rlc_count += 1 + (parameter as usize >> 10);
                        }
                        if rlc_count <= 63 {
                            current_macroblock.push_block_value(&input_state, parameter);
                        } else {
                            let next_state = input_state.next();
                            current_macroblock.push_block_value(&next_state, parameter);
                            input_state = next_state;
                            rlc_count = 0;
                        }
                    } else {
                        if !current_macroblock.cr_block.is_empty() {
                            input_state = input_state.next();
                        }
                        rlc_count = 0;
                    }
                }
                MacroblockDecodeState::Y4 => {
                    if parameter != END_CODE && rlc_count < 63 {
                        if !current_macroblock.block_data(&input_state).is_empty() {
                            rlc_count += 1 + (parameter as usize >> 10);
                        }
                        if rlc_count <= 63 {
                            current_macroblock.push_block_value(&input_state, parameter);
                        } else {
                            println!("Skip execute");
                            let decoded_block = decode_macroblock(ctx, &current_macroblock, &self.depth);
                            ctx.result_buffer.extend(decoded_block);
                            current_macroblock = Macroblock::new();
                            input_state = MacroblockDecodeState::Cr;
                            current_macroblock.push_block_value(&input_state, parameter);
                            rlc_count = 0;
                        }
                    } else {
                        //if !current_macroblock.block_data(&input_state).is_empty() {
                            println!("Execute");
                            let decoded_block = decode_macroblock(ctx, &current_macroblock, &self.depth);
                            ctx.result_buffer.extend(decoded_block);
                            current_macroblock = Macroblock::new();
                            input_state = MacroblockDecodeState::Cr;
                            rlc_count = 0;
                        //}
                    }
                },
                _ => {
                    if parameter != END_CODE && rlc_count < 63 {
                        if !current_macroblock.block_data(&input_state).is_empty() {
                            rlc_count += 1 + (parameter as usize >> 10);
                        }

                        if rlc_count <= 63 {
                            current_macroblock.push_block_value(&input_state, parameter);
                        } else {
                            let next_state = input_state.next();
                            current_macroblock.push_block_value(&next_state, parameter);
                            input_state = next_state;
                            rlc_count = 0;
                        }

                    } else {
                        if !current_macroblock.block_data(&input_state).is_empty() {
                            input_state = input_state.next();
                            rlc_count = 0;
                        }
                    }
                }
            }
        }
        println!("Done");
    }

    fn box_clone(&self) -> Box<dyn MdecCommand> {
        Box::new((*self).clone())
    }

    fn name(&self) -> &str {
        "DecodeMacroblock"
    }

    fn set_status(&self, status: &mut u32) {
        *status |= match self.depth {
            ColorDepth::B4 => 0,
            ColorDepth::B8 => 1,
            ColorDepth::B24 => 2,
            ColorDepth::B15 => 3,
        } << 25;

        status.set_bit(24, self.signed);
        status.set_bit(23, self.set_b15);
       
    }
}



fn decode_macroblock(ctx: & super::MDEC, macroblock: &Macroblock, color_depth: &ColorDepth) -> Vec<u32> {


    let decoded_cr = decode_block(ctx, macroblock.block_data(&MacroblockDecodeState::Cr), true);
    let decoded_cb = decode_block(ctx, macroblock.block_data(&MacroblockDecodeState::Cb), true);
    let decoded_y1 = decode_block(ctx, macroblock.block_data(&MacroblockDecodeState::Y1), false);
    let decoded_y2 = decode_block(ctx, macroblock.block_data(&MacroblockDecodeState::Y2), false);
    let decoded_y3 = decode_block(ctx, macroblock.block_data(&MacroblockDecodeState::Y3), false);
    let decoded_y4 = decode_block(ctx, macroblock.block_data(&MacroblockDecodeState::Y4), false);

    // Combine blocks into (Y, Cb, Cr) pixels

    let mut chroma_block: Vec<(f32, f32, f32)> = vec![(0.0,0.0,0.0); 16 * 16];

    fn loc_px(x: i32, y: i32) -> usize {
        (y * 16 + x) as usize
    }

    fn loc_bk(x: i32, y: i32) -> usize {
        (y * 8 + x) as usize
    }

    for x in 0..8 {
        for y in 0..7 {

            chroma_block[loc_px(x, y)].0 = decoded_y1[loc_bk(x, y)] + 128.0;
            chroma_block[loc_px(x + 8, y)].0 = decoded_y2[loc_bk(x, y)] + 128.0;
            chroma_block[loc_px(x, y + 8)].0 = decoded_y3[loc_bk(x, y)] + 128.0;
            chroma_block[loc_px(x + 8, y + 8)].0 = decoded_y4[loc_bk(x, y)] + 128.0;

            chroma_block[loc_px(x * 2, y * 2)].1 = decoded_cb[loc_bk(x, y)];
            chroma_block[loc_px(x * 2 + 1, y * 2)].1 = decoded_cb[loc_bk(x, y)];
            chroma_block[loc_px(x * 2, y * 2 + 1)].1 = decoded_cb[loc_bk(x, y)];
            chroma_block[loc_px(x * 2 + 1, y * 2 + 1)].1 = decoded_cb[loc_bk(x, y)];

            chroma_block[loc_px(x * 2, y * 2)].2 = decoded_cr[loc_bk(x, y)];
            chroma_block[loc_px(x * 2 + 1, y * 2)].2 = decoded_cr[loc_bk(x, y)];
            chroma_block[loc_px(x * 2, y * 2 + 1)].2 = decoded_cr[loc_bk(x, y)];
            chroma_block[loc_px(x * 2 + 1, y * 2 + 1)].2 = decoded_cr[loc_bk(x, y)];
        }
    }

    // Convert to rgb

    let mut rgb_block: Vec<u16> = chroma_block.iter().map(|(y, cr, cb)| {
        let red = (y + 1.402 * cr).clamp(0.0, 255.0) as u16;
        let green = (y - 0.3437 * cb - 0.7143 * cr).clamp(0.0, 255.0) as u16;
        let blue = (y + 1.772 * cb).clamp(0.0, 255.0) as u16;

        (blue & 0x1f << 10) | (green & 0x1f << 5) | (red & 0x1f)
    }).collect();
    
    // TODO do the real decoding
    match color_depth {
        ColorDepth::B4 => todo!(),
        ColorDepth::B8 => todo!(),
        ColorDepth::B24 => {
            rgb_block.iter().map(|pixel| {
                *pixel as u32
            }).collect()
        },
        ColorDepth::B15 =>  {
            rgb_block.chunks(2).map(|chunk| {
                (chunk[1] as u32) << 16 | (chunk[0] as u32)
            }).collect()
        },
    }
}


fn decode_block(ctx: &super::MDEC, raw_block: &Vec<u16>, is_chroma: bool) -> Vec<f32> {
    // Algorithm copied from https://raw.githubusercontent.com/m35/jpsxdec/readme/jpsxdec/PlayStation1_STR_format.txt
    
    // Translate the DC and AC run length codes into a 64 value list

    let mut coefficient_list: Vec<i16> = vec![0; 64];

    let dc_coefficient = raw_block[0] >> 10;
    let quantization_scale = raw_block[0] & 0x3FF;
    
    
    coefficient_list[0] = dc_coefficient as i16;
    

    let mut i = 0;

    for rlc in &raw_block[1..] {
        println!("i {} rlc {:#X} rlc_shift {}", i, rlc, rlc >> 10);
        i += 1 + (rlc >> 10);
        coefficient_list[i as usize] = (rlc & 0x3ff) as i16;
    }

    // Un-zig-zag the list into a matrix

    let mut coefficient_matrix: Vec<i16> = vec![0; 64];

    for i in 0..64 {
        coefficient_matrix[i] = coefficient_list[ZIG_ZAG_MATRIX[i]] as i16;
    }

    // Dequantization of the matrix
    let mut dequantized_matrix: Vec<i16> = vec![0; 64];


    for i in 0..64 {
        let quant = if is_chroma {
            ctx.color_quant_table[i]
        } else {
            ctx.luminance_quant_table[i]
        };
        if i == 0 {
            dequantized_matrix[i] = coefficient_matrix[i] * quant as i16;
        } else {
            dequantized_matrix[i] = (2 * coefficient_matrix[i] * quant as i16 * quantization_scale as i16) / 16;
        }
    }

    println!("dequant {:?}", dequantized_matrix);

    // Apply Inverse Discrete Cosine Transform

    let mut transformed_matrix: Vec<f32> = vec![0.0; 64];

    for block_x in 0..8 {
        for block_y in 0..8 {
            let mut total: f32 = 0.0;

            for dct_x in 0..8 {
                for dct_y in 0..8 {
                    let mut sub_total = dequantized_matrix[dct_y * 8 + dct_x] as f32;

                    if dct_x == 0 {
                        sub_total *= ((1.0/8.0) as f32).sqrt();
                    } else {
                        sub_total *= ((2.0/8.0) as f32).sqrt();
                    }

                    if dct_y == 0 {
                        sub_total *= ((1.0/8.0) as f32).sqrt();
                    } else {
                        sub_total *= ((2.0/8.0) as f32).sqrt();
                    }

                    sub_total *= f32::cos(dct_x as f32 * 3.14159 * (2.0 * dct_x as f32 + 1.0) / 16.0);
                    sub_total *= f32::cos(dct_y as f32 * 3.14159 * (2.0 * dct_y as f32 + 1.0) / 16.0);
                    total += sub_total;
                }
            }

            transformed_matrix[block_y * 8 + block_x] = total;
        }
    }
    println!("cos transform {:?}", transformed_matrix);
    transformed_matrix
    
}

const ZIG_ZAG_MATRIX: [usize; 64] = [
    0,  1,  5,  6, 14, 15, 27, 28,
    2,  4,  7, 13, 16, 26, 29, 42,
    3,  8, 12, 17, 25, 30, 41, 43,
    9, 11, 18, 24, 31, 40, 44, 53,
    10, 19, 23, 32, 39, 45, 52, 54,
    20, 22, 33, 38, 46, 51, 55, 60,
    21, 34, 37, 47, 50, 56, 59, 61,
    35, 36, 48, 49, 57, 58, 62, 63
];
use bit_field::BitField;

const H_RES: u32 = H_BLANK_START + 20;
const V_RES: u32 = V_BLANK_START + 40;
const H_BLANK_START: u32 = 640;
const V_BLANK_START: u32 = 480;

#[derive(Copy, Clone, Debug)]
enum TextureColorMode {
    FourBit,
    EightBit,
    FifteenBit,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Resolution {
    pub height: u32,
    pub width: u32,
}

#[derive(Copy, Clone, Debug)]
struct Point {
    x: i16,
    y: i16,
    color: u16,
    tex_x: i16,
    tex_y: i16,
}

impl Point {
    fn from_word(word: u32, color: u16) -> Self {
        Self {
            x: (word & 0xFFFF) as i16,
            y: ((word >> 16) & 0xFFFF) as i16,
            color,
            tex_x: 0,
            tex_y: 0,
        }
    }

    fn from_word_with_offset(word: u32, color: u16, offset: Point) -> Self {
        Self {
            x: ((word & 0xFFFF) as i32 + offset.x as i32) as i16,
            y: (((word >> 16) & 0xFFFF) as i32 + offset.y as i32) as i16,
            color,
            tex_x: 0,
            tex_y: 0,
        }
    }

    fn from_components(x: i16, y: i16, color: u16) -> Self {
        Self {
            x,
            y,
            color,
            tex_x: 0,
            tex_y: 0,
        }
    }

    fn new_textured_point(word: u32, tex_y: i16, tex_x: i16) -> Self {
        Self {
            x: (word & 0xFFFF) as i16,
            y: ((word >> 16) & 0xFFFF) as i16,
            color: 0,
            tex_x,
            tex_y,
        }
    }
}

pub struct Gpu {
    vram: Vec<u16>,
    status_reg: u32,
    pixel_count: u32,
    enabled: bool,
    gp0_buffer: Vec<u32>,

    texpage_x_base: u16,
    texpage_y_base: u16,
    texmode: TextureColorMode,
    palette_x: u16,
    palette_y: u16,
    blend_enabled: bool,
    blend_color: u16,

    draw_area_tl_point: Point,
    draw_area_br_point: Point,
    draw_offset: Point,

    irq_fired: bool,
    vblank_consumed: bool,
    hblank_consumed: bool,
    show_frame: bool,

    display_h_res: u32,
    display_v_res: u32,
}

impl Gpu {
    pub fn new() -> Gpu {
        Gpu {
            vram: vec![0; 1_048_576 / 2],
            status_reg: 0x1C000000,
            pixel_count: 0,
            enabled: false,
            gp0_buffer: Vec::new(),

            texpage_x_base: 0,
            texpage_y_base: 0,
            texmode: TextureColorMode::FifteenBit,
            palette_x: 0,
            palette_y: 0,
            blend_enabled: false,
            blend_color: 0xFFFF,

            draw_area_tl_point: Point::from_components(0, 0, 0),
            draw_area_br_point: Point::from_components(0, 0, 0),

            draw_offset: Point::from_components(0, 0, 0),
            irq_fired: false,
            vblank_consumed: false,
            hblank_consumed: false,
            show_frame: false,

            display_h_res: 640,
            display_v_res: 480,
        }
    }

    //Only reseting the big stuff. This will probably bite me later
    pub fn reset(&mut self) {
        self.vram = vec![0; 1_048_576 / 2];
        self.status_reg = 0x1C000000;
        self.gp0_buffer = Vec::new();
    }

    pub fn read_status_register(&mut self) -> u32 {
        ////println!("Reading GPUSTAT");
        let mut stat: u32 = 0;

        stat |= (self.texpage_x_base) as u32;
        stat |= (self.texpage_y_base << 4) as u32;

        stat |= match self.texmode {
            TextureColorMode::FourBit => 0,
            TextureColorMode::EightBit => 1,
            TextureColorMode::FifteenBit => 2,
        } << 7;

        stat |= 0x1C000000;

        stat
    }

    pub fn read_word_gp0(&mut self) -> u32 {
        //println!("Reading gp0");
        0x0 as u32
    }

    pub fn send_gp0_command(&mut self, value: u32) {
        self.gp0_push(value);

        let command = self.gp0_buffer[0];

        match command.gp0_header() {
            0x0 => {
                //Random junk
                match command >> 24 {
                    0x2 => {
                        //Quick rectangle fill
                        if self.gp0_buffer.len() < 3 {
                            //Not enough commands
                            return;
                        }

                        let x1 = self.gp0_buffer[1] & 0xFFFF;
                        let y1 = (self.gp0_buffer[1] >> 16) & 0xFFFF;
                        let x2 = ((self.gp0_buffer[2] & 0xFFFF) + x1);
                        let y2 = (((self.gp0_buffer[2] >> 16) & 0xFFFF) + y1);
                        self.draw_solid_box(
                            x1,
                            y1,
                            x2,
                            y2,
                            b24color_to_b15color(self.gp0_buffer[0] & 0x1FFFFFF) + 1,
                            false,
                        );
                    }
                    _ => {
                        //NOP
                    }
                }
            }

            0x1 => {
                //Render Polygon

                // If the polygon is textured or gouraud shaded, lets just lock up the emulator.
                // I only want to test flat shaded polygons right now

                let is_gouraud = command.get_bit(28);
                let is_textured = command.get_bit(26);
                let is_quad = command.get_bit(27);
                let verts = if is_quad { 4 } else { 3 };

                let packets = 1
                    + (verts * is_textured as usize)
                    + verts
                    + if is_gouraud { verts - 1 } else { 0 };

                if self.gp0_buffer.len() < packets {
                    // Not enough words for the command. Return early
                    return;
                }

                let fill = b24color_to_b15color(self.gp0_buffer[0] & 0x1FFFFFF);
                self.blend_enabled = self.gp0_buffer[0].get_bit(24);
                self.blend_color = fill;
                if is_quad {
                    if is_textured && is_gouraud {
                        //Should be blending in colors. Do that later
                        //println!("Tried to try draw texture blended quad!");
                    } else if is_textured {
                        //println!("GPU: Tex quad");
                        let points: Vec<Point> = vec![
                            Point::new_textured_point(
                                self.gp0_buffer[1],
                                ((self.gp0_buffer[2] >> 8) & 0xFF) as i16,
                                (self.gp0_buffer[2] & 0xFF) as i16,
                            ),
                            Point::new_textured_point(
                                self.gp0_buffer[3],
                                ((self.gp0_buffer[4] >> 8) & 0xFF) as i16,
                                (self.gp0_buffer[4] & 0xFF) as i16,
                            ),
                            Point::new_textured_point(
                                self.gp0_buffer[5],
                                ((self.gp0_buffer[6] >> 8) & 0xFF) as i16,
                                (self.gp0_buffer[6] & 0xFF) as i16,
                            ),
                            Point::new_textured_point(
                                self.gp0_buffer[7],
                                ((self.gp0_buffer[8] >> 8) & 0xFF) as i16,
                                (self.gp0_buffer[8] & 0xFF) as i16,
                            ),
                        ];

                        self.palette_x = ((self.gp0_buffer[2] >> 16) & 0x3F) as u16;
                        self.palette_y = ((self.gp0_buffer[2] >> 22) & 0x1FF) as u16;
                        self.texpage_x_base = ((self.gp0_buffer[4] >> 16) & 0xF) as u16;
                        self.texpage_y_base = ((self.gp0_buffer[4] >> 20) & 0x1) as u16;
                        self.blend_color = fill;

                        self.draw_textured_quad(&points, command.get_bit(25));
                    } else if is_gouraud {
                        //println!("GPU: gouraud quad");
                        let points: Vec<Point> = vec![
                            Point::from_word(self.gp0_buffer[1], fill),
                            Point::from_word(
                                self.gp0_buffer[3],
                                b24color_to_b15color(self.gp0_buffer[2]),
                            ),
                            Point::from_word(
                                self.gp0_buffer[5],
                                b24color_to_b15color(self.gp0_buffer[4]),
                            ),
                            Point::from_word(
                                self.gp0_buffer[7],
                                b24color_to_b15color(self.gp0_buffer[6]),
                            ),
                        ];
                        self.draw_shaded_quad(&points, command.get_bit(25));
                    } else {
                        let points: Vec<Point> = vec![
                            Point::from_word(self.gp0_buffer[1], 0),
                            Point::from_word(self.gp0_buffer[2], 0),
                            Point::from_word(self.gp0_buffer[3], 0),
                            Point::from_word(self.gp0_buffer[4], 0),
                        ];
                        self.draw_solid_quad(&points, fill, command.get_bit(25));
                    };
                } else {
                    if is_gouraud && is_textured {
                        //println!("Tried to try draw texture blended tri! Queue {:?}", self.gp0_buffer);
                    } else if is_textured {
                        //println!("GPU: Tex tri");
                        let points: Vec<Point> = vec![
                            Point::new_textured_point(
                                self.gp0_buffer[1],
                                ((self.gp0_buffer[2] >> 8) & 0xFF) as i16,
                                (self.gp0_buffer[2] & 0xFF) as i16,
                            ),
                            Point::new_textured_point(
                                self.gp0_buffer[3],
                                ((self.gp0_buffer[4] >> 8) & 0xFF) as i16,
                                (self.gp0_buffer[4] & 0xFF) as i16,
                            ),
                            Point::new_textured_point(
                                self.gp0_buffer[5],
                                ((self.gp0_buffer[6] >> 8) & 0xFF) as i16,
                                (self.gp0_buffer[6] & 0xFF) as i16,
                            ),
                        ];
                        ////println!("{:?}", points);
                        self.palette_x = ((self.gp0_buffer[2] >> 16) & 0x3F) as u16;
                        self.palette_y = ((self.gp0_buffer[2] >> 22) & 0x1FF) as u16;
                        //println!("palx {}", self.palette_x);
                        self.texpage_x_base = ((self.gp0_buffer[4] >> 16) & 0xF) as u16;
                        self.texpage_y_base = ((self.gp0_buffer[4] >> 20) & 0x1) as u16;
                        // self.blend_color = if fill == 0 {
                        //     0xFFFF
                        // } else {
                        //     fill
                        // };
                        self.blend_color = fill;
                        self.draw_textured_triangle(&points, command.get_bit(25));
                    } else if is_gouraud {
                        //println!("GPU: gouraud tri");
                        let points: Vec<Point> = vec![
                            Point::from_word(self.gp0_buffer[1], fill),
                            Point::from_word(
                                self.gp0_buffer[3],
                                b24color_to_b15color(self.gp0_buffer[2]),
                            ),
                            Point::from_word(
                                self.gp0_buffer[5],
                                b24color_to_b15color(self.gp0_buffer[4]),
                            ),
                        ];
                        ////println!("{:?}", points);
                        self.draw_shaded_triangle(&points, command.get_bit(25));
                    } else {
                        let points: Vec<Point> = vec![
                            Point::from_word(self.gp0_buffer[1], 0),
                            Point::from_word(self.gp0_buffer[2], 0),
                            Point::from_word(self.gp0_buffer[3], 0),
                        ];
                        self.draw_solid_triangle(&points, fill, command.get_bit(25));
                    }
                }
            }

            0x2 => {
                //Render line
                if command.get_bit(27) {
                    ////println!("{:?}", self.gp0_buffer);
                    if (self.gp0_buffer[self.gp0_buffer.len() - 1] & 0xF000F000) != 0x50005000 {
                        //Wait until terminating vertex
                        return;
                    }
                    //TODO draw polyline
                } else {
                    if self.gp0_buffer.len() < (3 + if command.get_bit(28) { 2 } else { 0 }) {
                        //Not enough commands
                        return;
                    }

                    //TODO draw line
                }
            }

            0x3 => {
                //Render Rectangle

                let size = (command >> 27) & 0x3;

                let length =
                    2 + if size == 0 { 1 } else { 0 } + if command.get_bit(26) { 1 } else { 0 };

                if self.gp0_buffer.len() < length {
                    //Not enough commands
                    return;
                }

                match size {
                    0b01 => {
                        //println!("GPU: Single point");
                        //Draw single pixel
                        let point = Point::from_word(self.gp0_buffer[1], 0);

                        let address = point_to_address(point.x as u32, point.y as u32) as usize;
                        let color = if command.get_bit(25) {
                            //Transparent
                            alpha_composite(
                                self.vram[address],
                                b24color_to_b15color(self.gp0_buffer[0] & 0x1FFFFFF),
                            )
                        } else {
                            b24color_to_b15color(self.gp0_buffer[0] & 0x1FFFFFF)
                        };
                        self.vram[address] = color;
                    }

                    0b0 => {
                        //Draw variable size
                        if command.get_bit(26) {
                            //println!("GPU: Tex box");
                            let tl_point = Point::new_textured_point(
                                self.gp0_buffer[1],
                                ((self.gp0_buffer[2] >> 8) & 0xFF) as i16,
                                (self.gp0_buffer[2] & 0xFF) as i16,
                            );

                            let size = Point::from_word(self.gp0_buffer[3], 0);

                            self.palette_x = ((self.gp0_buffer[2] >> 16) & 0x3F) as u16;
                            self.palette_y = ((self.gp0_buffer[2] >> 22) & 0x1FF) as u16;

                            self.draw_textured_box(&tl_point, size.x, size.y, command.get_bit(25));
                        } else {
                            //println!("GPU: solid box");
                            let tl_point = Point::from_word(self.gp0_buffer[1], 0);
                            let br_point =
                                Point::from_word_with_offset(self.gp0_buffer[2], 0, tl_point);

                            //println!("tl: {:?} br: {:?}", tl_point, br_point);

                            self.draw_solid_box(
                                (tl_point.x + self.draw_offset.x) as u32,
                                (tl_point.y + self.draw_offset.y) as u32,
                                (br_point.x + self.draw_offset.x) as u32,
                                (br_point.y + self.draw_offset.y) as u32,
                                b24color_to_b15color(self.gp0_buffer[0] & 0x1FFFFFF),
                                command.get_bit(25),
                            );
                        }
                    }

                    0b10 => {
                        //8x8 sprite
                        //println!("GPU: 8x8 sprite");
                        if command.get_bit(26) {
                            let tl_point = Point::new_textured_point(
                                self.gp0_buffer[1],
                                ((self.gp0_buffer[2] >> 8) & 0xFF) as i16,
                                (self.gp0_buffer[2] & 0xFF) as i16,
                            );

                            let size = Point::from_components(8, 8, 0);

                            self.palette_x = ((self.gp0_buffer[2] >> 16) & 0x3F) as u16;
                            self.palette_y = ((self.gp0_buffer[2] >> 22) & 0x1FF) as u16;

                            self.draw_textured_box(&tl_point, size.x, size.y, command.get_bit(25));
                        } else {
                            let x1 = self.gp0_buffer[1] & 0xFFFF;
                            let y1 = (self.gp0_buffer[1] >> 16) & 0xFFFF;
                            self.draw_solid_box(
                                x1,
                                y1,
                                x1 + 8,
                                y1 + 8,
                                b24color_to_b15color(self.gp0_buffer[0] & 0x1FFFFFF),
                                command.get_bit(25),
                            );
                        }
                    }

                    0b11 => {
                        //16x16 sprite
                        //println!("GPU: 16x16 sprite");
                        if command.get_bit(26) {
                            let tl_point = Point::new_textured_point(
                                self.gp0_buffer[1],
                                ((self.gp0_buffer[2] >> 8) & 0xFF) as i16,
                                (self.gp0_buffer[2] & 0xFF) as i16,
                            );

                            let size = Point::from_components(16, 16, 0);

                            self.palette_x = ((self.gp0_buffer[2] >> 16) & 0x3F) as u16;
                            self.palette_y = ((self.gp0_buffer[2] >> 22) & 0x1FF) as u16;

                            self.draw_textured_box(&tl_point, size.x, size.y, command.get_bit(25));
                        } else {
                            let x1 = self.gp0_buffer[1] & 0xFFFF;
                            let y1 = (self.gp0_buffer[1] >> 16) & 0xFFFF;
                            self.draw_solid_box(
                                x1,
                                y1,
                                x1 + 16,
                                y1 + 16,
                                b24color_to_b15color(self.gp0_buffer[0] & 0x1FFFFFF),
                                command.get_bit(25),
                            );
                        }
                    }

                    _ => {
                        //Lets do nothing with the others
                        //println!("Invalid size rect");
                    }
                }
            }

            0x4 => {
                //VRAM to VRAM blit
                //println!("GPU: VRAM -> VRAM blit");
                if self.gp0_buffer.len() < 4 {
                    //Not enough commands
                    return;
                }
                //println!("Running VRAM to VRAM transfer");
                let x_source = self.gp0_buffer[1] & 0xFFFF;
                let y_source = (self.gp0_buffer[1] >> 16) & 0xFFFF;
                let x_dest = self.gp0_buffer[2] & 0xFFFF;
                let y_dest = (self.gp0_buffer[2] >> 16) & 0xFFFF;
                let width = self.gp0_buffer[3] & 0xFFFF;
                let height = (self.gp0_buffer[3] >> 16) & 0xFFFF;

                self.copy_rectangle(x_source, y_source, x_dest, y_dest, width, height);
            }
            0x5 => {
                //CPU To VRAM
                if self.gp0_buffer.len() < 3 {
                    //Not enough for the header
                    return;
                }
                let width = (self.gp0_buffer[2] & 0xFFFF) as u16;
                let height = (((self.gp0_buffer[2] >> 16) & 0xFFFF) as u16) * 2;
                let length = (((width / 2) * height) / 2) + 3;
                if self.gp0_buffer.len() < length as usize {
                    //Not enough commands
                    return;
                }

                let base_x = (self.gp0_buffer[1] & 0xFFFF) as u16;
                let base_y = ((self.gp0_buffer[1] >> 16) & 0xFFFF) as u16;


                for index in 3..(length) {
                    let p2 = ((self.gp0_buffer[index as usize] >> 16) & 0xFFFF) as u16;
                    let p1 = (self.gp0_buffer[index as usize] & 0xFFFF) as u16;
                    let x = base_x + (((index - 3) * 2) % width);
                    let y = base_y + (((index - 3) * 2) / width);
                    let addr = point_to_address(x as u32, y as u32);
                    self.vram[addr as usize] = p1;
                    self.vram[(addr + 1) as usize] = p2;
                }
            }

            0x6 => {
                //VRAM to CPU
                if self.gp0_buffer.len() < 3 {
                    return;
                }
                println!("VRAM to CPU")
                //Lets ignore this one for now
            }
            0x7 => {
                //Env commands
                match command.command() {
                    0xE1 => {
                        //Draw Mode Setting
                        self.texpage_x_base = (command & 0xF) as u16;
                        self.texpage_y_base = if command.get_bit(4) { 1 } else { 0 };
                        self.texmode = match (command >> 7) & 0x3 {
                            0 => TextureColorMode::FourBit,
                            1 => TextureColorMode::EightBit,
                            2 => TextureColorMode::FifteenBit,
                            3 => TextureColorMode::FifteenBit, // This one is FifteenBit, for some reason
                            _ => panic!("Unknown texture color mode {}", (command >> 7) & 0x3),
                        };
                    }

                    0xE3 => {
                        //Set Drawing Area Top Left
                        self.draw_area_tl_point = Point::from_components(
                            ((command & 0x3FF) as u16) as i16,
                            (((command >> 10) & 0x1FF) as u16) as i16,
                            0,
                        );
                    }

                    0xE4 => {
                        //Set Drawing Area Bottom Right
                        self.draw_area_br_point = Point::from_components(
                            ((command & 0x3FF) as u16) as i16,
                            (((command >> 10) & 0x1FF) as u16) as i16,
                            0,
                        );
                    }

                    0xE2 => {
                        //Texture window area
                        //Not needed rn
                        //println!("GP0 command E2 not implemented!");
                    }

                    0xE5 => {
                        //Set Drawing Offset
                        let x = (command & 0x7FF) as i16;
                        let y = ((command >> 11) & 0x7FF) as i16;
                        self.draw_offset = Point::from_components(x, y, 0);
                    }

                    0xE6 => {
                        //Mask bit
                        //Also no needed
                        //println!("GP0 command E6 not implemented!");
                    }

                    
                    _ => println!(
                        "Unknown GPU ENV command {:#X}. Full command queue is {:#X}",
                        command.command(),
                        self.gp0_buffer[0]
                    ),
                }
            }

            0x1F => {
                panic!("GPU IRQ requested!");
            }

            _ => println!("unknown gp0 {:#X}!", command.gp0_header()),
        }
        //Made it to the end, so the command must have been executed
        self.gp0_clear();
    }

    pub fn send_gp1_command(&mut self, command: u32) {
        //println!("GP1 Command {:#X} parameter {:#X}", command.command(), command.parameter());
        match command.command() {
            0x0 => {
                //Reset GPU
                self.enabled = false;
                self.status_reg = 0;
                self.pixel_count = 0;
                self.vram = vec![0; 1_048_576 / 2];
            }

            0x1 => {
                //Reset Command buffer
                self.gp0_buffer.clear();
            }

            0x2 => {
                self.show_frame = true;
            }

            0x6 => {
                //Horizontal Display Range
                //Ignore this one for now
            }

            0x8 => {
                //Display mode
                self.display_h_res = {
                    if command.get_bit(6) {
                        368
                    } else {
                        match command & 0x3 {
                            0 => 256,
                            1 => 320,
                            2 => 512,
                            3 => 640,
                            _ => unreachable!(),
                        }
                    }
                };

                self.display_v_res = if command.get_bit(2) && command.get_bit(5) {
                    480
                } else {
                    240
                };
            }

            0x10 => {
                //Get gpu information
                //Ignoring this too
            }
            _ => println!(
                "Unknown gp1 command {:#X} parameter {}!",
                command.command(),
                command.parameter()
            ),
        }
    }

    pub fn execute_cycle(&mut self) {
        self.pixel_count += 1;

        if self.pixel_count % H_RES == 0 {
            self.hblank_consumed = false;
        }

        if self.pixel_count > H_RES * V_RES {
            self.pixel_count = 0;
            self.vblank_consumed = false;
        }
    }

    pub fn is_vblank(&self) -> bool {
        self.pixel_count > H_RES * V_BLANK_START
    }

    pub fn is_hblank(&self) -> bool {
        self.pixel_count % H_RES > H_BLANK_START
    }

    pub fn resolution(&self) -> Resolution {
        Resolution {
            width: self.display_h_res,
            height: self.display_v_res,
        }
    }

    pub fn consume_vblank(&mut self) -> bool {
        if !self.vblank_consumed && self.is_vblank() {
            self.vblank_consumed = true;
            true
        } else {
            false
        }
    }

    pub fn consume_hblank(&mut self) -> bool {
        if !self.hblank_consumed && self.is_hblank() {
            self.hblank_consumed = true;
            true
        } else {
            false
        }
    }

    pub fn end_of_frame(&self) -> bool {
        self.pixel_count == (self.display_h_res + 20) * (self.display_v_res + 40)
    }

    pub fn get_vram(&self) -> &Vec<u16> {
        &self.vram
    }

    ///Returns irq status. If true, function will return true then clear irq status
    pub fn consume_irq(&mut self) -> bool {
        if self.irq_fired {
            self.irq_fired = false;
            true
        } else {
            false
        }
    }

    fn gp0_push(&mut self, val: u32) {
        self.gp0_buffer.push(val);
    }

    fn gp0_clear(&mut self) {
        self.gp0_buffer.clear();
    }

    fn copy_horizontal_line(
        &mut self,
        x_source: u32,
        y_source: u32,
        x_dest: u32,
        y_dest: u32,
        width: u32,
    ) {
        for x_offset in 0..=width {
            let val =
                self.vram[(point_to_address(x_source + x_offset, y_source) as usize) % 524288];
            let addr = point_to_address(x_dest + x_offset, y_dest) as usize;
            self.vram[addr % 524288] = val;
        }
    }

    fn copy_rectangle(
        &mut self,
        x_source: u32,
        y_source: u32,
        x_dest: u32,
        y_dest: u32,
        width: u32,
        height: u32,
    ) {
        for y_offset in 0..height {
            self.copy_horizontal_line(
                x_source,
                y_source + y_offset,
                x_dest,
                y_dest + y_offset,
                width,
            );
        }
    }

    fn draw_horizontal_line(&mut self, x1: u32, x2: u32, y: u32, fill: u16, transparent: bool) {
        
        for x in x1..x2 {
            if self.out_of_draw_area(&Point::from_components(x as i16, y as i16, 0)) {
                continue;
            }
            let address = point_to_address(x, y) as usize;
            let color = if transparent {
                alpha_composite(self.vram[address % 524288], fill)
            } else {
                fill
            };
            if fill != 0 {
                self.vram[address % 524288] = color;
            }
        }
    }

    fn draw_horizontal_line_shaded(
        &mut self,
        x1: i16,
        x2: i16,
        y: i16,
        x1_color: u16,
        x2_color: u16,
        transparent: bool,
    ) {
        let (start, end, start_color, end_color) = if x1 > x2 {
            (x2, x1, x2_color, x1_color)
        } else {
            (x1, x2, x1_color, x2_color)
        };
        for x in start..end {
            if self.out_of_draw_area(&Point::from_components(x, y, 0)) {
                continue;
            }
            let address = point_to_address(x as u32, y as u32) as usize;
            let fill = lerp_color(start_color, end_color, start, end, x);
            ////println!("x {} end {} fill {:#X}", x, end, fill);
            let color = if transparent {
                alpha_composite(self.vram[address % 524288], fill)
            } else {
                fill
            };
            if fill != 0 {
                self.vram[address % 524288] = color;
            }
        }
    }
    fn out_of_draw_area(&self, test_point: &Point) -> bool {
        !(test_point.x > self.draw_area_tl_point.x
            && test_point.x < self.draw_area_br_point.x
            && test_point.y > self.draw_area_tl_point.y
            && test_point.y < self.draw_area_br_point.y)
    }

    fn draw_horizontal_line_textured(
        &mut self,
        x1: i16,
        x2: i16,
        y: i16,
        y1_tex: i16,
        y2_tex: i16,
        x1_tex: i16,
        x2_tex: i16,
        transparent: bool,
    ) {
        let (start, end) = if x1 > x2 { (x2, x1) } else { (x1, x2) };
        ////println!("x1: {} y1: {} x2: {} y2: {}", x1_tex, y1_tex, x2_tex, y2_tex);
        for x in start..end {
            if self.out_of_draw_area(&Point::from_components(x, y, 0)) {
                continue;
            }

            let address = point_to_address(x as u32, y as u32) as usize;

            let fill = self.get_texel(
                lerp_coords(x1_tex, x2_tex, start, end, x),
                lerp_coords(y1_tex, y2_tex, start, end, x),
            );
            //let fill = 0xFFFF;
            ////println!("x {} end {} fill {:#X}", x, end, fill);

            let color = if transparent {
                alpha_composite(self.vram[address % 524288], fill)
            } else {
                fill
            };
            if fill != 0 {
                self.vram[address % 524288] = color;
            }
        }
    }

    fn draw_solid_box(&mut self, x1: u32, y1: u32, x2: u32, y2: u32, fill: u16, transparent: bool) {
        for y in y1..y2 {
            self.draw_horizontal_line(x1, x2, y, fill, transparent);
        }
    }

    fn draw_textured_box(&mut self, tl_point: &Point, width: i16, height: i16, transparent: bool) {
        for offset in 0..height {
            self.draw_horizontal_line_textured(
                tl_point.x,
                tl_point.x + width,
                tl_point.y + offset,
                tl_point.tex_y + offset,
                tl_point.tex_y + offset,
                tl_point.tex_x,
                tl_point.tex_x + width,
                transparent,
            )
        }
    }

    fn draw_solid_flat_bottom_triangle(
        &mut self,
        p1: Point,
        p2: Point,
        p3: Point,
        fill: u16,
        transparent: bool,
    ) {
        let invslope1 = (p2.x - p1.x) as f32 / (p2.y - p1.y) as f32;
        let invslope2 = (p3.x - p1.x) as f32 / (p3.y - p1.y) as f32;

        let mut curx1 = p1.x as f32;
        let mut curx2 = p1.x as f32;

        for scanline in p1.y..p2.y {
            self.draw_horizontal_line(
                curx1 as u32,
                curx2 as u32,
                scanline as u32,
                fill,
                transparent,
            );
            curx1 = curx1 + invslope1;
            curx2 = curx2 + invslope2;
        }
    }

    fn draw_solid_flat_top_triangle(
        &mut self,
        p1: Point,
        p2: Point,
        p3: Point,
        fill: u16,
        transparent: bool,
    ) {
        let invslope1 = (p3.x - p1.x) as f32 / (p3.y - p1.y) as f32;
        let invslope2 = (p3.x - p2.x) as f32 / (p3.y - p2.y) as f32;

        let mut curx1 = p3.x as f32;
        let mut curx2 = p3.x as f32;

        for scanline in (p1.y..=p3.y).rev() {
            self.draw_horizontal_line(
                (curx1 + 0.5) as u32,
                (curx2 + 0.5) as u32,
                scanline as u32,
                fill,
                transparent,
            );
            curx1 = curx1 - invslope1;
            curx2 = curx2 - invslope2;
        }
    }

    fn draw_shaded_flat_bottom_triangle(
        &mut self,
        p1: Point,
        p2: Point,
        p3: Point,
        transparent: bool,
    ) {
        let invslope1 = (p2.x - p1.x) as f32 / (p2.y - p1.y) as f32;
        let invslope2 = (p3.x - p1.x) as f32 / (p3.y - p1.y) as f32;

        let mut curx1 = p1.x as f32;
        let mut curx2 = p1.x as f32;

        for scanline in p1.y..p3.y {
            let curx1_color = lerp_color(p1.color, p2.color, p1.y, p3.y, scanline);
            let curx2_color = lerp_color(p1.color, p3.color, p1.y, p3.y, scanline);
            self.draw_horizontal_line_shaded(
                curx1 as i16,
                curx2 as i16,
                scanline,
                curx1_color,
                curx2_color,
                transparent,
            );
            curx1 = curx1 + invslope1;
            curx2 = curx2 + invslope2;
        }
    }

    fn draw_shaded_flat_top_triangle(
        &mut self,
        p1: Point,
        p2: Point,
        p3: Point,
        transparent: bool,
    ) {
        let invslope1 = (p3.x - p1.x) as f32 / (p3.y - p1.y) as f32;
        let invslope2 = (p3.x - p2.x) as f32 / (p3.y - p2.y) as f32;

        let mut curx1 = p3.x as f32;
        let mut curx2 = p3.x as f32;

        for scanline in (p1.y..=p3.y).rev() {
            let curx1_color = lerp_color(p1.color, p3.color, p1.y, p3.y, scanline);
            let curx2_color = lerp_color(p2.color, p3.color, p1.y, p3.y, scanline);
            self.draw_horizontal_line_shaded(
                (curx1 + 0.5) as i16,
                (curx2 + 0.5) as i16,
                scanline,
                curx1_color,
                curx2_color,
                transparent,
            );
            curx1 = curx1 - invslope1;
            curx2 = curx2 - invslope2;
        }
    }

    fn draw_textured_flat_bottom_triangle(
        &mut self,
        p1: Point,
        p2: Point,
        p3: Point,
        transparent: bool,
    ) {
        let invslope1 = (p2.x - p1.x) as f32 / (p2.y - p1.y) as f32;
        let invslope2 = (p3.x - p1.x) as f32 / (p3.y - p1.y) as f32;

        let mut curx1 = p1.x as f32;
        let mut curx2 = p1.x as f32;

        for scanline in p1.y..p3.y {
            let y1 = lerp_coords(p1.tex_y, p3.tex_y, p1.y, p3.y, scanline);
            let y2 = lerp_coords(p1.tex_y, p2.tex_y, p1.y, p3.y, scanline);
            let x1 = lerp_coords(p1.tex_x, p3.tex_x, p1.y, p3.y, scanline);
            let x2 = lerp_coords(p1.tex_x, p2.tex_x, p1.y, p3.y, scanline);
            self.draw_horizontal_line_textured(
                curx1 as i16,
                curx2 as i16,
                scanline,
                y1,
                y2,
                x1,
                x2,
                transparent,
            );
            curx1 = curx1 + invslope1;
            curx2 = curx2 + invslope2;
        }
    }

    fn draw_textured_flat_top_triangle(
        &mut self,
        p1: Point,
        p2: Point,
        p3: Point,
        transparent: bool,
    ) {
        let invslope1 = (p3.x - p1.x) as f32 / (p3.y - p1.y) as f32;
        let invslope2 = (p3.x - p2.x) as f32 / (p3.y - p2.y) as f32;

        let mut curx1 = p3.x as f32;
        let mut curx2 = p3.x as f32;

        for scanline in (p1.y..=p3.y).rev() {
            let y1 = lerp_coords(p1.tex_y, p3.tex_y, p1.y, p3.y, scanline);
            let y2 = lerp_coords(p2.tex_y, p3.tex_y, p1.y, p3.y, scanline);
            let x1 = lerp_coords(p1.tex_x, p3.tex_x, p1.y, p3.y, scanline);
            let x2 = lerp_coords(p2.tex_x, p3.tex_x, p1.y, p3.y, scanline);
            self.draw_horizontal_line_textured(
                (curx1 + 0.5) as i16,
                (curx2 + 0.5) as i16,
                scanline,
                y1,
                y2,
                x1,
                x2,
                transparent,
            );
            curx1 = curx1 - invslope1;
            curx2 = curx2 - invslope2;
        }
    }

    fn draw_solid_triangle(&mut self, points: &[Point], fill: u16, transparent: bool) {
        let mut sp = points.to_vec();
        sp.sort_by_key(|p| p.y);

        if sp[1].y == sp[2].y {
            self.draw_solid_flat_bottom_triangle(sp[0], sp[1], sp[2], fill, transparent);
        } else if sp[0].y == sp[1].y {
            self.draw_solid_flat_top_triangle(sp[0], sp[1], sp[2], fill, transparent);
        } else {
            let bound_x = (sp[0].x
                + ((sp[1].y - sp[0].y) as f32 / (sp[2].y - sp[0].y) as f32) as i16
                    * (sp[2].x - sp[0].x)) as i16;
            let bound_point = Point::from_components(bound_x, sp[1].y, 0);
            self.draw_solid_flat_bottom_triangle(sp[0], sp[1], bound_point, fill, transparent);
            self.draw_solid_flat_top_triangle(sp[1], bound_point, sp[2], fill, transparent);
        }
    }

    fn draw_shaded_triangle(&mut self, points: &[Point], transparent: bool) {
        let mut sp = points.to_vec();
        sp.sort_by_key(|p| p.y);

        if sp[1].y == sp[2].y {
            self.draw_shaded_flat_bottom_triangle(sp[0], sp[1], sp[2], transparent);
        } else if sp[0].y == sp[1].y {
            self.draw_shaded_flat_top_triangle(sp[0], sp[1], sp[2], transparent);
        } else {
            let bound_x = (sp[0].x
                + ((sp[1].y - sp[0].y) as f32 / (sp[2].y - sp[0].y) as f32) as i16
                    * (sp[2].x - sp[0].x)) as i16;
            let bound_point = Point::from_components(bound_x, sp[1].y, sp[2].color);
            self.draw_shaded_flat_bottom_triangle(sp[0], bound_point, sp[1], transparent);
            self.draw_shaded_flat_top_triangle(sp[1], bound_point, sp[2], transparent);
        }
    }

    fn draw_textured_triangle(&mut self, points: &[Point], transparent: bool) {
        let mut sp = points.to_vec();
        sp.sort_by_key(|p| p.y);

        if sp[1].y == sp[2].y {
            self.draw_textured_flat_bottom_triangle(sp[0], sp[1], sp[2], transparent);
        } else if sp[0].y == sp[1].y {
            self.draw_textured_flat_top_triangle(sp[0], sp[1], sp[2], transparent);
        } else {
            let progress =
                sp[0].x + ((sp[1].y - sp[0].y) as f32 / (sp[2].y - sp[0].y) as f32) as i16;
            let bound_x = progress * ((sp[2].x - sp[0].x) as i16);
            let bound_point = Point {
                x: bound_x,
                y: sp[1].y,
                color: 0,
                tex_x: lerp_coords(sp[0].tex_x, sp[1].tex_x, sp[0].y, sp[1].y, progress),
                tex_y: lerp_coords(sp[0].tex_y, sp[1].tex_y, sp[0].y, sp[1].y, progress),
            };

            self.draw_textured_flat_bottom_triangle(sp[0], bound_point, sp[1], transparent);
            self.draw_textured_flat_top_triangle(sp[1], bound_point, sp[2], transparent);
        }
    }

    fn draw_solid_quad(&mut self, points: &[Point], fill: u16, transparent: bool) {
        self.draw_solid_triangle(&points[0..3], fill, transparent);
        self.draw_solid_triangle(&points[1..4], fill, transparent);
    }

    fn draw_shaded_quad(&mut self, points: &[Point], transparent: bool) {
        self.draw_shaded_triangle(&points[0..3], transparent);
        self.draw_shaded_triangle(&points[1..4], transparent);
    }

    fn draw_textured_quad(&mut self, points: &[Point], transparent: bool) {
        self.draw_textured_triangle(&points[0..3], transparent);
        self.draw_textured_triangle(&[points[1], points[3], points[2]], transparent);
    }

    fn get_texel(&self, x: i16, y: i16) -> u16 {
        //TODO inline variables. Just did this because I'm lazy
        let page_x = self.texpage_x_base;
        let page_y = self.texpage_y_base;
        let clut_x = self.palette_x;
        let clut_y = self.palette_y;
        let size = self.texmode;

        let pixel_val = match size {
            TextureColorMode::FifteenBit => {
                self.vram[point_to_address(
                    ((page_x * 64) as u32 + x as u32) as u32,
                    ((page_y * 256) as u32 + y as u32) as u32,
                ) as usize]
            }
            TextureColorMode::EightBit => {
                let value = self.vram[point_to_address(
                    (page_x * 64) as u32 + (x / 2) as u32,
                    (page_y * 256) as u32 + y as u32,
                ) as usize];
                let clut_index = (value >> (x % 2) * 8) & 0xF;
                self.vram
                    [point_to_address((clut_x * 16 + clut_index) as u32, clut_y as u32) as usize]
            }
            TextureColorMode::FourBit => {
                let value = self.vram[(point_to_address(
                    (page_x * 64) as u32 + (x / 4) as u32,
                    (page_y * 256) as u32 + y as u32,
                ) as usize) % 524288];
                let clut_index = (value >> (x % 4) * 4) & 0xF;
                self.vram
                    [point_to_address((clut_x * 16 + clut_index) as u32, clut_y as u32) as usize]
            }
        };
        if self.blend_enabled {
            pixel_val & self.blend_color
        } else {
            pixel_val
        }
    }
}

fn point_to_address(x: u32, y: u32) -> u32 {
    ((1024) as u32 * y).wrapping_add(x)
}

fn b24color_to_b15color(color: u32) -> u16 {
    let r = ((color >> 16) & 0xFF) / 8;
    let g = ((color >> 8) & 0xFF) / 8;
    let b = (color & 0xFF) / 8;
    ((r << 10) | (g << 5) | b) as u16
}

fn b15_to_rgb(color: u16) -> (u8, u8, u8) {
    (
        ((color >> 10) & 0x1F) as u8,
        ((color >> 5) & 0x1F) as u8,
        (color & 0x1F) as u8,
    )
}

fn rgb_to_b15(r: u8, g: u8, b: u8) -> u16 {
    ((r as u16) << 10) | ((g as u16) << 5) | (b as u16)
}

fn lerp_color(y0: u16, y1: u16, x0: i16, x1: i16, x: i16) -> u16 {
    let (sr, sg, sb) = b15_to_rgb(y0);
    let (er, eg, eb) = b15_to_rgb(y1);

    let ir = (sr as f32 + ((er as i32 - sr as i32) as f32 * ((x - x0) as f32 / (x1 - x0) as f32)))
        as u16;
    let ig = (sg as f32 + ((eg as i32 - sg as i32) as f32 * ((x - x0) as f32 / (x1 - x0) as f32)))
        as u16;
    let ib = (sb as f32 + ((eb as i32 - sb as i32) as f32 * ((x - x0) as f32 / (x1 - x0) as f32)))
        as u16;

    rgb_to_b15(ir as u8, ig as u8, ib as u8)

    //(y0 as f32 + ((y1 - y0) as f32 * ((x - x0) as f32 / (x1 - x0) as f32))) as u16
}

fn lerp_coords(y0: i16, y1: i16, x0: i16, x1: i16, x: i16) -> i16 {
    (y0 as f32 + ((y1 as i32 - y0 as i32) as f32 * ((x - x0) as f32 / (x1 - x0) as f32))) as i16
}

//TODO Make colors more accurate
fn alpha_composite(background_color: u16, alpha_color: u16) -> u16 {
    let (b_r, b_g, b_b) = b15_to_rgb(background_color);
    let (a_r, a_g, a_b) = b15_to_rgb(alpha_color);
    rgb_to_b15(a_r + b_r, a_g + b_g, a_b + b_b)
}

//Helper trait + impl
trait Command {
    fn gp0_header(&self) -> u8;
    fn command(&self) -> u8;
    fn parameter(&self) -> u32;
}

impl Command for u32 {
    fn gp0_header(&self) -> u8 {
        ((self.clone() >> 29) & 0x7) as u8
    }

    fn command(&self) -> u8 {
        ((self.clone() >> 24) & 0xFF) as u8
    }

    fn parameter(&self) -> u32 {
        self.clone() & 0x7FFFFF
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_lerp_color() {
        assert_eq!(15, lerp_color(10, 20, 100, 200, 150));
    }

    #[test]
    fn test_lerp_color_negative() {
        assert_eq!(15, lerp_color(20, 10, 100, 200, 150));
    }
}

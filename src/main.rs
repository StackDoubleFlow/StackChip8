use std::time::{Duration, SystemTime};
use std::{fs, env, thread};
use sdl2::audio::{AudioSpecDesired, AudioQueue};
use sdl2::keyboard::Keycode;
use sdl2::render::Canvas;
use sdl2::pixels::Color;
use sdl2::video::Window;
use sdl2::event::Event;
use sdl2::rect::Rect;
use sdl2::EventPump;
use rand::random;

static SCALE: u32 = 10;
static CHARACTER_SPRITES: [u8; 80] = [
    0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
    0x20, 0x60, 0x20, 0x20, 0x70, // 1
    0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
    0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
    0x90, 0x90, 0xF0, 0x10, 0x10, // 4
    0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
    0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
    0xF0, 0x10, 0x20, 0x40, 0x40, // 7
    0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
    0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
    0xF0, 0x90, 0xF0, 0x90, 0x90, // A
    0xF0, 0x80, 0x80, 0x80, 0xF0, // B
    0xF0, 0x80, 0x80, 0x80, 0xF0, // C
    0xE0, 0x90, 0x90, 0x90, 0xE0, // D
    0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
    0xF0, 0x80, 0xF0, 0x80, 0x80, // F
];

struct Chip8 {
    // Memory and registers
    mem: [u8; 0x1000],
    v: [u8; 16],
    i: u16,
    pc: usize,
    stack: Vec<usize>,
    // Timers
    delay_timer: u32,
    sound_timer: u32,
    last_timer_update: SystemTime,
    // Keyboard
    keys_pressed: [bool; 16],
    // Screen/SDL
    screen: [[bool; 64]; 32],
    canvas: Canvas<Window>,
    event_pump: EventPump,
    audio: AudioQueue<i16>,
    audio_playing: bool,

    running: bool,
}

impl Chip8 {

    fn gen_square_wave(bytes_to_write: i32) -> Vec<i16> {
        let tone_volume = 1_000i16;
        let period = 48_000 / 256;
        let sample_count = bytes_to_write;
        let mut result = Vec::new();

        for x in 0..sample_count {
            result.push(
                if (x / period) % 2 == 0 {
                    tone_volume
                }
                else {
                    -tone_volume
                }
            );
        }
        result
    } 

    fn load(filename: &str) -> Chip8 {
        let data = fs::read(filename).expect("Error reading chip8 rom file");
        let mut mem = [0; 0x1000];
        // Copy rom into Chip8 memory
        mem[0..80].copy_from_slice(&CHARACTER_SPRITES);
        mem[512..(512 + data.len())].copy_from_slice(data.as_slice());

        let sdl_context = sdl2::init().unwrap();
        let video_subsystem = sdl_context.video().unwrap();
        let window = video_subsystem.window("StackChip8", 64 * SCALE, 32 * SCALE).position_centered().build().unwrap();
        let mut canvas = window.into_canvas().build().unwrap();
        canvas.set_scale(SCALE as f32, SCALE as f32).unwrap();
        let event_pump = sdl_context.event_pump().unwrap();

        let audio_spec = AudioSpecDesired {
            freq: Some(48_000),
            channels: Some(2),
            samples: Some(4)
        };
        let audio_subsystem = sdl_context.audio().unwrap();
        let audio = audio_subsystem.open_queue(None, &audio_spec).unwrap();
        audio.queue(&Chip8::gen_square_wave(48_000 * 4));
        Chip8 {
            mem,
            v: [0; 16],
            i: 0,
            pc: 0x200,
            stack: Vec::new(),
            delay_timer: 0,
            sound_timer: 0,
            keys_pressed: [false; 16],
            screen: [[false; 64]; 32],
            running: false,
            last_timer_update: SystemTime::now(),
            event_pump, canvas, audio,
            audio_playing: false
        }
    }

    fn play_sound(&mut self) {
        if self.sound_timer > 0 && !self.audio_playing { self.audio.resume(); self.audio_playing = true };
        if self.sound_timer == 0 && self.audio_playing { self.audio.pause(); self.audio_playing = false };
    }

    fn match_keycode_to_key(keycode: Keycode) -> Option<usize> {
        match keycode {
            Keycode::Num1 => Some(1),
            Keycode::Num2 => Some(2),
            Keycode::Num3 => Some(3),
            Keycode::Num4 => Some(12),
            Keycode::Q => Some(4),
            Keycode::W => Some(5),
            Keycode::E => Some(6),
            Keycode::R => Some(13),
            Keycode::A => Some(7),
            Keycode::S => Some(8),
            Keycode::D => Some(9),
            Keycode::F => Some(14),
            Keycode::Z => Some(10),
            Keycode::X => Some(0),
            Keycode::C => Some(11),
            Keycode::V => Some(15),
            _ => None
        }
    }

    fn render(&mut self) {
        self.canvas.set_draw_color(Color::RGB(0, 0, 0));
        self.canvas.clear();
        self.canvas.set_draw_color(Color::RGB(255, 255, 255));
        for (y, row) in self.screen.iter().enumerate() {
            for (x, pixel) in row.iter().enumerate() {
                if *pixel {
                    let rect = Rect::new(x as i32, y as i32, 1, 1);
                    self.canvas.draw_rect(rect).unwrap();
                }
            }
        }
        self.canvas.present();
    }

    fn poll_events(&mut self) {
        for event in self.event_pump.poll_iter() {
            match event {
                Event::Quit {..} => self.running = false,
                Event::KeyDown { keycode: Some(k), .. } => {
                    let index = Chip8::match_keycode_to_key(k);
                    if let Some(i) = index { self.keys_pressed[i] = true }
                },
                Event::KeyUp { keycode: Some(k), .. } => {
                    let index = Chip8::match_keycode_to_key(k);
                    if let Some(i) = index { self.keys_pressed[i] = false }
                },
                _ => {}
            }
        }
    }

    fn wait_for_key(&mut self) -> u8 {
        loop {
            for event in self.event_pump.poll_iter() {
                match event {
                    Event::KeyDown { keycode: Some(k), .. } => {
                        let index = Chip8::match_keycode_to_key(k);
                        if let Some(i) = index { 
                            self.keys_pressed[i] = true;
                            return i as u8;
                        }
                    },
                    Event::KeyUp { keycode: Some(k), .. } => {
                        let index = Chip8::match_keycode_to_key(k);
                        if let Some(i) = index { self.keys_pressed[i] = false }
                    },
                    Event::Quit { .. } => { self.running = false }
                    _ => {}
                }
            }
            thread::sleep(Duration::from_millis(2));
        }
        
    }

    fn update_timers(&mut self) {
        let time_elapsed = self.last_timer_update.elapsed().unwrap();
        if time_elapsed.as_millis() >= 1000 / 60 {
            if self.delay_timer > 0 { self.delay_timer -= 1 };
            if self.sound_timer > 0 { self.sound_timer -= 1 };
            self.last_timer_update = SystemTime::now();
        }
    }

    fn clear_screen(&mut self) {
        self.screen = [[false; 64]; 32];
    }

    #[allow(clippy::cognitive_complexity)]
    fn step(&mut self) {
        let op = ((self.mem[self.pc] as u16) << 8) | (self.mem[self.pc + 1] as u16);
        println!("PC:{:04X} : Opcode:{:04X} : Delay Timer:{} : Sound Timer:{}", self.pc, op, self.delay_timer, self.sound_timer);
        let nnn = op & 0x0FFF;
        let nn = (op & 0x00FF) as u8;
        let n = op & 0x000F;
        let x = ((op & 0x0F00) >> 8) as usize;
        let y = ((op & 0x00F0) >> 4) as usize;
        match op & 0xF000 {
            0x0000 if op & 0x00FF == 0xE0 => { self.clear_screen(); self.pc += 2 },
            0x0000 if op & 0x00FF == 0xEE => { self.pc = self.stack.pop().unwrap() },
            0x0000 => { panic!("RCA 1802 programs are not supported!") },
            0x1000 => { self.pc = nnn as usize },
            0x2000 => { self.stack.push(self.pc + 2); self.pc = nnn as usize }
            0x3000 => { if self.v[x] == nn { self.pc += 4 } else { self.pc += 2 } }
            0x4000 => { if self.v[x] != nn { self.pc += 4 } else { self.pc += 2 } }
            0x5000 => { if self.v[x] == self.v[y] { self.pc += 4 } else { self.pc += 2 } }
            0x6000 => { self.v[x] = nn; self.pc += 2 }
            0x7000 => { self.v[x] = self.v[x].wrapping_add(nn); self.pc += 2 }
            #[allow(clippy::verbose_bit_mask)]
            0x8000 if op & 0x000F == 0 => { self.v[x] = self.v[y]; self.pc += 2 }
            0x8000 if op & 0x000F == 1 => { self.v[x] |= self.v[y]; self.pc += 2 }
            0x8000 if op & 0x000F == 2 => { self.v[x] &= self.v[y]; self.pc += 2 }
            0x8000 if op & 0x000F == 3 => { self.v[x] ^= self.v[y]; self.pc += 2 }
            0x8000 if op & 0x000F == 4 => {
                let res = self.v[x] as u16 + self.v[y] as u16;
                self.v[0xF] = if res > 255 { 1 } else { 0 };
                self.v[x] = res as u8;
                self.pc += 2;
            }
            0x8000 if op & 0x000F == 5 => {
                let res = self.v[x] as i16 - self.v[y] as i16;
                self.v[0xF] = (res >= 0) as u8;
                self.v[x] = res as u8;
                self.pc += 2;
            }
            0x8000 if op & 0x000F == 6 => { self.v[0xF] = self.v[x] & 0x1; self.v[x] >>= 1; self.pc += 2 }
            0x8000 if op & 0x000F == 7 => {
                let res = self.v[y] as i16 - self.v[x] as i16;
                self.v[0xF] = (res >= 0) as u8;
                self.v[x] = res as u8;
                self.pc += 2;
            }
            0x8000 if op & 0x000F == 0xE => { self.v[0xF] = (self.v[x] & 0x80) >> 7; self.v[x] <<= 1; self.pc += 2 }
            0x9000 => { if self.v[x] != self.v[y] { self.pc += 4 } else { self.pc += 2 } self.pc +=2 }
            0xA000 => { self.i = nnn; self.pc += 2 }
            0xB000 => { self.pc = self.v[0] as usize + nnn as usize; self.pc += 2 }
            0xC000 => { self.v[x] = random::<u8>() & nn; self.pc += 2 }
            0xD000 => {
                let mut collision = false;
                let ypos = self.v[y] as usize;
                let xpos = self.v[x] as usize;
                for sy in (0..n as usize).map(|y| y + ypos) {
                    let wy = if sy >= 32 { sy - 32 } else { sy };
                    for sx in (0..8).map(|x| x + xpos) {
                        let wx = if sx >= 64 { sx - 64 } else { sx };
                        if (self.mem[self.i as usize + (sy - ypos)] & (0x80 >> (sx - xpos))) != 0 {
                            if self.screen[wy][wx] { self.screen[wy][wx] = false; collision = true } else { self.screen[wy][wx] = true }
                        }
                    }
                }
                self.v[0xF] = collision as u8;
                self.pc += 2;
            }
            0xE000 if op & 0x00FF == 0x9E => { if self.keys_pressed[self.v[x] as usize] { self.pc += 4} else { self.pc += 2} }
            0xE000 if op & 0x00FF == 0xA1 => { if !self.keys_pressed[self.v[x] as usize] { self.pc += 4} else { self.pc += 2} }
            0xF000 if op & 0x00FF == 0x07 => { self.v[x] = self.delay_timer as u8; self.pc += 2 }
            0xF000 if op & 0x00FF == 0x0A => { self.v[x] = self.wait_for_key(); self.pc += 2 }
            0xF000 if op & 0x00FF == 0x15 => { self.delay_timer = self.v[x] as u32; self.pc +=2 }
            0xF000 if op & 0x00FF == 0x18 => { self.sound_timer = self.v[x] as u32; self.pc +=2 }
            0xF000 if op & 0x00FF == 0x1E => { 
                self.i += self.v[x as usize] as u16; 
                if self.i > 0xFFF { self.v[0xF] = 1} else { self.v[0xF] = 0 };
                self.pc += 2;
            }
            0xF000 if op & 0x00FF == 0x29 => { self.i = 5 * self.v[x] as u16; self.pc += 2 }
            0xF000 if op & 0x00FF == 0x33 => { 
                self.mem[self.i as usize] = (self.v[x] / 100) as u8;
                self.mem[self.i as usize + 1] = ((self.v[x] % 100) / 10) as u8;
                self.mem[self.i as usize + 2] = (self.v[x] % 10) as u8;
                self.pc += 2;
            }
            0xF000 if op & 0x00FF == 0x55 => { for o in 0..x { self.mem[self.i as usize + o] = self.v[o] } self.pc += 2 }
            0xF000 if op & 0x00FF == 0x65 => { for o in 0..x { self.v[o] = self.mem[self.i as usize + o] } self.pc += 2 }
            _ => { panic!("Unsupported opcode: {:04X}", op) }
        }
    }

    fn run(&mut self) {
        self.running = true;
        while self.running {
            self.poll_events();
            self.update_timers();
            self.play_sound();
            self.step();
            self.render();
            thread::sleep(Duration::from_millis(2));
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        panic!("Expecting an argument specifying a chip8 rom file!");
    }
    println!("Reading file: {}", args[1]);
    let mut chip8 = Chip8::load(&args[1]);
    chip8.run();
}
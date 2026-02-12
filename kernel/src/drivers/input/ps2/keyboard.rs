extern crate alloc;
use crate::drivers::{CharDevice, Device, DeviceError, DeviceInner, DeviceType, SharedDeviceOps};
use alloc::string::String;
use alloc::sync::Arc;
use pc_keyboard::{
    layouts, DecodedKey, HandleControl, KeyCode, KeyState, Keyboard as PcKeyboard, ScancodeSet1,
};
use ringbuf::{traits::*, HeapRb};
use spin::Mutex;

// Buffer size (increase to 256 bytes)
const BUFFER_SIZE: usize = 256;

// Keyboard controller ports
const KEYBOARD_DATA_PORT: u16 = 0x60;
const KEYBOARD_STATUS_PORT: u16 = 0x64;
const KEYBOARD_COMMAND_PORT: u16 = 0x64;

// LED control bits
const LED_SCROLL_LOCK: u8 = 0x01;
const LED_NUM_LOCK: u8 = 0x02;
const LED_CAPS_LOCK: u8 = 0x04;

// ioctl commands definitions
pub const KDGETLED: u64 = 0x4B31;
pub const KDSETLED: u64 = 0x4B32;
pub const KDGKBLED: u64 = 0x4B64;
pub const KDSKBLED: u64 = 0x4B65;
pub const KDGKBMODE: u64 = 0x4B44;
pub const KDSKBMODE: u64 = 0x4B45;
pub const KB_ENABLE: u64 = 0x4B36;
pub const KB_DISABLE: u64 = 0x4B37;
pub const KB_CLEAR_BUFFER: u64 = 0x4B38;

// Keyboard modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardMode {
    Raw = 0,
    MediumRaw = 1,
    Unicode = 2,
}

// Keyboard layout types (reserved for future extensions)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardLayout {
    Us104,
}

// Modifier key states
#[derive(Debug, Clone, Copy, Default)]
pub struct ModifierState {
    pub left_shift: bool,
    pub right_shift: bool,
    pub left_ctrl: bool,
    pub right_ctrl: bool,
    pub left_alt: bool,
    pub right_alt: bool,
    pub left_gui: bool,
    pub right_gui: bool,
    pub caps_lock: bool,
    pub num_lock: bool,
    pub scroll_lock: bool,
}

impl ModifierState {
    pub fn shift(&self) -> bool {
        self.left_shift || self.right_shift
    }

    pub fn ctrl(&self) -> bool {
        self.left_ctrl || self.right_ctrl
    }

    pub fn alt(&self) -> bool {
        self.left_alt || self.right_alt
    }

    pub fn gui(&self) -> bool {
        self.left_gui || self.right_gui
    }
}

// Keyboard LED states
#[derive(Debug, Clone, Copy, Default)]
pub struct LedState {
    pub scroll_lock: bool,
    pub num_lock: bool,
    pub caps_lock: bool,
}

impl LedState {
    pub fn to_bits(&self) -> u8 {
        let mut bits = 0u8;
        if self.scroll_lock {
            bits |= LED_SCROLL_LOCK;
        }
        if self.num_lock {
            bits |= LED_NUM_LOCK;
        }
        if self.caps_lock {
            bits |= LED_CAPS_LOCK;
        }
        bits
    }

    pub fn from_bits(bits: u8) -> Self {
        Self {
            scroll_lock: (bits & LED_SCROLL_LOCK) != 0,
            num_lock: (bits & LED_NUM_LOCK) != 0,
            caps_lock: (bits & LED_CAPS_LOCK) != 0,
        }
    }
}

// Keyboard inner state
pub struct KeyboardInner {
    pc_keyboard: PcKeyboard<layouts::Us104Key, ScancodeSet1>,
    enabled: bool,
    nonblocking: bool,
    modifiers: ModifierState,
    leds: LedState,
    mode: KeyboardMode,
}

/// Send a command to the keyboard controller
fn send_keyboard_command(command: u8, data: u8) {
    use x86_64::instructions::port::Port;

    unsafe {
        // Wait for keyboard controller to be ready
        let mut status_port = Port::<u8>::new(KEYBOARD_STATUS_PORT);
        while (status_port.read() & 0x02) != 0 {
            core::hint::spin_loop();
        }

        // Send command
        let mut command_port = Port::<u8>::new(KEYBOARD_COMMAND_PORT);
        command_port.write(command);

        // Wait for keyboard controller to be ready
        while (status_port.read() & 0x02) != 0 {
            core::hint::spin_loop();
        }

        // Send data
        let mut data_port = Port::<u8>::new(KEYBOARD_DATA_PORT);
        data_port.write(data);
    }
}

pub struct Keyboard {
    inner: Mutex<KeyboardInner>,
    producer: Mutex<ringbuf::wrap::caching::Caching<Arc<HeapRb<char>>, true, false>>,
    consumer: Mutex<ringbuf::wrap::caching::Caching<Arc<HeapRb<char>>, false, true>>,
    name: String,
}

impl KeyboardInner {
    fn new() -> Self {
        Self {
            pc_keyboard: PcKeyboard::new(
                ScancodeSet1::new(),
                layouts::Us104Key,
                HandleControl::Ignore,
            ),
            enabled: true,
            nonblocking: false,
            modifiers: ModifierState::default(),
            leds: LedState::default(),
            mode: KeyboardMode::Unicode,
        }
    }

    /// Update LED states and send to keyboard controller
    fn update_leds(&mut self) {
        self.leds.caps_lock = self.modifiers.caps_lock;
        self.leds.num_lock = self.modifiers.num_lock;
        self.leds.scroll_lock = self.modifiers.scroll_lock;

        // Send LED states to keyboard controller
        let led_bits = self.leds.to_bits();
        send_keyboard_command(0xED, led_bits);
    }

    /// Handle modifier key state changes
    fn update_modifier(&mut self, key_code: KeyCode, state: KeyState) {
        let pressed = state == KeyState::Down;

        match key_code {
            KeyCode::LShift => self.modifiers.left_shift = pressed,
            KeyCode::RShift => self.modifiers.right_shift = pressed,
            KeyCode::LControl => self.modifiers.left_ctrl = pressed,
            KeyCode::RControl => self.modifiers.right_ctrl = pressed,
            KeyCode::LAlt => self.modifiers.left_alt = pressed,
            KeyCode::RAltGr => self.modifiers.right_alt = pressed,
            KeyCode::LWin => self.modifiers.left_gui = pressed,
            KeyCode::RWin => self.modifiers.right_gui = pressed,
            KeyCode::CapsLock if pressed => {
                self.modifiers.caps_lock = !self.modifiers.caps_lock;
                self.update_leds();
            }
            KeyCode::NumpadLock if pressed => {
                self.modifiers.num_lock = !self.modifiers.num_lock;
                self.update_leds();
            }
            KeyCode::ScrollLock if pressed => {
                self.modifiers.scroll_lock = !self.modifiers.scroll_lock;
                self.update_leds();
            }
            _ => {}
        }
    }
}

impl Default for Keyboard {
    fn default() -> Self {
        let rb = HeapRb::<char>::new(BUFFER_SIZE);
        let (prod, cons) = rb.split();
        Self {
            inner: Mutex::new(KeyboardInner::new()),
            producer: Mutex::new(prod),
            consumer: Mutex::new(cons),
            name: String::from("keyboard"),
        }
    }
}

impl Keyboard {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_enabled(&self, enabled: bool) {
        x86_64::instructions::interrupts::without_interrupts(|| {
            self.inner.lock().enabled = enabled;
        });
    }

    pub fn is_enabled(&self) -> bool {
        x86_64::instructions::interrupts::without_interrupts(|| self.inner.lock().enabled)
    }

    pub fn set_nonblocking(&self, nonblocking: bool) {
        x86_64::instructions::interrupts::without_interrupts(|| {
            self.inner.lock().nonblocking = nonblocking;
        });
    }

    pub fn is_nonblocking(&self) -> bool {
        x86_64::instructions::interrupts::without_interrupts(|| self.inner.lock().nonblocking)
    }

    /// Push a character to the keyboard buffer
    fn push_char(&self, c: char) {
        let mut producer = self.producer.lock();
        // When buffer is full, drop the character if non-blocking
        // We think 256 bytes is enough for a keyboard buffer
        let _ = producer.try_push(c);
    }

    pub fn handle_scancode(&self, scancode: u8) {
        let mut key_to_push = None;
        {
            let mut inner = self.inner.lock();
            if !inner.enabled {
                return;
            }

            if let Ok(Some(key_event)) = inner.pc_keyboard.add_byte(scancode) {
                // Update modifier key states
                inner.update_modifier(key_event.code, key_event.state);

                if let Some(key) = inner.pc_keyboard.process_keyevent(key_event) {
                    match inner.mode {
                        KeyboardMode::Unicode => {
                            if let DecodedKey::Unicode(character) = key {
                                key_to_push = Some(character);
                            }
                        }
                        KeyboardMode::Raw => {
                            // Raw mode: push scancode as character
                            key_to_push = Some(scancode as char);
                        }
                        KeyboardMode::MediumRaw => {
                            // MediumRaw mode: push processed key code as character
                            if let DecodedKey::RawKey(key_code) = key {
                                key_to_push = Some((key_code as u8) as char);
                            }
                        }
                    }
                }
            }
        }

        if let Some(c) = key_to_push {
            self.push_char(c);
        }
    }

    pub fn get_modifier_state(&self) -> ModifierState {
        x86_64::instructions::interrupts::without_interrupts(|| self.inner.lock().modifiers)
    }

    pub fn set_led_state(&self, leds: LedState) {
        x86_64::instructions::interrupts::without_interrupts(|| {
            let mut inner = self.inner.lock();
            inner.leds = leds;
            inner.modifiers.scroll_lock = leds.scroll_lock;
            inner.modifiers.num_lock = leds.num_lock;
            inner.modifiers.caps_lock = leds.caps_lock;
            inner.update_leds();
        });
    }

    pub fn get_led_state(&self) -> LedState {
        x86_64::instructions::interrupts::without_interrupts(|| self.inner.lock().leds)
    }

    pub fn clear_buffer(&self) {
        x86_64::instructions::interrupts::without_interrupts(|| {
            let mut consumer = self.consumer.lock();
            while consumer.try_pop().is_some() {}
        });
    }

    pub fn set_mode(&self, mode: KeyboardMode) {
        x86_64::instructions::interrupts::without_interrupts(|| {
            self.inner.lock().mode = mode;
        });
    }

    pub fn get_mode(&self) -> KeyboardMode {
        x86_64::instructions::interrupts::without_interrupts(|| self.inner.lock().mode)
    }

    pub fn create_device() -> Device {
        Device::new_auto_assign(KEYBOARD.name.clone(), DeviceInner::Char(KEYBOARD.clone()))
    }
}

impl SharedDeviceOps for Keyboard {
    fn name(&self) -> &str {
        &self.name
    }

    fn device_type(&self) -> DeviceType {
        DeviceType::Char
    }

    fn open(&self) -> Result<(), DeviceError> {
        Ok(())
    }

    fn close(&self) -> Result<(), DeviceError> {
        Ok(())
    }

    fn ioctl(&self, cmd: u64, arg: u64) -> Result<u64, DeviceError> {
        match cmd {
            KDGETLED => {
                let leds = self.get_led_state();
                Ok(leds.to_bits() as u64)
            }
            KDSETLED => {
                let leds = LedState::from_bits(arg as u8);
                self.set_led_state(leds);
                Ok(0)
            }
            KDGKBLED => {
                let leds = self.get_led_state();
                Ok(leds.to_bits() as u64)
            }
            KDSKBLED => {
                let leds = LedState::from_bits(arg as u8);
                self.set_led_state(leds);
                Ok(0)
            }
            KDGKBMODE => {
                let mode = self.get_mode();
                Ok(mode as u64)
            }
            KDSKBMODE => {
                let mode = match arg {
                    0 => KeyboardMode::Raw,
                    1 => KeyboardMode::MediumRaw,
                    2 => KeyboardMode::Unicode,
                    _ => return Err(DeviceError::InvalidParam),
                };
                self.set_mode(mode);
                Ok(0)
            }
            KB_ENABLE => {
                self.set_enabled(true);
                Ok(0)
            }
            KB_DISABLE => {
                self.set_enabled(false);
                Ok(0)
            }
            KB_CLEAR_BUFFER => {
                self.clear_buffer();
                Ok(0)
            }
            _ => Err(DeviceError::NotSupported),
        }
    }
}

impl CharDevice for Keyboard {
    fn read(&self, buf: &mut [u8]) -> Result<usize, DeviceError> {
        x86_64::instructions::interrupts::without_interrupts(|| {
            let nonblocking = self.inner.lock().nonblocking;
            let mut consumer = self.consumer.lock();

            // Non-blocking mode: return WouldBlock if buffer is empty
            if nonblocking && consumer.is_empty() {
                return Err(DeviceError::WouldBlock);
            }

            let mut read_count = 0;

            while read_count < buf.len() && !consumer.is_empty() {
                // Peek the front character
                let (s1, s2): (&[char], &[char]) = consumer.as_slices();
                let c = if let Some(&c) = s1.first().or_else(|| s2.first()) {
                    c
                } else {
                    break;
                };

                let mut char_buf = [0u8; 4];
                let char_str = c.encode_utf8(&mut char_buf);
                let bytes = char_str.as_bytes();

                if read_count + bytes.len() <= buf.len() {
                    buf[read_count..read_count + bytes.len()].copy_from_slice(bytes);
                    read_count += bytes.len();
                    // Successfully read, pop it
                    consumer.try_pop();
                } else {
                    // Buffer space not enough, stop reading
                    break;
                }
            }

            if read_count == 0 && !buf.is_empty() {
                Err(DeviceError::WouldBlock)
            } else {
                Ok(read_count)
            }
        })
    }

    fn write(&self, _buf: &[u8]) -> Result<usize, DeviceError> {
        Err(DeviceError::NotSupported)
    }

    fn peek(&self, buf: &mut [u8]) -> Result<usize, DeviceError> {
        x86_64::instructions::interrupts::without_interrupts(|| {
            let consumer = self.consumer.lock();

            if consumer.is_empty() {
                return Err(DeviceError::WouldBlock);
            }

            let mut read_count = 0;

            // Use `as_slices` to get two slices of the consumer buffer
            let (s1, s2): (&[char], &[char]) = consumer.as_slices();
            for &c in s1.iter().chain(s2.iter()) {
                let mut char_buf = [0u8; 4];
                let char_str = c.encode_utf8(&mut char_buf);
                let bytes = char_str.as_bytes();

                if read_count + bytes.len() <= buf.len() {
                    buf[read_count..read_count + bytes.len()].copy_from_slice(bytes);
                    read_count += bytes.len();
                } else {
                    break;
                }
            }

            Ok(read_count)
        })
    }

    fn has_data(&self) -> bool {
        x86_64::instructions::interrupts::without_interrupts(|| !self.consumer.lock().is_empty())
    }

    fn set_nonblocking(&self, nonblocking: bool) -> Result<(), DeviceError> {
        self.set_nonblocking(nonblocking);
        Ok(())
    }
}

lazy_static::lazy_static! {
    pub static ref KEYBOARD: Arc<Keyboard> = Arc::new(Keyboard::new());
}

extern crate alloc;
use crate::drivers::{CharDevice, Device, DeviceError, DeviceInner, DeviceType, SharedDeviceOps};
use alloc::string::String;
use alloc::sync::Arc;
use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard as PcKeyboard, ScancodeSet1};
use spin::Mutex;

const BUFFER_SIZE: usize = 128;

pub struct KeyboardInner {
    pc_keyboard: PcKeyboard<layouts::Us104Key, ScancodeSet1>,
    enabled: bool,
    buffer: [char; BUFFER_SIZE],
    head: usize,
    tail: usize,
}

pub struct Keyboard {
    inner: Mutex<KeyboardInner>,
    name: String,
}

impl Default for Keyboard {
    fn default() -> Self {
        Self {
            inner: Mutex::new(KeyboardInner {
                pc_keyboard: PcKeyboard::new(
                    ScancodeSet1::new(),
                    layouts::Us104Key,
                    HandleControl::Ignore,
                ),
                enabled: true,
                buffer: ['\0'; BUFFER_SIZE],
                head: 0,
                tail: 0,
            }),
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

    pub fn handle_scancode(&self, scancode: u8) {
        let mut inner = self.inner.lock();
        if !inner.enabled {
            return;
        }

        if let Ok(Some(key_event)) = inner.pc_keyboard.add_byte(scancode) {
            if let Some(key) = inner.pc_keyboard.process_keyevent(key_event) {
                match key {
                    DecodedKey::Unicode(character) => {
                        let tail = inner.tail;
                        let next_tail = (tail + 1) % BUFFER_SIZE;
                        if next_tail != inner.head {
                            inner.buffer[tail] = character;
                            inner.tail = next_tail;
                        }
                    }
                    DecodedKey::RawKey(_) => {}
                }
            }
        }
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

    fn ioctl(&self, _cmd: u64, _arg: u64) -> Result<u64, DeviceError> {
        Err(DeviceError::NotSupported)
    }
}

impl CharDevice for Keyboard {
    fn read(&self, buf: &mut [u8]) -> Result<usize, DeviceError> {
        x86_64::instructions::interrupts::without_interrupts(|| {
            let mut inner = self.inner.lock();
            let mut read_count = 0;

            while read_count < buf.len() && inner.head != inner.tail {
                let c = inner.buffer[inner.head];
                inner.head = (inner.head + 1) % BUFFER_SIZE;

                let mut char_buf = [0u8; 4];
                let char_str = c.encode_utf8(&mut char_buf);
                let bytes = char_str.as_bytes();

                if read_count + bytes.len() <= buf.len() {
                    buf[read_count..read_count + bytes.len()].copy_from_slice(bytes);
                    read_count += bytes.len();
                } else {
                    inner.head = (inner.head + BUFFER_SIZE - 1) % BUFFER_SIZE;
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

    fn has_data(&self) -> bool {
        x86_64::instructions::interrupts::without_interrupts(|| {
            let inner = self.inner.lock();
            inner.head != inner.tail
        })
    }
}

lazy_static::lazy_static! {
    pub static ref KEYBOARD: Arc<Keyboard> = Arc::new(Keyboard::new());
}

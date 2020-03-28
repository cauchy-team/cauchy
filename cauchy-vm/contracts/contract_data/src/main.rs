// #![no_std]
// #[cfg_attr(target_arch = "wasm32", no_main)]
// #![no_main]
use core::panic::PanicInfo;
use std::convert::TryInto;
// #[panic_handler]
// fn panic(_info: &PanicInfo) -> ! {
//     loop {}
// }

static mut MY_DATA: Vec<u32> = Vec::new();

// #[cfg(not(target_arch = "wasm32"))]
// #[no_mangle]
fn main() {}

#[no_mangle]
pub extern "C" fn init() -> u32 {
    let reader = VMDataReader::new(DataType::ContractData);
    let data: Vec<u8> = reader.collect();
    u32::from_le_bytes(data[..4].try_into().unwrap())
}

#[no_mangle]
pub extern "C" fn inbox() -> u32 {
    0xFF
}

#[test]
fn test_init() {
    let res = init();
    assert!(res > 0);
}

pub enum DataType {
    ContractData,
    MsgData,
}

pub struct VMDataReader {
    cur_offset: u32,
    end_offset: u32,
    base_addr: u32,
}

impl VMDataReader {
    pub fn new(data: DataType) -> Self {
        let base_addr = match data {
            DataType::ContractData => 0x1u32 << 31,
            DataType::MsgData => 0x1u32 << 30,
        };
        unsafe {
            // The end_offset is the size + 4 bytes for the size
            let end_offset = *(base_addr as *const u32) + 4;
            VMDataReader {
                cur_offset: 4u32,
                end_offset,
                base_addr,
            }
        }
    }
}

impl Iterator for VMDataReader {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            if self.cur_offset < self.end_offset {
                let ptr = (self.base_addr + self.cur_offset) as *const u8;
                self.cur_offset += 1;
                Some(*ptr)
            } else {
                None
            }
        }
    }
}

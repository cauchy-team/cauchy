// #![no_std]
// #![no_main]
fn main() {}

#[no_mangle]
extern "C" fn init() -> u32 {
    let addr = (1 << 31);
    unsafe { *(addr as *const u32) }
}
#[no_mangle]
extern "C" fn inbox() -> u32 {
    let addr = (1 << 31);
    unsafe { *(addr as *const u32) }
}

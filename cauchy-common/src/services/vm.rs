pub enum VMSpawnError {
    Spawn(VMError),
}

#[derive(Debug)]
pub enum VMError {
    BadStatus(u32),
    Unknown,
}

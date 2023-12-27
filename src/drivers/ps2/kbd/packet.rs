pub struct KeyPacket {
    pub code: KeyCode,
    pub code_point: char,
    pub new_state: KeyState,
}

pub enum KeyState {
    Pressed,
    Released,
}

pub enum CodePoint {}

pub enum KeyCode {}

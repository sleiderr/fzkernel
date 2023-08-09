
pub fn code_to_char(scan_code : u8) -> u8{
    let char =
    match scan_code {
        0x10 => b"a",
        0x11 => b"z",
        0x12 => b"e",
        0x13 => b"r",
        0x14 => b"t",
        0x15 => b"y",
        0x16 => b"u",
        0x17 => b"i",
        0x18 => b"o",
        0x19 => b"p",
        0x1a => b"^",
        0x1b => b"$",
        0x1c => b"\n",
        0x1e => b"q",
        0x1f => b"s",
        0x20 => b"d",
        0x21 => b"f",
        0x22 => b"g",
        0x23 => b"h",
        0x24 => b"j",
        0x25 => b"k",
        0x26 => b"l",
        0x27 => b"m",
        0x28 => b"\0",
        0x29 => b"<",
        0x2c => b"w",
        0x2d => b"x",
        0x2e => b"c",
        0x2f => b"v",
        0x30 => b"b",
        0x31 => b"n",
        0x32 => b",",
        0x33 => b";",
        0x34 => b":",
        0x35 => b"=",
        _ => b"\0"
    };
    *char.get(0).unwrap()
}
use std::error::Error;
use std::path::Path;
use std::fs;
use std::io::{self, Read, Write, Seek, SeekFrom, BufWriter, BufReader};
use std::vec::Vec;



fn write_to_disk(path: &Path) -> Result<(), Box<dyn Error>> {

    let disk_img = fs::OpenOptions::new()
        .write(true)
        .truncate(false)
        .create(true)
        .open(path)?;

    let mut writer = BufWriter::new(&disk_img);

    println!("Burn MBR to disk image");

    let mbr = fs::File::open("target/f-initmbr/x86_64-fbios/release/f-initmbr.bin")?;
    let buff = BufReader::new(mbr)
        .bytes()
        .collect::<io::Result<Vec<u8>>>()?;
    writer.seek(SeekFrom::Start(0))?;
    writer.write_all(&buff)?;

    println!("MBR done !");

    println!("Burn bootloader to disk image");
    let mbr = fs::File::open("target/f-init/x86_64-fbios/release/f-init.bin")?;
    let buff = BufReader::new(mbr)
        .bytes()
        .collect::<io::Result<Vec<u8>>>()?;
    writer.write_all(&buff)?;

    println!("Bootloader done !");

    Ok(())

}


fn main() {

    let disk_path: &Path = Path::new("./boot.img");
    write_to_disk(disk_path).unwrap();

}

use std::error::Error;
use std::fs;
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::vec::Vec;

fn write_to_disk(path: &Path) -> Result<(), Box<dyn Error>> {
    let disk_img = fs::OpenOptions::new()
        .write(true)
        .truncate(false)
        .create(true)
        .open(path)?;

    let mut writer = BufWriter::new(&disk_img);

    println!("Burn MBR to disk image");

    let mbr = fs::File::open("target/mbr/x86_64-fbios/release/mbr.bin")?;
    let buff = BufReader::new(mbr)
        .bytes()
        .collect::<io::Result<Vec<u8>>>()?;
    writer.seek(SeekFrom::Start(0))?;
    writer.write_all(&buff)?;

    println!("MBR done !");

    println!("Burn bootloader to disk image");
    let mbr = fs::File::open("target/real/x86_64-fbios/release/real.bin")?;
    let buff = BufReader::new(mbr)
        .bytes()
        .collect::<io::Result<Vec<u8>>>()?;
    writer.write_all(&buff)?;

    let mbr = fs::File::open("target/main/x86_64-fbios/release/main.bin")?;
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

#[cfg(test)]
mod test {
    use external_tests::main;
    #[main(
        path = "/tmp/fp_tests",
        arch = "i386:x86-64",
        gdb = "gdb",
        port = "localhost:1234",
        symfile = "target/main/x86_64-fbios/release/main",
        bootfile = "boot.img"
    )]
    #[external_tests::tokio::test]
    pub async fn main() {}
}

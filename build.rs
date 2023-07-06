use llvm_tools::{exe, LlvmTools};
use std::env;
use std::path::Path;
use std::process::Command;

fn main() {
    let cargo_path = env::var("CARGO").expect("Missing CARGO environment variable");
    let cargo = Path::new(&cargo_path);
    let llvm_tools = LlvmTools::new().expect("LLVM tools not found");
    let objcopy = llvm_tools
        .tool(&exe("llvm-objcopy"))
        .expect("llvm-objcopy not found");

    let manifest_dir_path =
        env::var("CARGO_MANIFEST_DIR").expect("Missing CARGO_MANIFEST_DIR environment variable");
    let manifest_dir = Path::new(&manifest_dir_path);
    let current_dir = env::current_dir().expect("Couldn't get current directory");
    let target_dir_rel = manifest_dir.join("target");
    let target_dir = current_dir.join(target_dir_rel);

    let stage_1st_dir = manifest_dir.join("src/bios/f-initmbr");
    let stage_1st_triple = stage_1st_dir.join("x86_64-fbios.json");
    build_subproject(
        &stage_1st_dir,
        &stage_1st_triple,
        &target_dir,
        &objcopy,
        &cargo,
    );

    let stage_2_dir = manifest_dir.join("src/bios/f-init");
    let stage_2_triple = stage_2_dir.join("x86_64-fbios.json");
    build_subproject(
        &stage_2_dir,
        &stage_2_triple,
        &target_dir,
        &objcopy,
        cargo
    );
}

fn build_subproject(
    subproject_dir: &Path,
    target_triple: &Path,
    root_target_dir: &Path,
    objcopy: &Path,
    cargo: &Path,
) {
    println!("cargo:rerun-if-changed={}", &target_triple.display());
    println!("cargo:rerun-if-changed={}", &subproject_dir.display());
    let subproject_name = subproject_dir
        .file_stem()
        .expect("Couldn't get name")
        .to_str()
        .expect("Invalid string");
    let target_file = Path::new(&target_triple)
        .file_stem()
        .expect("Couldn't get target file stem");
    let target_dir = root_target_dir.join(subproject_name);

    let mut build_cmd = Command::new(cargo);
    build_cmd.current_dir(subproject_dir);
    build_cmd.arg("build").arg("--release");
    build_cmd.arg("-Zbuild-std=core,alloc");
    build_cmd.arg(format!("--target-dir={}", &target_dir.display()));
    build_cmd.arg("--target").arg(target_triple);
    let build_status = build_cmd.status().expect("Build failed");
    assert!(build_status.success(), "Build failed");

    let object_dir = target_dir.join(target_file).join("release");
    let object_path = object_dir.join(subproject_name);
    let binary_path = object_dir.join(subproject_name.to_string() + ".bin");
    let mut objcopy_cmd = Command::new(objcopy);
    objcopy_cmd
        .arg("-I")
        .arg("elf32-i386")
        .arg("-O")
        .arg("binary");
    objcopy_cmd.arg(object_path);
    objcopy_cmd.arg(binary_path);
    let objcopy_status = objcopy_cmd.status().expect("Objcopy failed!");
    assert!(objcopy_status.success(), "Objcopy failed!");
}

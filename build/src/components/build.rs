use crate::errors::BuildError;
use async_trait::async_trait;
use crossbeam::channel::{unbounded, Receiver, Sender};
use gpt::disk::LogicalBlockSize::Lb512;
use llvm_tools::{exe, LlvmTools};
use rayon::prelude::*;
use std::io::{Read, Seek, SeekFrom};
use std::ops::Add;
use std::os::unix::fs::FileExt;
use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, SystemTime},
};
use tokio::{fs::File, io::AsyncWriteExt};

pub enum BuildEvent {
    Update(String),
    StepFinished(String, usize),
    Finished(String, usize),
    StepFailed(String, String),
}

const DEFAULT_DISK_IMAGE_SIZE: u32 = 256 * 1024 * 1024;

pub type BuildResult = Result<(), BuildError>;

#[async_trait]
pub trait BuildStep {
    fn steps_count(&self) -> usize;
    async fn build(&mut self, master: Sender<BuildEvent>) -> BuildResult;
}

pub struct BuildBlueprint<'a> {
    pub steps: Vec<&'a mut dyn BuildStep>,
    pub incoming: Receiver<BuildEvent>,
    outgoing: Sender<BuildEvent>,
}

pub struct BootloaderBuild {
    pub config: BootloaderBuildConfig,
}

pub struct ImageDiskBuild {
    pub config: ImageDiskBuildConfig,
}

impl ImageDiskBuild {
    pub fn new(config: ImageDiskBuildConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl BuildStep for ImageDiskBuild {
    fn steps_count(&self) -> usize {
        1
    }

    async fn build(&mut self, master: Sender<BuildEvent>) -> BuildResult {
        let start = SystemTime::now();
        let mut disk_image = std::fs::File::options()
            .write(true)
            .read(true)
            .truncate(true)
            .create(true)
            .open(&self.config.disk_img)
            .unwrap();

        disk_image
            .set_len(u64::from(DEFAULT_DISK_IMAGE_SIZE))
            .unwrap();

        let mbr = gpt::mbr::ProtectiveMBR::with_lb_size(DEFAULT_DISK_IMAGE_SIZE);
        mbr.update_conservative(&mut disk_image)
            .map_err(|_| BuildError(None))?;
        let mut bootcode = [0u8; 440];
        let mut build_img =
            std::fs::File::open(&self.config.build_img).map_err(|_| BuildError(None))?;

        build_img
            .read(&mut bootcode)
            .map_err(|_| BuildError(None))?;
        gpt::mbr::write_bootcode(&mut disk_image, &bootcode);

        let mut gpt_disk = gpt::GptConfig::default()
            .initialized(false)
            .writable(true)
            .logical_block_size(Lb512)
            .create_from_device(Box::new(&mut disk_image), None)
            .map_err(|_| BuildError(None))?;

        gpt_disk
            .update_partitions(std::collections::BTreeMap::<u32, gpt::partition::Partition>::new())
            .map_err(|_| BuildError(None))?;

        let boot_part_id = gpt_disk
            .add_partition(
                "fzboot",
                1024 * 1024,
                gpt::partition_types::BASIC,
                0,
                Some(128),
            )
            .map_err(|_| BuildError(None))?;

        gpt_disk
            .add_partition(
                "kernelfs",
                1024 * 1024 * 62,
                gpt::partition_types::BASIC,
                0,
                None,
            )
            .map_err(|_| BuildError(None))?;

        gpt_disk
            .add_partition(
                "rootfs",
                1024 * 1024 * 128,
                gpt::partition_types::LINUX_FS,
                0,
                None,
            )
            .map_err(|_| BuildError(None))?;

        let boot_part_start_lba = gpt_disk.partitions().get(&boot_part_id).unwrap().first_lba;
        gpt_disk.write().map_err(|_| BuildError(None))?;

        let mut post_mbr_code = vec![0; (build_img.metadata().unwrap().len() - 0x200) as usize];
        build_img.seek(SeekFrom::Start(0x200));
        build_img
            .read(&mut post_mbr_code)
            .map_err(|_| BuildError(None))?;

        disk_image.write_at(&post_mbr_code, boot_part_start_lba * 0x200);
        master
            .send(BuildEvent::StepFinished(
                String::from("disk image"),
                start.elapsed().unwrap().as_micros() as usize,
            ))
            .unwrap();

        master.send(BuildEvent::Finished(String::from(""), 0));
        Ok(())
    }
}

pub struct ImageDiskBuildConfig {
    pub disk_img: PathBuf,
    pub build_img: PathBuf,
}

pub struct BootloaderBuildConfig {
    disk_img: PathBuf,
    kernel_img: PathBuf,
    bin_parts_path: Vec<PathBuf>,
    src_parts_path: Vec<PathBuf>,
}

impl BootloaderBuild {
    pub fn new(config: BootloaderBuildConfig) -> Self {
        Self { config }
    }

    fn build_fail(&self, master: Sender<BuildEvent>, output: Option<String>) -> BuildError {
        let pkg_version = env!("CARGO_PKG_VERSION");
        let pkg_name = env!("CARGO_PKG_NAME");
        let failure_msg = format!("Failed to build {pkg_name} {pkg_version}");
        let output = output.or(Some(String::from(""))).unwrap();
        master
            .send(BuildEvent::StepFailed(failure_msg, output))
            .unwrap();
        BuildError(None)
    }

    fn build_part(&self, path: &PathBuf) -> Result<(), BuildError> {
        let cargo = env::var("CARGO").expect("Could not locate cargo !");
        let cargo_path = Path::new(&cargo);
        let llvm_tools = LlvmTools::new().expect("Error loading LLVM-tools");
        let objcopy = llvm_tools
            .tool(&exe("llvm-objcopy"))
            .expect("Could not locate LLVM-objcopy");
        let manifest =
            env::var("CARGO_MANIFEST_DIR").expect("Could not load Cargo manifest directory");
        let root_path = Path::new(&manifest);
        let part_name = path
            .file_stem()
            .expect("Could not retrieve part name")
            .to_str()
            .expect("Could not retrieve part name");
        let target_path = root_path.join("target/").join(part_name);
        let target_triple = Path::new("x86_64-fbios.json");

        let mut build = Command::new(cargo_path);
        build.current_dir(root_path.join("../").join(path)).args([
            "build",
            "--release",
            "-Zbuild-std=core,alloc",
            "--target-dir",
            target_path.to_str().expect("Could not parse target path."),
            "--target",
            target_triple
                .to_str()
                .expect("Could not parse target triple path."),
        ]);
        let build_output = build.output().map_err(|_| BuildError(None))?;
        let cargo_output = String::from_utf8_lossy(&build_output.stderr).to_string();

        build_output
            .status
            .exit_ok()
            .map_err(|_| BuildError(Some(cargo_output)))?;

        let build_path = target_path
            .join(target_triple.file_stem().expect(""))
            .join("release");
        let obj_path = build_path.join(part_name);
        let bin_path = build_path.join(part_name.to_owned() + ".bin");
        let mut objcpy_cmd = Command::new(objcopy);
        objcpy_cmd
            .arg("-I")
            .arg("elf32-i386")
            .arg("-O")
            .arg("binary");
        objcpy_cmd.arg(obj_path);
        objcpy_cmd.arg(bin_path);
        objcpy_cmd.status().map_err(|_| {
            BuildError(Some(String::from(
                "Failed to convert object file to binary",
            )))
        })?;

        Ok(())
    }

    async fn write_part_to_img(&self, file: &mut File, path: &Path) -> Result<(), std::io::Error> {
        let part_bin = tokio::fs::read(path).await?;
        file.write_all(part_bin.as_slice()).await?;

        Ok(())
    }
}

#[async_trait]
impl BuildStep for BootloaderBuild {
    fn steps_count(&self) -> usize {
        self.config.bin_parts_path.len() + 1
    }

    async fn build(&mut self, master: Sender<BuildEvent>) -> BuildResult {
        let mut build_img = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .open(self.config.disk_img.as_path())
            .await
            .map_err(|_| BuildError(None))?;

        let mut kernel_img = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .open(self.config.kernel_img.as_path())
            .await
            .map_err(|_| BuildError(None))?;

        let make = Command::new("make")
            .current_dir("../src/x86/real")
            .output()
            .unwrap();

        if !make.status.success() {
            let make_output = String::from_utf8(make.stderr).unwrap();
            master
                .send(BuildEvent::StepFailed(
                    make_output.clone(),
                    String::from_utf8(make.stdout).unwrap().add(&make_output),
                ))
                .map_err(|_| self.build_fail(master.clone(), None))?;
        }

        self.config
            .src_parts_path
            .par_iter()
            .enumerate()
            .try_for_each(|(i, part)| {
                let start = SystemTime::now();
                self.build_part(part)?;
                let duration: Duration = start.elapsed().map_err(|_| BuildError(None))?;
                master
                    .send(BuildEvent::StepFinished(
                        String::from("bootcode"),
                        duration.as_micros() as usize,
                    ))
                    .map_err(|_| self.build_fail(master.clone(), None))?;
                Ok::<(), BuildError>(())
            })
            .map_err(|err| self.build_fail(master.clone(), err.0))?;

        let start = SystemTime::now();

        self.write_part_to_img(&mut build_img, Path::new("artifacts/boot.bin"))
            .await
            .map_err(|_| self.build_fail(master.clone(), None))?;

        for part in &self.config.bin_parts_path {
            if part.to_str().unwrap().contains("kernel") {
                self.write_part_to_img(&mut kernel_img, part)
                    .await
                    .map_err(|_| self.build_fail(master.clone(), None))?;
            } else {
                self.write_part_to_img(&mut build_img, part)
                    .await
                    .map_err(|_| self.build_fail(master.clone(), None))?;
            }
        }

        let duration: Duration = start
            .elapsed()
            .map_err(|_| self.build_fail(master.clone(), None))?;

        master
            .send(BuildEvent::StepFinished(
                String::from("kernel"),
                duration.as_micros() as usize,
            ))
            .map_err(|_| self.build_fail(master.clone(), None))?;

        Ok(())
    }
}

impl BootloaderBuildConfig {
    #[must_use]
    pub fn new(
        kernel_img: String,
        disk_img: String,
        src_root_path: String,
        target_root_path: String,
        parts: Vec<&str>,
    ) -> Self {
        let disk_img_path = PathBuf::from(disk_img);
        let kernel_img_path = PathBuf::from(kernel_img);
        let bin_parts_path: Vec<PathBuf> = parts
            .iter()
            .map(|part| PathBuf::from(target_root_path.replace("$name", part)))
            .collect();

        let src_parts_path: Vec<PathBuf> = parts
            .iter()
            .map(|part| PathBuf::from(src_root_path.replace("$name", part)))
            .collect();

        Self {
            kernel_img: kernel_img_path,
            disk_img: disk_img_path,
            bin_parts_path,
            src_parts_path,
        }
    }
}

impl<'a> Default for BuildBlueprint<'a> {
    fn default() -> Self {
        let (s, r) = unbounded();
        Self {
            steps: vec![],
            incoming: r,
            outgoing: s,
        }
    }
}

impl<'a> BuildBlueprint<'a> {
    pub fn add_step<T>(&'a mut self, step: &'a mut T)
    where
        T: BuildStep,
    {
        self.steps.push(step);
    }

    pub fn get_receiver(&self) -> Receiver<BuildEvent> {
        self.incoming.clone()
    }

    pub async fn build(&mut self) -> BuildResult {
        for step in self.steps.iter_mut() {
            step.build(self.outgoing.clone()).await?;
        }

        Ok(())
    }

    pub fn steps_count(&self) -> usize {
        let mut count = 0;

        for step in &self.steps {
            count += step.steps_count();
        }

        count
    }
}

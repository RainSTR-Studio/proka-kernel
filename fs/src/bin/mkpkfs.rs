//! The tool to create the proka file system.
use clap::Parser;
use proka_fs::definition::{DirEntry, Inode, SuperBlock};
use proka_fs::{BlockDevice, convert_name};
use proka_fs::check_fs_type;
use std::io::{Seek, SeekFrom, Read, Write};
use std::fs::{File, OpenOptions};
use colored::Colorize;

const BLOCK_SIZE: usize = 1024;

// Define CLI args
#[derive(Parser)]
#[command(about = "The ProkaFS creater")]
struct Args {
    /// The path to the file to create.
    #[arg(required = true)]
    path: String,
}

// Implement the block device for the file.
pub struct FileBlockDevice(File);

impl BlockDevice for FileBlockDevice {
    fn read_block(&mut self, block_num: u32, offset: u32, buf: &mut [u8]) -> Result<(), &'static str> {
        self.0.seek(SeekFrom::Start(block_num as u64 * BLOCK_SIZE as u64 + offset as u64))
            .map_err(|_| "Failed to seek to block")?;
        self.0.read_exact(buf).map_err(|_| "Failed to read block")
    }

    fn write_block(&mut self, block_num: u32, offset: u32, buf: &[u8]) -> Result<(), &'static str> {
        self.0.seek(SeekFrom::Start(block_num as u64 * BLOCK_SIZE as u64 + offset as u64))
            .map_err(|_| "Failed to seek to block")?;
        self.0.write_all(buf).map_err(|_| "Failed to write block")
    }
}

fn main() -> Result<(), &'static str> {
    println!("{}: The file system of {}", "ProkaFS (PKFS)".bold(), "ProkaOS".bold());
    println!("mkpkfs {}", "v0.1.0".cyan().bold());
    println!("Copyright (C) {} {year}, All rights reserved.", "RainSTR Studio".cyan().bold(), year = "2025-2026".bold());
    println!();
    /* Prework: Initialize the program */
    // Parse the CLI args.
    let args = Args::parse();
    // Open the file.
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&args.path)
        .map_err(|_| "Failed to open file")?;
    
    // Create the block device.
    let mut bd = FileBlockDevice(file);

    // Decide the data start block
    // If size > 64MB, the data start block is 65536, which can store max 2,097,120 files.
    // otherwise, the data start block is 1024, but only 32768 files.
    if check_fs_type(&mut bd)? == proka_fs::definition::FsType::Standard {
        println!("mkpkfs: [INFO] Detected the device size is {}MB", get_device_size(&args.path)? / 1024 / 1024);
        println!("mkpkfs: [INFO] Will use the Standard mode.");
        65536
    } else {
        println!("mkpkfs: [INFO] Detected the device size is {}MB", get_device_size(&args.path)? / 1024 / 1024);
        println!("mkpkfs: [INFO] Will use the Minimum mode.");
        1024
    };

    /* Stage 1: Initialize the super block */
    println!("mkpkfs: [INFO] Initialize the super block...");
    let super_block = SuperBlock::new(check_fs_type(&mut bd)?);
    bd.write_block(0, 0, super_block.as_bytes())?;

    /* Stage 2: Initialize the root inode */
    println!("mkpkfs: [INFO] Initialize the root inode...");
    let root_inode = Inode {
        inode_id: 0,
        file_type: proka_fs::definition::FileType::Directory,
        head_block: data_start_block,
        file_length: 2 * core::mem::size_of::<DirEntry>() as u64,
        _reserved: [0; 8],
    };
    bd.write_block(1, 0, root_inode.as_bytes())?;

    /* Stage 3: Initialize the root directory's basic information */
    // In this stage, we will create the root directory's basic information.
    // There are 2 dir entry we MUST define:
    // - "."
    // - ".."
    // These entries are pointed at the same directory: root.
    println!("mkpkfs: [INFO] Initialize the root directory's basic information...");

    // 3.1: Define the name of the "." and ".." entries.
    let name_dot = convert_name(b".");
    let name_parent = convert_name(b"..");

    // 3.2: Define the dir entry of the "." and ".." entries.
    let entry_dot = DirEntry {
        inode: 0,
        name: name_dot,
    };
    let entry_parent = DirEntry {
        inode: 0,
        name: name_parent,
    };

    // 3.3: Write the "." and ".." entries to the root directory.
    // 
    // # Note:
    // - The data block starts at block 1024, which is a constant currently.
    bd.write_block(data_start_block, 0, entry_dot.as_bytes())?;
    bd.write_block(data_start_block, core::mem::size_of::<DirEntry>() as u32, entry_parent.as_bytes())?;
    Ok(())
}
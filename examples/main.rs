extern crate gfapi_sys;
extern crate libc;

use std::path::Path;

use gfapi_sys::gluster::*;
use libc::{O_CREAT, O_RDWR, O_TRUNC, O_APPEND, SEEK_SET, timespec};

fn main() {
    let cluster = match Gluster::connect("test", "localhost", 24007) {
        Ok(c) => c,
        Err(e) => {
            println!("connection failed: {:?}", e);
            return;
        }
    };
    match cluster.mkdir(&Path::new("gfapi"), 0644) {
        Ok(_) => println!("mkdir gfapi success"),
        Err(e) => {
            println!("mkdir failed: {:?}", e);
        }

    }
    let file_handle =
        match cluster.create(&Path::new("gfapi/test"), O_CREAT | O_RDWR | O_TRUNC, 0644) {
            Ok(file_handle) => file_handle,
            Err(e) => {
                println!("create file failed: {:?}", e);
                return;
            }
        };


    match file_handle.write(&"hello world".as_bytes(), O_APPEND) {
        Ok(bytes_written) => {
            println!("Wrote {} bytes", bytes_written);
        }
        Err(e) => {
            println!("writing to file failed: {:?}", e);
            return;
        }
    };
    match file_handle.lseek(0, SEEK_SET) {
        Ok(_) => {
            println!("Seek back to 0");
        }
        Err(e) => {
            println!("Seeking in file failed: {:?}", e);
            return;
        }
    };
    let mut read_buff: Vec<u8> = Vec::with_capacity(1024);
    match file_handle.read(&mut read_buff, 1024, 0) {
        Ok(bytes_read) => {
            println!("Read {} bytes", bytes_read);
            read_buff.truncate(bytes_read as usize);
            println!("Contents: {:?}", read_buff);
        }
        Err(e) => {
            println!("writing to file failed: {:?}", e);
            return;
        }
    };

    // Zero out the access and modified times
    println!("Setting access and modified times");
    let file_times = [timespec {
                          tv_sec: 0,
                          tv_nsec: 0,
                      },
                      timespec {
                          tv_sec: 0,
                          tv_nsec: 0,
                      }];
    cluster.utimens(&Path::new("gfapi/test"), &file_times).unwrap();

    let d  = cluster.opendir(&Path::new("gfapi")).unwrap();
    for dir_entry in d {
        println!("Dir_entry: {:?}", dir_entry);
    }
}




use std::path::Path;

use gfapi_sys::gluster::*;
use libc::{timespec, O_APPEND, O_CREAT, O_RDWR, O_TRUNC, SEEK_SET, S_IRWXU};

#[test]
// A simple connect, mkdir, read write ls test.  Should provide a basic level of comfort that
// the bindings are correct.  The gluster we're testing again on travis only has 1 brick so
// any issues around networking or bricks coming up or down won't show up here.
fn integration_test1() {
    println!("Connecting to localhost gluster");
    let cluster = Gluster::connect("test", "localhost", 24007).unwrap();
    println!("Creating a directory");
    cluster.mkdir(&Path::new("gfapi/new_dir"), S_IRWXU).unwrap();
    println!("Creating a test file");
    let file_handle = cluster
        .create(
            &Path::new("gfapi/test"),
            O_CREAT | O_RDWR | O_TRUNC,
            S_IRWXU,
        ).unwrap();
    println!("Writing to test file");
    let bytes_written = file_handle
        .write(&"hello world".as_bytes(), O_APPEND)
        .unwrap();
    println!("Wrote {} bytes to gfapi/test", bytes_written);
    println!("Seeking back to 0");
    file_handle.lseek(0, SEEK_SET).unwrap();
    let mut read_buff: Vec<u8> = Vec::with_capacity(1024);
    println!("Read back test file");
    let bytes_read = file_handle.read(&mut read_buff, 1024, 0).unwrap();
    println!("Read {} bytes from gfapi/test", bytes_read);
    assert_eq!(bytes_written, bytes_read);
    let file_times = [
        timespec {
            tv_sec: 0,
            tv_nsec: 0,
        },
        timespec {
            tv_sec: 0,
            tv_nsec: 0,
        },
    ];
    cluster
        .utimens(&Path::new("gfapi/test"), &file_times)
        .unwrap();
    let d = cluster.opendir(&Path::new("gfapi")).unwrap();
    for dir_entry in d {
        println!("Dir_entry: {:?}", dir_entry);
    }
}

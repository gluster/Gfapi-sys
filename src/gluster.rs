use errno::errno;
use glfs::*;
use libc::{c_void, dev_t, mode_t, stat};

use std::error::Error as err;
use std::mem::zeroed;
use std::ffi::{CStr, CString, IntoStringError, NulError};
use std::io::Error;
use std::string::FromUtf8Error;

/// Custom error handling for the library
#[derive(Debug)]
pub enum GlusterError {
    FromUtf8Error(FromUtf8Error),
    NulError(NulError),
    Error(String),
    IoError(Error),
    IntoStringError(IntoStringError),
}

impl GlusterError {
    /// Create a new GlusterError with a String message
    fn new(err: String) -> GlusterError {
        GlusterError::Error(err)
    }

    /// Convert a GlusterError into a String representation.
    pub fn to_string(&self) -> String {
        match *self {
            GlusterError::FromUtf8Error(ref err) => err.utf8_error().to_string(),
            GlusterError::NulError(ref err) => err.description().to_string(),
            GlusterError::Error(ref err) => err.to_string(),
            GlusterError::IoError(ref err) => err.description().to_string(),
            GlusterError::IntoStringError(ref err) => err.description().to_string(),
        }
    }
}

impl From<NulError> for GlusterError {
    fn from(err: NulError) -> GlusterError {
        GlusterError::NulError(err)
    }
}

impl From<FromUtf8Error> for GlusterError {
    fn from(err: FromUtf8Error) -> GlusterError {
        GlusterError::FromUtf8Error(err)
    }
}
impl From<IntoStringError> for GlusterError {
    fn from(err: IntoStringError) -> GlusterError {
        GlusterError::IntoStringError(err)
    }
}
impl From<Error> for GlusterError {
    fn from(err: Error) -> GlusterError {
        GlusterError::IoError(err)
    }
}

fn get_error() -> String {
    let error = errno();
    format!("{}", error)
}

pub struct Gluster {
    cluster_handle: *mut Struct_glfs,
}

impl Drop for Gluster {
    fn drop(&mut self) {
        if self.cluster_handle.is_null() {
            // No cleanup needed
            return;
        }
        unsafe {
            glfs_fini(self.cluster_handle);
        }
    }
}

impl Gluster {
    /// Connect to a Ceph cluster and return a connection handle glfs_t
    pub fn connect(volume_name: &str, server: &str, port: i32) -> Result<Gluster, GlusterError> {
        let vol_name = try!(CString::new(volume_name));
        let vol_transport = try!(CString::new("tcp"));
        let vol_host = try!(CString::new(server));
        unsafe {
            let cluster_handle = glfs_new(vol_name.as_ptr());
            if cluster_handle.is_null() {
                return Err(GlusterError::new("glfs_new failed".to_string()));
            }
            let ret_code = glfs_set_volfile_server(cluster_handle,
                                                   vol_transport.as_ptr(),
                                                   vol_host.as_ptr(),
                                                   port);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }

            let ret_code = glfs_init(cluster_handle);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
            Ok(Gluster { cluster_handle: cluster_handle })
        }
    }

    /// Disconnect from a Gluster cluster and destroy the connection handle
    /// For clean up, this is only necessary after connect() has succeeded.
    /// Normally there is no need to call this function.  When Rust cleans
    /// up the Gluster struct it will automatically call disconnect
    pub fn disconnect(self) {
        if self.cluster_handle.is_null() {
            // No cleanup needed
            return;
        }
        unsafe {
            glfs_fini(self.cluster_handle);
        }
    }
    pub fn open(&self, path: &str, flags: i32) -> Result<*mut Struct_glfs_fd, GlusterError> {
        let path = try!(CString::new(path));
        unsafe {
            let file_handle = glfs_open(self.cluster_handle, path.as_ptr(), flags);
            Ok(file_handle)
        }
    }
    pub fn create(&self,
                  path: String,
                  flags: i32,
                  mode: mode_t)
                  -> Result<*mut Struct_glfs_fd, GlusterError> {
        let path = try!(CString::new(path));
        unsafe {
            let file_handle = glfs_creat(self.cluster_handle, path.as_ptr(), flags, mode);
            Ok(file_handle)
        }
    }
    pub fn close(file_handle: &mut Struct_glfs_fd) -> Result<(), GlusterError> {
        unsafe {
            let ret_code = glfs_close(file_handle);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn read(file_handle: &mut Struct_glfs_fd,
                fill_buffer: &mut [u8],
                flags: i32)
                -> Result<isize, GlusterError> {
        unsafe {
            let read_size = glfs_read(file_handle,
                                      fill_buffer.as_mut_ptr() as *mut c_void,
                                      fill_buffer.len(),
                                      flags);
            if read_size < 0 {
                return Err(GlusterError::new(get_error()));
            }
            Ok(read_size)

        }

    }
    pub fn write(file_handle: &mut Struct_glfs_fd,
                 buffer: &[u8],
                 flags: i32)
                 -> Result<isize, GlusterError> {
        unsafe {
            let write_size = glfs_write(file_handle,
                                        buffer.as_ptr() as *const c_void,
                                        buffer.len(),
                                        flags);
            if write_size < 0 {
                return Err(GlusterError::new(get_error()));
            }
            Ok(write_size)
        }
    }
    pub fn readv(file_handle: &mut Struct_glfs_fd,
                 iov: &mut [&mut [u8]],
                 flags: i32)
                 -> Result<isize, GlusterError> {
        unsafe {
            let read_size = glfs_readv(file_handle,
                                       iov.as_ptr() as *const iovec,
                                       iov.len() as i32,
                                       flags);
            if read_size < 0 {
                return Err(GlusterError::new(get_error()));
            }
            Ok(read_size)

        }
    }
    pub fn writev(file_handle: &mut Struct_glfs_fd,
                  iov: &[&[u8]],
                  flags: i32)
                  -> Result<isize, GlusterError> {
        unsafe {
            let write_size = glfs_writev(file_handle,
                                         iov.as_ptr() as *const iovec,
                                         iov.len() as i32,
                                         flags);
            if write_size < 0 {
                return Err(GlusterError::new(get_error()));
            }
            Ok(write_size)

        }
    }

    pub fn pread(file_handle: &mut Struct_glfs_fd,
                 fill_buffer: &mut [u8],
                 count: usize,
                 offset: i64,
                 flags: i32)
                 -> Result<isize, GlusterError> {
        unsafe {
            let read_size = glfs_pread(file_handle,
                                       fill_buffer.as_mut_ptr() as *mut c_void,
                                       count,
                                       offset,
                                       flags);
            if read_size < 0 {
                return Err(GlusterError::new(get_error()));
            }
            Ok(read_size)
        }
    }
    pub fn pwrite(file_handle: &mut Struct_glfs_fd,
                  buffer: &[u8],
                  count: usize,
                  offset: i64,
                  flags: i32)
                  -> Result<isize, GlusterError> {
        unsafe {
            let write_size = glfs_pwrite(file_handle,
                                         buffer.as_ptr() as *mut c_void,
                                         count,
                                         offset,
                                         flags);
            if write_size < 0 {
                return Err(GlusterError::new(get_error()));
            }
            Ok(write_size)

        }
    }

    pub fn preadv(file_handle: &mut Struct_glfs_fd,
                  iov: &mut [&mut [u8]],
                  offset: i64,
                  flags: i32)
                  -> Result<isize, GlusterError> {
        unsafe {
            let read_size = glfs_preadv(file_handle,
                                        iov.as_ptr() as *const iovec,
                                        iov.len() as i32,
                                        offset,
                                        flags);
            if read_size < 0 {
                return Err(GlusterError::new(get_error()));
            }
            Ok(read_size)
        }
    }
    // TODO: Use C IoVec
    pub fn pwritev(file_handle: &mut Struct_glfs_fd,
                   iov: &[&[u8]],
                   offset: i64,
                   flags: i32)
                   -> Result<isize, GlusterError> {
        unsafe {
            let write_size = glfs_pwritev(file_handle,
                                          iov.as_ptr() as *const iovec,
                                          iov.len() as i32,
                                          offset,
                                          flags);
            if write_size < 0 {
                return Err(GlusterError::new(get_error()));
            }
            Ok(write_size)
        }
    }
    pub fn lseek(file_handle: &mut Struct_glfs_fd,
                 offset: i64,
                 whence: i32)
                 -> Result<i64, GlusterError> {
        unsafe {
            let file_offset = glfs_lseek(file_handle, offset, whence);
            if file_offset < 0 {
                return Err(GlusterError::new(get_error()));
            }
            Ok(file_offset)

        }

    }
    pub fn truncate(&self, path: &str, length: i64) -> Result<(), GlusterError> {
        let path = try!(CString::new(path));

        unsafe {
            let ret_code = glfs_truncate(self.cluster_handle, path.as_ptr(), length);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn ftruncate(file_handle: &mut Struct_glfs_fd, length: i64) -> Result<(), GlusterError> {
        unsafe {
            let ret_code = glfs_ftruncate(file_handle, length);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn lsstat(&self, path: &str) -> Result<stat, GlusterError> {
        let path = try!(CString::new(path));
        unsafe {
            let mut stat_buf: stat = zeroed();
            let ret_code = glfs_lstat(self.cluster_handle, path.as_ptr(), &mut stat_buf);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
            Ok(stat_buf)
        }
    }
    pub fn stat(&self, path: &str) -> Result<stat, GlusterError> {
        let path = try!(CString::new(path));
        unsafe {
            let mut stat_buf: stat = zeroed();
            let ret_code = glfs_stat(self.cluster_handle, path.as_ptr(), &mut stat_buf);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
            Ok(stat_buf)
        }

    }
    pub fn fstat(file_handle: &mut Struct_glfs_fd) -> Result<stat, GlusterError> {
        unsafe {
            let mut stat_buf: stat = zeroed();
            let ret_code = glfs_fstat(file_handle, &mut stat_buf);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
            Ok(stat_buf)
        }
    }
    pub fn fsync(file_handle: &mut Struct_glfs_fd) -> Result<(), GlusterError> {
        unsafe {
            let ret_code = glfs_fsync(file_handle);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }

    pub fn fdatasync(file_handle: &mut Struct_glfs_fd) -> Result<(), GlusterError> {
        unsafe {
            let ret_code = glfs_fdatasync(file_handle);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }

        }
        Ok(())
    }
    pub fn access(&self, path: &str, mode: i32) -> Result<(), GlusterError> {
        let path = try!(CString::new(path));
        unsafe {
            let ret_code = glfs_access(self.cluster_handle, path.as_ptr(), mode);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }

        }
        Ok(())
    }

    pub fn symlink(&self, oldpath: &str, newpath: &str) -> Result<(), GlusterError> {
        let old_path = try!(CString::new(oldpath));
        let new_path = try!(CString::new(newpath));
        unsafe {
            let ret_code = glfs_symlink(self.cluster_handle, old_path.as_ptr(), new_path.as_ptr());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }

        }
        Ok(())
    }

    pub fn readlink(&self, path: &str, buf: &mut [u8]) -> Result<(), GlusterError> {
        let path = try!(CString::new(path));
        unsafe {
            let ret_code = glfs_readlink(self.cluster_handle,
                                         path.as_ptr(),
                                         buf.as_mut_ptr() as *mut i8,
                                         buf.len());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }

    pub fn mknod(&self, path: &str, mode: mode_t, dev: dev_t) -> Result<(), GlusterError> {
        let path = try!(CString::new(path));
        unsafe {
            let ret_code = glfs_mknod(self.cluster_handle, path.as_ptr(), mode, dev);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }

        }
        Ok(())
    }

    pub fn mkdir(&self, path: &str, mode: mode_t) -> Result<(), GlusterError> {
        let path = try!(CString::new(path));
        unsafe {
            let ret_code = glfs_mkdir(self.cluster_handle, path.as_ptr(), mode);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }

        }
        Ok(())
    }

    pub fn unlink(&self, path: &str) -> Result<(), GlusterError> {
        let path = try!(CString::new(path));
        unsafe {
            let ret_code = glfs_unlink(self.cluster_handle, path.as_ptr());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }

        }
        Ok(())
    }
    pub fn rmdir(&self, path: &str) -> Result<(), GlusterError> {
        let path = try!(CString::new(path));
        unsafe {
            let ret_code = glfs_rmdir(self.cluster_handle, path.as_ptr());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn rename(&self, oldpath: &str, newpath: &str) -> Result<(), GlusterError> {
        let old_path = try!(CString::new(oldpath));
        let new_path = try!(CString::new(newpath));
        unsafe {
            let ret_code = glfs_rename(self.cluster_handle, old_path.as_ptr(), new_path.as_ptr());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }

    pub fn link(&self, oldpath: &str, newpath: &str) -> Result<(), GlusterError> {
        let old_path = try!(CString::new(oldpath));
        let new_path = try!(CString::new(newpath));
        unsafe {
            let ret_code = glfs_link(self.cluster_handle, old_path.as_ptr(), new_path.as_ptr());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }

    pub fn opendir(&self, path: &str) -> Result<*mut Struct_glfs_fd, GlusterError> {
        let path = try!(CString::new(path));
        unsafe {
            let file_handle = glfs_opendir(self.cluster_handle, path.as_ptr());
            Ok(file_handle)
        }
    }
    pub fn getxattr(&self, path: &str, name: &str) -> Result<String, GlusterError> {
        let path = try!(CString::new(path));
        let name = try!(CString::new(name));
        let mut xattr_val_buff: Vec<u8> = Vec::with_capacity(1024);
        unsafe {
            let ret_code = glfs_getxattr(self.cluster_handle,
                                         path.as_ptr(),
                                         name.as_ptr(),
                                         xattr_val_buff.as_mut_ptr() as *mut c_void,
                                         xattr_val_buff.len());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
            // Set the buffer to the size of bytes read into it
            xattr_val_buff.set_len(ret_code as usize);
            Ok(String::from_utf8_lossy(&xattr_val_buff).into_owned())
        }
    }

    pub fn lgetxattr(&self, path: &str, name: &str) -> Result<String, GlusterError> {
        let path = try!(CString::new(path));
        let name = try!(CString::new(name));
        let mut xattr_val_buff: Vec<u8> = Vec::with_capacity(1024);
        unsafe {
            let ret_code = glfs_lgetxattr(self.cluster_handle,
                                          path.as_ptr(),
                                          name.as_ptr(),
                                          xattr_val_buff.as_mut_ptr() as *mut c_void,
                                          xattr_val_buff.len());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
            // Set the buffer to the size of bytes read into it
            xattr_val_buff.set_len(ret_code as usize);
            Ok(String::from_utf8_lossy(&xattr_val_buff).into_owned())
        }
    }
    pub fn fgetxattr(file_handle: &mut Struct_glfs_fd, name: &str) -> Result<String, GlusterError> {
        let name = try!(CString::new(name));
        let mut xattr_val_buff: Vec<u8> = Vec::with_capacity(1024);
        unsafe {
            let ret_code = glfs_fgetxattr(file_handle,
                                          name.as_ptr(),
                                          xattr_val_buff.as_mut_ptr() as *mut c_void,
                                          xattr_val_buff.len());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
            // Set the buffer to the size of bytes read into it
            xattr_val_buff.set_len(ret_code as usize);
            Ok(String::from_utf8_lossy(&xattr_val_buff).into_owned())
        }
    }
    pub fn listxattr(&self, path: &str) -> Result<String, GlusterError> {
        let path = try!(CString::new(path));
        let mut xattr_val_buff: Vec<u8> = Vec::with_capacity(1024);
        unsafe {
            let ret_code = glfs_listxattr(self.cluster_handle,
                                          path.as_ptr(),
                                          xattr_val_buff.as_mut_ptr() as *mut c_void,
                                          xattr_val_buff.len());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
            // Set the buffer to the size of bytes read into it
            xattr_val_buff.set_len(ret_code as usize);
            Ok(String::from_utf8_lossy(&xattr_val_buff).into_owned())
        }
    }
    pub fn llistxattr(&self, path: &str) -> Result<String, GlusterError> {
        let path = try!(CString::new(path));
        let mut xattr_val_buff: Vec<u8> = Vec::with_capacity(1024);
        unsafe {
            let ret_code = glfs_llistxattr(self.cluster_handle,
                                           path.as_ptr(),
                                           xattr_val_buff.as_mut_ptr() as *mut c_void,
                                           xattr_val_buff.len());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
            // Set the buffer to the size of bytes read into it
            xattr_val_buff.set_len(ret_code as usize);
            Ok(String::from_utf8_lossy(&xattr_val_buff).into_owned())
        }
    }
    pub fn flistxattr(file_handle: &mut Struct_glfs_fd) -> Result<String, GlusterError> {
        let mut xattr_val_buff: Vec<u8> = Vec::with_capacity(1024);
        unsafe {
            let ret_code = glfs_flistxattr(file_handle,
                                           xattr_val_buff.as_mut_ptr() as *mut c_void,
                                           xattr_val_buff.len());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
            // Set the buffer to the size of bytes read into it
            xattr_val_buff.set_len(ret_code as usize);
            Ok(String::from_utf8_lossy(&xattr_val_buff).into_owned())
        }
    }
    pub fn setxattr(&self,
                    path: &str,
                    name: &str,
                    value: &[u8],
                    flags: i32)
                    -> Result<(), GlusterError> {
        let path = try!(CString::new(path));
        let name = try!(CString::new(name));
        unsafe {
            let ret_code = glfs_setxattr(self.cluster_handle,
                                         path.as_ptr(),
                                         name.as_ptr(),
                                         value.as_ptr() as *const c_void,
                                         value.len(),
                                         flags);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn lsetxattr(&self,
                     name: &str,
                     value: &[u8],
                     path: &str,
                     flags: i32)
                     -> Result<(), GlusterError> {
        let name = try!(CString::new(name));
        let path = try!(CString::new(path));
        unsafe {
            let ret_code = glfs_lsetxattr(self.cluster_handle,
                                          path.as_ptr(),
                                          name.as_ptr(),
                                          value.as_ptr() as *const c_void,
                                          value.len(),
                                          flags);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn fsetxattr(file_handle: &mut Struct_glfs_fd,
                     name: &str,
                     value: &[u8],
                     flags: i32)
                     -> Result<(), GlusterError> {
        let name = try!(CString::new(name));
        unsafe {
            let ret_code = glfs_fsetxattr(file_handle,
                                          name.as_ptr(),
                                          value.as_ptr() as *const c_void,
                                          value.len(),
                                          flags);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn removexattr(&self, path: &str, name: &str) -> Result<(), GlusterError> {
        let path = try!(CString::new(path));
        let name = try!(CString::new(name));
        unsafe {
            let ret_code = glfs_removexattr(self.cluster_handle, path.as_ptr(), name.as_ptr());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn lremovexattr(&self, path: &str, name: &str) -> Result<(), GlusterError> {
        let path = try!(CString::new(path));
        let name = try!(CString::new(name));
        unsafe {
            let ret_code = glfs_lremovexattr(self.cluster_handle, path.as_ptr(), name.as_ptr());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn fremovexattr(file_handle: &mut Struct_glfs_fd, name: &str) -> Result<(), GlusterError> {
        let name = try!(CString::new(name));

        unsafe {
            let ret_code = glfs_fremovexattr(file_handle, name.as_ptr());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn fallocate(file_handle: &mut Struct_glfs_fd,
                     offset: i64,
                     keep_size: i32,
                     len: usize)
                     -> Result<(), GlusterError> {
        unsafe {
            let ret_code = glfs_fallocate(file_handle, keep_size, offset, len);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn discard(file_handle: &mut Struct_glfs_fd,
                   offset: i64,
                   len: usize)
                   -> Result<(), GlusterError> {
        unsafe {
            let ret_code = glfs_discard(file_handle, offset, len);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn zerofill(file_handle: &mut Struct_glfs_fd,
                    offset: i64,
                    len: i64)
                    -> Result<(), GlusterError> {
        unsafe {
            let ret_code = glfs_zerofill(file_handle, offset, len);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn getcwd(&self) -> Result<String, GlusterError> {
        let mut cwd_val_buff: Vec<u8> = Vec::with_capacity(1024);
        unsafe {
            let cwd = glfs_getcwd(self.cluster_handle,
                                  cwd_val_buff.as_mut_ptr() as *mut i8,
                                  cwd_val_buff.len());
            Ok(CStr::from_ptr(cwd).to_string_lossy().into_owned())
        }
    }
    pub fn chdir(&self, path: &str) -> Result<(), GlusterError> {
        let path = try!(CString::new(path));
        unsafe {
            let ret_code = glfs_chdir(self.cluster_handle, path.as_ptr());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn fchdir(file_handle: &mut Struct_glfs_fd) -> Result<(), GlusterError> {
        unsafe {
            let ret_code = glfs_fchdir(file_handle);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    // pub fn realpath(&self, path: &str) -> Result<String, GlusterError> {
    // let path = try!(CString::new(path));
    // let resolved_path_buf: Vec<u8> = Vec::with_capacity(512);
    // unsafe {
    // let real_path = glfs_realpath(self.cluster_handle,
    // path.as_ptr(),
    // resolved_path: *mut c_char);
    // Ok(CStr::from_ptr(real_path).to_string_lossy().into_owned())
    // }
    // }
    //
    pub fn dup(file_handle: &mut Struct_glfs_fd) -> Result<*mut Struct_glfs_fd, GlusterError> {
        unsafe {
            let file_handle = glfs_dup(file_handle);
            Ok(file_handle)
        }
    }
}

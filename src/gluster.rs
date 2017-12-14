use errno::{errno, Errno};
use glfs::*;
use libc::{c_uchar, c_void, dev_t, dirent, DT_DIR, DT_REG, ENOENT, flock, LOCK_SH, LOCK_EX,
           LOCK_UN, ino_t, mode_t, stat, timespec};
//use libffi::high::Closure3;

use std::error::Error as err;
use std::mem::zeroed;
use std::ffi::{CStr, CString, IntoStringError, NulError};
use std::fmt;
use std::io::Error;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::ptr;
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

impl fmt::Display for GlusterError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.description())
    }
}

impl err for GlusterError {
    fn description(&self) -> &str {
        match *self {
            GlusterError::FromUtf8Error(ref e) => e.description(),
            GlusterError::NulError(ref e) => e.description(),
            GlusterError::Error(ref e) => &e,
            GlusterError::IoError(ref e) => e.description(),
            GlusterError::IntoStringError(ref e) => e.description(),
        }
    }
    fn cause(&self) -> Option<&err> {
        match *self {
            GlusterError::FromUtf8Error(ref e) => e.cause(),
            GlusterError::NulError(ref e) => e.cause(),
            GlusterError::Error(_) => None,
            GlusterError::IoError(ref e) => e.cause(),
            GlusterError::IntoStringError(ref e) => e.cause(),
        }
    }
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

/// Apply or remove an advisory lock on the open file.
pub enum PosixLockCmd {
    /// Place  an  exclusive  lock.  Only one process may hold an
    /// exclusive lock for a given file at a given time.
    Exclusive,
    /// Place a shared lock. More than one  process may  hold  a shared
    /// lock for a given file at a given time.
    Shared,
    /// Remove an existing lock held by this process.
    Unlock,
}

impl Into<i32> for PosixLockCmd {
    fn into(self) -> i32 {
        match self {
            PosixLockCmd::Shared => LOCK_SH,
            PosixLockCmd::Exclusive => LOCK_EX,
            PosixLockCmd::Unlock => LOCK_UN,
        }
    }
}

// pub type glfs_io_cbk = ::std::option::Option<extern "C" fn(fd: *mut glfs_fd_t,
// ret: ssize_t,
// data: *mut c_void)
// -> ()>;pub type glfs_io_cbk = ::std::option::Option<extern "C" fn(fd: *mut glfs_fd_t,
// ret: ssize_t,
// data: *mut c_void)
// -> ()>;
//

#[derive(Debug)]
pub struct Gluster {
    cluster_handle: *mut Struct_glfs,
}

// As far as I can tell the cluster handle to gluster is thread safe
unsafe impl Send for Gluster {}
unsafe impl Sync for Gluster {}

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

/// This uses readdirplus which is very efficient in Gluster.  In addition
/// to returning directory entries this also stats each file.
#[derive(Debug)]
pub struct GlusterDirectoryPlus {
    pub dir_handle: *mut Struct_glfs_fd,
}

pub struct DirEntryPlus {
    pub path: PathBuf,
    pub inode: ino_t,
    pub file_type: c_uchar,
    pub stat: stat,
}

impl Iterator for GlusterDirectoryPlus {
    type Item = DirEntryPlus;
    fn next(&mut self) -> Option<DirEntryPlus> {
        let mut dirent: dirent = unsafe { zeroed() };
        let mut next_entry: *mut dirent = ptr::null_mut();
        unsafe {
            let mut stat_buf: stat = zeroed();
            let ret_code =
                glfs_readdirplus_r(self.dir_handle, &mut stat_buf, &mut dirent, &mut next_entry);
            if ret_code < 0 {
                glfs_closedir(self.dir_handle);
                return None;
            }
            if dirent.d_ino == 0 {
                // End of stream reached
                return None;
            }
            let telldir_retcode = glfs_telldir(self.dir_handle);
            if telldir_retcode < 0 {
                return None;
            }
            let file_name = CStr::from_ptr(dirent.d_name.as_ptr());
            return Some(DirEntryPlus {
                path: PathBuf::from(file_name.to_string_lossy().into_owned()),
                inode: dirent.d_ino,
                file_type: dirent.d_type,
                stat: stat_buf,
            });
        }

    }
}

#[derive(Debug)]
pub struct GlusterDirectory {
    pub dir_handle: *mut Struct_glfs_fd,
}

#[derive(Debug)]
pub struct DirEntry {
    pub path: PathBuf,
    pub inode: ino_t,
    pub file_type: c_uchar,
}

impl Iterator for GlusterDirectory {
    type Item = DirEntry;
    fn next(&mut self) -> Option<DirEntry> {
        let mut dirent: dirent = unsafe { zeroed() };
        let mut next_entry: *mut dirent = ptr::null_mut();
        unsafe {
            let ret_code = glfs_readdir_r(self.dir_handle, &mut dirent, &mut next_entry);
            if ret_code < 0 {
                glfs_closedir(self.dir_handle);
                return None;
            }
            if dirent.d_ino == 0 {
                // End of stream reached
                return None;
            }
            let telldir_retcode = glfs_telldir(self.dir_handle);
            if telldir_retcode < 0 {
                return None;
            }
            let file_name = CStr::from_ptr(dirent.d_name.as_ptr());
            return Some(DirEntry {
                path: PathBuf::from(file_name.to_string_lossy().into_owned()),
                inode: dirent.d_ino,
                file_type: dirent.d_type,
            });
        }

    }
}

impl Gluster {
    /// Connect to a Ceph cluster and return a connection handle glfs_t
    /// port is usually 24007 but may differ depending on how the service was configured
    pub fn connect(volume_name: &str, server: &str, port: u16) -> Result<Gluster, GlusterError> {
        let vol_name = try!(CString::new(volume_name));
        let vol_transport = try!(CString::new("tcp"));
        let vol_host = try!(CString::new(server));
        unsafe {
            let cluster_handle = glfs_new(vol_name.as_ptr());
            if cluster_handle.is_null() {
                return Err(GlusterError::new("glfs_new failed".to_string()));
            }
            let ret_code = glfs_set_volfile_server(
                cluster_handle,
                vol_transport.as_ptr(),
                vol_host.as_ptr(),
                port as ::libc::c_int,
            );
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
    pub fn open(&self, path: &Path, flags: i32) -> Result<*mut Struct_glfs_fd, GlusterError> {
        let path = try!(CString::new(path.as_os_str().as_bytes()));
        unsafe {
            let file_handle = glfs_open(self.cluster_handle, path.as_ptr(), flags);
            Ok(file_handle)
        }
    }
    pub fn create(
        &self,
        path: &Path,
        flags: i32,
        mode: mode_t,
    ) -> Result<*mut Struct_glfs_fd, GlusterError> {
        let path = try!(CString::new(path.as_os_str().as_bytes()));
        unsafe {
            let file_handle = glfs_creat(self.cluster_handle, path.as_ptr(), flags, mode);
            if file_handle.is_null() {
                return Err(GlusterError::new(get_error()));
            }
            Ok(file_handle)
        }
    }
    pub fn close(&self, file_handle: *mut Struct_glfs_fd) -> Result<(), GlusterError> {
        unsafe {
            let ret_code = glfs_close(file_handle);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn read(
        &self,
        file_handle: *mut Struct_glfs_fd,
        fill_buffer: &mut Vec<u8>,
        count: usize,
        flags: i32,
    ) -> Result<isize, GlusterError> {
        self.pread(file_handle, fill_buffer, count, 0, flags)
    }
    pub fn write(
        &self,
        file_handle: *mut Struct_glfs_fd,
        buffer: &[u8],
        flags: i32,
    ) -> Result<isize, GlusterError> {
        self.pwrite(file_handle, buffer, buffer.len(), 0, flags)
    }

    /*
    pub fn write_async<F>(
        &self,
        file_handle: *mut Struct_glfs_fd,
        buffer: &[u8],
        flags: i32,
        callback: F,
        data: &mut ::libc::c_void,
    ) -> Result<(), GlusterError>
    where
        F: Fn(*mut Struct_glfs_fd, isize, *mut ::libc::c_void),
    {
        let closure = Closure3::new(&callback);
        let callback_ptr = closure.code_ptr();
        unsafe {
            let ret_code = glfs_write_async(
                file_handle,
                buffer.as_ptr() as *const c_void,
                buffer.len(),
                flags,
                Some(*callback_ptr),
                data,
            );
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    */
    pub fn readv(
        &self,
        file_handle: *mut Struct_glfs_fd,
        iov: &mut [&mut [u8]],
        flags: i32,
    ) -> Result<isize, GlusterError> {
        unsafe {
            let read_size = glfs_readv(
                file_handle,
                iov.as_ptr() as *const iovec,
                iov.len() as i32,
                flags,
            );
            if read_size < 0 {
                return Err(GlusterError::new(get_error()));
            }
            Ok(read_size)

        }
    }
    pub fn writev(
        &self,
        file_handle: *mut Struct_glfs_fd,
        iov: &[&[u8]],
        flags: i32,
    ) -> Result<isize, GlusterError> {
        unsafe {
            let write_size = glfs_writev(
                file_handle,
                iov.as_ptr() as *const iovec,
                iov.len() as i32,
                flags,
            );
            if write_size < 0 {
                return Err(GlusterError::new(get_error()));
            }
            Ok(write_size)

        }
    }

    /// Read into fill_buffer at offset and return the number of bytes read
    pub fn pread(
        &self,
        file_handle: *mut Struct_glfs_fd,
        fill_buffer: &mut Vec<u8>,
        count: usize,
        offset: i64,
        flags: i32,
    ) -> Result<isize, GlusterError> {
        unsafe {
            let read_size = glfs_pread(
                file_handle,
                fill_buffer.as_mut_ptr() as *mut c_void,
                count,
                offset,
                flags,
            );
            if read_size < 0 {
                return Err(GlusterError::new(get_error()));
            }
            fill_buffer.set_len(read_size as usize);
            Ok(read_size)
        }
    }
    pub fn pwrite(
        &self,
        file_handle: *mut Struct_glfs_fd,
        buffer: &[u8],
        count: usize,
        offset: i64,
        flags: i32,
    ) -> Result<isize, GlusterError> {
        unsafe {
            let write_size = glfs_pwrite(
                file_handle,
                buffer.as_ptr() as *mut c_void,
                count,
                offset,
                flags,
            );
            if write_size < 0 {
                return Err(GlusterError::new(get_error()));
            }
            Ok(write_size)

        }
    }

    pub fn preadv(
        &self,
        file_handle: *mut Struct_glfs_fd,
        iov: &mut [&mut [u8]],
        offset: i64,
        flags: i32,
    ) -> Result<isize, GlusterError> {
        unsafe {
            let read_size = glfs_preadv(
                file_handle,
                iov.as_ptr() as *const iovec,
                iov.len() as i32,
                offset,
                flags,
            );
            if read_size < 0 {
                return Err(GlusterError::new(get_error()));
            }
            Ok(read_size)
        }
    }
    // TODO: Use C IoVec
    pub fn pwritev(
        &self,
        file_handle: *mut Struct_glfs_fd,
        iov: &[&[u8]],
        offset: i64,
        flags: i32,
    ) -> Result<isize, GlusterError> {
        unsafe {
            let write_size = glfs_pwritev(
                file_handle,
                iov.as_ptr() as *const iovec,
                iov.len() as i32,
                offset,
                flags,
            );
            if write_size < 0 {
                return Err(GlusterError::new(get_error()));
            }
            Ok(write_size)
        }
    }
    pub fn lseek(
        &self,
        file_handle: *mut Struct_glfs_fd,
        offset: i64,
        whence: i32,
    ) -> Result<i64, GlusterError> {
        unsafe {
            let file_offset = glfs_lseek(file_handle, offset, whence);
            if file_offset < 0 {
                return Err(GlusterError::new(get_error()));
            }
            Ok(file_offset)

        }

    }
    pub fn truncate(&self, path: &Path, length: i64) -> Result<(), GlusterError> {
        let path = try!(CString::new(path.as_os_str().as_bytes()));

        unsafe {
            let ret_code = glfs_truncate(self.cluster_handle, path.as_ptr(), length);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn ftruncate(
        &self,
        file_handle: *mut Struct_glfs_fd,
        length: i64,
    ) -> Result<(), GlusterError> {
        unsafe {
            let ret_code = glfs_ftruncate(file_handle, length);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn lsstat(&self, path: &Path) -> Result<stat, GlusterError> {
        let path = try!(CString::new(path.as_os_str().as_bytes()));
        unsafe {
            let mut stat_buf: stat = zeroed();
            let ret_code = glfs_lstat(self.cluster_handle, path.as_ptr(), &mut stat_buf);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
            Ok(stat_buf)
        }
    }
    /// Tests for the existance of a file.  Returns true/false respectively.
    pub fn exists(&self, path: &Path) -> Result<bool, GlusterError> {
        let path = try!(CString::new(path.as_os_str().as_bytes()));
        unsafe {
            let mut stat_buf: stat = zeroed();
            let ret_code = glfs_stat(self.cluster_handle, path.as_ptr(), &mut stat_buf);
            if ret_code < 0 {
                let error = errno();
                if error == Errno(ENOENT) {
                    return Ok(false);
                }
                return Err(GlusterError::new(get_error()));
            }
            Ok(true)
        }
    }

    pub fn stat(&self, path: &Path) -> Result<stat, GlusterError> {
        let path = try!(CString::new(path.as_os_str().as_bytes()));
        unsafe {
            let mut stat_buf: stat = zeroed();
            let ret_code = glfs_stat(self.cluster_handle, path.as_ptr(), &mut stat_buf);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
            Ok(stat_buf)
        }

    }
    pub fn fstat(&self, file_handle: *mut Struct_glfs_fd) -> Result<stat, GlusterError> {
        unsafe {
            let mut stat_buf: stat = zeroed();
            let ret_code = glfs_fstat(file_handle, &mut stat_buf);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
            Ok(stat_buf)
        }
    }
    pub fn fsync(&self, file_handle: *mut Struct_glfs_fd) -> Result<(), GlusterError> {
        unsafe {
            let ret_code = glfs_fsync(file_handle);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }

    pub fn fdatasync(&self, file_handle: *mut Struct_glfs_fd) -> Result<(), GlusterError> {
        unsafe {
            let ret_code = glfs_fdatasync(file_handle);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }

        }
        Ok(())
    }
    pub fn access(&self, path: &Path, mode: i32) -> Result<(), GlusterError> {
        let path = try!(CString::new(path.as_os_str().as_bytes()));
        unsafe {
            let ret_code = glfs_access(self.cluster_handle, path.as_ptr(), mode);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }

        }
        Ok(())
    }

    pub fn symlink(&self, oldpath: &Path, newpath: &Path) -> Result<(), GlusterError> {
        let old_path = try!(CString::new(oldpath.as_os_str().as_bytes()));
        let new_path = try!(CString::new(newpath.as_os_str().as_bytes()));
        unsafe {
            let ret_code = glfs_symlink(self.cluster_handle, old_path.as_ptr(), new_path.as_ptr());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }

        }
        Ok(())
    }

    pub fn readlink(&self, path: &Path, buf: &mut [u8]) -> Result<(), GlusterError> {
        let path = try!(CString::new(path.as_os_str().as_bytes()));
        unsafe {
            let ret_code = glfs_readlink(
                self.cluster_handle,
                path.as_ptr(),
                buf.as_mut_ptr() as *mut i8,
                buf.len(),
            );
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }

    pub fn mknod(&self, path: &Path, mode: mode_t, dev: dev_t) -> Result<(), GlusterError> {
        let path = try!(CString::new(path.as_os_str().as_bytes()));
        unsafe {
            let ret_code = glfs_mknod(self.cluster_handle, path.as_ptr(), mode, dev);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }

        }
        Ok(())
    }

    pub fn mkdir(&self, path: &Path, mode: mode_t) -> Result<(), GlusterError> {
        let path = try!(CString::new(path.as_os_str().as_bytes()));
        unsafe {
            let ret_code = glfs_mkdir(self.cluster_handle, path.as_ptr(), mode);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }

        }
        Ok(())
    }

    pub fn unlink(&self, path: &Path) -> Result<(), GlusterError> {
        let path = try!(CString::new(path.as_os_str().as_bytes()));
        unsafe {
            let ret_code = glfs_unlink(self.cluster_handle, path.as_ptr());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }

        }
        Ok(())
    }
    pub fn rmdir(&self, path: &Path) -> Result<(), GlusterError> {
        let path = try!(CString::new(path.as_os_str().as_bytes()));
        unsafe {
            let ret_code = glfs_rmdir(self.cluster_handle, path.as_ptr());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }

    fn is_empty(&self, p: &Path) -> Result<bool, GlusterError> {
        let this = Path::new(".");
        let parent = Path::new("..");
        let d = GlusterDirectory { dir_handle: self.opendir(&p)? };
        for dir_entry in d {
            if dir_entry.path == this || dir_entry.path == parent {
                continue;
            }
            match dir_entry.file_type {
                // If there's anything in here besides . or .. then return false
                _ => {
                    trace!("{:?} is not empty", dir_entry);
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    /// Removes a directory at this path, after removing all its contents.
    /// Use carefully!
    pub fn remove_dir_all(&self, path: &Path) -> Result<(), GlusterError> {
        trace!("Removing {}", path.display());
        let mut stack: Vec<PathBuf> = vec![path.to_path_buf()];
        let mut done = false;
        let this = Path::new(".");
        let parent = Path::new("..");
        while !done {
            trace!("stack: {:?}", stack);
            if let Some(mut p) = stack.pop() {
                if p == PathBuf::from("") {
                    // short circuit
                    trace!("break for PathBuf::from(\"\")");
                    break;
                }
                let d = GlusterDirectory { dir_handle: self.opendir(&p)? };
                // If there's nothing in there remove the directory
                if self.is_empty(&p)? {
                    self.rmdir(&p)?;
                    // Remove this dir from the PathBuf
                    p.pop();
                    // Push it back onto the working stack because there
                    // might be more work needed
                    stack.push(p);
                    continue;
                }
                for dir_entry in d {
                    trace!("dir_entry: {:?}", dir_entry);
                    if dir_entry.path == this || dir_entry.path == parent {
                        trace!("Skipping . or .. ");
                        continue;
                    }
                    match dir_entry.file_type {
                        DT_DIR => {
                            let mut p = PathBuf::from(&p);
                            p.push(dir_entry.path);
                            trace!("pushing: {}", p.display());
                            stack.push(p);
                        }
                        DT_REG => {
                            let mut p = PathBuf::from(&p);
                            p.push(dir_entry.path);
                            trace!("unlink: {}", p.display());
                            self.unlink(&p)?;
                        }
                        _ => {}
                    }
                }
                if stack.len() == 0 {
                    self.rmdir(&p)?;
                    // There's a parent directory left to remove
                    if p.pop() {
                        stack.push(p);
                    }
                }
            } else {
                done = true;
            }
        }

        // Check if we removed the original directory and exit
        if self.is_empty(&path)? {
            trace!("removing {}", path.display());
            let _ = self.rmdir(&path);
        }

        Ok(())
    }

    pub fn rename(&self, oldpath: &Path, newpath: &Path) -> Result<(), GlusterError> {
        let old_path = try!(CString::new(oldpath.as_os_str().as_bytes()));
        let new_path = try!(CString::new(newpath.as_os_str().as_bytes()));
        unsafe {
            let ret_code = glfs_rename(self.cluster_handle, old_path.as_ptr(), new_path.as_ptr());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }

    pub fn link(&self, oldpath: &Path, newpath: &Path) -> Result<(), GlusterError> {
        let old_path = try!(CString::new(oldpath.as_os_str().as_bytes()));
        let new_path = try!(CString::new(newpath.as_os_str().as_bytes()));
        unsafe {
            let ret_code = glfs_link(self.cluster_handle, old_path.as_ptr(), new_path.as_ptr());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }

    pub fn opendir(&self, path: &Path) -> Result<*mut Struct_glfs_fd, GlusterError> {
        let path = try!(CString::new(path.as_os_str().as_bytes()));
        unsafe {
            let file_handle = glfs_opendir(self.cluster_handle, path.as_ptr());
            Ok(file_handle)
        }
    }
    pub fn getxattr(&self, path: &Path, name: &str) -> Result<String, GlusterError> {
        let path = try!(CString::new(path.as_os_str().as_bytes()));
        let name = try!(CString::new(name));
        let mut xattr_val_buff: Vec<u8> = Vec::with_capacity(1024);
        unsafe {
            let ret_code = glfs_getxattr(
                self.cluster_handle,
                path.as_ptr(),
                name.as_ptr(),
                xattr_val_buff.as_mut_ptr() as *mut c_void,
                xattr_val_buff.len(),
            );
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
            // Set the buffer to the size of bytes read into it
            xattr_val_buff.set_len(ret_code as usize);
            Ok(String::from_utf8_lossy(&xattr_val_buff).into_owned())
        }
    }

    pub fn lgetxattr(&self, path: &Path, name: &str) -> Result<String, GlusterError> {
        let path = try!(CString::new(path.as_os_str().as_bytes()));
        let name = try!(CString::new(name));
        let mut xattr_val_buff: Vec<u8> = Vec::with_capacity(1024);
        unsafe {
            let ret_code = glfs_lgetxattr(
                self.cluster_handle,
                path.as_ptr(),
                name.as_ptr(),
                xattr_val_buff.as_mut_ptr() as *mut c_void,
                xattr_val_buff.len(),
            );
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
            // Set the buffer to the size of bytes read into it
            xattr_val_buff.set_len(ret_code as usize);
            Ok(String::from_utf8_lossy(&xattr_val_buff).into_owned())
        }
    }
    pub fn fgetxattr(
        &self,
        file_handle: *mut Struct_glfs_fd,
        name: &str,
    ) -> Result<String, GlusterError> {
        let name = try!(CString::new(name));
        let mut xattr_val_buff: Vec<u8> = Vec::with_capacity(1024);
        unsafe {
            let ret_code = glfs_fgetxattr(
                file_handle,
                name.as_ptr(),
                xattr_val_buff.as_mut_ptr() as *mut c_void,
                xattr_val_buff.len(),
            );
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
            // Set the buffer to the size of bytes read into it
            xattr_val_buff.set_len(ret_code as usize);
            Ok(String::from_utf8_lossy(&xattr_val_buff).into_owned())
        }
    }
    pub fn listxattr(&self, path: &Path) -> Result<String, GlusterError> {
        let path = try!(CString::new(path.as_os_str().as_bytes()));
        let mut xattr_val_buff: Vec<u8> = Vec::with_capacity(1024);
        unsafe {
            let ret_code = glfs_listxattr(
                self.cluster_handle,
                path.as_ptr(),
                xattr_val_buff.as_mut_ptr() as *mut c_void,
                xattr_val_buff.len(),
            );
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
            // Set the buffer to the size of bytes read into it
            xattr_val_buff.set_len(ret_code as usize);
            Ok(String::from_utf8_lossy(&xattr_val_buff).into_owned())
        }
    }
    pub fn llistxattr(&self, path: &Path) -> Result<String, GlusterError> {
        let path = try!(CString::new(path.as_os_str().as_bytes()));
        let mut xattr_val_buff: Vec<u8> = Vec::with_capacity(1024);
        unsafe {
            let ret_code = glfs_llistxattr(
                self.cluster_handle,
                path.as_ptr(),
                xattr_val_buff.as_mut_ptr() as *mut c_void,
                xattr_val_buff.len(),
            );
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
            // Set the buffer to the size of bytes read into it
            xattr_val_buff.set_len(ret_code as usize);
            Ok(String::from_utf8_lossy(&xattr_val_buff).into_owned())
        }
    }
    pub fn flistxattr(&self, file_handle: *mut Struct_glfs_fd) -> Result<String, GlusterError> {
        let mut xattr_val_buff: Vec<u8> = Vec::with_capacity(1024);
        unsafe {
            let ret_code = glfs_flistxattr(
                file_handle,
                xattr_val_buff.as_mut_ptr() as *mut c_void,
                xattr_val_buff.len(),
            );
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
            // Set the buffer to the size of bytes read into it
            xattr_val_buff.set_len(ret_code as usize);
            Ok(String::from_utf8_lossy(&xattr_val_buff).into_owned())
        }
    }
    pub fn setxattr(
        &self,
        path: &Path,
        name: &str,
        value: &[u8],
        flags: i32,
    ) -> Result<(), GlusterError> {
        let path = try!(CString::new(path.as_os_str().as_bytes()));
        let name = try!(CString::new(name));
        unsafe {
            let ret_code = glfs_setxattr(
                self.cluster_handle,
                path.as_ptr(),
                name.as_ptr(),
                value.as_ptr() as *const c_void,
                value.len(),
                flags,
            );
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn lsetxattr(
        &self,
        name: &str,
        value: &[u8],
        path: &Path,
        flags: i32,
    ) -> Result<(), GlusterError> {
        let name = try!(CString::new(name));
        let path = try!(CString::new(path.as_os_str().as_bytes()));
        unsafe {
            let ret_code = glfs_lsetxattr(
                self.cluster_handle,
                path.as_ptr(),
                name.as_ptr(),
                value.as_ptr() as *const c_void,
                value.len(),
                flags,
            );
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn fsetxattr(
        &self,
        file_handle: *mut Struct_glfs_fd,
        name: &str,
        value: &[u8],
        flags: i32,
    ) -> Result<(), GlusterError> {
        let name = try!(CString::new(name));
        unsafe {
            let ret_code = glfs_fsetxattr(
                file_handle,
                name.as_ptr(),
                value.as_ptr() as *const c_void,
                value.len(),
                flags,
            );
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn removexattr(&self, path: &Path, name: &str) -> Result<(), GlusterError> {
        let path = try!(CString::new(path.as_os_str().as_bytes()));
        let name = try!(CString::new(name));
        unsafe {
            let ret_code = glfs_removexattr(self.cluster_handle, path.as_ptr(), name.as_ptr());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn lremovexattr(&self, path: &Path, name: &str) -> Result<(), GlusterError> {
        let path = try!(CString::new(path.as_os_str().as_bytes()));
        let name = try!(CString::new(name));
        unsafe {
            let ret_code = glfs_lremovexattr(self.cluster_handle, path.as_ptr(), name.as_ptr());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn fremovexattr(
        &self,
        file_handle: *mut Struct_glfs_fd,
        name: &str,
    ) -> Result<(), GlusterError> {
        let name = try!(CString::new(name));

        unsafe {
            let ret_code = glfs_fremovexattr(file_handle, name.as_ptr());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn fallocate(
        &self,
        file_handle: *mut Struct_glfs_fd,
        offset: i64,
        keep_size: i32,
        len: usize,
    ) -> Result<(), GlusterError> {
        unsafe {
            let ret_code = glfs_fallocate(file_handle, keep_size, offset, len);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn discard(
        &self,
        file_handle: *mut Struct_glfs_fd,
        offset: i64,
        len: usize,
    ) -> Result<(), GlusterError> {
        unsafe {
            let ret_code = glfs_discard(file_handle, offset, len);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn zerofill(
        &self,
        file_handle: *mut Struct_glfs_fd,
        offset: i64,
        len: i64,
    ) -> Result<(), GlusterError> {
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
            let cwd = glfs_getcwd(
                self.cluster_handle,
                cwd_val_buff.as_mut_ptr() as *mut i8,
                cwd_val_buff.len(),
            );
            Ok(CStr::from_ptr(cwd).to_string_lossy().into_owned())
        }
    }
    pub fn chdir(&self, path: &Path) -> Result<(), GlusterError> {
        let path = try!(CString::new(path.as_os_str().as_bytes()));
        unsafe {
            let ret_code = glfs_chdir(self.cluster_handle, path.as_ptr());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn fchdir(&self, file_handle: *mut Struct_glfs_fd) -> Result<(), GlusterError> {
        unsafe {
            let ret_code = glfs_fchdir(file_handle);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }

    /// times[0] specifies the new "last access time" (atime);
    /// times[1] specifies the new "last modification time" (mtime).
    pub fn utimens(&self, path: &Path, times: &[timespec; 2]) -> Result<(), GlusterError> {
        let path = try!(CString::new(path.as_os_str().as_bytes()));
        unsafe {
            let ret_code = glfs_utimens(self.cluster_handle, path.as_ptr(), times.as_ptr());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }

    /// times[0] specifies the new "last access time" (atime);
    /// times[1] specifies the new "last modification time" (mtime).
    pub fn lutimens(&self, path: &Path, times: &[timespec; 2]) -> Result<(), GlusterError> {
        let path = try!(CString::new(path.as_os_str().as_bytes()));
        unsafe {
            let ret_code = glfs_lutimens(self.cluster_handle, path.as_ptr(), times.as_ptr());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }

    /// times[0] specifies the new "last access time" (atime);
    /// times[1] specifies the new "last modification time" (mtime).
    pub fn futimens(
        &self,
        file_handle: *mut Struct_glfs_fd,
        times: &[timespec; 2],
    ) -> Result<(), GlusterError> {
        unsafe {
            let ret_code = glfs_futimens(file_handle, times.as_ptr());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }

    pub fn posixlock(
        &self,
        file_handle: *mut Struct_glfs_fd,
        command: PosixLockCmd,
        flock: &mut flock,
    ) -> Result<(), GlusterError> {
        unsafe {
            let ret_code = glfs_posix_lock(file_handle, command.into(), flock);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }

    pub fn chmod(&self, path: &Path, mode: mode_t) -> Result<(), GlusterError> {
        let path = try!(CString::new(path.as_os_str().as_bytes()));
        unsafe {
            let ret_code = glfs_chmod(self.cluster_handle, path.as_ptr(), mode);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }

    pub fn fchmod(
        &self,
        file_handle: *mut Struct_glfs_fd,
        mode: mode_t,
    ) -> Result<(), GlusterError> {
        unsafe {
            let ret_code = glfs_fchmod(file_handle, mode);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }

    pub fn chown(&self, path: &Path, uid: u32, gid: u32) -> Result<(), GlusterError> {
        let path = try!(CString::new(path.as_os_str().as_bytes()));
        unsafe {
            let ret_code = glfs_chown(self.cluster_handle, path.as_ptr(), uid, gid);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }

    pub fn lchown(&self, path: &Path, uid: u32, gid: u32) -> Result<(), GlusterError> {
        let path = try!(CString::new(path.as_os_str().as_bytes()));
        unsafe {
            let ret_code = glfs_lchown(self.cluster_handle, path.as_ptr(), uid, gid);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }

    pub fn fchown(
        &self,
        file_handle: *mut Struct_glfs_fd,
        uid: u32,
        gid: u32,
    ) -> Result<(), GlusterError> {
        unsafe {
            let ret_code = glfs_fchown(file_handle, uid, gid);
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
    pub fn dup(
        &self,
        file_handle: *mut Struct_glfs_fd,
    ) -> Result<*mut Struct_glfs_fd, GlusterError> {
        unsafe {
            let file_handle = glfs_dup(file_handle);
            Ok(file_handle)
        }
    }
}

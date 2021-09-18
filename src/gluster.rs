use errno::{errno, Errno};
use crate::glfs::*;
use libc::{
    c_uchar, c_void, dev_t, dirent, flock, ino_t, mode_t, stat, statvfs, timespec, DT_DIR, ENOENT,
    LOCK_EX, LOCK_SH, LOCK_UN,
};
use uuid::Uuid;

use std::error::Error as err;
use std::ffi::{CStr, CString, IntoStringError, NulError};
use std::fmt;
use std::io::Error;
use std::mem::zeroed;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::ptr;
use std::string::FromUtf8Error;

/// Custom error handling for the library
#[derive(Debug)]
pub enum GlusterError {
    BytesError(uuid::BytesError),
    Error(String),
    FromUtf8Error(FromUtf8Error),
    IntoStringError(IntoStringError),
    IoError(Error),
    NulError(NulError),
}

impl fmt::Display for GlusterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.description())
    }
}

impl err for GlusterError {
    fn description(&self) -> &str {
        match *self {
            GlusterError::BytesError(ref e) => e.description(),
            GlusterError::Error(ref e) => &e,
            GlusterError::FromUtf8Error(ref e) => e.description(),
            GlusterError::IntoStringError(ref e) => e.description(),
            GlusterError::IoError(ref e) => e.description(),
            GlusterError::NulError(ref e) => e.description(),
        }
    }
    fn cause(&self) -> Option<&dyn err> {
        match *self {
            GlusterError::BytesError(ref e) => e.cause(),
            GlusterError::Error(_) => None,
            GlusterError::FromUtf8Error(ref e) => e.cause(),
            GlusterError::IntoStringError(ref e) => e.cause(),
            GlusterError::IoError(ref e) => e.cause(),
            GlusterError::NulError(ref e) => e.cause(),
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
            GlusterError::BytesError(ref err) => err.description().to_string(),
            GlusterError::Error(ref err) => err.to_string(),
            GlusterError::FromUtf8Error(ref err) => err.utf8_error().to_string(),
            GlusterError::IntoStringError(ref err) => err.description().to_string(),
            GlusterError::IoError(ref err) => err.description().to_string(),
            GlusterError::NulError(ref err) => err.description().to_string(),
        }
    }
}

impl From<uuid::BytesError> for GlusterError {
    fn from(err: uuid::BytesError) -> GlusterError {
        GlusterError::BytesError(err)
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

//impl From<uuid::parser::ParseError> for GlusterError {
//fn from(err: uuid::parser::ParseError) -> GlusterError {
//GlusterError::ParseError(err)
//}
//}

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

#[repr(i32)]
#[derive(PartialEq, Debug, Hash)]
///  None to Trace correspond to the equivalent gluster log levels
pub enum GlusterLogLevel {
    None = 0,
    Emerg,
    Alert,
    Critical,
    Error,
    Warning,
    Notice,
    Info,
    Debug,
    Trace,
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
    cluster_handle: *mut glfs,
}

/// Gluster file descriptor
#[derive(Debug)]
pub struct GlusterFile {
    file_handle: *mut glfs_fd,
}

impl Drop for GlusterFile {
    fn drop(&mut self) {
        if self.file_handle.is_null() {
            // No cleanup needed
            return;
        }
        unsafe {
            let retcode = glfs_close(self.file_handle);
            if retcode < 0 {
                error!("{:?}", GlusterError::new(get_error()));
            }
        }
    }
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
            let retcode = glfs_fini(self.cluster_handle);
            if retcode < 0 {
                error!("{:?}", GlusterError::new(get_error()));
            }
        }
    }
}

/// This uses readdirplus which is very efficient in Gluster.  In addition
/// to returning directory entries this also stats each file.
#[derive(Debug)]
pub struct GlusterDirectoryPlus {
    pub dir_handle: *mut glfs_fd,
}

impl Drop for GlusterDirectoryPlus {
    fn drop(&mut self) {
        if self.dir_handle.is_null() {
            // No cleanup needed
            return;
        }
        unsafe {
            let retcode = glfs_closedir(self.dir_handle);
            if retcode < 0 {
                error!("{:?}", GlusterError::new(get_error()));
            }
        }
    }
}

pub struct DirEntryPlus {
    pub path: PathBuf,
    pub inode: ino_t,
    pub file_type: c_uchar,
    pub stat: stat,
}

impl Iterator for GlusterDirectoryPlus {
    type Item = Result<DirEntryPlus, GlusterError>;
    fn next(&mut self) -> Option<Self::Item> {
        let mut dirent: dirent = unsafe { zeroed() };
        let mut next_entry: *mut dirent = ptr::null_mut();
        unsafe {
            let mut stat_buf: stat = zeroed();
            let ret_code =
                glfs_readdirplus_r(self.dir_handle, &mut stat_buf, &mut dirent, &mut next_entry);
            if ret_code < 0 {
                return Some(Err(GlusterError::new(get_error())));
            }
            if dirent.d_ino == 0 {
                // End of stream reached
                return None;
            }
            let file_name = CStr::from_ptr(dirent.d_name.as_ptr());
            Some(Ok(DirEntryPlus {
                path: PathBuf::from(file_name.to_string_lossy().into_owned()),
                inode: dirent.d_ino,
                file_type: dirent.d_type,
                stat: stat_buf,
            }))
        }
    }
}

#[derive(Debug)]
pub struct GlusterDirectory {
    pub dir_handle: *mut glfs_fd,
}

impl Drop for GlusterDirectory {
    fn drop(&mut self) {
        if self.dir_handle.is_null() {
            // No cleanup needed
            return;
        }
        unsafe {
            let retcode = glfs_closedir(self.dir_handle);
            if retcode < 0 {
                error!("{:?}", GlusterError::new(get_error()));
            }
        }
    }
}

#[derive(Debug)]
pub struct DirEntry {
    pub path: PathBuf,
    pub inode: ino_t,
    pub file_type: c_uchar,
}

impl Iterator for GlusterDirectory {
    type Item = Result<DirEntry, GlusterError>;
    fn next(&mut self) -> Option<Self::Item> {
        let mut dirent: dirent = unsafe { zeroed() };
        let mut next_entry: *mut dirent = ptr::null_mut();
        unsafe {
            let ret_code = glfs_readdir_r(self.dir_handle, &mut dirent, &mut next_entry);
            if ret_code < 0 {
                return Some(Err(GlusterError::new(get_error())));
            }
            if dirent.d_ino == 0 {
                // End of stream reached
                return None;
            }
            let file_name = CStr::from_ptr(dirent.d_name.as_ptr());
            Some(Ok(DirEntry {
                path: PathBuf::from(file_name.to_string_lossy().into_owned()),
                inode: dirent.d_ino,
                file_type: dirent.d_type,
            }))
        }
    }
}

impl Gluster {
    /// Connect to a Ceph cluster and return a connection handle glfs_t
    /// port is usually 24007 but may differ depending on how the service was configured
    pub fn connect(volume_name: &str, server: &str, port: u16) -> Result<Gluster, GlusterError> {
        let vol_name = CString::new(volume_name)?;
        let vol_transport = CString::new("tcp")?;
        let vol_host = CString::new(server)?;
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
                // We call glfs_fini here because Gluster hasn't been created yet
                // so Drop won't be run.
                glfs_fini(cluster_handle);
                return Err(GlusterError::new(get_error()));
            }

            let ret_code = glfs_init(cluster_handle);
            if ret_code < 0 {
                // We call glfs_fini here because Gluster hasn't been created yet
                // so Drop won't be run.
                glfs_fini(cluster_handle);
                return Err(GlusterError::new(get_error()));
            }
            Ok(Gluster { cluster_handle })
        }
    }

    /// This function specifies logging parameters for the virtual mount.
    /// Sets the log file to write to
    pub fn set_logging(
        &self,
        logfile: &Path,
        loglevel: GlusterLogLevel,
    ) -> Result<(), GlusterError> {
        let path = CString::new(logfile.as_os_str().as_bytes())?;
        unsafe {
            let ret_code = glfs_set_logging(self.cluster_handle, path.as_ptr(), loglevel as i32);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }

    /// Get the volfile associated with the virtual mount
    /// Sometimes it's useful e.g. for scripts to see the volfile, so that they
    /// can parse it and find subvolumes to do things like split-brain resolution
    /// or custom layouts.
    /// Note that the volume must be started (not necessarily mounted) for this
    /// to work.  Also this function isn't very useful at the moment.  It needs
    /// to be parsed into a volume graph before it's really usable.  
    // TODO: Change this from String to a struct
    pub fn get_volfile(&self) -> Result<String, GlusterError> {
        // Start with 1K buffer and see if that works.  Even small clusters
        // have pretty large volfiles.
        let capacity = 1024;
        let mut buffer: Vec<u8> = Vec::with_capacity(capacity);
        unsafe {
            // This will likely fail and gluster will tell me the size it needs
            let ret = glfs_get_volfile(
                self.cluster_handle,
                buffer.as_mut_ptr() as *mut c_void,
                buffer.capacity() as usize,
            );
            if ret > 0 {
                //>0: filled N bytes of buffer
                buffer.truncate(ret as usize);
                buffer.set_len(ret as usize);
                return Ok(String::from_utf8_lossy(&buffer).into_owned());
            }
            if ret == 0 {
                //0: no volfile available
                return Err(GlusterError::new("No volfile available".into()));
            }
            if ret < 0 {
                // <0: volfile length exceeds @len by N bytes (@buf unchanged)
                trace!(
                    "volfile length is too large.  resizing to {}",
                    capacity + ret.abs() as usize
                );
                let mut buffer: Vec<u8> = Vec::with_capacity(capacity + ret.abs() as usize);
                let retry = glfs_get_volfile(
                    self.cluster_handle,
                    buffer.as_mut_ptr() as *mut c_void,
                    buffer.capacity() as usize,
                );
                if retry > 0 {
                    //>0: filled N bytes of buffer
                    buffer.truncate(retry as usize);
                    buffer.set_len(retry as usize);
                    return Ok(String::from_utf8_lossy(&buffer).into_owned());
                }
                if retry == 0 {
                    //0: no volfile available
                    return Err(GlusterError::new("No volfile available".into()));
                }
                if ret < 0 {
                    // I give up
                    return Err(GlusterError::new(
                        "volfile changed size while checking".into(),
                    ));
                }
            }
        }
        Err(GlusterError::new("Unknown error getting volfile".into()))
    }

    /// Fetch the volume uuid from the glusterd management server
    pub fn get_volume_id(&self) -> Result<Uuid, GlusterError> {
        // Give it plenty of room
        let mut buff: Vec<u8> = Vec::with_capacity(128);

        unsafe {
            let ret_code = glfs_get_volumeid(
                self.cluster_handle,
                buff.as_mut_ptr() as *mut i8,
                buff.capacity(),
            );
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
            // Inform Rust how many bytes gluster copied into the buffer
            buff.set_len(ret_code as usize);
        }
        let uuid = Uuid::from_slice(&buff)?;
        Ok(uuid)
    }

    pub fn open(&self, path: &Path, flags: i32) -> Result<GlusterFile, GlusterError> {
        let path = CString::new(path.as_os_str().as_bytes())?;
        unsafe {
            let file_handle = glfs_open(self.cluster_handle, path.as_ptr(), flags);
            if file_handle.is_null() {
                return Err(GlusterError::new(get_error()));
            }
            Ok(GlusterFile { file_handle })
        }
    }

    pub fn create(
        &self,
        path: &Path,
        flags: i32,
        mode: mode_t,
    ) -> Result<GlusterFile, GlusterError> {
        let path = CString::new(path.as_os_str().as_bytes())?;
        unsafe {
            let file_handle = glfs_creat(self.cluster_handle, path.as_ptr(), flags, mode);
            if file_handle.is_null() {
                return Err(GlusterError::new(get_error()));
            }
            Ok(GlusterFile { file_handle })
        }
    }
    pub fn truncate(&self, path: &Path, length: i64) -> Result<(), GlusterError> {
        let path = CString::new(path.as_os_str().as_bytes())?;

        unsafe {
            let ret_code = glfs_truncate(self.cluster_handle, path.as_ptr(), length);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn lsstat(&self, path: &Path) -> Result<stat, GlusterError> {
        let path = CString::new(path.as_os_str().as_bytes())?;
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
        let path = CString::new(path.as_os_str().as_bytes())?;
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

    pub fn statvfs(&self, path: &Path) -> Result<statvfs, GlusterError> {
        let path = CString::new(path.as_os_str().as_bytes())?;
        unsafe {
            let mut stat_buf: statvfs = zeroed();
            let ret_code = glfs_statvfs(self.cluster_handle, path.as_ptr(), &mut stat_buf);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
            Ok(stat_buf)
        }
    }

    pub fn stat(&self, path: &Path) -> Result<stat, GlusterError> {
        let path = CString::new(path.as_os_str().as_bytes())?;
        unsafe {
            let mut stat_buf: stat = zeroed();
            let ret_code = glfs_stat(self.cluster_handle, path.as_ptr(), &mut stat_buf);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
            Ok(stat_buf)
        }
    }
    pub fn access(&self, path: &Path, mode: i32) -> Result<(), GlusterError> {
        let path = CString::new(path.as_os_str().as_bytes())?;
        unsafe {
            let ret_code = glfs_access(self.cluster_handle, path.as_ptr(), mode);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }

    pub fn symlink(&self, oldpath: &Path, newpath: &Path) -> Result<(), GlusterError> {
        let old_path = CString::new(oldpath.as_os_str().as_bytes())?;
        let new_path = CString::new(newpath.as_os_str().as_bytes())?;
        unsafe {
            let ret_code = glfs_symlink(self.cluster_handle, old_path.as_ptr(), new_path.as_ptr());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }

    pub fn readlink(&self, path: &Path, buf: &mut [u8]) -> Result<(), GlusterError> {
        let path = CString::new(path.as_os_str().as_bytes())?;
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
        let path = CString::new(path.as_os_str().as_bytes())?;
        unsafe {
            let ret_code = glfs_mknod(self.cluster_handle, path.as_ptr(), mode, dev);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }

    pub fn mkdir(&self, path: &Path, mode: mode_t) -> Result<(), GlusterError> {
        let path = CString::new(path.as_os_str().as_bytes())?;
        unsafe {
            let ret_code = glfs_mkdir(self.cluster_handle, path.as_ptr(), mode);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }

    pub fn unlink(&self, path: &Path) -> Result<(), GlusterError> {
        let path = CString::new(path.as_os_str().as_bytes())?;
        unsafe {
            let ret_code = glfs_unlink(self.cluster_handle, path.as_ptr());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn rmdir(&self, path: &Path) -> Result<(), GlusterError> {
        let path = CString::new(path.as_os_str().as_bytes())?;
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
        let d = self.opendir(&p)?;
        for dir_entry in d {
            let dir_entry = dir_entry?;
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
                let d = self.opendir(&p)?;
                // If there's nothing in there remove the directory
                if self.is_empty(&p)? {
                    if p == path {
                        break
                    }
                    self.rmdir(&p)?;
                    // Remove this dir from the PathBuf
                    p.pop();
                    // Push it back onto the working stack because there
                    // might be more work needed
                    stack.push(p);
                    continue;
                }
                for dir_entry in d {
                    let dir_entry = dir_entry?;
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
                        _ => {
                            // Everything else gets unlinked
                            // chr, fifo, file, socket, symlink
                            let mut p = PathBuf::from(&p);
                            p.push(dir_entry.path);
                            trace!("unlink: {}", p.display());
                            self.unlink(&p)?;
                        }
                    }
                }
                if self.is_empty(&p)? && (p != path) {
                    self.rmdir(&p)?;
                    // There's a parent directory left to remove
                    if stack.len() == 0 {
                        if p.pop() {
                            stack.push(p);
                        }
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
        let old_path = CString::new(oldpath.as_os_str().as_bytes())?;
        let new_path = CString::new(newpath.as_os_str().as_bytes())?;
        unsafe {
            let ret_code = glfs_rename(self.cluster_handle, old_path.as_ptr(), new_path.as_ptr());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }

    pub fn link(&self, oldpath: &Path, newpath: &Path) -> Result<(), GlusterError> {
        let old_path = CString::new(oldpath.as_os_str().as_bytes())?;
        let new_path = CString::new(newpath.as_os_str().as_bytes())?;
        unsafe {
            let ret_code = glfs_link(self.cluster_handle, old_path.as_ptr(), new_path.as_ptr());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }

    pub fn opendir(&self, path: &Path) -> Result<GlusterDirectory, GlusterError> {
        let path = CString::new(path.as_os_str().as_bytes())?;
        unsafe {
            let dir_handle = glfs_opendir(self.cluster_handle, path.as_ptr());
            Ok(GlusterDirectory { dir_handle })
        }
    }

    // Readdir plus opendir
    pub fn opendir_plus(&self, path: &Path) -> Result<GlusterDirectoryPlus, GlusterError> {
        let path = CString::new(path.as_os_str().as_bytes())?;
        unsafe {
            let dir_handle = glfs_opendir(self.cluster_handle, path.as_ptr());
            Ok(GlusterDirectoryPlus { dir_handle })
        }
    }

    pub fn getxattr(&self, path: &Path, name: &str) -> Result<String, GlusterError> {
        let path = CString::new(path.as_os_str().as_bytes())?;
        let name = CString::new(name)?;
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
        let path = CString::new(path.as_os_str().as_bytes())?;
        let name = CString::new(name)?;
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

    pub fn listxattr(&self, path: &Path) -> Result<String, GlusterError> {
        let path = CString::new(path.as_os_str().as_bytes())?;
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
        let path = CString::new(path.as_os_str().as_bytes())?;
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
    pub fn setxattr(
        &self,
        path: &Path,
        name: &str,
        value: &[u8],
        flags: i32,
    ) -> Result<(), GlusterError> {
        let path = CString::new(path.as_os_str().as_bytes())?;
        let name = CString::new(name)?;
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
        let name = CString::new(name)?;
        let path = CString::new(path.as_os_str().as_bytes())?;
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
    pub fn removexattr(&self, path: &Path, name: &str) -> Result<(), GlusterError> {
        let path = CString::new(path.as_os_str().as_bytes())?;
        let name = CString::new(name)?;
        unsafe {
            let ret_code = glfs_removexattr(self.cluster_handle, path.as_ptr(), name.as_ptr());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn lremovexattr(&self, path: &Path, name: &str) -> Result<(), GlusterError> {
        let path = CString::new(path.as_os_str().as_bytes())?;
        let name = CString::new(name)?;
        unsafe {
            let ret_code = glfs_lremovexattr(self.cluster_handle, path.as_ptr(), name.as_ptr());
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
        let path = CString::new(path.as_os_str().as_bytes())?;
        unsafe {
            let ret_code = glfs_chdir(self.cluster_handle, path.as_ptr());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    /// times[0] specifies the new "last access time" (atime);
    /// times[1] specifies the new "last modification time" (mtime).
    pub fn utimens(&self, path: &Path, times: &[timespec; 2]) -> Result<(), GlusterError> {
        let path = CString::new(path.as_os_str().as_bytes())?;
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
        let path = CString::new(path.as_os_str().as_bytes())?;
        unsafe {
            let ret_code = glfs_lutimens(self.cluster_handle, path.as_ptr(), times.as_ptr());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn chmod(&self, path: &Path, mode: mode_t) -> Result<(), GlusterError> {
        let path = CString::new(path.as_os_str().as_bytes())?;
        unsafe {
            let ret_code = glfs_chmod(self.cluster_handle, path.as_ptr(), mode);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn chown(&self, path: &Path, uid: u32, gid: u32) -> Result<(), GlusterError> {
        let path = CString::new(path.as_os_str().as_bytes())?;
        unsafe {
            let ret_code = glfs_chown(self.cluster_handle, path.as_ptr(), uid, gid);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }

    pub fn lchown(&self, path: &Path, uid: u32, gid: u32) -> Result<(), GlusterError> {
        let path = CString::new(path.as_os_str().as_bytes())?;
        unsafe {
            let ret_code = glfs_lchown(self.cluster_handle, path.as_ptr(), uid, gid);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
}

impl GlusterFile {
    pub fn read(
        &self,
        fill_buffer: &mut Vec<u8>,
        count: usize,
        flags: i32,
    ) -> Result<isize, GlusterError> {
        self.pread(fill_buffer, count, 0, flags)
    }
    pub fn write(&self, buffer: &[u8], flags: i32) -> Result<isize, GlusterError> {
        self.pwrite(buffer, buffer.len(), 0, flags)
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
    pub fn readv(&self, iov: &mut [&mut [u8]], flags: i32) -> Result<isize, GlusterError> {
        unsafe {
            let read_size = glfs_readv(
                self.file_handle,
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
    pub fn writev(&self, iov: &[&[u8]], flags: i32) -> Result<isize, GlusterError> {
        unsafe {
            let write_size = glfs_writev(
                self.file_handle,
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
        fill_buffer: &mut Vec<u8>,
        count: usize,
        offset: i64,
        flags: i32,
    ) -> Result<isize, GlusterError> {
        unsafe {
            let read_size = glfs_pread(
                self.file_handle,
                fill_buffer.as_mut_ptr() as *mut c_void,
                count,
                offset,
                flags,
                std::ptr::null_mut(),
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
        buffer: &[u8],
        count: usize,
        offset: i64,
        flags: i32,
    ) -> Result<isize, GlusterError> {
        unsafe {
            let write_size = glfs_pwrite(
                self.file_handle,
                buffer.as_ptr() as *mut c_void,
                count,
                offset,
                flags,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            );
            if write_size < 0 {
                return Err(GlusterError::new(get_error()));
            }
            Ok(write_size)
        }
    }

    pub fn preadv(
        &self,
        iov: &mut [&mut [u8]],
        offset: i64,
        flags: i32,
    ) -> Result<isize, GlusterError> {
        unsafe {
            let read_size = glfs_preadv(
                self.file_handle,
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
    pub fn pwritev(&self, iov: &[&[u8]], offset: i64, flags: i32) -> Result<isize, GlusterError> {
        unsafe {
            let write_size = glfs_pwritev(
                self.file_handle,
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
    pub fn lseek(&self, offset: i64, whence: i32) -> Result<i64, GlusterError> {
        unsafe {
            let file_offset = glfs_lseek(self.file_handle, offset, whence);
            if file_offset < 0 {
                return Err(GlusterError::new(get_error()));
            }
            Ok(file_offset)
        }
    }
    pub fn ftruncate(&self, length: i64) -> Result<(), GlusterError> {
        unsafe {
            let ret_code = glfs_ftruncate(self.file_handle, length, std::ptr::null_mut(), std::ptr::null_mut());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn fstat(&self) -> Result<stat, GlusterError> {
        unsafe {
            let mut stat_buf: stat = zeroed();
            let ret_code = glfs_fstat(self.file_handle, &mut stat_buf);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
            Ok(stat_buf)
        }
    }
    pub fn fsync(&self) -> Result<(), GlusterError> {
        unsafe {
            let ret_code = glfs_fsync(self.file_handle, std::ptr::null_mut(), std::ptr::null_mut());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }

    pub fn fdatasync(&self) -> Result<(), GlusterError> {
        unsafe {
            let ret_code = glfs_fdatasync(self.file_handle, std::ptr::null_mut(), std::ptr::null_mut());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn fgetxattr(&self, name: &str) -> Result<String, GlusterError> {
        let name = CString::new(name)?;
        let mut xattr_val_buff: Vec<u8> = Vec::with_capacity(1024);
        unsafe {
            let ret_code = glfs_fgetxattr(
                self.file_handle,
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

    pub fn flistxattr(&self) -> Result<String, GlusterError> {
        let mut xattr_val_buff: Vec<u8> = Vec::with_capacity(1024);
        unsafe {
            let ret_code = glfs_flistxattr(
                self.file_handle,
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

    pub fn fsetxattr(&self, name: &str, value: &[u8], flags: i32) -> Result<(), GlusterError> {
        let name = CString::new(name)?;
        unsafe {
            let ret_code = glfs_fsetxattr(
                self.file_handle,
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
    pub fn fremovexattr(&self, name: &str) -> Result<(), GlusterError> {
        let name = CString::new(name)?;

        unsafe {
            let ret_code = glfs_fremovexattr(self.file_handle, name.as_ptr());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn fallocate(&self, offset: i64, keep_size: i32, len: usize) -> Result<(), GlusterError> {
        unsafe {
            let ret_code = glfs_fallocate(self.file_handle, keep_size, offset, len);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn discard(&self, offset: i64, len: usize) -> Result<(), GlusterError> {
        unsafe {
            let ret_code = glfs_discard(self.file_handle, offset, len);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn zerofill(&self, offset: i64, len: i64) -> Result<(), GlusterError> {
        unsafe {
            let ret_code = glfs_zerofill(self.file_handle, offset, len);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }

    pub fn fchdir(&self) -> Result<(), GlusterError> {
        unsafe {
            let ret_code = glfs_fchdir(self.file_handle);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    /// times[0] specifies the new "last access time" (atime);
    /// times[1] specifies the new "last modification time" (mtime).
    pub fn futimens(&self, times: &[timespec; 2]) -> Result<(), GlusterError> {
        unsafe {
            let ret_code = glfs_futimens(self.file_handle, times.as_ptr());
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }

    pub fn posixlock(&self, command: PosixLockCmd, flock: &mut flock) -> Result<(), GlusterError> {
        unsafe {
            let ret_code = glfs_posix_lock(self.file_handle, command.into(), flock);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn fchmod(&self, mode: mode_t) -> Result<(), GlusterError> {
        unsafe {
            let ret_code = glfs_fchmod(self.file_handle, mode);
            if ret_code < 0 {
                return Err(GlusterError::new(get_error()));
            }
        }
        Ok(())
    }
    pub fn fchown(&self, uid: u32, gid: u32) -> Result<(), GlusterError> {
        unsafe {
            let ret_code = glfs_fchown(self.file_handle, uid, gid);
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
    pub fn dup(&self) -> Result<GlusterFile, GlusterError> {
        unsafe {
            let file_handle = glfs_dup(self.file_handle);
            Ok(GlusterFile { file_handle })
        }
    }
}

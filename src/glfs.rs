#![allow(non_camel_case_types)]
use libc::{c_char, c_int, c_long, c_void, dev_t, dirent, gid_t, flock, mode_t, off_t, size_t,
           stat, ssize_t, statvfs, timespec, uid_t};

pub enum Struct_glfs { }
pub type glfs_t = Struct_glfs;
pub enum Struct_glfs_fd { }
pub type glfs_fd_t = Struct_glfs_fd;
pub type glfs_io_cbk = ::std::option::Option<
    extern "C" fn(fd: *mut glfs_fd_t,
                  ret: ssize_t,
                  data: *mut c_void)
                  -> (),
>;

#[repr(C)]
pub struct iovec {
    pub iov_base: *const c_void,
    pub iov_len: size_t,
}

#[link(name = "gfapi")]
extern "C" {
    /// Create a new 'virtual mount' object.
    /// This is most likely the very first function you will use. This function
    /// will create a new glfs_t (virtual mount) object in memory.
    /// On this newly created glfs_t, you need to be either set a volfile path
    /// (glfs_set_volfile) or a volfile server (glfs_set_volfile_server).
    /// The glfs_t object needs to be initialized with glfs_init() before you
    /// can start issuing file operations on it.
    pub fn glfs_new(volname: *const c_char) -> *mut glfs_t;

    /// Specify the path to the volume specification file.
    /// If you are using a static volume specification file (without dynamic
    /// volume management abilities from the CLI), then specify the path to
    /// the volume specification file.
    /// This is incompatible with glfs_set_volfile_server().
    pub fn glfs_set_volfile(fs: *mut glfs_t, volfile: *const c_char) -> c_int;
    /// Specify the list of addresses for management server.
    /// This function specifies the list of addresses for the management server
    /// (glusterd) to connect, and establish the volume configuration. The @volname
    /// parameter passed to glfs_new() is the volume which will be virtually
    /// mounted as the glfs_t object. All operations performed by the CLI at
    /// the management server will automatically be reflected in the 'virtual
    /// mount' object as it maintains a connection to glusterd and polls on
    /// configuration change notifications.
    ///  This is incompatible with glfs_set_volfile().
    pub fn glfs_set_volfile_server(
        fs: *mut glfs_t,
        transport: *const c_char,
        host: *const c_char,
        port: c_int,
    ) -> c_int;
    pub fn glfs_unset_volfile_server(
        fs: *mut glfs_t,
        transport: *const c_char,
        host: *const c_char,
        port: c_int,
    ) -> c_int;
    ///  This function specifies logging parameters for the virtual mount.
    /// Default log file is /dev/null.
    pub fn glfs_set_logging(fs: *mut glfs_t, logfile: *const c_char, loglevel: c_int) -> c_int;
    ///  This function initializes the glfs_t object. This consists of many steps:
    /// Spawn a poll-loop thread.
    /// Establish connection to management daemon and receive volume specification.
    /// Construct translator graph and initialize graph.
    /// Wait for initialization (connecting to all bricks) to complete.
    pub fn glfs_init(fs: *mut glfs_t) -> c_int;

    /// This function attempts to gracefully destroy glfs_t object. An attempt is
    /// made to wait for all background processing to complete before returning.
    /// glfs_fini() must be called after all operations on glfs_t is finished.
    pub fn glfs_fini(fs: *mut glfs_t) -> c_int;

    /// Get the volfile associated with the virtual mount
    /// Sometimes it's useful e.g. for scripts to see the volfile, so that they
    /// can parse it and find subvolumes to do things like split-brain resolution
    /// or custom layouts.  The API here was specifically intended to make access
    /// e.g. from Python as simple as possible.
    /// Note that the volume must be started (not necessarily mounted) for this
    /// to work.
    pub fn glfs_get_volfile(fs: *mut glfs_t, buf: *mut c_void, len: size_t) -> ssize_t;

    /// This function when invoked for the first time sends RPC call to the
    /// the management server (glusterd) to fetch volume uuid and stores it
    /// in the glusterfs_context linked to the glfs object fs which can be used
    /// in the subsequent calls. Later it parses that UUID to convert it from
    /// cannonical string format into an opaque byte array and copy it into
    /// the volid array. Incase if either of the input parameters, volid or size,
    /// is NULL, number of bytes required to copy the volume UUID is returned.
    pub fn glfs_get_volumeid(fs: *mut Struct_glfs, volid: *mut c_char, size: size_t) -> c_int;

    pub fn glfs_setfsuid(fsuid: uid_t) -> c_int;
    pub fn glfs_setfsgid(fsgid: gid_t) -> c_int;
    pub fn glfs_setfsgroups(size: size_t, list: *const gid_t) -> c_int;
    /// This function opens a file on a virtual mount.
    pub fn glfs_open(fs: *mut glfs_t, path: *const c_char, flags: c_int) -> *mut glfs_fd_t;
    /// This function opens a file on a virtual mount.
    pub fn glfs_creat(
        fs: *mut glfs_t,
        path: *const c_char,
        flags: c_int,
        mode: mode_t,
    ) -> *mut glfs_fd_t;
    pub fn glfs_close(fd: *mut glfs_fd_t) -> c_int;
    pub fn glfs_from_glfd(fd: *mut glfs_fd_t) -> *mut glfs_t;
    pub fn glfs_set_xlator_option(
        fs: *mut glfs_t,
        xlator: *const c_char,
        key: *const c_char,
        value: *const c_char,
    ) -> c_int;
    pub fn glfs_read(fd: *mut glfs_fd_t, buf: *mut c_void, count: size_t, flags: c_int) -> ssize_t;
    pub fn glfs_write(
        fd: *mut glfs_fd_t,
        buf: *const c_void,
        count: size_t,
        flags: c_int,
    ) -> ssize_t;
    pub fn glfs_read_async(
        fd: *mut glfs_fd_t,
        buf: *mut c_void,
        count: size_t,
        flags: c_int,
        _fn: glfs_io_cbk,
        data: *mut c_void,
    ) -> c_int;
    pub fn glfs_write_async(
        fd: *mut glfs_fd_t,
        buf: *const c_void,
        count: size_t,
        flags: c_int,
        _fn: glfs_io_cbk,
        data: *mut c_void,
    ) -> c_int;
    pub fn glfs_readv(
        fd: *mut glfs_fd_t,
        iov: *const iovec,
        iovcnt: c_int,
        flags: c_int,
    ) -> ssize_t;
    pub fn glfs_writev(
        fd: *mut glfs_fd_t,
        iov: *const iovec,
        iovcnt: c_int,
        flags: c_int,
    ) -> ssize_t;
    pub fn glfs_readv_async(
        fd: *mut glfs_fd_t,
        iov: *const iovec,
        count: c_int,
        flags: c_int,
        _fn: glfs_io_cbk,
        data: *mut c_void,
    ) -> c_int;
    pub fn glfs_writev_async(
        fd: *mut glfs_fd_t,
        iov: *const iovec,
        count: c_int,
        flags: c_int,
        _fn: glfs_io_cbk,
        data: *mut c_void,
    ) -> c_int;
    pub fn glfs_pread(
        fd: *mut glfs_fd_t,
        buf: *mut c_void,
        count: size_t,
        offset: off_t,
        flags: c_int,
    ) -> ssize_t;
    pub fn glfs_pwrite(
        fd: *mut glfs_fd_t,
        buf: *const c_void,
        count: size_t,
        offset: off_t,
        flags: c_int,
    ) -> ssize_t;
    pub fn glfs_pread_async(
        fd: *mut glfs_fd_t,
        buf: *mut c_void,
        count: size_t,
        offset: off_t,
        flags: c_int,
        _fn: glfs_io_cbk,
        data: *mut c_void,
    ) -> c_int;
    pub fn glfs_pwrite_async(
        fd: *mut glfs_fd_t,
        buf: *const c_void,
        count: c_int,
        offset: off_t,
        flags: c_int,
        _fn: glfs_io_cbk,
        data: *mut c_void,
    ) -> c_int;
    pub fn glfs_preadv(
        fd: *mut glfs_fd_t,
        iov: *const iovec,
        iovcnt: c_int,
        offset: off_t,
        flags: c_int,
    ) -> ssize_t;
    pub fn glfs_pwritev(
        fd: *mut glfs_fd_t,
        iov: *const iovec,
        iovcnt: c_int,
        offset: off_t,
        flags: c_int,
    ) -> ssize_t;
    pub fn glfs_preadv_async(
        fd: *mut glfs_fd_t,
        iov: *const iovec,
        count: c_int,
        offset: off_t,
        flags: c_int,
        _fn: glfs_io_cbk,
        data: *mut c_void,
    ) -> c_int;
    pub fn glfs_pwritev_async(
        fd: *mut glfs_fd_t,
        iov: *const iovec,
        count: c_int,
        offset: off_t,
        flags: c_int,
        _fn: glfs_io_cbk,
        data: *mut c_void,
    ) -> c_int;
    pub fn glfs_lseek(fd: *mut glfs_fd_t, offset: off_t, whence: c_int) -> off_t;
    pub fn glfs_truncate(fs: *mut glfs_t, path: *const c_char, length: off_t) -> c_int;
    pub fn glfs_ftruncate(fd: *mut glfs_fd_t, length: off_t) -> c_int;
    pub fn glfs_ftruncate_async(
        fd: *mut glfs_fd_t,
        length: off_t,
        _fn: glfs_io_cbk,
        data: *mut c_void,
    ) -> c_int;
    pub fn glfs_lstat(fs: *mut glfs_t, path: *const c_char, buf: *mut stat) -> c_int;
    pub fn glfs_stat(fs: *mut glfs_t, path: *const c_char, buf: *mut stat) -> c_int;
    pub fn glfs_fstat(fd: *mut glfs_fd_t, buf: *mut stat) -> c_int;
    pub fn glfs_fsync(fd: *mut glfs_fd_t) -> c_int;
    pub fn glfs_fsync_async(fd: *mut glfs_fd_t, _fn: glfs_io_cbk, data: *mut c_void) -> c_int;
    pub fn glfs_fdatasync(fd: *mut glfs_fd_t) -> c_int;
    pub fn glfs_fdatasync_async(fd: *mut glfs_fd_t, _fn: glfs_io_cbk, data: *mut c_void) -> c_int;
    pub fn glfs_access(fs: *mut glfs_t, path: *const c_char, mode: c_int) -> c_int;
    pub fn glfs_symlink(fs: *mut glfs_t, oldpath: *const c_char, newpath: *const c_char) -> c_int;
    pub fn glfs_readlink(
        fs: *mut glfs_t,
        path: *const c_char,
        buf: *mut c_char,
        bufsiz: size_t,
    ) -> c_int;
    pub fn glfs_mknod(fs: *mut glfs_t, path: *const c_char, mode: mode_t, dev: dev_t) -> c_int;
    pub fn glfs_mkdir(fs: *mut glfs_t, path: *const c_char, mode: mode_t) -> c_int;
    pub fn glfs_unlink(fs: *mut glfs_t, path: *const c_char) -> c_int;
    pub fn glfs_rmdir(fs: *mut glfs_t, path: *const c_char) -> c_int;
    pub fn glfs_rename(fs: *mut glfs_t, oldpath: *const c_char, newpath: *const c_char) -> c_int;
    pub fn glfs_link(fs: *mut glfs_t, oldpath: *const c_char, newpath: *const c_char) -> c_int;
    pub fn glfs_opendir(fs: *mut glfs_t, path: *const c_char) -> *mut glfs_fd_t;

    /// glfs_readdir_r and glfs_readdirplus_r ARE thread safe AND re-entrant,
    /// but the interface has ambiguity about the size of dirent to be allocated
    /// before calling the APIs. 512 byte buffer (for dirent) is sufficient for
    /// all known systems which are tested againt glusterfs/gfapi, but may be
    /// insufficient in the future.
    pub fn glfs_readdir_r(
        fd: *mut glfs_fd_t,
        dirent: *mut dirent,
        result: *mut *mut dirent,
    ) -> c_int;
    /// glfs_readdir_r and glfs_readdirplus_r ARE thread safe AND re-entrant,
    /// but the interface has ambiguity about the size of dirent to be allocated
    /// before calling the APIs. 512 byte buffer (for dirent) is sufficient for
    /// all known systems which are tested againt glusterfs/gfapi, but may be
    /// insufficient in the future.
    pub fn glfs_readdirplus_r(
        fd: *mut glfs_fd_t,
        stat: *mut stat,
        dirent: *mut dirent,
        result: *mut *mut dirent,
    ) -> c_int;

    /// glfs_readdir and glfs_readdirplus are NEITHER thread safe NOR re-entrant
    /// when called on the same directory handle. However they ARE thread safe
    /// AND re-entrant when called on different directory handles (which may be
    /// referring to the same directory too.)
    pub fn glfs_readdir(fd: *mut glfs_fd_t) -> *mut dirent;
    pub fn glfs_readdirplus(fd: *mut glfs_fd_t, stat: *mut stat) -> *mut dirent;
    pub fn glfs_telldir(fd: *mut glfs_fd_t) -> c_long;
    pub fn glfs_seekdir(fd: *mut glfs_fd_t, offset: c_long) -> ();
    pub fn glfs_closedir(fd: *mut glfs_fd_t) -> c_int;
    pub fn glfs_statvfs(fs: *mut glfs_t, path: *const c_char, buf: *mut statvfs) -> c_int;
    pub fn glfs_chmod(fs: *mut glfs_t, path: *const c_char, mode: mode_t) -> c_int;
    pub fn glfs_fchmod(fd: *mut glfs_fd_t, mode: mode_t) -> c_int;
    pub fn glfs_chown(fs: *mut glfs_t, path: *const c_char, uid: uid_t, gid: gid_t) -> c_int;
    pub fn glfs_lchown(fs: *mut glfs_t, path: *const c_char, uid: uid_t, gid: gid_t) -> c_int;
    pub fn glfs_fchown(fd: *mut glfs_fd_t, uid: uid_t, gid: gid_t) -> c_int;
    pub fn glfs_utimens(fs: *mut glfs_t, path: *const c_char, times: *const timespec) -> c_int;
    pub fn glfs_lutimens(fs: *mut glfs_t, path: *const c_char, times: *const timespec) -> c_int;
    pub fn glfs_futimens(fd: *mut glfs_fd_t, times: *const timespec) -> c_int;
    pub fn glfs_getxattr(
        fs: *mut glfs_t,
        path: *const c_char,
        name: *const c_char,
        value: *mut c_void,
        size: size_t,
    ) -> ssize_t;
    pub fn glfs_lgetxattr(
        fs: *mut glfs_t,
        path: *const c_char,
        name: *const c_char,
        value: *mut c_void,
        size: size_t,
    ) -> ssize_t;
    pub fn glfs_fgetxattr(
        fd: *mut glfs_fd_t,
        name: *const c_char,
        value: *mut c_void,
        size: size_t,
    ) -> ssize_t;
    pub fn glfs_listxattr(
        fs: *mut glfs_t,
        path: *const c_char,
        value: *mut c_void,
        size: size_t,
    ) -> ssize_t;
    pub fn glfs_llistxattr(
        fs: *mut glfs_t,
        path: *const c_char,
        value: *mut c_void,
        size: size_t,
    ) -> ssize_t;
    pub fn glfs_flistxattr(fd: *mut glfs_fd_t, value: *mut c_void, size: size_t) -> ssize_t;
    pub fn glfs_setxattr(
        fs: *mut glfs_t,
        path: *const c_char,
        name: *const c_char,
        value: *const c_void,
        size: size_t,
        flags: c_int,
    ) -> c_int;
    pub fn glfs_lsetxattr(
        fs: *mut glfs_t,
        path: *const c_char,
        name: *const c_char,
        value: *const c_void,
        size: size_t,
        flags: c_int,
    ) -> c_int;
    pub fn glfs_fsetxattr(
        fd: *mut glfs_fd_t,
        name: *const c_char,
        value: *const c_void,
        size: size_t,
        flags: c_int,
    ) -> c_int;
    pub fn glfs_removexattr(fs: *mut glfs_t, path: *const c_char, name: *const c_char) -> c_int;
    pub fn glfs_lremovexattr(fs: *mut glfs_t, path: *const c_char, name: *const c_char) -> c_int;
    pub fn glfs_fremovexattr(fd: *mut glfs_fd_t, name: *const c_char) -> c_int;
    pub fn glfs_fallocate(
        fd: *mut glfs_fd_t,
        keep_size: c_int,
        offset: off_t,
        len: size_t,
    ) -> c_int;
    pub fn glfs_discard(fd: *mut glfs_fd_t, offset: off_t, len: size_t) -> c_int;
    pub fn glfs_discard_async(
        fd: *mut glfs_fd_t,
        length: off_t,
        lent: size_t,
        _fn: glfs_io_cbk,
        data: *mut c_void,
    ) -> c_int;
    pub fn glfs_zerofill(fd: *mut glfs_fd_t, offset: off_t, len: off_t) -> c_int;
    pub fn glfs_zerofill_async(
        fd: *mut glfs_fd_t,
        length: off_t,
        len: off_t,
        _fn: glfs_io_cbk,
        data: *mut c_void,
    ) -> c_int;
    pub fn glfs_getcwd(fs: *mut glfs_t, buf: *mut c_char, size: size_t) -> *mut c_char;
    pub fn glfs_chdir(fs: *mut glfs_t, path: *const c_char) -> c_int;
    pub fn glfs_fchdir(fd: *mut glfs_fd_t) -> c_int;
    pub fn glfs_realpath(
        fs: *mut glfs_t,
        path: *const c_char,
        resolved_path: *mut c_char,
    ) -> *mut c_char;
    pub fn glfs_posix_lock(fd: *mut glfs_fd_t, cmd: c_int, flock: *mut flock) -> c_int;
    pub fn glfs_dup(fd: *mut glfs_fd_t) -> *mut glfs_fd_t;
}

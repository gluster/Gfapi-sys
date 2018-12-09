//! GlusterFS API bindings
//! GlusterFS is a scalable network filesystem suitable for data-intensive
//! tasks such as cloud storage and media streaming.
//! This crate exposes the glfs module for low level interaction with the api.
//! It also exposes a set of safe wrappers in the gluster module

#[macro_use]
extern crate log;

pub mod glfs;
pub mod gluster;

use crate::jdwp::JdwpJavaVirtualMachine;
use jdwp::JdwpConnection;
use model::JavaVirtualMachine;
use std::io::Result;
use std::net::ToSocketAddrs;

#[macro_use]
extern crate num_derive;

// These shouldn't be 'pub' long term, maybe?
pub mod hprof;
pub mod jdwp;
pub mod model;

pub fn attach_live<A: ToSocketAddrs>(jvm_debug_addr: A) -> Result<Box<dyn JavaVirtualMachine>> {
    Ok(Box::new(JdwpJavaVirtualMachine::new(JdwpConnection::new(
        jvm_debug_addr,
    )?)))
}

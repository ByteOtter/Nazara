//! # Translator Module
//!
//! This module handles the translation and processing of the data sent to or received from NetBox.
use thanix_client::types::{
    WritableDeviceWithConfigContextRequest, WritableVirtualMachineWithConfigContextRequest,
};

use crate::Machine;

/// Translate the machine information to a `WritableDeviceWithConfigContextRequest` required by
/// NetBox's API.
pub fn information_to_device(machine: &Machine) -> WritableDeviceWithConfigContextRequest {
    todo!()
}

pub fn information_to_vm(machine: &Machine) -> WritableVirtualMachineWithConfigContextRequest {
    todo!()
}

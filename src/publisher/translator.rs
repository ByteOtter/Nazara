//! # Translator Module
//!
//! This module handles the translation and processing of the data sent to or received from NetBox.
//!
use core::net::IpAddr;
use std::process;
use std::str::FromStr;
use thanix_client::paths::{
    self, DcimPlatformsListQuery, DcimSitesListQuery, IpamIpAddressesListQuery,
};
use thanix_client::types::{
    IPAddress, Platform, Site, WritableDeviceWithConfigContextRequest, WritableIPAddressRequest,
    WritableInterfaceRequest, WritableVirtualMachineWithConfigContextRequest,
};
use thanix_client::util::ThanixClient;

use crate::collectors::network_collector::NetworkInformation;
use crate::{configuration::config_parser::ConfigData, Machine};

/// Translate the machine information to a `WritableDeviceWithConfigContextRequest` required by
/// NetBox's API.
///
/// *Certain information provided in the config file, like the CPU platform, will be overwritten
/// if another one is detected by the collector!*
///
/// # Parameters
///
/// - state: `&ThanixClient` - API Client instance used for search and validation.
/// - machine: `&Machine` - Collected information about the device.
/// - config_data: `ConfigData` - Additional information about the device provided by config file
/// or CLI.
///
/// # Returns
///
/// - device: `WritableDeviceWithConfigContextRequest` - Payload for machine creation request
pub fn information_to_device(
    state: &ThanixClient,
    machine: &Machine,
    config_data: ConfigData,
) -> WritableDeviceWithConfigContextRequest {
    println!("Creating Device object...");

    let wanted_platform: Option<String> = if let Some(arch) =
        machine.dmi_information.cpu_information.arch.as_ref()
    {
        println!("\x1b[36m[info]\x1b[0m CPU architecture was collected. Used by default, overriding possible config options...");
        Some(arch.clone())
    } else if let Some(config_value) = config_data.system.platform_name.as_ref() {
        println!(
            "\x1b[36m[info]\x1b[0m Architecture was not collected. Using config specifications..."
        );
        Some(config_value.clone())
    } else {
        println!(
            "[\x1b[33m[warning]\x1b[0m No cpu architecture specified. Proceeding with 'none'..."
        );
        None
    };

    let mut payload: WritableDeviceWithConfigContextRequest =
        WritableDeviceWithConfigContextRequest::default();

    payload.name = Some(config_data.system.name.clone());
    payload.device_type = config_data.system.device_type;
    payload.role = config_data.system.device_role;
    payload.tenant = config_data.system.tenant;
    payload.platform = match wanted_platform {
        Some(platform_name) => get_platform_id(&state, platform_name),
        None => None,
    };
    payload.serial = machine.dmi_information.system_information.serial.clone();
    // payload.asset_tag = todo!();
    payload.site = match get_site_id(state, &config_data) {
        Some(site_id) => site_id,
        None => {
            eprintln!(
                "\x1b[31m[error]\x1b[0m An Error occured while validating the site ID or name."
            );
            process::exit(1);
        }
    };
    payload.rack = config_data.system.rack;
    payload.face = config_data.system.face;
    // payload.position = todo!();
    // payload.longitude = todo!();
    // payload.latitude = todo!();
    payload.status = config_data.system.status;
    payload.airflow = config_data.system.airflow;
    payload.comments = config_data.system.comments;
    // payload.config_template = todo!();
    payload.custom_fields = config_data.system.custom_fields;
    payload.description = config_data.system.description;
    // payload.local_context_data = todo!();
    // payload.oob_ip = todo!();
    payload.primary_ip4 = get_primary_addresses(
        state,
        machine,
        config_data
            .system
            .primary_network_interface
            .clone()
            .unwrap(),
    );
    payload.primary_ip6 = get_primary_addresses(
        state,
        machine,
        config_data.system.primary_network_interface.unwrap(),
    );
    // payload.tags = todo!();
    // payload.virtual_chassis = todo!();
    // payload.vc_position = todo!();
    // payload.vc_priority = todo!();
    payload.tenant = config_data.system.tenant;
    payload.location = config_data.system.location;

    payload
}

/// Translate gathered information about the virtual machine into a usable Payload.
///
/// # Parameters
///
/// * state: `&ThanixClient` - The client instance to be used for communication.
/// * machine: `&Machine` - The collected information about the virtual machine.
/// * config_data: `&ConfigData` - Data parsed from the `nazar-config.toml`.
///
/// # Returns
///
/// * payload: `WritableVirtualMachineWithConfigContextRequest` - Payload for the VM POST or UPDATE
/// request.
pub fn information_to_vm(
    state: &ThanixClient,
    machine: &Machine,
    config_data: ConfigData,
) -> WritableVirtualMachineWithConfigContextRequest {
    todo!("Translation of collected information to VM not implemented yet!")
}

/// Translate gathered information into a `WritableInterfaceRequest` payload.
///
/// # Parameters
///
/// * state: `&ThanixClient` - The client instance to be used for communication.
/// * machine: `&Machine` - The collectedd information about the device or machine.
/// * config_data: `ConfigData` - The configuration data.
/// * device_id: `&i64` - The ID of the device that this interface belongs to.
///
/// # Returns
///
/// * payload: `WritableInterfaceRequest` - Payload for creating an interface.
pub fn information_to_interface(
    machine: &Machine,
    config_data: ConfigData,
    device_id: &i64,
) -> WritableInterfaceRequest {
    println!("Creating Network Interface...");

    let mut payload: WritableInterfaceRequest = WritableInterfaceRequest::default();

    payload.device = Some(device_id.to_owned());
    payload.name = config_data.system.primary_network_interface.clone();

    // FIXME:
    payload.r#type = Some(config_data.nwi.r#type);
    payload.parent = config_data.nwi.parent;
    payload.bridge = config_data.nwi.bridge;
    payload.lag = config_data.nwi.lag;
    payload.mtu = config_data.nwi.mtu;

    // Get the interface we are looking for as primary, then get its parameters.
    // These filter statements can probably be split off into their own function.
    payload.mac_address = match &config_data.system.primary_network_interface {
        Some(nwi_name) => {
            let interface = machine
                .network_information
                .iter()
                .find(|nwi| nwi.name == nwi_name.to_owned());
            interface.map(|nwi| nwi.mac_addr.clone()).flatten()
        }
        None => None,
    };
    payload.speed = match config_data.system.primary_network_interface {
        Some(nwi_name) => {
            let interface = machine
                .network_information
                .iter()
                .find(|nwi| nwi.name == nwi_name);
            interface.map(|nwi| nwi.interface_speed.clone()).flatten()
        }
        None => None,
    };
    payload.description = Some(String::from(
        "This interface was automatically created by Nazara.",
    ));
    payload.mode = Some(config_data.nwi.mode.unwrap_or(String::from("")));
    payload.rf_role = Some(config_data.nwi.rf_role.unwrap_or(String::from("")));
    payload.rf_channel = Some(config_data.nwi.rf_channel.unwrap_or(String::from("")));
    payload.poe_mode = Some(config_data.nwi.poe_mode.unwrap_or(String::from("")));
    payload.poe_type = Some(config_data.nwi.poe_type.unwrap_or(String::from("")));
    payload.custom_fields = config_data.nwi.custom_fields;
    payload.mark_connected = Some(config_data.nwi.mark_connected);
    payload.enabled = Some(config_data.nwi.enabled);
    payload.vdcs = Some(config_data.nwi.vdcs.unwrap_or_default());
    payload.label = Some(config_data.nwi.label.unwrap_or_default());
    payload.mgmt_only = Some(config_data.nwi.mgmt_only);
    payload.tagged_vlans = Some(config_data.nwi.tagged_vlans.unwrap_or_default());
    payload.wireless_lans = Some(config_data.nwi.wireless_lans.unwrap_or_default());
    payload.tags = Some(Vec::new());

    payload
}

/// Returns the payload necessary to create a new IP address.
///
/// # Parameters
///
/// * state: `&ThanixClient` - The client instance necessary for communication.
/// * machine: `&Machine` - Collected machine information.
/// * config_data: `&ConfigData` - Data read from the config file.
/// * interface_id: `i64` - ID of the network interface this IP belongs to.
pub fn information_to_ip(
    machine: &Machine,
    config_data: &ConfigData,
    interface_id: i64,
) -> WritableIPAddressRequest {
    println!("Creating IP Address payload...");

    let mut payload: WritableIPAddressRequest = WritableIPAddressRequest::default();

    let local_interface: &NetworkInformation = machine
        .network_information
        .iter()
        .find(|s| {
            s.name
                == <std::option::Option<std::string::String> as Clone>::clone(
                    &config_data.system.primary_network_interface,
                )
                .unwrap()
        })
        .unwrap();
    payload.address = format!("{}", local_interface.v4ip.unwrap());
    // payload.vrf = todo!();
    // payload.tenant = todo!();
    payload.status = String::from("active");
    // payload.role = todo!();
    payload.assigned_object_type = Some(String::from("dcim.interface"));
    payload.assigned_object_id = Some(interface_id as u64);
    // payload.nat_inside = todo!();
    // payload.dns_name = todo!();
    payload.description = String::from("This Address was automatically created by Nazara.");
    payload.comments = String::from("Automatically created by Nazara. Dummy only.");
    // payload.tags = todo!();
    payload.custom_fields = config_data.nwi.custom_fields.clone();

    payload
}

/// Returns the ID of the platform this machine uses.
///
/// # Parameters
///
/// * state: `&ThanixClient` - The client required for searching for the platform.
///
/// # Returns
///
/// Returns `Some(i64)` if the specified platform exists, else returns `None`.
///
/// # Aborts
///
/// If the netBox connection fails, this may terimnate the process.
fn get_platform_id(state: &ThanixClient, platform_name: String) -> Option<i64> {
    println!("Searching for id of platform '{}' ... ", platform_name);
    let platform_list: Vec<Platform>;

    match paths::dcim_platforms_list(&state, DcimPlatformsListQuery::default()) {
        Ok(response) => {
            println!("List received. Analyzing...");

            platform_list = match response {
                paths::DcimPlatformsListResponse::Http200(platforms) => platforms.results,
                _ => {
                    todo!("Handling of non 200 Response code when getting platforms not implemented yet.")
                }
            };
        }
        Err(e) => {
            eprintln!(
                "[\x1b[31m[error]\x1b[0m Failure while receiving list of platforms.\n{}",
                e
            );
            process::exit(1);
        }
    };

    for platform in platform_list {
        if platform.name == platform_name {
            println!("\x1b[32m[success]\x1b[0m Platform ID found. Proceeding...");
            return Some(platform.id);
        }
    }
    None
}

/// Returns the ID of the ipv4 Adress linked to this device if it exists.
/// If not, a new IPv4 Adress object will be created in NetBox.
///
/// The function will retrieve a list of IPv4 Adresses from NetBox,
/// then search this list for the IP Adress Nazara collected.
///
/// The `primary_network_interface` paramter specified in the `nazara_config.toml`
/// will be used to specify which adress to search for.
///
/// # Parameters
///
/// * state: `&ThanixClient` - The client required for making API requests.
/// * machine: `&Machine` - The collected machine information.
///
/// # Returns
///
/// Returns the ID of the IP address object `i64` if a match has been found.
/// Else returns `None`.
fn get_primary_addresses(
    state: &ThanixClient,
    machine: &Machine,
    preferred_nwi: String,
) -> Option<i64> {
    println!("Retrieving list of Addresses...");
    let ip_list: Vec<IPAddress>;
    let key_nwi: &NetworkInformation;

    if let Some(nwi_match) = machine
        .network_information
        .iter()
        .find(|nwi| nwi.name == preferred_nwi)
    {
        key_nwi = nwi_match;
    } else {
        eprintln!(
            "\x1b[31m[error] Specified Network Interface '{}' not found!",
            preferred_nwi
        );
        process::exit(1);
    };

    // TODO: Split this API call off so it is only done once.
    match paths::ipam_ip_addresses_list(&state, IpamIpAddressesListQuery::default()) {
        Ok(response) => {
            println!("IPAddress list received. Analyzing...");

            ip_list = match response {
                paths::IpamIpAddressesListResponse::Http200(adresses) => adresses.results,
                paths::IpamIpAddressesListResponse::Other(response) => {
                    eprintln!("\x1b[31m[error]\x1b[0m Failure while trying to retrieve list of IPAddresses. \n --- Unexpected response: {} ---",
                    response.text().unwrap()
                    );
                    process::exit(1);
                }
            };
        }
        Err(e) => {
            eprintln!(
                "\x1b[31m[error]\x1b[0m Failure while retrieving list of IPv4 Adresses.\n --- Unexpected response: {} ---",
                e
            );
            process::exit(1);
        }
    }

    let mut result: Option<i64> = None;

    for (idx, addr) in ip_list.iter().enumerate() {
        print! {"Searching for matching IP Adress... ({:?}/{:?})\r", idx+1, ip_list.len()};
        let ip = IpAddr::from_str(addr.address.clone().split("/").next().unwrap()).unwrap(); // TODO: Errorhandling
        match ip {
            IpAddr::V4(x) => match key_nwi.v4ip {
                Some(y) => {
                    if x == y {
                        result = Some(addr.id);
                    }
                }
                None => todo!(),
            },
            IpAddr::V6(x) => match key_nwi.v6ip {
                Some(y) => {
                    if x == y {
                        result = Some(addr.id);
                    }
                }
                None => todo!(),
            },
        }
    }
    result
}

/// Search for the site specified in the config file by ID or by name.
///
/// # Parameters
///
/// * state: `&ThanixClient` - The client required for performing API requests.
/// * config_data: `&ConfigData` - The configuration data found in the config file.
///
/// # Returns
///
/// * site_id: `i64` - The ID of the site if found. If not found, returns 0.
///
/// # Aborts
///
/// Unexpected API responses may terminate the process.
fn get_site_id(state: &ThanixClient, config_data: &ConfigData) -> Option<i64> {
    println!("Searching for site...");
    if config_data.system.site_id.is_some() {
        // Check if site with given ID exists.
        let target = match paths::dcim_sites_retrieve(state, config_data.system.site_id.unwrap()) {
            Ok(response) => match response {
                paths::DcimSitesRetrieveResponse::Http200(site) => site.id,
                paths::DcimSitesRetrieveResponse::Other(response) => {
                    eprintln!(
                        "\x1b[31m[error]\x1b[0m Error while searching for site by site_id.\n--- Unexpected response: {} ---",
                            response.text().unwrap()
                    );
                    process::exit(1);
                }
            },
            Err(e) => {
                eprintln!(
                    "\x1b[31m[error]\x1b[0m Error while searching for site.\n{}",
                    e
                );
                process::exit(1);
            }
        };
        println!("\x1b[32m[success]\x1b[0m Valid site ID. Proceeding...");
        return Some(target);
    } else {
        println!("\x1b[36m[info]\x1b[0m No 'site_id' specified. Searching by name...");
        let site_list: Vec<Site>;
        match paths::dcim_sites_list(state, DcimSitesListQuery::default()) {
            Ok(response) => match response {
                paths::DcimSitesListResponse::Http200(sites) => site_list = sites.results,
                paths::DcimSitesListResponse::Other(response) => {
                    eprintln!("\x1b[31[error] Error while retrieving site list.\n--- Unexpected response: {} ---",
                    response.text().unwrap()
                    );
                    process::exit(1);
                }
            },
            Err(e) => {
                eprintln!(
                    "\x1b[31m[error]\x1b[0m Error while performing site list query.\n{}",
                    e
                );
                process::exit(1);
            }
        }
        let target: String = config_data.system.site_name.clone().unwrap();

        return Some(
            site_list
                .iter()
                .find(|site| &site.name == &target)
                .unwrap()
                .id,
        );
    }
    None
}

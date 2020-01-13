use failure::*;
use lazy_static::lazy_static;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};

use proxmox::api::{api, schema::*};
use proxmox::tools::{fs::replace_file, fs::CreateOptions};

use crate::api2::types::*;
use crate::section_config::{SectionConfig, SectionConfigData, SectionConfigPlugin};

lazy_static! {
    static ref CONFIG: SectionConfig = init();
}

// fixme: define better schemas

pub const DIR_NAME_SCHEMA: Schema = StringSchema::new("Directory name").schema();

#[api(
    properties: {
        comment: {
            optional: true,
            schema: SINGLE_LINE_COMMENT_SCHEMA,
        },
        path: {
            schema: DIR_NAME_SCHEMA,
        },
    }
)]
#[derive(Serialize,Deserialize)]
/// Datastore configuration properties.
pub struct DataStoreConfig {
    pub comment: Option<String>,
    pub path: String,
 }

fn init() -> SectionConfig {
    let obj_schema = match DataStoreConfig::API_SCHEMA {
        Schema::Object(ref obj_schema) => obj_schema,
        _ => unreachable!(),
    };

    let plugin = SectionConfigPlugin::new("datastore".to_string(), obj_schema);
    let mut config = SectionConfig::new(&DATASTORE_SCHEMA);
    config.register_plugin(plugin);

    config
}

const DATASTORE_CFG_FILENAME: &str = "/etc/proxmox-backup/datastore.cfg";

pub fn config() -> Result<SectionConfigData, Error> {
    let content = match std::fs::read_to_string(DATASTORE_CFG_FILENAME) {
        Ok(c) => c,
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                String::from("")
            } else {
                bail!("unable to read '{}' - {}", DATASTORE_CFG_FILENAME, err);
            }
        }
    };

    CONFIG.parse(DATASTORE_CFG_FILENAME, &content)
}

pub fn save_config(config: &SectionConfigData) -> Result<(), Error> {
    let raw = CONFIG.write(DATASTORE_CFG_FILENAME, &config)?;

    let backup_user = crate::backup::backup_user()?;
    let mode = nix::sys::stat::Mode::from_bits_truncate(0o0640);
    // set the correct owner/group/permissions while saving file
    // owner(rw) = root, group(r)= backup
    let options = CreateOptions::new()
        .perm(mode)
        .owner(nix::unistd::ROOT)
        .group(backup_user.gid);

    replace_file(DATASTORE_CFG_FILENAME, raw.as_bytes(), options)?;

    Ok(())
}

// shell completion helper
pub fn complete_datastore_name(_arg: &str, _param: &HashMap<String, String>) -> Vec<String> {
    match config() {
        Ok(data) => data.sections.iter().map(|(id, _)| id.to_string()).collect(),
        Err(_) => return vec![],
    }
}

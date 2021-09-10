use anyhow::{bail, Error};
use ::serde::{Deserialize, Serialize};

use proxmox::{
    api::{
        api,
        Router,
        RpcEnvironment,
        Permission,
    },
};

use pbs_api_types::{
    Authid, MediaPoolConfig, MediaPoolConfigUpdater, MEDIA_POOL_NAME_SCHEMA,
    PRIV_TAPE_AUDIT, PRIV_TAPE_MODIFY,
};

use pbs_config::CachedUserInfo;

#[api(
    protected: true,
    input: {
        properties: {
            config: {
                type: MediaPoolConfig,
                flatten: true,
            },
        },
    },
    access: {
        permission: &Permission::Privilege(&["tape", "pool"], PRIV_TAPE_MODIFY, false),
    },
)]
/// Create a new media pool
pub fn create_pool(
    config: MediaPoolConfig,
) -> Result<(), Error> {

    let _lock = pbs_config::media_pool::lock()?;

    let (mut section_config, _digest) = pbs_config::media_pool::config()?;

    if section_config.sections.get(&config.name).is_some() {
        bail!("Media pool '{}' already exists", config.name);
    }

    section_config.set_data(&config.name, "pool", &config)?;

    pbs_config::media_pool::save_config(&section_config)?;

    Ok(())
}

#[api(
    returns: {
        description: "The list of configured media pools (with config digest).",
        type: Array,
        items: {
            type: MediaPoolConfig,
        },
    },
    access: {
        description: "List configured media pools filtered by Tape.Audit privileges",
        permission: &Permission::Anybody,
    },
)]
/// List media pools
pub fn list_pools(
    mut rpcenv: &mut dyn RpcEnvironment,
) -> Result<Vec<MediaPoolConfig>, Error> {
    let auth_id: Authid = rpcenv.get_auth_id().unwrap().parse()?;
    let user_info = CachedUserInfo::new()?;

    let (config, digest) = pbs_config::media_pool::config()?;

    let list = config.convert_to_typed_array::<MediaPoolConfig>("pool")?;

     let list = list
        .into_iter()
        .filter(|pool| {
            let privs = user_info.lookup_privs(&auth_id, &["tape", "pool", &pool.name]);
            privs & PRIV_TAPE_AUDIT != 0
        })
        .collect();

    rpcenv["digest"] = proxmox::tools::digest_to_hex(&digest).into();

    Ok(list)
}

#[api(
    input: {
        properties: {
            name: {
                schema: MEDIA_POOL_NAME_SCHEMA,
            },
        },
    },
    returns: {
        type: MediaPoolConfig,
    },
    access: {
        permission: &Permission::Privilege(&["tape", "pool", "{name}"], PRIV_TAPE_AUDIT, false),
    },
)]
/// Get media pool configuration
pub fn get_config(name: String) -> Result<MediaPoolConfig, Error> {

    let (config, _digest) = pbs_config::media_pool::config()?;

    let data: MediaPoolConfig = config.lookup("pool", &name)?;

    Ok(data)
}

#[api()]
#[derive(Serialize, Deserialize)]
#[allow(non_camel_case_types)]
/// Deletable property name
pub enum DeletableProperty {
    /// Delete media set allocation policy.
    allocation,
    /// Delete pool retention policy
    retention,
    /// Delete media set naming template
    template,
    /// Delete encryption fingerprint
    encrypt,
    /// Delete comment
    comment,
}

#[api(
    protected: true,
    input: {
        properties: {
            name: {
                schema: MEDIA_POOL_NAME_SCHEMA,
            },
            update: {
                type: MediaPoolConfigUpdater,
                flatten: true,
            },
            delete: {
                description: "List of properties to delete.",
                type: Array,
                optional: true,
                items: {
                    type: DeletableProperty,
                }
            },
       },
    },
    access: {
        permission: &Permission::Privilege(&["tape", "pool", "{name}"], PRIV_TAPE_MODIFY, false),
    },
)]
/// Update media pool settings
pub fn update_pool(
    name: String,
    update: MediaPoolConfigUpdater,
    delete: Option<Vec<DeletableProperty>>,
) -> Result<(), Error> {

    let _lock = pbs_config::media_pool::lock()?;

    let (mut config, _digest) = pbs_config::media_pool::config()?;

    let mut data: MediaPoolConfig = config.lookup("pool", &name)?;

    if let Some(delete) = delete {
        for delete_prop in delete {
            match delete_prop {
                DeletableProperty::allocation => { data.allocation = None; },
                DeletableProperty::retention => { data.retention = None; },
                DeletableProperty::template => { data.template = None; },
                DeletableProperty::encrypt => { data.encrypt = None; },
                DeletableProperty::comment => { data.comment = None; },
            }
        }
    }

    if update.allocation.is_some() { data.allocation = update.allocation; }
    if update.retention.is_some() { data.retention = update.retention; }
    if update.template.is_some() { data.template = update.template; }
    if update.encrypt.is_some() { data.encrypt = update.encrypt; }

    if let Some(comment) = update.comment {
        let comment = comment.trim();
        if comment.is_empty() {
            data.comment = None;
        } else {
            data.comment = Some(comment.to_string());
        }
    }

    config.set_data(&name, "pool", &data)?;

    pbs_config::media_pool::save_config(&config)?;

    Ok(())
}

#[api(
    protected: true,
    input: {
        properties: {
            name: {
                schema: MEDIA_POOL_NAME_SCHEMA,
            },
        },
    },
    access: {
        permission: &Permission::Privilege(&["tape", "pool", "{name}"], PRIV_TAPE_MODIFY, false),
    },
)]
/// Delete a media pool configuration
pub fn delete_pool(name: String) -> Result<(), Error> {

    let _lock = pbs_config::media_pool::lock()?;

    let (mut config, _digest) = pbs_config::media_pool::config()?;

    match config.sections.get(&name) {
        Some(_) => { config.sections.remove(&name); },
        None => bail!("delete pool '{}' failed - no such pool", name),
    }

    pbs_config::media_pool::save_config(&config)?;

    Ok(())
}

const ITEM_ROUTER: Router = Router::new()
    .get(&API_METHOD_GET_CONFIG)
    .put(&API_METHOD_UPDATE_POOL)
    .delete(&API_METHOD_DELETE_POOL);


pub const ROUTER: Router = Router::new()
    .get(&API_METHOD_LIST_POOLS)
    .post(&API_METHOD_CREATE_POOL)
    .match_all("name", &ITEM_ROUTER);

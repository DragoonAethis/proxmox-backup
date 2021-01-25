//! Manage Access Control Lists

use anyhow::{bail, Error};

use proxmox::api::{api, Router, RpcEnvironment, Permission};
use proxmox::tools::fs::open_file_locked;

use crate::api2::types::*;
use crate::config::acl;
use crate::config::acl::{Role, PRIV_SYS_AUDIT, PRIV_PERMISSIONS_MODIFY};
use crate::config::cached_user_info::CachedUserInfo;

fn extract_acl_node_data(
    node: &acl::AclTreeNode,
    path: &str,
    list: &mut Vec<AclListItem>,
    exact: bool,
    token_user: &Option<Authid>,
) {
    // tokens can't have tokens, so we can early return
    if let Some(token_user) = token_user {
        if token_user.is_token() {
            return;
        }
    }

    for (user, roles) in &node.users {
        if let Some(token_user) = token_user {
            if !user.is_token()
                || user.user() != token_user.user() {
                 continue;
            }
        }

        for (role, propagate) in roles {
            list.push(AclListItem {
                path: if path.is_empty() { String::from("/") } else { path.to_string() },
                propagate: *propagate,
                ugid_type: String::from("user"),
                ugid: user.to_string(),
                roleid: role.to_string(),
            });
        }
    }
    for (group, roles) in &node.groups {
        if token_user.is_some() {
            continue;
        }

        for (role, propagate) in roles {
            list.push(AclListItem {
                path: if path.is_empty() { String::from("/") } else { path.to_string() },
                propagate: *propagate,
                ugid_type: String::from("group"),
                ugid: group.to_string(),
                roleid: role.to_string(),
            });
        }
    }
    if exact {
        return;
    }
    for (comp, child) in &node.children {
        let new_path = format!("{}/{}", path, comp);
        extract_acl_node_data(child, &new_path, list, exact, token_user);
    }
}

#[api(
    input: {
        properties: {
	    path: {
                schema: ACL_PATH_SCHEMA,
                optional: true,
            },
            exact: {
                description: "If set, returns only ACL for the exact path.",
                type: bool,
                optional: true,
                default: false,
            },
        },
    },
    returns: {
        description: "ACL entry list.",
        type: Array,
        items: {
            type: AclListItem,
        }
    },
    access: {
        permission: &Permission::Anybody,
        description: "Returns all ACLs if user has Sys.Audit on '/access/acl', or just the ACLs containing the user's API tokens.",
    },
)]
/// Read Access Control List (ACLs).
pub fn read_acl(
    path: Option<String>,
    exact: bool,
    mut rpcenv: &mut dyn RpcEnvironment,
) -> Result<Vec<AclListItem>, Error> {
    let auth_id = rpcenv.get_auth_id().unwrap().parse()?;

    let user_info = CachedUserInfo::new()?;

    let top_level_privs = user_info.lookup_privs(&auth_id, &["access", "acl"]);
    let auth_id_filter = if (top_level_privs & PRIV_SYS_AUDIT) == 0 {
        Some(auth_id)
    } else {
        None
    };

    let (mut tree, digest) = acl::config()?;

    let mut list: Vec<AclListItem> = Vec::new();
    if let Some(path) = &path {
        if let Some(node) = &tree.find_node(path) {
            extract_acl_node_data(&node, path, &mut list, exact, &auth_id_filter);
        }
    } else {
        extract_acl_node_data(&tree.root, "", &mut list, exact, &auth_id_filter);
    }

    rpcenv["digest"] = proxmox::tools::digest_to_hex(&digest).into();

    Ok(list)
}

#[api(
    protected: true,
    input: {
        properties: {
	    path: {
                schema: ACL_PATH_SCHEMA,
            },
	    role: {
                type: Role,
            },
            propagate: {
                optional: true,
                schema: ACL_PROPAGATE_SCHEMA,
            },
            "auth-id": {
                optional: true,
                type: Authid,
            },
            group: {
                optional: true,
                schema: PROXMOX_GROUP_ID_SCHEMA,
            },
            delete: {
                optional: true,
                description: "Remove permissions (instead of adding it).",
                type: bool,
            },
            digest: {
                optional: true,
                schema: PROXMOX_CONFIG_DIGEST_SCHEMA,
            },
       },
    },
    access: {
        permission: &Permission::Anybody,
        description: "Requires Permissions.Modify on '/access/acl', limited to updating ACLs of the user's API tokens otherwise."
    },
)]
/// Update Access Control List (ACLs).
#[allow(clippy::too_many_arguments)]
pub fn update_acl(
    path: String,
    role: String,
    propagate: Option<bool>,
    auth_id: Option<Authid>,
    group: Option<String>,
    delete: Option<bool>,
    digest: Option<String>,
    rpcenv: &mut dyn RpcEnvironment,
) -> Result<(), Error> {
    let current_auth_id: Authid = rpcenv.get_auth_id().unwrap().parse()?;

    let user_info = CachedUserInfo::new()?;

    let top_level_privs = user_info.lookup_privs(&current_auth_id, &["access", "acl"]);
    if top_level_privs & PRIV_PERMISSIONS_MODIFY == 0 {
        if group.is_some() {
            bail!("Unprivileged users are not allowed to create group ACL item.");
        }

        match &auth_id {
            Some(auth_id) => {
                if current_auth_id.is_token() {
                    bail!("Unprivileged API tokens can't set ACL items.");
                } else if !auth_id.is_token() {
                    bail!("Unprivileged users can only set ACL items for API tokens.");
                } else if auth_id.user() != current_auth_id.user() {
                    bail!("Unprivileged users can only set ACL items for their own API tokens.");
                }
            },
            None => { bail!("Unprivileged user needs to provide auth_id to update ACL item."); },
        };
    }

    let _lock = open_file_locked(acl::ACL_CFG_LOCKFILE, std::time::Duration::new(10, 0), true)?;

    let (mut tree, expected_digest) = acl::config()?;

    if let Some(ref digest) = digest {
        let digest = proxmox::tools::hex_to_digest(digest)?;
        crate::tools::detect_modified_configuration_file(&digest, &expected_digest)?;
    }

    let propagate = propagate.unwrap_or(true);

    let delete = delete.unwrap_or(false);

    if let Some(ref _group) = group {
        bail!("parameter 'group' - groups are currently not supported.");
    } else if let Some(ref auth_id) = auth_id {
        if !delete { // Note: we allow to delete non-existent users
            let user_cfg = crate::config::user::cached_config()?;
            if user_cfg.sections.get(&auth_id.to_string()).is_none() {
                bail!(format!("no such {}.",
                              if auth_id.is_token() { "API token" } else { "user" }));
            }
        }
    } else {
        bail!("missing 'userid' or 'group' parameter.");
    }

    if !delete { // Note: we allow to delete entries with invalid path
        acl::check_acl_path(&path)?;
    }

    if let Some(auth_id) = auth_id {
        if delete {
            tree.delete_user_role(&path, &auth_id, &role);
        } else {
            tree.insert_user_role(&path, &auth_id, &role, propagate);
        }
    } else if let Some(group) = group {
        if delete {
            tree.delete_group_role(&path, &group, &role);
        } else {
            tree.insert_group_role(&path, &group, &role, propagate);
        }
    }

    acl::save_config(&tree)?;

    Ok(())
}

pub const ROUTER: Router = Router::new()
    .get(&API_METHOD_READ_ACL)
    .put(&API_METHOD_UPDATE_ACL);

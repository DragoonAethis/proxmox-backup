use std::collections::HashMap;

use anyhow::{format_err, Error};
use serde_json::{json, Value};

use proxmox::api::{api, cli::*, RpcEnvironment};

use proxmox_backup::tools;
use proxmox_backup::config;
use proxmox_backup::api2::{self, types::* };
use proxmox_backup::client::*;
use proxmox_backup::tools::ticket::*;
use proxmox_backup::auth_helpers::*;

mod proxmox_backup_manager;
use proxmox_backup_manager::*;

async fn view_task_result(
    client: HttpClient,
    result: Value,
    output_format: &str,
) -> Result<(), Error> {
    let data = &result["data"];
    if output_format == "text" {
        if let Some(upid) = data.as_str() {
            display_task_log(client, upid, true).await?;
        }
    } else {
        format_and_print_result(&data, &output_format);
    }

    Ok(())
}

// Note: local workers should print logs to stdout, so there is no need
// to fetch/display logs. We just wait for the worker to finish.
pub async fn wait_for_local_worker(upid_str: &str) -> Result<(), Error> {

    let upid: proxmox_backup::server::UPID = upid_str.parse()?;

    let sleep_duration = core::time::Duration::new(0, 100_000_000);

    loop {
        if proxmox_backup::server::worker_is_active_local(&upid) {
            tokio::time::delay_for(sleep_duration).await;
        } else {
            break;
        }
    }
    Ok(())
}

fn connect() -> Result<HttpClient, Error> {

    let uid = nix::unistd::Uid::current();

    let mut options = HttpClientOptions::new()
        .prefix(Some("proxmox-backup".to_string()))
        .verify_cert(false); // not required for connection to localhost

    let client = if uid.is_root()  {
        let ticket = assemble_rsa_ticket(private_auth_key(), "PBS", Some("root@pam"), None)?;
        options = options.password(Some(ticket));
        HttpClient::new("localhost", "root@pam", options)?
    } else {
        options = options.ticket_cache(true).interactive(true);
        HttpClient::new("localhost", "root@pam", options)?
    };

    Ok(client)
}

#[api(
   input: {
        properties: {
            store: {
                schema: DATASTORE_SCHEMA,
            },
            "output-format": {
                schema: OUTPUT_FORMAT,
                optional: true,
            },
        }
   }
)]
/// Start garbage collection for a specific datastore.
async fn start_garbage_collection(param: Value) -> Result<Value, Error> {

    let output_format = get_output_format(&param);

    let store = tools::required_string_param(&param, "store")?;

    let mut client = connect()?;

    let path = format!("api2/json/admin/datastore/{}/gc", store);

    let result = client.post(&path, None).await?;

    view_task_result(client, result, &output_format).await?;

    Ok(Value::Null)
}

#[api(
   input: {
        properties: {
            store: {
                schema: DATASTORE_SCHEMA,
            },
            "output-format": {
                schema: OUTPUT_FORMAT,
                optional: true,
            },
        }
   }
)]
/// Show garbage collection status for a specific datastore.
async fn garbage_collection_status(param: Value) -> Result<Value, Error> {

    let output_format = get_output_format(&param);

    let store = tools::required_string_param(&param, "store")?;

    let client = connect()?;

    let path = format!("api2/json/admin/datastore/{}/gc", store);

    let mut result = client.get(&path, None).await?;
    let mut data = result["data"].take();
    let schema = api2::admin::datastore::API_RETURN_SCHEMA_GARBAGE_COLLECTION_STATUS;

    let options = default_table_format_options();

    format_and_print_result_full(&mut data, schema, &output_format, &options);

    Ok(Value::Null)
}

fn garbage_collection_commands() -> CommandLineInterface {

    let cmd_def = CliCommandMap::new()
        .insert("status",
                CliCommand::new(&API_METHOD_GARBAGE_COLLECTION_STATUS)
                .arg_param(&["store"])
                .completion_cb("store", config::datastore::complete_datastore_name)
        )
        .insert("start",
                CliCommand::new(&API_METHOD_START_GARBAGE_COLLECTION)
                .arg_param(&["store"])
                .completion_cb("store", config::datastore::complete_datastore_name)
        );

    cmd_def.into()
}

#[api(
    input: {
        properties: {
            limit: {
                description: "The maximal number of tasks to list.",
                type: Integer,
                optional: true,
                minimum: 1,
                maximum: 1000,
                default: 50,
            },
            "output-format": {
                schema: OUTPUT_FORMAT,
                optional: true,
            },
            all: {
                type: Boolean,
                description: "Also list stopped tasks.",
                optional: true,
            }
        }
    }
)]
/// List running server tasks.
async fn task_list(param: Value) -> Result<Value, Error> {

    let output_format = get_output_format(&param);

    let client = connect()?;

    let limit = param["limit"].as_u64().unwrap_or(50) as usize;
    let running = !param["all"].as_bool().unwrap_or(false);
    let args = json!({
        "running": running,
        "start": 0,
        "limit": limit,
    });
    let mut result = client.get("api2/json/nodes/localhost/tasks", Some(args)).await?;

    let mut data = result["data"].take();
    let schema = api2::node::tasks::API_RETURN_SCHEMA_LIST_TASKS;

    let options = default_table_format_options()
        .column(ColumnConfig::new("starttime").right_align(false).renderer(tools::format::render_epoch))
        .column(ColumnConfig::new("endtime").right_align(false).renderer(tools::format::render_epoch))
        .column(ColumnConfig::new("upid"))
        .column(ColumnConfig::new("status").renderer(tools::format::render_task_status));

    format_and_print_result_full(&mut data, schema, &output_format, &options);

    Ok(Value::Null)
}

#[api(
    input: {
        properties: {
            upid: {
                schema: UPID_SCHEMA,
            },
        }
    }
)]
/// Display the task log.
async fn task_log(param: Value) -> Result<Value, Error> {

    let upid = tools::required_string_param(&param, "upid")?;

    let client = connect()?;

    display_task_log(client, upid, true).await?;

    Ok(Value::Null)
}

#[api(
    input: {
        properties: {
            upid: {
                schema: UPID_SCHEMA,
            },
        }
    }
)]
/// Try to stop a specific task.
async fn task_stop(param: Value) -> Result<Value, Error> {

    let upid_str = tools::required_string_param(&param, "upid")?;

    let mut client = connect()?;

    let path = format!("api2/json/nodes/localhost/tasks/{}", upid_str);
    let _ = client.delete(&path, None).await?;

    Ok(Value::Null)
}

fn task_mgmt_cli() -> CommandLineInterface {

    let task_log_cmd_def = CliCommand::new(&API_METHOD_TASK_LOG)
        .arg_param(&["upid"]);

    let task_stop_cmd_def = CliCommand::new(&API_METHOD_TASK_STOP)
        .arg_param(&["upid"]);

    let cmd_def = CliCommandMap::new()
        .insert("list", CliCommand::new(&API_METHOD_TASK_LIST))
        .insert("log", task_log_cmd_def)
        .insert("stop", task_stop_cmd_def);

    cmd_def.into()
}

// fixme: avoid API redefinition
#[api(
   input: {
        properties: {
            "local-store": {
                schema: DATASTORE_SCHEMA,
            },
            remote: {
                schema: REMOTE_ID_SCHEMA,
            },
            "remote-store": {
                schema: DATASTORE_SCHEMA,
            },
            "remove-vanished": {
                schema: REMOVE_VANISHED_BACKUPS_SCHEMA,
                optional: true,
            },
            "output-format": {
                schema: OUTPUT_FORMAT,
                optional: true,
            },
        }
   }
)]
/// Sync datastore from another repository
async fn pull_datastore(
    remote: String,
    remote_store: String,
    local_store: String,
    remove_vanished: Option<bool>,
    param: Value,
) -> Result<Value, Error> {

    let output_format = get_output_format(&param);

    let mut client = connect()?;

    let mut args = json!({
        "store": local_store,
        "remote": remote,
        "remote-store": remote_store,
    });

    if let Some(remove_vanished) = remove_vanished {
        args["remove-vanished"] = Value::from(remove_vanished);
    }

    let result = client.post("api2/json/pull", Some(args)).await?;

    view_task_result(client, result, &output_format).await?;

    Ok(Value::Null)
}

fn main() {

    let cmd_def = CliCommandMap::new()
        .insert("acl", acl_commands())
        .insert("datastore", datastore_commands())
        .insert("disk", disk_commands())
        .insert("dns", dns_commands())
        .insert("network", network_commands())
        .insert("user", user_commands())
        .insert("remote", remote_commands())
        .insert("garbage-collection", garbage_collection_commands())
        .insert("cert", cert_mgmt_cli())
        .insert("sync-job", sync_job_commands())
        .insert("task", task_mgmt_cli())
        .insert(
            "pull",
            CliCommand::new(&API_METHOD_PULL_DATASTORE)
                .arg_param(&["remote", "remote-store", "local-store"])
                .completion_cb("local-store", config::datastore::complete_datastore_name)
                .completion_cb("remote", config::remote::complete_remote_name)
                .completion_cb("remote-store", complete_remote_datastore_name)
        );

    let mut rpcenv = CliEnvironment::new();
    rpcenv.set_user(Some(String::from("root@pam")));

   proxmox_backup::tools::runtime::main(run_async_cli_command(cmd_def, rpcenv));
}

// shell completion helper
pub fn complete_remote_datastore_name(_arg: &str, param: &HashMap<String, String>) -> Vec<String> {

    let mut list = Vec::new();

    let _ = proxmox::try_block!({
        let remote = param.get("remote").ok_or_else(|| format_err!("no remote"))?;
        let (remote_config, _digest) = config::remote::config()?;

        let remote: config::remote::Remote = remote_config.lookup("remote", &remote)?;

        let options = HttpClientOptions::new()
            .password(Some(remote.password.clone()))
            .fingerprint(remote.fingerprint.clone());

        let client = HttpClient::new(
            &remote.host,
            &remote.userid,
            options,
        )?;

        let result = crate::tools::runtime::block_on(client.get("api2/json/admin/datastore", None))?;

        if let Some(data) = result["data"].as_array() {
            for item in data {
                if let Some(store) = item["store"].as_str() {
                    list.push(store.to_owned());
                }
            }
        }

        Ok(())
    }).map_err(|_err: Error| { /* ignore */ });

    list
}

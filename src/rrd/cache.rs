use std::time::{SystemTime, UNIX_EPOCH};
use std::path::PathBuf;
use std::collections::HashMap;
use std::sync::{RwLock};

use anyhow::{format_err, Error};
use lazy_static::lazy_static;
use serde_json::{json, Value};

use proxmox::tools::fs::{create_path, CreateOptions};

use crate::api2::types::{RRDMode, RRDTimeFrameResolution};

use super::*;

const PBS_RRD_BASEDIR: &str = "/var/lib/proxmox-backup/rrdb";

lazy_static!{
    static ref RRD_CACHE: RwLock<HashMap<String, RRD>> = {
        RwLock::new(HashMap::new())
    };
}

/// Create rrdd stat dir with correct permission
pub fn create_rrdb_dir() -> Result<(), Error> {

    let backup_user = crate::backup::backup_user()?;
    let opts = CreateOptions::new()
        .owner(backup_user.uid)
        .group(backup_user.gid);

    create_path(PBS_RRD_BASEDIR, None, Some(opts))
        .map_err(|err: Error| format_err!("unable to create rrdb stat dir - {}", err))?;

    Ok(())
}

fn now() -> Result<f64, Error> {
    let time = SystemTime::now().duration_since(UNIX_EPOCH)?;
    Ok(time.as_secs_f64())
}

pub fn update_value(rel_path: &str, value: f64, dst: DST, save: bool) -> Result<(), Error> {

    let mut path = PathBuf::from(PBS_RRD_BASEDIR);
    path.push(rel_path);

    std::fs::create_dir_all(path.parent().unwrap())?;

    let mut map = RRD_CACHE.write().unwrap();
    let now = now()?;

    if let Some(rrd) = map.get_mut(rel_path) {
        rrd.update(now, value);
        if save { rrd.save(&path)?; }
    } else {
        let mut rrd = match RRD::load(&path) {
            Ok(rrd) => rrd,
            Err(err) => {
                if err.kind() != std::io::ErrorKind::NotFound {
                    eprintln!("overwriting RRD file {:?}, because of load error: {}", path, err);
                }
                RRD::new(dst)
            },
        };
        rrd.update(now, value);
        if save { rrd.save(&path)?; }
        map.insert(rel_path.into(), rrd);
    }

    Ok(())
}

pub fn extract_data(
    base: &str,
    items: &[&str],
    timeframe: RRDTimeFrameResolution,
    mode: RRDMode,
) -> Result<Value, Error> {

    let now = now()?;

    let map = RRD_CACHE.read().unwrap();

    let mut result = Vec::new();

    for name in items.iter() {
        let rrd = match map.get(&format!("{}/{}", base, name)) {
            Some(rrd) => rrd,
            None => continue,
        };
        let (start, reso, list) = rrd.extract_data(now, timeframe, mode);
        let mut t = start;
        for index in 0..RRD_DATA_ENTRIES {
            if result.len() <= index {
                if let Some(value) = list[index] {
                    result.push(json!({ "time": t, *name: value }));
                } else {
                    result.push(json!({ "time": t }));
                }
            } else {
                if let Some(value) = list[index] {
                    result[index][name] = value.into();
                }
            }
            t += reso;
        }
    }

    Ok(result.into())
}

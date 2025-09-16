// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use aptos_crypto::HashValue;
use aptos_db::AptosDB;
use aptos_storage_interface::DbReaderWriter;
use aptos_config::config::{StorageDirPaths, PrunerConfig, RocksdbConfigs};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    pub version: u64,
    pub block_height: u64,
    pub root_hash: HashValue,
    pub timestamp_usecs: u64,
    pub chain_id: u8,
    pub epoch: u64,
    pub created_at: u64,
}

pub struct SnapshotManager;

impl SnapshotManager {
    pub fn create_snapshot<P: AsRef<Path>>(data_dir: P, snapshot_path: P) -> Result<()> {
        let db_path = data_dir.as_ref().join("db");
        let cp_path = snapshot_path.as_ref();
        std::fs::create_dir_all(cp_path)?;
        AptosDB::create_checkpoint(db_path, cp_path, false)?;
        Ok(())
    }

    pub fn inspect_snapshot<P: AsRef<Path>>(snapshot_path: P) -> Result<SnapshotMetadata> {
        let storage_dir_paths = StorageDirPaths::from_path(snapshot_path.as_ref());
        let pruner_config = PrunerConfig::default();
        let rocksdb_config = RocksdbConfigs::default();
        let dbrw = DbReaderWriter::new(AptosDB::open(
            storage_dir_paths,
            true,
            pruner_config,
            rocksdb_config,
            false,
            0,
            0,
            None,
        )?);

        let li = dbrw.reader.get_latest_ledger_info()?;
        let info = li.ledger_info();
        let version = info.version();
        let block_height = info.round();
        let root_hash = info.commit_info().executed_state_id();
        let timestamp_usecs = info.timestamp_usecs();
        let epoch = info.epoch();
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();

        Ok(SnapshotMetadata {
            version,
            block_height,
            root_hash,
            timestamp_usecs,
            chain_id: 0,
            epoch,
            created_at,
        })
    }

    pub fn validate_snapshot<P: AsRef<Path>>(snapshot_path: P) -> Result<bool> {
        if !snapshot_path.as_ref().exists() {
            return Ok(false);
        }
        match Self::inspect_snapshot(snapshot_path) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    pub fn list_snapshots<P: AsRef<Path>>(snapshot_dir: P) -> Result<Vec<String>> {
        let mut snapshots = Vec::new();
        if snapshot_dir.as_ref().is_dir() {
            for entry in std::fs::read_dir(snapshot_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() && path.extension().is_none() {
                    if let Some(file_name) = path.file_name() {
                        if let Some(name) = file_name.to_str() {
                            snapshots.push(name.to_string());
                        }
                    }
                }
            }
        }
        Ok(snapshots)
    }

    pub fn get_snapshot_metadata<P: AsRef<Path>>(snapshot_path: P) -> Result<SnapshotMetadata> {
        Self::inspect_snapshot(snapshot_path)
    }

    pub fn restore_snapshot_to_db<P: AsRef<Path>>(snapshot_path: P, data_dir: P) -> Result<SnapshotMetadata> {
        let db_dir = data_dir.as_ref().join("db");
        if db_dir.exists() {
            std::fs::remove_dir_all(&db_dir)?;
        }
        copy_dir_recursively(snapshot_path.as_ref(), &db_dir)?;
        Self::inspect_snapshot(&db_dir)
    }
}

use std::fs::{self, File};
use std::io::Write;

fn copy_dir_recursively(from: &Path, to: &Path) -> Result<()> {
    if !to.exists() {
        fs::create_dir_all(to)?;
    }
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = to.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursively(&src_path, &dst_path)?;
        } else if src_path.is_file() {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

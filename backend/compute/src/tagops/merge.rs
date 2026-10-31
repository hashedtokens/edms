use crate::eid::allocator::EidAllocator;
use crate::tagops::{
    collection_tags::merge_collection_tags_op,
    db::{
        bulk_fetch_endpoint_details, bulk_fetch_endpoint_tags,
        endpoints_with_tags_in_collection_in, get_storage_root, log_entry, open_core,
        read_collection_member_eids_in, write_collection_member_eids_in, DynResult,
    },
    types::{ActivityEntry, CollectionTagFilter},
};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

pub const DEFAULT_MERGE_BATCH_SIZE: usize = 500;

/// Merge coordinator: 5-Stage bulk-read, in-memory reconcile, bulk-write, and physical QP merge.
/// The storage root is taken to be the folder the database file sits in; use
/// `merge_by_tags_in` when the storage is somewhere else (as it is in Docker).
pub fn merge_by_tags_coordinator(
    db_path: &str,
    target_collection: &str,
    sources: &[CollectionTagFilter],
    log: &mut Vec<ActivityEntry>,
) -> DynResult<()> {
    merge_by_tags_in(db_path, &get_storage_root(db_path), target_collection, sources, log)
}

/// `merge_by_tags_coordinator` with the storage root given: where the
/// collection files (`storage/collections`) and the QP files
/// (`storage/globalEQPData`) are.
pub fn merge_by_tags_in(
    db_path: &str,
    storage_root: &Path,
    target_collection: &str,
    sources: &[CollectionTagFilter],
    log: &mut Vec<ActivityEntry>,
) -> DynResult<()> {
    if sources.is_empty() {
        return Err("No sources provided".into());
    }

    for src in sources {
        if src.collection_id == target_collection {
            return Err(format!(
                "Self-merge is not allowed: source and target are both '{}'",
                target_collection
            )
            .into());
        }
    }

    let (core, queries) = open_core(db_path)?;
    let allocator = EidAllocator::new(db_path);
    let _ = allocator.initialize();

    log_entry(log, format!("Starting merge into '{}'", target_collection));

    // Stage 2 (Initial): Bulk read target context
    let mut target_members: HashSet<String> =
        read_collection_member_eids_in(&core, storage_root, target_collection)?
            .into_iter()
            .collect();

    let mut target_identity_map: HashMap<(String, String), String> = HashMap::new();
    if !target_members.is_empty() {
        let member_vec: Vec<String> = target_members.iter().cloned().collect();
        let target_details = bulk_fetch_endpoint_details(&core, &member_vec)?;
        for (eid, (ep_str, method, _)) in target_details {
            target_identity_map.insert((ep_str, method), eid);
        }
    }

    for src in sources {
        let is_whole_folder = src.tags.is_empty();

        log_entry(
            log,
            format!(
                "  Processing '{}' ({} tag filter(s))",
                src.collection_id,
                src.tags.len()
            ),
        );

        // Stage 1: Bulk Read Source Context
        let source_eids = endpoints_with_tags_in_collection_in(
            &core,
            storage_root,
            &src.collection_id,
            &src.tags,
        )?;

        if source_eids.is_empty() {
            log_entry(
                log,
                format!("  Completed '{}': matched 0, net-new 0", src.collection_id),
            );
            continue;
        }

        let details_map = bulk_fetch_endpoint_details(&core, &source_eids)?;
        let tags_map = bulk_fetch_endpoint_tags(&core, &source_eids)?;

        // In-memory grouping: Group source endpoints by logical identity (endpoint_str, method)
        struct GroupedEndpoint {
            endpoint_str: String,
            method: String,
            annotation: Option<String>,
            source_eids: Vec<String>,
            all_tags: HashSet<String>,
        }

        let mut logical_order: Vec<(String, String)> = Vec::new();
        let mut grouped_map: HashMap<(String, String), GroupedEndpoint> = HashMap::new();

        for eid in &source_eids {
            if let Some((ep_str, method, annotation)) = details_map.get(eid) {
                let key = (ep_str.clone(), method.clone());
                let entry = grouped_map.entry(key.clone()).or_insert_with(|| {
                    logical_order.push(key);
                    GroupedEndpoint {
                        endpoint_str: ep_str.clone(),
                        method: method.clone(),
                        annotation: annotation.clone(),
                        source_eids: Vec::new(),
                        all_tags: HashSet::new(),
                    }
                });

                entry.source_eids.push(eid.clone());
                if let Some(tags) = tags_map.get(eid) {
                    for tag in tags {
                        entry.all_tags.insert(tag.clone());
                    }
                }
            }
        }

        let mut total_matched = 0;
        let mut total_net_new = 0;

        // Stage 3 & 4: In-Memory Reconciliation & Bulk Database Writes in Bounded Batches
        for chunk_keys in logical_order.chunks(DEFAULT_MERGE_BATCH_SIZE) {
            let mut batch_matched = 0;
            let mut batch_net_new = 0;

            // Count how many endpoints in this chunk need a fresh EID before
            // entering the reconciliation loop, then allocate them all in a
            // single transaction (one BEGIN IMMEDIATE instead of one per EID).
            let net_new_count = chunk_keys
                .iter()
                .filter(|key| !target_identity_map.contains_key(*key))
                .count();
            let mut pre_allocated: VecDeque<String> = if net_new_count > 0 {
                allocator.allocate_batch(net_new_count)?.into()
            } else {
                VecDeque::new()
            };

            let mut net_new_endpoints: Vec<(String, String, String, Option<String>)> = Vec::new();
            let mut tags_to_insert: Vec<(String, String)> = Vec::new();
            let mut new_member_eids: Vec<String> = Vec::new();
            let mut qp_copy_tasks: Vec<(String, String)> = Vec::new();

            for key in chunk_keys {
                let group = grouped_map.get(key).unwrap();
                if let Some(target_eid) = target_identity_map.get(key) {
                    // Case A: MATCHED — endpoint already exists in target
                    batch_matched += 1;
                    if target_members.insert(target_eid.clone()) {
                        new_member_eids.push(target_eid.clone());
                    }
                    for tag in &group.all_tags {
                        tags_to_insert.push((target_eid.clone(), tag.clone()));
                    }
                    for src_eid in &group.source_eids {
                        qp_copy_tasks.push((src_eid.clone(), target_eid.clone()));
                    }
                } else {
                    // Case B: NET-NEW — use a pre-allocated EID from the batch
                    batch_net_new += 1;
                    let new_eid = pre_allocated
                        .pop_front()
                        .ok_or("allocator batch underflow — net-new count mismatch")?;
                    target_identity_map.insert(key.clone(), new_eid.clone());
                    target_members.insert(new_eid.clone());
                    new_member_eids.push(new_eid.clone());

                    net_new_endpoints.push((
                        new_eid.clone(),
                        group.endpoint_str.clone(),
                        group.method.clone(),
                        group.annotation.clone(),
                    ));

                    for tag in &group.all_tags {
                        tags_to_insert.push((new_eid.clone(), tag.clone()));
                    }
                    for src_eid in &group.source_eids {
                        qp_copy_tasks.push((src_eid.clone(), new_eid.clone()));
                    }
                }
            }

            // Execute batched database writes in central edms.db
            core.base.execute("BEGIN IMMEDIATE", &[])?;
            let write_res = (|| -> DynResult<()> {
                // 1. Insert net-new endpoints
                for (eid, ep_str, method, annotation) in &net_new_endpoints {
                    core.proc(
                        "INSERT INTO endpoints (endpoint_id, endpoint_str, method, annotation) VALUES (?, ?, ?, ?)",
                        &[eid, ep_str, method, annotation],
                    )?;
                }

                // 2. Insert tags
                for (eid, tag) in &tags_to_insert {
                    core.proc(
                        "INSERT OR IGNORE INTO tags (endpoint_id, tag) VALUES (?, ?)",
                        &[eid, tag],
                    )?;
                }

                Ok(())
            })();

            match write_res {
                Ok(()) => {
                    core.base.execute("COMMIT", &[])?;
                }
                Err(e) => {
                    let _ = core.base.execute("ROLLBACK", &[]);
                    return Err(e);
                }
            }

            // 3. Write target collection membership (both collection SQLite file and bookmarks table)
            if !new_member_eids.is_empty() {
                write_collection_member_eids_in(&core, storage_root, target_collection, &new_member_eids)?;
            }

            // Stage 5: Physical Filesystem Merge & QP Copy
            // Errors are non-fatal (DB is already committed) but logged so
            // that operators can identify and re-trigger partial copies.
            for (src_eid, dest_eid) in &qp_copy_tasks {
                if let Err(e) = merge_physical_qp_files(storage_root, src_eid, dest_eid) {
                    log_entry(
                        log,
                        format!("    ⚠ QP copy {src_eid} → {dest_eid} failed: {e}"),
                    );
                }
            }

            total_matched += batch_matched;
            total_net_new += batch_net_new;

            log_entry(
                log,
                format!(
                    "    → Batch: processed {}, matched {}, net-new {}",
                    chunk_keys.len(),
                    batch_matched,
                    batch_net_new
                ),
            );
        }

        // C(T): runs ONCE per source, AFTER all its endpoint batches,
        // ONLY for whole-folder merges, in its own short transaction.
        if is_whole_folder {
            core.base.execute("BEGIN IMMEDIATE", &[])?;
            match merge_collection_tags_op(&core, &queries, &src.collection_id, target_collection)
            {
                Ok(tags_added) => {
                    let _ = core.base.execute("COMMIT", &[]);
                    log_entry(log, format!("    → Collection tags merged ({} new)", tags_added));
                }
                Err(e) => {
                    let _ = core.base.execute("ROLLBACK", &[]);
                    return Err(e);
                }
            }
        }

        log_entry(
            log,
            format!(
                "  Completed '{}': matched {}, net-new {}",
                src.collection_id, total_matched, total_net_new
            ),
        );
    }

    log_entry(log, format!("Merge into '{}' complete.", target_collection));
    Ok(())
}

/// Copies and re-indexes physical QP files (request, response, headers) from `source_eid` into `target_eid`.
pub fn merge_physical_qp_files(
    storage_root: &Path,
    source_eid: &str,
    target_eid: &str,
) -> std::io::Result<()> {
    let global_eqp_dir = storage_root.join("storage").join("globalEQPData");
    let src_dir = global_eqp_dir.join(source_eid);
    if !src_dir.exists() {
        return Ok(());
    }

    let target_dir = global_eqp_dir.join(target_eid);
    std::fs::create_dir_all(&target_dir)?;

    if source_eid == target_eid {
        return Ok(());
    }

    // Determine the highest existing request index in target_dir
    let mut highest_request = 0;
    if let Ok(entries) = std::fs::read_dir(&target_dir) {
        for entry in entries.flatten() {
            let filename = entry.file_name().to_string_lossy().to_string();
            if filename.starts_with(target_eid) && filename.contains("-request-") {
                if let Some(num_str) = filename.strip_prefix(&format!("{target_eid}-request-")) {
                    if let Some(num_clean) = num_str.strip_suffix(".json") {
                        if let Ok(num) = num_clean.parse::<u32>() {
                            if num > highest_request {
                                highest_request = num;
                            }
                        }
                    }
                }
            }
        }
    }

    // Collect source files grouped by request number
    let mut src_requests: std::collections::BTreeMap<u32, Vec<(String, std::path::PathBuf)>> =
        std::collections::BTreeMap::new();

    if let Ok(entries) = std::fs::read_dir(&src_dir) {
        for entry in entries.flatten() {
            let fname = entry.file_name().to_string_lossy().to_string();
            if !fname.starts_with(source_eid) {
                continue;
            }
            let rest = &fname[source_eid.len()..];
            for kind in &["-request-", "-response-", "-headers-"] {
                if rest.starts_with(kind) {
                    if let Some(num_part) = rest.strip_prefix(kind) {
                        if let Some(num_clean) = num_part.strip_suffix(".json") {
                            if let Ok(num) = num_clean.parse::<u32>() {
                                src_requests.entry(num).or_default().push((
                                    kind.trim_matches('-').to_string(),
                                    entry.path(),
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    // Copy each request tuple into target_dir with new sequential number
    for (_src_num, files) in src_requests {
        highest_request += 1;
        for (kind, src_path) in files {
            let dest_name = format!("{target_eid}-{kind}-{highest_request}.json");
            let dest_path = target_dir.join(dest_name);
            let _ = std::fs::copy(&src_path, &dest_path);
        }
    }

    Ok(())
}

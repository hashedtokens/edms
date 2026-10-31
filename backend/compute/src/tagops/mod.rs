// tagops — background tag operations for the compute IPC child.
// SQL keys reference webserver/libs/edms/src/queries.yaml.
// Per-row CRUD lives in webserver/libs/edms/src/ops/tag_ops.rs.

pub mod db;
pub mod collection_merge;
pub mod collection_tags;
pub mod merge;
mod ops;
pub mod types;

pub use ops::*;
pub use types::*;

#[cfg(test)]
mod tests {
    use super::*;
    use db::log_entry;
    use edms::query_loader::QueryMap;

    fn make_tags(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_log_entry_has_timestamp() {
        let mut log = Vec::new();
        log_entry(&mut log, "test message".to_string());
        assert_eq!(log.len(), 1);
        assert!(log[0].timestamp.contains('T'));
        assert_eq!(log[0].message, "test message");
    }

    // Verify every query key used in tagops exists in queries.yaml.

    #[test]
    fn test_querymap_has_t1() { assert!(QueryMap::load().get_tag_query("T1").is_some()); }
    #[test]
    fn test_querymap_has_t4() { assert!(QueryMap::load().get_tag_query("T4").is_some()); }
    #[test]
    fn test_querymap_has_t8() { assert!(QueryMap::load().get_tag_query("T8").is_some()); }
    #[test]
    fn test_querymap_has_b3() { assert!(QueryMap::load().get_bookmark_query("B3").is_some()); }
    #[test]
    fn test_querymap_has_b7() { assert!(QueryMap::load().get_bookmark_query("B7").is_some()); }
    #[test]
    fn test_querymap_has_b8() { assert!(QueryMap::load().get_bookmark_query("B8").is_some()); }
    #[test]
    fn test_querymap_has_b9() { assert!(QueryMap::load().get_bookmark_query("B9").is_some()); }

    #[test]
    fn test_log_shape_bulk_add() {
        let mut log: Vec<ActivityEntry> = Vec::new();
        log_entry(&mut log, "Bulk-add 2 tag(s) to 3 endpoint(s)".to_string());
        log_entry(&mut log, "Bulk-add complete.".to_string());
        assert_eq!(log.len(), 2);
        assert!(log[1].message.contains("complete"));
    }

    #[test]
    fn test_log_shape_bulk_remove() {
        let mut log: Vec<ActivityEntry> = Vec::new();
        log_entry(&mut log, "Bulk-remove 1 tag(s) from 2 endpoint(s)".to_string());
        log_entry(&mut log, "Bulk-remove complete.".to_string());
        assert_eq!(log.len(), 2);
        assert!(log[0].message.contains("Bulk-remove"));
    }

    #[test]
    fn test_log_shape_rename() {
        let mut log: Vec<ActivityEntry> = Vec::new();
        log_entry(&mut log, "Renaming tag 'old' → 'new'".to_string());
        log_entry(&mut log, "Renamed 'old' → 'new' on 5 row(s).".to_string());
        assert_eq!(log.len(), 2);
        assert!(log[1].message.contains("row(s)"));
    }

    #[test]
    fn test_log_shape_merge_no_sources() {
        let mut log: Vec<ActivityEntry> = Vec::new();
        log_entry(&mut log, "Starting merge into 'Empty'".to_string());
        log_entry(&mut log, "Merge into 'Empty' complete.".to_string());
        assert_eq!(log.len(), 2);
        assert!(log[0].message.contains("Starting merge"));
    }

    #[test]
    fn test_b8_in_clause_expansion() {
        let q = QueryMap::load();
        let b8 = q.get_bookmark_query("B8").unwrap();
        let tags = make_tags(&["T1", "T2", "T3"]);
        let ph = tags.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
        let expanded = b8.replacen("IN (?)", &format!("IN ({ph})"), 1);
        assert!(expanded.contains("IN (?, ?, ?)"));
        assert!(!expanded.contains("IN (?)"));
    }
}

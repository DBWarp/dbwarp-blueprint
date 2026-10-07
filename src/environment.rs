//! Privacy-safe source-environment helpers.
//!
//! These functions classify only values returned through the selected source
//! endpoint. They never inspect the workstation running `dbwarp-blueprint`.

use dbwarp_blueprint_core::SourceEnvironment;

pub fn database_source_environment(
    cpu_count: Option<u64>,
    cpu_basis: &str,
    memory_bytes: Option<u64>,
    memory_basis: &str,
    capacity_scope: &str,
    mut catalogs_read: Vec<String>,
    mut catalogs_unreadable: Vec<String>,
) -> SourceEnvironment {
    let cpu_count = cpu_count.filter(|value| *value > 0);
    let memory_bytes = memory_bytes.filter(|value| *value > 0);
    catalogs_read.sort();
    catalogs_read.dedup();
    catalogs_unreadable.sort();
    catalogs_unreadable.dedup();
    let cpu_capacity_band = cpu_count.map(cpu_band).unwrap_or("unknown");
    let memory_capacity_band = memory_bytes.map(memory_band).unwrap_or("unknown");
    let capacity_scope = if cpu_count.is_some() || memory_bytes.is_some() {
        capacity_scope
    } else {
        "unknown"
    };
    let capacity_visibility =
        if cpu_count.is_some() && memory_bytes.is_some() && catalogs_unreadable.is_empty() {
            "full"
        } else if cpu_count.is_some() || memory_bytes.is_some() {
            "partial"
        } else {
            "unknown"
        };
    SourceEnvironment {
        contract: dbwarp_blueprint_core::SOURCE_ENVIRONMENT_CONTRACT.to_string(),
        evidence_origin: "database-endpoint".to_string(),
        hosting_model: "unknown".to_string(),
        infrastructure_location: "unknown".to_string(),
        capacity_scope: capacity_scope.to_string(),
        capacity_visibility: capacity_visibility.to_string(),
        cpu_capacity_band: cpu_capacity_band.to_string(),
        cpu_capacity_basis: if cpu_count.is_some() {
            cpu_basis.to_string()
        } else {
            "unknown".to_string()
        },
        memory_capacity_band: memory_capacity_band.to_string(),
        memory_capacity_basis: if memory_bytes.is_some() {
            memory_basis.to_string()
        } else {
            "unknown".to_string()
        },
        collector_machine_excluded: true,
        catalogs_read,
        catalogs_unreadable,
        ..SourceEnvironment::default()
    }
}

fn cpu_band(count: u64) -> &'static str {
    match count {
        0 => "unknown",
        1 => "1",
        2 => "2",
        3..=4 => "3-4",
        5..=8 => "5-8",
        9..=16 => "9-16",
        17..=32 => "17-32",
        33..=64 => "33-64",
        65..=128 => "65-128",
        _ => "129-plus",
    }
}

fn memory_band(bytes: u64) -> &'static str {
    const GIB: u64 = 1024 * 1024 * 1024;
    match bytes {
        0 => "unknown",
        value if value < 2 * GIB => "under-2-gib",
        value if value < 4 * GIB => "2-4-gib",
        value if value < 8 * GIB => "4-8-gib",
        value if value < 16 * GIB => "8-16-gib",
        value if value < 32 * GIB => "16-32-gib",
        value if value < 64 * GIB => "32-64-gib",
        value if value < 128 * GIB => "64-128-gib",
        value if value < 256 * GIB => "128-256-gib",
        value if value < 512 * GIB => "256-512-gib",
        _ => "512-gib-plus",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_database_capacity_never_claims_the_collector_host() {
        let environment = database_source_environment(
            None,
            "unknown",
            Some(8 * 1024 * 1024 * 1024),
            "database-buffer-cache",
            "connected-instance",
            vec!["pg-capacity-settings".to_string()],
            Vec::new(),
        );
        assert_eq!(environment.capacity_visibility, "partial");
        assert_eq!(environment.cpu_capacity_band, "unknown");
        assert_eq!(environment.memory_capacity_band, "8-16-gib");
        assert!(environment.collector_machine_excluded);
    }

    #[test]
    fn denied_capacity_catalog_is_explicitly_unknown() {
        let environment = database_source_environment(
            None,
            "unknown",
            None,
            "unknown",
            "unknown",
            Vec::new(),
            vec!["sqlserver-os-sys-info".to_string()],
        );
        assert_eq!(environment.evidence_origin, "database-endpoint");
        assert_eq!(environment.capacity_visibility, "unknown");
        assert_eq!(environment.capacity_scope, "unknown");
        assert_eq!(environment.catalogs_unreadable.len(), 1);
        assert!(environment.collector_machine_excluded);
    }

    #[test]
    fn readable_catalog_without_usable_capacity_stays_unknown() {
        let environment = database_source_environment(
            None,
            "unknown",
            None,
            "unknown",
            "unknown",
            vec!["pg-capacity-settings".to_string()],
            Vec::new(),
        );
        assert_eq!(environment.capacity_visibility, "unknown");
        assert_eq!(environment.catalogs_read, ["pg-capacity-settings"]);
        assert!(environment.catalogs_unreadable.is_empty());
    }
}

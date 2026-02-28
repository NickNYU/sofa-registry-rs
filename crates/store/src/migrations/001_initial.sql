CREATE TABLE IF NOT EXISTS distribute_lock (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    data_center TEXT NOT NULL,
    lock_name TEXT NOT NULL,
    owner TEXT NOT NULL,
    duration INTEGER NOT NULL DEFAULT 30000,
    term INTEGER NOT NULL DEFAULT 0,
    term_duration INTEGER NOT NULL DEFAULT 0,
    gmt_create TEXT NOT NULL DEFAULT (datetime('now')),
    gmt_modified TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(data_center, lock_name)
);

CREATE TABLE IF NOT EXISTS app_revision (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    data_center TEXT NOT NULL,
    revision TEXT NOT NULL,
    app_name TEXT NOT NULL,
    base_params TEXT,
    service_params TEXT,
    deleted INTEGER NOT NULL DEFAULT 0,
    gmt_create TEXT NOT NULL DEFAULT (datetime('now')),
    gmt_modified TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(data_center, revision)
);

CREATE TABLE IF NOT EXISTS provide_data (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    data_center TEXT NOT NULL,
    data_key TEXT NOT NULL,
    data_value TEXT,
    version INTEGER NOT NULL DEFAULT 0,
    gmt_create TEXT NOT NULL DEFAULT (datetime('now')),
    gmt_modified TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(data_center, data_key)
);

CREATE TABLE IF NOT EXISTS interface_apps_index (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    data_center TEXT NOT NULL,
    app_name TEXT NOT NULL,
    interface_name TEXT NOT NULL,
    gmt_create TEXT NOT NULL DEFAULT (datetime('now')),
    gmt_modified TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(data_center, app_name, interface_name)
);

CREATE TABLE IF NOT EXISTS client_manager_address (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    data_center TEXT NOT NULL,
    address TEXT NOT NULL,
    operation TEXT NOT NULL DEFAULT 'CLIENT_OFF',
    gmt_create TEXT NOT NULL DEFAULT (datetime('now')),
    gmt_modified TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(data_center, address)
);

CREATE TABLE IF NOT EXISTS recover_config (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    property_table TEXT NOT NULL,
    property_key TEXT NOT NULL,
    property_value TEXT,
    gmt_create TEXT NOT NULL DEFAULT (datetime('now')),
    gmt_modified TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(property_table, property_key)
);

CREATE TABLE IF NOT EXISTS multi_cluster_sync_info (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    data_center TEXT NOT NULL,
    remote_data_center TEXT NOT NULL,
    remote_meta_address TEXT NOT NULL,
    enable_push INTEGER NOT NULL DEFAULT 0,
    enable_sync INTEGER NOT NULL DEFAULT 0,
    sync_data_info_ids TEXT,
    gmt_create TEXT NOT NULL DEFAULT (datetime('now')),
    gmt_modified TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(data_center, remote_data_center)
);

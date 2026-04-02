-- Migration 009: Metric history policy + narrower history writes.
--
-- Controls which metrics get written to metrics_history.
-- Default: only store metrics matching known patterns.
-- metrics_current is always complete (all scraped series).

CREATE TABLE metric_history_policy (
    pattern        TEXT PRIMARY KEY,  -- metric name or prefix pattern (e.g. 'node_load%')
    mode           TEXT NOT NULL DEFAULT 'full' CHECK (mode IN ('full', 'sample', 'drop')),
    sample_every   INTEGER,           -- for 'sample' mode: write every N generations
    enabled        INTEGER NOT NULL DEFAULT 1,
    notes          TEXT
);

-- Seed with substrate metrics worth trending
INSERT INTO metric_history_policy (pattern, mode, notes) VALUES
    ('node_load1', 'full', 'CPU load 1m'),
    ('node_load5', 'full', 'CPU load 5m'),
    ('node_load15', 'full', 'CPU load 15m'),
    ('node_memory_MemAvailable_bytes', 'full', 'Available memory'),
    ('node_memory_MemTotal_bytes', 'full', 'Total memory'),
    ('node_filesystem_avail_bytes', 'full', 'Filesystem free space'),
    ('node_filesystem_size_bytes', 'full', 'Filesystem total size'),
    ('node_disk_read_bytes_total', 'full', 'Disk read throughput'),
    ('node_disk_written_bytes_total', 'full', 'Disk write throughput'),
    ('node_network_receive_bytes_total', 'full', 'Network RX'),
    ('node_network_transmit_bytes_total', 'full', 'Network TX'),
    ('node_cpu_seconds_total', 'full', 'CPU time by mode'),
    ('node_scrape_collector_duration_seconds', 'sample', 'Exporter internals'),
    ('node_scrape_collector_success', 'full', 'Scrape health'),
    ('process_resident_memory_bytes', 'sample', 'Process RSS'),
    ('process_cpu_seconds_total', 'sample', 'Process CPU'),
    ('up', 'full', 'Target reachability');

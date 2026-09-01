-- =============================================================================
-- DBWarp Blueprint collector-account removal — MySQL 8.0 / 8.4 / 9.7
-- =============================================================================
-- Run as an account with CREATE USER authority after the approved capture.
-- EDIT each host pattern to match the corresponding grant script exactly.
-- DROP USER atomically removes the account and all of its grants.

DROP USER IF EXISTS 'dbwarp_blueprint_basic'@'collector-host';
DROP USER IF EXISTS 'dbwarp_blueprint_standard'@'collector-host';
DROP USER IF EXISTS 'dbwarp_blueprint_enhanced'@'collector-host';

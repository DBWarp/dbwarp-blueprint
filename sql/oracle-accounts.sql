-- Oracle collector principals shared by the Oracle preview grant and revoke files.
-- Edit this one file before running either side of the lifecycle. The Oracle
-- preview provides the Basic catalogue tier.
-- The privilege-free role is a lifecycle marker: choose a previously unused
-- local name and do not grant it to any other account. The grant script creates
-- and assigns it without changing the account's profile or password policy.
-- Recovery is deliberately not configured here. Pass NO to either script for
-- normal use. Grant scripts accept YES only for the exact interrupted marker
-- state described by the grant script; revoke scripts require NO. This keeps
-- recovery a one-run decision rather than a persistent shared-file setting.
DEFINE bp_basic_user = DBWARP_BLUEPRINT_BASIC
DEFINE bp_basic_marker_role = DBWARP_BLUEPRINT_BASIC_MARKER

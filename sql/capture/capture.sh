#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "Usage: $0 BPCOLLECTOR@service OWNER [OWNER ...]" >&2
  echo "   or: $0 BPCOLLECTOR@service --owners-file PATH" >&2
  echo "Owner names are ordinary Oracle schema names; encoding and family selection are automatic." >&2
}

startup_failure_message() {
  local ora_code=$1
  if [[ -z "$ora_code" ]]; then
    # `EXIT SQL.SQLCODE` is reduced to the low byte by Unix process status.
    # Map only unambiguous remedies; status 1 stays generic.
    case "$status" in
      7) ora_code=1017 ;;
      235) ora_code=1045 ;;
      249) ora_code=1031 ;;
      82) ora_code=942 ;;
      134) ora_code=12154 ;;
      126) ora_code=12162 ;;
      118) ora_code=12170 ;;
      9) ora_code=12535 ;;
      39) ora_code=12505 ;;
      30) ora_code=12514 ;;
      3) ora_code=12541 ;;
      255) ora_code=12545 ;;
      253) ora_code=12547 ;;
      240) ora_code=12560 ;;
      160) ora_code=28000 ;;
      159) ora_code=28001 ;;
      223) ora_code=20001 ;;
      220) ora_code=20004 ;;
      219) ora_code=20005 ;;
      218) ora_code=20006 ;;
    esac
  fi
  local ora_label=
  if [[ -n "$ora_code" ]]; then
    printf -v ora_label '%05d' "$ora_code"
  fi
  case "$ora_code" in
    1017) echo "DBP1426E Oracle rejected the collector credentials (ORA-01017). Verify the account name and password, then retry." ;;
    1045) echo "DBP1426E The collector cannot create a session (ORA-01045). Apply the minimum or Basic grant script, then retry." ;;
    1031|942) echo "DBP1426E Oracle denied or lacks a required admission catalogue (ORA-$ora_label). Reapply the matching minimum or Basic grant script in the selected PDB." ;;
    12154|12162) echo "DBP1426E SQL*Net could not resolve the requested service (ORA-$ora_label). Check the service alias and Oracle network configuration." ;;
    12170|12535) echo "DBP1426E The Oracle connection timed out (ORA-$ora_label). Check routing, firewall policy, listener reachability, and the service name." ;;
    12505|12514) echo "DBP1426E The listener does not know the requested service (ORA-$ora_label). Use the PDB service registered with the listener." ;;
    12541|12543|12545) echo "DBP1426E The Oracle listener or host is unreachable (ORA-$ora_label). Check the host, port, listener, routing, and firewall policy." ;;
    12547|12560) echo "DBP1426E The local SQL*Plus client or Oracle protocol stack failed (ORA-$ora_label). Check the client installation and Oracle environment." ;;
    28000) echo "DBP1426E The collector account is locked (ORA-28000). Ask the DBA to unlock the dedicated collector account." ;;
    28001) echo "DBP1426E The collector password is expired (ORA-28001). Reset it under your password policy, then retry." ;;
    20001) echo "DBP1426E The owner input was malformed (ORA-20001). Pass ordinary exact-case schema names, one per argument or owner-file line." ;;
    20004) echo "DBP1426E This Oracle server family is not supported by the selected capture pack (ORA-20004). Use the pack shipped with this DBWarp Blueprint build." ;;
    20005) echo "DBP1426E The session connected to CDB\$ROOT (ORA-20005). Connect to the intended pluggable-database service and retry." ;;
    20006) echo "DBP1426E A requested owner was not found (ORA-20006). Verify every schema name and its exact Oracle letter case, then retry." ;;
    *)
      if (( status == 0 )); then
        echo "DBP1426E SQL*Plus exited successfully without a capture header. Use the complete capture pack shipped with this DBWarp Blueprint build and retry from a private empty directory."
      else
        echo "DBP1426E SQL*Plus exited with status $status before producing a capture header. Check the client installation, login, selected PDB, collector grants, and service reachability."
      fi
      ;;
  esac
}

if (( $# < 2 )); then
  usage
  exit 2
fi

connect=$1
shift
connect_user=${connect%%@*}
if [[ "$connect" != *@* || -z "$connect_user" || "$connect_user" == *'/'* ||
      "$connect" == *$'\n'* || "$connect" == *$'\r'* ]] ||
   LC_ALL=C grep -q '[[:space:][:cntrl:]]' <<<"$connect"; then
  echo "DBP1426E The connection must be username@service without a password; SQL*Plus will prompt securely." >&2
  exit 2
fi
case "$connect_user" in
  [Ss][Yy][Ss]|[Ss][Yy][Ss][Tt][Ee][Mm])
    echo "DBP1426E Run the capture as the dedicated minimum- or Basic-grant collector account, not SYS or SYSTEM." >&2
    exit 2
    ;;
esac

owners=()
if [[ ${1-} == --owners-file ]]; then
  if (( $# != 2 )); then
    echo "DBP1426E --owners-file must be followed by exactly one readable file path." >&2
    exit 2
  fi
  if [[ ! -f "$2" || ! -r "$2" ]]; then
    echo "DBP1426E The owner file is not a readable regular file." >&2
    exit 2
  fi
  while IFS= read -r owner || [[ -n "$owner" ]]; do
    owner=${owner%$'\r'}
    owner=${owner#$'\xEF\xBB\xBF'}
    owners[${#owners[@]}]=$owner
  done <"$2"
else
  owners=("$@")
fi
if (( ${#owners[@]} == 0 )); then
  echo "DBP1426E Provide at least one owner name, directly or through --owners-file." >&2
  exit 2
fi

encoded=()
for owner in "${owners[@]}"; do
  if [[ -z "$owner" || "$owner" == *$'\n'* || "$owner" == *$'\r'* ]] ||
     LC_ALL=C grep -q '[[:cntrl:]]' <<<"$owner"; then
    echo "DBP1426E Owner names must be non-empty and contain no control characters." >&2
    exit 2
  fi
  owner_hex=$(printf '%s' "$owner" | LC_ALL=C od -An -v -tx1 | tr -d ' \n' | tr 'a-f' 'A-F')
  if (( ${#owner_hex} == 0 || ${#owner_hex} > 256 )); then
    echo "DBP1426E Each owner name must occupy 1-128 UTF-8 bytes." >&2
    exit 2
  fi
  encoded+=("$owner_hex")
done
if (( ${#encoded[@]} > 16384 )); then
  echo "DBP1426E At most 16,384 owners can be captured in one spool." >&2
  exit 2
fi

sorted_encoded=()
while IFS= read -r owner_hex; do
  sorted_encoded[${#sorted_encoded[@]}]=$owner_hex
done < <(printf '%s\n' "${encoded[@]}" | LC_ALL=C sort)
encoded=("${sorted_encoded[@]}")
for (( index = 1; index < ${#encoded[@]}; index++ )); do
  if [[ "${encoded[index - 1]}" == "${encoded[index]}" ]]; then
    echo "DBP1426E Owner names must be unique." >&2
    exit 2
  fi
done
owner_hex_list=$(IFS=,; printf '%s' "${encoded[*]}")

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
spool_path=$PWD/dbwarp-blueprint-oracle-basic.spool
raw_path=$PWD/dbwarp-blueprint-oracle-basic.raw
bind_path=$PWD/dbwarp-blueprint-owner-bind.sql
partial_path=$PWD/.dbwarp-blueprint-oracle-basic.spool.partial
for path in "$spool_path" "$raw_path" "$bind_path" "$partial_path"; do
  if [[ -e "$path" ]]; then
    echo "DBP1426E Refusing to overwrite $path; move or delete the previous capture file first." >&2
    exit 2
  fi
done
if ! command -v sqlplus >/dev/null 2>&1; then
  echo "DBP1426E sqlplus was not found on PATH." >&2
  exit 2
fi

umask 077
: >"$raw_path"
{
  printf '%s\n' 'BEGIN'
  offset=0
  first_chunk=1
  while (( offset < ${#owner_hex_list} )); do
    chunk=${owner_hex_list:offset:180}
    offset=$((offset + ${#chunk}))
    if (( first_chunk != 0 )); then
      printf "  :DBWARP_BP_OWNER_HEX_LIST := TO_CLOB('%s');\n" "$chunk"
      first_chunk=0
    else
      printf "  :DBWARP_BP_OWNER_HEX_LIST := :DBWARP_BP_OWNER_HEX_LIST || '%s';\n" "$chunk"
    fi
  done
  printf '%s\n' 'END;' '/'
} >"$bind_path"
chmod 600 "$raw_path" "$bind_path"
sqlplus_pid=
# These functions are invoked through traps; ShellCheck cannot follow that
# indirect control flow.
# shellcheck disable=SC2317
cleanup() {
  if [[ -n "$sqlplus_pid" ]] && kill -0 "$sqlplus_pid" 2>/dev/null; then
    kill -TERM "$sqlplus_pid" 2>/dev/null || true
  fi
  rm -f "$bind_path" "$raw_path" "$partial_path"
}
interrupted=0
# shellcheck disable=SC2317
handle_interrupt() {
  if [[ -n "$sqlplus_pid" ]] && kill -0 "$sqlplus_pid" 2>/dev/null; then
    # Stop the client before sanitizing. A closed marker below records that
    # server-side cancellation could not be confirmed from this local signal.
    interrupted=1
    kill -TERM "$sqlplus_pid" 2>/dev/null || true
  fi
}
# shellcheck disable=SC2317
handle_termination() {
  code=$1
  if [[ -n "$sqlplus_pid" ]]; then
    if kill -0 "$sqlplus_pid" 2>/dev/null; then
      interrupted=1
      kill -TERM "$sqlplus_pid" 2>/dev/null || true
    fi
    return
  fi
  exit "$code"
}
# shellcheck disable=SC2317
remove_incomplete_final_record() {
  if [[ ! -s "$raw_path" ]]; then return; fi
  final_byte=$(tail -c 1 "$raw_path" | od -An -tu1 | tr -d '[:space:]')
  if [[ "$final_byte" == 10 ]]; then return; fi
  sed '$d' "$raw_path" >"$partial_path"
  chmod 600 "$partial_path"
  mv "$partial_path" "$raw_path"
}
trap cleanup EXIT
trap 'handle_termination 129' HUP
trap handle_interrupt INT
trap 'handle_termination 143' TERM
set +e
exec 3<&0
# The bind uses an explicit ./ path. Restrict SQL*Plus's nested-script search to
# this capture pack so an unrelated file in the DBA's run directory cannot
# replace a generated family script.
SQLPATH="$script_dir" sqlplus -L "$connect" "@$script_dir/run.sql" <&3 &
sqlplus_pid=$!
wait "$sqlplus_pid"
status=$?
if (( status != 0 || interrupted != 0 )); then
  # A killed or failed client can leave one buffered frame incomplete. Keep
  # only newline-terminated records before classifying the retained prefix.
  remove_incomplete_final_record
fi
if (( interrupted != 0 )); then
  wait "$sqlplus_pid" 2>/dev/null || true
  # Start the closed marker on a fresh line even after a complete prior frame.
  printf '\n%s\n' 'DBWARP_BP_RAW|A|UNCONFIRMED_CANCELLATION' >>"$raw_path"
fi
sqlplus_pid=
exec 3<&-
set -e
trap - INT
trap - HUP TERM
if [[ ! -s "$raw_path" ]]; then
  startup_failure_message "" >&2
  if (( status == 0 )); then exit 1; else exit "$status"; fi
fi
raw_family=
while IFS='|' read -r header_prefix header_kind header_contract header_family header_manifest header_digest header_expected header_extra; do
  if [[ "$header_prefix" == DBWARP_BP_RAW && "$header_kind" == H &&
        "$header_contract" == 2 && ${#header_manifest} -eq 64 &&
        ${#header_digest} -eq 64 && "$header_expected" != *[!0-9]* &&
        -z "$header_extra" ]]; then
    raw_family=$header_family
    break
  fi
done <"$raw_path"
if [[ -z "$raw_family" ]]; then
  startup_ora=$(LC_ALL=C sed -n 's/^[[:space:]]*ORA-\([0-9][0-9]*\):.*/\1/p' "$raw_path" | sed -n '1p')
  if [[ -n "$startup_ora" ]]; then
    startup_ora=$((10#$startup_ora))
    startup_failure_message "$startup_ora" >&2
    exit 1
  fi
fi
case "$raw_family" in
  oracle-12.1) family_script=$script_dir/oracle-12c/basic-12.1.sql ;;
  oracle-12.2) family_script=$script_dir/oracle-12c/basic-12.2.sql ;;
  oracle-19c) family_script=$script_dir/oracle-19c/basic.sql ;;
  oracle-21c) family_script=$script_dir/oracle-21c/basic.sql ;;
  oracle-26ai) family_script=$script_dir/oracle-26ai/basic.sql ;;
  *) startup_failure_message "" >&2; exit 1 ;;
esac
if ! command -v cksum >/dev/null 2>&1; then
  echo "DBP1426E The POSIX cksum utility is required to verify the capture script." >&2
  exit 1
fi
expected_checksum=$(LC_ALL=C sed -n 's/^-- POSIX cksum: \([0-9][0-9]*\)$/\1/p' "$family_script")
if [[ ${#expected_checksum} -ne 10 || "$expected_checksum" == *[!0-9]* ]]; then
  echo "DBP1426E The selected capture script has no valid portable checksum." >&2
  exit 1
fi
checksum_result=$(LC_ALL=C sed 's/^-- POSIX cksum: [0-9][0-9]*$/-- POSIX cksum: 0000000000/' "$family_script" | cksum)
read -r actual_checksum actual_size checksum_extra <<<"$checksum_result"
actual_checksum=$(printf '%010u' "$actual_checksum")
if [[ "$expected_checksum" != "$actual_checksum" ||
      -z "$actual_size" || "$actual_size" == *[!0-9]* ||
      -n "$checksum_extra" ]]; then
  echo "DBP1426E The selected capture script failed its portable integrity check." >&2
  exit 1
fi
if ! LC_ALL=C awk -f "$script_dir/sanitize.awk" "$raw_path" >"$partial_path"; then
  echo "DBP1426E Capture output was not a valid DBWarp Blueprint Oracle Basic stream; preserve the terminal diagnostic and rerun from a private empty directory." >&2
  exit 1
fi
chmod 600 "$partial_path"
mv "$partial_path" "$spool_path"
chmod 600 "$spool_path"
if (( status != 0 || interrupted != 0 )); then
  echo "SQL*Plus ended after producing a classified partial spool; conversion will preserve its recorded gaps." >&2
fi
exit 0

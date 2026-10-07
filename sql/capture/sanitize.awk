# DBWarp Blueprint Oracle Basic raw-spool sanitizer.
#
# Input can contain Oracle/SQL*Plus diagnostics with native object names. Only
# validated framed data and closed error classes are written to the transferable
# spool; Oracle message text is never copied.

BEGIN {
    FS = "|"
    OFS = "|"
    raw = "DBWARP_BP_RAW"
    final = "DBWARP_BP"
    if (max_rows == "") max_rows = 1000000
    if (max_bytes == "") max_bytes = 268435456
    expected = 0
    sequence = 0
    total_rows = 0
    total_bytes = 0
    query_rows = 0
    in_query = 0
    abort_reason = "NONE"
    fatal = 0
}

function fail_closed(reason) {
    if (abort_reason == "NONE") abort_reason = reason
    malformed = 1
    fatal = 1
}

function emit_missing(from, i) {
    for (i = from; i <= expected; i++) print final, "Q", i, "S", "NOT_REACHED"
}

function classify_ora(code) {
    if (code == 1031) return "PERMISSION_DENIED"
    if (code == 942) return "OBJECT_ABSENT"
    if (code == 28 || code == 3113 || code == 3114 || code == 3135 || code == 12537 || code == 12547) return "SESSION_LOST"
    return "DATABASE_ERROR"
}

function finish_query(   klass) {
    if (!in_query) return
    if (ora_code != "") {
        klass = classify_ora(ora_code + 0)
        if ((ora_code + 0) == 1013) {
            print final, "Q", sequence, "A", "UNCONFIRMED_CANCELLATION", query_rows
            abort_reason = "UNCONFIRMED_CANCELLATION"
            fatal = 1
        } else {
            print final, "Q", sequence, "F", klass, "ORA", ora_code + 0, query_rows
            if (klass == "SESSION_LOST") {
                abort_reason = "SESSION_LOST"
                fatal = 1
            }
        }
    } else if (local_failure != "") {
        print final, "Q", sequence, "F", local_failure, "LOCAL", 0, query_rows
    } else if (query_noise) {
        malformed = 1
        fatal = 1
    } else {
        print final, "Q", sequence, "E", query_rows
    }
    in_query = 0
    ora_code = ""
    local_failure = ""
    query_noise = 0
    query_rows = 0
}

$1 == raw && $2 == "H" {
    if (seen_header || NF != 7 || $3 != 2 || $7 !~ /^[0-9]+$/) {
        fail_closed("SESSION_LOST")
        next
    }
    seen_header = 1
    expected = $7 + 0
    print final, "H", $3, $4, $5, $6
    next
}

$1 == raw && ($2 == "M" || $2 == "O") {
    if (!seen_header || in_query || fatal) {
        fail_closed("SESSION_LOST")
        next
    }
    $1 = final
    print
    next
}

$1 == raw && $2 == "C" {
    if (!seen_header || in_query || NF != 3 || $3 !~ /^[0-9]+$/ || start_ms != 0) {
        fail_closed("SESSION_LOST")
        next
    }
    start_ms = $3 + 0
    next
}

fatal {
    # A bounded abort is final. SQL*Plus may already have written later valid
    # frames, or the launcher may add its cancellation marker after SQL*Plus
    # recorded an earlier abort. Ignore that tail rather than rejecting the
    # usable prefix. Preserve a genuine end timestamp when one is available.
    if ($1 == raw && $2 == "X" && NF == 4 && $4 ~ /^[0-9]+$/) end_ms = $4 + 0
    next
}

$1 == raw && $2 == "A" && $3 == "UNCONFIRMED_CANCELLATION" && NF == 3 {
    if (!seen_header) {
        fail_closed("SESSION_LOST")
        next
    }
    if (in_query) {
        print final, "Q", sequence, "A", "UNCONFIRMED_CANCELLATION", query_rows
        in_query = 0
    }
    abort_reason = "UNCONFIRMED_CANCELLATION"
    fatal = 1
    next
}

$1 == raw && $2 == "Q" && $4 == "S" {
    if (fatal || in_query || ($3 + 0) != sequence + 1 || NF != 5) {
        fail_closed("SESSION_LOST")
        next
    }
    sequence++
    print final, "Q", sequence, "S", $5
    next
}

$1 == raw && $2 == "Q" && $7 == "B" {
    if (fatal || in_query || ($3 + 0) != sequence + 1 || NF != 7) {
        fail_closed("SESSION_LOST")
        next
    }
    sequence++
    in_query = 1
    query_rows = 0
    ora_code = ""
    local_failure = ""
    query_noise = 0
    print final, "Q", sequence, $4, $5, $6, "B"
    next
}

$1 == raw && $2 == "R" {
    if (fatal || !in_query || ($3 + 0) != sequence || ($4 + 0) != query_rows + 1 || $NF != "Z") {
        fail_closed("SESSION_LOST")
        next
    }
    row_bytes = 0
    oversize = 0
    for (i = 5; i < NF; i++) {
        if ($i == "L") oversize = 1
        else if ($i == "N") { }
        else if ($i ~ /^T[0-9A-Fa-f]*$/ && (length($i) - 1) % 2 == 0) row_bytes += (length($i) - 1) / 2
        else {
            fail_closed("SESSION_LOST")
            next
        }
    }
    query_rows++
    total_rows++
    total_bytes += row_bytes
    if (oversize) {
        local_failure = "TRANSIENT_VALUE_LIMIT"
        next
    }
    if (total_rows > max_rows) {
        print final, "Q", sequence, "A", "ROW_LIMIT", query_rows
        abort_reason = "ROW_LIMIT"
        fatal = 1
        in_query = 0
        next
    }
    if (total_bytes > max_bytes) {
        print final, "Q", sequence, "A", "BYTE_LIMIT", query_rows
        abort_reason = "BYTE_LIMIT"
        fatal = 1
        in_query = 0
        next
    }
    if (local_failure == "") {
        $1 = final
        print
    }
    next
}

$1 == raw && $2 == "Q" && $4 == "E" {
    if (fatal || !in_query || ($3 + 0) != sequence || NF != 4) {
        fail_closed("SESSION_LOST")
        next
    }
    finish_query()
    next
}

$1 == raw && $2 == "X" {
    if (fatal || in_query || NF != 4 || ($3 + 0) != expected || sequence != expected || $4 !~ /^[0-9]+$/) {
        fail_closed("SESSION_LOST")
        next
    }
    saw_end = 1
    end_ms = $4 + 0
    next
}

/^[[:space:]]*(ORA|SP2)-[0-9]+:/ {
    if (in_query && match($0, /(ORA|SP2)-[0-9]+/)) {
        code_text = substr($0, RSTART + 4, RLENGTH - 4)
        if (substr($0, RSTART, 3) == "ORA") ora_code = code_text + 0
        else {
            abort_reason = "SESSION_LOST"
            fatal = 1
        }
    } else {
        fail_closed("SESSION_LOST")
    }
    next
}

/^[[:space:]]*ERROR at line [0-9]+:[[:space:]]*$/ {
    # SQL*Plus emits this location-only line before the classified ORA code.
    # It contains no catalogue value and is safe to discard.
    next
}

NF > 0 {
    # SQL*Plus can print the rejected SQL line and a caret before its ORA code.
    # Never copy that native text. Remember it until the closed ORA class
    # arrives; if no ORA follows, the query frame is malformed and fatal.
    if (seen_header && in_query) query_noise = 1
    else if (seen_header) fail_closed("SESSION_LOST")
}

END {
    if (!seen_header) exit 3
    if (malformed) exit 3
    if (in_query) {
        print final, "Q", sequence, "A", "SESSION_LOST", query_rows
        abort_reason = "SESSION_LOST"
        fatal = 1
        in_query = 0
    }
    if (!saw_end && abort_reason == "NONE") abort_reason = "SESSION_LOST"
    if (sequence < expected) emit_missing(sequence + 1)
    # Complete raw captures carry both database timestamps. A session lost
    # before the final timestamp retains its proven structural prefix and uses
    # zero rather than depending on non-POSIX awk wall-clock extensions.
    elapsed_ms = start_ms > 0 && end_ms >= start_ms ? end_ms - start_ms : 0
    print final, "X", expected, total_rows, total_bytes, elapsed_ms, abort_reason
}

param(
    [Parameter(Mandatory = $true)][string] $InputPath,
    [Parameter(Mandatory = $true)][string] $OutputPath
)

$ErrorActionPreference = 'Stop'
# Framed content is ASCII plus hex. Native SQL*Plus diagnostics may use the
# host code page and are discarded, so replacement decoding is safer than
# rejecting an otherwise usable prefix before it can be sanitized.
$utf8 = New-Object System.Text.UTF8Encoding($false, $false)
$reader = New-Object System.IO.StreamReader($InputPath, $utf8, $true)
$writer = New-Object System.IO.StreamWriter($OutputPath, $false, $utf8)
$started = [DateTimeOffset]::UtcNow
$expected = 0
$sequence = 0
$totalRows = [uint64]0
$totalBytes = [uint64]0
$queryRows = [uint64]0
$inQuery = $false
$seenHeader = $false
$sawEnd = $false
$abortReason = 'NONE'
$startMs = [uint64]0
$endMs = [uint64]0
$oraCode = $null
$localFailure = $null
$queryNoise = $false
$malformed = $false

function Write-Frame([string[]] $Fields) {
    $writer.WriteLine(($Fields -join '|'))
}

function Set-Abort([string] $Reason) {
    if ($script:abortReason -eq 'NONE') { $script:abortReason = $Reason }
}

function Set-Malformed {
    $script:malformed = $true
}

function Finish-Query {
    if (-not $script:inQuery) { return }
    if ($null -ne $script:oraCode) {
        [int] $code = $script:oraCode
        if ($code -eq 1013) {
            Write-Frame @('DBWARP_BP', 'Q', "$script:sequence", 'A', 'UNCONFIRMED_CANCELLATION', "$script:queryRows")
            Set-Abort 'UNCONFIRMED_CANCELLATION'
        } else {
            $class = if ($code -eq 1031) { 'PERMISSION_DENIED' } elseif ($code -eq 942) { 'OBJECT_ABSENT' } elseif ($code -in @(28, 3113, 3114, 3135, 12537, 12547)) { 'SESSION_LOST' } else { 'DATABASE_ERROR' }
            Write-Frame @('DBWARP_BP', 'Q', "$script:sequence", 'F', $class, 'ORA', "$code", "$script:queryRows")
            if ($class -eq 'SESSION_LOST') { Set-Abort 'SESSION_LOST' }
        }
    } elseif ($null -ne $script:localFailure) {
        Write-Frame @('DBWARP_BP', 'Q', "$script:sequence", 'F', $script:localFailure, 'LOCAL', '0', "$script:queryRows")
    } elseif ($script:queryNoise) {
        Set-Malformed
    } else {
        Write-Frame @('DBWARP_BP', 'Q', "$script:sequence", 'E', "$script:queryRows")
    }
    $script:inQuery = $false
    $script:oraCode = $null
    $script:localFailure = $null
    $script:queryNoise = $false
    $script:queryRows = [uint64]0
}

try {
    while ($null -ne ($line = $reader.ReadLine())) {
        if ($malformed) { continue }
        $fields = $line.Split('|')
        if ($abortReason -ne 'NONE') { continue }
        if ($fields[0] -eq 'DBWARP_BP_RAW') {
            if ($fields.Count -eq 7 -and $fields[1] -eq 'H' -and $fields[2] -eq '2' -and -not $seenHeader) {
                $seenHeader = $true
                $expected = [int]::Parse($fields[6], [Globalization.CultureInfo]::InvariantCulture)
                Write-Frame @('DBWARP_BP', 'H', $fields[2], $fields[3], $fields[4], $fields[5])
                continue
            }
            if ($seenHeader -and -not $inQuery -and $fields.Count -eq 4 -and $fields[1] -in @('M', 'O')) {
                $fields[0] = 'DBWARP_BP'; Write-Frame $fields; continue
            }
            if ($seenHeader -and -not $inQuery -and $fields.Count -eq 3 -and $fields[1] -eq 'C' -and $startMs -eq 0) {
                $startMs = [uint64]::Parse($fields[2], [Globalization.CultureInfo]::InvariantCulture)
                continue
            }
            if ($seenHeader -and $fields.Count -eq 3 -and $fields[1] -eq 'A' -and $fields[2] -eq 'UNCONFIRMED_CANCELLATION') {
                if ($inQuery) {
                    Write-Frame @('DBWARP_BP', 'Q', "$sequence", 'A', 'UNCONFIRMED_CANCELLATION', "$queryRows")
                    $inQuery = $false
                }
                Set-Abort 'UNCONFIRMED_CANCELLATION'
                continue
            }
            if ($fields.Count -eq 5 -and $fields[1] -eq 'Q' -and $fields[3] -eq 'S' -and [int]$fields[2] -eq $sequence + 1) {
                $sequence++; Write-Frame @('DBWARP_BP', 'Q', "$sequence", 'S', $fields[4]); continue
            }
            if ($fields.Count -eq 7 -and $fields[1] -eq 'Q' -and $fields[6] -eq 'B' -and -not $inQuery -and [int]$fields[2] -eq $sequence + 1) {
                $sequence++; $inQuery = $true; $queryRows = 0; $oraCode = $null; $localFailure = $null; $queryNoise = $false
                Write-Frame @('DBWARP_BP', 'Q', "$sequence", $fields[3], $fields[4], $fields[5], 'B')
                continue
            }
            if ($fields.Count -ge 6 -and $fields[1] -eq 'R' -and $inQuery -and [int]$fields[2] -eq $sequence -and [int]$fields[3] -eq $queryRows + 1 -and $fields[-1] -eq 'Z') {
                [uint64] $rowBytes = 0
                $oversize = $false
                for ($index = 4; $index -lt $fields.Count - 1; $index++) {
                    $token = $fields[$index]
                    if ($token -eq 'L') { $oversize = $true }
                    elseif ($token -eq 'N') { }
                    elseif ($token -cmatch '^T(?:[0-9A-Fa-f]{2})*$') { $rowBytes += [uint64](($token.Length - 1) / 2) }
                    else { Set-Malformed; break }
                }
                if ($malformed) { continue }
                $queryRows++; $totalRows++; $totalBytes += $rowBytes
                if ($oversize) { $localFailure = 'TRANSIENT_VALUE_LIMIT'; continue }
                if ($totalRows -gt 1000000) { Write-Frame @('DBWARP_BP', 'Q', "$sequence", 'A', 'ROW_LIMIT', "$queryRows"); Set-Abort 'ROW_LIMIT'; $inQuery = $false; continue }
                if ($totalBytes -gt 268435456) { Write-Frame @('DBWARP_BP', 'Q', "$sequence", 'A', 'BYTE_LIMIT', "$queryRows"); Set-Abort 'BYTE_LIMIT'; $inQuery = $false; continue }
                if ($null -eq $localFailure -and $abortReason -eq 'NONE') { $fields[0] = 'DBWARP_BP'; Write-Frame $fields }
                continue
            }
            if ($fields.Count -eq 4 -and $fields[1] -eq 'Q' -and $fields[3] -eq 'E' -and $inQuery -and [int]$fields[2] -eq $sequence) { Finish-Query; continue }
            if ($fields.Count -eq 4 -and $fields[1] -eq 'X' -and [int]$fields[2] -eq $expected -and $sequence -eq $expected -and -not $inQuery) { $sawEnd = $true; $endMs = [uint64]::Parse($fields[3], [Globalization.CultureInfo]::InvariantCulture); continue }
            Set-Malformed
            continue
        }
        if ($line -cmatch '^\s*ORA-([0-9]+):') {
            if ($inQuery) { $oraCode = [int]$Matches[1] } else { Set-Malformed }
        } elseif ($line -cmatch '^\s*SP2-[0-9]+:') {
            Set-Malformed
        } elseif ($line -cmatch '^\s*ERROR at line [0-9]+:\s*$') {
            # Location-only SQL*Plus output; the following ORA code carries
            # the closed failure class and the line itself is discarded.
        } elseif ($seenHeader -and $line.Length -ne 0) {
            if ($inQuery) { $queryNoise = $true } else { Set-Malformed }
        }
    }
    if (-not $seenHeader) { throw 'DBP1426E Capture did not produce a framed spool. Check the selected PDB, collector grants, exact-case owner names, and server family.' }
    if ($malformed) { throw 'DBP1426E Capture output violated the DBWarp Blueprint framing contract.' }
    if ($inQuery) { Write-Frame @('DBWARP_BP', 'Q', "$sequence", 'A', 'SESSION_LOST', "$queryRows"); Set-Abort 'SESSION_LOST' }
    if (-not $sawEnd -and $abortReason -eq 'NONE') { Set-Abort 'SESSION_LOST' }
    for ($index = $sequence + 1; $index -le $expected; $index++) { Write-Frame @('DBWARP_BP', 'Q', "$index", 'S', 'NOT_REACHED') }
    if ($startMs -gt 0) {
        $observedEnd = if ($endMs -gt 0) { $endMs } else { [uint64][DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() }
        $elapsed = if ($observedEnd -ge $startMs) { $observedEnd - $startMs } else { [uint64]0 }
    } else {
        $elapsed = [uint64]([DateTimeOffset]::UtcNow - $started).TotalMilliseconds
    }
    Write-Frame @('DBWARP_BP', 'X', "$expected", "$totalRows", "$totalBytes", "$elapsed", $abortReason)
} catch {
    # PowerShell's conversion exceptions can quote the rejected input. Keep
    # native catalogue text out of the terminal just as the Unix sanitizer
    # does, and expose only the closed framing failure.
    throw 'DBP1426E Capture output violated the DBWarp Blueprint framing contract.'
} finally {
    $reader.Dispose()
    $writer.Dispose()
}

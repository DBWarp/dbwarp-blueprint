param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string] $Connect,

    [Parameter(Position = 1, ValueFromRemainingArguments = $true)]
    [string[]] $Owners,

    [Parameter(Mandatory = $false)]
    [string] $OwnerFile
)

$ErrorActionPreference = 'Stop'

trap {
    $message = $_.Exception.Message -replace '[\r\n]+', ' '
    [Console]::Error.WriteLine($message)
    exit 1
}

function Get-StartupFailureMessage([int] $Status, [string] $RawPath) {
    $oraCode = $null
    if (Test-Path -LiteralPath $RawPath) {
        foreach ($line in [System.IO.File]::ReadLines($RawPath, $utf8NoBom)) {
            if ($line -cmatch '^\s*ORA-([0-9]+):') { $oraCode = [int]$Matches[1]; break }
        }
    }
    if ($null -eq $oraCode -and $Status -lt 0) {
        $oraCode = -$Status
    }
    # Windows SQL*Plus exits with the whole ORA number (for example 20001 or
    # 3106); only Unix truncates it to the 8-bit values decoded below.
    if ($null -eq $oraCode -and $Status -gt 255) {
        $oraCode = $Status
    }
    if ($null -eq $oraCode) {
        $oraCode = switch ($Status -band 255) {
            7 { 1017; break }
            235 { 1045; break }
            249 { 1031; break }
            82 { 942; break }
            134 { 12154; break }
            126 { 12162; break }
            118 { 12170; break }
            9 { 12535; break }
            39 { 12505; break }
            30 { 12514; break }
            3 { 12541; break }
            255 { 12545; break }
            253 { 12547; break }
            240 { 12560; break }
            160 { 28000; break }
            159 { 28001; break }
            223 { 20001; break }
            220 { 20004; break }
            219 { 20005; break }
            218 { 20006; break }
            default { $null }
        }
    }
    $oraLabel = if ($null -eq $oraCode) { '' } else { '{0:D5}' -f [int]$oraCode }
    switch ($oraCode) {
        1017 { return 'DBP1426E Oracle rejected the collector credentials (ORA-01017). Verify the account name and password, then retry.' }
        1045 { return 'DBP1426E The collector cannot create a session (ORA-01045). Apply the minimum or Basic grant script, then retry.' }
        { $_ -in @(1031, 942) } { return "DBP1426E Oracle denied or lacks a required admission catalogue (ORA-$oraLabel). Reapply the matching minimum or Basic grant script in the selected PDB." }
        { $_ -in @(12154, 12162) } { return "DBP1426E SQL*Net could not resolve the requested service (ORA-$oraLabel). Check the service alias and Oracle network configuration." }
        { $_ -in @(12170, 12535) } { return "DBP1426E The Oracle connection timed out (ORA-$oraLabel). Check routing, firewall policy, listener reachability, and the service name." }
        { $_ -in @(12505, 12514) } { return "DBP1426E The listener does not know the requested service (ORA-$oraLabel). Use the PDB service registered with the listener." }
        { $_ -in @(12541, 12543, 12545) } { return "DBP1426E The Oracle listener or host is unreachable (ORA-$oraLabel). Check the host, port, listener, routing, and firewall policy." }
        { $_ -in @(3106, 12547, 12560) } { return "DBP1426E The local SQL*Plus client or Oracle protocol stack failed (ORA-$oraLabel). Check the client installation and Oracle environment." }
        28000 { return 'DBP1426E The collector account is locked (ORA-28000). Ask the DBA to unlock the dedicated collector account.' }
        28001 { return 'DBP1426E The collector password is expired (ORA-28001). Reset it under your password policy, then retry.' }
        20001 { return 'DBP1426E The owner input was malformed (ORA-20001). Pass ordinary exact-case schema names, one per argument or owner-file line.' }
        20004 { return 'DBP1426E This Oracle server family is not supported by the selected capture pack (ORA-20004). Use the pack shipped with this DBWarp Blueprint build.' }
        20005 { return 'DBP1426E The session connected to CDB$ROOT (ORA-20005). Connect to the intended pluggable-database service and retry.' }
        20006 { return 'DBP1426E A requested owner was not found (ORA-20006). Verify every schema name and its exact Oracle letter case, then retry.' }
        default {
            if ($Status -eq 0) {
                return 'DBP1426E SQL*Plus exited successfully without a capture header. Use the complete capture pack shipped with this DBWarp Blueprint build and retry from a private empty directory.'
            }
            return "DBP1426E SQL*Plus exited with status $Status before producing a capture header. Check the client installation, login, selected PDB, collector grants, and service reachability."
        }
    }
}

$at = $Connect.IndexOf('@')
if ($at -le 0 -or $Connect.Substring(0, $at).Contains('/') -or
    $Connect.ToCharArray().Where({ [char]::IsWhiteSpace($_) -or [char]::IsControl($_) }).Count -ne 0) {
    throw 'DBP1426E The connection must be username@service without a password; SQL*Plus will prompt securely.'
}
$connectUser = $Connect.Substring(0, $at)
if ($connectUser.Equals('SYS', [StringComparison]::OrdinalIgnoreCase) -or
    $connectUser.Equals('SYSTEM', [StringComparison]::OrdinalIgnoreCase)) {
    throw 'DBP1426E Run the capture as the dedicated minimum- or Basic-grant collector account, not SYS or SYSTEM.'
}
$utf8 = New-Object System.Text.UTF8Encoding($false, $true)
if (-not [string]::IsNullOrEmpty($OwnerFile)) {
    if ($Owners.Count -ne 0) {
        throw 'DBP1426E Pass owner names directly or through -OwnerFile, not both.'
    }
    if (-not (Test-Path -LiteralPath $OwnerFile -PathType Leaf)) {
        throw 'DBP1426E The owner file is not a readable regular file.'
    }
    $Owners = [System.IO.File]::ReadAllLines($OwnerFile, $utf8)
}
if ($Owners.Count -eq 0) {
    throw 'DBP1426E Provide at least one ordinary Oracle owner name.'
}
if ($Owners.Count -gt 16384) {
    throw 'DBP1426E At most 16,384 owners can be captured in one spool.'
}

[string[]] $encoded = foreach ($owner in $Owners) {
    if ([string]::IsNullOrEmpty($owner) -or
        $owner.ToCharArray().Where({ [char]::IsControl($_) }).Count -ne 0) {
        throw 'DBP1426E Owner names must be non-empty and contain no control characters.'
    }
    [byte[]] $bytes = $utf8.GetBytes($owner)
    if ($bytes.Count -eq 0 -or $bytes.Count -gt 128) {
        throw 'DBP1426E Each owner name must occupy 1-128 UTF-8 bytes.'
    }
    -join ($bytes | ForEach-Object { $_.ToString('X2') })
}
[Array]::Sort($encoded, [StringComparer]::Ordinal)
for ($index = 1; $index -lt $encoded.Count; $index++) {
    if ($encoded[$index - 1] -ceq $encoded[$index]) {
        throw 'DBP1426E Owner names must be unique.'
    }
}
$ownerHexList = $encoded -join ','

$sqlplus = Get-Command sqlplus -CommandType Application -ErrorAction SilentlyContinue
if ($null -eq $sqlplus) {
    throw 'DBP1426E sqlplus was not found on PATH.'
}
$spoolPath = Join-Path (Get-Location) 'dbwarp-blueprint-oracle-basic.spool'
$rawPath = Join-Path (Get-Location) 'dbwarp-blueprint-oracle-basic.raw'
$bindPath = Join-Path (Get-Location) 'dbwarp-blueprint-owner-bind.sql'
$partialPath = Join-Path (Get-Location) '.dbwarp-blueprint-oracle-basic.spool.partial'
foreach ($path in @($spoolPath, $rawPath, $bindPath, $partialPath)) {
    if (Test-Path -LiteralPath $path) {
        throw "DBP1426E Refusing to overwrite $path; move or delete the previous capture file first."
    }
}

$identity = [System.Security.Principal.WindowsIdentity]::GetCurrent().User
$acl = New-Object System.Security.AccessControl.FileSecurity
$acl.SetOwner($identity)
$acl.SetAccessRuleProtection($true, $false)
$rule = New-Object System.Security.AccessControl.FileSystemAccessRule(
    $identity,
    [System.Security.AccessControl.FileSystemRights]::FullControl,
    [System.Security.AccessControl.AccessControlType]::Allow
)
$acl.AddAccessRule($rule)
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)

function Protect-CaptureFile([string] $Path) {
    Set-Acl -LiteralPath $Path -AclObject $acl
}

function Remove-IncompleteFinalRecord([string] $Path) {
    $stream = [System.IO.File]::Open($Path, [System.IO.FileMode]::Open,
        [System.IO.FileAccess]::ReadWrite, [System.IO.FileShare]::Read)
    try {
        if ($stream.Length -eq 0) { return }
        $stream.Position = $stream.Length - 1
        if ($stream.ReadByte() -eq 10) { return }
        for ($position = $stream.Length - 1; $position -ge 0; $position--) {
            $stream.Position = $position
            if ($stream.ReadByte() -eq 10) {
                $stream.SetLength($position + 1)
                return
            }
        }
        $stream.SetLength(0)
    } finally {
        $stream.Dispose()
    }
}

try {
    [System.IO.File]::WriteAllText($rawPath, '', $utf8NoBom)
    # @() keeps a single chunk as a one-element array; without it PowerShell
    # returns a bare string and $chunks[0] would bind only its first character.
    $chunks = @(for ($offset = 0; $offset -lt $ownerHexList.Length; $offset += 180) {
        $ownerHexList.Substring($offset, [Math]::Min(180, $ownerHexList.Length - $offset))
    })
    $bindLines = @('BEGIN')
    for ($index = 0; $index -lt $chunks.Count; $index++) {
        if ($index -eq 0) {
            $bindLines += "  :DBWARP_BP_OWNER_HEX_LIST := TO_CLOB('$($chunks[$index])');"
        } else {
            $bindLines += "  :DBWARP_BP_OWNER_HEX_LIST := :DBWARP_BP_OWNER_HEX_LIST || '$($chunks[$index])';"
        }
    }
    $bindLines += @('END;', '/')
    [System.IO.File]::WriteAllLines($bindPath, $bindLines, $utf8NoBom)
    Protect-CaptureFile $rawPath
    Protect-CaptureFile $bindPath

    $status = 130
    $interrupted = $false
    $previousSqlPath = $env:SQLPATH
    try {
        # The bind uses an explicit ./ path. Restrict nested-script lookup to
        # this capture pack so the run directory cannot replace family SQL.
        $env:SQLPATH = $PSScriptRoot
        & $sqlplus.Source '-L' $Connect "@$PSScriptRoot\run.sql"
        $status = $LASTEXITCODE
    } catch [System.Management.Automation.PipelineStoppedException] {
        # Preserve and sanitize the completed raw prefix after an interactive
        # cancellation instead of deleting the only usable capture evidence.
        $status = 130
        $interrupted = $true
    } finally {
        if ($null -eq $previousSqlPath) {
            Remove-Item Env:SQLPATH -ErrorAction SilentlyContinue
        } else {
            $env:SQLPATH = $previousSqlPath
        }
    }
    if (($status -ne 0 -or $interrupted) -and (Test-Path -LiteralPath $rawPath)) {
        Remove-IncompleteFinalRecord $rawPath
    }
    if ($interrupted -and (Test-Path -LiteralPath $rawPath)) {
        [System.IO.File]::AppendAllText(
            $rawPath,
            "`nDBWARP_BP_RAW|A|UNCONFIRMED_CANCELLATION`n",
            $utf8NoBom
        )
    }
    if (-not (Test-Path -LiteralPath $rawPath) -or (Get-Item -LiteralPath $rawPath).Length -eq 0) {
        throw (Get-StartupFailureMessage $status $rawPath)
    }
    $rawHeader = [System.IO.File]::ReadLines($rawPath, $utf8NoBom) |
        Where-Object { $_.StartsWith('DBWARP_BP_RAW|H|') } |
        Select-Object -First 1
    $headerFields = $rawHeader -split '\|'
    if ($headerFields.Count -ne 7) { throw (Get-StartupFailureMessage $status $rawPath) }
    $familyScript = switch ($headerFields[3]) {
        'oracle-12.1' { Join-Path $PSScriptRoot 'oracle-12c\basic-12.1.sql' }
        'oracle-12.2' { Join-Path $PSScriptRoot 'oracle-12c\basic-12.2.sql' }
        'oracle-19c' { Join-Path $PSScriptRoot 'oracle-19c\basic.sql' }
        'oracle-21c' { Join-Path $PSScriptRoot 'oracle-21c\basic.sql' }
        'oracle-26ai' { Join-Path $PSScriptRoot 'oracle-26ai\basic.sql' }
        default { throw (Get-StartupFailureMessage $status $rawPath) }
    }
    $zeroDigest = '0' * 64
    $familyBody = [System.IO.File]::ReadAllText($familyScript, $utf8NoBom).Replace("`r`n", "`n")
    $normalizedBody = [Text.RegularExpressions.Regex]::Replace(
        $familyBody,
        '(PROMPT DBWARP_BP_RAW\|H\|2\|[^|]+\|[0-9a-f]+\|)[0-9a-f]{64}(\|)',
        "`${1}$zeroDigest`${2}",
        [Text.RegularExpressions.RegexOptions]::CultureInvariant
    )
    $normalizedBody = [Text.RegularExpressions.Regex]::Replace(
        $normalizedBody,
        '(?m)^-- POSIX cksum: [0-9]{10}$',
        '-- POSIX cksum: 0000000000',
        [Text.RegularExpressions.RegexOptions]::CultureInvariant
    )
    $sha256 = [Security.Cryptography.SHA256]::Create()
    try { $actualDigest = -join ($sha256.ComputeHash($utf8NoBom.GetBytes($normalizedBody)) | ForEach-Object { $_.ToString('x2') }) }
    finally { $sha256.Dispose() }
    if ($headerFields[5] -cne $actualDigest) {
        throw 'DBP1426E The executed capture script does not match its recorded SHA-256 identity.'
    }
    & "$PSScriptRoot\sanitize.ps1" -InputPath $rawPath -OutputPath $partialPath
    Protect-CaptureFile $partialPath
    Move-Item -LiteralPath $partialPath -Destination $spoolPath
    Protect-CaptureFile $spoolPath
    if ($status -ne 0) {
        Write-Warning 'SQL*Plus ended after producing a classified partial spool; conversion will preserve its recorded gaps.'
    }
} finally {
    foreach ($path in @($bindPath, $rawPath, $partialPath)) {
        if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -Force }
    }
}

exit 0

param(
    [int]$Limit = 100,
    [switch]$All,
    [switch]$Reset,
    [switch]$SelfTest,
    [string]$SourceRoot = '\\wsl$\Ubuntu\mnt\wsl\PHYSICALDRIVE0p1\calibre\en_nonfiction',
    [string]$BaseConfigPath = 'config/control-plane.toml',
    [string]$DevConfigPath = '.cache/caliberate/control-plane-en-nonfiction.toml'
)

$ErrorActionPreference = 'Stop'

$devDatabasePath = '.cache/caliberate/data/en-nonfiction-dev.sqlite'

function Parse-CalibreFileName {
    param([Parameter(Mandatory)][string]$Stem)

    $separator = ' - '
    $separatorIndex = $Stem.LastIndexOf($separator, [System.StringComparison]::Ordinal)
    if ($separatorIndex -lt 1 -or $separatorIndex -ge ($Stem.Length - $separator.Length)) {
        return [pscustomobject]@{ Title = $Stem; Authors = @() }
    }

    $title = $Stem.Substring(0, $separatorIndex).Trim()
    $authorText = $Stem.Substring($separatorIndex + $separator.Length).Trim()
    $authors = @($authorText -replace '\s+and\s+', ',' -replace '&', ',' |
        ForEach-Object { $_.Split(',') } |
        ForEach-Object { $_.Trim() } |
        Where-Object { $_ })
    if (-not $title -or $authors.Count -eq 0) {
        return [pscustomobject]@{ Title = $Stem; Authors = @() }
    }
    return [pscustomobject]@{ Title = $title; Authors = $authors }
}

function Assert-Equal {
    param($Expected, $Actual, [string]$Message)
    if (-not (@($Expected) -join "`n" -ceq @($Actual) -join "`n")) {
        throw "Self-test failed: $Message (expected '$Expected', actual '$Actual')"
    }
}

function Invoke-SelfTests {
    $parsed = Parse-CalibreFileName 'Title - Author'
    Assert-Equal 'Title' $parsed.Title 'Title - Author title'
    Assert-Equal 'Author' $parsed.Authors 'Title - Author author'

    $parsed = Parse-CalibreFileName 'Part One - Revised - Author'
    Assert-Equal 'Part One - Revised' $parsed.Title 'last separator title'
    Assert-Equal 'Author' $parsed.Authors 'last separator author'

    $parsed = Parse-CalibreFileName 'Standalone Title'
    Assert-Equal 'Standalone Title' $parsed.Title 'separator-free title'
    Assert-Equal @() $parsed.Authors 'separator-free authors'

    $parsed = Parse-CalibreFileName 'Title - Jane Doe & John Roe and Sam Lee'
    Assert-Equal @('Jane Doe', 'John Roe', 'Sam Lee') $parsed.Authors 'multiple authors'
    Write-Host 'Calibre filename parser self-tests passed.'
}

if ($SelfTest) {
    Invoke-SelfTests
    exit 0
}

if (-not (Test-Path -LiteralPath $SourceRoot)) {
    throw "Library source does not exist or is not reachable: $SourceRoot"
}

if (-not (Test-Path -LiteralPath $BaseConfigPath)) {
    throw "Base config file not found: $BaseConfigPath"
}

Write-Host "Configuring Caliberate for reference-backed development library:"
Write-Host "  $SourceRoot"

$config = Get-Content -LiteralPath $BaseConfigPath -Raw
$tomlPath = $SourceRoot.Replace('\', '\\')

$config = [regex]::Replace(
    $config,
    '(?m)^library_dir\s*=\s*".*"\s*$',
    "library_dir = `"$tomlPath`""
)
$config = [regex]::Replace(
    $config,
    '(?m)^default_mode\s*=\s*"(?:copy|reference)"\s*$',
    'default_mode = "reference"'
)
$config = [regex]::Replace(
    $config,
    '(?m)^active_library_label\s*=\s*".*"\s*$',
    'active_library_label = "en_nonfiction (reference index)"'
)
$config = [regex]::Replace(
    $config,
    '(?m)^sqlite_path\s*=\s*".*"\s*$',
    'sqlite_path = "./.cache/caliberate/data/en-nonfiction-dev.sqlite"'
)

$devConfigParent = Split-Path -Parent $DevConfigPath
if ($devConfigParent) {
    New-Item -ItemType Directory -Force -Path $devConfigParent | Out-Null
}
Set-Content -LiteralPath $DevConfigPath -Value $config -Encoding UTF8
Write-Host "Local dev config written to $DevConfigPath"

if ($Reset) {
    $databaseFiles = @(
        $devDatabasePath,
        "$devDatabasePath-wal",
        "$devDatabasePath-shm"
    )
    foreach ($databaseFile in $databaseFiles) {
        if (Test-Path -LiteralPath $databaseFile) {
            Remove-Item -LiteralPath $databaseFile -Force
            Write-Host "Removed dedicated dev database artifact: $databaseFile"
        }
    }
    Write-Host 'Dedicated dev index reset; source ebooks and the normal database were not modified.'
}

Write-Host 'Building calibredb...'
& cargo build -p caliberate-app --bin calibredb
if ($LASTEXITCODE -ne 0) {
    throw "cargo build failed with exit code $LASTEXITCODE"
}

$calibredb = Join-Path (Get-Location) 'target\debug\calibredb.exe'
if (-not (Test-Path -LiteralPath $calibredb)) {
    throw "calibredb binary not found after build: $calibredb"
}

$supported = @('.epub', '.mobi', '.azw', '.azw3', '.pdf', '.docx')
$files = Get-ChildItem -LiteralPath $SourceRoot -Recurse -File |
    Where-Object { $supported -contains $_.Extension.ToLowerInvariant() } |
    Sort-Object FullName

$totalFound = @($files).Count
if (-not $All -and $Limit -gt 0) {
    $files = @($files | Select-Object -First $Limit)
} else {
    $files = @($files)
}

Write-Host "Found $totalFound supported ebook files. Indexing $($files.Count)."
Write-Host 'Source files remain in place; ingest mode is reference.'

$addedOrSkipped = 0
$failed = 0
foreach ($file in $files) {
    Write-Host "[$($addedOrSkipped + $failed + 1)/$($files.Count)] $($file.FullName)"
    $addOutput = & $calibredb --config $DevConfigPath add --path $file.FullName --mode reference 2>&1
    if ($LASTEXITCODE -eq 0) {
        $addedOrSkipped++
        $addedId = ($addOutput | Select-String -Pattern '^Added book (\d+)$').Matches.Groups[1].Value
        if ($addedId) {
            $metadata = Parse-CalibreFileName $file.BaseName
            & $calibredb --config $DevConfigPath set title --id $addedId --title $metadata.Title
            if ($LASTEXITCODE -ne 0) { throw "Failed to set title for $($file.FullName)" }
            if ($metadata.Authors.Count -gt 0) {
                & $calibredb --config $DevConfigPath set authors --id $addedId --value $metadata.Authors
                if ($LASTEXITCODE -ne 0) { throw "Failed to set authors for $($file.FullName)" }
            }
        }
    } else {
        $failed++
        Write-Warning "Failed to index: $($file.FullName)"
    }
}

Write-Host ''
Write-Host "Index complete: $addedOrSkipped successful/skipped, $failed failed."
Write-Host 'Launch the GUI with:'
Write-Host "  cargo run -p caliberate-app --bin caliberate-gui -- --config $DevConfigPath"

if ($failed -gt 0) {
    exit 1
}

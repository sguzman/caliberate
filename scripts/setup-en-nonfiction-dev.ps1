param(
    [int]$Limit = 100,
    [switch]$All,
    [string]$SourceRoot = '\\wsl$\Ubuntu\mnt\wsl\PHYSICALDRIVE0p1\calibre\en_nonfiction',
    [string]$ConfigPath = 'config/control-plane.toml'
)

$ErrorActionPreference = 'Stop'

if (-not (Test-Path -LiteralPath $SourceRoot)) {
    throw "Library source does not exist or is not reachable: $SourceRoot"
}

if (-not (Test-Path -LiteralPath $ConfigPath)) {
    throw "Config file not found: $ConfigPath"
}

Write-Host "Configuring Caliberate for reference-backed development library:"
Write-Host "  $SourceRoot"

$config = Get-Content -LiteralPath $ConfigPath -Raw
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

Set-Content -LiteralPath $ConfigPath -Value $config -Encoding UTF8

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
    & $calibredb --config $ConfigPath add --path $file.FullName --mode reference
    if ($LASTEXITCODE -eq 0) {
        $addedOrSkipped++
    } else {
        $failed++
        Write-Warning "Failed to index: $($file.FullName)"
    }
}

Write-Host ''
Write-Host "Index complete: $addedOrSkipped successful/skipped, $failed failed."
Write-Host 'Launch the GUI with:'
Write-Host '  cargo run -p caliberate-app --bin caliberate-gui'

if ($failed -gt 0) {
    exit 1
}

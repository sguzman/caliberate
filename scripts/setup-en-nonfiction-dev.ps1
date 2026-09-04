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
$devDatabaseTomlPath = './.cache/caliberate/data/en-nonfiction-dev.sqlite'

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

function New-AuthorArguments {
    param([Parameter(Mandatory)][string[]]$Authors)

    $arguments = @()
    foreach ($author in $Authors) {
        $arguments += @('--value', $author)
    }
    return $arguments
}

function Convert-DevStartupConfig {
    param([Parameter(Mandatory)][string]$Config)

    $Config = [regex]::Replace(
        $Config,
        '(?m)^startup_open_last_library\s*=\s*(?:true|false)\s*$',
        'startup_open_last_library = false'
    )
    return [regex]::Replace(
        $Config,
        '(?m)^recent_libraries\s*=\s*\[.*\]\s*$',
        "recent_libraries = [`"$devDatabaseTomlPath`"]"
    )
}

function Find-SupportedEbookFiles {
    param(
        [Parameter(Mandatory)][string]$Root,
        [Parameter(Mandatory)][string[]]$SupportedExtensions,
        [int]$Limit = 0,
        [switch]$All
    )

    $bounded = -not $All -and $Limit -gt 0
    $pendingDirectories = New-Object 'System.Collections.Generic.Stack[string]'
    $pendingDirectories.Push((Get-Item -LiteralPath $Root -Force).FullName)
    $selected = New-Object 'System.Collections.Generic.List[object]'

    while ($pendingDirectories.Count -gt 0) {
        $directory = $pendingDirectories.Pop()
        $entries = @(Get-ChildItem -LiteralPath $directory -Force -ErrorAction Stop | Sort-Object FullName)
        $subdirectories = @($entries | Where-Object { $_.PSIsContainer } | Sort-Object FullName)

        foreach ($entry in $entries) {
            if (-not $entry.PSIsContainer -and
                $SupportedExtensions -contains $entry.Extension.ToLowerInvariant()) {
                $selected.Add($entry)
                if ($bounded -and $selected.Count -ge $Limit) {
                    return $selected.ToArray()
                }
            }
        }

        for ($index = $subdirectories.Count - 1; $index -ge 0; $index--) {
            $pendingDirectories.Push($subdirectories[$index].FullName)
        }
    }

    if ($All -or -not $bounded) {
        return @($selected | Sort-Object FullName)
    }
    return $selected.ToArray()
}

function Assert-Contains {
    param([string]$Text, [string]$Expected, [string]$Message)
    if (-not $Text.Contains($Expected)) {
        throw "Self-test failed: $Message (missing '$Expected')"
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

    $authorArguments = @(New-AuthorArguments $parsed.Authors)
    Assert-Equal @('--value', 'Jane Doe', '--value', 'John Roe', '--value', 'Sam Lee') `
        $authorArguments 'repeated author value flags'

    $startupConfig = Convert-DevStartupConfig @'
startup_open_last_library = true
sqlite_path = "./normal.db"
recent_libraries = ["./normal.db"]
'@
    Assert-Contains $startupConfig 'startup_open_last_library = false' 'startup override disabled'
    Assert-Contains $startupConfig 'recent_libraries = ["./.cache/caliberate/data/en-nonfiction-dev.sqlite"]' `
        'recent library points to dedicated dev database'

    $selfTestRoot = Join-Path ([System.IO.Path]::GetTempPath()) "caliberate-discovery-$([guid]::NewGuid())"
    try {
        $nestedRoot = Join-Path $selfTestRoot 'nested'
        New-Item -ItemType Directory -Path $nestedRoot -Force | Out-Null
        $sourceFiles = @(
            (Join-Path $selfTestRoot 'a-first.epub'),
            (Join-Path $selfTestRoot 'ignored.txt'),
            (Join-Path $nestedRoot 'b-second.pdf'),
            (Join-Path $nestedRoot 'c-third.docx'),
            (Join-Path $nestedRoot 'd-fourth.mobi')
        )
        foreach ($sourceFile in $sourceFiles) {
            Set-Content -LiteralPath $sourceFile -Value "self-test content: $sourceFile" -Encoding UTF8
        }

        $supported = @('.epub', '.mobi', '.azw', '.azw3', '.pdf', '.docx')
        $beforeHashes = @{}
        foreach ($sourceFile in Get-ChildItem -LiteralPath $selfTestRoot -Recurse -File) {
            $beforeHashes[$sourceFile.FullName] = (Get-FileHash -LiteralPath $sourceFile.FullName -Algorithm SHA256).Hash
        }

        $boundedFiles = @(Find-SupportedEbookFiles -Root $selfTestRoot -SupportedExtensions $supported -Limit 2)
        Assert-Equal 2 $boundedFiles.Count 'bounded discovery limit'
        if (@($boundedFiles | Where-Object { $supported -notcontains $_.Extension.ToLowerInvariant() }).Count -ne 0) {
            throw 'Self-test failed: bounded discovery returned an unsupported file'
        }
        if (@($boundedFiles | Where-Object { $_.FullName -like "*$([IO.Path]::DirectorySeparatorChar)nested$([IO.Path]::DirectorySeparatorChar)*" }).Count -eq 0) {
            throw 'Self-test failed: bounded discovery did not traverse nested directories'
        }

        $allFiles = @(Find-SupportedEbookFiles -Root $selfTestRoot -SupportedExtensions $supported -All)
        Assert-Equal 4 $allFiles.Count 'unbounded discovery count'
        foreach ($sourceFile in Get-ChildItem -LiteralPath $selfTestRoot -Recurse -File) {
            $afterHash = (Get-FileHash -LiteralPath $sourceFile.FullName -Algorithm SHA256).Hash
            Assert-Equal $beforeHashes[$sourceFile.FullName] $afterHash "discovery modified $($sourceFile.FullName)"
        }
        Write-Host 'Bounded discovery self-tests passed.'
    } finally {
        if (Test-Path -LiteralPath $selfTestRoot) {
            Remove-Item -LiteralPath $selfTestRoot -Recurse -Force
        }
    }
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
    "sqlite_path = `"$devDatabaseTomlPath`""
)
$config = Convert-DevStartupConfig $config

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
if (-not $All -and $Limit -gt 0) {
    Write-Host "Discovering up to $Limit supported ebook files..."
} else {
    Write-Host 'Scanning the full source tree for supported ebook files...'
}
$files = @(Find-SupportedEbookFiles -Root $SourceRoot -SupportedExtensions $supported -Limit $Limit -All:$All)

if (-not $All -and $Limit -gt 0) {
    Write-Host "Discovery selected $($files.Count) supported ebook files."
} else {
    Write-Host "Discovery found $($files.Count) supported ebook files."
}
Write-Host "Indexing $($files.Count) files."
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
                $authorArguments = @(New-AuthorArguments $metadata.Authors)
                & $calibredb --config $DevConfigPath set authors --id $addedId @authorArguments
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

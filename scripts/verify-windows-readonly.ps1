[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$InstallerPath,

    [Parameter(Mandatory = $true)]
    [string]$RepositoryRoot
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if (-not [System.OperatingSystem]::IsWindows()) {
    throw "The P1-B installer evidence script must run on Windows."
}

function Write-Evidence {
    param(
        [Parameter(Mandatory = $true)][string]$Stage,
        [Parameter(Mandatory = $true)][string]$Status,
        [Parameter(Mandatory = $true)][object]$Data
    )

    $record = [ordered]@{
        contract = "jzmatrix.windows-readonly-evidence"
        version = "1.0.0"
        stage = $Stage
        status = $Status
        data = $Data
    }
    Write-Host ("JZMATRIX_EVIDENCE " + ($record | ConvertTo-Json -Depth 20 -Compress))
}

function Assert-Condition {
    param(
        [Parameter(Mandatory = $true)][bool]$Condition,
        [Parameter(Mandatory = $true)][string]$Message
    )

    if (-not $Condition) {
        throw $Message
    }
}

function Get-OptionalPropertyValue {
    param(
        [AllowNull()][object]$Object,
        [Parameter(Mandatory = $true)][string]$Name
    )

    if ($null -eq $Object) {
        return $null
    }
    $property = $Object.PSObject.Properties[$Name]
    if ($null -eq $property) {
        return $null
    }
    return $property.Value
}

function Get-ProductRegistryEntries {
    $roots = @(
        "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall",
        "HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall",
        "HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"
    )
    $entries = @()
    foreach ($root in $roots) {
        if (-not (Test-Path -LiteralPath $root)) {
            continue
        }
        foreach ($key in Get-ChildItem -LiteralPath $root -ErrorAction SilentlyContinue) {
            $properties = Get-ItemProperty -LiteralPath $key.PSPath -ErrorAction SilentlyContinue
            $displayName = Get-OptionalPropertyValue -Object $properties -Name "DisplayName"
            if ($displayName -eq "JZMatrix Workbench") {
                $entries += [pscustomobject]@{
                    registry_path = $key.Name
                    display_name = $displayName
                    display_version = Get-OptionalPropertyValue -Object $properties -Name "DisplayVersion"
                    install_location = Get-OptionalPropertyValue -Object $properties -Name "InstallLocation"
                    uninstall_string = Get-OptionalPropertyValue -Object $properties -Name "UninstallString"
                }
            }
        }
    }
    return @($entries)
}

function Remove-TaskRegistryEntries {
    param([Parameter(Mandatory = $true)][string]$SandboxRoot)

    $roots = @(
        "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall",
        "HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall",
        "HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"
    )
    foreach ($root in $roots) {
        if (-not (Test-Path -LiteralPath $root)) {
            continue
        }
        foreach ($key in Get-ChildItem -LiteralPath $root -ErrorAction SilentlyContinue) {
            $properties = Get-ItemProperty -LiteralPath $key.PSPath -ErrorAction SilentlyContinue
            $displayName = Get-OptionalPropertyValue -Object $properties -Name "DisplayName"
            $installLocation = Get-OptionalPropertyValue -Object $properties -Name "InstallLocation"
            $uninstallString = Get-OptionalPropertyValue -Object $properties -Name "UninstallString"
            $scope = "$installLocation $uninstallString"
            if ($displayName -eq "JZMatrix Workbench" -and
                $scope.IndexOf($SandboxRoot, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) {
                Remove-Item -LiteralPath $key.PSPath -Recurse -Force
            }
        }
    }
}

function Get-InstalledProductProcesses {
    param([Parameter(Mandatory = $true)][string]$InstallRoot)

    $processes = @()
    foreach ($process in Get-CimInstance Win32_Process -ErrorAction Stop) {
        if ($null -ne $process.ExecutablePath -and
            $process.ExecutablePath.StartsWith($InstallRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
            $processes += [pscustomobject]@{
                process_id = [int]$process.ProcessId
                name = $process.Name
            }
        }
    }
    return @($processes)
}

function Get-ProductSnapshot {
    param(
        [Parameter(Mandatory = $true)][string]$InstallRoot,
        [Parameter(Mandatory = $true)][string[]]$ProductDataRoots
    )

    return [ordered]@{
        install_root_exists = Test-Path -LiteralPath $InstallRoot
        product_data = @($ProductDataRoots | ForEach-Object {
            [pscustomobject]@{
                path = $_
                exists = Test-Path -LiteralPath $_
            }
        })
        registry_entries = @(Get-ProductRegistryEntries)
        product_processes = @(Get-InstalledProductProcesses -InstallRoot $InstallRoot)
    }
}

function Get-WebView2State {
    $roots = @(
        (Join-Path ${env:ProgramFiles(x86)} "Microsoft\EdgeWebView\Application"),
        (Join-Path $env:ProgramFiles "Microsoft\EdgeWebView\Application")
    ) | Select-Object -Unique
    $versions = @()
    foreach ($root in $roots) {
        if (Test-Path -LiteralPath $root) {
            $versions += @(Get-ChildItem -LiteralPath $root -Directory -ErrorAction SilentlyContinue |
                Where-Object Name -Match "^\d+\." |
                ForEach-Object Name)
        }
    }
    return [ordered]@{
        detected = $versions.Count -gt 0
        versions = @($versions | Sort-Object -Unique)
        installer_strategy = "embedded_offline_installer"
    }
}

function Invoke-JsonCli {
    param(
        [Parameter(Mandatory = $true)][string]$CliPath,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [Parameter(Mandatory = $true)][string]$Stage,
        [Parameter(Mandatory = $true)][string]$EvidenceRoot
    )

    $stdoutPath = Join-Path $EvidenceRoot "$Stage.stdout.json"
    $stderrPath = Join-Path $EvidenceRoot "$Stage.stderr.txt"
    $process = Start-Process -FilePath $CliPath -ArgumentList $Arguments -Wait -PassThru `
        -RedirectStandardOutput $stdoutPath -RedirectStandardError $stderrPath
    $raw = if (Test-Path -LiteralPath $stdoutPath) {
        Get-Content -LiteralPath $stdoutPath -Raw
    } else {
        ""
    }
    $stderr = if (Test-Path -LiteralPath $stderrPath) {
        Get-Content -LiteralPath $stderrPath -Raw
    } else {
        ""
    }
    Write-Host "JZMATRIX_RAW_STDOUT $Stage $($raw.Trim())"
    if (-not [string]::IsNullOrWhiteSpace($stderr)) {
        Write-Host "JZMATRIX_RAW_STDERR $Stage $($stderr.Trim())"
    }
    Write-Evidence -Stage $Stage -Status $(if ($process.ExitCode -eq 0) { "pass" } else { "blocked" }) -Data ([ordered]@{
        exit_code = $process.ExitCode
        stdout_lines = @($raw.Trim() -split "`r?`n").Count
        stderr_empty = [string]::IsNullOrWhiteSpace($stderr)
    })
    Assert-Condition ($process.ExitCode -eq 0) "$Stage exited with code $($process.ExitCode)."
    Assert-Condition (-not [string]::IsNullOrWhiteSpace($raw)) "$Stage returned empty stdout."
    Assert-Condition (@($raw.Trim() -split "`r?`n").Count -eq 1) "$Stage must return exactly one JSON line."
    return ($raw | ConvertFrom-Json)
}

function Get-ProcessTreeIds {
    param([Parameter(Mandatory = $true)][int]$RootProcessId)

    $all = @(Get-CimInstance Win32_Process -ErrorAction Stop)
    $ids = [System.Collections.Generic.HashSet[int]]::new()
    [void]$ids.Add($RootProcessId)
    $changed = $true
    while ($changed) {
        $changed = $false
        foreach ($process in $all) {
            if ($ids.Contains([int]$process.ParentProcessId) -and -not $ids.Contains([int]$process.ProcessId)) {
                [void]$ids.Add([int]$process.ProcessId)
                $changed = $true
            }
        }
    }
    return @($ids | ForEach-Object { [int]$_ })
}

function Stop-ExactProcesses {
    param([int[]]$ProcessIds)

    foreach ($processId in @($ProcessIds | Sort-Object -Descending -Unique)) {
        Stop-Process -Id $processId -Force -ErrorAction SilentlyContinue
    }
}

function Remove-SafeTaskPath {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$AllowedParent
    )

    $fullPath = [System.IO.Path]::GetFullPath($Path)
    $fullParent = [System.IO.Path]::GetFullPath($AllowedParent).TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
    Assert-Condition ($fullPath.StartsWith($fullParent, [System.StringComparison]::OrdinalIgnoreCase)) "Cleanup target is outside the task root."
    if (Test-Path -LiteralPath $fullPath) {
        Remove-Item -LiteralPath $fullPath -Recurse -Force
    }
}

$resolvedInstaller = (Resolve-Path -LiteralPath $InstallerPath).Path
$resolvedRepository = (Resolve-Path -LiteralPath $RepositoryRoot).Path
$resolvedRunnerTemp = (Resolve-Path -LiteralPath $env:RUNNER_TEMP).Path
$sandboxRoot = Join-Path $resolvedRunnerTemp ("jzmatrix-p1b-" + [guid]::NewGuid().ToString("N"))
$installRoot = Join-Path $sandboxRoot "installed-product"
$evidenceRoot = Join-Path $sandboxRoot "evidence"
$emptyProjectRoot = Join-Path $sandboxRoot "empty-project"
$isolatedRoamingRoot = Join-Path $sandboxRoot "roaming-app-data"
$isolatedLocalRoot = Join-Path $sandboxRoot "local-app-data"
$isolatedTempRoot = Join-Path $sandboxRoot "runtime-temp"
$actualRoamingRoot = [Environment]::GetFolderPath([Environment+SpecialFolder]::ApplicationData)
$productDataRoots = @(
    (Join-Path $actualRoamingRoot "com.jiezijiuwei.jzmatrix"),
    (Join-Path $actualRoamingRoot "JIEZIJIUWEI\JZMatrix Workbench")
)
$originalEnvironment = [ordered]@{
    PATH = $env:PATH
    APPDATA = $env:APPDATA
    LOCALAPPDATA = $env:LOCALAPPDATA
    TEMP = $env:TEMP
    TMP = $env:TMP
    CARGO_HOME = $env:CARGO_HOME
    RUSTUP_HOME = $env:RUSTUP_HOME
    NPM_CONFIG_CACHE = $env:NPM_CONFIG_CACHE
}
$desktopProcessIds = @()
$uninstallerPath = $null
$failureMessage = $null

New-Item -ItemType Directory -Path $evidenceRoot, $emptyProjectRoot, $isolatedRoamingRoot, $isolatedLocalRoot, $isolatedTempRoot -Force | Out-Null
$env:APPDATA = $isolatedRoamingRoot
$env:LOCALAPPDATA = $isolatedLocalRoot
$env:TEMP = $isolatedTempRoot
$env:TMP = $isolatedTempRoot

try {
    Assert-Condition ($resolvedInstaller.StartsWith($resolvedRepository, [System.StringComparison]::OrdinalIgnoreCase)) "Installer must come from this repository build."
    Assert-Condition ((Get-Item -LiteralPath $resolvedInstaller).Length -gt 0) "Installer is empty."

    $beforeSnapshot = Get-ProductSnapshot -InstallRoot $installRoot -ProductDataRoots $productDataRoots
    Write-Evidence -Stage "before_install" -Status "pass" -Data ([ordered]@{
        snapshot = $beforeSnapshot
        webview2 = Get-WebView2State
        sandbox_root = $sandboxRoot
        installer_sha256 = (Get-FileHash -LiteralPath $resolvedInstaller -Algorithm SHA256).Hash.ToLowerInvariant()
    })
    Assert-Condition (-not $beforeSnapshot.install_root_exists) "Install root must be absent before installation."
    Assert-Condition ($beforeSnapshot.registry_entries.Count -eq 0) "A pre-existing JZMatrix installation would make this evidence ambiguous."
    Assert-Condition (@($beforeSnapshot.product_data | Where-Object exists).Count -eq 0) "Pre-existing product data would make this evidence ambiguous."

    $installProcess = Start-Process -FilePath $resolvedInstaller -ArgumentList @("/S", "/D=$installRoot") -Wait -PassThru
    Write-Evidence -Stage "nsis_install" -Status $(if ($installProcess.ExitCode -eq 0) { "pass" } else { "blocked" }) -Data ([ordered]@{
        exit_code = $installProcess.ExitCode
        install_root = $installRoot
        install_root_exists = Test-Path -LiteralPath $installRoot
    })
    Assert-Condition ($installProcess.ExitCode -eq 0) "NSIS installation failed."
    Assert-Condition (Test-Path -LiteralPath $installRoot) "NSIS did not create the requested install root."

    $installedExecutables = @(Get-ChildItem -LiteralPath $installRoot -File -Recurse -Filter *.exe)
    Write-Evidence -Stage "installed_executable_inventory" -Status "pass" -Data ([ordered]@{
        executables = @($installedExecutables | ForEach-Object {
            [System.IO.Path]::GetRelativePath($installRoot, $_.FullName)
        })
    })
    $desktopCandidates = @($installedExecutables | Where-Object Name -eq "jzmatrix-desktop.exe")
    $cliCandidates = @($installedExecutables | Where-Object Name -eq "jzmatrix.exe")
    Assert-Condition ($desktopCandidates.Count -eq 1) "Expected exactly one installed desktop executable."
    Assert-Condition ($cliCandidates.Count -eq 1) "Expected exactly one bundled jzmatrix CLI."
    $desktopPath = $desktopCandidates[0].FullName
    $cliPath = $cliCandidates[0].FullName

    $requiredResourceSuffixes = @(
        "fixtures\offline-demo\manifest.json",
        "fixtures\offline-demo\offline-demo.json",
        "fixtures\platform-events\manifest.json",
        "fixtures\platform-events\codex-cli-v1.json",
        "fixtures\platform-events\claude-code-v1.json"
    )
    $installedFiles = @(Get-ChildItem -LiteralPath $installRoot -File -Recurse)
    $resourcePaths = @{}
    foreach ($suffix in $requiredResourceSuffixes) {
        $matches = @($installedFiles | Where-Object FullName -Like "*$suffix")
        Assert-Condition ($matches.Count -eq 1) "Missing or ambiguous packaged resource: $suffix"
        $resourcePaths[$suffix] = $matches[0].FullName
    }
    Write-Evidence -Stage "packaged_resources" -Status "pass" -Data ([ordered]@{
        cli_relative_path = [System.IO.Path]::GetRelativePath($installRoot, $cliPath)
        resources = @($requiredResourceSuffixes | ForEach-Object {
            [pscustomobject]@{
                relative_path = [System.IO.Path]::GetRelativePath($installRoot, $resourcePaths[$_])
                sha256 = (Get-FileHash -LiteralPath $resourcePaths[$_] -Algorithm SHA256).Hash.ToLowerInvariant()
            }
        })
    })

    $runtimePath = @(
        (Join-Path $env:SystemRoot "System32"),
        $env:SystemRoot,
        (Join-Path $env:SystemRoot "System32\WindowsPowerShell\v1.0")
    ) -join ";"
    $env:PATH = $runtimePath
    $env:CARGO_HOME = Join-Path $sandboxRoot "absent-cargo-cache"
    $env:RUSTUP_HOME = Join-Path $sandboxRoot "absent-rustup-cache"
    $env:NPM_CONFIG_CACHE = Join-Path $sandboxRoot "absent-npm-cache"
    $runtimeDependencies = [ordered]@{
        node_available = $null -ne (Get-Command node -ErrorAction SilentlyContinue)
        npm_available = $null -ne (Get-Command npm -ErrorAction SilentlyContinue)
        cargo_available = $null -ne (Get-Command cargo -ErrorAction SilentlyContinue)
        rustc_available = $null -ne (Get-Command rustc -ErrorAction SilentlyContinue)
        cargo_cache_exists = Test-Path -LiteralPath $env:CARGO_HOME
        rustup_cache_exists = Test-Path -LiteralPath $env:RUSTUP_HOME
        npm_cache_exists = Test-Path -LiteralPath $env:NPM_CONFIG_CACHE
    }
    Write-Evidence -Stage "clean_runtime_environment" -Status "pass" -Data $runtimeDependencies
    Assert-Condition (-not $runtimeDependencies.node_available) "Node must be absent from the packaged runtime PATH."
    Assert-Condition (-not $runtimeDependencies.npm_available) "npm must be absent from the packaged runtime PATH."
    Assert-Condition (-not $runtimeDependencies.cargo_available) "Cargo must be absent from the packaged runtime PATH."
    Assert-Condition (-not $runtimeDependencies.rustc_available) "rustc must be absent from the packaged runtime PATH."
    Assert-Condition (-not $runtimeDependencies.cargo_cache_exists) "Cargo cache must be absent from the packaged runtime environment."
    Assert-Condition (-not $runtimeDependencies.rustup_cache_exists) "rustup cache must be absent from the packaged runtime environment."
    Assert-Condition (-not $runtimeDependencies.npm_cache_exists) "npm cache must be absent from the packaged runtime environment."

    Push-Location $emptyProjectRoot
    try {
        $doctor = Invoke-JsonCli -CliPath $cliPath -Arguments @("doctor", "--ephemeral", "--json") -Stage "doctor" -EvidenceRoot $evidenceRoot
        Assert-Condition ($doctor.contract -eq "jzmatrix.cli-response") "Doctor returned the wrong contract."
        Assert-Condition ($doctor.status -eq "pass") "Doctor did not pass."
        Assert-Condition ($doctor.extensions.database.integrity -eq "ok") "SQLite integrity was not verified."
        Assert-Condition ([int]$doctor.extensions.database.foreign_key_violations -eq 0) "SQLite foreign-key violations were found."
        Assert-Condition ([int]$doctor.extensions.database.migration_count -eq 1) "SQLite migration count is unexpected."
        Assert-Condition ($doctor.extensions.storage.cleanup_succeeded -eq $true) "Ephemeral doctor database cleanup failed."
        Assert-Condition (@($doctor.extensions.database.tables).Count -eq 3) "SQLite schema table count is unexpected."

        $offlineDemo = Invoke-JsonCli -CliPath $cliPath -Arguments @("offline-demo", "--json") -Stage "offline_demo" -EvidenceRoot $evidenceRoot
        Assert-Condition ($offlineDemo.contract -eq "jzmatrix.cli-response") "Offline demo returned the wrong contract."
        Assert-Condition ($offlineDemo.status -eq "pass") "Offline demo did not pass."
        Assert-Condition ($offlineDemo.outcome -eq "not_committed") "Offline demo must remain read-only."
        Assert-Condition ($offlineDemo.data.data_source -eq "demo") "Offline demo data source is not demo."
        Assert-Condition ($offlineDemo.data.groups[0].events[1].status -eq "not_run") "Offline demo must not imply an external send."

        $platformManifestPath = $resourcePaths["fixtures\platform-events\manifest.json"]
        $platformManifest = Get-Content -LiteralPath $platformManifestPath -Raw | ConvertFrom-Json
        $manifestHash = (Get-FileHash -LiteralPath $platformManifestPath -Algorithm SHA256).Hash.ToLowerInvariant()
        $fixtureResults = @()
        foreach ($fixtureEntry in $platformManifest.fixtures) {
            $stage = "fixture_" + ($fixtureEntry.id -replace "[^a-zA-Z0-9_-]", "_")
            $fixture = Invoke-JsonCli -CliPath $cliPath -Arguments @("fixture", "inspect", "--id", $fixtureEntry.id, "--json") -Stage $stage -EvidenceRoot $evidenceRoot
            $fixtureResourcePath = $resourcePaths["fixtures\platform-events\$($fixtureEntry.file)"]
            $fixtureResourceHash = (Get-FileHash -LiteralPath $fixtureResourcePath -Algorithm SHA256).Hash.ToLowerInvariant()
            Assert-Condition ($fixture.contract -eq "jzmatrix.cli-response") "Fixture returned the wrong contract."
            Assert-Condition ($fixture.data.parse_status -eq "parsed") "Fixture did not parse."
            Assert-Condition ($fixture.data.manifest_sha256 -eq $manifestHash) "Fixture manifest digest does not match the packaged manifest."
            Assert-Condition ($fixture.data.fixture_sha256 -eq $fixtureEntry.sha256) "Fixture parser digest does not match the manifest entry."
            Assert-Condition ($fixtureResourceHash -eq $fixtureEntry.sha256) "Packaged fixture digest does not match the manifest entry."
            Assert-Condition ($fixture.data.facts.completed.state -eq "observed") "Completion evidence was not retained."
            Assert-Condition ($fixture.data.facts.sent_not_confirmed.state -eq "unknown") "Message state was fabricated from completion."
            Assert-Condition ($fixture.data.facts.delivered.state -eq "unknown") "Delivery state was fabricated from completion."
            Assert-Condition ($fixture.data.facts.accepted.state -eq "unknown") "Acceptance state was fabricated from completion."
            $fixtureResults += [pscustomobject]@{
                fixture_id = $fixtureEntry.id
                manifest_digest_match = $true
                parse_status = $fixture.data.parse_status
                completed = $fixture.data.facts.completed.state
                sent_not_confirmed = $fixture.data.facts.sent_not_confirmed.state
                delivered = $fixture.data.facts.delivered.state
                accepted = $fixture.data.facts.accepted.state
            }
            if ($fixtureEntry.id -eq "codex-cli-synthetic-v1") {
                Assert-Condition ($fixture.data.extensions.input_extensions.future_marker.preserved -eq $true) "Fixture extension was not retained."
            }
        }
        Assert-Condition ($fixtureResults.Count -eq 2) "Expected exactly two bundled platform fixtures."
        Write-Evidence -Stage "fixture_assertions" -Status "pass" -Data ([ordered]@{
            ntfs_root = $sandboxRoot
            manifest_sha256 = $manifestHash
            fixtures = $fixtureResults
            extensions_retained = $true
            state_facts_are_distinct = $true
        })
    } finally {
        Pop-Location
    }

    $desktopProcess = Start-Process -FilePath $desktopPath -PassThru
    Start-Sleep -Seconds 8
    $desktopProcess.Refresh()
    Assert-Condition (-not $desktopProcess.HasExited) "Installed desktop process exited before the startup check."
    $desktopProcessIds = @(Get-ProcessTreeIds -RootProcessId $desktopProcess.Id)
    Write-Evidence -Stage "desktop_start" -Status "pass" -Data ([ordered]@{
        exit_code = $null
        root_process_id = $desktopProcess.Id
        process_tree_ids = $desktopProcessIds
        executable_relative_path = [System.IO.Path]::GetRelativePath($installRoot, $desktopPath)
        webview_ui_assertion = "not_proven_on_hosted_runner"
    })

    Assert-Condition ($null -ne (Get-Command Get-NetTCPConnection -ErrorAction SilentlyContinue)) "Get-NetTCPConnection is unavailable."
    $activeStates = @("Established", "SynSent", "SynReceived")
    $connections = @(Get-NetTCPConnection -ErrorAction Stop | Where-Object {
        $desktopProcessIds -contains [int]$_.OwningProcess -and $activeStates -contains $_.State.ToString()
    })
    $externalConnections = @($connections | Where-Object {
        $_.RemoteAddress -notin @("127.0.0.1", "::1", "0.0.0.0", "::")
    })
    Write-Evidence -Stage "runtime_network_snapshot" -Status $(if ($externalConnections.Count -eq 0) { "pass" } else { "blocked" }) -Data ([ordered]@{
        process_tree_ids = $desktopProcessIds
        active_external_connection_count = $externalConnections.Count
        active_external_connections = @($externalConnections | ForEach-Object {
            [pscustomobject]@{
                owning_process = [int]$_.OwningProcess
                state = $_.State.ToString()
                remote_address = $_.RemoteAddress
                remote_port = $_.RemotePort
            }
        })
        capture_scope = "single_post_start_snapshot"
        continuous_capture_proven = $false
    })
    Assert-Condition ($externalConnections.Count -eq 0) "The installed product process tree had an active external connection."

    Stop-ExactProcesses -ProcessIds $desktopProcessIds
    Start-Sleep -Seconds 2
    $desktopProcessIds = @()

    $uninstallerCandidates = @(Get-ChildItem -LiteralPath $installRoot -File -Recurse | Where-Object Name -eq "uninstall.exe")
    Assert-Condition ($uninstallerCandidates.Count -eq 1) "Expected exactly one NSIS uninstaller."
    $uninstallerPath = $uninstallerCandidates[0].FullName
    $beforeUninstall = Get-ProductSnapshot -InstallRoot $installRoot -ProductDataRoots $productDataRoots
    Write-Evidence -Stage "before_uninstall" -Status "pass" -Data $beforeUninstall

    $uninstallProcess = Start-Process -FilePath $uninstallerPath -ArgumentList @("/S") -Wait -PassThru
    for ($attempt = 0; $attempt -lt 30 -and (Test-Path -LiteralPath $installRoot); $attempt++) {
        Start-Sleep -Milliseconds 500
    }
    Write-Evidence -Stage "nsis_uninstall" -Status $(if ($uninstallProcess.ExitCode -eq 0) { "pass" } else { "blocked" }) -Data ([ordered]@{
        exit_code = $uninstallProcess.ExitCode
        install_root_exists = Test-Path -LiteralPath $installRoot
    })
    Assert-Condition ($uninstallProcess.ExitCode -eq 0) "NSIS uninstallation failed."

    $rawAfterUninstall = Get-ProductSnapshot -InstallRoot $installRoot -ProductDataRoots $productDataRoots
    Write-Evidence -Stage "post_uninstall_raw" -Status "pass" -Data ([ordered]@{
        snapshot = $rawAfterUninstall
        application_data_policy = "exact product data is removed by the test cleanup after evidence capture"
    })
    Assert-Condition (-not $rawAfterUninstall.install_root_exists) "Install root remained after uninstall."
    Assert-Condition ($rawAfterUninstall.registry_entries.Count -eq 0) "Product uninstall registry entries remained."
    Assert-Condition ($rawAfterUninstall.product_processes.Count -eq 0) "Product processes remained after uninstall."

    foreach ($productDataRoot in $productDataRoots) {
        if (Test-Path -LiteralPath $productDataRoot) {
            Remove-SafeTaskPath -Path $productDataRoot -AllowedParent $actualRoamingRoot
        }
    }
    $afterUninstall = Get-ProductSnapshot -InstallRoot $installRoot -ProductDataRoots $productDataRoots
    Write-Evidence -Stage "after_uninstall" -Status "pass" -Data $afterUninstall
    Assert-Condition (-not $afterUninstall.install_root_exists) "Install root remained after uninstall."
    Assert-Condition ($afterUninstall.registry_entries.Count -eq 0) "Product uninstall registry entries remained."
    Assert-Condition ($afterUninstall.product_processes.Count -eq 0) "Product processes remained after uninstall."
    Assert-Condition (@($afterUninstall.product_data | Where-Object exists).Count -eq 0) "Product data remained after cleanup."
} catch {
    $failureMessage = $_.Exception.Message
    Write-Evidence -Stage "failure" -Status "blocked" -Data ([ordered]@{
        message = $failureMessage
        webview_ui_proven = $false
    })
} finally {
    if ($desktopProcessIds.Count -gt 0) {
        Stop-ExactProcesses -ProcessIds $desktopProcessIds
    }
    if ($null -ne $uninstallerPath -and (Test-Path -LiteralPath $uninstallerPath)) {
        Start-Process -FilePath $uninstallerPath -ArgumentList @("/S") -Wait -ErrorAction SilentlyContinue | Out-Null
    }
    Remove-TaskRegistryEntries -SandboxRoot $sandboxRoot
    foreach ($productDataRoot in $productDataRoots) {
        if (Test-Path -LiteralPath $productDataRoot) {
            Remove-SafeTaskPath -Path $productDataRoot -AllowedParent $actualRoamingRoot
        }
    }
    $env:PATH = $originalEnvironment.PATH
    $env:APPDATA = $originalEnvironment.APPDATA
    $env:LOCALAPPDATA = $originalEnvironment.LOCALAPPDATA
    $env:TEMP = $originalEnvironment.TEMP
    $env:TMP = $originalEnvironment.TMP
    $env:CARGO_HOME = $originalEnvironment.CARGO_HOME
    $env:RUSTUP_HOME = $originalEnvironment.RUSTUP_HOME
    $env:NPM_CONFIG_CACHE = $originalEnvironment.NPM_CONFIG_CACHE
    Remove-SafeTaskPath -Path $sandboxRoot -AllowedParent $resolvedRunnerTemp
    $cleanupSnapshot = [ordered]@{
        sandbox_root_exists = Test-Path -LiteralPath $sandboxRoot
        registry_entries = @(Get-ProductRegistryEntries)
        product_data = @($productDataRoots | ForEach-Object {
            [pscustomobject]@{ path = $_; exists = Test-Path -LiteralPath $_ }
        })
    }
    $cleanupOk = (-not $cleanupSnapshot.sandbox_root_exists) -and
        $cleanupSnapshot.registry_entries.Count -eq 0 -and
        @($cleanupSnapshot.product_data | Where-Object exists).Count -eq 0
    Write-Evidence -Stage "after_cleanup" -Status $(if ($cleanupOk) { "pass" } else { "blocked" }) -Data $cleanupSnapshot
    if (-not $cleanupOk -and $null -eq $failureMessage) {
        $failureMessage = "Final task cleanup left product residuals."
    }
}

if ($null -ne $failureMessage) {
    throw $failureMessage
}

Write-Evidence -Stage "summary" -Status "pass" -Data ([ordered]@{
    unsigned_nsis = $true
    formal_windows_support = $false
    real_agent_integration = $false
    product_runtime_dependency_downloads = $false
    webview_ui_proven = $false
    network_evidence = "single post-start process-tree snapshot only"
})

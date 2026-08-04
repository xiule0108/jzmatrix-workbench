[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet("job_start", "after_cli_resource_build", "after_rust_workspace", "after_installer_build")]
    [string]$Stage
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if (-not [System.OperatingSystem]::IsWindows()) {
    throw "The Windows profile boundary check must run on Windows."
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

$roamingRoot = [Environment]::GetFolderPath([Environment+SpecialFolder]::ApplicationData)
$productData = @(
    (Join-Path $roamingRoot "com.jiezijiuwei.jzmatrix"),
    (Join-Path $roamingRoot "JIEZIJIUWEI\JZMatrix Workbench")
) | ForEach-Object {
    [pscustomobject]@{
        path = $_
        exists = Test-Path -LiteralPath $_
    }
}

$registryEntries = @()
$registryRoots = @(
    "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall",
    "HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall",
    "HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"
)
foreach ($root in $registryRoots) {
    if (-not (Test-Path -LiteralPath $root)) {
        continue
    }
    foreach ($key in Get-ChildItem -LiteralPath $root -ErrorAction SilentlyContinue) {
        $properties = Get-ItemProperty -LiteralPath $key.PSPath -ErrorAction SilentlyContinue
        $displayName = Get-OptionalPropertyValue -Object $properties -Name "DisplayName"
        if ($displayName -eq "JZMatrix Workbench") {
            $registryEntries += [pscustomobject]@{
                registry_path = $key.Name
                display_name = $displayName
            }
        }
    }
}

$record = [ordered]@{
    contract = "jzmatrix.windows-profile-preinstall"
    version = "1.0.0"
    stage = $Stage
    status = if (@($productData | Where-Object exists).Count -eq 0 -and $registryEntries.Count -eq 0) {
        "pass"
    } else {
        "blocked"
    }
    data = [ordered]@{
        product_data = @($productData)
        registry_entries = @($registryEntries)
        mutation_performed = $false
    }
}
Write-Host ("JZMATRIX_PREINSTALL_EVIDENCE " + ($record | ConvertTo-Json -Depth 10 -Compress))

if ($record.status -ne "pass") {
    throw "Windows profile state is not clean at stage $Stage."
}

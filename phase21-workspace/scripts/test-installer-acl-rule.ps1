#Requires -Version 7.0
# Runs the installed-state verifier's own install-root ACL rule against a real temporary folder.
$ErrorActionPreference='Stop'
$source=Join-Path $PSScriptRoot 'verify-installer-security.ps1'
$ast=[Management.Automation.Language.Parser]::ParseFile($source,[ref]$null,[ref]$null)
foreach ($name in 'Sid-Of','Assert-ProtectedAcl') {
    $definition=$ast.Find({param($n) $n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -eq $name},$true)
    if (-not $definition) { throw "Verifier function missing: $name" }
    . ([scriptblock]::Create($definition.Extent.Text))
}
$folder=Join-Path ([IO.Path]::GetTempPath()) ('aethercore-acl-rule-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory $folder | Out-Null
function Set-InstallRootAcl([string]$UsersRights,[string]$ExtraAce='') {
    $acl=[Security.AccessControl.DirectorySecurity]::new()
    $acl.SetSecurityDescriptorSddlForm("D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;$UsersRights;;;BU)$ExtraAce",'Access')
    Set-Acl -LiteralPath $folder -AclObject $acl
}
try {
    Set-InstallRootAcl '0x1200a9'   # read and execute, as the installer grants Users
    Assert-ProtectedAcl $folder $false
    foreach ($rights in '0x1301bf','0x116') {   # modify; write only
        Set-InstallRootAcl $rights
        $rejected=$false
        try { Assert-ProtectedAcl $folder $false } catch { $rejected=$_.Exception.Message -match 'write/modify' }
        if (-not $rejected) { throw "Users rights $rights were not rejected on the install root." }
    }
    # RX on the root plus an inherit-only generic grant: every new file would inherit Users write.
    foreach ($generic in '(A;OICIIO;GW;;;BU)','(A;OICIIO;GA;;;BU)') {
        Set-InstallRootAcl '0x1200a9' $generic
        $rejected=$false
        try { Assert-ProtectedAcl $folder $false } catch { $rejected=$_.Exception.Message -match 'write/modify' }
        if (-not $rejected) { throw "Inherit-only generic grant $generic was not rejected on the install root." }
    }
} finally {
    Set-InstallRootAcl '0x1200a9'
    Remove-Item -LiteralPath $folder -Recurse -Force
}
Write-Output 'INSTALL_ROOT_ACL_PASS: Users RX accepted; Users modify, write and inherit-only GW/GA rejected.'

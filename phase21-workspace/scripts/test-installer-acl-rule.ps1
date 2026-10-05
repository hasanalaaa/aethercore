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
function Set-InstallRootAcl([string]$UsersRights) {
    $acl=[Security.AccessControl.DirectorySecurity]::new()
    $acl.SetSecurityDescriptorSddlForm("D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;$UsersRights;;;BU)",'Access')
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
} finally {
    Set-InstallRootAcl '0x1200a9'
    Remove-Item -LiteralPath $folder -Recurse -Force
}
Write-Output 'INSTALL_ROOT_ACL_PASS: Users RX accepted; Users modify and write rejected.'

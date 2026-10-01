#Requires -Version 7.0
# Native fixture only: loopback data, no prerequisite download or installation.
$ErrorActionPreference = 'Stop'
if ($env:OS -ne 'Windows_NT') { throw 'Run this fixture on Windows.' }
$Root = Split-Path $PSScriptRoot -Parent
$webScript = Join-Path $PSScriptRoot 'fetch-webview2.ps1'
$vcSource = Get-Content (Join-Path $PSScriptRoot 'build-release.ps1') -Raw
$webSource = Get-Content $webScript -Raw
foreach ($source in @($webSource,$vcSource)) {
    if ($source -notmatch 'curl\.exe.*--connect-timeout 30.*--max-time 120.*--retry 2.*--retry-max-time 360') { throw 'Prerequisite transfer has no complete native curl time bound.' }
    if ($source -notmatch 'Get-AuthenticodeSignature' -or $source -notmatch 'Microsoft Corporation') { throw 'Microsoft signature validation is required.' }
}
Add-Type @'
using System;
using System.Net;
using System.Net.Sockets;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
public sealed class PrerequisiteFixtureServer : IDisposable {
    readonly TcpListener listener = new TcpListener(IPAddress.Loopback, 0);
    readonly CancellationTokenSource stop = new CancellationTokenSource();
    public int Requests;
    public bool Stall;
    public int Port { get { return ((IPEndPoint)listener.LocalEndpoint).Port; } }
    public PrerequisiteFixtureServer(bool stall) {
        Stall = stall; listener.Start();
        Task.Run(() => {
            while (!stop.IsCancellationRequested) {
                TcpClient client;
                try { client = listener.AcceptTcpClient(); } catch { break; }
                Task.Run(() => {
                    using (client) {
                        var stream = client.GetStream();
                        var buffer = new byte[4096]; stream.Read(buffer,0,buffer.Length);
                        var n = Interlocked.Increment(ref Requests);
                        if (!Stall && n == 1) {
                            var fail = Encoding.ASCII.GetBytes("HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                            stream.Write(fail,0,fail.Length); return;
                        }
                        var header = Encoding.ASCII.GetBytes("HTTP/1.1 200 OK\r\nContent-Length: 500001\r\nConnection: close\r\n\r\n");
                        stream.Write(header,0,header.Length);
                        if (Stall) {
                            stream.WriteByte(0); stream.Flush();
                            stop.Token.WaitHandle.WaitOne();
                        } else {
                            var bytes = new byte[500001]; stream.Write(bytes,0,bytes.Length);
                        }
                    }
                });
            }
        });
    }
    public void Dispose() { stop.Cancel(); listener.Stop(); }
}
'@
$global:PrerequisiteFixtureCurl = (Get-Command curl.exe -CommandType Application).Source
$global:PrerequisiteFixtureUri = ''
$fixture = Join-Path ([IO.Path]::GetTempPath()) ('aethercore-prerequisite-' + [guid]::NewGuid())
New-Item -ItemType Directory $fixture | Out-Null
$previous = Get-Location
function global:curl.exe {
    $call = @($args)
    if ($call[-1] -notin @('https://go.microsoft.com/fwlink/p/?LinkId=2124703','https://aka.ms/vc14/vc_redist.x64.exe')) { throw 'Release URL changed.' }
    $call[-1] = $global:PrerequisiteFixtureUri
    $call[[array]::IndexOf($call,'--connect-timeout') + 1] = '1'
    $call[[array]::IndexOf($call,'--max-time') + 1] = '1'
    $call[[array]::IndexOf($call,'--retry-max-time') + 1] = '7'
    & $global:PrerequisiteFixtureCurl @call
    $global:LASTEXITCODE = $LASTEXITCODE
}
try {
    foreach ($kind in @('webview','vc')) {
        foreach ($stall in @($true,$false)) {
            $server = [PrerequisiteFixtureServer]::new($stall)
            $global:PrerequisiteFixtureUri = "http://127.0.0.1:$($server.Port)/fixture"
            $destination = Join-Path $fixture $(if ($kind -eq 'webview') { 'MicrosoftEdgeWebview2Setup.exe' } else { 'vc_redist.x64.exe' })
            $watch = [Diagnostics.Stopwatch]::StartNew()
            $failure = $null
            try {
                if ($kind -eq 'webview') {
                    & $webScript -OutputPath ([IO.Path]::GetRelativePath($Root,$destination))
                } else {
                    $Prereqs = $fixture
                    $start = $vcSource.IndexOf('$vcredist =')
                    $end = $vcSource.IndexOf('$msi =',$start)
                    & ([scriptblock]::Create($vcSource.Substring($start,$end-$start)))
                }
            } catch { $failure = $_.Exception.Message } finally { $server.Dispose() }
            if (-not $failure -or (Test-Path $destination)) { throw "$kind accepted an incomplete/unsigned fixture." }
            if ($stall) {
                if ($watch.Elapsed.TotalSeconds -gt 12 -or $server.Requests -ne 3 -or (Test-Path "$destination.download")) { throw "$kind failed timeout/retry/partial cleanup: $failure" }
            } elseif ($server.Requests -ne 2 -or $failure -notmatch 'signature') { throw "$kind did not retain Microsoft signature rejection after a successful retry: $failure" }
            Write-Host "PASS $kind stall=$stall requests=$($server.Requests) elapsed_ms=$($watch.ElapsedMilliseconds) failure=$failure"
        }
    }
} finally {
    Remove-Item Function:\curl.exe
    Remove-Variable PrerequisiteFixtureCurl,PrerequisiteFixtureUri -Scope Global
    Set-Location $previous
    Remove-Item $fixture -Recurse -Force
}

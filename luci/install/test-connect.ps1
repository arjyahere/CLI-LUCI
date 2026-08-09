# test-connect.ps1 — Run on Machine 2 to test connectivity to Machine 1
# Usage: .\test-connect.ps1 <machine1-ip>

param(
    [Parameter(Mandatory=$true)]
    [string]$TargetIP
)

$Port = 9528
$Target = "${TargetIP}:${Port}"

Write-Host "Testing connection to ${Target}..." -ForegroundColor Cyan

try {
    # Create TCP client
    $client = New-Object System.Net.Sockets.TcpClient
    $connectTask = $client.ConnectAsync($TargetIP, $Port)
    
    if ($connectTask.Wait(5000)) {
        if ($client.Connected) {
            Write-Host "SUCCESS: Connected to ${Target}" -ForegroundColor Green
            
            $stream = $client.GetStream()
            $stream.ReadTimeout = 5000
            $stream.WriteTimeout = 5000
            
            # Build a Ping message using luci-protocol format
            $message = @{
                id = [guid]::NewGuid().ToString()
                msg_type = "Ping"
                sender_id = "test-client"
                timestamp = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ss.fffZ")
                payload = @{ type = "Ping" }
            } | ConvertTo-Json -Compress
            
            $msgBytes = [System.Text.Encoding]::UTF8.GetBytes($message)
            $lenBytes = [BitConverter]::GetBytes([uint32]$msgBytes.Length)
            
            # Send length prefix + message
            $stream.Write($lenBytes, 0, 4)
            $stream.Write($msgBytes, 0, $msgBytes.Length)
            Write-Host "SENT: Ping message ($($msgBytes.Length) bytes)" -ForegroundColor Yellow
            
            # Read response
            $lenBuf = New-Object byte[] 4
            $stream.Read($lenBuf, 0, 4) | Out-Null
            $respLen = [BitConverter]::ToUInt32($lenBuf, 0)
            
            if ($respLen -gt 0 -and $respLen -lt 10485760) {
                $respBuf = New-Object byte[] $respLen
                $stream.Read($respBuf, 0, $respLen) | Out-Null
                $response = [System.Text.Encoding]::UTF8.GetString($respBuf)
                Write-Host "RECEIVED: $response" -ForegroundColor Green
                Write-Host ""
                Write-Host "Protocol test PASSED!" -ForegroundColor Green
            } else {
                Write-Host "WARNING: Unexpected response length: $respLen" -ForegroundColor Yellow
            }
            
            $client.Close()
        } else {
            Write-Host "FAILED: Could not connect" -ForegroundColor Red
        }
    } else {
        Write-Host "FAILED: Connection timed out (5s)" -ForegroundColor Red
        $client.Close()
    }
} catch {
    Write-Host "ERROR: $($_.Exception.Message)" -ForegroundColor Red
    if ($client.Connected) { $client.Close() }
}

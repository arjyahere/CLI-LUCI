# test-listen.ps1 — Run on Machine 1 as a simple test server
# Listens on port 9528 and responds to luci-protocol messages
# Usage: .\test-listen.ps1

param(
    [int]$Port = 9528
)

$listener = New-Object System.Net.Sockets.TcpListener([System.Net.IPAddress]::Any, $Port)
$listener.Start()

Write-Host "Listening on port $Port ..." -ForegroundColor Cyan
Write-Host "Waiting for connections..." -ForegroundColor Yellow
Write-Host "Press Ctrl+C to stop" -ForegroundColor Gray
Write-Host ""

try {
    while ($true) {
        $client = $listener.AcceptTcpClient()
        $endpoint = $client.Client.RemoteEndPoint.ToString()
        Write-Host "Connection from $endpoint" -ForegroundColor Green
        
        $stream = $client.GetStream()
        $stream.ReadTimeout = 10000
        
        try {
            # Read length prefix
            $lenBuf = New-Object byte[] 4
            $stream.Read($lenBuf, 0, 4) | Out-Null
            $msgLen = [BitConverter]::ToUInt32($lenBuf, 0)
            
            if ($msgLen -gt 0 -and $msgLen -lt 10485760) {
                # Read message
                $msgBuf = New-Object byte[] $msgLen
                $stream.Read($msgBuf, 0, $msgLen) | Out-Null
                $message = [System.Text.Encoding]::UTF8.GetString($msgBuf)
                
                Write-Host "  RECV: $message" -ForegroundColor Yellow
                
                # Parse and respond
                $parsed = $message | ConvertFrom-Json
                
                if ($parsed.msg_type -eq "Ping") {
                    $response = @{
                        id = [guid]::NewGuid().ToString()
                        msg_type = "Pong"
                        sender_id = "test-server"
                        timestamp = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ss.fffZ")
                        payload = @{
                            type = "Pong"
                            node_name = $env:COMPUTERNAME
                        }
                    } | ConvertTo-Json -Compress
                    
                    $respBytes = [System.Text.Encoding]::UTF8.GetBytes($response)
                    $respLen = [BitConverter]::GetBytes([uint32]$respBytes.Length)
                    
                    $stream.Write($respLen, 0, 4)
                    $stream.Write($respBytes, 0, $respBytes.Length)
                    
                    Write-Host "  SEND: Pong (node: $env:COMPUTERNAME)" -ForegroundColor Green
                } else {
                    Write-Host "  Unknown message type: $($parsed.msg_type)" -ForegroundColor Yellow
                }
            } else {
                Write-Host "  Invalid message length: $msgLen" -ForegroundColor Red
            }
        } catch {
            Write-Host "  Error: $($_.Exception.Message)" -ForegroundColor Red
        }
        
        $client.Close()
        Write-Host "  Connection closed" -ForegroundColor Gray
        Write-Host ""
    }
} finally {
    $listener.Stop()
    Write-Host "Server stopped" -ForegroundColor Cyan
}

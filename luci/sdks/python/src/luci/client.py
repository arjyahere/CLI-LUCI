"""
Luci Client - Interface to the Luci daemon.
"""

import json
import socket
import struct
from typing import Optional, Dict, Any, List


class LuciClient:
    """Client for communicating with the Luci daemon."""

    def __init__(self, host: str = "127.0.0.1", port: int = 9527):
        """
        Initialize the Luci client.

        Args:
            host: Daemon host (default: 127.0.0.1)
            port: Daemon port (default: 9527)
        """
        self.host = host
        self.port = port

    def _send_command(self, command: Dict[str, Any]) -> Dict[str, Any]:
        """Send a command to the daemon and return the response."""
        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        sock.settimeout(30)

        try:
            sock.connect((self.host, self.port))

            # Send length-prefixed request
            request = json.dumps({"command": command}).encode()
            sock.sendall(struct.pack("<I", len(request)) + request)

            # Read length-prefixed response
            length_data = self._recv_exact(sock, 4)
            length = struct.unpack("<I", length_data)[0]

            response_data = self._recv_exact(sock, length)
            response = json.loads(response_data)

            return response

        finally:
            sock.close()

    def _recv_exact(self, sock: socket.socket, n: int) -> bytes:
        """Receive exactly n bytes from socket."""
        data = b""
        while len(data) < n:
            chunk = sock.recv(n - len(data))
            if not chunk:
                raise ConnectionError("Connection closed")
            data += chunk
        return data

    def ping(self) -> bool:
        """Check if daemon is running."""
        try:
            response = self._send_command({"type": "Ping"})
            return response.get("type") == "Success"
        except (ConnectionRefusedError, socket.timeout):
            return False

    def status(self) -> Dict[str, Any]:
        """Get daemon status."""
        response = self._send_command({"type": "GetDaemonStatus"})
        if response.get("type") == "Success":
            return response.get("payload", {})
        raise RuntimeError(response.get("message", "Unknown error"))

    def create_network(self, name: str) -> Dict[str, str]:
        """
        Create a new network.

        Args:
            name: Network name

        Returns:
            Dict with network_id and invite_code
        """
        response = self._send_command({
            "type": "CreateNetwork",
            "name": name,
        })
        if response.get("type") == "Success":
            return response.get("payload", {})
        raise RuntimeError(response.get("message", "Unknown error"))

    def join_network(self, invite_code: str) -> bool:
        """
        Join an existing network.

        Args:
            invite_code: Invite code from network creator

        Returns:
            True if successful
        """
        response = self._send_command({
            "type": "JoinNetwork",
            "invite_code": invite_code,
        })
        return response.get("type") == "Success"

    def list_devices(self) -> List[Dict[str, Any]]:
        """List all devices in the network."""
        response = self._send_command({"type": "ListDevices"})
        if response.get("type") == "Success":
            return response.get("payload", {}).get("devices", [])
        raise RuntimeError(response.get("message", "Unknown error"))

    def resources(self) -> Dict[str, Any]:
        """Get combined resources."""
        response = self._send_command({"type": "GetResources"})
        if response.get("type") == "Success":
            return response.get("payload", {})
        raise RuntimeError(response.get("message", "Unknown error"))

    def run(
        self,
        script: str,
        args: Optional[List[str]] = None,
        ram: Optional[str] = None,
        cpu: Optional[int] = None,
        node: Optional[str] = None,
    ) -> str:
        """
        Run a script or command.

        Args:
            script: Script path or command
            args: Arguments to pass
            ram: Required RAM (e.g., "4GB")
            cpu: Required CPU cores
            node: Run on specific node

        Returns:
            Job ID
        """
        command = {
            "type": "RunJob",
            "name": script,
            "job_type": "custom",
            "args": args or [],
        }

        if ram:
            # Parse RAM string to bytes
            ram_bytes = self._parse_ram(ram)
            command["ram"] = ram_bytes

        if cpu:
            command["cpu"] = cpu

        if node:
            command["node"] = node

        response = self._send_command(command)
        if response.get("type") == "Success":
            return response.get("payload", {}).get("job_id", "")
        raise RuntimeError(response.get("message", "Unknown error"))

    def job_status(self, job_id: str) -> Dict[str, Any]:
        """
        Get job status.

        Args:
            job_id: Job ID

        Returns:
            Job status dict
        """
        response = self._send_command({
            "type": "GetJobStatus",
            "job_id": job_id,
        })
        if response.get("type") == "Success":
            return response.get("payload", {})
        raise RuntimeError(response.get("message", "Unknown error"))

    def list_jobs(self) -> List[Dict[str, Any]]:
        """List all jobs."""
        response = self._send_command({"type": "ListJobs"})
        if response.get("type") == "Success":
            return response.get("payload", {}).get("jobs", [])
        raise RuntimeError(response.get("message", "Unknown error"))

    def cancel_job(self, job_id: str) -> bool:
        """
        Cancel a job.

        Args:
            job_id: Job ID

        Returns:
            True if successful
        """
        response = self._send_command({
            "type": "CancelJob",
            "job_id": job_id,
        })
        return response.get("type") == "Success"

    def create_memory_pool(self, name: str, size_gb: float) -> Dict[str, Any]:
        """
        Create a virtual memory pool.

        Args:
            name: Pool name
            size_gb: Size in GB

        Returns:
            Pool info dict
        """
        response = self._send_command({
            "type": "CreateMemoryPool",
            "name": name,
            "size_gb": size_gb,
        })
        if response.get("type") == "Success":
            return response.get("payload", {})
        raise RuntimeError(response.get("message", "Unknown error"))

    def list_memory_pools(self) -> List[Dict[str, Any]]:
        """List memory pools."""
        response = self._send_command({"type": "ListMemoryPools"})
        if response.get("type") == "Success":
            return response.get("payload", {}).get("pools", [])
        raise RuntimeError(response.get("message", "Unknown error"))

    def _parse_ram(self, ram: str) -> int:
        """Parse RAM string to bytes."""
        ram = ram.upper().strip()
        if ram.endswith("GB"):
            return int(float(ram[:-2]) * 1024 * 1024 * 1024)
        elif ram.endswith("MB"):
            return int(float(ram[:-2]) * 1024 * 1024)
        elif ram.endswith("KB"):
            return int(float(ram[:-2]) * 1024)
        else:
            return int(ram)

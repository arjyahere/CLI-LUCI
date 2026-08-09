"""
Luci Memory - Distributed memory management.
"""

from typing import Optional, List, Dict, Any
import struct


class MemoryPool:
    """Represents a distributed memory pool."""

    def __init__(self, client, pool_id: str, name: str, size_gb: float):
        """
        Initialize a memory pool.

        Args:
            client: LuciClient instance
            pool_id: Pool ID
            name: Pool name
            size_gb: Size in GB
        """
        self._client = client
        self.id = pool_id
        self.name = name
        self.size_gb = size_gb

    @classmethod
    def create(cls, client, name: str, size_gb: float) -> "MemoryPool":
        """
        Create a new memory pool.

        Args:
            client: LuciClient instance
            name: Pool name
            size_gb: Size in GB

        Returns:
            MemoryPool instance
        """
        result = client.create_memory_pool(name, size_gb)
        return cls(
            client=client,
            pool_id=result.get("pool_id", ""),
            name=name,
            size_gb=result.get("total_size_gb", size_gb),
        )

    def info(self) -> dict:
        """Get pool information."""
        return self._client._send_command({
            "type": "GetMemoryPoolInfo",
            "pool_id": self.id,
        })

    def delete(self) -> bool:
        """Delete the memory pool."""
        response = self._client._send_command({
            "type": "DeleteMemoryPool",
            "pool_id": self.id,
        })
        return response.get("type") == "Success"

    def __repr__(self) -> str:
        return f"MemoryPool(id={self.id!r}, name={self.name!r}, size_gb={self.size_gb})"


class DistributedArray:
    """
    A distributed array that spans multiple devices.

    Data is split into chunks, each potentially owned by a different node.
    When local data is accessed, it's served directly.
    When remote data is accessed, it triggers a network fetch via IPC.
    """

    def __init__(self, client=None, shape: tuple = (0,), dtype: str = "float32"):
        """
        Initialize a distributed array.

        Args:
            client: LuciClient instance for remote access
            shape: Array shape
            dtype: Data type
        """
        self._client = client
        self.shape = shape
        self.dtype = dtype
        self._total_len = 1
        for s in shape:
            self._total_len *= s
        self._chunks = []
        # Initialize with single local chunk
        self._chunks.append({
            "start_index": 0,
            "end_index": self._total_len,
            "owner_node": "local",
            "data": [0.0] * self._total_len,
        })

    @classmethod
    def zeros(cls, shape: tuple, dtype: str = "float32") -> "DistributedArray":
        """Create a zero-filled array."""
        return cls(shape=shape, dtype=dtype)

    @classmethod
    def ones(cls, shape: tuple, dtype: str = "float32") -> "DistributedArray":
        """Create a one-filled array."""
        arr = cls(shape=shape, dtype=dtype)
        arr._chunks[0]["data"] = [1.0] * arr._total_len
        return arr

    @classmethod
    def distributed(cls, shape: tuple, dtype: str, node_ids: List[str]) -> "DistributedArray":
        """Create a distributed array split across multiple nodes."""
        arr = cls(shape=shape, dtype=dtype)
        total_len = arr._total_len
        chunk_size = (total_len + len(node_ids) - 1) // len(node_ids)
        
        arr._chunks = []
        for i, node_id in enumerate(node_ids):
            start = i * chunk_size
            end = min(start + chunk_size, total_len)
            if start < total_len:
                arr._chunks.append({
                    "start_index": start,
                    "end_index": end,
                    "owner_node": node_id,
                    "data": [0.0] * (end - start),
                })
        return arr

    def _find_chunk(self, index: int) -> Optional[tuple]:
        """Find which chunk contains the given flat index."""
        for i, chunk in enumerate(self._chunks):
            if chunk["start_index"] <= index < chunk["end_index"]:
                return (i, index - chunk["start_index"])
        return None

    def get(self, index: int) -> Optional[float]:
        """Get a value at the flat index."""
        result = self._find_chunk(index)
        if result is None:
            return None
        
        chunk_idx, local_idx = result
        chunk = self._chunks[chunk_idx]
        
        if chunk["owner_node"] == "local":
            return chunk["data"][local_idx]
        
        # Remote chunk - try to fetch via IPC
        if self._client is not None:
            page_id = index // 4096
            pool_id = f"array_{self.dtype}"
            try:
                data = self._client.read_remote_memory_page(pool_id, page_id, chunk["owner_node"])
                # Parse value from page data (8 bytes per float64)
                offset = (index % 4096) * 8
                if offset + 8 <= len(data):
                    return struct.unpack('<d', data[offset:offset+8])[0]
            except Exception as e:
                print(f"Warning: Failed to fetch remote chunk: {e}")
        return None

    def set(self, index: int, value: float) -> bool:
        """Set a value at the flat index."""
        result = self._find_chunk(index)
        if result is None:
            return False
        
        chunk_idx, local_idx = result
        chunk = self._chunks[chunk_idx]
        
        if chunk["owner_node"] == "local":
            chunk["data"][local_idx] = value
            return True
        
        # Remote chunk - try to write via IPC
        if self._client is not None:
            page_id = index // 4096
            pool_id = f"array_{self.dtype}"
            try:
                offset = (index % 4096) * 8
                data = bytearray(4096)
                struct.pack_into('<d', data, offset, value)
                self._client.write_remote_memory_page(pool_id, page_id, bytes(data), chunk["owner_node"])
                return True
            except Exception as e:
                print(f"Warning: Failed to write remote chunk: {e}")
        return False

    def len(self) -> int:
        """Get the total number of elements."""
        return self._total_len

    def num_chunks(self) -> int:
        """Get the number of chunks."""
        return len(self._chunks)

    def owner_at(self, index: int) -> Optional[str]:
        """Get the owner node for a given index."""
        result = self._find_chunk(index)
        if result is None:
            return None
        return self._chunks[result[0]]["owner_node"]

    def __repr__(self) -> str:
        return f"DistributedArray(shape={self.shape}, dtype={self.dtype!r}, len={self._total_len}, chunks={len(self._chunks)})"

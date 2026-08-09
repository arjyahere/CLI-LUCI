"""
Tests for Luci Python SDK memory module.
"""

import pytest
from luci.memory import MemoryPool, DistributedArray


class TestMemoryPool:
    """Tests for MemoryPool class."""

    def test_memory_pool_repr(self):
        """Test MemoryPool string representation."""
        pool = MemoryPool(client=None, pool_id="test-123", name="test-pool", size_gb=8.0)
        assert "test-123" in repr(pool)
        assert "test-pool" in repr(pool)
        assert "8.0" in repr(pool)


class TestDistributedArray:
    """Tests for DistributedArray class."""

    def test_zeros(self):
        """Test creating zero-filled array."""
        arr = DistributedArray.zeros((10,), dtype="float64")
        assert arr.shape == (10,)
        assert arr.dtype == "float64"
        assert arr.len() == 10
        assert arr.num_chunks() == 1

    def test_ones(self):
        """Test creating one-filled array."""
        arr = DistributedArray.ones((5,), dtype="float32")
        assert arr.shape == (5,)
        assert arr.len() == 5
        # All values should be 1.0
        for i in range(5):
            assert arr.get(i) == 1.0

    def test_get_set_local(self):
        """Test get/set on local chunks."""
        arr = DistributedArray.zeros((10,), dtype="float64")
        arr.set(0, 42.0)
        arr.set(5, 99.0)
        assert arr.get(0) == 42.0
        assert arr.get(5) == 99.0
        assert arr.get(1) == 0.0

    def test_distributed(self):
        """Test creating distributed array."""
        arr = DistributedArray.distributed((100,), "float64", ["node1", "node2"])
        assert arr.len() == 100
        assert arr.num_chunks() == 2
        assert arr.owner_at(0) == "node1"
        assert arr.owner_at(50) == "node2"

    def test_repr(self):
        """Test string representation."""
        arr = DistributedArray.zeros((10, 20), dtype="float32")
        assert "(10, 20)" in repr(arr)
        assert "float32" in repr(arr)

    def test_out_of_bounds(self):
        """Test accessing out of bounds index."""
        arr = DistributedArray.zeros((5,), dtype="float64")
        assert arr.get(10) is None
        assert arr.set(10, 1.0) is False

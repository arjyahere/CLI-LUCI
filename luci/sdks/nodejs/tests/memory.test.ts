import { DistributedArray, MemoryPool } from '../src/memory';

describe('DistributedArray', () => {
  test('zeros creates array with correct shape', () => {
    const arr = DistributedArray.zeros([10], 'float64');
    expect(arr.shape).toEqual([10]);
    expect(arr.dtype).toBe('float64');
    expect(arr.length).toBe(10);
    expect(arr.numChunks).toBe(1);
  });

  test('ones creates array with all ones', async () => {
    const arr = DistributedArray.ones([5], 'float32');
    expect(arr.shape).toEqual([5]);
    expect(arr.length).toBe(5);
    
    for (let i = 0; i < 5; i++) {
      expect(await arr.get(i)).toBe(1.0);
    }
  });

  test('get/set works on local chunks', async () => {
    const arr = DistributedArray.zeros([10], 'float64');
    await arr.set(0, 42.0);
    await arr.set(5, 99.0);
    
    expect(await arr.get(0)).toBe(42.0);
    expect(await arr.get(5)).toBe(99.0);
    expect(await arr.get(1)).toBe(0.0);
  });

  test('distributed creates array with multiple chunks', () => {
    const arr = DistributedArray.distributed([100], 'float64', ['node1', 'node2']);
    expect(arr.length).toBe(100);
    expect(arr.numChunks).toBe(2);
    expect(arr.ownerAt(0)).toBe('node1');
    expect(arr.ownerAt(50)).toBe('node2');
  });

  test('toString returns correct string', () => {
    const arr = DistributedArray.zeros([10, 20], 'float32');
    const str = arr.toString();
    expect(str).toContain('[10,20]');
    expect(str).toContain('float32');
  });

  test('out of bounds returns null', async () => {
    const arr = DistributedArray.zeros([5], 'float64');
    expect(await arr.get(10)).toBeNull();
    expect(await arr.set(10, 1.0)).toBe(false);
  });
});

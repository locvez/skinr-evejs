import { describe, expect, it, vi } from 'vitest';
import { createSofSource } from './sof-source';

const ROOT = 'res:/dx9/model/spaceobjectfactory/';

describe('SOF catalogue fallback', () => {
  it('reads indexed records directly without loading the complete catalogue', async () => {
    const read = vi.fn().mockResolvedValue(new Uint8Array([1]));
    const catalogue = vi.fn();
    const source = createSofSource(() => true, read, catalogue);
    expect(await source(`${ROOT}factions/caldaribase.black`)).toEqual(new Uint8Array([1]));
    expect(catalogue).not.toHaveBeenCalled();
  });

  it('recovers exact missing factions and patterns in one shared catalogue read', async () => {
    const faction = { name: 'halloween_19_duvolle', colorSet: { primary: 'green' } };
    const pattern = { name: 'igc_xx_edencomm', projections: ['original projection'] };
    const catalogue = vi.fn().mockResolvedValue({ faction: [faction], pattern: [pattern] });
    const source = createSofSource(() => false, vi.fn(), catalogue);
    const [a, b] = await Promise.all([
      source(`${ROOT}factions/halloween_19_duvolle.black`),
      source(`${ROOT}patterns/igc_xx_edencomm.black`),
    ]);
    expect(a).toEqual({ object: faction });
    expect(b).toEqual({ object: pattern });
    expect(catalogue).toHaveBeenCalledTimes(1);
    await expect(source(`${ROOT}patterns/unknown.black`)).rejects.toThrow(
      'Not in the client SOF catalogue',
    );
  });

  it('reports a failed catalogue read and permits a later retry', async () => {
    const catalogue = vi
      .fn()
      .mockRejectedValueOnce(new Error('HTTP 503'))
      .mockResolvedValue({
        faction: [{ name: 'halloween_19_duvolle' }],
      });
    const source = createSofSource(() => false, vi.fn(), catalogue);
    await expect(source(`${ROOT}factions/halloween_19_duvolle.black`)).rejects.toThrow('HTTP 503');
    await expect(source(`${ROOT}factions/halloween_19_duvolle.black`)).resolves.toEqual({
      object: { name: 'halloween_19_duvolle' },
    });
    expect(catalogue).toHaveBeenCalledTimes(2);
  });
});

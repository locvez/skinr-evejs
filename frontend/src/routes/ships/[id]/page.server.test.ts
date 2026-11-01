import { describe, expect, it } from 'vitest';
import { load } from './+page.server';

const designs = { slot_materials: {}, designs: [{ id: 'design' }], components: {} };

async function loadShip(skinrs: Response | Error) {
  const fetch = async (path: string | URL | Request) => {
    if (String(path).endsWith('/evejs-target')) return Response.json(null);
    if (String(path).endsWith('/skinrs')) {
      if (skinrs instanceof Error) throw skinrs;
      return skinrs;
    }
    return Response.json({ id: 601, name: 'Ibis', skins: [] });
  };
  return load({ fetch, params: { id: '601' } } as Parameters<typeof load>[0]);
}

describe('ship SKINR loading', () => {
  it('passes designs through to the page', async () => {
    const result = await loadShip(Response.json(designs));
    expect(result).toMatchObject({ skinrs: designs, skinrsUnavailable: false });
  });

  it('distinguishes an empty catalogue from a failed load', async () => {
    const empty = { ...designs, designs: [] };
    expect(await loadShip(Response.json(empty))).toMatchObject({
      skinrs: empty,
      skinrsUnavailable: false,
    });
    expect(await loadShip(new Response('Unavailable', { status: 503 }))).toMatchObject({
      ship: { id: 601 },
      skinrs: empty,
      skinrsUnavailable: true,
    });
    expect(await loadShip(new Error('Connection refused'))).toMatchObject({
      skinrsUnavailable: true,
    });
  });
});

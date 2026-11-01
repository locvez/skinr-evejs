-- Reimport SDE overrides with valid `none` sentinels instead of empty arguments.
delete from sde_meta where key = 'seeded_build';

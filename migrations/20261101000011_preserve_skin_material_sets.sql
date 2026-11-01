-- A skin can keep the base faction while changing materials and patterns.
alter table ship_skins add column dna_suffix text not null default '';

-- Reimport the cached SDE on the next bootstrap, even if its build is unchanged.
delete from sde_meta where key = 'seeded_build';

-- Imported designs remain viewable independently of current marketplace listings.
create table skinr_imports (
    skinr_id text primary key references skinr_designs(id),
    source_file text not null,
    imported_at timestamptz not null default now()
);

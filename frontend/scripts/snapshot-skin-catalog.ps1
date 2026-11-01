# Run from the repository root before the Node audit scripts. Reads only from Docker.
$ErrorActionPreference = 'Stop'
$auditRepo = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
Push-Location $auditRepo
try {
    $auditStorage = Join-Path $auditRepo 'storage'
    $auditSde = Join-Path $auditStorage 'audit-sde'
    New-Item -ItemType Directory -Force $auditSde | Out-Null
    $auditImporter = docker compose ps -aq sde-import
    if ($LASTEXITCODE -ne 0 -or -not $auditImporter) {
        throw 'Start the local stack with docker compose up -d first.'
    }
    foreach ($auditFile in @('skins', 'skinMaterials', 'graphicMaterialSets')) {
        docker cp "${auditImporter}:/app/storage/sde/${auditFile}.jsonl" (Join-Path $auditSde "${auditFile}.jsonl")
        if ($LASTEXITCODE -ne 0) { throw "Could not copy $auditFile" }
    }
    $auditSql = @'
select json_build_object(
    'missing_active_designs', (select count(distinct l.skinr_id)
        from skinr_listings l left join skinr_designs d on d.id=l.skinr_id
        where l.state='listed' and l.expires>now() and d.id is null),
    'unknown_ship_designs', (select count(distinct d.id)
        from skinr_designs d join skinr_listings l on l.skinr_id=d.id
        left join ships s on s.id=d.ship_type_id
        where l.state='listed' and l.expires>now() and s.id is null),
    'ships', (select json_agg(s) from (select id,name,faction_id,race_id,sof_faction from ships) s),
    'designs', (select json_agg(d) from (
        select d.id,d.ship_type_id,count(l.id) listings,coalesce(sum(l.quantity),0) quantity,
            min(l.price_plex) min_price_plex, i.skinr_id is not null imported
        from skinr_designs d
        left join skinr_listings l on l.skinr_id=d.id and l.state='listed' and l.expires>now()
        left join skinr_imports i on i.skinr_id=d.id
        where l.id is not null or i.skinr_id is not null group by d.id,i.skinr_id
    ) d)
);
'@
    $auditSnapshot = docker compose exec -T postgres psql -U skinr -d skinr -At -c $auditSql
    if ($LASTEXITCODE -ne 0) { throw 'Could not read the catalogue snapshot' }
    $auditSnapshot | Set-Content -LiteralPath (Join-Path $auditStorage 'catalog-db.json') -Encoding utf8
    Write-Output 'Snapshot saved. In frontend/, run node scripts/audit-skins.ts, then node scripts/audit-skin-dna.ts and node scripts/audit-skin-renders.mjs.'
}
finally {
    Pop-Location
}

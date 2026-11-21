-- Tilde in a schema of its own beside another application's tables, as hosted Tilde Cloud on
-- tilde-api's database (DATABASE_URL `options=-c search_path=<schema>,public`): the search
-- path's first schema is created when missing, so migrations, grants to tilde_tenant and the
-- ledger all land in it. The default path ("$user", public) creates nothing.
DO $$
DECLARE
    first TEXT := btrim(split_part(current_setting('search_path'), ',', 1), ' "');
BEGIN
    IF first <> '' AND first <> '$user' AND current_schema() IS DISTINCT FROM first THEN
        EXECUTE format('CREATE SCHEMA IF NOT EXISTS %I', first);
    END IF;
END $$;

--: Record()

--! run (p1) : Record
SELECT toolset_hash FROM connection_tool_discovery WHERE connection_id=:p1 FOR UPDATE;

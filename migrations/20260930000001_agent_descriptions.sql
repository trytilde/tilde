-- A short free-text summary of what an agent does, searched by the registry alongside its name.
ALTER TABLE agents ADD COLUMN description TEXT NOT NULL DEFAULT '' CHECK (length(description) <= 500);

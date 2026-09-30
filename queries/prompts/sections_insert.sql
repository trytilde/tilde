--! run (p1, p2, p3, p4)
INSERT INTO prompt_sections(version_id,name,hash,content)
SELECT :p1,s.name,s.hash,s.content FROM UNNEST(:p2::TEXT[],:p3::BYTEA[],:p4::TEXT[]) AS s(name,hash,content);

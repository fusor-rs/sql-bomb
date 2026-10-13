SELECT
    namespace.nspname AS schema_name,
    class.relname AS table_name,
    CASE class.relkind
        WHEN 'v' THEN 'VIEW'
        WHEN 'm' THEN 'MATERIALIZED VIEW'
        WHEN 'f' THEN 'FOREIGN TABLE'
        ELSE 'TABLE'
    END AS table_type,
    array_agg(attribute.attname ORDER BY attribute.attnum) AS column_names,
    array_agg(attribute.atttypid ORDER BY attribute.attnum) AS column_types,
    array_agg(NOT attribute.attnotnull ORDER BY attribute.attnum) AS nullable
FROM pg_catalog.pg_class AS class
JOIN pg_catalog.pg_namespace AS namespace ON namespace.oid = class.relnamespace
JOIN pg_catalog.pg_attribute AS attribute ON attribute.attrelid = class.oid
WHERE class.relkind IN ('r', 'p', 'v', 'm', 'f')
    AND attribute.attnum > 0
    AND NOT attribute.attisdropped
    AND namespace.nspname NOT IN ('pg_catalog', 'information_schema')
    AND namespace.nspname NOT LIKE 'pg\_%'
GROUP BY namespace.nspname, class.relname, class.relkind
ORDER BY namespace.nspname, class.relname

-- The truth of this example (workspace ADR 0010): the schema as it is now,
-- in one plain `.sql` file. `rok gen` reads these files; the staged
-- migration pairs beside them are the history — how the schema got here.
--
-- The file names carry no version: the version belongs to the migration.

-- The people who wrote the books.
CREATE TABLE authors (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    email TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- The books themselves, one author each.
CREATE TABLE books (
    id UUID PRIMARY KEY,
    author_id UUID NOT NULL REFERENCES authors (id),
    title TEXT NOT NULL,
    isbn TEXT NOT NULL UNIQUE,
    published_at DATE
);

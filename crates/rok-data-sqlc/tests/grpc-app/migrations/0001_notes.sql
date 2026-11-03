-- The schema of the gRPC fixture: the axum fixture's accounts, notes and
-- labels (the notes tenant-scoped, versioned and soft-deleted), and the
-- gadgets, whose columns hold every type a message carries.

CREATE TABLE accounts (
    id UUID PRIMARY KEY,
    email TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL
);

CREATE TABLE notes (
    id UUID PRIMARY KEY,
    owner_id UUID NOT NULL REFERENCES accounts (id),
    tenant_id UUID NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    revision BIGINT NOT NULL DEFAULT 1,
    deleted_at TIMESTAMPTZ
);

CREATE TABLE labels (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL UNIQUE
);

CREATE TABLE note_labels (
    note_id UUID NOT NULL REFERENCES notes (id),
    label_id BIGINT NOT NULL REFERENCES labels (id),
    PRIMARY KEY (note_id, label_id)
);

CREATE TYPE mood AS ENUM ('happy', 'very sad');

CREATE TABLE gadgets (
    id BIGSERIAL PRIMARY KEY,
    code UUID NOT NULL,
    small SMALLINT NOT NULL,
    count INTEGER NOT NULL,
    big BIGINT,
    ratio REAL NOT NULL,
    score DOUBLE PRECISION NOT NULL,
    price NUMERIC(10, 2) NOT NULL,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    blob BYTEA NOT NULL,
    made_on DATE NOT NULL,
    seen_at TIMESTAMP NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    meta JSONB NOT NULL,
    extra JSON,
    mood mood NOT NULL,
    tags TEXT[] NOT NULL,
    aliases TEXT[],
    nickname TEXT
);

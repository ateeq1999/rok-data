-- The second file of the truth: the loans — who has which book, and since
-- when. It adds the table the books of the other file are lent from, and
-- the foreign key that wires the two files together.

CREATE TABLE loans (
    id UUID PRIMARY KEY,
    book_id UUID NOT NULL REFERENCES books (id),
    borrower TEXT NOT NULL,
    lent_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    returned_at TIMESTAMPTZ
);

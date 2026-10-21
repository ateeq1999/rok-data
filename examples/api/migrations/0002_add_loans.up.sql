CREATE TABLE "loans" (
    "id" UUID NOT NULL,
    "book_id" UUID NOT NULL,
    "borrower" TEXT NOT NULL,
    "lent_at" TIMESTAMPTZ NOT NULL DEFAULT now(),
    "returned_at" TIMESTAMPTZ,
    CONSTRAINT "loans_pkey" PRIMARY KEY ("id")
);
ALTER TABLE "loans" ADD CONSTRAINT "loans_book_id_fkey" FOREIGN KEY ("book_id") REFERENCES "books" ("id");

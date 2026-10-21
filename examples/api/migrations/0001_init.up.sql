CREATE TABLE "authors" (
    "id" UUID NOT NULL,
    "name" TEXT NOT NULL,
    "email" TEXT NOT NULL,
    "created_at" TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT "authors_pkey" PRIMARY KEY ("id"),
    CONSTRAINT "authors_email_key" UNIQUE ("email")
);
CREATE TABLE "books" (
    "id" UUID NOT NULL,
    "author_id" UUID NOT NULL,
    "title" TEXT NOT NULL,
    "isbn" TEXT NOT NULL,
    "published_at" DATE,
    CONSTRAINT "books_pkey" PRIMARY KEY ("id"),
    CONSTRAINT "books_isbn_key" UNIQUE ("isbn")
);
ALTER TABLE "books" ADD CONSTRAINT "books_author_id_fkey" FOREIGN KEY ("author_id") REFERENCES "authors" ("id");

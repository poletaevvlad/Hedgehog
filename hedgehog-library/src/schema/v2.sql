CREATE TABLE groups (
    "id" INTEGER NOT NULL PRIMARY KEY,
    "name" TEXT NOT NULL,
    "ordering" INTEGER NOT NULL
);

ALTER TABLE feeds ADD COLUMN group_id INTEGER REFERENCES groups("id") ON DELETE SET NULL;
ALTER TABLE feeds ADD COLUMN title_override TEXT;

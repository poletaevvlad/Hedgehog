PRAGMA foreign_keys = ON;

CREATE TABLE feeds (
    "title" TEXT,
    "description" TEXT,
    "link" TEXT,
    "author" TEXT,
    "copyright" TEXT,
    "status" INTEGER DEFAULT 0 NOT NULL,
    "error_code" INTEGER,
    "source" TEXT NOT NULL
);

CREATE TABLE episodes (
    "feed_id" INTEGER NOT NULL,
    "guid" TEXT NOT NULL,
    "title" TEXT,
    "description" TEXT,
    "link" TEXT,
    "duration" INTEGER,
    "publication_date" TEXT,
    "episode_number" INTEGER,
    "media_url" TEXT NOT NULL,
    "is_new" INTEGER,
    "is_finished" INTEGER,
    "position" INTEGER,
    "error_code" INTEGER,
    FOREIGN KEY("feed_id") REFERENCES feeds("rowid")
);

CREATE UNIQUE INDEX episodes_feed_id_guid_index ON episodes ("feed_id", "guid");

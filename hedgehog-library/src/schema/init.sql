CREATE TABLE feeds (
    "id" INTEGER NOT NULL PRIMARY KEY,
    "title" TEXT,
    "description" TEXT,
    "link" TEXT,
    "author" TEXT,
    "copyright" TEXT,
    "status" INTEGER DEFAULT 0 NOT NULL,
    "error_code" INTEGER DEFAULT 0,
    "source" TEXT NOT NULL,
    "enabled" INTEGER DEFAULT 1 NOT NULL
);

CREATE TABLE episodes (
    "id" INTEGER NOT NULL PRIMARY KEY,
    "feed_id" INTEGER NOT NULL,
    "guid" TEXT NOT NULL,
    "title" TEXT,
    "description" TEXT,
    "link" TEXT,
    "duration" INTEGER,
    "publication_date" TEXT,
    "episode_number" INTEGER,
    "media_url" TEXT NOT NULL,
    "status" INTEGER NOT NULL DEFAULT 0,
    "position" INTEGER NOT NULL DEFAULT 0,
    "error_code" INTEGER,
    FOREIGN KEY("feed_id") REFERENCES feeds("id") ON DELETE CASCADE
);

CREATE UNIQUE INDEX episodes_feed_id_guid_index ON episodes ("feed_id", "guid");

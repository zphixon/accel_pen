CREATE TABLE ap_user (
    ap_user_id SERIAL PRIMARY KEY,
    nadeo_display_name TEXT NOT NULL,
    nadeo_account_id TEXT UNIQUE NOT NULL,
    nadeo_login TEXT UNIQUE NOT NULL,
    nadeo_club_tag TEXT,
    site_admin BOOLEAN NOT NULL DEFAULT FALSE,
    registered TIMESTAMP WITH TIME ZONE
);

CREATE TABLE map (
    ap_map_id SERIAL PRIMARY KEY,
    gbx_mapuid TEXT UNIQUE NOT NULL,
    map_name TEXT NOT NULL,
    votes INTEGER NOT NULL DEFAULT 1,
    uploaded TIMESTAMP WITH TIME ZONE NOT NULL,
    created TIMESTAMP WITH TIME ZONE NOT NULL,
    author_time INTEGER NOT NULL,
    gold_time INTEGER NOT NULL,
    silver_time INTEGER NOT NULL,
    bronze_time INTEGER NOT NULL
);

CREATE TABLE map_user (
    ap_map_id INTEGER NOT NULL,
    ap_user_id INTEGER NOT NULL,
    is_author BOOLEAN NOT NULL,
    is_uploader BOOLEAN NOT NULL,
    may_manage BOOLEAN NOT NULL,
    may_grant BOOLEAN NOT NULL,
    other TEXT,

    PRIMARY KEY (ap_map_id, ap_user_id),

    CONSTRAINT map_user_ap_map_id_fk
        FOREIGN KEY (ap_map_id) REFERENCES map (ap_map_id)
        ON UPDATE CASCADE ON DELETE CASCADE,

    CONSTRAINT map_user_ap_user_id_fk
        FOREIGN KEY (ap_user_id) REFERENCES ap_user (ap_user_id)
        ON UPDATE CASCADE ON DELETE CASCADE
);

CREATE TABLE map_data (
    ap_map_id INTEGER PRIMARY KEY,
    gbx_data BYTEA NOT NULL,

    CONSTRAINT map_data_ap_map_id_fk
        FOREIGN KEY (ap_map_id) REFERENCES map (ap_map_id)
        ON UPDATE CASCADE ON DELETE CASCADE
);

CREATE TABLE map_thumbnail (
    ap_map_id INTEGER PRIMARY KEY,
    thumbnail BYTEA NOT NULL,
    thumbnail_small BYTEA NOT NULL,

    CONSTRAINT map_thumbnail_ap_map_id_fk
        FOREIGN KEY (ap_map_id) REFERENCES map (ap_map_id)
        ON UPDATE CASCADE ON DELETE CASCADE
);

CREATE TABLE tag (
    tag_id INTEGER UNIQUE NOT NULL PRIMARY KEY,
    tag_name TEXT NOT NULL,
    tag_definition TEXT,
    implication INTEGER
);

CREATE TABLE tag_implies (
    row_id SERIAL PRIMARY KEY,
    implication INTEGER NOT NULL,
    implyer INTEGER NOT NULL,
    implied INTEGER NOT NULL,
    
    CONSTRAINT tag_implies_implyer_fk
        FOREIGN KEY (implyer) REFERENCES tag (tag_id)
        ON UPDATE CASCADE ON DELETE CASCADE,

    CONSTRAINT tag_implies_implied_fk
        FOREIGN KEY (implied) REFERENCES tag (tag_id)
        ON UPDATE CASCADE ON DELETE CASCADE
);

CREATE TABLE map_tag (
    ap_map_id INTEGER NOT NULL,
    tag_id INTEGER NOT NULL,

    PRIMARY KEY (ap_map_id, tag_id),

    CONSTRAINT map_tag_ap_map_id_fk
        FOREIGN KEY (ap_map_id) REFERENCES map (ap_map_id)
        ON UPDATE CASCADE ON DELETE CASCADE,

    CONSTRAINT map_tag_tag_id_fk
        FOREIGN KEY (tag_id) REFERENCES tag (tag_id)
        ON UPDATE CASCADE ON DELETE CASCADE
);

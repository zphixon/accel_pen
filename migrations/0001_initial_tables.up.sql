CREATE TABLE ap_user (
    user_id SERIAL UNIQUE NOT NULL,
    display_name TEXT NOT NULL,
    account_id TEXT UNIQUE NOT NULL,
    site_admin BOOLEAN NOT NULL DEFAULT FALSE,
    registered TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT (timezone('utc', now())),

    CONSTRAINT pk_user PRIMARY KEY (user_id)
);

CREATE TABLE map (
    ap_id SERIAL UNIQUE NOT NULL,
    gbx_mapuid TEXT UNIQUE NOT NULL,
    gbx_data BYTEA NOT NULL,
    mapname TEXT NOT NULL,
    author INTEGER NOT NULL,
    votes INTEGER NOT NULL DEFAULT 1,
    uploaded TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT (timezone('utc', now())),
    created TIMESTAMP WITH TIME ZONE NOT NULL,
    thumbnail BYTEA NOT NULL,

    CONSTRAINT pk_map PRIMARY KEY (ap_id, gbx_mapuid),
    CONSTRAINT fk_map_author FOREIGN KEY (author)
        REFERENCES ap_user (user_id)
        ON UPDATE CASCADE ON DELETE CASCADE
);

CREATE TABLE vote (
    user_id INTEGER,
    ap_id INTEGER,
    vote_value SMALLINT NOT NULL,
    cast_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT (timezone('utc', now())),

    CONSTRAINT pk_vote PRIMARY KEY (user_id, ap_id),
    CONSTRAINT fk_vote_user FOREIGN KEY (user_id)
        REFERENCES ap_user (user_id)
        ON UPDATE CASCADE ON DELETE CASCADE,
    CONSTRAINT fk_vote_map FOREIGN KEY (ap_id)
        REFERENCES map (ap_id)
        ON UPDATE CASCADE ON DELETE CASCADE
);

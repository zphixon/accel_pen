CREATE TABLE user (
    user_id INTEGER UNSIGNED AUTO_INCREMENT,
    display_name VARCHAR(128) NOT NULL,
    account_id CHAR(36) UNIQUE NOT NULL,
    site_admin BOOLEAN NOT NULL DEFAULT FALSE,
    registered TIMESTAMP NOT NULL,

    PRIMARY KEY (user_id)
);

CREATE TABLE map (
    ap_id INTEGER UNSIGNED AUTO_INCREMENT,
    gbx_mapuid VARCHAR(128) NOT NULL,
    gbx_data MEDIUMBLOB NOT NULL,
    mapname TEXT NOT NULL,
    author INTEGER UNSIGNED NOT NULL,
    votes INTEGER NOT NULL DEFAULT 1,
    uploaded TIMESTAMP NOT NULL,
    created TIMESTAMP NOT NULL,

    PRIMARY KEY (ap_id),
    FOREIGN KEY (author)
        REFERENCES user (user_id)
        ON UPDATE CASCADE ON DELETE CASCADE
);

CREATE TABLE vote (
    user_id INTEGER UNSIGNED,
    map_ap_id INTEGER UNSIGNED,
    vote_value TINYINT NOT NULL,
    cast TIMESTAMP NOT NULL,

    PRIMARY KEY (user_id, map_ap_id),
    FOREIGN KEY (user_id)
        REFERENCES user (user_id)
        ON UPDATE CASCADE ON DELETE CASCADE,
    FOREIGN KEY (map_ap_id)
        REFERENCES map (ap_id)
        ON UPDATE CASCADE ON DELETE CASCADE
);
